//! neo's single Relm4 component: owns every screen's state, binds it to the
//! Blueprint view tree in [`crate::ui`], and bridges
//! [`crate::matrix_bridge`]'s `mpsc` channel into Relm4's message loop.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;
use relm4::abstractions::Toaster;
use relm4::factory::FactoryVecDeque;
use relm4::prelude::*;

use crate::factory::lobby_row::{LobbyRoomRow, LobbyRoomRowOutput};
use crate::factory::message_row::{format_day_label, local_day, MessageRow, TimelineRow};
use crate::factory::room_row::{RoomRow, RoomRowOutput};
use crate::factory::space_chip::{SpaceChip, SpaceChipInput, SpaceChipOutput};
use crate::matrix_bridge::{self, MatrixCmd, MatrixEvent};
use crate::state::{ChatMessage, ConnectionState, LobbyRoom, RoomSummary, Screen};
use crate::ui::AppUi;

pub struct AppModel {
    cmd_tx: tokio::sync::mpsc::UnboundedSender<MatrixCmd>,

    screen: Screen,
    dark: bool,

    // --- Connect form ---
    homeserver_buf: gtk::EntryBuffer,
    username_buf: gtk::EntryBuffer,
    password_buf: gtk::EntryBuffer,
    remember_me: bool,

    // --- Session ---
    connection: ConnectionState,
    user_id: Option<String>,
    homeserver: Option<String>,
    error: Option<String>,
    sso_url: Option<String>,
    /// Which of the two sign-in buttons is the one currently connecting —
    /// otherwise both would show "Signing in…" for whichever one fired.
    sso_pending: bool,

    // --- Rooms ---
    rooms: Vec<RoomSummary>,
    room_filter_buf: gtk::EntryBuffer,
    rooms_empty_hint: Option<String>,
    /// True from a successful login until the first room list lands, so the
    /// rooms screen (which now appears right after authentication succeeds,
    /// before that first sync finishes) shows "Loading rooms…" instead of
    /// the misleading "No rooms yet" empty state.
    loading_rooms: bool,
    active_room: Option<String>,
    messages: HashMap<String, Vec<ChatMessage>>,
    /// True while a `LoadOlder` pagination request is in flight, so the
    /// scroll-position handler below doesn't fire another one on every
    /// intermediate scroll event while we're already waiting on one.
    loading_older: bool,
    /// True from opening a room until its first timeline fetch lands, so the
    /// room screen can show a spinner instead of a blank scroller — replaces
    /// the (already-cached) messages for a room you've visited before, no
    /// spinner needed then.
    loading_messages: bool,
    /// The message scroller's vertical adjustment, stashed here (set once,
    /// right after `init` builds the widgets) so `apply_timeline` can read
    /// its current position before a "load older" rebuild — see
    /// `restore_scroll_anchor`.
    message_vadj: Option<gtk::Adjustment>,
    /// Set just before a "load older" rebuild to `Some((old_upper,
    /// old_value))`; the scroller's `connect_changed` handler (wired up in
    /// `init`) consumes it to shift `value` by exactly how much taller the
    /// content got, so prepending history keeps the same messages in view
    /// instead of snapping the scroller to the bottom or to a stale offset.
    restore_scroll_anchor: Rc<Cell<Option<(f64, f64)>>>,

    // --- Spaces ---
    space_children: HashMap<String, Vec<String>>,
    active_space: Option<String>,
    has_spaces: bool,
    /// Identity (id, label, avatar) of what's currently rendered in
    /// `space_factory`, in the same order — diffed against on every sync so
    /// an update that doesn't actually add/remove/rename a space just
    /// toggles `selected` on the existing chips in place, rather than
    /// tearing the whole list down and rebuilding it. Rebuilding would
    /// destroy the GTK button you just clicked and recreate a new one,
    /// dropping keyboard focus off it.
    space_chip_shape: Vec<(Option<String>, String, Option<Vec<u8>>)>,

    // --- Lobby (a space's full room directory, joined + not) ---
    lobby_space: Option<String>,
    lobby_rooms: Vec<LobbyRoom>,
    loading_lobby: bool,

