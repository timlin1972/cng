use async_trait::async_trait;
use clap::{Parser, Subcommand};
use log::Level::Error;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use tokio::sync::mpsc::Sender;

use crate::msg::{self, log, Data, Msg};
use crate::panels::panels_main::{self, Popup};

pub const NAME: &str = "Brief";
const POPUP_HELP: &str = "Help";
const UNKNOWN_COMMAND: &str = "Unknown command. Input 'h' for help.";

const HELP_TEXT: &str = r#"Commands:
    h    - Help
    q    - Quit

    p <plugin> <action>
        plugin: plugins, device, log, ...
                use 'p plugins show' to get plugin list
        action: init, show, action
    Example:
        p devices show pi5
        p wol wake linds
        p mqtt send pi5 p wol wake linds
"#;

#[derive(Parser, Debug)]
#[command(
    name = "Center NG",
    version = "1.0",
    author = "Tim",
    about = "Center Next Generation"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    H,
    Q,
    P {
        plugin: String,
        action: String,
        data1: Option<String>,
        data2: Option<String>,
        data3: Option<String>,
        data4: Option<String>,
        data5: Option<String>,
    },
}

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
                y: 40,
                text: HELP_TEXT.to_owned(),
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
            Data::Log(log) => {
                self.output_push(log.msg.clone());
            }
            _ => {
                log(
                    &self.msg_tx,
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

                    ret = self.run(&self.input.clone()).await;
                    self.input.clear();
                }
                KeyCode::Char(c) => self.input.push(c),
                KeyCode::Backspace => {
                    self.input.pop();
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
                self.output_push(UNKNOWN_COMMAND.to_owned());
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
                data1,
                data2,
                data3,
                data4,
                data5,
            }) => {
                msg::cmd(
                    &self.msg_tx,
                    plugin,
                    action,
                    data1,
                    data2,
                    data3,
                    data4,
                    data5,
                )
                .await;
            }

            None => {
                self.output_push(UNKNOWN_COMMAND.to_owned());
            }
        }

        ret
    }

    fn popup(&self) -> Option<&Popup> {
        self.popup.iter().find(|&p| p.show)
    }
}
