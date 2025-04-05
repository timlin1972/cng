use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::Path;

use log::Level::{Error, Info};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::Sender;

use crate::cfg;
use crate::msg::{self, log, Msg, Reply};
use crate::plugins::nas::files_data;
use crate::{error, info, reply_me};

pub const NAME: &str = "nas";

const LISTENING: &str = "0.0.0.0";
const SERVER_PORT: u16 = 9760;
const CLIENT_PORT: u16 = 9761;

const BUFFER_SIZE: usize = 4096;

struct SyncAction {
    action: String, // GET or PUT
    filename: String,
}

fn create_sync_actions(
    files_data_nas: &files_data::FilesData,
    files_data_local: &files_data::FilesData,
) -> Vec<SyncAction> {
    let mut sync_actions: Vec<SyncAction> = vec![];

    for file_nas in &files_data_nas.files_data {
        match files_data_local
            .files_data
            .iter()
            .find(|d| d.filename == file_nas.filename)
        {
            None => sync_actions.push(SyncAction {
                action: "GET".to_owned(),
                filename: file_nas.filename.clone(),
            }),
            Some(t) => {
                if t.md5 != file_nas.md5 {
                    if t.modified < file_nas.modified {
                        sync_actions.push(SyncAction {
                            action: "GET".to_owned(),
                            filename: file_nas.filename.clone(),
                        })
                    } else {
                        sync_actions.push(SyncAction {
                            action: "PUT".to_owned(),
                            filename: file_nas.filename.clone(),
                        })
                    }
                }
            }
        }
    }

    for file_local in &files_data_local.files_data {
        if !files_data_nas
            .files_data
            .iter()
            .any(|d| d.filename == file_local.filename)
        {
            sync_actions.push(SyncAction {
                action: "PUT".to_owned(),
                filename: file_local.filename.clone(),
            })
        }
    }

    sync_actions
}

