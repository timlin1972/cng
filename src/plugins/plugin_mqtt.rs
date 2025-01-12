use async_trait::async_trait;
use log::Level::{Error, Info, Trace};
use rumqttc::{AsyncClient, Event, LastWill, MqttOptions, Outgoing, Packet, Publish, QoS};
use tokio::sync::mpsc::Sender;

use crate::msg::{self, device_update, log, Cmd, Data, DevInfo, Msg};
use crate::plugins::plugins_main;
use crate::{cfg, utils};

const NAME: &str = "mqtt";
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
        log(&self.msg_tx, Trace, format!("[{NAME}] init")).await;

        log(
            &self.msg_tx,
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
                        log(&msg_tx_clone, Trace, format!("[{NAME}] {notification:?}.")).await;
                    }
                }
            }
        });

        // subscribe
        subscribe(&self.msg_tx, &client, "tln/#").await;

        // publish onboard
        publish(
            &self.msg_tx,
            &client,
            &format!("tln/{}/onboard", cfg::get_name()),
            true,
            "1",
        )
        .await;

        log(
            &self.msg_tx,
            Trace,
            format!("[{NAME}] Connected to MQTT broker."),
        )
        .await;

        self.client = Some(client);
    }

    async fn show(&mut self) {
        log(&self.msg_tx, Info, format!("Broker: {BROKER}")).await;
        log(&self.msg_tx, Info, format!("Id: {}", cfg::get_name())).await;
    }

    async fn send(&mut self, cmd: &Cmd) {
        let target_device = match &cmd.data1 {
            Some(t) => t,
            None => {
                log(
                    &self.msg_tx,
                    Error,
                    format!("[{NAME}] Target device is not found."),
                )
                .await;
                return;
            }
        };

        let mut msg = String::new();
        if let Some(t) = &cmd.data2 {
            msg += &t;
            msg += " ";
        }

        if let Some(t) = &cmd.data3 {
            msg += &t;
            msg += " ";
        }

        if let Some(t) = &cmd.data4 {
            msg += &t;
            msg += " ";
        }

        if let Some(t) = &cmd.data5 {
            msg += &t;
            msg += " ";
        }

        let msg = msg.trim();

        publish(
            &self.msg_tx,
            &self.client.as_ref().unwrap(),
            &format!("tln/{target_device}/ask"),
            false,
            &msg,
        )
        .await;
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
                "show" => self.show().await,
                "send" => self.send(cmd).await,
                _ => {
                    log(
                        &self.msg_tx,
                        Error,
                        format!("[{NAME}] unknown action: {:?}.", cmd.action),
                    )
                    .await;
                }
            },
            _ => {
                log(
                    &self.msg_tx,
                    Error,
                    format!("[{NAME}] unknown msg: {msg:?}."),
                )
                .await;
            }
        }
    }
}

async fn subscribe(msg_tx: &Sender<Msg>, client: &rumqttc::AsyncClient, topic: &str) {
    log(msg_tx, Trace, format!("[{NAME}] -> subscribe: {topic}")).await;

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
            Error,
            format!("[{NAME}] Failed to publish: {topic}, '{payload}'."),
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
    log(msg_tx, Trace, format!("[{NAME}] <- ({publish:?})")).await;
}

async fn process_event_publish_ask(msg_tx: &Sender<Msg>, publish: &Publish) -> bool {
    let topic = &publish.topic;

    let re = regex::Regex::new(r"^tln/([^/]+)/ask$").unwrap();
    if let Some(captures) = re.captures(topic) {
        if let Some(name) = captures.get(1) {
            let name = name.as_str();
            let payload = std::str::from_utf8(&publish.payload).unwrap();
            let payload_vec: Vec<String> =
                payload.split_whitespace().map(|s| s.to_string()).collect();

            log(
                msg_tx,
                Trace,
                format!("[{NAME}] <- publish::ask: {name}, '{payload}'"),
            )
            .await;

            if name == cfg::get_name() {
                if let Some(t) = payload_vec.get(0) {
                    if t != "p" {
                        log(msg_tx, Error, format!("[{NAME}] p is not existed.")).await;
                        return true;
                    }
                } else {
                    log(msg_tx, Error, format!("[{NAME}] p is not existed.")).await;
                    return true;
                };

                let plugin = if let Some(t) = payload_vec.get(1) {
                    t.to_owned()
                } else {
                    log(msg_tx, Error, format!("[{NAME}] plugin is not existed.")).await;
                    return true;
                };

                let action = if let Some(t) = payload_vec.get(2) {
                    t.to_owned()
                } else {
                    log(msg_tx, Error, format!("[{NAME}] action is not existed.")).await;
                    return true;
                };

                let data1 = payload_vec.get(3).cloned();
                let data2 = payload_vec.get(4).cloned();
                let data3 = payload_vec.get(5).cloned();
                let data4 = payload_vec.get(6).cloned();
                let data5 = payload_vec.get(7).cloned();

                msg::cmd(msg_tx, plugin, action, data1, data2, data3, data4, data5).await;
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
