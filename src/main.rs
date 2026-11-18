mod app;
mod keys;
mod nyaa;
mod os;
mod popup;
mod title;
mod ui;

fn main() -> std::io::Result<()> {
    ratatui::run(|terminal| app::App::default().run(terminal))
}
