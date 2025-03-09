use std::fs::{self, metadata, File};
use std::io::{Read, Write};
use std::path::Path;
use std::str;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use log::Level::{Error, Info, Trace};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{self, Sender};
use tokio::task;

use crate::cfg;
use crate::msg::{self, log, Data, DevInfo, Msg, Reply};
use crate::plugins::{plugin_mqtt, plugin_nas, plugins_main};
use crate::utils;

pub const NAME: &str = "nas";
const NAS_LISTENING: &str = "0.0.0.0";
const NAS_PORT: u16 = 9760;

#[derive(Debug)]
struct ClientMsg {
    action: String,
    data: Vec<String>,
}

#[derive(Debug)]
struct FileData {
    tailscale_ip: String,
    filename: String,
    md5: String,
    modified: i64,
}
#[derive(Debug)]
pub struct Plugin {
    name: String,
    msg_tx: Sender<Msg>,
    client_tx: Option<Sender<ClientMsg>>,
    tailscale_ip: Option<String>,
    file_data: Vec<FileData>,
    sync: bool,
    devices: Vec<DevInfo>,
}

fn server(msg_tx_clone: Sender<Msg>) {
    tokio::spawn(async move {
        let listening = format!("{NAS_LISTENING}:{NAS_PORT}");
        let listener = TcpListener::bind(&listening).await.unwrap();
        log(
            &msg_tx_clone,
            Reply::Device(cfg::name()),
            Info,
            format!("[{NAME}] Listening on {listening}"),
        )
        .await;

        loop {
            let (mut socket, addr) = listener.accept().await.unwrap();

            let msg_tx_clone = msg_tx_clone.clone();
            tokio::spawn(async move {
                let mut buffer = [0; 1024];
                match socket.read(&mut buffer).await {
                    Ok(size) if size > 0 => {
                        let request = str::from_utf8(&buffer[..size]).unwrap();

                        let lines: Vec<&str> = request.splitn(2, '\n').collect();
                        let command = lines[0].trim();

                        log(
                            &msg_tx_clone,
                            Reply::Device(cfg::name()),
                            Info,
                            format!("[{NAME}] Recv: from {addr} '{command}'"),
                        )
                        .await;

                        // GET
                        if let Some(filename) = command.strip_prefix("GET ") {
                            let filename = Path::new(cfg::FILE_FOLDER).join(filename.trim());
                            if filename.exists() {
                                match File::open(&filename) {
                                    Ok(mut file) => {
                                        let mut contents = Vec::new();
                                        file.read_to_end(&mut contents).unwrap();
                                        let _ = socket.write_all(&contents).await;
                                    }
                                    Err(e) => {
                                        log(
                                            &msg_tx_clone,
                                            Reply::Device(cfg::name()),
                                            Error,
                                            format!("[{NAME}] Failed to open file {filename:?}. Err: {e}"),
                                        )
                                        .await;
                                    }
                                }
                            } else {
                                let _ = socket.write_all(b"ERROR: File not found").await;
                                log(
                                    &msg_tx_clone,
                                    Reply::Device(cfg::name()),
                                    Error,
                                    format!("[{NAME}] ERROR: File not found"),
                                )
                                .await;
                            }
                        }

                        // PUT
                        if let Some(filename) = command.strip_prefix("PUT ") {
                            let file_path = format!("{}/{}", cfg::FILE_FOLDER, filename.trim());

                            let mut file = File::create(file_path).unwrap();
                            let mut data_buffer = Vec::new();
                            let lines_1_bytes = lines[1].as_bytes();
                            data_buffer.extend_from_slice(lines_1_bytes);
                            while let Ok(size) = socket.read(&mut buffer).await {
                                if size == 0 {
                                    break;
                                }
                                data_buffer.extend_from_slice(&buffer[..size]);
                            }

                            file.write_all(&data_buffer).unwrap();
                        }
                    }
                    Err(e) => {
                        log(
                            &msg_tx_clone,
                            Reply::Device(cfg::name()),
                            Error,
                            format!("[{NAME}] Failed to read socket. Err: {e}"),
                        )
                        .await;
                    }
                    _ => (),
                }
            });
        }
    });
}

