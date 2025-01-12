use async_trait::async_trait;
use log::Level::{Error, Trace};
use tokio::sync::mpsc::Sender;

use crate::msg::{log, Data, Msg};
use crate::panels::panels_main;
use crate::plugins::plugins_main;

const NAME: &str = "log";

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
                _ => {
                    log(
                        &self.msg_tx,
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
                    Error,
                    format!("[{NAME}] unknown msg: {msg:?}"),
                )
                .await;
            }
        }
    }
}
