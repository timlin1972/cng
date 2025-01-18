mod app_cli;
mod app_gui;
mod cfg;
mod command;
mod msg;
mod panels;
mod plugins;
mod utils;

pub const MSG_SIZE: usize = 4096;
pub const KEY_SIZE: usize = 32;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("Panic occurred: {:?}", info);
        std::process::exit(1); // 立即退出程序
    }));

    let mode = cfg::mode();

    match mode.as_str() {
        cfg::MODE_GUI => {
            let terminal = ratatui::init();
            let _app_result = app_gui::App::new().run(terminal).await;
            ratatui::restore();
        }
        cfg::MODE_CLI => {
            let _app_result = app_cli::App::new().run().await;
        }
        _ => {
            println!("unknown mode: {}", mode);
        }
    }
    std::process::exit(1); //  workaround
                           // _app_result
}