fn client(msg_tx_clone: Sender<Msg>, mut client_rx: mpsc::Receiver<ClientMsg>) {
    tokio::spawn(async move {
        while let Some(event) = client_rx.recv().await {
            match event.action.as_str() {
                "GET" => {
                    let server_ip = &event.data[0];
                    let filename = &event.data[1];

                    let mut stream = match TcpStream::connect(format!("{server_ip}:{NAS_PORT}"))
                        .await
                    {
                        Ok(s) => s,
                        Err(e) => {
                            log(
                                    &msg_tx_clone,
                                    Reply::Device(cfg::name()),
                                    Error,
                                    format!("[{NAME}] Failed to connect to {server_ip}:{NAS_PORT}. Err: {e}"),
                                )
                                .await;
                            continue;
                        }
                    };

                    let request = format!("GET {filename}\n");
                    stream.write_all(request.as_bytes()).await.unwrap();

                    let mut buffer = Vec::new();
                    stream.read_to_end(&mut buffer).await.unwrap();

                    if buffer.starts_with(b"ERROR") {
                        log(
                            &msg_tx_clone,
                            Reply::Device(cfg::name()),
                            Info,
                            format!(
                                "[{NAME}] Failed to GET. Err: {}",
                                String::from_utf8_lossy(&buffer)
                            ),
                        )
                        .await;
                        continue;
                    }

                    let filename = format!("{}/{filename}", cfg::FILE_FOLDER);

                    let mut file = File::create(&filename).unwrap();
                    file.write_all(&buffer).unwrap();
                    log(
                        &msg_tx_clone,
                        Reply::Device(cfg::name()),
                        Trace,
                        format!("[{NAME}] Recv: from {server_ip}, {filename}",),
                    )
                    .await;
                }
                "PUT" => {
                    let server_ip = &event.data[0];
                    let filename = &event.data[1];

                    let mut stream = TcpStream::connect(format!("{server_ip}:{NAS_PORT}"))
                        .await
                        .unwrap();

                    let request = format!("PUT {filename}\n");
                    stream.write_all(request.as_bytes()).await.unwrap();

                    log(
                        &msg_tx_clone,
                        Reply::Device(cfg::name()),
                        Info,
                        format!("[{NAME}] PUT requst: {request:?}"),
                    )
                    .await;

                    let file_path = format!("{}/{filename}", cfg::FILE_FOLDER);
                    let mut file = File::open(file_path).unwrap();
                    let mut contents = Vec::new();
                    file.read_to_end(&mut contents).unwrap();
                    stream.write_all(&contents).await.unwrap();
                    log(
                        &msg_tx_clone,
                        Reply::Device(cfg::name()),
                        Info,
                        format!("[{NAME}] Sent: to {server_ip}, {filename}"),
                    )
                    .await;
                }
                _ => {
                    log(
                        &msg_tx_clone,
                        Reply::Device(cfg::name()),
                        Error,
                        format!("[{NAME}] Unknown event: {event:?}"),
                    )
                    .await;
                }
            }
        }
    });
}

fn monitor_get_file(file_path: &str) -> String {
    let keyword = "/shared/";
    if let Some(pos) = file_path.find(keyword) {
        let result = &file_path[pos + keyword.len()..];
        return result.to_owned();
    }

    "".to_owned()
}

