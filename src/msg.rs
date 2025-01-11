use tokio::sync::mpsc::Sender;

#[derive(Debug, Clone)]
pub enum Data {
    Log(Log),
    Devices(Vec<DevInfo>),
    DeviceUpdate(DevInfo),
    // ...
}

#[derive(Debug)]
pub struct Msg {
    pub plugin: String,
    pub data: Data,
}

#[derive(Debug, Clone)]
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
    msg_tx
        .send(Msg {
            plugin: "log".to_owned(),
            data: Data::Log(Log { level, msg }),
        })
        .await
        .unwrap();
}

pub async fn devices(msg_tx: &Sender<Msg>, devices: Vec<DevInfo>) {
    msg_tx
        .send(Msg {
            plugin: "panels".to_owned(),
            data: Data::Devices(devices),
        })
        .await
        .unwrap();
}

pub async fn device_update(msg_tx: &Sender<Msg>, device: DevInfo) {
    msg_tx
        .send(Msg {
            plugin: "devices".to_owned(),
            data: Data::DeviceUpdate(device),
        })
        .await
        .unwrap();
}
