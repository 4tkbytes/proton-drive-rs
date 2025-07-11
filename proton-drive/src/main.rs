mod auth;
mod index;

use r2d2::Pool;
use proton_sdk_sys::{data::Callback, prost::Message};
use std::{fs::OpenOptions, sync::{Arc, Mutex}};
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
use r2d2_sqlite::{rusqlite::params, SqliteConnectionManager};
use proton_sdk_sys::protobufs::{FileUploadRequest, FileUploaderCreationRequest, ShareMetadata};
use proton_sdk_rs::uploads::UploaderBuilder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("================== Proton Drive (primitive) ==================");
    let (session, is_first_run, password) = auth::create_new_session().await;

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
            println!("Drive client successfully created!");
            debug!("Drive client created {:?}", cli.handle());
            Arc::new(cli)
        }
        Err(e) => anyhow::bail!(e),
    };

    let volumes = client.get_volumes().await?;

    let main_volume = &volumes[0];

    let share = client.get_shares(main_volume).await?;

    let identity = NodeIdentity { 
        node_id: share.root_node_id.clone(), 
        share_id: share.share_id.clone(), 
        volume_id: main_volume.volume_id.clone()
    };

    let manager = SqliteConnectionManager::file("index.db");
    let pool = Arc::new(Pool::new(manager)?);

    if is_first_run {
        index::index(&client, &identity, password, &pool).await?;
        println!("Ding! Initial indexing is done");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(".cfg")
            .unwrap();
        writeln!(file, "INITIAL_INDEX=true").unwrap();
    } else {
        println!("No big indexing");
    }

    // loop {
    //     update(client.clone(), pool.clone(), 8).await;
    // }

    Ok(())
}

async fn update(client: Arc<DriveClient>, pool: Arc<Pool<SqliteConnectionManager>>, number_of_workers: usize) {
    let folders: Vec<(String, Vec<u8>)> = {
        let conn = pool.get().unwrap();
        let mut stmt = conn.prepare("SELECT full_path, node FROM folders").unwrap();
        stmt.query_map([], |row| {
            let path: String = row.get(0)?;
            let node: Vec<u8> = row.get(1)?;
            Ok((path, node))
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
    };

    let folder_queue = Arc::new(Mutex::new(folders));
    let mut handles = vec![];

    for _ in 0..number_of_workers {
        let queue = Arc::clone(&folder_queue);
        let client = Arc::clone(&client);
        let pool = Arc::clone(&pool);

        handles.push(thread::spawn(move || {
            loop {
                let (folder_path, node_bytes) = {
                    let mut q = queue.lock().unwrap();
                    if q.is_empty() {
                        break;
                    }
                    q.pop().unwrap()
                };

                let node_identity: NodeIdentity = match NodeIdentity::decode(node_bytes.as_slice()) {
                    Ok(n) => n,
                    Err(e) => {
                        log::error!("Failed to decode node for {}: {:?}", folder_path, e);
                        continue;
                    }
                };

                // Call get_folder_children (sync version)
                let children = match client.get_folder_children_blocking(node_identity) {
                    Ok(c) => c,
                    Err(e) => {
                        log::error!("Failed to get children for {}: {:?}", folder_path, e);
                        continue;
                    }
                };

                let conn = pool.get().unwrap();
                for child in children {
                    let (is_folder, folder) = utils::node_is_folder(child.clone());
                    let (is_file, file) = utils::node_is_file(child.clone());

                    if is_folder {
                        let folder_name = folder.as_ref().map(|f| f.name.clone()).unwrap_or_default();
                        let full_path = format!("{}/{}", folder_path, folder_name);
                        let mut stmt = conn.prepare("SELECT COUNT(*) FROM folders WHERE full_path = ?1").unwrap();
                        let exists: i64 = stmt.query_row(params![full_path], |row| row.get(0)).unwrap();
                        if exists == 0 {
                            log::info!("New folder detected: {}", full_path);
                            let node_bytes = folder.unwrap().to_bytes().unwrap();
                            conn.execute(
                                "INSERT INTO folders (full_path, folder_name, checked, node) VALUES (?1, ?2, 0, ?3)",
                                params![full_path, folder_name, node_bytes],
                            ).unwrap();
                        }
                    } else if is_file {
                        let file_name = file.as_ref().map(|f| f.name.clone()).unwrap_or_default();
                        let full_path = format!("{}/{}", folder_path, file_name);
                        let mut stmt = conn.prepare("SELECT COUNT(*) FROM files WHERE full_path = ?1").unwrap();
                        let exists: i64 = stmt.query_row(params![full_path], |row| row.get(0)).unwrap();
                        if exists == 0 {
                            log::info!("New file detected: {}", full_path);
                            let node_bytes = file.unwrap().to_bytes().unwrap();
                            conn.execute(
                                "INSERT INTO files (full_path, file_name, checked, node) VALUES (?1, ?2, 0, ?3)",
                                params![full_path, file_name, node_bytes],
                            ).unwrap();
                        }
                    }
                }
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}

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

    // // uploading an example file
    // const FILE: &'static str = "C:/Users/thrib/Downloads/protobuf-31.1.zip";
    // let metadata = fs::metadata(FILE)?;
    // let file_size = metadata.file_size();
    //
    // let request = FileUploaderCreationRequest {
    //     file_size: file_size as i64,
    //     number_of_samples: 0,
    // };
    //
    // let uploader = UploaderBuilder::new(&client)
    //     .with_request(request)
    //     .build()
    //     .await?;
    //
    // let metadata = fs::metadata(FILE)?;
    // let file_name = std::path::Path::new(FILE)
    //     .file_name()
    //     .and_then(|n| n.to_str())
    //     .unwrap_or("protobuf-31.1.zip")
    //     .to_string();
    //
    // let operation = OperationIdentifier {
    //     r#type: OperationType::Download.into(),
    //     identifier: Uuid::new_v4().to_string(),
    //     timestamp: Utc::now().to_rfc3339()
    // };
    //
    // let share_metadata = ShareMetadata {
    //     share_id: share.share_id.clone(),
    //     membership_address_id: share.membership_address_id.clone(),
    //     membership_email_address: share.membership_email_address.clone(),
    // };
    // let modified = metadata.modified()?;
    // let last_modification_date = modified.duration_since(std::time::UNIX_EPOCH)?.as_secs() as i64;
    //
    // let request = FileUploadRequest {
    //     share_metadata: Some(share_metadata),
    //     parent_folder_identity: Some(identity),
    //     name: file_name.clone(),
    //     mime_type: mime_guess::from_path(FILE).first_or_octet_stream().to_string(),
    //     source_file_path: FILE.to_string(),
    //     thumbnail: None,
    //     last_modification_date,
    //     operation_id: Some(operation),
    // };
    //
    // uploader.upload_file_or_revision(request, Some(move |progress| {
    //     info!("Uploading file [{}] at progress: {}", file_name, progress * 100.0);
    // })).await?;
