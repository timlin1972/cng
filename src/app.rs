use log::Level::Info;
use ratatui::{DefaultTerminal, Frame};
use async_channel::{unbounded, Sender};

use crate::{
    cfg, device, mqtt,
    msg::{self, Msg},
    panels::panels_main,
};

pub struct App {
    device: device::Device,
    panels: panels_main::Panels,
    mqtt: mqtt::Mqtt,
    msg_tx: Sender<Msg>,
}

impl App {
    pub fn new() -> Self {
        let (msg_tx, msg_rx) = unbounded();

        Self {
            panels: panels_main::Panels::new(msg_rx),
            mqtt: mqtt::Mqtt::new(msg_tx.clone()),
            device: device::Device::new(msg_tx.clone()),
            msg_tx,
        }
    }

    pub async fn run(
        mut self,
        mut terminal: DefaultTerminal,
    ) -> Result<(), Box<dyn std::error::Error>> {
        msg::log(
            &self.msg_tx,
            Info,
            format!("Welcome to {}!", cfg::get_name()),
        )
        .await;

        self.mqtt.connect().await;
        self.device.test().await;

        loop {
            terminal.draw(|frame| self.draw(frame))?;

            match self.panels.key().await {
                panels_main::RetKey::RKLeave => return Ok(()),
                panels_main::RetKey::RKContinue => {}
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        self.panels.draw(frame);
    }
}
