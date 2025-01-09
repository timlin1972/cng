use ratatui::{DefaultTerminal, Frame};

use crate::{cfg, mqtt, panels::panels_main};

pub struct App {
    panels: panels_main::Panels,
    mqtt: mqtt::Mqtt,
}

impl App {
    pub fn new() -> Self {
        let app = App {
            panels: panels_main::Panels::new(),
            mqtt: mqtt::Mqtt::new(),
        };

        app
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<(), Box<dyn std::error::Error>> {
        self.log(
            log::Level::Info,
            &format!("Welcome to {}!", cfg::get_name()),
        );

        loop {
            terminal.draw(|frame| self.draw(frame))?;

            match self.panels.key() {
                panels_main::RetKey::RKLeave => return Ok(()),
                panels_main::RetKey::RKContinue => {}
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        self.panels.draw(frame);
    }

    fn log(&mut self, level: log::Level, msg: &str) {
        self.panels.log(level, msg);
    }
}
