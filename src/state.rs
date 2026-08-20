//! Plain domain data shared between the Matrix bridge thread and the
//! Relm4/GTK UI. Room/message identifiers are plain `String`s here (Matrix
//! `RoomId`/`EventId` values) so `matrix_bridge` doesn't have to depend on
//! any GTK types, and the UI doesn't have to depend on `ruma`.

/// Top-level screen the shell is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Connect,
    Rooms,
    Room,
    Settings,
    Lobby,
}

impl Screen {
    /// Name used as the `gtk::Stack` page id.
    pub fn name(self) -> &'static str {
        match self {
            Screen::Connect => "connect",
            Screen::Rooms => "rooms",
            Screen::Room => "room",
            Screen::Settings => "settings",
            Screen::Lobby => "lobby",
        }
    }
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
    /// True for Matrix Spaces (`m.room.create` with `type: "m.space"`) —
    /// these group other rooms rather than carrying messages themselves.
    pub is_space: bool,
    /// Raw bytes of the room's avatar thumbnail, if it has one set. Only
    /// fetched for spaces today (see `matrix_bridge::room_summary`), so the
    /// space filter chips can show an image instead of just initials.
    pub avatar: Option<Vec<u8>>,
}

/// One room in a space's Lobby directory — every room the space advertises
/// via `m.space.child`, whether or not the user has joined it yet. Fetched
/// from the homeserver's `/hierarchy` endpoint (unlike the joined-only
/// channel list, which only reflects locally-cached state), so unjoined
/// rooms are discoverable and joinable here.
#[derive(Debug, Clone)]
pub struct LobbyRoom {
    pub id: String,
    pub name: String,
    pub joined: bool,
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
