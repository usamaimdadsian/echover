mod app;
mod domain;
mod library;
mod playback;
mod persistence;
mod ui;
mod window;

fn main() {
    tracing_subscriber::fmt::init();

    if let Err(error) = app::run() {
        eprintln!("failed to start Echover: {error}");
        std::process::exit(1);
    }
}
