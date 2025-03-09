use clap::{Parser, Subcommand};
use tokio::sync::mpsc::Sender;

use crate::cfg;
use crate::msg::{self, Msg, Reply};

pub const UNKNOWN_COMMAND: &str = "Unknown command. Input 'h' for help.";
pub const ALL_TEXT: &str = "All";
pub const EDITOR_TEXT: &str = "Editor, which is not supported in CLI mode.";

#[derive(Parser, Debug)]
#[command(
    name = "Center NG",
    version = "1.0",
    author = "Tim",
    about = "Center Next Generation"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    H,
    Q,
    A,
    E {
        filename: Option<String>,
    },
    P {
        plugin: String,
        action: String,
        data: Vec<String>,
    },
}

pub fn get_help() -> Vec<String> {
    vec![
        "Commands:".to_owned(),
        "h    - Help".to_owned(),
        "q    - Quit".to_owned(),
        "p <plugin> <action> ...".to_owned(),
        "    plugin: plugins, device, log, ...".to_owned(),
        "            use 'p plugins show' to get plugin list".to_owned(),
        "    action: init, show, action".to_owned(),
        "Example:".to_owned(),
        "    p plugins show".to_owned(),
        "    p devices show".to_owned(),
        "    p devices show pi5".to_owned(),
        "    p mqtt show".to_owned(),
        "    p mqtt ask pi5 p wol wake linds".to_owned(),
        "    p mqtt ask pi5 p system quit".to_owned(),
        "    p wol wake linds".to_owned(),
        "    p ping ping www.google.com".to_owned(),
        "    p shell start".to_owned(),
        "    p shell cmd \"pwd\"".to_owned(),
        "    p shell stop".to_owned(),
    ]
}

pub async fn run(msg_tx: &Sender<Msg>, cmd: &str) -> bool {
    let mut ret = false;
    let args = shlex::split(&format!("cmd {cmd}"))
        .ok_or("error: Invalid quoting")
        .unwrap();
    let cli = match Cli::try_parse_from(args) {
        Ok(t) => t,
        Err(_) => {
            println!("{UNKNOWN_COMMAND}");
            return ret;
        }
    };

    match cli.command {
        Some(Commands::H) => {
            println!("{}", get_help().join("\n")); // cli mode
        }
        Some(Commands::A) => {
            println!("{ALL_TEXT}"); // cli mode
        }
        Some(Commands::E { filename: _ }) => {
            println!("{EDITOR_TEXT}"); // cli mode
        }
        Some(Commands::Q) => {
            println!("Exiting..."); // cli mode
            ret = true;
        }
        Some(Commands::P {
            plugin,
            action,
            data,
        }) => {
            msg::cmd(msg_tx, Reply::Device(cfg::name()), plugin, action, data).await;
        }

        None => {
            println!("{UNKNOWN_COMMAND}");
        }
    }

    ret
}
