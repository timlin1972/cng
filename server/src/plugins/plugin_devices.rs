use async_trait::async_trait;
use log::Level::{Error, Info};
use tokio::sync::mpsc::Sender;

use crate::cfg;
use crate::msg::{self, devices, log, Cmd, Data, DevInfo, Msg, Reply};
use crate::plugins::{plugin_mqtt, plugin_nas, plugin_system, plugins_main};
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
                Reply::Device(cfg::name()),
                plugin_mqtt::NAME.to_owned(),
                msg::ACT_ASK.to_owned(),
                vec![
                    device_name.to_owned(),
                    "p".to_owned(),
                    plugin_system::NAME.to_owned(),
                    msg::ACT_UPDATE.to_owned(),
                ],
            )
            .await;

            // if I am NAS, ask others to update NAS but do not ask myself
            if cfg::name() == cfg::nas() && device_name != cfg::nas() {
                msg::cmd(
                    msg_tx,
                    Reply::Device(cfg::name()),
                    plugin_mqtt::NAME.to_owned(),
                    msg::ACT_ASK.to_owned(),
                    vec![
                        device_name.to_owned(),
                        "p".to_owned(),
                        plugin_nas::NAME.to_owned(),
                        msg::ACT_UPDATE.to_owned(),
                    ],
                )
                .await;
            }
        }

        if let Some(d) = self.devices.iter_mut().find(|d| d.name == device.name) {
            d.ts = device.ts;
            // log if onboard is changed
            if device.onboard.is_some() && (device.onboard != d.onboard) {
                log(
                    &self.msg_tx,
                    Reply::Device(cfg::name()),
                    Info,
                    format!(
                        "[{NAME}] device '{}' {} at {}",
                        device.name,
                        if device.onboard.unwrap() {
                            "onboard"
                        } else {
                            "offboard"
                        },
                        utils::ts_str_full(utils::ts()),
                    ),
                )
                .await;
            }
            if device.onboard.is_some() {
                // ask system update if onboard from false to true
                if device.onboard.unwrap() && (d.onboard.is_none() || !d.onboard.unwrap()) {
                    ask_device_update(&self.msg_tx, &device.name).await;
                }
                d.onboard = device.onboard;
            }
            if device.app_uptime.is_some() {
                d.app_uptime = device.app_uptime;
            }
            if device.host_uptime.is_some() {
                d.host_uptime = device.host_uptime;
            }
            if device.tailscale_ip.is_some() {
                d.tailscale_ip = device.tailscale_ip.clone();
            }
            if device.version.is_some() {
                d.version = device.version.clone();
            }
            if device.temperature.is_some() {
                d.temperature = device.temperature;
            }
            if device.os.is_some() {
                d.os = device.os.clone();
            }
            if device.cpu_arch.is_some() {
                d.cpu_arch = device.cpu_arch.clone();
            }
            if device.cpu_usage.is_some() {
                d.cpu_usage = device.cpu_usage;
            }
            if device.weather.is_some() {
                d.weather = device.weather.clone();
            }
            if device.last_seen.is_some() {
                d.last_seen = device.last_seen;
            }

            // clear all if not onboard
            if device.onboard.is_some() && !device.onboard.unwrap() {
                d.app_uptime = None;
                d.host_uptime = None;
                d.tailscale_ip = None;
                d.version = None;
                d.temperature = None;
                d.os = None;
                d.cpu_arch = None;
                d.cpu_usage = None;
                d.weather = None;
                // d.last_seen = None;  // keep last_seen
            }
        } else {
            self.devices.push(device.clone());
            if device.onboard.is_some() {
                log(
                    &self.msg_tx,
                    Reply::Device(cfg::name()),
                    Info,
                    format!(
                        "[{NAME}] device '{}' {} at {}",
                        device.name,
                        if device.onboard.unwrap() {
                            "onboard"
                        } else {
                            "offboard"
                        },
                        utils::ts_str_full(utils::ts()),
                    ),
                )
                .await;
            }
            if device.onboard.is_some() && device.onboard.unwrap() {
                ask_device_update(&self.msg_tx, &device.name).await;
            }
        }

        // if no cfg::nas() in devices or not onboard, ask NAS to unsync
        if !self.devices.iter().any(|d| d.name == cfg::nas()) {
            msg::cmd(
                &self.msg_tx,
                Reply::Device(cfg::name()),
                plugin_nas::NAME.to_owned(),
                msg::ACT_NAS.to_owned(),
                vec!["sync".to_owned(), false.to_string()],
            )
            .await;
        } else {
            let nas = self.devices.iter().find(|d| d.name == cfg::nas()).unwrap();
            if nas.onboard.is_some() && !nas.onboard.unwrap() {
                msg::cmd(
                    &self.msg_tx,
                    Reply::Device(cfg::name()),
                    plugin_nas::NAME.to_owned(),
                    msg::ACT_NAS.to_owned(),
                    vec!["sync".to_owned(), false.to_string()],
                )
                .await;
            }
        }

        devices(&self.msg_tx, self.devices.clone()).await;

        // send to plugin_nas
        self.msg_tx
            .send(Msg {
                ts: utils::ts(),
                plugin: plugin_nas::NAME.to_owned(),
                data: Data::Devices(self.devices.clone()),
            })
            .await
            .unwrap();
    }

    async fn init(&mut self) {
        log(
            &self.msg_tx,
            Reply::Device(cfg::name()),
            Info,
            format!("[{NAME}] init"),
        )
        .await;
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

        // app uptime
        let app_uptime = if let Some(t) = device.app_uptime {
            utils::uptime_str(t)
        } else {
            "n/a".to_owned()
        };
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("    App uptime: {app_uptime}"),
        )
        .await;

        // host uptime
        let host_uptime = if let Some(t) = device.host_uptime {
            utils::uptime_str(t)
        } else {
            "n/a".to_owned()
        };
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("    Host uptime: {host_uptime}"),
        )
        .await;

        // tailscale ip
        let tailscale_ip = device.tailscale_ip.clone().unwrap_or("n/a".to_owned());
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("    Tailscale IP: {tailscale_ip}"),
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
            format!("{:.1}°C", t)
        } else {
            "n/a".to_owned()
        };
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("    Temperature: {temperature}"),
        )
        .await;

        // os
        let os = device.os.clone().unwrap_or("n/a".to_owned());
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("    OS: {os}"),
        )
        .await;

        // cpu arch
        let cpu_arch = device.cpu_arch.clone().unwrap_or("n/a".to_owned());
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("    CPU Arch: {cpu_arch}"),
        )
        .await;

        // cpu usage
        let cpu_usage = if let Some(t) = device.cpu_usage {
            format!("{:.1}%", t)
        } else {
            "n/a".to_owned()
        };
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("    CPU Usage: {cpu_usage}"),
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
        match &cmd.reply {
            Reply::Device(_) => {
                for device in &self.devices {
                    if let Some(t) = &cmd.data.first() {
                        if *t == &device.name {
                            self.show_device(cmd, device).await;
                        }
                    } else {
                        self.show_device(cmd, device).await;
                    }
                }
            }
            Reply::Web(sender) => {
                sender
                    .send(serde_json::to_value(self.devices.clone()).unwrap())
                    .await
                    .unwrap();
            }
        }
    }

    async fn help(&self, cmd: &Cmd) {
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!(
                "[{NAME}] {ACT_INIT}, {ACT_HELP}, {ACT_SHOW} [device]",
                NAME = NAME,
                ACT_INIT = msg::ACT_INIT,
                ACT_HELP = msg::ACT_HELP,
                ACT_SHOW = msg::ACT_SHOW,
            ),
        )
        .await;
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
                msg::ACT_HELP => self.help(cmd).await,
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
                    Reply::Device(cfg::name()),
                    Error,
                    format!("[{NAME}] unknown msg: {msg:?}"),
                )
                .await;
            }
        }

        false
    }
}
