use async_trait::async_trait;
use log::Level::{Error, Info};
use tokio::sync::mpsc::Sender;

use crate::cfg;
use crate::msg::{self, log, Cmd, Data, Msg, Reply};
use crate::plugins::plugins_main;
use crate::{error, info, init, unknown};

const NAME: &str = "wol";
const LIN_DS_MAC: [u8; 6] = [0x90, 0x09, 0xd0, 0x64, 0x4e, 0xa4];

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
        init!(&self.msg_tx, NAME);
    }

    async fn show(&mut self) {
        info!(&self.msg_tx, "linds".to_owned());

        info!(
            &self.msg_tx,
            format!(
                "  mac: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                LIN_DS_MAC[0],
                LIN_DS_MAC[1],
                LIN_DS_MAC[2],
                LIN_DS_MAC[3],
                LIN_DS_MAC[4],
                LIN_DS_MAC[5],
            )
        );
    }

    async fn wake(&mut self, cmd: &Cmd) {
        let mac = match &cmd.data.first() {
            Some(t) => match t.as_str() {
                "linds" => LIN_DS_MAC,
                _ => {
                    log(
                        &self.msg_tx,
                        cmd.reply.clone(),
                        Error,
                        format!("[{NAME}] Device '{t}' not found."),
                    )
                    .await;
                    return;
                }
            },
            None => {
                log(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    Error,
                    format!("[{NAME}] Device is missing."),
                )
                .await;
                return;
            }
        };

        match wol::send_wol(wol::MacAddr(mac), None, None) {
            Ok(_) => {
                log(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    Info,
                    format!("[{NAME}] Send wol ok."),
                )
                .await;
            }
            Err(e) => {
                log(
                    &self.msg_tx,
                    cmd.reply.clone(),
                    Error,
                    format!("[{NAME}] Failed to send wol. Err: {e:?}"),
                )
                .await;
            }
        }
    }

    async fn help(&self) {
        info!(
            &self.msg_tx,
            format!(
                "[{NAME}] {ACT_HELP}, {ACT_INIT}, {ACT_SHOW}, {ACT_WAKE} [linds]",
                NAME = NAME,
                ACT_HELP = msg::ACT_HELP,
                ACT_INIT = msg::ACT_INIT,
                ACT_SHOW = msg::ACT_SHOW,
                ACT_WAKE = msg::ACT_WAKE,
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
                msg::ACT_SHOW => self.show().await,
                msg::ACT_WAKE => self.wake(cmd).await,
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
                unknown!(&self.msg_tx, NAME, msg);
            }
        }

        false
    }
}
