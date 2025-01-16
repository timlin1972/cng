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
    Cmd(Cmd),
}

#[derive(Debug)]
pub struct Msg {
    pub ts: u64,
    pub plugin: String,
    pub data: Data,
}

//              plugin      data[0]             data[1] data[2] data[3] data[4]
//  show        devices     device (optional)   -       -       -       -
//  show        others      -                   -       -       -       -
//  init        all         -                   -       -       -       -
//  ask         mqtt        target_device       p       plugin  action  -
//  reply       all         level               msg     -       -       -
//  quit        all         -                   -       -       -       -
//  publish     mqtt        topic               retain  payload -       -
//  disconnect  mqtt        -                   -       -       -       -
//  wake        wol         device              -       -       -       -
//  ping        ping        ip                  -       -       -       -
//  countdown   devices     -                   -       -       -       -
//  update      system      -                   -       -       -       -
//  start       shell       -                   -       -       -       -
//  cmd         shell       cmd                 -       -       -       -
//  stop        shell       -                   -       -       -       -
pub const ACT_SHOW: &str = "show";
pub const ACT_INIT: &str = "init";
pub const ACT_ASK: &str = "ask";
pub const ACT_REPLY: &str = "reply";
pub const ACT_QUIT: &str = "quit";
pub const ACT_PUBLISH: &str = "publish";
pub const ACT_DISCONNECT: &str = "disconnect";
pub const ACT_WAKE: &str = "wake";
pub const ACT_PING: &str = "ping";
pub const ACT_COUNTDOWN: &str = "countdown";
pub const ACT_UPDATE: &str = "update";
pub const ACT_START: &str = "start";
pub const ACT_STOP: &str = "stop";
pub const ACT_CMD: &str = "cmd";

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
                    action: "reply".to_owned(),
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
