use async_trait::async_trait;
use log::Level::{Error, Info, Trace};
use rumqttc::{AsyncClient, Event, LastWill, MqttOptions, Outgoing, Packet, Publish, QoS};
use tokio::sync::mpsc::Sender;

use crate::msg::{device_update, log, Data, DevInfo, Msg};
use crate::plugins::plugins_main;
use crate::{cfg, utils};

const NAME: &str = "mqtt";
const BROKER: &str = "broker.emqx.io";

#[derive(Debug)]
pub struct Plugin {
    name: String,
    msg_tx: Sender<Msg>,
}

impl Plugin {
    pub fn new(msg_tx: Sender<Msg>) -> Self {
        Self {
            name: NAME.to_owned(),
            msg_tx,
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
                    Event::Incoming(Packet::PingResp) | Event::Outgoing(Outgoing::PingReq) => (),
                    Event::Incoming(Packet::Publish(publish)) => {
                        process_event_publish(&msg_tx_clone, &publish).await;
                    }
                    _ => {
                        log(&msg_tx_clone, Trace, format!("[{NAME}] {notification:?}")).await;
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
            format!("[{NAME}] Connected to MQTT broker"),
        )
        .await;
    }

    async fn show(&mut self) {
        log(&self.msg_tx, Info, format!("Broker: {BROKER}")).await;
        log(&self.msg_tx, Info, format!("Id: {}", cfg::get_name())).await;
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
                _ => {
                    log(
                        &self.msg_tx,
                        Error,
                        format!("[{NAME}] unknown action: {:?}", cmd.action),
                    )
                    .await;
                }
            },
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

async fn subscribe(msg_tx: &Sender<Msg>, client: &rumqttc::AsyncClient, topic: &str) {
    log(msg_tx, Trace, format!("[{NAME}] Subscribe: '{topic}'")).await;

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
        format!("[{NAME}] Publish: '{topic}::{payload}'"),
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
            format!("[{NAME}] Failed to publish: '{topic}::{payload}'"),
        )
        .await;
    }
}

async fn process_event_publish(msg_tx: &Sender<Msg>, publish: &Publish) {
    if process_event_publish_onboard(msg_tx, publish).await {
        return;
    }
    log(msg_tx, Trace, format!("[{NAME}] Incoming({publish:?})")).await;
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
                        format!("[{NAME}] Error: Incoming publish::onboard({name}, {e:?}"),
                    )
                    .await;
                    return true;
                }
            };
            if onboard != 0 && onboard != 1 {
                log(
                    msg_tx,
                    Error,
                    format!("[{NAME}] Error: Incoming publish::onboard({name}, {onboard})"),
                )
                .await;
                return true;
            }

            log(
                msg_tx,
                Trace,
                format!("[{NAME}] Incoming publish::onboard({name}, {onboard})"),
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
