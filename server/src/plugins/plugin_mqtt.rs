use async_trait::async_trait;
use log::Level::{Error, Info, Trace};
use rumqttc::{AsyncClient, LastWill, MqttOptions, QoS};
use tokio::sync::mpsc::Sender;

use crate::cfg;
use crate::msg::{self, log, Cmd, Data, Msg};
use crate::plugins::{mqtt, plugins_main};
use crate::utils;

pub const NAME: &str = "mqtt";
const BROKER: &str = "broker.emqx.io";
const MQTT_KEEP_ALIVE: u64 = 180;

#[derive(Debug)]
pub struct Plugin {
    name: String,
    msg_tx: Sender<Msg>,
    client: Option<AsyncClient>,
}

impl Plugin {
    pub fn new(msg_tx: Sender<Msg>) -> Self {
        Self {
            name: NAME.to_owned(),
            msg_tx,
            client: None,
        }
    }

    async fn init(&mut self) {
        log(&self.msg_tx, cfg::name(), Trace, format!("[{NAME}] init")).await;

        log(
            &self.msg_tx,
            cfg::name(),
            Trace,
            format!("[{NAME}] Connecting to MQTT broker"),
        )
        .await;

        // connect to MQTT broker
        let mut mqttoptions = MqttOptions::new(cfg::name(), BROKER, 1883);
        let last_will = LastWill::new(
            format!("tln/{}/onboard", cfg::name()),
            "0",
            QoS::AtMostOnce,
            true,
        );
        mqttoptions
            .set_keep_alive(std::time::Duration::from_secs(MQTT_KEEP_ALIVE))
            .set_last_will(last_will);

        let (client, mut connection) = AsyncClient::new(mqttoptions, 10);

        // subscribe
        mqtt::utils::subscribe(&self.msg_tx, Some(&client), "tln/#").await;

        let msg_tx_clone = self.msg_tx.clone();
        tokio::spawn(async move {
            log(
                &msg_tx_clone,
                cfg::name(),
                Trace,
                format!("[{NAME}] Start to receive mqtt message."),
            )
            .await;

            while let Ok(notification) = connection.poll().await {
                mqtt::utils::process_event(&msg_tx_clone, notification).await;
            }
            log(
                &msg_tx_clone,
                cfg::name(),
                Error,
                format!("[{NAME}] Receive mqtt message stopped."),
            )
            .await;

            // disconnect
            msg::cmd(
                &msg_tx_clone,
                cfg::name(),
                NAME.to_owned(),
                msg::ACT_DISCONNECT.to_owned(),
                vec![],
            )
            .await;
        });

        // keep the following code for reference
        // publish(
        //     &self.msg_tx,
        //     &client,
        //     &format!("tln/moxa/1"),
        //     true,
        //     &"",
        // )
        // .await;

        self.client = Some(client);
    }

    async fn show(&mut self, cmd: &Cmd) {
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("Broker: {BROKER}"),
        )
        .await;
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("Id: {}", cfg::name()),
        )
        .await;
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!(
                "Status: {}",
                if self.client.is_some() {
                    "connected"
                } else {
                    "disconnected"
                }
            ),
        )
        .await;
    }

    async fn reply(&mut self, cmd: &Cmd) {
        let mut msg = String::new();
        for t in &cmd.data {
            msg += t;
            msg += " ";
        }

        let msg = msg.trim();

        let enc_msg = utils::encrypt(&cfg::key(), msg).unwrap();

        mqtt::utils::publish(
            &self.msg_tx,
            self.client.as_ref(),
            &format!("tln/{}/{}", cmd.reply, msg::ACT_REPLY),
            false,
            &enc_msg,
        )
        .await;
    }

    async fn ask(&mut self, cmd: &Cmd) {
        let target_device = match &cmd.data.first() {
            Some(t) => t.to_owned(),
            None => {
                log(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    Error,
                    format!("[{NAME}] Target device is missing."),
                )
                .await;
                return;
            }
        };

        let mut msg = String::new();
        msg += &format!("r {} ", cfg::name());
        for t in &cmd.data[1..] {
            // if t content space
            if t.contains(" ") {
                msg += &format!("\"{t}\"");
            } else {
                msg += t;
            }
            msg += " ";
        }

        let msg = msg.trim();

        let enc_msg = utils::encrypt(&cfg::key(), msg).unwrap();

        mqtt::utils::publish(
            &self.msg_tx,
            self.client.as_ref(),
            &format!("tln/{target_device}/ask"),
            false,
            &enc_msg,
        )
        .await;
    }

    async fn file(&mut self, cmd: &Cmd) {
        let mut msg = String::new();
        for t in &cmd.data[0..] {
            msg += t;
            msg += " ";
        }

        let msg = msg.trim();

        let enc_msg = utils::encrypt(&cfg::key(), msg).unwrap();

        mqtt::utils::publish(
            &self.msg_tx,
            self.client.as_ref(),
            &format!("tln/{}/file", cmd.reply),
            false,
            &enc_msg,
        )
        .await;
    }

    async fn help(&self, cmd: &Cmd) {
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!(
                "[{NAME}] {ACT_HELP}, {ACT_INIT}, {ACT_SHOW}, {ACT_ASK}, {ACT_REPLY}, {ACT_FILE}, {ACT_PUBLISH}, {ACT_DISCONNECT}",
                NAME = NAME,
                ACT_HELP = msg::ACT_HELP,
                ACT_INIT = msg::ACT_INIT,
                ACT_SHOW = msg::ACT_SHOW,
                ACT_ASK = msg::ACT_ASK,
                ACT_REPLY = msg::ACT_REPLY,
                ACT_FILE = msg::ACT_FILE,
                ACT_PUBLISH = msg::ACT_PUBLISH,
                ACT_DISCONNECT = msg::ACT_DISCONNECT,
            ),
        )
        .await;

        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            "p mqtt ask pi5 p system quit".to_owned(),
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
                msg::ACT_HELP => self.help(cmd).await,
                msg::ACT_INIT => self.init().await,
                msg::ACT_SHOW => self.show(cmd).await,
                msg::ACT_ASK => self.ask(cmd).await,
                msg::ACT_REPLY => self.reply(cmd).await,
                msg::ACT_FILE => self.file(cmd).await,
                msg::ACT_PUBLISH => {
                    mqtt::utils::publish(
                        &self.msg_tx,
                        self.client.as_ref(),
                        &format!("tln/{}/{}", cfg::name(), cmd.data[0]),
                        cmd.data[1] == "true",
                        &cmd.data[2],
                    )
                    .await;
                }
                msg::ACT_DISCONNECT => {
                    mqtt::utils::disconnect(&self.msg_tx, self.client.as_ref()).await;
                    self.client = None;
                }
                _ => {
                    log(
                        &self.msg_tx,
                        cfg::name(),
                        Error,
                        format!("[{NAME}] unknown action: {:?}.", cmd.action),
                    )
                    .await;
                }
            },
            _ => {
                log(
                    &self.msg_tx,
                    cfg::name(),
                    Error,
                    format!("[{NAME}] unknown msg: {msg:?}."),
                )
                .await;
            }
        }

        false
    }
}