fn monitor(msg_tx_clone: Sender<Msg>) {
    tokio::spawn(async move {
        let path_to_watch = Path::new(cfg::FILE_FOLDER);

        let (tx, mut rx) = mpsc::channel(100);

        let _watcher_handle = task::spawn_blocking(move || {
            let mut watcher = RecommendedWatcher::new(
                move |res| {
                    let _ = tx.blocking_send(res);
                },
                Config::default(),
            )
            .expect("Watcher 初始化失敗");

            watcher
                .watch(Path::new(path_to_watch), RecursiveMode::Recursive)
                .expect("無法監聽目錄");

            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        });

        log(
            &msg_tx_clone,
            Reply::Device(cfg::name()),
            Info,
            format!("[{NAME}] Monitoring {path_to_watch:?}"),
        )
        .await;

        while let Some(event) = rx.recv().await {
            match event {
                Ok(event) => match event.kind {
                    notify::event::EventKind::Create(_) => (),
                    notify::event::EventKind::Modify(_) => {
                        for path in event.paths.iter() {
                            let filename = monitor_get_file(path.to_str().unwrap());
                            msg::cmd(
                                &msg_tx_clone,
                                Reply::Device(cfg::name()),
                                NAME.to_owned(),
                                msg::ACT_NAS.to_owned(),
                                vec!["remote_modify".to_owned(), filename.to_owned()],
                            )
                            .await;
                            log(
                                &msg_tx_clone,
                                Reply::Device(cfg::name()),
                                Info,
                                format!("[{NAME}][monitor] File is modified: {filename}"),
                            )
                            .await;
                        }
                    }
                    notify::event::EventKind::Remove(_) => {
                        for path in event.paths.iter() {
                            let filename = monitor_get_file(path.to_str().unwrap());
                            msg::cmd(
                                &msg_tx_clone,
                                Reply::Device(cfg::name()),
                                NAME.to_owned(),
                                msg::ACT_NAS.to_owned(),
                                vec!["remote_remove".to_owned(), filename.to_owned()],
                            )
                            .await;
                            log(
                                &msg_tx_clone,
                                Reply::Device(cfg::name()),
                                Info,
                                format!("[{NAME}][monitor] File is removed: {filename}"),
                            )
                            .await;
                        }
                    }
                    notify::event::EventKind::Access(_) => (),
                    _ => {
                        log(
                            &msg_tx_clone,
                            Reply::Device(cfg::name()),
                            Info,
                            format!("[{NAME}] Unhandled event: {event:?}"),
                        )
                        .await;
                    }
                },
                Err(e) => eprintln!("[{NAME}] Invalid event. Err: {e:?}"),
            }
        }
    });
}

impl Plugin {
    pub fn new(msg_tx: Sender<Msg>) -> Self {
        Self {
            name: NAME.to_owned(),
            msg_tx,
            client_tx: None,
            tailscale_ip: None,
            file_data: vec![],
            sync: false,
            devices: vec![],
        }
    }

    async fn init(&mut self) {
        // monitor CFG::FILE_FOLDER
        monitor(self.msg_tx.clone());

        // if I am NOT cfg::nas(), start the server
        if cfg::name() != cfg::nas() {
            server(self.msg_tx.clone());
        }

        // start the client
        let (client_tx, client_rx) = mpsc::channel(100);
        self.client_tx = Some(client_tx);
        client(self.msg_tx.clone(), client_rx);

        log(
            &self.msg_tx,
            Reply::Device(cfg::name()),
            Info,
            format!("[{NAME}] init"),
        )
        .await;
    }

    async fn help(&self) {}

