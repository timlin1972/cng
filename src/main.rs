mod app;
mod cfg;
mod log_to_file;
mod msg;
mod panels;
mod plugins;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    cfg::init();

    let terminal = ratatui::init();
    let _app_result = app::App::new().run(terminal).await;
    ratatui::restore();
    std::process::exit(1); //  workaround
                           // _app_result
}
