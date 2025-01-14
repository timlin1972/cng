use async_trait::async_trait;
use log::Level::{Error, Trace};
use tokio::sync::mpsc::Sender;

use crate::msg::{cmd, log, Data, Msg};
use crate::plugins::{plugin_mqtt, plugins_main};
use crate::{cfg, msg, utils};

pub const NAME: &str = "system";
const ONBOARD_POLLING: u64 = 300;

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

        let msg_tx_clone = self.msg_tx.clone();
        tokio::spawn(async move {
            loop {
                cmd(
                    &msg_tx_clone,
                    cfg::get_name(),
                    plugin_mqtt::NAME.to_owned(),
                    msg::ACT_PUBLISH.to_owned(),
                    vec!["onboard".to_owned(), "true".to_owned(), "1".to_owned()],
                )
                .await;

                let uptime = utils::uptime();
                cmd(
                    &msg_tx_clone,
                    cfg::get_name(),
                    plugin_mqtt::NAME.to_owned(),
                    msg::ACT_PUBLISH.to_owned(),
                    vec!["uptime".to_owned(), "false".to_owned(), uptime.to_string()],
                )
                .await;
                tokio::time::sleep(tokio::time::Duration::from_secs(ONBOARD_POLLING)).await;
            }
        });
    }
}

#[async_trait]
impl plugins_main::Plugin for Plugin {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    async fn msg(&mut self, msg: &Msg) -> bool {
        let mut ret = false;
        match &msg.data {
            Data::Cmd(cmd) => match cmd.action.as_str() {
                msg::ACT_INIT => self.init().await,
                msg::ACT_QUIT => {
                    ret = true;
                }
                _ => {
                    log(
                        &self.msg_tx,
                        cfg::get_name(),
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

        ret
    }
}
