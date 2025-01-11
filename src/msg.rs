use tokio::sync::mpsc::Sender;

#[derive(Debug)]
pub enum Msg {
    Log(Log),
    Devices(Vec<DevInfo>),
    DeviceUpdate(DevInfo),
    // ...
}

#[derive(Debug)]
pub struct Log {
    pub level: log::Level,
    pub msg: String,
}

#[derive(Debug, Clone)]
pub struct DevInfo {
    pub name: String,
    pub onboard: bool,
}

pub async fn log(msg_tx: &Sender<Msg>, level: log::Level, msg: String) {
    msg_tx.send(Msg::Log(Log { level, msg })).await.unwrap();
}

pub async fn devices(msg_tx: &Sender<Msg>, devices: Vec<DevInfo>) {
    msg_tx.send(Msg::Devices(devices)).await.unwrap();
}

pub async fn device_update(msg_tx: &Sender<Msg>, device: DevInfo) {
    msg_tx.send(Msg::DeviceUpdate(device)).await.unwrap();
}