    // --- Compose ---
    compose_buf: gtk::EntryBuffer,

    // --- Factories ---
    room_factory: FactoryVecDeque<RoomRow>,
    message_factory: FactoryVecDeque<MessageRow>,
    space_factory: FactoryVecDeque<SpaceChip>,
    lobby_factory: FactoryVecDeque<LobbyRoomRow>,

    toaster: Toaster,
    ui: AppUi,
}

#[derive(Debug)]
pub enum AppMsg {
    // Connect screen
    Login,
    Sso,
    ReopenSso,
    CancelSso,
    ToggleRemember(bool),

    // Rooms screen
    FilterChanged,
    SelectSpace(Option<String>),
    OpenRoom(String),
    OpenSettings,
    OpenLobby,

    // Lobby screen
    LobbyOpenRoom(String),
    LobbyJoinRoom(String),

    // Room screen
    Back,
    Send,
    /// Auto-triggered by the message scroller nearing the top of the loaded
    /// history — see the `connect_value_changed` handler wired up in `init`.
    LoadOlder,

    // Settings screen
    ToggleTheme(bool),
    Logout,

    // From the bridge thread.
    Bridge(MatrixEvent),
}

#[relm4::component(pub)]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        #[root]
        adw::ApplicationWindow {
            set_title: Some("neo"),
            set_default_width: 420,
            set_default_height: 800,

            #[local_ref]
            toast_overlay -> adw::ToastOverlay {}
        }
    }

    fn init(_init: Self::Init, root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);

        let (cmd_tx, mut evt_rx) = matrix_bridge::spawn();
        let bridge_sender = sender.clone();
        relm4::spawn(async move {
            while let Some(evt) = evt_rx.recv().await {
                bridge_sender.input(AppMsg::Bridge(evt));
            }
        });

        let room_factory = FactoryVecDeque::builder().launch_default().forward(
            sender.input_sender(),
            |out| match out {
                RoomRowOutput::Open(id) => AppMsg::OpenRoom(id),
            },
        );
        let space_factory = FactoryVecDeque::builder().launch_default().forward(
            sender.input_sender(),
            |out| match out {
                SpaceChipOutput::Select(id) => AppMsg::SelectSpace(id),
            },
        );
        let lobby_factory = FactoryVecDeque::builder().launch_default().forward(
            sender.input_sender(),
            |out| match out {
                LobbyRoomRowOutput::Open(id) => AppMsg::LobbyOpenRoom(id),
                LobbyRoomRowOutput::Join(id) => AppMsg::LobbyJoinRoom(id),
            },
        );
        let message_factory = FactoryVecDeque::builder().launch_default().detach();

        let homeserver_buf = gtk::EntryBuffer::new(Some("matrix.org"));
        let username_buf = gtk::EntryBuffer::default();
        let password_buf = gtk::EntryBuffer::default();
        let room_filter_buf = gtk::EntryBuffer::default();
        let compose_buf = gtk::EntryBuffer::default();
        let toaster = Toaster::default();
        let ui = AppUi::new(
            &homeserver_buf,
            &username_buf,
            &password_buf,
            &room_filter_buf,
            &compose_buf,
            space_factory.widget(),
            room_factory.widget(),
            lobby_factory.widget(),
            message_factory.widget(),
        );
        let toast_overlay = toaster.overlay_widget().clone();
        toast_overlay.set_child(Some(&ui.root));

        let mut model = AppModel {
            cmd_tx,
            screen: Screen::Connect,
            dark: true,
            homeserver_buf,
            username_buf,
            password_buf,
            remember_me: true,
            connection: ConnectionState::Disconnected,
            user_id: None,
            homeserver: None,
            error: None,
            sso_url: None,
            sso_pending: false,
            rooms: Vec::new(),
            room_filter_buf,
            rooms_empty_hint: Some("No rooms yet — joined rooms show up here.".to_owned()),
            loading_rooms: false,
            active_room: None,
            messages: HashMap::new(),
            loading_older: false,
            loading_messages: false,
            message_vadj: None,
            restore_scroll_anchor: Rc::new(Cell::new(None)),
            space_children: HashMap::new(),
            active_space: None,
            has_spaces: false,
            lobby_space: None,
            lobby_rooms: Vec::new(),
            loading_lobby: false,
            space_chip_shape: Vec::new(),
            compose_buf,
            room_factory,
            message_factory,
            space_factory,
            lobby_factory,
            toaster,
            ui,
        };

        model.connect_ui(&sender);
        model.refresh_ui();

        let widgets = view_output!();

        // Keep the timeline pinned to the newest message as it grows, same
        // as the old egui `ScrollArea::stick_to_bottom` — but only while the
        // user hasn't scrolled away from the bottom (`sticky_bottom`), and
        // preserving position rather than snapping when older history gets
        // prepended above the current view (`restore_scroll_anchor`).
        //
        // Also replaces the old manual "Load older" button: nearing the top
        // asks the bridge for more history on its own, Element/Discord-style.
        let vadj = model.ui.message_scroller.vadjustment();
        model.message_vadj = Some(vadj.clone());

        const LOAD_OLDER_THRESHOLD: f64 = 200.0;
        let sticky_bottom = Rc::new(Cell::new(true));

        {
            let sticky_bottom = sticky_bottom.clone();
            let sender = sender.clone();
            vadj.connect_value_changed(move |adj| {
                sticky_bottom.set(adj.value() + adj.page_size() >= adj.upper() - 1.0);
                if adj.value() <= LOAD_OLDER_THRESHOLD {
                    sender.input(AppMsg::LoadOlder);
                }
            });
        }
        {
            let restore_scroll_anchor = model.restore_scroll_anchor.clone();
            vadj.connect_changed(move |adj| {
                if let Some((old_upper, old_value)) = restore_scroll_anchor.take() {
                    adj.set_value(old_value + (adj.upper() - old_upper));
                } else if sticky_bottom.get() {
                    adj.set_value(adj.upper() - adj.page_size());
                }
            });
        }

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            AppMsg::Login => {
                self.connection = ConnectionState::Connecting;
                self.error = None;
                self.sso_pending = false;
                let _ = self.cmd_tx.send(MatrixCmd::Login {
                    homeserver: self.homeserver_buf.text().trim().to_owned(),
                    username: self.username_buf.text().trim().to_owned(),
                    password: self.password_buf.text().to_string(),
                    remember: self.remember_me,
                });
            }
            AppMsg::Sso => {
                self.connection = ConnectionState::Connecting;
                self.error = None;
                self.sso_url = None;
                self.sso_pending = true;
                let _ = self.cmd_tx.send(MatrixCmd::LoginSso {
                    homeserver: self.homeserver_buf.text().trim().to_owned(),
                    remember: self.remember_me,
                });
            }
            AppMsg::ReopenSso => {
                if let Some(url) = &self.sso_url {
                    let _ = open::that_detached(url);
                }
            }
            AppMsg::CancelSso => {
                // The bridge task may still complete in the background (it
                // isn't cancelled) — if it does, the resulting LoggedIn
                // event is honored normally. This just stops the UI from
                // waiting.
                self.connection = ConnectionState::Disconnected;
                self.sso_url = None;
                self.sso_pending = false;
            }
            AppMsg::ToggleRemember(v) => self.remember_me = v,

            AppMsg::FilterChanged => self.sync_room_list(),
            AppMsg::SelectSpace(id) => {
                self.active_space = id;
                self.sync_room_list();
                self.sync_space_chips();
                // The rail stays visible while viewing a channel, so picking
                // a different space from there should land you on that
                // space's channel list — not leave you staring at the old
                // channel while the list re-filters behind it.
                if self.screen == Screen::Room {
                    self.screen = Screen::Rooms;
                }
            }
            AppMsg::OpenRoom(id) => self.open_room(id),
            AppMsg::OpenSettings => self.screen = Screen::Settings,
            AppMsg::OpenLobby => {
                if let Some(space_id) = self.active_space.clone() {
                    self.lobby_space = Some(space_id.clone());
                    self.lobby_rooms.clear();
                    self.loading_lobby = true;
                    self.sync_lobby_rows();
                    self.screen = Screen::Lobby;
                    let _ = self.cmd_tx.send(MatrixCmd::OpenLobby(space_id));
                }
            }

            AppMsg::LobbyOpenRoom(id) => self.open_room(id),
            AppMsg::LobbyJoinRoom(id) => {
                if let Some(space_id) = self.lobby_space.clone() {
                    let via = self
                        .lobby_rooms
                        .iter()
                        .find(|r| r.id == id)
                        .map(|r| r.via.clone())
                        .unwrap_or_default();
                    let _ = self.cmd_tx.send(MatrixCmd::JoinRoom { room_id: id, space_id, via });
                }
            }

            AppMsg::Back => self.screen = Screen::Rooms,
            AppMsg::Send => {
                let text = self.compose_buf.text().trim().to_owned();
                if !text.is_empty() {
                    if let Some(room_id) = self.active_room.clone() {
                        let _ = self.cmd_tx.send(MatrixCmd::Send { room_id, text });
                    }
                    self.compose_buf.set_text("");
                }
            }
            AppMsg::LoadOlder => {
                // Guards against the scroller re-firing this on every
                // intermediate scroll event while still near the top and a
                // request is already in flight — the manual button used to
                // get this for free from `set_sensitive: !loading_older`.
                if !self.loading_older {
                    if let Some(room_id) = self.active_room.clone() {
                        self.loading_older = true;
                        let _ = self.cmd_tx.send(MatrixCmd::LoadOlder(room_id));
                    }
                }
            }

            AppMsg::ToggleTheme(dark) => {
                self.dark = dark;
                let scheme =
                    if dark { adw::ColorScheme::ForceDark } else { adw::ColorScheme::ForceLight };
                adw::StyleManager::default().set_color_scheme(scheme);
            }
            AppMsg::Logout => {
                let _ = self.cmd_tx.send(MatrixCmd::Logout);
            }

            AppMsg::Bridge(evt) => self.handle_bridge_event(evt),
        }
        self.refresh_ui();
    }
}

