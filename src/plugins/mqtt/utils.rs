use log::Level::{Error, Info, Trace};
use rumqttc::{AsyncClient, Event, Outgoing, Packet, Publish, QoS};
use tokio::sync::mpsc::Sender;

use crate::msg::{self, device_update, log, DevInfo, Msg};
use crate::plugins::{plugin_file, plugin_mqtt, plugin_system};
use crate::{cfg, utils};

const NAME: &str = "mqtt::utils";
const RESTART_DELAY: u64 = 30;

pub async fn subscribe(msg_tx: &Sender<Msg>, client: Option<&AsyncClient>, topic: &str) {
    if client.is_none() {
        log(
            msg_tx,
            cfg::name(),
            Trace,
            format!("[{NAME}] -> subscribe: {topic} failed: client disconnected."),
        )
        .await;
        return;
    }
    let client = client.unwrap();

    log(
        msg_tx,
        cfg::name(),
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
            cfg::name(),
            Trace,
            format!("[{NAME}] -> publish: {topic}, '{payload}' failed: client disconnected."),
        )
        .await;
        return;
    }
    let client = client.unwrap();

    log(
        msg_tx,
        cfg::name(),
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
            cfg::name(),
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
            cfg::name(),
            Trace,
            format!("[{NAME}] -> disconnect failed: client disconnected."),
        )
        .await;
        return;
    }
    let client = client.unwrap();

    log(
        msg_tx,
        cfg::name(),
        Trace,
        format!("[{NAME}] -> disconnect"),
    )
    .await;

    let _ = client.disconnect().await;

    log(
        msg_tx,
        cfg::name(),
        Error,
        format!("[{NAME}] Waiting for {RESTART_DELAY} secs to restart."),
    )
    .await;

    tokio::time::sleep(tokio::time::Duration::from_secs(RESTART_DELAY)).await;

    // init
    msg::cmd(
        msg_tx,
        cfg::name(),
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
                cfg::name(),
                Trace,
                format!("[{NAME}] Not process: {event:?}."),
            )
            .await;
        }
    }
}