pub fn server(msg_tx_clone: Sender<Msg>) {
    tokio::spawn(async move {
        let listening = format!("{LISTENING}:{CLIENT_PORT}");
        let listener = TcpListener::bind(&listening).await.unwrap();
        info!(&msg_tx_clone, format!("[{NAME}] Listening on {listening}"));

        loop {
            let (mut socket, addr) = listener.accept().await.unwrap();

            let msg_tx_clone = msg_tx_clone.clone();
            tokio::spawn(async move {
                let mut buffer = [0; BUFFER_SIZE];
                match socket.read(&mut buffer).await {
                    Ok(size) if size > 0 => {
                        let mut received_data = Vec::new();
                        received_data.extend_from_slice(&buffer[..size]);

                        let pos = received_data.iter().position(|&b| b == b'\n').unwrap();

                        let command = &received_data[..=pos];
                        let command = String::from_utf8_lossy(command).trim().to_string();
                        received_data.drain(..=pos);

                        info!(
                            &msg_tx_clone,
                            format!("[{NAME}] Recv: from {addr} '{command}'")
                        );

                        // PUT files_data
                        if let Some(nas_ip) = command.strip_prefix("PUT files_data ") {
                            let nas_ip_clone = nas_ip.to_owned();

                            while let Ok(size) = socket.read(&mut buffer).await {
                                if size == 0 {
                                    break;
                                }
                                received_data.extend_from_slice(&buffer[..size]);
                            }

                            info!(
                                &msg_tx_clone,
                                format!(
                                    "[{NAME}] [Ok] Recv: files_data, size: {}",
                                    received_data.len()
                                )
                            );

                            let files_data_nas_str = std::str::from_utf8(&received_data).unwrap();
                            let files_data_nas: files_data::FilesData =
                                serde_json::from_str(files_data_nas_str).unwrap();

                            let dir = Path::new(cfg::FILE_FOLDER);
                            let files_data_local = files_data::get_files_data(dir);

                            let sync_actions: Vec<SyncAction> =
                                create_sync_actions(&files_data_nas, &files_data_local);

                            info!(&msg_tx_clone, format!("[{NAME}] [Ok] Actions ready"));

                            // connect to NAS
                            let sync_actions_len = sync_actions.len();
                            for (idx, item) in sync_actions.iter().enumerate() {
                                let mut stream = match TcpStream::connect(format!(
                                    "{nas_ip_clone}:{SERVER_PORT}"
                                ))
                                .await
                                {
                                    Ok(s) => s,
                                    Err(e) => {
                                        error!(
                                            &msg_tx_clone,
                                            format!("[{NAME}] Failed to connect to {nas_ip_clone}:{SERVER_PORT}. Err: {e}")
                                        );
                                        continue;
                                    }
                                };

                                match item.action.as_str() {
                                    "GET" => {
                                        info!(
                                            &msg_tx_clone,
                                            format!(
                                                "[{NAME}] [Go] [{idx}/{sync_actions_len}] {} {}",
                                                item.action, item.filename
                                            )
                                        );

                                        let request = format!("GET {}\n", item.filename);
                                        stream.write_all(request.as_bytes()).await.unwrap();

                                        let mut buffer = Vec::new();
                                        stream.read_to_end(&mut buffer).await.unwrap();

                                        if buffer.starts_with(b"ERROR") {
                                            info!(
                                                &msg_tx_clone,
                                                format!(
                                                    "[{NAME}] Failed to GET. Err: {}",
                                                    String::from_utf8_lossy(&buffer)
                                                )
                                            );
                                            continue;
                                        }

                                        let filename = Path::new(&item.filename);

                                        // Ensure the parent directories exist
                                        if let Some(parent) = filename.parent() {
                                            fs::create_dir_all(parent).unwrap();
                                        }

                                        let mut file = File::create(filename).unwrap();
                                        file.write_all(&buffer).unwrap();
                                        info!(
                                            &msg_tx_clone,
                                            format!(
                                                "[{NAME}] [Ok] [{idx}/{sync_actions_len}] {} {}",
                                                item.action, item.filename
                                            )
                                        );
                                    }
                                    "PUT" => {
                                        info!(
                                            &msg_tx_clone,
                                            format!(
                                                "[{NAME}] [Go] [{idx}/{sync_actions_len}] {} {}",
                                                item.action, item.filename
                                            )
                                        );

                                        let file = File::open(&item.filename).unwrap();
                                        let mut reader = BufReader::new(file);

                                        let request = format!("PUT {}\n", item.filename);
                                        stream.write_all(request.as_bytes()).await.unwrap();

                                        let mut buffer = [0; 4096];
                                        while let Ok(n) = reader.read(&mut buffer) {
                                            if n == 0 {
                                                break;
                                            }
                                            stream.write_all(&buffer[..n]).await.unwrap();
                                        }

                                        info!(
                                            &msg_tx_clone,
                                            format!(
                                                "[{NAME}] [Ok] [{idx}/{sync_actions_len}] {} {}",
                                                item.action, item.filename
                                            )
                                        );
                                    }
                                    _ => (),
                                }
                            }

                            // send END
                            let mut stream = match TcpStream::connect(format!(
                                "{nas_ip_clone}:{SERVER_PORT}"
                            ))
                            .await
                            {
                                Ok(s) => s,
                                Err(e) => {
                                    error!(
                                        &msg_tx_clone,
                                        format!("[{NAME}] Failed to connect to {nas_ip_clone}:{SERVER_PORT}. Err: {e}")
                                    );
                                    return;
                                }
                            };

                            info!(&msg_tx_clone, format!("[{NAME}] END"));

                            let request = "END\n".to_owned();
                            stream.write_all(request.as_bytes()).await.unwrap();

                            msg::cmd(
                                &msg_tx_clone,
                                reply_me!(),
                                NAME.to_owned(),
                                msg::ACT_NAS.to_owned(),
                                vec!["sync_local".to_owned(), "true".to_owned()],
                            )
                            .await;
                        }
                    }
                    _ => (),
                }
            });
        }
    });
}
