use async_trait::async_trait;
use log::Level::{Error, Info, Trace};
use tokio::sync::mpsc::Sender;

use crate::cfg;
use crate::msg::{self, log, Cmd, Data, Msg, Reply};
use crate::panels::panels_main;
use crate::plugins::plugins_main;
use crate::{error, info, init, unknown};

pub const NAME: &str = "log";

#[derive(Debug)]
pub struct Plugin {
    name: String,
    msg_tx: Sender<Msg>,
    trace: u8,
}

impl Plugin {
    pub fn new(msg_tx: Sender<Msg>) -> Self {
        Self {
            name: NAME.to_owned(),
            msg_tx,
            trace: cfg::trace(),
        }
    }

    async fn init(&mut self) {
        init!(&self.msg_tx, NAME);
    }

    async fn show(&mut self, cmd: &Cmd) {
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("Trace: {}", self.trace),
        )
        .await;
    }

    async fn trace(&mut self, cmd: &Cmd) {
        match cmd.data.first() {
            Some(trace) => match trace.parse::<u8>() {
                Ok(trace) => {
                    self.trace = trace;
                }
                Err(_) => {
                    log(
                        &self.msg_tx,
                        cmd.reply.clone(),
                        Error,
                        format!("[{NAME}] invalid trace: {:?}", trace),
                    )
                    .await;
                }
            },
            None => {
                log(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    Error,
                    format!("[{NAME}] missing trace"),
                )
                .await;
            }
        }
    }

    async fn help(&self) {
        info!(
            &self.msg_tx,
            format!(
                "[{NAME}] {ACT_HELP}, {ACT_INIT}, {ACT_SHOW}, {ACT_TRACE} [0-1]",
                ACT_HELP = msg::ACT_HELP,
                ACT_INIT = msg::ACT_INIT,
                ACT_SHOW = msg::ACT_SHOW,
                ACT_TRACE = msg::ACT_TRACE,
            )
        );
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
                msg::ACT_HELP => self.help().await,
                msg::ACT_INIT => self.init().await,
                msg::ACT_SHOW => self.show(cmd).await,
                msg::ACT_TRACE => self.trace(cmd).await,
                _ => {
                    unknown!(&self.msg_tx, NAME, cmd.action);
                }
            },
            // redirect log to panels
            Data::Log(log) => match cfg::mode().as_str() {
                cfg::MODE_CLI => {
                    if self.trace == 0 && log.level == Trace {
                        return false;
                    }
                    println!("[{}] {}", log.level, log.msg);
                }
                cfg::MODE_GUI => {
                    if self.trace == 0 && log.level == Trace {
                        return false;
                    }
                    self.msg_tx
                        .send(Msg {
                            ts: msg.ts,
                            plugin: panels_main::NAME.to_owned(),
                            data: Data::Log(log.clone()),
                        })
                        .await
                        .unwrap();
                }
                _ => (),
            },
            _ => {
                unknown!(&self.msg_tx, NAME, msg);
            }
        }

        false
    }
}
