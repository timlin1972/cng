use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

use log::Level::{Error, Info};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{self, Sender};
use tokio::time::{timeout, Duration};

use crate::cfg;
use crate::msg::{self, log, Msg, Reply};
use crate::plugins::nas::files_data;
use crate::utils;
use crate::{error, info, reply_me, unknown};

pub const NAME: &str = "nas";

const LISTENING: &str = "0.0.0.0";
const SERVER_PORT: u16 = 9760;
const CLIENT_PORT: u16 = 9761;

const BUFFER_SIZE: usize = 4096;

struct LastSyncInfo {
    device_name: String,
    last_sync_time: u64,
}

#[derive(Debug)]
pub struct ClientMsg {
    pub action: String,
    pub data: Vec<String>,
}

pub fn client(
    msg_tx_clone: Sender<Msg>,
    mut client_rx: mpsc::Receiver<ClientMsg>,
    tailscale_ip: String,
) {
    tokio::spawn(async move {
        info!(&msg_tx_clone, format!("[{NAME}] Client started"));

        let mut last_sync_infos: Vec<LastSyncInfo> = Vec::new();

        while let Some(event) = client_rx.recv().await {
            match event.action.as_str() {
                "SYNC_DEVICE" => {
                    let device_name = &event.data[0];
                    let device_tailscale_ip = &event.data[1];
                    let device_remote_modify_time = &event.data[2];
                    let device_remote_modify_time = device_remote_modify_time
                        .to_string()
                        .parse::<u64>()
                        .unwrap();

                    if device_tailscale_ip == &tailscale_ip {
                        continue;
                    }

                    info!(
                        &msg_tx_clone,
                        format!("[{NAME}] Do SYNC_DEVICE for {device_name}")
                    );

                    // Check if the device is already in the last_sync_infos list
                    if let Some(last_sync_info) = last_sync_infos
                        .iter_mut()
                        .find(|s| s.device_name == *device_name)
                    {
                        if device_remote_modify_time < last_sync_info.last_sync_time {
                            continue;
                        } else {
                            last_sync_info.last_sync_time = utils::ts();
                        }
                    } else {
                        last_sync_infos.push(LastSyncInfo {
                            device_name: device_name.to_string(),
                            last_sync_time: utils::ts(),
                        });
                    }

                    let files_data = files_data::get_files_data(Path::new(cfg::FILE_FOLDER));
                    let files_data_str = serde_json::to_string(&files_data).unwrap();

                    // send files_data
                    {
                        let mut stream = match TcpStream::connect(format!(
                            "{device_tailscale_ip}:{CLIENT_PORT}"
                        ))
                        .await
                        {
                            Ok(s) => s,
                            Err(e) => {
                                error!(
                                    &msg_tx_clone,
                                    format!("[{NAME}] Failed to connect to {device_tailscale_ip}:{CLIENT_PORT}. Err: {e}")
                                );
                                continue;
                            }
                        };

                        info!(
                            &msg_tx_clone,
                            format!(
                                "[{NAME}] [Go] Send files_data to: {device_name}:{device_tailscale_ip}, size: {}",
                                files_data_str.len()
                            )
                        );

                        let request = format!("PUT files_data {tailscale_ip}\n");
                        stream.write_all(request.as_bytes()).await.unwrap();
                        stream.write_all(files_data_str.as_bytes()).await.unwrap();

                        info!(
                            &msg_tx_clone,
                            format!(
                                "[{NAME}] [Ok] Send files_data to: {device_name}:{device_tailscale_ip}, size: {}",
                                files_data_str.len()
                            )
                        );
                    }

                    // accept Non-NAS to send GET/PUT
                    let listening = format!("{LISTENING}:{SERVER_PORT}");
                    let listener = TcpListener::bind(&listening).await.unwrap();
                    info!(&msg_tx_clone, format!("[{NAME}] Listening on {listening}"));

                    let mut idx = 0;
                    loop {
                        let (mut socket, addr) = listener.accept().await.unwrap();

                        let mut buffer = [0; BUFFER_SIZE];
                        match timeout(Duration::from_secs(10), socket.read(&mut buffer)).await {
                            Ok(Ok(size)) if size > 0 => {
                                let mut received_data = Vec::new();
                                received_data.extend_from_slice(&buffer[..size]);

                                let pos = received_data.iter().position(|&b| b == b'\n').unwrap();

                                let command = &received_data[..=pos];
                                let command = String::from_utf8_lossy(command).trim().to_string();
                                received_data.drain(..=pos);

                                info!(
                                    &msg_tx_clone,
                                    format!("[{NAME}] [{idx}] [Go] Recv: from {addr} '{command}'")
                                );

                                // GET filename
                                if let Some(filename) = command.strip_prefix("GET ") {
                                    let filename = Path::new(filename);
                                    if filename.exists() {
                                        match File::open(filename) {
                                            Ok(mut file) => {
                                                let mut contents = Vec::new();
                                                file.read_to_end(&mut contents).unwrap();
                                                if socket.write_all(&contents).await.is_err() {
                                                    error!(
                                                        &msg_tx_clone,
                                                        format!(
                                                            "[{NAME}] Failed to send file contents"
                                                        )
                                                    );

                                                    break;
                                                }
                                            }
                                            Err(e) => {
                                                error!(
                                                    &msg_tx_clone,
                                                    format!("[{NAME}] Failed to open file {filename:?}. Err: {e}")
                                                );
                                            }
                                        }
                                    } else {
                                        error!(
                                            &msg_tx_clone,
                                            format!("[{NAME}] ERROR: File not found")
                                        );

                                        if socket.write_all(b"ERROR: File not found").await.is_err()
                                        {
                                            error!(
                                                &msg_tx_clone,
                                                format!("[{NAME}] Failed to send error message")
                                            );

                                            break;
                                        }
                                    }

                                    info!(
                                        &msg_tx_clone,
                                        format!(
                                            "[{NAME}] [{idx}] [Ok] Recv: from {addr} '{command}'"
                                        )
                                    );

                                    idx += 1;
                                    continue;
                                }

                                // PUT filename
                                if let Some(filename) = command.strip_prefix("PUT ") {
                                    while let Ok(size) = socket.read(&mut buffer).await {
                                        if size == 0 {
                                            break;
                                        }
                                        received_data.extend_from_slice(&buffer[..size]);
                                    }

                                    let filename = Path::new(filename);

                                    // Ensure the parent directories exist
                                    if let Some(parent) = filename.parent() {
                                        fs::create_dir_all(parent).unwrap();
                                    }

                                    let mut file = File::create(filename).unwrap();
                                    file.write_all(&received_data).unwrap();

                                    info!(
                                        &msg_tx_clone,
                                        format!(
                                            "[{NAME}] [{idx}] [Ok] Recv: from {addr} '{command}'"
                                        )
                                    );

                                    idx += 1;
                                    continue;
                                }

                                // END
                                if command.strip_prefix("END").is_some() {
                                    msg::cmd(
                                        &msg_tx_clone,
                                        reply_me!(),
                                        NAME.to_owned(),
                                        msg::ACT_NAS.to_owned(),
                                        vec!["sync_remote".to_owned(), device_name.to_owned()],
                                    )
                                    .await;

                                    info!(
                                        &msg_tx_clone,
                                        format!("[{NAME}] [{idx}] [Ok] Recv: END")
                                    );

                                    break;
                                }
                            }
                            Ok(Ok(0)) => {
                                error!(&msg_tx_clone, format!("[{NAME}] ⚠️ 客戶端關閉連線"));

                                break;
                            }
                            Ok(Err(e)) => {
                                error!(&msg_tx_clone, format!("[{NAME}] ❌ 讀取錯誤: {e}"));

                                break;
                            }
                            Err(_) => {
                                error!(
                                    &msg_tx_clone,
                                    format!("[{NAME}] ⏰ 讀取超時，跳過這次連線")
                                );

                                break;
                            }
                            _ => {
                                error!(
                                    &msg_tx_clone,
                                    format!("[{NAME}] Error reading from socket")
                                );

                                break;
                            }
                        }
                    }
                }
                _ => {
                    unknown!(&msg_tx_clone, NAME, event);
                }
            }
        }
    });
}
