use async_trait::async_trait;
use log::Level::{Error, Trace};
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
                text: "'h' to toggle help".to_owned(),
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

    async fn init(&mut self) {
        log(&self.msg_tx, cfg::name(), Trace, format!("[{NAME}] init")).await;
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
                    "{:<12} {:<7} {:16} {:<10} {:<11} {:<10}",
                    "Name", "Onboard", "Uptime", "Version", "Last update", "Countdown"
                ));
                for device in devices.iter() {
                    let onboard = if let Some(t) = device.onboard {
                        if t {
                            "On"
                        } else {
                            "Off"
                        }
                    } else {
                        "n/a"
                    };

                    let uptime = if let Some(t) = device.uptime {
                        utils::uptime_str(t)
                    } else {
                        "n/a".to_owned()
                    };

                    let version = if let Some(t) = &device.version {
                        t.clone()
                    } else {
                        "n/a".to_owned()
                    };

                    // countdown
                    let remaining: i64 = 10 - ((utils::ts() + 1 - device.ts) / 60) as i64; // +1 to avoid overflow
                    let countdown = if remaining > 0 {
                        remaining.to_string()
                    } else {
                        "failed".to_owned()
                    };

                    self.output_push(format!(
                        "{:<12} {onboard:<7} {uptime:16} {version:<10} {:<11} {countdown:<10}",
                        device.name,
                        utils::ts_str(device.ts),
                    ));
                }
            }
            _ => {
                log(
                    &self.msg_tx,
                    cfg::name(),
                    Error,
                    format!("[{NAME}] unknown msg: {msg:?}"),
                )
                .await;
            }
        }
    }

    async fn key(&mut self, key: KeyEvent) -> bool {
        let is_show = self.popup.iter().any(|p| p.show);

        match is_show {
            true => {
                for p in &mut self.popup {
                    p.show = false;
                }
            }
            #[allow(clippy::single_match)]
            false => match key.code {
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

        false
    }

    fn popup(&self) -> Option<&Popup> {
        self.popup.iter().find(|&p| p.show)
    }
}
