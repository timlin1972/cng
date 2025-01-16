use async_trait::async_trait;
use log::Level::{Error, Trace};
use tokio::sync::mpsc::Sender;

use crate::cfg;
use crate::msg::{self, log, Data, Msg};
use crate::panels::panels_main;
use crate::plugins::plugins_main;

pub const NAME: &str = "log";

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
        log(&self.msg_tx, cfg::name(), Trace, format!("[{NAME}] init")).await;
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
                _ => {
                    log(
                        &self.msg_tx,
                        cfg::name(),
                        Error,
                        format!("[{NAME}] unknown action: {:?}", cmd.action),
                    )
                    .await;
                }
            },
            // redirect log to panels
            Data::Log(log) => {
                self.msg_tx
                    .send(Msg {
                        ts: msg.ts,
                        plugin: panels_main::NAME.to_owned(),
                        data: Data::Log(log.clone()),
                    })
                    .await
                    .unwrap();
            }
            _ => {
                log(
                    &self.msg_tx,
                    cfg::name(),
                    Error,
                    format!("[{NAME}] unknown msg: {msg:?}"),
                )
                .await;
            }
        }

        false
    }
}
