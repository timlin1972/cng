use async_trait::async_trait;
use log::Level::{Error, Info, Trace};
use serde::Serialize;
use tokio::sync::mpsc::Sender;

use crate::msg::{self, log, Cmd, Data, Msg, Reply};
use crate::plugins::{plugin_mqtt, plugins_main};
use crate::{cfg, utils};

pub const NAME: &str = "system";
const VERSION: &str = "0.2.6";
const ONBOARD_POLLING: u64 = 300;

fn get_temperature() -> f32 {
    let components = sysinfo::Components::new_with_refreshed_list();
    for component in &components {
        if component.label().to_ascii_lowercase().contains("cpu") {
            return component.temperature().unwrap_or(0.0);
        }
    }

    0.0
}

#[derive(Debug)]
struct Device {
    name: String,
    temperature: f32,
    weather: String,
    ts_start: u64,
    tailscale_ip: String,
}

#[derive(Debug, Serialize)]
struct DeviceForWeb {
    name: String,
    app_uptime: u64,
    host_uptime: u64,
    temperature: f32,
    weather: String,
    tailscale_ip: String,
}

#[derive(Debug)]
pub struct Plugin {
    msg_tx: Sender<Msg>,
    name: String,
    device: Device,
}

impl Plugin {
    pub fn new(msg_tx: Sender<Msg>) -> Self {
        let device = Device {
            name: cfg::name().to_owned(),
            temperature: 0.0,
            weather: "n/a".to_owned(),
            ts_start: utils::uptime(),
            tailscale_ip: "n/a".to_owned(),
        };

        Self {
            msg_tx,
            device,
            name: NAME.to_owned(),
        }
    }

    async fn init(&mut self) {
        let msg_tx_clone = self.msg_tx.clone();
        tokio::spawn(async move {
            loop {
                // tailscale ip
                let tailscale_ip = utils::get_tailscale_ip();
                msg::cmd(
                    &msg_tx_clone,
                    Reply::Device(cfg::name()),
                    NAME.to_owned(),
                    msg::ACT_UPDATE_ITEM.to_owned(),
                    vec!["tailscale_ip".to_owned(), tailscale_ip.clone()],
                )
                .await;
                msg::tailscale_ip(&msg_tx_clone, &tailscale_ip).await;

                // weather
                let weather = utils::device_weather().await;
                msg::cmd(
                    &msg_tx_clone,
                    Reply::Device(cfg::name()),
                    NAME.to_owned(),
                    msg::ACT_UPDATE_ITEM.to_owned(),
                    vec!["weather".to_owned(), weather],
                )
                .await;

                // temperature
                let temperature = get_temperature();
                msg::cmd(
                    &msg_tx_clone,
                    Reply::Device(cfg::name()),
                    NAME.to_owned(),
                    msg::ACT_UPDATE_ITEM.to_owned(),
                    vec!["temperature".to_owned(), temperature.to_string()],
                )
                .await;

                msg::cmd(
                    &msg_tx_clone,
                    Reply::Device(cfg::name()),
                    NAME.to_owned(),
                    msg::ACT_UPDATE.to_owned(),
                    vec![],
                )
                .await;

                tokio::time::sleep(tokio::time::Duration::from_secs(ONBOARD_POLLING)).await;
            }
        });

        log(
            &self.msg_tx,
            Reply::Device(cfg::name()),
            Trace,
            format!("[{NAME}] init"),
        )
        .await;
    }

    async fn show(&mut self, cmd: &Cmd) {
        match &cmd.reply {
            Reply::Device(_) => {
                // app uptime
                log(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    Info,
                    format!(
                        "[{NAME}] App uptime: {}",
                        utils::uptime_str(utils::uptime() - self.device.ts_start)
                    ),
                )
                .await;

                // host uptime
                log(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    Info,
                    format!(
                        "[{NAME}] Host uptime: {}",
                        utils::uptime_str(utils::uptime())
                    ),
                )
                .await;

                // tailscale ip
                log(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    Info,
                    format!("[{NAME}] Tailscale IP: {}", self.device.tailscale_ip),
                )
                .await;

                // version
                log(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    Info,
                    format!("[{NAME}] Version: {VERSION}"),
                )
                .await;

                // temperature
                log(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    Info,
                    format!("[{NAME}] Temperature: {:.1}°C", get_temperature()),
                )
                .await;

                // weather
                log(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    Info,
                    format!("[{NAME}] Weather: {}", self.device.weather),
                )
                .await;
            }
            Reply::Web(sender) => {
                let device_for_web = DeviceForWeb {
                    name: self.device.name.clone(),
                    app_uptime: utils::uptime() - self.device.ts_start,
                    host_uptime: utils::uptime(),
                    temperature: get_temperature(),
                    weather: self.device.weather.clone(),
                    tailscale_ip: self.device.tailscale_ip.clone(),
                };
                sender
                    .send(serde_json::to_value(device_for_web).unwrap())
                    .await
                    .unwrap();
            }
        }
    }

