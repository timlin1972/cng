use tokio::sync::mpsc::Sender;

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

#[derive(Debug, Clone)]
pub struct Cmd {
    pub action: String,
    pub data1: Option<String>,
    pub data2: Option<String>,
    pub data3: Option<String>,
    pub data4: Option<String>,
    pub data5: Option<String>,
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
    pub onboard: bool,
}

pub async fn log(msg_tx: &Sender<Msg>, level: log::Level, msg: String) {
    msg_tx
        .send(Msg {
            ts: utils::ts(),
            plugin: "log".to_owned(),
            data: Data::Log(Log { level, msg }),
        })
        .await
        .unwrap();
}

pub async fn devices(msg_tx: &Sender<Msg>, devices: Vec<DevInfo>) {
    msg_tx
        .send(Msg {
            ts: utils::ts(),
            plugin: "panels".to_owned(),
            data: Data::Devices(devices),
        })
        .await
        .unwrap();
}

pub async fn device_update(msg_tx: &Sender<Msg>, device: DevInfo) {
    msg_tx
        .send(Msg {
            ts: utils::ts(),
            plugin: "devices".to_owned(),
            data: Data::DeviceUpdate(device),
        })
        .await
        .unwrap();
}

pub async fn cmd(
    msg_tx: &Sender<Msg>,
    plugin: String,
    action: String,
    data1: Option<String>,
    data2: Option<String>,
    data3: Option<String>,
    data4: Option<String>,
    data5: Option<String>,
) {
    msg_tx
        .send(Msg {
            ts: utils::ts(),
            plugin,
            data: Data::Cmd(Cmd {
                action,
                data1,
                data2,
                data3,
                data4,
                data5,
            }),
        })
        .await
        .unwrap();
}
