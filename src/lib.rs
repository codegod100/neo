//! Shared Neo application entrypoint for desktop and Android.

mod app;
mod factory;
mod matrix_bridge;
mod state;
mod ui;

use relm4::RelmApp;

pub const APP_ID: &str = "org.neo.Neo";

pub fn run() {
    env_logger::init();
    let app = RelmApp::new(APP_ID);
    app.run::<app::AppModel>(());
}

#[cfg(target_os = "android")]
mod android;
