use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::windows::wins;

#[derive(Debug)]
pub struct Window {
    title: String,
    input: String,
    output: Vec<String>,
}

impl Window {
    pub fn new(title: String) -> Self {
        Self {
            title,
            input: "".to_owned(),
            output: vec![],
        }
    }
}

impl wins::Window for Window {
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

    fn key(&mut self, key: KeyEvent) -> wins::RetKey {
        let mut ret = wins::RetKey::RKContinue;

        match key.code {
            KeyCode::Char('q') => {
                ret = wins::RetKey::RKLeave;
            }
            KeyCode::Enter => {
                self.output.push(format!("> {}", self.input));
                ret = wins::RetKey::RKCommand(self.input.clone());
                self.input.clear();
            }
            KeyCode::Char(c) => self.input.push(c),
            KeyCode::Backspace => {
                self.input.pop();
            }
            _ => {}
        }

        ret
    }
}
