use clap::{Parser, Subcommand};

use crate::{windows::wins, wins::Window};

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
    Exit,
}

pub fn run(cmd: &str, win_main: &mut Box<dyn Window>) -> wins::RetKey {
    let mut ret = wins::RetKey::RKContinue;
    let args = shlex::split(&format!("cmd {cmd}")).ok_or("error: Invalid quoting").unwrap();
    let cli = match Cli::try_parse_from(args) {
        Ok(t) => t,
        Err(_) => {
            win_main.output_push(UNKNOWN_COMMAND.to_owned());
            return ret;
        }
    };
                
    match cli.command {
        Some(Commands::H) => {
            win_main.output_push("Commands:".to_owned());
            win_main.output_push("    h - Help".to_owned());
            win_main.output_push("    exit - exit".to_owned());
        },
        Some(Commands::Init) => {
            win_main.output_push("Initializing".to_owned());
        },
        Some(Commands::Reg) => {
            win_main.output_push("Registering".to_owned());
        },
        Some(Commands::Exit) => {
            win_main.output_push("Exit".to_owned());
            ret = wins::RetKey::RKLeave;
        },
        None => {
            win_main.output_push(UNKNOWN_COMMAND.to_owned());
        }
    }

    ret
}