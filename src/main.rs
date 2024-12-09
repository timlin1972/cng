use color_eyre::Result;
use ratatui::{DefaultTerminal, Frame};

use crate::windows::wins;
mod command;

mod windows {
    pub mod win_error;
    pub mod win_info;
    pub mod win_main;
    pub mod wins;
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let app_result = App::new().run(terminal);
    ratatui::restore();
    app_result
}

struct App {
    windows: wins::Windows,
}

impl App {
    fn new() -> Self {
        Self {
            windows: wins::Windows::new(),
        }
    }

    fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;

            match self.windows.key() {
                wins::RetKey::RKLeave => return Ok(()),
                wins::RetKey::RKContinue => {},
                wins::RetKey::RKCommand(cmd) => {
                    let win_main = self.windows.windows.get_mut(0).unwrap();
                    if command::run(&cmd, win_main) == wins::RetKey::RKLeave {
                        return Ok(());
                    }
                }
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        self.windows.draw(frame);
    }
}




