use clap::{Parser, Subcommand};
use tokio::sync::mpsc::Sender;

use crate::cfg;
use crate::msg::{self, Msg, Reply};

pub const UNKNOWN_COMMAND: &str = "Unknown command. Input 'h' for help.";
pub const HELP_TEXT: &str = r#"Commands:
    h    - Help
    q    - Quit

    p <plugin> <action> ...
        plugin: plugins, device, log, ...
                use 'p plugins show' to get plugin list
        action: init, show, action
    Example:
        p plugins show
        p devices show
        p devices show pi5
        p mqtt show
        p mqtt ask pi5 p wol wake linds
        p mqtt ask pi5 p system quit
        p wol wake linds
        p ping ping www.google.com
        p shell start
        p shell cmd "pwd"
        p shell stop
"#;
pub const ALL_TEXT: &str = "All";
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
    P {
        plugin: String,
        action: String,
        data: Vec<String>,
    },
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
            println!("{HELP_TEXT}");
        }
        Some(Commands::A) => {
            println!("{ALL_TEXT}");
        }
        Some(Commands::Q) => {
            println!("Exiting...");
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
