use log::Level::{Error, Info, Trace};
use rumqttc::{AsyncClient, Event, Outgoing, Packet, Publish, QoS};
use tokio::sync::mpsc::Sender;

use crate::msg::{self, device_update, log, DevInfo, Msg};
use crate::plugins::{plugin_mqtt, plugin_system};
use crate::{cfg, utils};

const NAME: &str = "mqtt::utils";

pub async fn subscribe(msg_tx: &Sender<Msg>, client: Option<&AsyncClient>, topic: &str) {
    if client.is_none() {
        log(
            msg_tx,
            cfg::get_name(),
            Trace,
            format!("[{NAME}] -> subscribe: {topic} failed: client disconnected."),
        )
        .await;
        return;
    }
    let client = client.unwrap();

    log(
        msg_tx,
        cfg::get_name(),
        Trace,
        format!("[{NAME}] -> subscribe: {topic}"),
    )
    .await;

    client.subscribe(topic, QoS::AtMostOnce).await.unwrap();
}

pub async fn publish(
    msg_tx: &Sender<Msg>,
    client: Option<&AsyncClient>,
    topic: &str,
    retain: bool,
    payload: &str,
) {
    if client.is_none() {
        log(
            msg_tx,
            cfg::get_name(),
            Trace,
            format!("[{NAME}] -> publish: {topic}, '{payload}' failed: client disconnected."),
        )
        .await;
        return;
    }
    let client = client.unwrap();

    log(
        msg_tx,
        cfg::get_name(),
        Trace,
        format!("[{NAME}] -> publish: {topic}, '{payload}'"),
    )
    .await;

    if let Err(e) = client
        .publish(topic, QoS::AtLeastOnce, retain, payload)
        .await
    {
        log(
            msg_tx,
            cfg::get_name(),
            Error,
            format!("[{NAME}] -> publish: {topic}, '{payload}' failed: {e}."),
        )
        .await;
    }
}

pub async fn disconnect(msg_tx: &Sender<Msg>, client: Option<&AsyncClient>) {
    if client.is_none() {
        log(
            msg_tx,
            cfg::get_name(),
            Trace,
            format!("[{NAME}] -> disconnect failed: client disconnected."),
        )
        .await;
        return;
    }
    let client = client.unwrap();

    log(
        msg_tx,
        cfg::get_name(),
        Trace,
        format!("[{NAME}] -> disconnect"),
    )
    .await;

    let _ = client.disconnect().await;

    // init
    msg::cmd(
        msg_tx,
        cfg::get_name(),
        plugin_mqtt::NAME.to_owned(),
        msg::ACT_INIT.to_owned(),
        vec![],
    )
    .await;
}

pub async fn process_event(msg_tx: &Sender<Msg>, event: Event) {
    match event {
        // ignore
        Event::Incoming(Packet::PingResp)
        | Event::Outgoing(Outgoing::PingReq)
        | Event::Outgoing(Outgoing::Publish(_))
        | Event::Outgoing(Outgoing::Subscribe(_))
        | Event::Incoming(Packet::SubAck(_))
        | Event::Incoming(Packet::PubAck(_)) => (),

        // publish
        Event::Incoming(Packet::Publish(publish)) => {
            process_event_publish(msg_tx, &publish).await;
        }

        // conn ack
        Event::Incoming(Packet::ConnAck(_)) => {
            process_event_conn_ack(msg_tx).await;
        }
        _ => {
            log(
                msg_tx,
                cfg::get_name(),
                Trace,
                format!("[{NAME}] Not process: {event:?}."),
            )
            .await;
        }
    }
}

async fn process_event_conn_ack(msg_tx: &Sender<Msg>) {
    log(
        msg_tx,
        cfg::get_name(),
        Trace,
        format!("[{NAME}] <- connAck"),
    )
    .await;

    msg::cmd(
        msg_tx,
        cfg::get_name(),
        plugin_system::NAME.to_owned(),
        msg::ACT_UPDATE.to_owned(),
        vec![],
    )
    .await;
}

async fn process_event_publish(msg_tx: &Sender<Msg>, publish: &Publish) {
    if process_event_publish_system(msg_tx, publish).await {
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

async fn process_event_publish_system(msg_tx: &Sender<Msg>, publish: &Publish) -> bool {
    let topic = &publish.topic;

    let re = regex::Regex::new(r"^tln/([^/]+)/(onboard|uptime|version)$").unwrap();
    if let Some(captures) = re.captures(topic) {
        let name = &captures[1];
        let key = &captures[2];
        let payload = std::str::from_utf8(&publish.payload).unwrap();

        let (onboard, uptime, version) = match key {
            "onboard" => {
                let onboard = match payload.parse::<u64>() {
                    Ok(t) => t,
                    Err(e) => {
                        log(
                            msg_tx,
                            cfg::get_name(),
                            Error,
                            format!("[{NAME}] Error: <- publish::onboard: {name}: {e:?}."),
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

                (Some(onboard == 1), None, None)
            }
            "uptime" => {
                let uptime = match payload.parse::<u64>() {
                    Ok(t) => t,
                    Err(e) => {
                        log(
                            msg_tx,
                            cfg::get_name(),
                            Error,
                            format!("[{NAME}] Error: <- publish::uptime: {name}: {e:?}."),
                        )
                        .await;
                        return true;
                    }
                };
                (None, Some(uptime), None)
            }
            "version" => (None, None, Some(payload.to_owned())),
            _ => {
                log(
                    msg_tx,
                    cfg::get_name(),
                    Error,
                    format!("[{NAME}] Error: <- publish: {name}. Unknown key: '{key}'."),
                )
                .await;
                return true;
            }
        };

        log(
            msg_tx,
            cfg::get_name(),
            Trace,
            format!("[{NAME}] <- publish::{key}: {name}, '{payload}'"),
        )
        .await;

        device_update(
            msg_tx,
            DevInfo {
                ts: utils::ts(),
                name: name.to_owned(),
                onboard,
                uptime,
                version,
            },
        )
        .await;

        return true;
    }

    false
}
