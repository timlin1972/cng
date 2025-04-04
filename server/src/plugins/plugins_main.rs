use async_trait::async_trait;
use log::Level::{Error, Info};
use tokio::sync::mpsc::Sender;

use crate::cfg;
use crate::msg::{self, cmd, log, Cmd, Data, Msg, Reply};
use crate::plugins::{
    plugin_devices, plugin_file, plugin_log, plugin_mqtt, plugin_nas, plugin_ping, plugin_shell,
    plugin_stocks, plugin_system, plugin_todos, plugin_weather, plugin_wol, plugin_worldtime,
};
use crate::{error, info, init, reply_me, unknown};

pub const NAME: &str = "plugins";

#[async_trait]
pub trait Plugin {
    fn name(&self) -> &str;
    async fn msg(&mut self, msg: &Msg) -> bool;
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
            Box::new(plugin_wol::Plugin::new(msg_tx.clone())) as Box<dyn Plugin>,
            Box::new(plugin_system::Plugin::new(msg_tx.clone())) as Box<dyn Plugin>,
            Box::new(plugin_ping::Plugin::new(msg_tx.clone())) as Box<dyn Plugin>,
            Box::new(plugin_shell::Plugin::new(msg_tx.clone())) as Box<dyn Plugin>,
            Box::new(plugin_weather::Plugin::new(msg_tx.clone())) as Box<dyn Plugin>,
            Box::new(plugin_file::Plugin::new(msg_tx.clone())) as Box<dyn Plugin>,
            Box::new(plugin_worldtime::Plugin::new(msg_tx.clone())) as Box<dyn Plugin>,
            Box::new(plugin_todos::Plugin::new(msg_tx.clone())) as Box<dyn Plugin>,
            Box::new(plugin_nas::Plugin::new(msg_tx.clone())) as Box<dyn Plugin>,
            Box::new(plugin_stocks::Plugin::new(msg_tx.clone())) as Box<dyn Plugin>,
        ];

        Self { plugins, msg_tx }
    }

    pub async fn init(&mut self) {
        init!(&self.msg_tx, NAME);

        for plugin in &mut self.plugins {
            cmd(
                &self.msg_tx,
                reply_me!(),
                plugin.name().to_owned(),
                "init".to_owned(),
                vec![],
            )
            .await;
        }
    }

    fn get_plugin_mut(&mut self, name: &str) -> Option<&mut Box<dyn Plugin>> {
        self.plugins.iter_mut().find(|p| p.name() == name)
    }

    async fn show(&mut self, cmd: &Cmd) {
        for plugin in &self.plugins {
            log(
                &self.msg_tx,
                cmd.reply.clone(),
                Info,
                plugin.name().to_string(),
            )
            .await;
        }
    }

    async fn help(&self) {
        info!(&self.msg_tx, format!("[{NAME}] help: init, show"));
    }

    pub async fn msg(&mut self, msg: &Msg) -> bool {
        let mut ret = false;
        if msg.plugin == NAME {
            match &msg.data {
                Data::Cmd(cmd) => match cmd.action.as_str() {
                    msg::ACT_HELP => self.help().await,
                    msg::ACT_SHOW => self.show(cmd).await,
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
        } else {
            match self.get_plugin_mut(&msg.plugin) {
                Some(t) => ret = t.msg(msg).await,
                None => {
                    let reply = if let Data::Cmd(cmd) = &msg.data {
                        cmd.reply.clone()
                    } else {
                        reply_me!()
                    };

                    log(
                        &self.msg_tx,
                        reply,
                        Info,
                        format!("[{NAME}] Plugin '{}' not found", msg.plugin),
                    )
                    .await;
                }
            }
        }

        ret
    }
}
