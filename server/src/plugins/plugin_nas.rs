use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::Path;
use std::str;

use async_trait::async_trait;
use chrono::NaiveDate;
use log::Level::{Error, Info};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{self, Sender};
use tokio::time::Duration;

use crate::cfg;
use crate::msg::{self, log, Cmd, Data, DevInfo, Msg, Reply};
use crate::plugins::nas::{client, files_data, monitor};
use crate::plugins::{plugin_mqtt, plugins_main};
use crate::utils;
use crate::{error, info, init, reply_me, unknown};

pub const NAME: &str = "nas";

const LISTENING: &str = "0.0.0.0";
const SERVER_PORT: u16 = 9760;
const CLIENT_PORT: u16 = 9761;

const BACKUP_DIR: &str = "./backup";

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

#[derive(Debug, Clone)]
struct DevInfoNas {
    name: String,
    onboard: Option<bool>,
    tailscale_ip: Option<String>,
    sync: bool,
}

#[derive(Debug)]
pub struct Plugin {
    name: String,
    msg_tx: Sender<Msg>,
    tailscale_ip: String,
    devices: Vec<DevInfoNas>,
    client_tx: Option<Sender<client::ClientMsg>>,
    sync: bool,
    sync_tx: Option<Sender<bool>>,
}

impl Plugin {
    pub fn new(msg_tx: Sender<Msg>) -> Self {
        Self {
            name: NAME.to_owned(),
            msg_tx,
            tailscale_ip: utils::get_tailscale_ip(),
            devices: vec![],
            client_tx: None,
            sync: false,
            sync_tx: None,
        }
    }

    async fn init(&mut self) {
        // NAS: start the client
        if cfg::name() == cfg::nas() {
            let (client_tx, client_rx) = mpsc::channel(1024);
            self.client_tx = Some(client_tx);
            client::client(self.msg_tx.clone(), client_rx, self.tailscale_ip.clone());
        }

        // Not NAS: start the server
        if cfg::name() != cfg::nas() {
            server(self.msg_tx.clone());
        }

        // NAS: start the backup
        if cfg::name() == cfg::nas() {
            backup(self.msg_tx.clone());
        }

        // monitor CFG::FILE_FOLDER
        if cfg::name() == cfg::nas() {
            monitor::monitor(self.msg_tx.clone());
        } else {
            let (sync_tx, mut sync_rx) = mpsc::channel(512);
            self.sync_tx = Some(sync_tx);
            let msg_tx_clone = self.msg_tx.clone();
            tokio::spawn(async move {
                let mut monitor_started = false;
                while let Some(sync) = sync_rx.recv().await {
                    if !monitor_started && sync {
                        monitor::monitor(msg_tx_clone.clone());
                        monitor_started = true;
                    }
                }
            });
        }

        init!(&self.msg_tx, NAME);
    }

    async fn help(&self) {
        info!(
            &self.msg_tx,
            format!("[{NAME}] {ACT_SHOW}", ACT_SHOW = msg::ACT_SHOW)
        );
    }

