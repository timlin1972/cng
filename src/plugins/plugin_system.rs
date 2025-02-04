use async_trait::async_trait;
use log::Level::{Error, Info, Trace};
use tokio::sync::mpsc::Sender;

use crate::msg::{self, log, Cmd, Data, Msg};
use crate::plugins::{plugin_mqtt, plugins_main};
use crate::{cfg, utils};

pub const NAME: &str = "system";
const VERSION: &str = "0.1.5";
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
pub struct Plugin {
    name: String,
    msg_tx: Sender<Msg>,
    temperature: f32,
    weather: String,
    ts_start: u64,
}

impl Plugin {
    pub fn new(msg_tx: Sender<Msg>) -> Self {
        Self {
            name: NAME.to_owned(),
            msg_tx,
            temperature: 0.0,
            weather: "n/a".to_owned(),
            ts_start: utils::uptime(),
        }
    }

    async fn init(&mut self) {
        let msg_tx_clone = self.msg_tx.clone();
        tokio::spawn(async move {
            loop {
                // weather
                let weather = utils::device_weather().await;
                msg::cmd(
                    &msg_tx_clone,
                    cfg::name(),
                    NAME.to_owned(),
                    msg::ACT_UPDATE_ITEM.to_owned(),
                    vec!["weather".to_owned(), weather],
                )
                .await;

                // temperature
                let temperature = get_temperature();
                msg::cmd(
                    &msg_tx_clone,
                    cfg::name(),
                    NAME.to_owned(),
                    msg::ACT_UPDATE_ITEM.to_owned(),
                    vec!["temperature".to_owned(), temperature.to_string()],
                )
                .await;

                msg::cmd(
                    &msg_tx_clone,
                    cfg::name(),
                    NAME.to_owned(),
                    msg::ACT_UPDATE.to_owned(),
                    vec![],
                )
                .await;

                tokio::time::sleep(tokio::time::Duration::from_secs(ONBOARD_POLLING)).await;
            }
        });

        log(&self.msg_tx, cfg::name(), Trace, format!("[{NAME}] init")).await;
    }

    async fn show(&mut self, cmd: &Cmd) {
        // app uptime
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!(
                "[{NAME}] App uptime: {}",
                utils::uptime_str(utils::uptime() - self.ts_start)
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
            format!("[{NAME}] Weather: {}", self.weather),
        )
        .await;
    }

    // self update
    async fn update_item(&mut self, cmd: &Cmd) {
        match cmd.data.first().unwrap().as_str() {
            "weather" => {
                self.weather = cmd.data.get(1).unwrap().to_owned();
            }
            "temperature" => {
                let temperature = cmd.data.get(1).unwrap().parse::<f32>().unwrap_or(0.0);
                self.temperature = temperature;
            }
            _ => {
                log(
                    &self.msg_tx,
                    cfg::name(),
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
        let uptime = utils::uptime() - self.ts_start;
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
                self.temperature.to_string(),
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
                self.weather.clone(),
            ],
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
                        cfg::name(),
                        Error,
                        format!("[{NAME}] unknown action: {:?}", cmd.action),
                    )
                    .await;
                }
            },
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

        ret
    }
}
