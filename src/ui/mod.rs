//! Screen renderers. Each module renders one [`crate::state::Screen`] and
//! returns an `*Action` enum for `app.rs` to dispatch — screens never touch
//! the network bridge directly.

pub mod connect;
pub mod room;
pub mod rooms;
pub mod settings;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ConnectAction {
    #[default]
    None,
    Login,
    /// Start SSO login with the current `form_homeserver`.
    Sso,
    /// Give up on an in-flight SSO login and go back to the form.
    CancelSso,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum RoomsAction {
    #[default]
    None,
    Open(String),
    OpenSettings,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum RoomAction {
    #[default]
    None,
    Send(String),
    Back,
    LoadOlder,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SettingsAction {
    #[default]
    None,
    Back,
    Logout,
    ToggleTheme,
}
