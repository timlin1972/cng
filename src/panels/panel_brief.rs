use ratatui::crossterm::event::{KeyCode, KeyEvent};
use clap::{Parser, Subcommand};

use crate::panels::panels_main;

pub const TITLE: &str = "brief";
const UNKNOWN_COMMAND: &str = "Unknown command. Input 'h' for help.";

#[derive(Parser, Debug)]
#[command(name = "Center NG", version = "1.0", author = "Tim", about = "Center Next Generation")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    H,
    Init,
    Reg,
    Quit,
}

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
            KeyCode::Enter => {
                self.output.push(format!("> {}", self.input));

                ret = self.run(&self.input.clone());
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
    
    fn run(&mut self, cmd: &str) -> panels_main::RetKey {
        let mut ret = panels_main::RetKey::RKContinue;
        let args = shlex::split(&format!("cmd {cmd}")).ok_or("error: Invalid quoting").unwrap();
        let cli = match Cli::try_parse_from(args) {
            Ok(t) => t,
            Err(_) => {
                self.output_push(UNKNOWN_COMMAND.to_owned());
                return ret;
            }
        };
                    
        match cli.command {
            Some(Commands::H) => {
                self.output_push("Commands:".to_owned());
                self.output_push("    h - Help".to_owned());
                self.output_push("    quit - quit".to_owned());
            },
            Some(Commands::Init) => {
                self.output_push("Initializing".to_owned());
            },
            Some(Commands::Reg) => {
                self.output_push("Registering".to_owned());
            },
            Some(Commands::Quit) => {
                self.output_push("Quit".to_owned());
                ret = panels_main::RetKey::RKLeave;
            },
            None => {
                self.output_push(UNKNOWN_COMMAND.to_owned());
            }
        }

        ret
    }
}
