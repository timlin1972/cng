use async_trait::async_trait;
use log::Level::{Error, Trace};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use tokio::sync::mpsc::Sender;

use crate::msg::{log, Data, DevInfo, Msg};
use crate::panels::panels_main::{self, Popup};
use crate::utils;
use crate::{cfg, msg};

pub const NAME: &str = "Devices";
const POPUP_HELP: &str = "Help";
const DEVICES_POLLING: u64 = 60;

#[derive(Debug)]
pub struct Panel {
    name: String,
    input: String,
    output: Vec<String>,
    popup: Vec<Popup>,
    msg_tx: Sender<Msg>,
    devices: Vec<DevInfo>,
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
            devices: vec![],
        }
    }

    pub fn devices_refresh(&mut self) {
        self.output.clear();
        self.output.push(format!(
            "{:<12} {:<7} {:13} {:<10} {:<7} {:<11} {:<10}",
            "Name", "Onboard", "Uptime", "Version", "Temp", "Last update", "Countdown"
        ));
        for device in self.devices.iter() {
            // onboard
            let onboard = if let Some(t) = device.onboard {
                if t {
                    "On"
                } else {
                    "Off"
                }
            } else {
                "n/a"
            };

            // uptime
            let uptime = if let Some(t) = device.uptime {
                utils::uptime_str(t)
            } else {
                "n/a".to_owned()
            };

            // version
            let version = if let Some(t) = &device.version {
                t.clone()
            } else {
                "n/a".to_owned()
            };

            // temperature
            let temperature = if let Some(t) = device.temperature {
                format!("{:.1}°C", t)
            } else {
                "n/a".to_owned()
            };

            // countdown
            let passing = utils::ts() - device.ts;
            let passing_mins = if passing != 0 { passing / 60 } else { 0 };
            let countdown = if passing_mins > 10 {
                "failed".to_owned()
            } else {
                format!("{}", 10 - passing_mins)
            };

            self.output.push(format!(
                "{:<12} {onboard:<7} {uptime:13} {version:<10} {temperature:<7} {:<11} {countdown:<10}",
                device.name,
                utils::ts_str(device.ts),
            ));
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

        let msg_tx_clone = self.msg_tx.clone();
        tokio::spawn(async move {
            loop {
                msg::device_countdown(&msg_tx_clone).await;
                tokio::time::sleep(tokio::time::Duration::from_secs(DEVICES_POLLING)).await;
            }
        });
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
                self.devices = devices.clone();
                self.devices_refresh();
            }
            Data::DeviceCountdown => {
                self.devices_refresh();
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
