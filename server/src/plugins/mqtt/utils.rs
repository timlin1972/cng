use log::Level::{Error, Info, Trace};
use rumqttc::{AsyncClient, Event, Outgoing, Packet, Publish, QoS};
use tokio::sync::mpsc::Sender;

use crate::msg::{self, device_update, log, DevInfo, Msg, Reply};
use crate::plugins::{plugin_file, plugin_mqtt, plugin_nas, plugin_system};
use crate::{cfg, utils};
use crate::{error, info, reply_me, trace};

const NAME: &str = "mqtt::utils";
const RESTART_DELAY: u64 = 30;

pub async fn subscribe(msg_tx: &Sender<Msg>, client: Option<&AsyncClient>, topic: &str) {
    if client.is_none() {
        trace!(
            msg_tx,
            format!("[{NAME}] -> subscribe: {topic} failed: client disconnected.")
        );
        return;
    }
    let client = client.unwrap();

    trace!(msg_tx, format!("[{NAME}] -> subscribe: {topic}"));

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
        trace!(
            msg_tx,
            format!("[{NAME}] -> pub: {topic}, '{payload}' failed: client disconnected.")
        );
        return;
    }
    let client = client.unwrap();

    trace!(msg_tx, format!("[{NAME}] -> pub: {topic}, '{payload}'"));

    if let Err(e) = client
        .publish(topic, QoS::AtMostOnce, retain, payload)
        .await
    {
        error!(
            msg_tx,
            format!("[{NAME}] -> pub: {topic}, '{payload}' failed: {e}.")
        );
    }
}

pub async fn disconnect(msg_tx: &Sender<Msg>, client: Option<&AsyncClient>) {
    if client.is_none() {
        trace!(
            msg_tx,
            format!("[{NAME}] -> disconnect failed: client disconnected.")
        );
        return;
    }
    let client = client.unwrap();

    trace!(msg_tx, format!("[{NAME}] -> disconnect"));

    let _ = client.disconnect().await;

    error!(
        msg_tx,
        format!("[{NAME}] Waiting for {RESTART_DELAY} secs to restart.")
    );

    tokio::time::sleep(tokio::time::Duration::from_secs(RESTART_DELAY)).await;

    // init
    msg::cmd(
        msg_tx,
        reply_me!(),
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
            trace!(msg_tx, format!("[{NAME}] Not process: {event:?}."));
        }
    }
}

