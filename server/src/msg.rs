use serde::Serialize;
use tokio::sync::mpsc::Sender;

use crate::cfg;
use crate::panels::panels_main;
use crate::plugins::{plugin_devices, plugin_log, plugin_mqtt, plugin_nas};
use crate::utils;

#[derive(Debug, Clone)]
pub enum Data {
    Log(Log),
    Devices(Vec<DevInfo>),
    DeviceUpdate(DevInfo),
    DeviceCountdown,
    Weather(Vec<City>),
    Worldtime(Vec<Worldtime>),
    Cmd(Cmd),
    TailscaleIP(String),
}

#[derive(Debug)]
pub struct Msg {
    pub ts: u64,
    pub plugin: String,
    pub data: Data,
}

//              plugin      data[0]         data[1]         data[2]         data[3] data[4]
//  show        devices     device (opt)    -               -               -       -
//  show        others      -               -               -               -       -
//  init        all         -               -               -               -       -
//  ask         mqtt        target_device   p               plugin          action  -
//  reply       all         level           msg             -               -       -
//  quit        all         -               -               -               -       -
//  publish     mqtt        topic           retain          payload         -       -
//  disconnect  mqtt        -               -               -               -       -
//  wake        wol         device          -               -               -       -
//  ping        ping        ip              -               -               -       -
//  update      system      -               -               -               -       -
//  update_item system      item            value           -               -       -
//  start       shell       -               -               -               -       -
//  cmd         shell       cmd             -               -               -       -
//  stop        shell       -               -               -               -       -
//  trace       log         0/1             -               -               -       -
//  weather     weather     name            time            temperature     code    -
//  update      weather     -               -               -               -       -
//  get         file        filename        -               -               -       -
//  stop        file        -               -               -               -       -
//  worldtime   worldtime   name            datetime        -               -       -
//  add         todos       title           desc            priority        -       -

pub const ACT_SHOW: &str = "show";
pub const ACT_INIT: &str = "init";
pub const ACT_ASK: &str = "ask";
pub const ACT_REPLY: &str = "reply";
pub const ACT_QUIT: &str = "quit";
pub const ACT_PUBLISH: &str = "publish";
pub const ACT_DISCONNECT: &str = "disconnect";
pub const ACT_WAKE: &str = "wake";
pub const ACT_PING: &str = "ping";
pub const ACT_UPDATE: &str = "update";
pub const ACT_UPDATE_ITEM: &str = "update_item";
pub const ACT_START: &str = "start";
pub const ACT_STOP: &str = "stop";
pub const ACT_CMD: &str = "cmd";
pub const ACT_TRACE: &str = "trace";
pub const ACT_WEATHER: &str = "weather";
pub const ACT_WEATHER_DAILY: &str = "weather_daily";
pub const ACT_PUT: &str = "put";
pub const ACT_FILE: &str = "file";
pub const ACT_WORLDTIME: &str = "worldtime";
pub const ACT_HELP: &str = "help";
pub const ACT_ADD: &str = "add";
pub const ACT_NAS: &str = "nas";

#[derive(Debug, Clone)]
pub enum Reply {
    Device(String),
    Web(Sender<serde_json::Value>),
}

