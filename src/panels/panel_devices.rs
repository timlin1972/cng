use async_trait::async_trait;
use log::Level::Error;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use tokio::sync::mpsc::Sender;

use crate::cfg;
use crate::msg::{log, Data, Msg};
use crate::panels::panels_main::{self, Popup};
use crate::utils;

pub const NAME: &str = "Devices";
const POPUP_HELP: &str = "Help";

#[derive(Debug)]
pub struct Panel {
    name: String,
    input: String,
    output: Vec<String>,
    popup: Vec<Popup>,
    msg_tx: Sender<Msg>,
}

impl Panel {
    pub fn new(msg_tx: Sender<Msg>) -> Self {
        Self {
            name: NAME.to_owned(),
            input: "".to_owned(),
            output: vec![],
            popup: vec![Popup {
                show: false,
                name: POPUP_HELP.to_owned(),
                x: 50,
                y: 30,
                text: "Press 'q' to quit, 'h' to toggle help".to_owned(),
            }],
            msg_tx,
        }
    }
}

#[async_trait]
impl panels_main::Panel for Panel {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn input(&self) -> &str {
        self.input.as_str()
    }

    fn output(&self) -> &Vec<String> {
        &self.output
    }

    fn output_clear(&mut self) {
        self.output.clear();
    }

    fn output_push(&mut self, output: String) {
        self.output.push(output);
    }

    async fn msg(&mut self, msg: &Msg) {
        match &msg.data {
            Data::Devices(devices) => {
                self.output_clear();
                self.output_push(format!(
                    "{:<16} {:<7} {:16} {:<11}",
                    "Name", "Onboard", "Uptime", "Last update"
                ));
                for device in devices.iter() {
                    let uptime = if let Some(t) = device.uptime {
                        utils::uptime_str(t)
                    } else {
                        "n/a".to_owned()
                    };

                    self.output_push(format!(
                        "{:<16} {:<7} {:16} {:<11}",
                        device.name,
                        if device.onboard.unwrap() { "On" } else { "Off" },
                        uptime,
                        utils::ts_str(device.ts)
                    ));
                }
            }
            _ => {
                log(
                    &self.msg_tx,
                    cfg::get_name(),
                    Error,
                    format!("[{NAME}] unknown msg: {msg:?}"),
                )
                .await;
            }
        }
    }

    async fn key(&mut self, key: KeyEvent) -> bool {
        let mut ret = false;

        let is_show = self.popup.iter().any(|p| p.show);

        match is_show {
            true => {
                for p in &mut self.popup {
                    p.show = false;
                }
            }
            false => match key.code {
                KeyCode::Char('q') => {
                    ret = true;
                }
                KeyCode::Char('h') => {
                    for p in &mut self.popup {
                        if p.name == POPUP_HELP {
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
