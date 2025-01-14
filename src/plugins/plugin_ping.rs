use std::net::{IpAddr, ToSocketAddrs};

use async_trait::async_trait;
use log::Level::{Error, Info, Trace};
use tokio::sync::mpsc::Sender;

use crate::msg::{log, Cmd, Data, Msg};
use crate::plugins::plugins_main;
use crate::{cfg, msg};

pub const NAME: &str = "ping";

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
        log(
            &self.msg_tx,
            cfg::get_name(),
            Trace,
            format!("[{NAME}] init"),
        )
        .await;
    }

    async fn ping(&mut self, cmd: &Cmd) {
        let target = match &cmd.data.first() {
            Some(t) => t.to_owned(),
            None => {
                log(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    Error,
                    format!("[{NAME}] Destination is missing."),
                )
                .await;
                return;
            }
        };

        let ip = match resolve_to_ip(target) {
            Ok(ip) => ip,
            Err(e) => {
                log(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    Error,
                    format!("[{NAME}] Failed to resolve {target}: {e}"),
                )
                .await;
                return;
            }
        };

        let payload = [0; 8];

        let (_packet, duration) = match surge_ping::ping(ip, &payload).await {
            Ok((_packet, duration)) => (_packet, duration),
            Err(e) => {
                log(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    Info,
                    format!("[{NAME}] Failed to ping: {e}"),
                )
                .await;
                return;
            }
        };

        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("[{NAME}] Ping took {duration:.3?}"),
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
                msg::ACT_INIT => self.init().await,
                msg::ACT_PING => self.ping(cmd).await,
                _ => {
                    log(
                        &self.msg_tx,
                        cmd.reply.clone(),
                        Error,
                        format!("[{NAME}] unknown action: {:?}", cmd.action),
                    )
                    .await;
                }
            },
            _ => {
                log(
                    &self.msg_tx,
                    cfg::get_name(),
                    Error,
                    format!("[{NAME}] unknown msg: {msg:?}"),
                )
                .await;
            }
        }

        false
    }
}

fn resolve_to_ip(input: &str) -> Result<IpAddr, std::io::Error> {
    if let Ok(ip) = input.parse::<IpAddr>() {
        return Ok(ip);
    }

    let addrs = (input, 0).to_socket_addrs()?;
    addrs
        .map(|addr| addr.ip())
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No IP address found"))
}
