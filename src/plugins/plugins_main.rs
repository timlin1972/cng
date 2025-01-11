use async_trait::async_trait;
use log::Level::Trace;
use tokio::sync::mpsc::Sender;

use crate::msg::{log, Msg};
use crate::plugins::{plugin_devices, plugin_log, plugin_mqtt};

const NAME: &str = "plugins";

#[async_trait]
pub trait Plugin {
    fn name(&self) -> &str;
    async fn init(&mut self);
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
        log(&self.msg_tx, Trace, format!("[{NAME}] init")).await;
        for plugin in &mut self.plugins {
            plugin.init().await;
        }
    }

    fn get_plugin_mut(&mut self, name: &str) -> &mut Box<dyn Plugin> {
        self.plugins
            .iter_mut()
            .find(|p| p.name() == name)
            .unwrap_or_else(|| panic!("Plugin not found: {}", name))
    }

    pub async fn msg(&mut self, msg: &Msg) {
        self.get_plugin_mut(&msg.plugin).msg(msg).await;
    }
}
