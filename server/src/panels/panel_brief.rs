use async_trait::async_trait;
use clap::Parser;
use log::Level::{Error, Info};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc::Sender;

use crate::cfg;
use crate::command::{self, Cli, Commands};
use crate::msg::{self, log, Data, Msg, Reply};
use crate::panels::panels_main::{self, PanelInfo, Popup};

pub const NAME: &str = "Brief";

const POPUP_HELP: &str = "Help";
const POPUP_ALL: &str = "All";
const POPUP_EDITOR: &str = "Editor";

const NOTE_PATH: &str = "./shared/note.md";

#[derive(Debug)]
pub struct Panel {
    panel_info: PanelInfo,
    history: Vec<String>,
    history_index: usize,
    editor_filename: Option<String>,
}

impl Panel {
    pub fn new(msg_tx: Sender<Msg>) -> Self {
        let panel_info = panels_main::PanelInfo::new(
            NAME,
            vec![
                Popup {
                    name: POPUP_HELP.to_owned(),
                    x: 50,
                    y: 70,
                    output: command::get_help(),
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
                Popup {
                    name: POPUP_EDITOR.to_owned(),
                    x: 100,
                    y: 80,
                    output: vec![],
                    cursor_x: Some(0),
                    cursor_y: Some(0),
                },
            ],
            msg_tx,
        );

        Self {
            panel_info,
            history: vec![],
            history_index: 0,
            editor_filename: None,
        }
    }
}

#[async_trait]
impl panels_main::Panel for Panel {
    fn get_panel_info(&self) -> &PanelInfo {
        &self.panel_info
    }

    async fn init(&mut self) {
        log(
            &self.panel_info.msg_tx,
            Reply::Device(cfg::name()),
            Info,
            format!("[{NAME}] init"),
        )
        .await;
    }

    async fn msg(&mut self, msg: &Msg) {
        match &msg.data {
            Data::Log(log) => {
                panels_main::output_push(&mut self.panel_info.output, log.msg.clone());
            }
            _ => {
                log(
                    &self.panel_info.msg_tx,
                    Reply::Device(cfg::name()),
                    Error,
                    format!("[{NAME}] unknown msg: {msg:?}"),
                )
                .await;
            }
        }
    }

    async fn key(&mut self, key: KeyEvent) -> bool {
        async fn key_main(myself: &mut Panel, key: KeyEvent) -> bool {
            let mut ret = false;

            match key.code {
                KeyCode::Enter => {
                    myself
                        .panel_info
                        .output
                        .push(format!("> {}", myself.panel_info.input));
                    // ignore if the input is as the same as the last one
                    if myself.history.is_empty()
                        || myself.history.last().unwrap() != &myself.panel_info.input
                    {
                        myself.history.push(myself.panel_info.input.clone());
                        myself.history_index = myself.history.len();
                    }

                    ret = myself.run(&myself.panel_info.input.clone()).await;
                    myself.panel_info.input.clear();
                }
                KeyCode::Char(c) => myself.panel_info.input.push(c),
                KeyCode::Backspace => {
                    myself.panel_info.input.pop();
                }
                KeyCode::Up => {
                    if myself.history_index > 0 {
                        myself.history_index -= 1;
                        myself.panel_info.input = myself.history[myself.history_index].clone();
                    }
                }
                KeyCode::Down => {
                    if myself.history_index < myself.history.len() {
                        myself.history_index += 1;
                        if myself.history_index < myself.history.len() {
                            myself.panel_info.input = myself.history[myself.history_index].clone();
                        } else {
                            myself.panel_info.input.clear();
                        }
                    }
                }
                _ => {}
            }

            ret
        }

        async fn key_popup_editor(myself: &mut Panel, key: KeyEvent) -> bool {
            let popup_name = myself.panel_info.active_popup_name.as_ref().unwrap();

            let active_popup = myself
                .panel_info
                .popup
                .iter_mut()
                .find(|p| &p.name == popup_name)
                .unwrap();

            let mut cursor_x = active_popup.cursor_x.unwrap_or(0);
            let mut cursor_y = active_popup.cursor_y.unwrap_or(0);
            match key.code {
                KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    myself.panel_info.active_popup_name = None;
                }
                KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if let Some(filename) = &myself.editor_filename {
                        std::fs::write(filename, active_popup.output.join("\n")).unwrap();
                    }
                }
                KeyCode::Up => {
                    if cursor_y > 0 {
                        cursor_y -= 1;
                        cursor_x = cursor_x.min(active_popup.output[cursor_y].len());
                    } else {
                        cursor_x = 0;
                    }
                }
                KeyCode::Down => {
                    if cursor_y + 1 < active_popup.output.len() {
                        cursor_y += 1;
                        cursor_x = cursor_x.min(active_popup.output[cursor_y].len());
                    } else {
                        cursor_x = active_popup.output[cursor_y].len();
                    }
                }
                KeyCode::Left => {
                    if cursor_x > 0 {
                        cursor_x -= 1;
                    } else if cursor_y > 0 {
                        cursor_y -= 1;
                        cursor_x = active_popup.output[cursor_y].len();
                    }
                }
                KeyCode::Right => {
                    if cursor_x < active_popup.output[cursor_y].len() {
                        cursor_x += 1;
                    } else if cursor_y + 1 < active_popup.output.len() {
                        cursor_y += 1;
                        cursor_x = 0;
                    }
                }
                KeyCode::Enter => {
                    let current_line = active_popup.output[cursor_y].clone();
                    let (before_cursor, after_cursor) = current_line.split_at(cursor_x);
                    active_popup.output[cursor_y] = before_cursor.to_string();
                    active_popup
                        .output
                        .insert(cursor_y + 1, after_cursor.to_string());
                    cursor_y += 1;
                    cursor_x = 0;
                }
                KeyCode::Backspace => {
                    if cursor_x > 0 {
                        active_popup.output[cursor_y].remove(cursor_x - 1);
                        cursor_x -= 1;
                    } else if cursor_y > 0 {
                        let prev_len = active_popup.output[cursor_y - 1].len();
                        let line = active_popup.output.remove(cursor_y);
                        cursor_y -= 1;
                        cursor_x = prev_len;
                        active_popup.output[cursor_y].push_str(&line);
                    }
                }
                KeyCode::Delete => {
                    if cursor_x < active_popup.output[cursor_y].len() {
                        active_popup.output[cursor_y].remove(cursor_x);
                    }
                }
                KeyCode::Tab => {
                    let spaces_to_add = 4 - (cursor_x % 4);
                    active_popup.output[cursor_y].insert_str(cursor_x, &" ".repeat(spaces_to_add));
                    cursor_x += spaces_to_add;
                }
                KeyCode::Home => {
                    cursor_x = 0;
                }
                KeyCode::End => {
                    cursor_x = active_popup.output[cursor_y].len();
                }
                KeyCode::Char(c) => {
                    active_popup.output[cursor_y].insert(cursor_x, c);
                    cursor_x += 1;
                }
                _ => {}
            }

