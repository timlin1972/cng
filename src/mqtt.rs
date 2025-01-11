use rumqttc::{AsyncClient, Event, LastWill, MqttOptions, Outgoing, Packet, Publish, QoS};
use tokio::sync::mpsc::Sender;

use log::Level::{Error, Info, Trace};

use crate::cfg;
use crate::msg::{device_update, log, DevInfo, Msg};

const MODULE: &str = "mqtt";
const BROKER: &str = "broker.emqx.io";

pub struct Mqtt {
    msg_tx: Sender<Msg>,
}

impl Mqtt {
    pub fn new(msg_tx: Sender<Msg>) -> Mqtt {
        Mqtt { msg_tx }
    }

    pub async fn connect(&mut self) {
        log(
            &self.msg_tx,
            Info,
            format!("[{MODULE}] Connecting to MQTT broker"),
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
                Info,
                format!("[{MODULE}] Start to receive mqtt message."),
            )
            .await;

            while let Ok(notification) = connection.poll().await {
                match notification {
                    Event::Incoming(Packet::PingResp) | Event::Outgoing(Outgoing::PingReq) => (),
                    Event::Incoming(Packet::Publish(publish)) => {
                        process_event_publish(&msg_tx_clone, &publish).await;
                    }
                    _ => {
                        log(
                            &msg_tx_clone,
                            Trace,
                            format!("[{MODULE}] {notification:?}"),
                        )
                        .await;
                    }
                }
            }
        });

        // clear DEF_NAME
        publish(
            &self.msg_tx,
            &client,
            &format!("tln/{}/onboard", cfg::DEF_NAME),
            true,
            "",
        )
        .await;

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
            Info,
            format!("[{MODULE}] Connected to MQTT broker"),
        )
        .await;
    }
}

async fn subscribe(msg_tx: &Sender<Msg>, client: &rumqttc::AsyncClient, topic: &str) {
    log(msg_tx, Info, format!("[{MODULE}] Subscribe: '{topic}'")).await;
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
        Info,
        format!("[{MODULE}] Publish: '{topic}::{payload}'"),
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
            format!("[{MODULE}] Failed to publish: '{topic}::{payload}'"),
        )
        .await;
    }
}

async fn process_event_publish(msg_tx: &Sender<Msg>, publish: &Publish) {
    log(msg_tx, Trace, format!("[{MODULE}] Incoming({publish:?})")).await;

    process_event_publish_onboard(msg_tx, publish).await;
}

async fn process_event_publish_onboard(msg_tx: &Sender<Msg>, publish: &Publish) {
    let topic = &publish.topic;

    let re = regex::Regex::new(r"^tln/([^/]+)/onboard$").unwrap();
    if let Some(captures) = re.captures(topic) {
        if let Some(name) = captures.get(1) {
            let payload = std::str::from_utf8(&publish.payload).unwrap();
            let onboard = match payload.parse::<u64>() {
                Ok(t) => t,
                Err(_) => return,
            };
            if onboard != 0 && onboard != 1 {
                return;
            }

            device_update(
                msg_tx,
                DevInfo {
                    name: name.as_str().to_owned(),
                    onboard: onboard == 1,
                },
            )
            .await;
        }
    }
}