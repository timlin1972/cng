use crate::msg::{self, DevInfo, Msg};

const MODULE: &str = "device";

pub struct Device {
    msg_tx: tokio::sync::mpsc::Sender<Msg>,
    devinfos: Vec<DevInfo>,
}

impl Device {
    pub fn new(msg_tx: tokio::sync::mpsc::Sender<Msg>) -> Self {
        Self {
            msg_tx,
            devinfos: vec![],
        }
    }

    pub async fn test(&mut self) {
        let devices = vec![
            DevInfo {
                name: "dev1".to_string(),
                onboard: true,
            },
            DevInfo {
                name: "dev2".to_string(),
                onboard: true,
            },
        ];

        msg::devices(&self.msg_tx, devices).await;
    }
}
