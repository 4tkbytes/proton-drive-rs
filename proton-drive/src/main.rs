use async_recursion::async_recursion;
use chrono::Utc;
use log::*;
use proton_sdk_rs::{
    downloads::DownloaderBuilder, drive::{DriveClient, DriveClientBuilder}, observability::OptionalObservability, sessions::{Session, SessionBuilder, SessionCallbacks, SessionPlatform}, utils, AddressKeyRegistrationRequest, ClientId, FileDownloadRequest, FromByteArray, NodeIdentity, OperationIdentifier, OperationType, ProtonClientOptions, ProtonDriveClientCreateRequest, RevisionMetadata, SessionInfo, SessionRenewRequest, SessionResumeRequest, SessionTokens, ToByteArray, VolumeMetadata
};
use proton_sdk_sys::{logger, prost::bytes::buf};
use tokio::time::timeout;
use uuid::Uuid;
use std::{
    env, fs::File, io::{self, Read, Write}, thread, time::Duration
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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

    let mut is_tokens = false;

    let mut tokens = match File::open("token.bin") {
        Ok(value) => {
            let mut file = File::open("token.bin").expect("Failed to open tokens.bin");
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer).expect("Reading buffer error");
            is_tokens = true;
            SessionInfo::from_bytes(&buffer)?
        },
        Err(_) => {log::warn!("No previous session found"); SessionInfo::default()}
    };

    let request = SessionResumeRequest { 
        session_id: tokens.session_id, 
        username: username.clone(), 
        user_id: tokens.user_id, 
        access_token: tokens.access_token, 
        refresh_token: tokens.refresh_token, 
        scopes: tokens.scopes, 
        is_waiting_for_second_factor_code: tokens.is_waiting_for_second_factor_code, 
        password_mode: tokens.password_mode, 
        options: Some(ProtonClientOptions { 
            app_version: "macos-drive@1.0.0-alpha.1+proton-sdk-sys".to_string(), 
            ..Default::default()
        })
    };

    let mut session = None;

    if is_tokens {
        // renew session
        let session_result = SessionBuilder::resume_session(request, SessionCallbacks {
            request_response: Some(Box::new(|data| {
                    let data_str = String::from_utf8_lossy(data);
                    trace!("HTTP: {} bytes", data.len());
                    if data.len() < 500 {
                        trace!("   Content: {}", data_str);
                    } else {
                        trace!("   Content (truncated): {}...", &data_str[..200]);
                    }
                })),
            secret_requested: None,
            tokens_refreshed: Some(Box::new(|data: &[u8]| {
                let mut file = File::create("tokens.bin").unwrap();
                file.write_all(data).unwrap();
                debug!("Overrided tokens in tokens.env");
            })),
            two_factor_requested: Some(Box::new(|_closure: &[u8]| {
                print!("Enter 2FA code: ");
                io::stdout().flush().ok();
                let mut code = String::new();
                if io::stdin().read_line(&mut code).is_ok() {
                    let code = code.trim();
                    if !code.is_empty() {
                        Some(proton_sdk_sys::protobufs::StringResponse {
                            value: code.to_string(),
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }))
        })
        .await;
        session = Some(match session_result {
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
        });
    } else {
        // create new session
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
        .with_two_factor_requested_callback(|_context| {
            print!("Enter 2FA code: ");
            io::stdout().flush().ok();
            let mut code = String::new();
            if io::stdin().read_line(&mut code).is_ok() {
                let code = code.trim();
                if !code.is_empty() {
                    Some(proton_sdk_sys::protobufs::StringResponse {
                        value: code.to_string(),
                    })
                } else {
                    None
                }
            } else {
                None
            }
        })
        .with_tokens_refreshed_callback(|data: &[u8]| {
            // let session = SessionTokens::from_bytes(data).unwrap();
            let mut file = File::create("token.bin").unwrap();
            file.write_all(data).unwrap();
            debug!("Overrided tokens in tokens.env");
        })
        .begin()
        .await;
        session = Some(match session_result {
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
        });
    }

    let session = session.unwrap();

    info!("Creating observability");
    let obs = OptionalObservability::enabled(session.handle())?;
    trace!("Observability handle: {:?}", obs.handle());

    info!("Creating Drive client");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let create_request = ProtonDriveClientCreateRequest {
        client_id: Some(ClientId {
            value: "proton-sdk-rs".to_string(),
        }),
    };
    trace!("Request: {:?}", create_request);

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
        trace!("volume metadata: {:?}", volume);
    }

    let main_volume = &volumes[0];

    let share = client.get_shares(main_volume).await?;

    log::trace!("share information: {:?}", share);

    let identity = NodeIdentity { 
        node_id: share.root_node_id.clone(), 
        share_id: share.share_id.clone(), 
        volume_id: main_volume.volume_id.clone()
    };

    let list = recursive_list_file_root(&client, &identity, "".to_string()).await?;

    for item in list {
        log::info!("{}", item)
    }
    let downloader = DownloaderBuilder::new(&client).build().await?;

    let children = client.get_folder_children(identity.clone()).await?;
    for child in children {
        let (is_file, file_node) = utils::node_is_file(child);
        if is_file && file_node.as_ref().unwrap().name == "BadApple.mp4" {
            let file = file_node.as_ref().unwrap();
            let mut file_identity = file.node_identity.clone();
            if let Some(ref mut fi) = file_identity {
                if fi.share_id.is_none() {
                    log::warn!("Missing share id, adding...");
                    fi.share_id = identity.share_id.clone();
                }
                if fi.volume_id.is_none() {
                    log::warn!("Missing volume id, adding...");
                    fi.volume_id = identity.volume_id.clone();
                }
            };
            let revision_info = file.active_revision.as_ref().unwrap();
            let revision = RevisionMetadata { 
                revision_id: revision_info.revision_id.clone(),
                state: revision_info.state,
                manifest_signature: revision_info.manifest_signature.clone(), 
                signature_email_address: revision_info.signature_email_address.clone(), 
                samples_sha256_digests: revision_info.samples_sha256_digests.clone()
            };
            let operation = OperationIdentifier { 
                r#type: OperationType::Download.into(),
                identifier: Uuid::new_v4().to_string(), 
                timestamp: Utc::now().to_rfc3339()
            };
            trace!("share id: {:?}", file.node_identity.as_ref().unwrap().share_id);
            trace!("volume id: {:?}", file.node_identity.as_ref().unwrap().volume_id);
            trace!("node id: {:?}", file.node_identity.as_ref().unwrap().node_id);
            trace!("global share id: {:?}", share.share_id);

            let request = FileDownloadRequest { 
                file_identity, 
                revision_metadata: Some(revision), 
                target_file_path: String::from("C:/Users/thrib/Downloads/BadApple.mp4"), 
                operation_id: Some(operation)
            };
            
            let status = downloader.download_file(
                request,
                 Some(|progress| println!("Progress: {:.1}%", progress * 100.0)),
                &client.session().cancellation_token()
                )
                .await?;
            trace!("downloader status: {:?}", status);
        }
    }

    Ok(())
}

#[async_recursion]
async fn recursive_list_file_root(
    client: &DriveClient,
    identity: &NodeIdentity,
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
                let mut sub_files = recursive_list_file_root(client, &new_identity, new_parent).await?;
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
