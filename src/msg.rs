use tokio::sync::mpsc::Sender;

use crate::cfg;
use crate::panels::panels_main;
use crate::plugins::{plugin_devices, plugin_log, plugin_mqtt};
use crate::utils;

#[derive(Debug, Clone)]
pub enum Data {
    Log(Log),
    Devices(Vec<DevInfo>),
    DeviceUpdate(DevInfo),
    DeviceCountdown,
    Weather(Vec<City>),
    Cmd(Cmd),
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
pub const ACT_PUT: &str = "put";
pub const ACT_FILE: &str = "file";

#[derive(Debug, Clone)]
pub struct Cmd {
    pub reply: String,
    pub action: String,
    pub data: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Log {
    pub level: log::Level,
    pub msg: String,
}

#[derive(Debug, Clone)]
pub struct DevInfo {
    pub ts: u64,
    pub name: String,
    pub onboard: Option<bool>,
    pub uptime: Option<u64>,
    pub version: Option<String>,
    pub temperature: Option<f32>,
    pub weather: Option<String>,
    pub last_seen: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct City {
    pub name: String,
    pub latitude: f32,
    pub longitude: f32,
    pub ts: Option<i64>,
    pub temperature: Option<f32>,
    pub code: Option<u8>,
}

pub async fn file_filename(msg_tx: &Sender<Msg>, reply: String, filename: String, sequence: usize) {
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

pub async fn file_content(msg_tx: &Sender<Msg>, reply: String, sequence: usize, content: &[u8]) {
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

pub async fn file_end(msg_tx: &Sender<Msg>, reply: String, sequence: usize) {
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

pub async fn log(msg_tx: &Sender<Msg>, reply: String, level: log::Level, msg: String) {
    if reply == cfg::name() {
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
                    reply,
                    action: ACT_REPLY.to_owned(),
                    data: vec![level.to_string(), msg],
                }),
            })
            .await
            .unwrap();
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

#[allow(clippy::too_many_arguments)]
pub async fn cmd(
    msg_tx: &Sender<Msg>,
    reply: String,
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
