use async_trait::async_trait;
use log::Level::{Error, Info, Trace};
use rumqttc::{AsyncClient, Event, LastWill, MqttOptions, Outgoing, Packet, Publish, QoS};
use tokio::sync::mpsc::Sender;

use crate::msg::{self, device_update, log, Cmd, Data, DevInfo, Msg};
use crate::plugins::plugins_main;
use crate::{cfg, utils};

pub const NAME: &str = "mqtt";
const BROKER: &str = "broker.emqx.io";

#[derive(Debug)]
pub struct Plugin {
    name: String,
    msg_tx: Sender<Msg>,
    client: Option<rumqttc::AsyncClient>,
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
        log(
            &self.msg_tx,
            cfg::get_name(),
            Trace,
            format!("[{NAME}] init"),
        )
        .await;

        log(
            &self.msg_tx,
            cfg::get_name(),
            Trace,
            format!("[{NAME}] Connecting to MQTT broker"),
        )
        .await;

        // connect to MQTT broker
        let mut mqttoptions = MqttOptions::new(cfg::get_name(), BROKER, 1883);
        let last_will = LastWill::new(
            format!("tln/{}/onboard", cfg::get_name()),
            "0",
            QoS::AtMostOnce,
            true,
        );
        mqttoptions
            .set_keep_alive(std::time::Duration::from_secs(5))
            .set_last_will(last_will);

        let (client, mut connection) = AsyncClient::new(mqttoptions, 10);

        let msg_tx_clone = self.msg_tx.clone();
        tokio::spawn(async move {
            log(
                &msg_tx_clone,
                cfg::get_name(),
                Trace,
                format!("[{NAME}] Start to receive mqtt message."),
            )
            .await;

            while let Ok(notification) = connection.poll().await {
                match notification {
                    Event::Incoming(Packet::PingResp)
                    | Event::Outgoing(Outgoing::PingReq)
                    | Event::Outgoing(Outgoing::Publish(_))
                    | Event::Outgoing(Outgoing::Subscribe(_))
                    | Event::Incoming(Packet::SubAck(_))
                    | Event::Incoming(Packet::PubAck(_)) => (),
                    Event::Incoming(Packet::Publish(publish)) => {
                        process_event_publish(&msg_tx_clone, &publish).await;
                    }
                    _ => {
                        log(
                            &msg_tx_clone,
                            cfg::get_name(),
                            Trace,
                            format!("[{NAME}] {notification:?}."),
                        )
                        .await;
                    }
                }
            }
        });

        // subscribe
        subscribe(&self.msg_tx, &client, "tln/#").await;

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

    async fn show(&mut self) {
        log(
            &self.msg_tx,
            cfg::get_name(),
            Info,
            format!("Broker: {BROKER}"),
        )
        .await;
        log(
            &self.msg_tx,
            cfg::get_name(),
            Info,
            format!("Id: {}", cfg::get_name()),
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

        let enc_msg = utils::encrypt(&cfg::get_key(), msg).unwrap();

        publish(
            &self.msg_tx,
            self.client.as_ref().unwrap(),
            &format!("tln/{}/{}", cmd.reply, msg::ACT_REPLY),
            false,
            &enc_msg,
        )
        .await;
    }

    async fn send(&mut self, cmd: &Cmd) {
        let target_device = match &cmd.data.first() {
            Some(t) => t.to_owned(),
            None => {
                log(
                    &self.msg_tx,
                    cfg::get_name(),
                    Error,
                    format!("[{NAME}] Target device is not found."),
                )
                .await;
                return;
            }
        };

        let mut msg = String::new();
        msg += &format!("r {} ", cfg::get_name());
        for t in &cmd.data[1..] {
            msg += t;
            msg += " ";
        }

        let msg = msg.trim();

        let enc_msg = utils::encrypt(&cfg::get_key(), msg).unwrap();

        publish(
            &self.msg_tx,
            self.client.as_ref().unwrap(),
            &format!("tln/{target_device}/ask"),
            false,
            &enc_msg,
        )
        .await;
    }

    async fn publish(&mut self, cmd: &Cmd) {
        publish(
            &self.msg_tx,
            self.client.as_ref().unwrap(),
            &format!("tln/{}/{}", cfg::get_name(), cmd.data[0]),
            cmd.data[1] == "true",
            &cmd.data[2],
        )
        .await;
    }

    async fn disconnect(&mut self) {
        if let Some(t) = &self.client {
            let _ = t.disconnect().await;
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
                msg::ACT_SHOW => self.show().await,
                msg::ACT_SEND => self.send(cmd).await,
                msg::ACT_REPLY => self.reply(cmd).await,
                msg::ACT_PUBLISH => self.publish(cmd).await,
                msg::ACT_DISCONNECT => self.disconnect().await,
                _ => {
                    log(
                        &self.msg_tx,
                        cfg::get_name(),
                        Error,
                        format!("[{NAME}] unknown action: {:?}.", cmd.action),
                    )
                    .await;
                }
            },
            _ => {
                log(
                    &self.msg_tx,
                    cfg::get_name(),
                    Error,
                    format!("[{NAME}] unknown msg: {msg:?}."),
                )
                .await;
            }
        }

        false
    }
}

