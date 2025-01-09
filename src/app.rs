use ratatui::{DefaultTerminal, Frame};

use crate::panels::panels_main;

pub struct App {
    panels: panels_main::Panels,
}

impl App {
    pub fn new() -> Self {
        Self {
            panels: panels_main::Panels::new(),
        }
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<(), Box<dyn std::error::Error>> {
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
}