    async fn update_item(&self, cmd: &msg::Cmd) {
        let tailscale_ip = match &self.tailscale_ip {
            None => {
                log(
                    &self.msg_tx,
                    Reply::Device(cfg::name()),
                    Error,
                    format!("[{NAME}] Failed to update_item due to no Tailscale IP"),
                )
                .await;
                return;
            }
            Some(t) => t,
        };

        let filename = cmd.data.first().unwrap();
        let file_path = format!("{}/{}", cfg::FILE_FOLDER, filename);

        match metadata(&file_path) {
            Ok(metadata) => {
                let modified_time = metadata.modified().expect("無法獲取修改時間");
                let datetime: DateTime<Utc> = modified_time.into();
                let md5 = utils::calculate_md5(&file_path).unwrap();

                msg::nas_file(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    "report",
                    tailscale_ip.to_owned(),
                    cfg::name().to_owned(),
                    "file_single".to_owned(),
                    filename.to_owned(),
                    md5,
                    datetime.timestamp().to_string(),
                )
                .await;
            }
            Err(_) => {
                msg::nas_file(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    "report",
                    tailscale_ip.to_owned(),
                    cfg::name().to_owned(),
                    "file_single".to_owned(),
                    filename.to_owned(),
                    "md5".to_owned(),
                    "0".to_owned(),
                )
                .await;
            }
        }
    }

    async fn update(&mut self, cmd: &msg::Cmd) {
        log(
            &self.msg_tx,
            Reply::Device(cfg::name()),
            Info,
            format!("[{NAME}] report start"),
        )
        .await;

        let tailscale_ip = match &self.tailscale_ip {
            None => {
                log(
                    &self.msg_tx,
                    Reply::Device(cfg::name()),
                    Error,
                    format!("[{NAME}] Failed to update due to no Tailscale IP"),
                )
                .await;
                return;
            }
            Some(t) => t,
        };

        msg::nas_file(
            &self.msg_tx,
            cmd.reply.clone(),
            "report",
            tailscale_ip.to_owned(),
            cfg::name().to_owned(),
            "start".to_owned(),
            "".to_owned(),
            "".to_owned(),
            "".to_owned(),
        )
        .await;

        // for all files in cfg::FILE_FOLDER
        let dir = Path::new(cfg::FILE_FOLDER);
        if dir.is_dir() {
            for entry in fs::read_dir(dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.is_file() {
                    let metadata = entry.metadata().expect("無法讀取檔案元數據");
                    let modified_time = metadata.modified().expect("無法獲取修改時間");
                    let datetime: DateTime<Utc> = modified_time.into();

                    let file_path = format!(
                        "{}/{}",
                        cfg::FILE_FOLDER,
                        entry.file_name().to_string_lossy()
                    );
                    let md5 = utils::calculate_md5(&file_path).unwrap();

                    msg::nas_file(
                        &self.msg_tx,
                        cmd.reply.clone(),
                        "report",
                        tailscale_ip.to_owned(),
                        cfg::name().to_owned(),
                        "file".to_owned(),
                        entry.file_name().to_string_lossy().to_string(),
                        md5,
                        datetime.timestamp().to_string(),
                    )
                    .await;
                }
                // else if path.is_dir() {
                //     visit_dirs(&path)?;  // 遞歸進入子目錄
                // }
            }
        }

        msg::nas_file(
            &self.msg_tx,
            cmd.reply.clone(),
            "report",
            tailscale_ip.to_owned(),
            cfg::name().to_owned(),
            "end".to_owned(),
            "".to_owned(),
            "".to_owned(),
            "".to_owned(),
        )
        .await;

        log(
            &self.msg_tx,
            Reply::Device(cfg::name()),
            Info,
            format!("[{NAME}] report end"),
        )
        .await;
    }