async fn subscribe(msg_tx: &Sender<Msg>, client: &rumqttc::AsyncClient, topic: &str) {
    log(
        msg_tx,
        cfg::get_name(),
        Trace,
        format!("[{NAME}] -> subscribe: {topic}"),
    )
    .await;

    client.subscribe(topic, QoS::AtMostOnce).await.unwrap();
}

async fn publish(
    msg_tx: &Sender<Msg>,
    client: &rumqttc::AsyncClient,
    topic: &str,
    retain: bool,
    payload: &str,
) {
    log(
        msg_tx,
        cfg::get_name(),
        Trace,
        format!("[{NAME}] -> publish: {topic}, '{payload}'"),
    )
    .await;

    if client
        .publish(topic, QoS::AtLeastOnce, retain, payload)
        .await
        .is_err()
    {
        log(
            msg_tx,
            cfg::get_name(),
            Error,
            format!("[{NAME}] Failed to publish: {topic}, '{payload}'."),
        )
        .await;

        msg::cmd(
            msg_tx,
            cfg::get_name(),
            NAME.to_owned(),
            msg::ACT_DISCONNECT.to_owned(),
            vec![],
        )
        .await;
        msg::cmd(
            msg_tx,
            cfg::get_name(),
            NAME.to_owned(),
            msg::ACT_INIT.to_owned(),
            vec![],
        )
        .await;
    }
}

async fn process_event_publish(msg_tx: &Sender<Msg>, publish: &Publish) {
    if process_event_publish_onboard(msg_tx, publish).await {
        return;
    }
    if process_event_publish_ask(msg_tx, publish).await {
        return;
    }
    if process_event_publish_reply(msg_tx, publish).await {
        return;
    }
    log(
        msg_tx,
        cfg::get_name(),
        Trace,
        format!("[{NAME}] <- ({publish:?})"),
    )
    .await;
}