    // self update
    async fn update_item(&mut self, cmd: &Cmd) {
        match cmd.data.first().unwrap().as_str() {
            "weather" => {
                self.device.weather = cmd.data.get(1).unwrap().to_owned();
            }
            "temperature" => {
                let temperature = cmd.data.get(1).unwrap().parse::<f32>().unwrap_or(0.0);
                self.device.temperature = temperature;
            }
            "tailscale_ip" => {
                self.device.tailscale_ip = cmd.data.get(1).unwrap().to_owned();
            }
            _ => {
                log(
                    &self.msg_tx,
                    Reply::Device(cfg::name()),
                    Error,
                    format!("[{NAME}] unknown item: {:?}", cmd.data.first().unwrap()),
                )
                .await;
            }
        }
    }

    async fn update(&mut self, cmd: &Cmd) {
        // onboard
        msg::cmd(
            &self.msg_tx,
            cmd.reply.clone(),
            plugin_mqtt::NAME.to_owned(),
            msg::ACT_PUBLISH.to_owned(),
            vec!["onboard".to_owned(), "true".to_owned(), "1".to_owned()],
        )
        .await;

        // app uptime
        let uptime = utils::uptime() - self.device.ts_start;
        msg::cmd(
            &self.msg_tx,
            cmd.reply.clone(),
            plugin_mqtt::NAME.to_owned(),
            msg::ACT_PUBLISH.to_owned(),
            vec![
                "app_uptime".to_owned(),
                "false".to_owned(),
                uptime.to_string(),
            ],
        )
        .await;

        // host uptime
        let uptime = utils::uptime();
        msg::cmd(
            &self.msg_tx,
            cmd.reply.clone(),
            plugin_mqtt::NAME.to_owned(),
            msg::ACT_PUBLISH.to_owned(),
            vec![
                "host_uptime".to_owned(),
                "false".to_owned(),
                uptime.to_string(),
            ],
        )
        .await;

        // tailscale ip
        msg::cmd(
            &self.msg_tx,
            cmd.reply.clone(),
            plugin_mqtt::NAME.to_owned(),
            msg::ACT_PUBLISH.to_owned(),
            vec![
                "tailscale_ip".to_owned(),
                "false".to_owned(),
                self.device.tailscale_ip.clone(),
            ],
        )
        .await;

        // version
        msg::cmd(
            &self.msg_tx,
            cmd.reply.clone(),
            plugin_mqtt::NAME.to_owned(),
            msg::ACT_PUBLISH.to_owned(),
            vec!["version".to_owned(), "false".to_owned(), VERSION.to_owned()],
        )
        .await;

        // temperature
        msg::cmd(
            &self.msg_tx,
            cmd.reply.clone(),
            plugin_mqtt::NAME.to_owned(),
            msg::ACT_PUBLISH.to_owned(),
            vec![
                "temperature".to_owned(),
                "false".to_owned(),
                self.device.temperature.to_string(),
            ],
        )
        .await;

        // weather
        msg::cmd(
            &self.msg_tx,
            cmd.reply.clone(),
            plugin_mqtt::NAME.to_owned(),
            msg::ACT_PUBLISH.to_owned(),
            vec![
                "weather".to_owned(),
                "false".to_owned(),
                self.device.weather.clone(),
            ],
        )
        .await;
    }

    async fn help(&mut self) {
        log(
            &self.msg_tx,
            Reply::Device(cfg::name()),
            Info,
            format!("[{NAME}] help: init, show, update, update_item, quit",),
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
        let mut ret = false;
        match &msg.data {
            Data::Cmd(cmd) => match cmd.action.as_str() {
                msg::ACT_HELP => self.help().await,
                msg::ACT_INIT => self.init().await,
                msg::ACT_SHOW => self.show(cmd).await,
                msg::ACT_UPDATE => self.update(cmd).await,
                msg::ACT_UPDATE_ITEM => self.update_item(cmd).await,
                msg::ACT_QUIT => {
                    ret = true;
                }
                _ => {
                    log(
                        &self.msg_tx,
                        Reply::Device(cfg::name()),
                        Error,
                        format!("[{NAME}] unknown action: {:?}", cmd.action),
                    )
                    .await;
                }
            },
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

        ret
    }
}
