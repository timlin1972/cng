use async_trait::async_trait;
use clap::Parser;
use log::Level::{Error, Trace};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use tokio::sync::mpsc::Sender;

use crate::cfg;
use crate::command::{self, Cli, Commands};
use crate::msg::{self, log, Data, Msg};
use crate::panels::panels_main::{self, Popup};

pub const NAME: &str = "Brief";
const POPUP_HELP: &str = "Help";

#[derive(Debug)]
pub struct Panel {
    name: String,
    input: String,
    output: Vec<String>,
    popup: Vec<Popup>,
    msg_tx: Sender<Msg>,
    history: Vec<String>,
    history_index: usize,
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
                y: 40,
                text: command::HELP_TEXT.to_owned(),
            }],
            msg_tx,
            history: vec![],
            history_index: 0,
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
            Data::Log(log) => {
                self.output_push(log.msg.clone());
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
        let mut ret = false;

        let is_show = self.popup.iter().any(|p| p.show);

        match is_show {
            true => {
                for p in &mut self.popup {
                    p.show = false;
                }
            }
            false => match key.code {
                KeyCode::Enter => {
                    self.output.push(format!("> {}", self.input));
                    // ignore if the input is as the same as the last one
                    if self.history.is_empty() || self.history.last().unwrap() != &self.input {
                        self.history.push(self.input.clone());
                        self.history_index = self.history.len();
                    }

                    ret = self.run(&self.input.clone()).await;
                    self.input.clear();
                }
                KeyCode::Char(c) => self.input.push(c),
                KeyCode::Backspace => {
                    self.input.pop();
                }
                KeyCode::Up => {
                    if self.history_index > 0 {
                        self.history_index -= 1;
                        self.input = self.history[self.history_index].clone();
                    }
                }
                KeyCode::Down => {
                    if self.history_index < self.history.len() {
                        self.history_index += 1;
                        if self.history_index < self.history.len() {
                            self.input = self.history[self.history_index].clone();
                        } else {
                            self.input.clear();
                        }
                    }
                }
                _ => {}
            },
        }

        ret
    }

    async fn run(&mut self, cmd: &str) -> bool {
        let mut ret = false;
        let args = shlex::split(&format!("cmd {cmd}"))
            .ok_or("error: Invalid quoting")
            .unwrap();
        let cli = match Cli::try_parse_from(args) {
            Ok(t) => t,
            Err(_) => {
                self.output_push(command::UNKNOWN_COMMAND.to_owned());
                return ret;
            }
        };

        match cli.command {
            Some(Commands::H) => {
                self.output_push("Popup Help window".to_owned());
                for p in &mut self.popup {
                    if p.name == POPUP_HELP {
                        p.show = true;
                        break;
                    }
                }
            }
            Some(Commands::Q) => {
                self.output_push("Quit".to_owned());
                ret = true;
            }
            Some(Commands::P {
                plugin,
                action,
                data,
            }) => {
                msg::cmd(&self.msg_tx, cfg::name(), plugin, action, data).await;
            }

            None => {
                self.output_push(command::UNKNOWN_COMMAND.to_owned());
            }
        }

        ret
    }

    fn popup(&self) -> Option<&Popup> {
        self.popup.iter().find(|&p| p.show)
    }
}
