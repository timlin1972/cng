use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::panels::panels_main::{self, Popup};

pub const TITLE: &str = "Error";

#[derive(Debug)]
pub struct Panel {
    title: String,
    input: String,
    output: Vec<String>,
    popup: Vec<Popup>,
}

impl Panel {
    pub fn new() -> Self {
        Self {
            title: TITLE.to_owned(),
            input: "".to_owned(),
            output: vec![],
            popup: vec![Popup {
                show: false,
                title: "Help".to_owned(),
                x: 50,
                y: 30,
                text: "Press 'q' to quit, 'h' to toggle help".to_owned(),
            }],
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

    fn output_push(&mut self, output: String) {
        self.output.push(output);
    }

    fn output(&self) -> &Vec<String> {
        &self.output
    }

    fn key(&mut self, key: KeyEvent) -> panels_main::RetKey {
        let mut ret = panels_main::RetKey::RKContinue;

        let is_show = self.popup.iter().any(|p| p.show);

        match is_show {
            true => {
                for p in &mut self.popup {
                    p.show = false;
                }
            }
            false => match key.code {
                KeyCode::Char('q') => {
                    ret = panels_main::RetKey::RKLeave;
                }
                KeyCode::Char('h') => {
                    for p in &mut self.popup {
                        if p.title == "Help" {
                            p.show = true;
                            break;
                        }
                    }
                }

                _ => {}
            },
        }

        ret
    }

    fn popup(&self) -> Option<&Popup> {
        self.popup.iter().find(|&p| p.show)
    }
}