impl AppModel {
    fn connect_ui(&self, sender: &ComponentSender<Self>) {
        macro_rules! clicked {
            ($widget:expr, $message:expr) => {{
                let sender = sender.clone();
                $widget.connect_clicked(move |_| sender.input($message));
            }};
        }

        clicked!(self.ui.reopen_sso, AppMsg::ReopenSso);
        clicked!(self.ui.cancel_sso, AppMsg::CancelSso);
        clicked!(self.ui.login_button, AppMsg::Login);
        clicked!(self.ui.sso_button, AppMsg::Sso);
        clicked!(self.ui.settings_button, AppMsg::OpenSettings);
        clicked!(self.ui.lobby_button, AppMsg::OpenLobby);
        clicked!(self.ui.lobby_back, AppMsg::Back);
        clicked!(self.ui.room_back, AppMsg::Back);
        clicked!(self.ui.send_button, AppMsg::Send);
        clicked!(self.ui.settings_back, AppMsg::Back);
        clicked!(self.ui.logout_button, AppMsg::Logout);

        {
            let sender = sender.clone();
            self.ui.password_entry.connect_activate(move |_| sender.input(AppMsg::Login));
        }
        {
            let sender = sender.clone();
            self.ui.compose_entry.connect_activate(move |_| sender.input(AppMsg::Send));
        }
        {
            let sender = sender.clone();
            self.ui.room_filter.connect_changed(move |_| sender.input(AppMsg::FilterChanged));
        }
        {
            let sender = sender.clone();
            self.ui.remember_switch.connect_active_notify(move |row| {
                sender.input(AppMsg::ToggleRemember(row.is_active()));
            });
        }
        {
            let sender = sender.clone();
            self.ui.dark_switch.connect_active_notify(move |row| {
                sender.input(AppMsg::ToggleTheme(row.is_active()));
            });
        }
    }

