mod app;
mod cfg;
mod mqtt;
mod panels;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    cfg::init();

    let terminal = ratatui::init();
    let app_result = app::App::new().run(terminal);
    ratatui::restore();
    app_result
}