    async fn show_devices(&self, cmd: &Cmd) {
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("[{NAME}] Devices"),
        )
        .await;

        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!(
                "[{NAME}] {:10} {:7} {:15} {:4}",
                "Name", "Onboard", "Tailscale IP", "Sync"
            ),
        )
        .await;

        for device in &self.devices {
            let (onboard, tailscale_ip, sync) = get_device_display(device);
            log(
                &self.msg_tx,
                cmd.reply.clone(),
                Info,
                format!(
                    "[{NAME}] {:10} {onboard:7} {tailscale_ip:15} {sync:4}",
                    device.name
                ),
            )
            .await;
        }
    }

    async fn show_shared(&self, cmd: &Cmd) {
        let output = list_files_recursively(Path::new(cfg::FILE_FOLDER));

        for line in output {
            log(
                &self.msg_tx,
                cmd.reply.clone(),
                Info,
                format!("[{NAME}] {line}"),
            )
            .await;
        }
    }

    async fn show_sync(&self, cmd: &Cmd) {
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("[{NAME}] Sync: {}", self.sync),
        )
        .await;
    }

    async fn show(&self, cmd: &Cmd) {
        if cfg::name() != cfg::nas() {
            self.show_sync(cmd).await;
        }
        self.show_shared(cmd).await;
        self.show_devices(cmd).await;
    }

    async fn update_device(&mut self, devices: &[DevInfo]) {
        async fn send_sync_device(
            msg_tx: &Sender<Msg>,
            client_tx: Option<&Sender<client::ClientMsg>>,
            device_nas: &DevInfoNas,
        ) {
            if cfg::name() == cfg::nas() && is_ready_to_sync(device_nas) {
                info!(
                    msg_tx,
                    format!("[{NAME}] Device ready: {}", device_nas.name)
                );

                client_tx
                    .unwrap()
                    .send(client::ClientMsg {
                        action: "SYNC_DEVICE".to_owned(),
                        data: vec![
                            device_nas.name.clone(),
                            device_nas.tailscale_ip.clone().unwrap(),
                            u64::MAX.to_string(),
                        ],
                    })
                    .await
                    .unwrap();
            }
        }

        for device in devices {
            let device_nas = self.devices.iter_mut().find(|d| d.name == device.name);
            match device_nas {
                None => {
                    let device_nas = DevInfoNas {
                        name: device.name.clone(),
                        onboard: device.onboard,
                        tailscale_ip: device.tailscale_ip.clone(),
                        sync: false,
                    };
                    info!(
                        &self.msg_tx,
                        format!("[{NAME}] Device new: {}", device_nas.name)
                    );

                    send_sync_device(&self.msg_tx, self.client_tx.as_ref(), &device_nas).await;
                    self.devices.push(device_nas);
                }
                Some(device_nas) => {
                    if device_nas.onboard != device.onboard {
                        device_nas.onboard = device.onboard;
                        device_nas.sync = false;
                        send_sync_device(&self.msg_tx, self.client_tx.as_ref(), device_nas).await;
                    }

                    if device_nas.tailscale_ip != device.tailscale_ip {
                        device_nas.tailscale_ip = device.tailscale_ip.clone();
                        device_nas.sync = false;
                        send_sync_device(&self.msg_tx, self.client_tx.as_ref(), device_nas).await;
                    }
                }
            }
        }
    }

    async fn nas(&mut self, cmd: &msg::Cmd) {
        match cmd.data.first().unwrap().as_str() {
            "sync_local" => match cmd.data.get(1) {
                Some(sync) => {
                    self.sync = sync == "true";
                    if self.sync {
                        info!(&self.msg_tx, format!("[{NAME}] Synced"));

                        self.sync_tx.as_ref().unwrap().send(true).await.unwrap();
                    }
                }
                None => {
                    log(
                        &self.msg_tx,
                        cmd.reply.clone(),
                        Error,
                        format!("[{NAME}] Sync: missing value"),
                    )
                    .await;
                }
            },
            "sync_remote" => {
                let device_name = cmd.data.get(1).unwrap();
                let device_nas = self
                    .devices
                    .iter_mut()
                    .find(|d| &d.name == device_name)
                    .unwrap();
                device_nas.sync = true;
                log(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    Info,
                    format!("[{NAME}] Device synced: {device_name}"),
                )
                .await;
            }
            "remote_modify" => {
                let filename = cmd.data.get(1).unwrap();
                let remote_modify_time = cmd.data.get(2).unwrap();

                log(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    Info,
                    format!("[{NAME}] remote_modify: {filename}"),
                )
                .await;

                // if I am NAS_CLIENT, send to NAS_SERVER
                // and if I am synced
                if cfg::name() != cfg::nas() && self.sync {
                    // ask NAS_SERVER to sync
                    msg::cmd(
                        &self.msg_tx,
                        reply_me!(),
                        plugin_mqtt::NAME.to_owned(),
                        msg::ACT_ASK.to_owned(),
                        vec![
                            cfg::nas().to_owned(),
                            "p".to_owned(),
                            NAME.to_owned(),
                            "nas".to_owned(),
                            "ask_sync".to_owned(),
                            cfg::name().to_owned(),
                            self.tailscale_ip.to_owned(),
                            remote_modify_time.to_owned(),
                        ],
                    )
                    .await;
                }
                // if I am NAS_SERVER, send to all NAS_CLIENT
                else if cfg::name() == cfg::nas() {
                    for device in &self.devices {
                        if device.name == cfg::nas() {
                            continue;
                        }

                        if device.onboard.is_some()
                            && device.onboard.unwrap()
                            && device.tailscale_ip.is_some()
                            && device.tailscale_ip.as_ref().unwrap() != "n/a"
                        {
                            self.client_tx
                                .as_ref()
                                .unwrap()
                                .send(client::ClientMsg {
                                    action: "SYNC_DEVICE".to_owned(),
                                    data: vec![
                                        device.name.clone(),
                                        device.tailscale_ip.clone().unwrap(),
                                        remote_modify_time.to_owned(),
                                    ],
                                })
                                .await
                                .unwrap();
                        }
                    }
                }
            }
            "ask_sync" => {
                let device_name = cmd.data.get(1).unwrap();
                let device_tailscale_ip = cmd.data.get(2).unwrap();
                let remote_modify_time = cmd.data.get(3).unwrap();
                log(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    Info,
                    format!("[{NAME}] ask_sync: {device_name} {device_tailscale_ip}"),
                )
                .await;

                self.client_tx
                    .as_ref()
                    .unwrap()
                    .send(client::ClientMsg {
                        action: "SYNC_DEVICE".to_owned(),
                        data: vec![
                            device_name.to_owned(),
                            device_tailscale_ip.to_owned(),
                            remote_modify_time.to_owned(),
                        ],
                    })
                    .await
                    .unwrap();
            }
            "remote_remove" => {
                let filename = cmd.data.get(1).unwrap();
                log(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    Info,
                    format!("[{NAME}] remote_remove: {filename}"),
                )
                .await;

                // send to all devices except myself
                for device in &self.devices {
                    if device.name == cfg::name() {
                        continue;
                    }
                    if device.onboard.is_some()
                        && device.onboard.unwrap()
                        && device.tailscale_ip.is_some()
                        && device.tailscale_ip.as_ref().unwrap() != "n/a"
                    {
                        msg::cmd(
                            &self.msg_tx,
                            cmd.reply.clone(),
                            plugin_mqtt::NAME.to_owned(),
                            msg::ACT_ASK.to_owned(),
                            vec![
                                device.name.to_owned(),
                                "p".to_owned(),
                                NAME.to_owned(),
                                "nas".to_owned(),
                                "remove".to_owned(),
                                filename.to_owned(),
                            ],
                        )
                        .await;
                    }
                }
            }
            "remove" => {
                let filename = cmd.data.get(1).unwrap();
                if Path::new(filename).exists() {
                    fs::remove_file(filename).unwrap();
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
                        format!("[{NAME}] remove not found: {filename}"),
                    )
                    .await;
                }
            }
            _ => {
                log(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    Error,
                    format!("[{NAME}] Unknown nas action: {:?}", cmd.data),
                )
                .await;
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
                msg::ACT_HELP => self.help().await,
                msg::ACT_INIT => self.init().await,
                msg::ACT_SHOW => self.show(cmd).await,
                msg::ACT_NAS => self.nas(cmd).await,
                _ => {
                    log(
                        &self.msg_tx,
                        cmd.reply.clone(),
                        Error,
                        format!("[{NAME}] Unknown action: {:?}", cmd.action),
                    )
                    .await;
                }
            },
            Data::Devices(devices) => {
                self.update_device(devices).await;
            }
            _ => {
                unknown!(&self.msg_tx, NAME, msg);
            }
        }

        false
    }
}