    fn refresh_ui(&self) {
        let connecting = self.connection == ConnectionState::Connecting;
        let waiting_for_sso = self.sso_url.is_some();
        let room_name = self.active_room_name();
        let room_address = self.active_room_address();

        self.ui.window_title.set_subtitle(self.connection.label());
        self.ui.space_rail.set_visible(self.has_spaces);
        self.ui.screen_stack.set_visible_child_name(self.screen.name());
        self.ui.sso_waiting.set_visible(waiting_for_sso);
        self.ui.login_form.set_visible(!waiting_for_sso);
        self.ui.sso_url.set_label(self.sso_url.as_deref().unwrap_or(""));
        self.ui.login_error.set_visible(self.error.is_some());
        self.ui.login_error.set_label(self.error.as_deref().unwrap_or(""));
        self.ui.login_button.set_label(
            if connecting && !self.sso_pending { "Signing in…" } else { "Sign in" },
        );
        self.ui.sso_button.set_label(
            if connecting && self.sso_pending { "Signing in…" } else { "Sign in with SSO" },
        );
        self.ui.login_button.set_sensitive(!connecting);
        self.ui.sso_button.set_sensitive(!connecting);
        if self.ui.remember_switch.is_active() != self.remember_me {
            self.ui.remember_switch.set_active(self.remember_me);
        }

        self.ui.rooms_user.set_label(self.user_id.as_deref().unwrap_or(""));
        self.ui.lobby_button.set_visible(self.active_space.is_some());
        self.ui.rooms_empty.set_visible(self.rooms_empty_hint.is_some());
        self.ui.rooms_empty.set_label(self.rooms_empty_hint.as_deref().unwrap_or(""));
        self.ui
            .lobby_stack
            .set_visible_child_name(if self.loading_lobby { "loading" } else { "rooms" });
        self.ui
            .lobby_empty
            .set_visible(!self.loading_lobby && self.lobby_rooms.is_empty());

        self.ui.room_name.set_label(&room_name);
        self.ui.room_address.set_label(&room_address);
        self.ui
            .message_stack
            .set_visible_child_name(if self.loading_messages { "loading" } else { "messages" });

        self.ui.settings_user.set_subtitle(self.user_id.as_deref().unwrap_or("—"));
        self.ui
            .settings_homeserver
            .set_subtitle(self.homeserver.as_deref().unwrap_or("—"));
        if self.ui.dark_switch.is_active() != self.dark {
            self.ui.dark_switch.set_active(self.dark);
        }
    }

