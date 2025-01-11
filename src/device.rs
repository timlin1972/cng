use tokio::sync::mpsc::Sender;

use crate::msg::{self, DevInfo, Msg};

const MODULE: &str = "device";

pub struct Device {
    msg_tx: Sender<Msg>,
    devinfos: Vec<DevInfo>,
}

impl Device {
    pub fn new(msg_tx: Sender<Msg>) -> Self {
        Self {
            msg_tx,
            devinfos: vec![],
        }
    }

    pub async fn device_update(&mut self, device: DevInfo) {
        self.devinfos.push(device);
        msg::devices(&self.msg_tx, self.devinfos.clone()).await;
    }
}
