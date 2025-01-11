use log::Level::Info;
use ratatui::crossterm::event::{self, Event};
use ratatui::{DefaultTerminal, Frame};
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task,
};

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
    msg_rx: Receiver<Msg>,
    key_rx: mpsc::Receiver<Event>,
}

impl App {
    pub fn new() -> Self {
        let (msg_tx, msg_rx) = mpsc::channel(32);

        // read key
        let (key_tx, key_rx) = mpsc::channel(32);
        tokio::spawn(async move {
            loop {
                if let Ok(event) = task::spawn_blocking(event::read).await.unwrap() {
                    if key_tx.send(event.clone()).await.is_err() {
                        break;
                    }
                }
            }
        });

        Self {
            panels: panels_main::Panels::new(),
            mqtt: mqtt::Mqtt::new(msg_tx.clone()),
            device: device::Device::new(msg_tx.clone()),
            msg_tx,
            msg_rx,
            key_rx,
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

        loop {
            terminal.draw(|frame| self.draw(frame))?;

            tokio::select! {
                Some(msg) = self.msg_rx.recv() => {
                    match msg {
                        Msg::Log(log) => self.panels.log(log.level, log.msg),
                        Msg::Devices(devices) => self.panels.devices(devices),
                        Msg::DeviceUpdate(device) => self.device.device_update(device).await,
                    }
                }
                Some(event) = self.key_rx.recv() => {
                    if let Event::Key(key) = event {
                        match self.panels.key(key).await {
                            panels_main::RetKey::RKLeave => return Ok(()),
                            panels_main::RetKey::RKContinue => (),
                        }
                    }
                }
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        self.panels.draw(frame);
    }
}