async fn process_event_publish_ask(msg_tx: &Sender<Msg>, publish: &Publish) -> bool {
    let topic = &publish.topic;

    let re = regex::Regex::new(r"^tln/([^/]+)/ask$").unwrap();
    if let Some(captures) = re.captures(topic) {
        if let Some(name) = captures.get(1) {
            let name = name.as_str();
            let payload = std::str::from_utf8(&publish.payload).unwrap();
            let dec_payload = utils::decrypt(&cfg::get_key(), payload).unwrap();

            let payload_vec: Vec<String> = dec_payload
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();

            log(
                msg_tx,
                cfg::get_name(),
                Trace,
                format!("[{NAME}] <- publish::ask: {name}, '{dec_payload}'"),
            )
            .await;

            if name == cfg::get_name() {
                if let Some(t) = payload_vec.first() {
                    if t != "r" {
                        log(
                            msg_tx,
                            cfg::get_name(),
                            Error,
                            format!("[{NAME}] r is missing."),
                        )
                        .await;
                        return true;
                    }
                } else {
                    log(
                        msg_tx,
                        cfg::get_name(),
                        Error,
                        format!("[{NAME}] r is missing."),
                    )
                    .await;
                    return true;
                };

                let reply = if let Some(t) = payload_vec.get(1) {
                    t.to_owned()
                } else {
                    log(
                        msg_tx,
                        cfg::get_name(),
                        Error,
                        format!("[{NAME}] reply is missing."),
                    )
                    .await;
                    return true;
                };

                if let Some(t) = payload_vec.get(2) {
                    if t != "p" {
                        log(msg_tx, reply, Error, format!("[{NAME}] p is missing.")).await;
                        return true;
                    }
                } else {
                    log(msg_tx, reply, Error, format!("[{NAME}] p is missing.")).await;
                    return true;
                };

                let plugin = if let Some(t) = payload_vec.get(3) {
                    t.to_owned()
                } else {
                    log(msg_tx, reply, Error, format!("[{NAME}] plugin is missing.")).await;
                    return true;
                };

                let action = if let Some(t) = payload_vec.get(4) {
                    t.to_owned()
                } else {
                    log(msg_tx, reply, Error, format!("[{NAME}] action is missing.")).await;
                    return true;
                };

                let mut data = vec![];
                for i in 5..=9 {
                    if let Some(t) = payload_vec.get(i) {
                        data.push(t.to_owned());
                    }
                }

                msg::cmd(msg_tx, reply, plugin, action, data).await;
            }
        }

        return true;
    }

    false
}

async fn process_event_publish_reply(msg_tx: &Sender<Msg>, publish: &Publish) -> bool {
    let topic = &publish.topic;

    let re = regex::Regex::new(r"^tln/([^/]+)/reply$").unwrap();
    if let Some(captures) = re.captures(topic) {
        if let Some(name) = captures.get(1) {
            let name = name.as_str();

            if name == cfg::get_name() {
                let payload = std::str::from_utf8(&publish.payload).unwrap();
                let dec_payload = utils::decrypt(&cfg::get_key(), payload).unwrap();

                log(
                    msg_tx,
                    cfg::get_name(),
                    Trace,
                    format!("[{NAME}] <- publish::reply: {name}, '{dec_payload}'"),
                )
                .await;

                log(msg_tx, cfg::get_name(), Info, format!("R: {dec_payload}")).await;
            }
        }

        return true;
    }

    false
}

async fn process_event_publish_onboard(msg_tx: &Sender<Msg>, publish: &Publish) -> bool {
    let topic = &publish.topic;

    let re = regex::Regex::new(r"^tln/([^/]+)/onboard$").unwrap();
    if let Some(captures) = re.captures(topic) {
        if let Some(name) = captures.get(1) {
            let name = name.as_str();
            let payload = std::str::from_utf8(&publish.payload).unwrap();
            let onboard = match payload.parse::<u64>() {
                Ok(t) => t,
                Err(e) => {
                    log(
                        msg_tx,
                        cfg::get_name(),
                        Error,
                        format!("[{NAME}] Error: <- publish::onboard: {name}. Err: {e:?}."),
                    )
                    .await;
                    return true;
                }
            };
            if onboard != 0 && onboard != 1 {
                log(
                    msg_tx,
                    cfg::get_name(),
                    Error,
                    format!(
                        "[{NAME}] Error: <- publish::onboard: {name}. Wrong onboard: '{onboard}'."
                    ),
                )
                .await;
                return true;
            }

            log(
                msg_tx,
                cfg::get_name(),
                Trace,
                format!("[{NAME}] <- publish::onboard: {name}, '{onboard}'"),
            )
            .await;

            device_update(
                msg_tx,
                DevInfo {
                    ts: utils::ts(),
                    name: name.to_owned(),
                    onboard: onboard == 1,
                },
            )
            .await;
        }

        return true;
    }

    false
}