    async fn sync_file(&self, filename: &str, tailscale_ip: &str, md5: &str, modified: i64) {
        // check if filename is in cfg::FILE_FOLDER
        // if existed, check if modified time is newer than modified
        let path = Path::new(cfg::FILE_FOLDER).join(filename);
        if !path.exists() {
            self.client_tx
                .as_ref()
                .unwrap()
                .send(ClientMsg {
                    action: "GET".to_owned(),
                    data: vec![tailscale_ip.to_owned(), filename.to_owned()],
                })
                .await
                .unwrap();
        } else {
            let file_path = format!("{}/{}", cfg::FILE_FOLDER, filename);
            let local_md5 = utils::calculate_md5(&file_path).unwrap();
            if local_md5 != md5 {
                let metadata = path.metadata().expect("無法讀取檔案元數據");
                let modified_time = metadata.modified().expect("無法獲取修改時間");
                let datetime: DateTime<Utc> = modified_time.into();
                if datetime.timestamp() <= modified {
                    self.client_tx
                        .as_ref()
                        .unwrap()
                        .send(ClientMsg {
                            action: "GET".to_owned(),
                            data: vec![tailscale_ip.to_owned(), filename.to_owned()],
                        })
                        .await
                        .unwrap();
                } else {
                    self.client_tx
                        .as_ref()
                        .unwrap()
                        .send(ClientMsg {
                            action: "PUT".to_owned(),
                            data: vec![tailscale_ip.to_owned(), filename.to_owned()],
                        })
                        .await
                        .unwrap();
                }
            }
        }
    }

    // client only
    async fn report(&mut self, cmd: &msg::Cmd) {
        let tailscale_ip = cmd.data[1].as_str();
        let device_name = cmd.data[2].as_str();
        let stage = cmd.data[3].as_str();

        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("[{NAME}] NAS report to '{device_name}': stage {stage}"),
        )
        .await;