async fn process_event_conn_ack(msg_tx: &Sender<Msg>) {
    log(msg_tx, cfg::name(), Trace, format!("[{NAME}] <- connAck")).await;

    msg::cmd(
        msg_tx,
        cfg::name(),
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
    if process_event_publish_file(msg_tx, publish).await {
        return;
    }
    log(
        msg_tx,
        cfg::name(),
        Trace,
        format!("[{NAME}] <- ({publish:?})"),
    )
    .await;
}

fn parse(input: &str) -> Vec<String> {
    let re = regex::Regex::new(r#""([^"]+)"|(\S+)"#).unwrap();
    re.captures_iter(input)
        .map(|cap| {
            if let Some(m) = cap.get(1) {
                m.as_str().to_string() // 捕獲引號內的字串
            } else {
                cap.get(2).unwrap().as_str().to_string() // 捕獲無引號的字串
            }
        })
        .collect()
}

async fn process_event_publish_ask(msg_tx: &Sender<Msg>, publish: &Publish) -> bool {
    let topic = &publish.topic;

    let re = regex::Regex::new(r"^tln/([^/]+)/ask$").unwrap();
    if let Some(captures) = re.captures(topic) {
        if let Some(name) = captures.get(1) {
            let name = name.as_str();
            let payload = std::str::from_utf8(&publish.payload).unwrap();
            let dec_payload = utils::decrypt(&cfg::key(), payload).unwrap();

            let payload_vec: Vec<String> = parse(&dec_payload);

            log(
                msg_tx,
                cfg::name(),
                Trace,
                format!("[{NAME}] <- publish::ask: {name}, '{dec_payload}'"),
            )
            .await;

            if name == cfg::name() {
                if let Some(t) = payload_vec.first() {
                    if t != "r" {
                        log(
                            msg_tx,
                            cfg::name(),
                            Error,
                            format!("[{NAME}] r is missing."),
                        )
                        .await;
                        return true;
                    }
                } else {
                    log(
                        msg_tx,
                        cfg::name(),
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
                        cfg::name(),
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

            if name == cfg::name() {
                let payload = std::str::from_utf8(&publish.payload).unwrap();
                let dec_payload = utils::decrypt(&cfg::key(), payload).unwrap();

                log(
                    msg_tx,
                    cfg::name(),
                    Trace,
                    format!("[{NAME}] <- publish::reply: {name}, '{dec_payload}'"),
                )
                .await;

                log(msg_tx, cfg::name(), Info, format!("R: {dec_payload}")).await;
            }
        }

        return true;
    }

    false
}

async fn process_event_publish_system(msg_tx: &Sender<Msg>, publish: &Publish) -> bool {
    let topic = &publish.topic;

    let re =
        regex::Regex::new(r"^tln/([^/]+)/(onboard|uptime|version|temperature|weather)$").unwrap();
    if let Some(captures) = re.captures(topic) {
        let name = &captures[1];
        let key = &captures[2];
        let payload = std::str::from_utf8(&publish.payload).unwrap();

        let (onboard, uptime, version, temperature, weather) = match key {
            "onboard" => {
                let onboard = match payload.parse::<u64>() {
                    Ok(t) => t,
                    Err(e) => {
                        log(
                            msg_tx,
                            cfg::name(),
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
                        cfg::name(),
                        Error,
                        format!(
                            "[{NAME}] Error: <- publish::onboard: {name}. Wrong onboard: '{onboard}'."
                        ),
                    )
                    .await;
                    return true;
                }

                (Some(onboard == 1), None, None, None, None)
            }
            "uptime" => {
                let uptime = match payload.parse::<u64>() {
                    Ok(t) => t,
                    Err(e) => {
                        log(
                            msg_tx,
                            cfg::name(),
                            Error,
                            format!("[{NAME}] Error: <- publish::uptime: {name}. Wrong uptime: {payload}: {e:?}."),
                        )
                        .await;
                        return true;
                    }
                };
                (None, Some(uptime), None, None, None)
            }
            "version" => (None, None, Some(payload.to_owned()), None, None),
            "temperature" => {
                let temperature = match payload.parse::<f32>() {
                    Ok(t) => t,
                    Err(e) => {
                        log(
                            msg_tx,
                            cfg::name(),
                            Error,
                            format!("[{NAME}] Error: <- publish::temperature: {name}: Wrong temperature: {payload}: {e:?}."),
                        )
                        .await;
                        return true;
                    }
                };
                (None, None, None, Some(temperature), None)
            }
            "weather" => (None, None, None, None, Some(payload.to_owned())),
            _ => {
                log(
                    msg_tx,
                    cfg::name(),
                    Error,
                    format!("[{NAME}] Error: <- publish: {name}. Unknown key: '{key}'."),
                )
                .await;
                return true;
            }
        };

        log(
            msg_tx,
            cfg::name(),
            Trace,
            format!("[{NAME}] <- publish::{key}: {name}, '{payload}'"),
        )
        .await;

        // update the last seen if onboard is true OR any of uptime, version, temperature, weather is not None
        let last_seen = if (onboard.is_some() && onboard.unwrap())
            || uptime.is_some()
            || version.is_some()
            || temperature.is_some()
            || weather.is_some()
        {
            Some(utils::ts())
        } else {
            None
        };

        device_update(
            msg_tx,
            DevInfo {
                ts: utils::ts(),
                name: name.to_owned(),
                onboard,
                uptime,
                version,
                temperature,
                weather,
                last_seen,
            },
        )
        .await;

        return true;
    }

    false
}

async fn process_event_publish_file(msg_tx: &Sender<Msg>, publish: &Publish) -> bool {
    let topic = &publish.topic;

    let re = regex::Regex::new(r"^tln/([^/]+)/file$").unwrap();
    if let Some(captures) = re.captures(topic) {
        if let Some(name) = captures.get(1) {
            let name = name.as_str();

            if name == cfg::name() {
                let payload = std::str::from_utf8(&publish.payload).unwrap();
                let dec_payload = utils::decrypt(&cfg::key(), payload).unwrap();

                let payload_vec: Vec<String> = dec_payload
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();

                log(
                    msg_tx,
                    cfg::name(),
                    Trace,
                    format!("[{NAME}] <- publish::reply: {name}, '{dec_payload}'"),
                )
                .await;

                msg::cmd(
                    msg_tx,
                    cfg::name(),
                    plugin_file::NAME.to_owned(),
                    msg::ACT_FILE.to_owned(),
                    payload_vec,
                )
                .await;
            }
        }

        return true;
    }

    false
}