#[derive(Debug, Clone)]
pub struct Cmd {
    pub reply: Reply,
    pub action: String,
    pub data: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Log {
    pub level: log::Level,
    pub msg: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DevInfo {
    pub ts: u64,
    pub name: String,
    pub onboard: Option<bool>,
    pub app_uptime: Option<u64>,
    pub host_uptime: Option<u64>,
    pub version: Option<String>,
    pub temperature: Option<f32>,
    pub os: Option<String>,
    pub cpu_arch: Option<String>,
    pub cpu_usage: Option<f32>,
    pub weather: Option<String>,
    pub last_seen: Option<u64>,
    pub tailscale_ip: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct City {
    pub name: String,
    pub latitude: f32,
    pub longitude: f32,
    pub weather: Option<utils::Weather>,
}

#[derive(Debug, Clone)]
pub struct Worldtime {
    pub name: String,
    pub timezone: String,
    pub datetime: String,
}

impl Worldtime {
    pub fn new(name: String, timezone: String) -> Self {
        Self {
            name,
            timezone,
            datetime: "n/a".to_owned(),
        }
    }
}

pub async fn file_filename(msg_tx: &Sender<Msg>, reply: Reply, filename: String, sequence: usize) {
    msg_tx
        .send(Msg {
            ts: utils::ts(),
            plugin: plugin_mqtt::NAME.to_owned(),
            data: Data::Cmd(Cmd {
                reply,
                action: ACT_FILE.to_owned(),
                data: vec!["filename".to_owned(), filename, sequence.to_string()],
            }),
        })
        .await
        .unwrap();
}

pub async fn file_content(msg_tx: &Sender<Msg>, reply: Reply, sequence: usize, content: &[u8]) {
    let content = ascii85::encode(content);

    msg_tx
        .send(Msg {
            ts: utils::ts(),
            plugin: plugin_mqtt::NAME.to_owned(),
            data: Data::Cmd(Cmd {
                reply,
                action: ACT_FILE.to_owned(),
                data: vec!["content".to_owned(), sequence.to_string(), content],
            }),
        })
        .await
        .unwrap();
}

pub async fn file_end(msg_tx: &Sender<Msg>, reply: Reply, sequence: usize) {
    msg_tx
        .send(Msg {
            ts: utils::ts(),
            plugin: plugin_mqtt::NAME.to_owned(),
            data: Data::Cmd(Cmd {
                reply,
                action: ACT_FILE.to_owned(),
                data: vec!["end".to_owned(), sequence.to_string()],
            }),
        })
        .await
        .unwrap();
}

pub async fn log(msg_tx: &Sender<Msg>, reply: Reply, level: log::Level, msg: String) {
    match reply {
        Reply::Device(device) => {
            if device == cfg::name() {
                msg_tx
                    .send(Msg {
                        ts: utils::ts(),
                        plugin: plugin_log::NAME.to_owned(),
                        data: Data::Log(Log { level, msg }),
                    })
                    .await
                    .unwrap();
            } else {
                msg_tx
                    .send(Msg {
                        ts: utils::ts(),
                        plugin: plugin_mqtt::NAME.to_owned(),
                        data: Data::Cmd(Cmd {
                            reply: Reply::Device(device),
                            action: ACT_REPLY.to_owned(),
                            data: vec![level.to_string(), msg],
                        }),
                    })
                    .await
                    .unwrap();
            }
        }
        Reply::Web(sender) => {
            if let Err(e) = sender
                .send(serde_json::json!(vec![level.to_string(), msg]))
                .await
            {
                eprintln!("Failed to send response: {:?}", e);
            }
        }
    }
}

pub async fn devices(msg_tx: &Sender<Msg>, devices: Vec<DevInfo>) {
    msg_tx
        .send(Msg {
            ts: utils::ts(),
            plugin: panels_main::NAME.to_owned(),
            data: Data::Devices(devices),
        })
        .await
        .unwrap();
}

pub async fn device_countdown(msg_tx: &Sender<Msg>) {
    msg_tx
        .send(Msg {
            ts: utils::ts(),
            plugin: panels_main::NAME.to_owned(),
            data: Data::DeviceCountdown,
        })
        .await
        .unwrap();
}

pub async fn device_update(msg_tx: &Sender<Msg>, device: DevInfo) {
    msg_tx
        .send(Msg {
            ts: utils::ts(),
            plugin: plugin_devices::NAME.to_owned(),
            data: Data::DeviceUpdate(device),
        })
        .await
        .unwrap();
}

pub async fn weather(msg_tx: &Sender<Msg>, weather: Vec<City>) {
    msg_tx
        .send(Msg {
            ts: utils::ts(),
            plugin: panels_main::NAME.to_owned(),
            data: Data::Weather(weather),
        })
        .await
        .unwrap();
}

pub async fn worldtime(msg_tx: &Sender<Msg>, worldtime: Vec<Worldtime>) {
    msg_tx
        .send(Msg {
            ts: utils::ts(),
            plugin: panels_main::NAME.to_owned(),
            data: Data::Worldtime(worldtime),
        })
        .await
        .unwrap();
}

#[allow(clippy::too_many_arguments)]
pub async fn cmd(
    msg_tx: &Sender<Msg>,
    reply: Reply,
    plugin: String,
    action: String,
    data: Vec<String>,
) {
    msg_tx
        .send(Msg {
            ts: utils::ts(),
            plugin,
            data: Data::Cmd(Cmd {
                reply,
                action,
                data,
            }),
        })
        .await
        .unwrap();
}

#[allow(clippy::too_many_arguments)]
pub async fn nas_file(
    msg_tx: &Sender<Msg>,
    reply: Reply,
    action: &str,
    tailscale_ip: String,
    device_name: String,
    stage: String,
    filename: String,
    md5: String,
    modified: String,
) {
    msg_tx
        .send(Msg {
            ts: utils::ts(),
            plugin: plugin_mqtt::NAME.to_owned(),
            data: Data::Cmd(Cmd {
                reply,
                action: ACT_NAS.to_owned(),
                data: vec![
                    action.to_string(),
                    tailscale_ip,
                    device_name,
                    stage,
                    filename,
                    md5,
                    modified,
                ],
            }),
        })
        .await
        .unwrap();
}

pub async fn tailscale_ip(msg_tx: &Sender<Msg>, tailscale_ip: &str) {
    msg_tx
        .send(Msg {
            ts: utils::ts(),
            plugin: plugin_nas::NAME.to_owned(),
            data: Data::TailscaleIP(tailscale_ip.to_owned()),
        })
        .await
        .unwrap();
}
