use async_recursion::async_recursion;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use r2d2_sqlite::rusqlite::params;
use proton_sdk_rs::drive::DriveClient;
use proton_sdk_rs::utils;
use proton_sdk_sys::protobufs::{NodeIdentity, NodeType, ToByteArray};

pub async fn index(
    client: &DriveClient,
    identity: &NodeIdentity,
    password: String,
    pool: &Pool<SqliteConnectionManager>,
) -> anyhow::Result<()> {
    {
        let conn = pool.get()?;
        conn.execute_batch(&format!("PRAGMA key = '{}';", password))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                full_path TEXT NOT NULL UNIQUE,
                file_name TEXT NOT NULL,
                checked BOOLEAN NOT NULL DEFAULT 0,
                node BLOB NOT NULL
            );
            CREATE TABLE IF NOT EXISTS folders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                full_path TEXT NOT NULL UNIQUE,
                folder_name TEXT NOT NULL,
                checked BOOLEAN NOT NULL DEFAULT 0,
                node BLOB NOT NULL
            );",
        )?;
    }

    let mut file_count = 0;
    recursive_list_file_root(
        client,
        identity,
        "".to_string(),
        &mut file_count,
        &|count| println!("Indexed {} files...", count),
        pool,
    )
        .await?;

    Ok(())
}

#[async_recursion]
pub async fn recursive_list_file_root<F>(
    client: &DriveClient,
    identity: &NodeIdentity,
    parent_folder: String,
    file_count: &mut usize,
    progress_callback: &F,
    pool: &Pool<SqliteConnectionManager>,
) -> anyhow::Result<()>
where
    F: Fn(usize) + Send + Sync,
{
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

                let folder_name_clone = folder.name.clone();
                let full_path_clone = folder_name.clone();
                let node_bytes = folder.to_bytes()?;
                let pool_for_blocking = pool.clone();
                tokio::task::spawn_blocking(move || {
                    let conn = pool_for_blocking.get()?;
                    conn.execute(
                        "INSERT INTO folders (full_path, folder_name, checked, node) VALUES (?1, ?2, 0, ?3)
                            ON CONFLICT(full_path) DO UPDATE SET node = excluded.node, folder_name = excluded.folder_name, checked = 0",
                        params![full_path_clone, folder_name_clone, node_bytes],
                    )?;
                    Ok::<_, anyhow::Error>(())
                })
                .await??;

                recursive_list_file_root(client, &new_identity, folder_name, file_count, progress_callback, pool).await?;
            }
        } else {
            let (is_file, file) = utils::node_is_file(child.clone());
            if is_file {
                *file_count += 1;
                progress_callback(*file_count);
                if let Some(file) = file {
                    let file_name = file.name.clone();
                    let full_path = if parent_folder.is_empty() {
                        file_name.clone()
                    } else {
                        format!("{}/{}", parent_folder, file_name)
                    };
                    let node_bytes = file.to_bytes()?;
                    let pool = pool.clone();
                    let file_name_clone = file_name.clone();
                    let full_path_clone = full_path.clone();
                    tokio::task::spawn_blocking(move || {
                        let conn = pool.get()?;
                        conn.execute(
                            "INSERT INTO files (full_path, file_name, checked, node) VALUES (?1, ?2, 0, ?3)
        ON CONFLICT(full_path) DO UPDATE SET node = excluded.node, file_name = excluded.file_name, checked = 0",
                            params![full_path_clone, file_name_clone, node_bytes],
                        )?;
                        Ok::<_, anyhow::Error>(())
                    })
                        .await??;
                    println!("{}", full_path);
                }
            }
        }
    }
    Ok(())
}