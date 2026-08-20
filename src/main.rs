//! neo — a matrix.org chat client built with Relm4 (GTK4 + libadwaita),
//! following sleek's connect → chats → chat → settings flow but speaking
//! Matrix instead of IRC.

mod app;
mod factory;
mod matrix_bridge;
mod state;

use relm4::RelmApp;

fn main() {
    env_logger::init();
    let app = RelmApp::new("org.neo.Neo");
    app.run::<app::AppModel>(());
}
