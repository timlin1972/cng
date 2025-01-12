use async_trait::async_trait;
use log::Level::{Error, Info, Trace};
use tokio::sync::mpsc::Sender;

use crate::msg::{devices, log, Cmd, Data, DevInfo, Msg};
use crate::plugins::plugins_main;
use crate::utils;

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

    async fn init(&mut self) {
        log(&self.msg_tx, Trace, format!("[{NAME}] init")).await;
    }

    async fn show_device(&self, device: &DevInfo) {
        log(&self.msg_tx, Info, format!("{}", device.name)).await;
        log(
            &self.msg_tx,
            Info,
            format!("    Onboard: {}", if device.onboard { "On" } else { "off" }),
        )
        .await;
        log(
            &self.msg_tx,
            Info,
            format!("    Last update: {}", utils::ts_str_full(device.ts)),
        )
        .await;
    }

    async fn show(&mut self, cmd: &Cmd) {
        for device in &self.devices {
            if let Some(t) = &cmd.data1 {
                if *t == device.name {
                    self.show_device(device).await;
                }
            } else {
                log(&self.msg_tx, Info, format!("{}", device.name)).await;
            }
        }
    }
}

#[async_trait]
impl plugins_main::Plugin for Plugin {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    async fn msg(&mut self, msg: &Msg) {
        match &msg.data {
            Data::Cmd(cmd) => match cmd.action.as_str() {
                "init" => self.init().await,
                "show" => self.show(cmd).await,
                _ => {
                    log(
                        &self.msg_tx,
                        Error,
                        format!("[{NAME}] unknown action: {:?}", cmd.action),
                    )
                    .await;
                }
            },
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