            active_popup.cursor_x = Some(cursor_x);
            active_popup.cursor_y = Some(cursor_y);

            false
        }

        async fn key_popup(myself: &mut Panel, key: KeyEvent) -> bool {
            let mut ret = false;

            match myself
                .panel_info
                .active_popup_name
                .as_ref()
                .unwrap()
                .as_str()
            {
                POPUP_HELP | POPUP_ALL => myself.panel_info.active_popup_name = None,
                POPUP_EDITOR => ret = key_popup_editor(myself, key).await,
                _ => {}
            }

            ret
        }

        match self.panel_info.active_popup_name.is_some() {
            true => key_popup(self, key).await,
            false => key_main(self, key).await,
        }
    }

    async fn run(&mut self, cmd: &str) -> bool {
        let mut ret = false;
        let args = shlex::split(&format!("cmd {cmd}"))
            .ok_or("error: Invalid quoting")
            .unwrap();
        let cli = match Cli::try_parse_from(args) {
            Ok(t) => t,
            Err(_) => {
                panels_main::output_push(
                    &mut self.panel_info.output,
                    command::UNKNOWN_COMMAND.to_owned(),
                );
                return ret;
            }
        };

        match cli.command {
            Some(Commands::H) => {
                panels_main::output_push(
                    &mut self.panel_info.output,
                    "Popup Help window".to_owned(),
                );
                self.panel_info.active_popup_name = Some(POPUP_HELP.to_owned());
            }
            Some(Commands::Q) => {
                panels_main::output_push(&mut self.panel_info.output, "Quit".to_owned());
                ret = true;
            }
            Some(Commands::E { filename }) => {
                panels_main::output_push(
                    &mut self.panel_info.output,
                    "Popup Editor window".to_owned(),
                );

                self.panel_info.active_popup_name = Some(POPUP_EDITOR.to_owned());

                let filename = filename.unwrap_or(NOTE_PATH.to_owned());

                let active_popup = self
                    .panel_info
                    .popup
                    .iter_mut()
                    .find(|p| p.name == POPUP_EDITOR)
                    .unwrap();

                active_popup.output = std::fs::read_to_string(&filename)
                    .unwrap_or_default()
                    .lines()
                    .map(String::from)
                    .collect();

                self.editor_filename = Some(filename);
            }
            Some(Commands::A) => {
                panels_main::output_push(
                    &mut self.panel_info.output,
                    "Popup All window".to_owned(),
                );
                self.panel_info.active_popup_name = Some(POPUP_ALL.to_owned());
                let active_popup = self
                    .panel_info
                    .popup
                    .iter_mut()
                    .find(|p| p.name == POPUP_ALL)
                    .unwrap();

                active_popup.output = self.panel_info.output.clone();
            }
            Some(Commands::P {
                plugin,
                action,
                data,
            }) => {
                msg::cmd(
                    &self.panel_info.msg_tx,
                    Reply::Device(cfg::name()),
                    plugin,
                    action,
                    data,
                )
                .await;
            }

            None => {
                panels_main::output_push(
                    &mut self.panel_info.output,
                    command::UNKNOWN_COMMAND.to_owned(),
                );
            }
        }

        ret
    }
}
