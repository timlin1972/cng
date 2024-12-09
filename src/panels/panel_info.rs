use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::panels::panels_main;

pub const TITLE: &str = "info";

#[derive(Debug)]
pub struct Panel {
    title: String,
    input: String,
    output: Vec<String>,
}

impl Panel {
    pub fn new() -> Self {
        Self {
            title: TITLE.to_owned(),
            input: "".to_owned(),
            output: vec![],
        }
    }
}

impl panels_main::Panel for Panel {
    fn title(&self) -> &str {
        self.title.as_str()
    }

    fn input(&self) -> &str {
        self.input.as_str()
    }

    fn output(&self) -> &Vec<String> {
        &self.output
    }
    
    fn output_push(&mut self, output: String) {
        self.output.push(output);
    }

    fn key(&mut self, key: KeyEvent) -> panels_main::RetKey {
        let mut ret = panels_main::RetKey::RKContinue;

        match key.code {
            KeyCode::Char('q') => {
                ret = panels_main::RetKey::RKLeave;
            }
            _ => {}
        }

        ret
    }
}