    fn handle_bridge_event(&mut self, evt: MatrixEvent) {
        match evt {
            MatrixEvent::Connecting => {
                self.connection = ConnectionState::Connecting;
                self.error = None;
            }
            MatrixEvent::SsoUrl(url) => {
                // Best-effort auto-open; the connect screen also shows `url`
                // as a fallback link/"Open again" button in case there's no
                // default browser configured (e.g. sandboxed/headless).
                let _ = open::that_detached(&url);
                self.sso_url = Some(url);
            }
            MatrixEvent::LoggedIn { user_id, homeserver } => {
                self.connection = ConnectionState::Connected;
                self.user_id = Some(user_id);
                self.homeserver = Some(homeserver);
                self.error = None;
                self.sso_url = None;
                self.sso_pending = false;
                self.screen = Screen::Rooms;
                self.password_buf.set_text("");
                // The room list hasn't loaded yet at this point (the bridge
                // reports LoggedIn as soon as auth succeeds, before its
                // first sync) — show "Loading rooms…" instead of the
                // steady-state "No rooms yet" empty hint for that gap.
                self.loading_rooms = true;
                self.rooms_empty_hint = Some("Loading rooms…".to_owned());
                self.toaster.add_toast(adw::Toast::new("Signed in"));
            }
            MatrixEvent::LoginFailed(err) => {
                self.connection = ConnectionState::Disconnected;
                self.sso_url = None;
                self.sso_pending = false;
                self.error = Some(err);
            }
            MatrixEvent::Rooms(rooms) => {
                self.rooms = rooms;
                self.sync_room_list();
                self.sync_space_chips();
            }
            MatrixEvent::SpaceChildren(space_children) => {
                // A space the user switched into may have been left/removed
                // server-side by the time this update lands — fall back to
                // Home instead of showing an empty, unrecoverable list.
                //
                // `space_children` omits entries for spaces with zero
                // children (see `collect_space_children`), so checking its
                // keys can't distinguish "left/removed" from "still joined,
                // just empty" — an empty space would fail `contains_key` on
                // every one of these updates and get silently kicked back
                // to Home a moment after being selected. Check against the
                // known room list instead.
                if let Some(active) = &self.active_space {
                    let still_joined = self.rooms.iter().any(|r| r.is_space && &r.id == active);
                    if !still_joined {
                        self.active_space = None;
                    }
                }
                self.space_children = space_children;
                self.loading_rooms = false;
                self.sync_room_list();
                self.sync_space_chips();
            }
            MatrixEvent::Timeline { room_id, messages } => {
                self.apply_timeline(room_id, messages);
            }
            MatrixEvent::SendFailed { room_id: _, error } => {
                self.toaster.add_toast(adw::Toast::new(&format!("Send failed: {error}")));
            }
            MatrixEvent::SpaceHierarchy { space_id, rooms } => {
                // A stale response for a lobby the user has since navigated
                // away from — ignore it rather than repopulating the wrong
                // screen's list.
                if self.lobby_space.as_deref() == Some(space_id.as_str()) {
                    self.lobby_rooms = rooms;
                    self.loading_lobby = false;
                    self.sync_lobby_rows();
                }
            }
            MatrixEvent::Error(err) => {
                self.toaster.add_toast(adw::Toast::new(&err));
            }
            MatrixEvent::LoggedOut => self.reset_session(),
        }
    }