async fn process_event_conn_ack(msg_tx: &Sender<Msg>) {
    trace!(msg_tx, format!("[{NAME}] <- connAck"));

    msg::cmd(
        msg_tx,
        reply_me!(),
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
    if process_event_publish_nas(msg_tx, publish).await {
        return;
    }
    trace!(msg_tx, format!("[{NAME}] <- ({publish:?})"));
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

            trace!(
                msg_tx,
                format!("[{NAME}] <- pub::ask: {name}, '{dec_payload}'")
            );

            if name == cfg::name() {
                if let Some(t) = payload_vec.first() {
                    if t != "r" {
                        error!(msg_tx, format!("[{NAME}] r is missing."));
                        return true;
                    }
                } else {
                    error!(msg_tx, format!("[{NAME}] r is missing."));
                    return true;
                };

                let reply = if let Some(t) = payload_vec.get(1) {
                    Reply::Device(t.to_owned())
                } else {
                    error!(msg_tx, format!("[{NAME}] reply is missing."));
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
                for i in 5..=payload_vec.len() - 1 {
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

                trace!(
                    msg_tx,
                    format!("[{NAME}] <- pub::reply: {name}, '{dec_payload}'")
                );

                info!(msg_tx, format!("R: {dec_payload}"));
            }
        }

        return true;
    }

    false
}

async fn process_event_publish_system(msg_tx: &Sender<Msg>, publish: &Publish) -> bool {
    let topic = &publish.topic;

    let re = regex::Regex::new(
        r"^tln/([^/]+)/(onboard|app_uptime|host_uptime|version|temperature|weather|tailscale_ip|os|cpu_arch|cpu_usage|memory_usage|disk_usage)$",
    )
    .unwrap();
    if let Some(captures) = re.captures(topic) {
        let name = &captures[1];
        let key = &captures[2];
        let payload = std::str::from_utf8(&publish.payload).unwrap();

        let (
            onboard,
            app_uptime,
            host_uptime,
            version,
            temperature,
            weather,
            tailscale_ip,
            os,
            cpu_arch,
            cpu_usage,
            memory_usage,
            disk_usage,
        ) = match key {
            "onboard" => {
                let onboard = match payload.parse::<u64>() {
                    Ok(t) => t,
                    Err(e) => {
                        error!(
                            msg_tx,
                            format!("[{NAME}] Error: <- pub::onboard: {name}: {e:?}.")
                        );
                        return true;
                    }
                };

                if onboard != 0 && onboard != 1 {
                    error!(
                        msg_tx,
                        format!(
                            "[{NAME}] Error: <- pub::onboard: {name}. Wrong onboard: '{onboard}'."
                        )
                    );
                    return true;
                }

                (
                    Some(onboard == 1),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            }
            "app_uptime" => {
                let app_uptime = match payload.parse::<u64>() {
                    Ok(t) => t,
                    Err(e) => {
                        error!(
                            msg_tx,
                            format!("[{NAME}] Error: <- pub::app_uptime: {name}. Wrong uptime: {payload}: {e:?}.")
                        );
                        return true;
                    }
                };
                (
                    None,
                    Some(app_uptime),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            }
            "host_uptime" => {
                let host_uptime = match payload.parse::<u64>() {
                    Ok(t) => t,
                    Err(e) => {
                        error!(
                            msg_tx,
                            format!("[{NAME}] Error: <- pub::host_uptime: {name}. Wrong uptime: {payload}: {e:?}.")
                        );
                        return true;
                    }
                };
                (
                    None,
                    None,
                    Some(host_uptime),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            }
            "version" => (
                None,
                None,
                None,
                Some(payload.to_owned()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            "temperature" => {
                let temperature = match payload.parse::<f32>() {
                    Ok(t) => t,
                    Err(e) => {
                        error!(
                            msg_tx,
                            format!("[{NAME}] Error: <- pub::temperature: {name}: Wrong temperature: {payload}: {e:?}.")
                        );
                        return true;
                    }
                };
                (
                    None,
                    None,
                    None,
                    None,
                    Some(temperature),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            }
            "weather" => (
                None,
                None,
                None,
                None,
                None,
                Some(payload.to_owned()),
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            "tailscale_ip" => (
                None,
                None,
                None,
                None,
                None,
                None,
                Some(payload.to_owned()),
                None,
                None,
                None,
                None,
                None,
            ),
            "os" => (
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(payload.to_owned()),
                None,
                None,
                None,
                None,
            ),
            "cpu_arch" => (
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(payload.to_owned()),
                None,
                None,
                None,
            ),
            "cpu_usage" => {
                let cpu_usage = match payload.parse::<f32>() {
                    Ok(t) => t,
                    Err(e) => {
                        error!(
                            msg_tx,
                            format!("[{NAME}] Error: <- pub::cpu_usage: {name}: Wrong cpu_usage: {payload}: {e:?}.")
                        );
                        return true;
                    }
                };
                (
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(cpu_usage),
                    None,
                    None,
                )
            }
            "memory_usage" => {
                let memory_usage = match payload.parse::<f32>() {
                    Ok(t) => t,
                    Err(e) => {
                        error!(
                            msg_tx,
                            format!("[{NAME}] Error: <- pub::memory_usage: {name}: Wrong memory_usage: {payload}: {e:?}.")
                        );
                        return true;
                    }
                };
                (
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(memory_usage),
                    None,
                )
            }
            "disk_usage" => {
                let disk_usage = match payload.parse::<f32>() {
                    Ok(t) => t,
                    Err(e) => {
                        error!(
                            msg_tx,
                            format!("[{NAME}] Error: <- pub::disk_usage: {name}: Wrong disk_usage: {payload}: {e:?}.")
                        );
                        return true;
                    }
                };
                (
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(disk_usage),
                )
            }
            _ => {
                error!(
                    msg_tx,
                    format!("[{NAME}] Error: <- pub: {name}. Unknown key: '{key}'.")
                );
                return true;
            }
        };

        trace!(
            msg_tx,
            format!("[{NAME}] <- pub::{key}: {name}, '{payload}'")
        );

        // update the last seen if onboard is true OR any of uptime, version, temperature, os, cpu_arch, cpu_usage, memory_usage, disk_usage, weather is not None
        let last_seen = if (onboard.is_some() && onboard.unwrap())
            || app_uptime.is_some()
            || host_uptime.is_some()
            || version.is_some()
            || temperature.is_some()
            || os.is_some()
            || cpu_arch.is_some()
            || cpu_usage.is_some()
            || memory_usage.is_some()
            || disk_usage.is_some()
            || weather.is_some()
            || tailscale_ip.is_some()
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
                app_uptime,
                host_uptime,
                version,
                temperature,
                os,
                cpu_arch,
                cpu_usage,
                memory_usage,
                disk_usage,
                weather,
                last_seen,
                tailscale_ip,
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

                trace!(
                    msg_tx,
                    format!("[{NAME}] <- pub::reply: {name}, '{dec_payload}'")
                );

                msg::cmd(
                    msg_tx,
                    reply_me!(),
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

async fn process_event_publish_nas(msg_tx: &Sender<Msg>, publish: &Publish) -> bool {
    let topic = &publish.topic;

    let re = regex::Regex::new(r"^tln/([^/]+)/nas$").unwrap();
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

                trace!(
                    msg_tx,
                    format!("[{NAME}] <- pub::reply: {name}, '{dec_payload}'")
                );

                msg::cmd(
                    msg_tx,
                    reply_me!(),
                    plugin_nas::NAME.to_owned(),
                    msg::ACT_NAS.to_owned(),
                    payload_vec,
                )
                .await;
            }
        }

        return true;
    }

    false
}