        match stage {
            "file_single" => {
                let filename = cmd.data[4].as_str();
                let md5 = cmd.data[5].as_str();
                let modified = cmd.data[6].parse::<i64>().unwrap_or(0);

                self.sync_file(filename, tailscale_ip, md5, modified).await;
            }
            "start" => {
                // remove all tailscale_ip in self.file_data
                self.file_data.retain(|x| x.tailscale_ip != tailscale_ip);
                log(
                    &self.msg_tx,
                    Reply::Device(cfg::name()),
                    Trace,
                    format!("[{NAME}] report start: {tailscale_ip}",),
                )
                .await;
            }
            "file" => {
                let filename = cmd.data[4].as_str();
                let md5 = cmd.data[5].as_str();
                let modified = cmd.data[6].parse::<i64>().unwrap_or(0);

                self.file_data.push(FileData {
                    tailscale_ip: tailscale_ip.to_owned(),
                    filename: filename.to_owned(),
                    md5: md5.to_owned(),
                    modified,
                });
            }
            "end" => {
                // check if file_data is in self.file_data
                for file_data in &self.file_data {
                    if file_data.tailscale_ip == tailscale_ip {
                        self.sync_file(
                            &file_data.filename,
                            tailscale_ip,
                            &file_data.md5,
                            file_data.modified,
                        )
                        .await;
                    }
                }

                // sent to remote if local file is NOT in self.file_data
                let dir = Path::new(cfg::FILE_FOLDER);
                if dir.is_dir() {
                    for entry in fs::read_dir(dir).unwrap() {
                        let entry = entry.unwrap();
                        let path = entry.path();
                        if path.is_file() {
                            let filename = entry.file_name().to_string_lossy().to_string();
                            if !self
                                .file_data
                                .iter()
                                .any(|x| x.tailscale_ip == tailscale_ip && x.filename == filename)
                            {
                                self.client_tx
                                    .as_ref()
                                    .unwrap()
                                    .send(ClientMsg {
                                        action: "PUT".to_owned(),
                                        data: vec![tailscale_ip.to_owned(), filename],
                                    })
                                    .await
                                    .unwrap();
                            }
                        }
                    }
                }

                self.file_data.retain(|x| x.tailscale_ip != tailscale_ip);

                log(
                    &self.msg_tx,
                    Reply::Device(cfg::name()),
                    Trace,
                    format!("[{NAME}] report end: {tailscale_ip}",),
                )
                .await;

                msg::cmd(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    plugin_mqtt::NAME.to_owned(),
                    msg::ACT_ASK.to_owned(),
                    vec![
                        device_name.to_owned(),
                        "p".to_owned(),
                        plugin_nas::NAME.to_owned(),
                        "nas".to_owned(),
                        "sync".to_owned(),
                        "true".to_owned(),
                    ],
                )
                .await;
            }
            _ => {
                log(
                    &self.msg_tx,
                    Reply::Device(cfg::name()),
                    Info,
                    format!("[{NAME}] Unknown stage: {stage}"),
                )
                .await;
            }
        }
    }

    async fn nas(&mut self, cmd: &msg::Cmd) {
        match cmd.data.first().unwrap().as_str() {
            "report" => self.report(cmd).await,
            "sync" => match cmd.data.get(1) {
                Some(sync) => {
                    self.sync = sync == "true";
                    if self.sync {
                        log(
                            &self.msg_tx,
                            Reply::Device(cfg::name()),
                            Info,
                            format!("[{NAME}] synced"),
                        )
                        .await;
                    }
                }
                None => {
                    log(
                        &self.msg_tx,
                        cmd.reply.clone(),
                        Error,
                        format!("[{NAME}] sync: missing value"),
                    )
                    .await;
                }
            },
            "modify" => {
                let tailscale_ip = cmd.data.get(1).unwrap();
                let filename = cmd.data.get(2).unwrap();

                log(
                    &self.msg_tx,
                    Reply::Device(cfg::name()),
                    Info,
                    format!("[{NAME}] modify: {tailscale_ip}:{filename}"),
                )
                .await;

                self.client_tx
                    .as_ref()
                    .unwrap()
                    .send(ClientMsg {
                        action: "GET".to_owned(),
                        data: vec![tailscale_ip.to_owned(), filename.clone()],
                    })
                    .await
                    .unwrap();
            }
            "remove" => {
                let filename = cmd.data.get(1).unwrap();
                let file_path = format!("{}/{}", cfg::FILE_FOLDER, filename);
                if Path::new(&file_path).exists() {
                    fs::remove_file(&file_path).unwrap();
                    log(
                        &self.msg_tx,
                        cmd.reply.clone(),
                        Info,
                        format!("[{NAME}] removed: {filename}"),
                    )
                    .await;
                } else {
                    log(
                        &self.msg_tx,
                        cmd.reply.clone(),
                        Error,
                        format!("[{NAME}] not found: {filename}"),
                    )
                    .await;
                }
            }
            "remote_remove" => {
                let filename = cmd.data.get(1).unwrap();

                // send to all devices except myself
                for device in &self.devices {
                    if device.name == cfg::name() {
                        continue;
                    }
                    if device.onboard.is_some() && device.onboard.unwrap() {
                        msg::cmd(
                            &self.msg_tx,
                            cmd.reply.clone(),
                            plugin_mqtt::NAME.to_owned(),
                            msg::ACT_ASK.to_owned(),
                            vec![
                                device.name.to_owned(),
                                "p".to_owned(),
                                plugin_nas::NAME.to_owned(),
                                "nas".to_owned(),
                                "remove".to_owned(),
                                filename.to_owned(),
                            ],
                        )
                        .await;
                    }
                }
            }
            "remote_modify" => {
                let filename = cmd.data.get(1).unwrap();

                // if I am server, send to client (NAS)
                if cfg::name() != cfg::nas() {
                    let tailscale_ip = match &self.tailscale_ip {
                        None => {
                            log(
                                &self.msg_tx,
                                Reply::Device(cfg::name()),
                                Error,
                                format!("[{NAME}] Failed to remote_modify due to no Tailscale IP"),
                            )
                            .await;
                            return;
                        }
                        Some(t) => t,
                    };

                    let file_path = format!("{}/{filename}", cfg::FILE_FOLDER);
                    match metadata(&file_path) {
                        Ok(metadata) => {
                            let modified_time = metadata.modified().expect("無法獲取修改時間");
                            let datetime: DateTime<Utc> = modified_time.into();
                            let md5 = utils::calculate_md5(&file_path).unwrap();

                            msg::cmd(
                                &self.msg_tx,
                                Reply::Device(cfg::name()),
                                plugin_mqtt::NAME.to_owned(),
                                msg::ACT_ASK.to_owned(),
                                vec![
                                    cfg::nas().to_owned(),
                                    "p".to_owned(),
                                    plugin_nas::NAME.to_owned(),
                                    "nas".to_owned(),
                                    "report".to_owned(),
                                    tailscale_ip.to_owned(),
                                    cfg::name().to_owned(),
                                    "file_single".to_owned(),
                                    filename.to_owned(),
                                    md5,
                                    datetime.timestamp().to_string(),
                                ],
                            )
                            .await;
                        }
                        Err(e) => {
                            log(
                                &self.msg_tx,
                                Reply::Device(cfg::name()),
                                Error,
                                format!("[{NAME}] Failed to get metadata for the file {filename}. Err: {e:?}."),
                            )
                            .await;
                        }
                    }
                }
                // if I am client (NAS), send to all server
                else {
                    for device in &self.devices {
                        if device.name == cfg::nas() {
                            continue;
                        }
                        if device.onboard.is_some() && device.onboard.unwrap() {
                            msg::cmd(
                                &self.msg_tx,
                                Reply::Device(cfg::name()),
                                plugin_mqtt::NAME.to_owned(),
                                msg::ACT_ASK.to_owned(),
                                vec![
                                    device.name.to_owned(),
                                    "p".to_owned(),
                                    plugin_nas::NAME.to_owned(),
                                    msg::ACT_UPDATE_ITEM.to_owned(),
                                    filename.to_owned(),
                                ],
                            )
                            .await;
                        }
                    }
                }
            }
            _ => {
                log(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    Error,
                    format!("[{NAME}] unknown nas action: {:?}", cmd.data),
                )
                .await;
            }
        }
    }

    async fn show(&self, cmd: &msg::Cmd) {
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("[{NAME}] sync: {}", self.sync),
        )
        .await;

        // show all files in cfg::FILE_FOLDER
        let dir = Path::new(cfg::FILE_FOLDER);
        if dir.is_dir() {
            for entry in fs::read_dir(dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.is_file() {
                    let metadata = entry.metadata().expect("無法讀取檔案元數據");
                    let modified_time = metadata.modified().expect("無法獲取修改時間");
                    let datetime: DateTime<Utc> = modified_time.into();

                    log(
                        &self.msg_tx,
                        cmd.reply.clone(),
                        Info,
                        format!(
                            "[{NAME}] {datetime} {filename}",
                            filename = entry.file_name().to_string_lossy(),
                            datetime = utils::ts_str_full(datetime.timestamp() as u64)
                        ),
                    )
                    .await;
                }
            }
        }
    }
}

#[async_trait]
impl plugins_main::Plugin for Plugin {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    async fn msg(&mut self, msg: &Msg) -> bool {
        match &msg.data {
            Data::Cmd(cmd) => match cmd.action.as_str() {
                msg::ACT_SHOW => self.show(cmd).await,
                msg::ACT_HELP => self.help().await,
                msg::ACT_INIT => self.init().await,
                msg::ACT_UPDATE => self.update(cmd).await,
                msg::ACT_UPDATE_ITEM => self.update_item(cmd).await,
                msg::ACT_NAS => self.nas(cmd).await,
                _ => {
                    log(
                        &self.msg_tx,
                        cmd.reply.clone(),
                        Error,
                        format!("[{NAME}] unknown action: {:?}", cmd.action),
                    )
                    .await;
                }
            },
            Data::TailscaleIP(tailscale_ip) => {
                self.tailscale_ip = Some(tailscale_ip.to_owned());
            }
            Data::Devices(devices) => {
                self.devices = devices.to_owned();
            }
            _ => {
                log(
                    &self.msg_tx,
                    Reply::Device(cfg::name()),
                    Error,
                    format!("[{NAME}] unknown msg: {msg:?}"),
                )
                .await;
            }
        }

        false
    }
}
