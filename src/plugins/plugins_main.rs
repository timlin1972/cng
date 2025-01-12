use async_trait::async_trait;
use log::Level::{Error, Info};
use tokio::sync::mpsc::Sender;

use crate::msg::{cmd, log, Data, Msg};
use crate::plugins::{plugin_devices, plugin_log, plugin_mqtt};

pub const NAME: &str = "plugins";

#[async_trait]
pub trait Plugin {
    fn name(&self) -> &str;
    async fn msg(&mut self, msg: &Msg);
}

pub struct Plugins {
    plugins: Vec<Box<dyn Plugin>>,
    msg_tx: Sender<Msg>,
}

impl Plugins {
    pub fn new(msg_tx: Sender<Msg>) -> Self {
        let plugins = vec![
            Box::new(plugin_log::Plugin::new(msg_tx.clone())) as Box<dyn Plugin>,
            Box::new(plugin_devices::Plugin::new(msg_tx.clone())) as Box<dyn Plugin>,
            Box::new(plugin_mqtt::Plugin::new(msg_tx.clone())) as Box<dyn Plugin>,
        ];

        Self { plugins, msg_tx }
    }

    pub async fn init(&mut self) {
        log(&self.msg_tx, Info, format!("[{NAME}] init")).await;
        for plugin in &mut self.plugins {
            cmd(
                &self.msg_tx,
                plugin.name().to_owned(),
                "init".to_owned(),
                None,
                None,
            )
            .await;
        }
    }

    fn get_plugin_mut(&mut self, name: &str) -> Option<&mut Box<dyn Plugin>> {
        self.plugins.iter_mut().find(|p| p.name() == name)
    }

    async fn show(&mut self) {
        for plugin in &self.plugins {
            log(&self.msg_tx, Info, format!("{}", plugin.name())).await;
        }
    }

    pub async fn msg(&mut self, msg: &Msg) {
        if msg.plugin == NAME {
            match &msg.data {
                Data::Cmd(cmd) => match cmd.action.as_str() {
                    "show" => self.show().await,
                    _ => {
                        log(
                            &self.msg_tx,
                            Error,
                            format!("[{NAME}] unknown action: {:?}", cmd.action),
                        )
                        .await;
                    }
                },
                _ => {
                    log(
                        &self.msg_tx,
                        Error,
                        format!("[{NAME}] unknown msg: {msg:?}"),
                    )
                    .await;
                }
            }
        } else {
            match self.get_plugin_mut(&msg.plugin) {
                Some(t) => t.msg(msg).await,
                None => {
                    log(
                        &self.msg_tx,
                        Info,
                        format!("[{NAME}] Plugin '{}' not found", msg.plugin),
                    )
                    .await;
                }
            }
        }
    }
}
