use log::Level::Info;
use ratatui::crossterm::event::{self, Event};
use ratatui::{DefaultTerminal, Frame};
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task,
};

use crate::{
    cfg,
    msg::{log, Msg},
    panels::panels_main,
    plugins::plugins_main,
};

pub struct App {
    panels: panels_main::Panels,
    plugins: plugins_main::Plugins,
    msg_tx: Sender<Msg>,
    msg_rx: Receiver<Msg>,
    key_rx: Receiver<Event>,
    cfg_name: String,
}

impl App {
    pub fn new() -> Self {
        let (msg_tx, msg_rx) = mpsc::channel(512);

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

        let cfg_name = cfg::name();

        Self {
            panels: panels_main::Panels::new(msg_tx.clone()),
            plugins: plugins_main::Plugins::new(msg_tx.clone()),
            msg_tx,
            msg_rx,
            key_rx,
            cfg_name: cfg_name.to_owned(),
        }
    }

    pub async fn run(
        mut self,
        mut terminal: DefaultTerminal,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log(
            &self.msg_tx,
            self.cfg_name.to_owned(),
            Info,
            format!("Welcome to {}!", self.cfg_name),
        )
        .await;

        self.panels.init().await;
        self.plugins.init().await;

        loop {
            terminal.draw(|frame| self.draw(frame))?;

            tokio::select! {
                Some(msg) = self.msg_rx.recv() => {
                    if msg.plugin == panels_main::NAME {
                        self.panels.msg(&msg).await;
                    }
                    else if self.plugins.msg(&msg).await { return Ok(()) }
                }
                Some(event) = self.key_rx.recv() => {
                    if let Event::Key(key) = event {
                        if self.panels.key(key).await { return Ok(()) }
                    }
                }
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        self.panels.draw(frame);
    }
}
