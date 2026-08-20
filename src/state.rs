//! Application state — shared between the eframe UI thread and the Matrix
//! bridge thread. Room/message identifiers are plain `String`s here (Matrix
//! `RoomId`/`EventId` values) so the UI layer never has to depend on `ruma`.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Top-level screen the shell is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Connect,
    Rooms,
    Room,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
}

impl ConnectionState {
    pub fn label(self) -> &'static str {
        match self {
            ConnectionState::Disconnected => "Disconnected",
            ConnectionState::Connecting => "Connecting…",
            ConnectionState::Connected => "Connected",
        }
    }
}

/// One row in the room list.
#[derive(Debug, Clone)]
pub struct RoomSummary {
    pub id: String,
    pub name: String,
    pub preview: String,
    pub encrypted: bool,
    pub direct: bool,
}

/// One rendered chat line.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub event_id: Option<String>,
    pub sender: String,
    pub body: String,
    pub ts_millis: i64,
    pub own: bool,
    pub pending: bool,
}

/// A toast/status banner shown briefly at the bottom of the shell.
#[derive(Debug, Clone)]
pub struct Toast {
    pub text: String,
    pub created: Instant,
}

const TOAST_DURATION: Duration = Duration::from_secs(4);

pub struct AppState {
    pub screen: Screen,
    pub dark: bool,

    // --- Connect form ---
    pub form_homeserver: String,
    pub form_username: String,
    pub form_password: String,
    pub remember_me: bool,

    // --- Session ---
    pub connection: ConnectionState,
    pub user_id: Option<String>,
    pub homeserver: Option<String>,
    pub error: Option<String>,
    /// Set while an SSO login is in flight — the URL the bridge opened (or
    /// tried to open) in the system browser, shown as a fallback link.
    pub sso_url: Option<String>,

    // --- Rooms ---
    pub rooms: Vec<RoomSummary>,
    pub room_filter: String,
    pub active_room: Option<String>,
    pub messages: HashMap<String, Vec<ChatMessage>>,
    pub oldest_token: HashMap<String, Option<String>>,
    pub loading_older: bool,

    // --- Compose ---
    pub compose: String,

    // --- Misc UI ---
    pub toasts: Vec<Toast>,
    pub last_poll: Instant,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            screen: Screen::Connect,
            dark: true,
            form_homeserver: "matrix.org".to_owned(),
            form_username: String::new(),
            form_password: String::new(),
            remember_me: true,
            connection: ConnectionState::Disconnected,
            user_id: None,
            homeserver: None,
            error: None,
            sso_url: None,
            rooms: Vec::new(),
            room_filter: String::new(),
            active_room: None,
            messages: HashMap::new(),
            oldest_token: HashMap::new(),
            loading_older: false,
            compose: String::new(),
            toasts: Vec::new(),
            last_poll: Instant::now() - Duration::from_secs(60),
        }
    }
}

impl AppState {
    pub fn toast(&mut self, text: impl Into<String>) {
        self.toasts.push(Toast { text: text.into(), created: Instant::now() });
    }

    pub fn expire_toasts(&mut self) {
        self.toasts.retain(|t| t.created.elapsed() < TOAST_DURATION);
    }

    pub fn active_room_summary(&self) -> Option<&RoomSummary> {
        let id = self.active_room.as_ref()?;
        self.rooms.iter().find(|r| &r.id == id)
    }

    pub fn filtered_rooms(&self) -> Vec<&RoomSummary> {
        let needle = self.room_filter.trim().to_lowercase();
        self.rooms
            .iter()
            .filter(|r| needle.is_empty() || r.name.to_lowercase().contains(&needle))
            .collect()
    }

    pub fn reset_session(&mut self) {
        self.connection = ConnectionState::Disconnected;
        self.user_id = None;
        self.homeserver = None;
        self.sso_url = None;
        self.rooms.clear();
        self.active_room = None;
        self.messages.clear();
        self.oldest_token.clear();
        self.screen = Screen::Connect;
    }
}
