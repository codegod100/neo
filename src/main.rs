//! neo — a matrix.org chat client shelled in Vidya, following sleek's
//! connect → chats → chat → settings flow but speaking Matrix instead of IRC.

mod app;
mod matrix_bridge;
mod state;
mod ui;

use eframe::egui::ViewportBuilder;

fn main() -> eframe::Result {
    env_logger::init();

    let viewport = ViewportBuilder::default()
        .with_title("neo")
        .with_inner_size([420.0, 800.0])
        .with_min_inner_size([320.0, 480.0]);

    let options = eframe::NativeOptions { viewport, ..Default::default() };

    eframe::run_native(
        "neo",
        options,
        Box::new(|cc| Ok(Box::new(app::NeoApp::new(&cc.egui_ctx)))),
    )
}
