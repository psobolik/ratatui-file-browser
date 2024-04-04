mod app;
mod constants;
mod options;
mod stateful_list;
mod tui;
mod util;

use crate::options::Options;
use app::App;
use clap::Parser;
use color_eyre::eyre::Result;
use tui::Event;

async fn run() -> Result<()> {
    let mut tui = tui::Tui::new()?.tick_rate(1.0).frame_rate(30.0).mouse(true);
    tui.enter()?;
    let mut app = App::default();
    app.set_event_tx(Some(tui.event_tx.clone()));

    loop {
        let event = tui.next().await?; // blocks until next event

        if let Event::Render = event.clone() {
            tui.draw(|f| {
                app.render(f);
            })?;
        }
        app.handle_event(event).await;
        if app.should_quit {
            break;
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let options = Options::parse();
    if let Some(init_path) = options.init_path {
        if let Err(error) = std::env::set_current_dir(init_path) {
            eprintln!("Error: {}", error);
            std::process::exit(1);
        }
    }
    run().await
}
