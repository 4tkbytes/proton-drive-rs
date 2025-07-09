use proton_sdk_sys::data::Callback;
use std::fs::OpenOptions;
use async_recursion::async_recursion;
use chrono::Utc;
use log::*;
use proton_sdk_rs::{
    downloads::DownloaderBuilder, drive::{DriveClient, DriveClientBuilder}, observability::OptionalObservability, sessions::{SessionBuilder, SessionPlatform}, utils, AddressKeyRegistrationRequest, ClientId, FileDownloadRequest, NodeIdentity, OperationIdentifier, OperationType, ProtonDriveClientCreateRequest, RevisionMetadata, ToByteArray, VolumeMetadata
};
use proton_sdk_sys::logger;
use tokio::time::timeout;
use uuid::Uuid;
use std::{env, fs, io::{self, Write}, thread, time::Duration};
use std::os::windows::prelude::MetadataExt;
use proton_sdk_sys::protobufs::{FileUploadRequest, FileUploaderCreationRequest, ShareMetadata};
use proton_sdk_rs::uploads::UploaderBuilder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {

    if let Err(_) = dotenv::dotenv() {
        let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let env_path = workspace_root.join(".cfg");
        dotenv::from_path(env_path).ok();
    }

    if let Ok(log_level) = env::var("RUST_LOG") {
        env_logger::init_from_env(env_logger::Env::default().default_filter_or(&log_level));
    } else {
        env_logger::init();
        warn!("No RUST_LOG environment variable found. Setting default log level.");
    }

    let username = env::var("PROTON_USERNAME").unwrap_or_else(|_| {
        print!("Enter your Proton username: ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let username = input.trim().to_string();

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(".cfg")
            .unwrap();
        writeln!(file, "PROTON_USERNAME={}", username).unwrap();

        username
    });

    let password = env::var("PROTON_PASSWORD").unwrap_or_else(|_| {
        print!("Enter your Proton password: ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let password = input.trim().to_string();

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(".cfg")
            .unwrap();
        writeln!(file, "PROTON_PASSWORD={}", password).unwrap();

        password
    });

    let session_result = SessionBuilder::new(username, password.clone())
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
        .with_two_factor_requested_callback(move |_context| {
            print!("Enter 2FA code: ");
            io::stdout().flush().ok();
            let mut code = String::new();
            let code_opt = if io::stdin().read_line(&mut code).is_ok() {
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
            };

            let data_pass_opt = match env::var("NO_DATA_PASS").as_deref() {
                Ok("true") => {
                    warn!("Data password not provided, setting as users password");
                    Some(proton_sdk_sys::protobufs::StringResponse {
                        value: password.clone(),
                    })
                }
                _ => {
                    print!("Enter data pass (if any, or leave blank): ");
                    io::stdout().flush().ok();
                    let mut data_pass = String::new();
                    let data_pass_result = if io::stdin().read_line(&mut data_pass).is_ok() {
                        let data_pass = data_pass.trim();
                        let mut file = OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(".cfg")
                            .unwrap();
                        if !data_pass.is_empty() {
                            writeln!(file, "NO_DATA_PASS=false").ok();
                            Some(proton_sdk_sys::protobufs::StringResponse {
                                value: data_pass.to_string(),
                            })
                        } else {
                            writeln!(file, "NO_DATA_PASS=true").ok();
                            Some(proton_sdk_sys::protobufs::StringResponse {
                                value: password.clone(),
                            })
                        }
                    } else {
                        None
                    };
                    data_pass_result
                }
            };

            (code_opt, data_pass_opt)
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

    trace!("share information: {:?}", share);

    let identity = NodeIdentity { 
        node_id: share.root_node_id.clone(), 
        share_id: share.share_id.clone(), 
        volume_id: main_volume.volume_id.clone()
    };

    // // recursive_list_file_root(&client, &identity, "".to_string()).await?;
    // let downloader = DownloaderBuilder::new(&client).build().await?;
    //
    // let children = client.get_folder_children(identity.clone()).await?;
    // for child in children {
    //     let (is_file, file_node) = utils::node_is_file(child);
    //     if is_file && file_node.as_ref().unwrap().name == "BadApple.mp4" {
    //         let file = file_node.as_ref().unwrap();
    //         let mut file_identity = file.node_identity.clone();
    //         if let Some(ref mut fi) = file_identity {
    //             if fi.share_id.is_none() {
    //                 log::warn!("Missing share id, adding...");
    //                 fi.share_id = identity.share_id.clone();
    //             }
    //             if fi.volume_id.is_none() {
    //                 log::warn!("Missing volume id, adding...");
    //                 fi.volume_id = identity.volume_id.clone();
    //             }
    //         };
    //         let revision_info = file.active_revision.as_ref().unwrap();
    //         let revision = RevisionMetadata {
    //             revision_id: revision_info.revision_id.clone(),
    //             state: revision_info.state,
    //             manifest_signature: revision_info.manifest_signature.clone(),
    //             signature_email_address: revision_info.signature_email_address.clone(),
    //             samples_sha256_digests: revision_info.samples_sha256_digests.clone()
    //         };
    //         let operation = OperationIdentifier {
    //             r#type: OperationType::Download.into(),
    //             identifier: Uuid::new_v4().to_string(),
    //             timestamp: Utc::now().to_rfc3339()
    //         };
    //         trace!("share id: {:?}", file.node_identity.as_ref().unwrap().share_id);
    //         trace!("volume id: {:?}", file.node_identity.as_ref().unwrap().volume_id);
    //         trace!("node id: {:?}", file.node_identity.as_ref().unwrap().node_id);
    //         trace!("global share id: {:?}", share.share_id);
    //
    //         let request = FileDownloadRequest {
    //             file_identity,
    //             revision_metadata: Some(revision),
    //             target_file_path: String::from("C:/Users/thrib/Downloads/BadApple.mp4"),
    //             operation_id: Some(operation)
    //         };
    //
    //         let status = downloader.download_file(
    //             request,
    //              Some(|progress| println!("Progress: {:.1}%", progress * 100.0)),
    //             &client.session().cancellation_token()
    //             )
    //             .await?;
    //         trace!("downloader status: {:?}", status);
    //     }
    // }

    // uploading an example file
    let file = "C:/Users/thrib/Downloads/Headquarters.dll";
    let metadata = fs::metadata(file)?;
    let file_size = metadata.file_size();

    let request = FileUploaderCreationRequest {
        file_size: file_size as i64,
        number_of_samples: 0,
    };

    let uploader = UploaderBuilder::new(&client)
        .with_request(request)
        .build()
        .await?;

    let file_path = "C:/Users/thrib/Downloads/Headquarters.dll";
    let metadata = std::fs::metadata(file_path)?;
    let file_name = std::path::Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Headquarters.dll")
        .to_string();

    let operation = OperationIdentifier {
        r#type: OperationType::Download.into(),
        identifier: Uuid::new_v4().to_string(),
        timestamp: Utc::now().to_rfc3339()
    };

    let share_metadata = ShareMetadata {
        share_id: share.share_id.clone(),
        membership_address_id: share.membership_address_id.clone(),
        membership_email_address: share.membership_email_address.clone(),
    };

    let request = FileUploadRequest {
        share_metadata: Some(share_metadata),
        parent_folder_identity: Some(identity),
        name: file_name.clone(),
        mime_type: mime_guess::from_path(file_path).first_or_octet_stream().to_string(),
        source_file_path: file_path.to_string(),
        thumbnail: None,
        last_modification_date: metadata.modified()?.elapsed()?.as_secs() as i64,
        operation_id: Some(operation),
    };

    uploader.upload_file_or_revision(request, Some(move |progress| {
        info!("Uploading file [{}] at progress: {}", file_name, progress * 100.0);
    })).await?;

    Ok(())
}

#[async_recursion]
async fn recursive_list_file_root(
    client: &DriveClient,
    identity: &NodeIdentity,
    parent_folder: String,
) -> anyhow::Result<()> {
    let children = client.get_folder_children(identity.clone()).await?;

    for child in children {
        let (is_folder, folder) = utils::node_is_folder(child.clone());
        if is_folder {
            if let Some(folder) = folder {
                let folder_name = if parent_folder.is_empty() {
                    folder.name.clone()
                } else {
                    format!("{}/{}", parent_folder, folder.name)
                };
                // println!("{}", folder_name);

                let new_identity = {
                    let node_id = folder.node_identity.as_ref()
                        .and_then(|ni| ni.node_id.clone())
                        .or_else(|| identity.node_id.clone());
                    let share_id = folder.node_identity.as_ref()
                        .and_then(|ni| ni.share_id.clone())
                        .or_else(|| identity.share_id.clone());
                    let volume_id = folder.node_identity.as_ref()
                        .and_then(|ni| ni.volume_id.clone())
                        .or_else(|| identity.volume_id.clone());
                    NodeIdentity { node_id, share_id, volume_id }
                };

                recursive_list_file_root(client, &new_identity, folder_name).await?;
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
                    println!("{}", file_name);
                }
            }
        }
    }

    Ok(())
}