fn backup(msg_tx_clone: Sender<Msg>) {
    tokio::spawn(async move {
        loop {
            // check if backup is needed
            // backup dir is BACKUP_DIR+current_date (e.g. ./backup/2023-10-01)
            let now = chrono::Local::now();
            let date = now.format("%Y-%m-%d").to_string();
            let backup_dir = format!("{BACKUP_DIR}/{date}");
            if !Path::new(&backup_dir).exists() {
                fs::create_dir_all(&backup_dir).unwrap();

                // copy all files from cfg::FILE_FOLDER to backup_dir recursively
                let files = get_all_files_recursively(Path::new(cfg::FILE_FOLDER));
                for file in &files {
                    let src = Path::new(file);
                    let dst =
                        Path::new(&backup_dir).join(src.strip_prefix("./shared").unwrap_or(src));
                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent).unwrap();
                    }
                    fs::copy(src, dst).unwrap();
                }

                info!(
                    &msg_tx_clone,
                    format!("[{NAME}] Backup created: {backup_dir}")
                );

                // we keep at most 7 days of backup
                let keep_latest_n = 7;
                let mut date_dirs: Vec<(NaiveDate, String)> = fs::read_dir(BACKUP_DIR)
                    .unwrap()
                    .filter_map(|entry| {
                        let entry = entry.ok().unwrap();
                        let name = entry.file_name().to_string_lossy().into_owned();

                        match NaiveDate::parse_from_str(&name, "%Y-%m-%d") {
                            Ok(date) => Some((date, name)),
                            Err(_) => None,
                        }
                    })
                    .collect();

                date_dirs.sort_by_key(|(date, _)| *date);

                if date_dirs.len() > keep_latest_n {
                    let to_delete = &date_dirs[..date_dirs.len() - keep_latest_n];
                    for (_, name) in to_delete {
                        let path = Path::new(BACKUP_DIR).join(name);
                        if path.is_dir() {
                            info!(
                                &msg_tx_clone,
                                format!("[{NAME}] Backup removed: {}", path.display())
                            );
                            fs::remove_dir_all(path).unwrap();
                        }
                    }
                }
            }

            // sleep for 4 hours
            tokio::time::sleep(Duration::from_secs(4 * 60 * 60)).await;
        }
    });
}

