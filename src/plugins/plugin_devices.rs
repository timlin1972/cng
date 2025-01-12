use async_trait::async_trait;
use log::Level::{Error, Trace};
use tokio::sync::mpsc::Sender;

use crate::msg::{devices, log, Data, DevInfo, Msg};
use crate::plugins::plugins_main;

const NAME: &str = "devices";

#[derive(Debug)]
pub struct Plugin {
    name: String,
    msg_tx: Sender<Msg>,
    devices: Vec<DevInfo>,
}

impl Plugin {
    pub fn new(msg_tx: Sender<Msg>) -> Self {
        Self {
            name: NAME.to_owned(),
            msg_tx,
            devices: vec![],
        }
    }

    async fn device_update(&mut self, device: &DevInfo) {
        if let Some(d) = self.devices.iter_mut().find(|d| d.name == device.name) {
            d.onboard = device.onboard;
            d.ts = device.ts;
        } else {
            self.devices.push(device.clone());
        }

        devices(&self.msg_tx, self.devices.clone()).await;
    }
}

#[async_trait]
impl plugins_main::Plugin for Plugin {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    async fn init(&mut self) {
        log(&self.msg_tx, Trace, format!("[{NAME}] init")).await;
    }

    async fn msg(&mut self, msg: &Msg) {
        match &msg.data {
            Data::DeviceUpdate(device) => {
                self.device_update(device).await;
            }
            _ => {
                log(
                    &self.msg_tx,
                    Error,
                    format!("[{NAME}] unknown msg: {msg:?}"),
                )
                .await;
            }
        }
    }
}