    /// Non-space joined rooms, filtered by the search box and (if set) the
    /// active space's local `m.space.child` membership.
    fn filtered_rooms(&self) -> Vec<&RoomSummary> {
        let needle = self.room_filter_buf.text().trim().to_lowercase();
        let space_members = self.active_space.as_ref().and_then(|id| self.space_children.get(id));
        self.rooms
            .iter()
            .filter(|r| !r.is_space)
            .filter(|r| needle.is_empty() || r.name.to_lowercase().contains(&needle))
            .filter(|r| space_members.map_or(true, |members| members.iter().any(|id| id == &r.id)))
            .collect()
    }

    fn sync_room_list(&mut self) {
        let rooms: Vec<RoomSummary> = self.filtered_rooms().into_iter().cloned().collect();
        self.rooms_empty_hint = if !rooms.is_empty() {
            None
        } else if self.loading_rooms {
            // First room list hasn't landed yet — don't let a stray call
            // here (e.g. the filter box) flash "No rooms yet" before it does.
            Some("Loading rooms…".to_owned())
        } else if self.rooms.iter().any(|r| !r.is_space) {
            Some("No rooms match your search.".to_owned())
        } else {
            Some("No rooms yet — joined rooms show up here.".to_owned())
        };

        let mut guard = self.room_factory.guard();
        guard.clear();
        for room in rooms {
            guard.push_back(room);
        }
    }

    fn sync_space_chips(&mut self) {
        let spaces: Vec<&RoomSummary> = self.rooms.iter().filter(|r| r.is_space).collect();
        self.has_spaces = !spaces.is_empty();

        let mut shape: Vec<(Option<String>, String, Option<Vec<u8>>)> =
            Vec::with_capacity(spaces.len() + 1);
        shape.push((None, "Home".to_owned(), None));
        for space in &spaces {
            shape.push((Some(space.id.clone()), space.name.clone(), space.avatar.clone()));
        }

        if shape == self.space_chip_shape {
            // Same chips as last render — just move the highlight, in place,
            // so a click doesn't destroy and recreate the button underneath
            // it (which would drop keyboard focus off whichever space you
            // just picked).
            for (i, (id, _, _)) in shape.iter().enumerate() {
                let selected = self.active_space.as_ref() == id.as_ref();
                self.space_factory.send(i, SpaceChipInput::SetSelected(selected));
            }
            return;
        }

        let mut guard = self.space_factory.guard();
        guard.clear();
        for (id, label, avatar) in &shape {
            guard.push_back(SpaceChip {
                id: id.clone(),
                label: label.clone(),
                avatar: avatar.clone(),
                state: (self.active_space.as_ref() == id.as_ref()).into(),
            });
        }
        drop(guard);
        self.space_chip_shape = shape;
    }

