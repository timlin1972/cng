use async_channel::Sender;

pub enum Msg {
    Log(Log),
    Devices(Vec<DevInfo>),
    // ...
}

#[derive(Debug)]
pub struct Log {
    pub level: log::Level,
    pub msg: String,
}

#[derive(Debug)]
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
