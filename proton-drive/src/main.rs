use async_recursion::async_recursion;
use log::*;
use proton_sdk_rs::{
    drive::{DriveClient, DriveClientBuilder}, observability::OptionalObservability, sessions::{SessionBuilder, SessionPlatform}, utils, AddressKeyRegistrationRequest, ClientId, NodeIdentity, ProtonDriveClientCreateRequest, VolumeMetadata
};
use tokio::time::timeout;
use std::{
    env,
    io::{self, Write}, thread, time::Duration,
};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    if let Err(_) = dotenv::dotenv() {
        let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let env_path = workspace_root.join(".env");
        dotenv::from_path(env_path).ok();
    }

    if let Ok(log_level) = env::var("RUST_LOG") {
        env_logger::init_from_env(env_logger::Env::default().default_filter_or(&log_level));
    } else {
        env_logger::init();
        warn!("No RUST_LOG environment variable found. Setting default log level.");
    }

    let username =
        env::var("PROTON_USERNAME").expect("You must provide a username in the .env file");
    let password =
        env::var("PROTON_PASSWORD").expect("You must provide a password in the .env file");
    
    let censor = |input: &String, censor: char| {
        let mut temp = String::new();
        for len in 0..input.len()-2 {
            temp.push(censor);
        }
        temp
    };

    debug!("Creating session for user: {}", username);
    debug!(
        "Using credentials: username={}, password={}chars",
        format!("{}{}{}", username.chars().next().unwrap(), censor(&username, '*'), username.chars().last().unwrap()),
        password.len()
    );

    let session_result = SessionBuilder::new(username, password)
        // .with_app_version(SessionPlatform::Windows, "proton-drive-rs", "1.0.0")
        .with_rclone_app_version_spoof()
        .with_request_response_callback(|data| {
            let data_str = String::from_utf8_lossy(data);
            trace!("HTTP: {} bytes", data.len());
            if data.len() < 500 {
                trace!("   Content: {}", data_str);
            } else {
                trace!("   Content (truncated): {}...", &data_str[..200]);
            }
        })
        .with_secret_requested_callback(|| {
            print!("2FA/Secret required. Enter 'y' to continue: ");
            io::stdout().flush().unwrap();
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            let result = input.trim().to_lowercase() == "y";
            trace!("   Secret callback returning: {}", result);
            result
        })
        .with_tokens_refreshed_callback(|tokens| {
            std::fs::write("tokens.cache", tokens).ok();
            println!("Authentication tokens refreshed: {} bytes", tokens.len());
            let tokens_str = String::from_utf8_lossy(tokens);
            if tokens_str.is_ascii() && tokens_str.len() < 200 {
                trace!("   Tokens: {}", tokens_str);
            }
        })
        .begin()
        .await;

    let session = match session_result {
        Ok(session) => {
            info!("Session created successfully!");
            debug!("Session handle: {:?}", session.handle());
            session
        }
        Err(e) => {
            println!("Failed to create session: {}", e);

            match e {
                proton_sdk_rs::sessions::SessionError::SdkError(sdk_err) => {
                    error!("SDK Error Details: {}", sdk_err);
                }
                proton_sdk_rs::sessions::SessionError::OperationFailed(code) => {
                    error!("SDK operation failed with code: {}", code);
                    match code {
                        -1 => error!(
                            "   Possible causes: Invalid credentials, network issues, or SDK not initialized"
                        ),
                        401 => println!("   Authentication failed - check username/password"),
                        403 => println!("   Access forbidden - account may be locked or suspended"),
                        422 => println!("   Invalid request format"),
                        _ => println!("   Unknown error code: {}", code),
                    }
                }
                proton_sdk_rs::sessions::SessionError::ProtobufError(proto_err) => {
                    error!("Protobuf Error: {}", proto_err);
                }
                _ => {
                    error!("Other error: {}", e);
                }
            }
            panic!("Failed to create session");
        }
    };

    info!("Creating observability");
    let obs = OptionalObservability::enabled(session.handle())?;
    info!("Observability handle: {:?}", obs.handle());

    info!("Creating Drive client");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let create_request = ProtonDriveClientCreateRequest {
        client_id: Some(ClientId {
            value: "proton-sdk-rs".to_string(),
        }),
    };
    info!("Request: {:?}", create_request);

    let client = match DriveClientBuilder::new(session)
        .with_observability(obs.handle())
        .with_request(create_request)
        .build()
    {
        Ok(cli) => {
            info!("Drive client created {:?}", cli.handle());
            cli
        }
        Err(e) => anyhow::bail!(e),
    };

    let volumes = client.get_volumes().await?;
    for volume in &volumes {
        println!("{:?}", volume);
    }

    let main_volume = &volumes[0];

    let share = client.get_shares(main_volume).await?;

    log::info!("Share information: {:?}", share);

    let list = recursive_list_file_root(&client, NodeIdentity { 
        node_id: share.root_node_id.clone(), 
        share_id: share.share_id.clone(), 
        volume_id: main_volume.volume_id.clone()
    }, "".to_string()).await?;

    for item in list {
        log::info!("{}", item)
    }

    Ok(())
}

#[async_recursion]
async fn recursive_list_file_root(
    client: &DriveClient,
    identity: NodeIdentity,
    parent_folder: String,
) -> anyhow::Result<Vec<String>> {
    let mut files = Vec::new();

    let children = client.get_folder_children(identity.clone()).await?;

    for child in children {
        let (is_folder, folder) = utils::node_is_folder(child.clone());
        if is_folder {
            if let Some(folder) = folder {
                let new_identity = NodeIdentity {
                    node_id: folder.node_identity.as_ref().and_then(|ni| ni.node_id.clone()),
                    share_id: folder.node_identity.as_ref().and_then(|ni| ni.share_id.clone()).or(identity.share_id.clone()),
                    volume_id: folder.node_identity.as_ref().and_then(|ni| ni.volume_id.clone()).or(identity.volume_id.clone()),
                };
                let folder_name = folder.name.clone();
                let new_parent = if parent_folder.is_empty() {
                    folder_name
                } else {
                    format!("{}/{}", parent_folder, folder_name)
                };
                // Recurse into the folder
                let mut sub_files = recursive_list_file_root(client, new_identity, new_parent).await?;
                files.append(&mut sub_files);
            }
        } else {
            let (is_file, file) = utils::node_is_file(child);
            if is_file {
                if let Some(file) = file {
                    let file_name = if parent_folder.is_empty() {
                        file.name.clone()
                    } else {
                        format!("{}/{}", parent_folder, file.name)
                    };
                    files.push(file_name);
                }
            }
        }
    }

    Ok(files)
}