fn server(msg_tx_clone: Sender<Msg>) {
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

                            info!(&msg_tx_clone, format!("[{NAME}] [Ok] Actions ready"));

                            let files_data_nas_str = std::str::from_utf8(&received_data).unwrap();
                            let files_data_nas: files_data::FilesData =
                                serde_json::from_str(files_data_nas_str).unwrap();

                            let dir = Path::new(cfg::FILE_FOLDER);
                            let files_data_local = files_data::get_files_data(dir);

                            let sync_actions: Vec<SyncAction> =
                                create_sync_actions(&files_data_nas, &files_data_local);

                            info!(&msg_tx_clone, format!("[{NAME}] [Ok] Recv: files_data"));

                            // connect to NAS
                            for item in &sync_actions {
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
                                                "[{NAME}] [Go] Send: {} {}",
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
                                                "[{NAME}] [Ok] Send: {} {}",
                                                item.action, item.filename
                                            )
                                        );
                                    }
                                    "PUT" => {
                                        info!(
                                            &msg_tx_clone,
                                            format!(
                                                "[{NAME}] [Go] Send: {} {}",
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
                                                "[{NAME}] [Ok] Send: {} {}",
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

                            info!(&msg_tx_clone, format!("[{NAME}] Send: END"));

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

fn is_ready_to_sync(device_nas: &DevInfoNas) -> bool {
    device_nas.onboard.unwrap_or(false)
        && device_nas.tailscale_ip.is_some()
        && device_nas.tailscale_ip.as_ref().unwrap() != "n/a"
        && !device_nas.sync
}

fn get_device_display(device: &DevInfoNas) -> (String, String, String) {
    let onboard = match device.onboard {
        Some(true) => "Y",
        Some(false) => "N",
        None => "n/a",
    };
    let tailscale_ip = match &device.tailscale_ip {
        Some(ip) => ip,
        None => "n/a",
    };
    let sync = if device.sync { "Y" } else { "N" };

    (
        onboard.to_string(),
        tailscale_ip.to_string(),
        sync.to_string(),
    )
}

fn list_files_recursively(path: &Path) -> Vec<String> {
    let mut output = Vec::new();

    if path.is_dir() {
        output.push(format!("[D] {}", path.display()));
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() {
                let filename = entry.file_name().to_string_lossy().to_string();
                output.push(format!("    {filename}"));
            }
        }
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                output.extend(list_files_recursively(&path));
            }
        }
    }

    output
}

fn get_all_files_recursively(path: &Path) -> Vec<String> {
    let mut output = Vec::new();

    if path.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() {
                let full_path = path.to_string_lossy().to_string();
                output.push(full_path);
            }
        }
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                output.extend(get_all_files_recursively(&path));
            }
        }
    }

    output
}
