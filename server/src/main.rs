use log::Level::Info;
use tokio::sync::mpsc;

mod app_cli;
mod app_gui;
mod cfg;
mod command;
mod msg;
mod panels;
mod plugins;
mod utils;
mod web;

use msg::{log, Reply};

const MSG_SIZE: usize = 4096;
pub const KEY_SIZE: usize = 32;

#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("Panic occurred: {:?}", info);
        std::process::exit(1); // 立即退出程序
    }));

    let (msg_tx, msg_rx) = mpsc::channel(MSG_SIZE);

    log(
        &msg_tx,
        Reply::Device(cfg::name()),
        Info,
        format!("Welcome to {}!", cfg::name()),
    )
    .await;

    web::web_main::run(msg_tx.clone()).await?;

    let mode = cfg::mode();

    match mode.as_str() {
        cfg::MODE_GUI => {
            let terminal = ratatui::init();
            let _app_result = app_gui::App::new(msg_tx, msg_rx).run(terminal).await;
            ratatui::restore();
        }
        cfg::MODE_CLI => {
            let _app_result = app_cli::App::new(msg_tx, msg_rx).run().await;
        }
        _ => {
            println!("unknown mode: {}", mode);
        }
    }

    std::process::exit(1); //  workaround
                           // _app_result
}