    /// Switches to the Room screen and (re)subscribes to `id`'s live
    /// timeline. Shared by the normal joined-channel list and the Lobby
    /// directory's "already joined" rows.
    fn open_room(&mut self, id: String) {
        self.active_room = Some(id.clone());
        self.screen = Screen::Room;
        let has_cache = self.messages.get(&id).is_some_and(|m| !m.is_empty());
        self.loading_messages = !has_cache;
        self.sync_messages();
        let _ = self.cmd_tx.send(MatrixCmd::OpenRoom(id));
    }

    fn sync_lobby_rows(&mut self) {
        let mut guard = self.lobby_factory.guard();
        guard.clear();
        for room in self.lobby_rooms.clone() {
            guard.push_back(room);
        }
    }

    fn active_room_summary(&self) -> Option<&RoomSummary> {
        let id = self.active_room.as_ref()?;
        self.rooms.iter().find(|r| &r.id == id)
    }

    fn active_room_name(&self) -> String {
        self.active_room_summary().map(|r| r.name.clone()).unwrap_or_else(|| "Room".to_owned())
    }

    /// The room's Matrix ID (e.g. `!abc123:example.org`), shown under the
    /// channel name so users can find/share the room's real address.
    fn active_room_address(&self) -> String {
        self.active_room_summary().map(|r| r.id.clone()).unwrap_or_default()
    }

    /// Every `MatrixEvent::Timeline` is a full, already-deduplicated snapshot
    /// of the room's live timeline (the bridge re-flattens its
    /// `eyeball_im::Vector` on every diff), so there's no merge/dedup logic
    /// needed here anymore — just replace.
    fn apply_timeline(&mut self, room_id: String, incoming: Vec<ChatMessage>) {
        let was_loading_older = self.loading_older;
        self.loading_older = false;
        self.messages.insert(room_id.clone(), incoming);

        if self.active_room.as_deref() == Some(room_id.as_str()) {
            self.loading_messages = false;
            // See `restore_scroll_anchor`'s doc comment: snapshot the
            // scroller's current position now, before `sync_messages`
            // rebuilds the list with older history prepended.
            if was_loading_older {
                if let Some(vadj) = &self.message_vadj {
                    self.restore_scroll_anchor.set(Some((vadj.upper(), vadj.value())));
                }
            }
            self.sync_messages();
        }
    }

    fn sync_messages(&mut self) {
        let mut guard = self.message_factory.guard();
        guard.clear();
        let Some(room_id) = self.active_room.clone() else { return };
        let mut last_day = None;
        for msg in self.messages.get(&room_id).cloned().unwrap_or_default() {
            let day = local_day(msg.ts_millis);
            if day.is_some() && day != last_day {
                last_day = day;
                guard.push_back(TimelineRow::DaySeparator(format_day_label(day.unwrap())));
            }
            guard.push_back(TimelineRow::Message(msg));
        }
    }

    fn reset_session(&mut self) {
        self.connection = ConnectionState::Disconnected;
        self.user_id = None;
        self.homeserver = None;
        self.sso_url = None;
        self.rooms.clear();
        self.loading_rooms = false;
        self.active_room = None;
        self.messages.clear();
        self.loading_messages = false;
        self.space_children.clear();
        self.active_space = None;
        self.lobby_space = None;
        self.lobby_rooms.clear();
        self.loading_lobby = false;
        self.screen = Screen::Connect;
        self.sync_room_list();
        self.sync_space_chips();
        self.sync_lobby_rows();
        self.sync_messages();
    }
}
