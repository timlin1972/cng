use async_trait::async_trait;
use log::Level::{Error, Info, Trace};
use tokio::sync::mpsc::Sender;

use crate::cfg;
use crate::msg::{self, devices, log, Cmd, Data, DevInfo, Msg};
use crate::plugins::{plugin_mqtt, plugins_main};
use crate::utils;

pub const NAME: &str = "devices";

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
        async fn ask_device_update(msg_tx: &Sender<Msg>, device_name: &str) {
            msg::cmd(
                msg_tx,
                cfg::name(),
                plugin_mqtt::NAME.to_owned(),
                msg::ACT_ASK.to_owned(),
                vec![
                    device_name.to_owned(),
                    "p".to_owned(),
                    "system".to_owned(),
                    "update".to_owned(),
                ],
            )
            .await;
        }

        if let Some(d) = self.devices.iter_mut().find(|d| d.name == device.name) {
            d.ts = device.ts;
            if device.onboard.is_some() {
                // ask system update if onboard from false to true
                if device.onboard.unwrap() && (d.onboard.is_none() || !d.onboard.unwrap()) {
                    ask_device_update(&self.msg_tx, &device.name).await;
                }
                d.onboard = device.onboard;
            }
            if device.uptime.is_some() {
                d.uptime = device.uptime;
            }
            if device.version.is_some() {
                d.version = device.version.clone();
            }
            if device.temperature.is_some() {
                d.temperature = device.temperature;
            }
            if device.weather.is_some() {
                d.weather = device.weather.clone();
            }

            // clear all if not onboard
            if device.onboard.is_some() && !device.onboard.unwrap() {
                d.uptime = None;
                d.version = None;
                d.temperature = None;
                d.weather = None;
            }
        } else {
            self.devices.push(device.clone());
            if device.onboard.is_some() && device.onboard.unwrap() {
                ask_device_update(&self.msg_tx, &device.name).await;
            }
        }

        devices(&self.msg_tx, self.devices.clone()).await;
    }

    async fn init(&mut self) {
        log(&self.msg_tx, cfg::name(), Trace, format!("[{NAME}] init")).await;
    }

    async fn show_device(&self, cmd: &Cmd, device: &DevInfo) {
        // name
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            device.name.to_string(),
        )
        .await;

        // onboard
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!(
                "    Onboard: {}",
                if device.onboard.unwrap() { "On" } else { "off" }
            ),
        )
        .await;

        // uptime
        let uptime = if let Some(t) = device.uptime {
            utils::uptime_str(t)
        } else {
            "n/a".to_owned()
        };
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("    Uptime: {uptime}"),
        )
        .await;

        // version
        let version = device.version.clone().unwrap_or("n/a".to_owned());
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("    Version: {version}"),
        )
        .await;

        // temperature
        let temperature = if let Some(t) = device.temperature {
            format!("{:.1}", t)
        } else {
            "n/a".to_owned()
        };
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("    Temperature: {temperature}°C"),
        )
        .await;

        // weather
        let weather = device.weather.clone().unwrap_or("n/a".to_owned());
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("    Weather: {weather}"),
        )
        .await;

        // last update
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("    Last update: {}", utils::ts_str_full(device.ts)),
        )
        .await;
    }

    async fn show(&mut self, cmd: &Cmd) {
        for device in &self.devices {
            if let Some(t) = &cmd.data.first() {
                if *t == &device.name {
                    self.show_device(cmd, device).await;
                }
            } else {
                self.show_device(cmd, device).await;
                // log(
                //     &self.msg_tx,
                //     cmd.reply.clone(),
                //     Info,
                //     format!("{}: {}", device.name, if device.onboard.unwrap() { "On" } else { "off" }),
                // )
                // .await;
            }
        }
    }
}

#[async_trait]
impl plugins_main::Plugin for Plugin {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    async fn msg(&mut self, msg: &Msg) -> bool {
        match &msg.data {
            Data::Cmd(cmd) => match cmd.action.as_str() {
                msg::ACT_INIT => self.init().await,
                msg::ACT_SHOW => self.show(cmd).await,
                _ => {
                    log(
                        &self.msg_tx,
                        cmd.reply.clone(),
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
                    cfg::name(),
                    Error,
                    format!("[{NAME}] unknown msg: {msg:?}"),
                )
                .await;
            }
        }

        false
    }
}
