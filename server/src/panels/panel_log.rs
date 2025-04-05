use async_trait::async_trait;
use log::Level::{Error, Info};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use tokio::sync::mpsc::Sender;

use crate::cfg;
use crate::msg::{log, Data, Msg, Reply};
use crate::panels::panels_main::{self, PanelInfo, Popup};
use crate::utils;
use crate::{error, info, init, unknown};

pub const NAME: &str = "Log";

const POPUP_ALL: &str = "All";
const POPUP_HELP: &str = "Help";

#[derive(Debug)]
pub struct Panel {
    panel_info: PanelInfo,
}

impl Panel {
    pub fn new(msg_tx: Sender<Msg>) -> Self {
        let help_text: Vec<String> = vec![
            "Commands:".to_owned(),
            "c - Clear".to_owned(),
            "h - Help".to_owned(),
        ];

        let panel_info = PanelInfo::new(
            NAME,
            vec![
                Popup {
                    name: POPUP_HELP.to_owned(),
                    x: 50,
                    y: 30,
                    output: help_text,
                    cursor_x: None,
                    cursor_y: None,
                },
                Popup {
                    name: POPUP_ALL.to_owned(),
                    x: 100,
                    y: 80,
                    output: vec![],
                    cursor_x: None,
                    cursor_y: None,
                },
            ],
            msg_tx,
        );

        Self { panel_info }
    }
}

#[async_trait]
impl panels_main::Panel for Panel {
    fn get_panel_info(&self) -> &PanelInfo {
        &self.panel_info
    }

    async fn init(&mut self) {
        init!(&self.panel_info.msg_tx, NAME);
    }

    async fn msg(&mut self, msg: &Msg) {
        match &msg.data {
            Data::Log(log) => {
                panels_main::output_push(
                    &mut self.panel_info.output,
                    format!("{} {}", utils::ts_str(msg.ts), log.msg.clone()),
                );
            }
            _ => {
                unknown!(&self.panel_info.msg_tx, NAME, msg);
            }
        }
    }

    async fn key(&mut self, key: KeyEvent) -> bool {
        match self.panel_info.active_popup_name.is_some() {
            true => {
                self.panel_info.active_popup_name = None;
            }
            false => match key.code {
                KeyCode::Char('c') => self.panel_info.output.clear(),
                KeyCode::Char('h') => {
                    self.panel_info.active_popup_name = Some(POPUP_HELP.to_owned());
                }
                KeyCode::Char('q') => return true,
                KeyCode::Char('a') => {
                    self.panel_info.active_popup_name = Some(POPUP_ALL.to_owned());
                    let active_popup = self
                        .panel_info
                        .popup
                        .iter_mut()
                        .find(|p| p.name == POPUP_ALL)
                        .unwrap();
                    active_popup.output = self.panel_info.output.clone();
                }
                _ => {}
            },
        }

        false
    }
}
