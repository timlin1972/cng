use ratatui::crossterm::event::{self, Event};
use ratatui::{DefaultTerminal, Frame};
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task,
};

use crate::{msg::Msg, panels::panels_main, plugins::plugins_main, KEY_SIZE};

pub struct App {
    panels: panels_main::Panels,
    plugins: plugins_main::Plugins,
    msg_rx: Receiver<Msg>,
    key_rx: Receiver<Event>,
}

impl App {
    pub fn new(msg_tx: Sender<Msg>, msg_rx: Receiver<Msg>) -> Self {
        // read key
        let (key_tx, key_rx) = mpsc::channel(KEY_SIZE);
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
            panels: panels_main::Panels::new(msg_tx.clone()),
            plugins: plugins_main::Plugins::new(msg_tx.clone()),
            msg_rx,
            key_rx,
        }
    }

    pub async fn run(
        mut self,
        mut terminal: DefaultTerminal,
    ) -> Result<(), Box<dyn std::error::Error>> {
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
                        if key.kind == event::KeyEventKind::Release { continue }
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
