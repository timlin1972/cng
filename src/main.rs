mod app;
mod panels;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let terminal = ratatui::init();
    let app_result = app::App::new().run(terminal);
    ratatui::restore();
    app_result
}
