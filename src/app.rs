//! neo's single Relm4 component: owns every screen's state and view tree
//! (switched via a `gtk::Stack`, not separate child components — the whole
//! app is small enough that one `AppModel` mirrors the old single-`AppState`
//! shape closely) and bridges [`crate::matrix_bridge`]'s `mpsc` channel into
//! Relm4's message loop.

use std::collections::HashMap;

use adw::prelude::*;
use relm4::abstractions::Toaster;
use relm4::factory::FactoryVecDeque;
use relm4::prelude::*;

use crate::factory::message_row::MessageRow;
use crate::factory::room_row::{RoomRow, RoomRowOutput};
use crate::factory::space_chip::{SpaceChip, SpaceChipOutput};
use crate::matrix_bridge::{self, MatrixCmd, MatrixEvent};
use crate::state::{ChatMessage, ConnectionState, RoomSummary, Screen};

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
    loading_older: bool,
    /// True from opening a room until its first timeline fetch lands, so the
    /// room screen can show a spinner instead of a blank scroller — replaces
    /// the (already-cached) messages for a room you've visited before, no
    /// spinner needed then.
    loading_messages: bool,

    // --- Spaces ---
    space_children: HashMap<String, Vec<String>>,
    active_space: Option<String>,
    has_spaces: bool,

    // --- Compose ---
    compose_buf: gtk::EntryBuffer,

    // --- Factories ---
    room_factory: FactoryVecDeque<RoomRow>,
    message_factory: FactoryVecDeque<MessageRow>,
    space_factory: FactoryVecDeque<SpaceChip>,

    toaster: Toaster,
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

    // Room screen
    Back,
    Send,
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
            toast_overlay -> adw::ToastOverlay {
                #[wrap(Some)]
                set_child = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,

                    adw::HeaderBar {
                        #[wrap(Some)]
                        set_title_widget = &adw::WindowTitle {
                            set_title: "neo",
                            #[watch]
                            set_subtitle: model.connection.label(),
                        },
                    },

                    gtk::Stack {
                        set_vexpand: true,
                        set_transition_type: gtk::StackTransitionType::Crossfade,

                        add_named[Some("connect")] = &gtk::ScrolledWindow {
                            set_vexpand: true,

                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_spacing: 16,
                                set_margin_all: 24,
                                set_valign: gtk::Align::Center,
                                set_halign: gtk::Align::Center,
                                set_width_request: 360,

                                gtk::Label { set_label: "neo", add_css_class: "title-1" },
                                gtk::Label {
                                    set_label: "A matrix.org client.",
                                    add_css_class: "dim-label",
                                },

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 12,
                                    #[watch]
                                    set_visible: model.sso_url.is_some(),

                                    gtk::Label {
                                        set_label: "Continue in your browser",
                                        add_css_class: "title-3",
                                    },
                                    gtk::Label {
                                        set_label: "We opened your homeserver's sign-in page. Waiting for it to finish…",
                                        set_wrap: true,
                                        add_css_class: "dim-label",
                                    },
                                    gtk::Spinner { set_spinning: true, set_halign: gtk::Align::Center },
                                    gtk::Label {
                                        #[watch]
                                        set_label: model.sso_url.as_deref().unwrap_or(""),
                                        set_wrap: true,
                                        set_selectable: true,
                                        add_css_class: "caption",
                                        add_css_class: "dim-label",
                                    },
                                    gtk::Box {
                                        set_orientation: gtk::Orientation::Horizontal,
                                        set_spacing: 8,
                                        set_halign: gtk::Align::Center,

                                        gtk::Button {
                                            set_label: "Open again",
                                            connect_clicked => AppMsg::ReopenSso,
                                        },
                                        gtk::Button {
                                            set_label: "Cancel",
                                            connect_clicked => AppMsg::CancelSso,
                                        },
                                    },
                                },

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 12,
                                    #[watch]
                                    set_visible: model.sso_url.is_none(),

                                    gtk::Label {
                                        #[watch]
                                        set_visible: model.error.is_some(),
                                        #[watch]
                                        set_label: model.error.as_deref().unwrap_or(""),
                                        set_wrap: true,
                                        add_css_class: "error",
                                    },

                                    adw::PreferencesGroup {
                                        set_title: "Sign in",
                                        set_description: Some("Your homeserver account — the same one Element uses."),

                                        adw::ActionRow {
                                            set_title: "Homeserver",
                                            add_suffix = &gtk::Entry {
                                                set_buffer: &model.homeserver_buf,
                                                set_valign: gtk::Align::Center,
                                                set_hexpand: true,
                                            },
                                        },
                                        adw::ActionRow {
                                            set_title: "Username",
                                            add_suffix = &gtk::Entry {
                                                set_buffer: &model.username_buf,
                                                set_valign: gtk::Align::Center,
                                                set_hexpand: true,
                                            },
                                        },
                                        adw::ActionRow {
                                            set_title: "Password",
                                            add_suffix = &gtk::Entry {
                                                set_buffer: &model.password_buf,
                                                set_visibility: false,
                                                set_input_purpose: gtk::InputPurpose::Password,
                                                set_valign: gtk::Align::Center,
                                                set_hexpand: true,
                                                connect_activate => AppMsg::Login,
                                            },
                                        },
                                        // AdwSwitchRow (like ActionRow) is a
                                        // ListBoxRow — it must live inside a
                                        // PreferencesGroup/ListBox, not a
                                        // plain Box.
                                        adw::SwitchRow {
                                            set_title: "Remember me on this device",
                                            #[watch]
                                            #[block_signal(remember_handler)]
                                            set_active: model.remember_me,
                                            connect_active_notify[sender] => move |row| {
                                                sender.input(AppMsg::ToggleRemember(row.is_active()));
                                            } @remember_handler,
                                        },
                                    },

                                    gtk::Button {
                                        add_css_class: "suggested-action",
                                        add_css_class: "pill",
                                        #[watch]
                                        set_label: if model.connection == ConnectionState::Connecting
                                            && !model.sso_pending
                                        {
                                            "Signing in…"
                                        } else {
                                            "Sign in"
                                        },
                                        #[watch]
                                        set_sensitive: model.connection != ConnectionState::Connecting,
                                        connect_clicked => AppMsg::Login,
                                    },

                                    gtk::Label {
                                        set_label: "— or —",
                                        set_halign: gtk::Align::Center,
                                        add_css_class: "dim-label",
                                    },

                                    gtk::Button {
                                        add_css_class: "pill",
                                        #[watch]
                                        set_label: if model.connection == ConnectionState::Connecting
                                            && model.sso_pending
                                        {
                                            "Signing in…"
                                        } else {
                                            "Sign in with SSO"
                                        },
                                        #[watch]
                                        set_sensitive: model.connection != ConnectionState::Connecting,
                                        connect_clicked => AppMsg::Sso,
                                    },

                                    gtk::Label {
                                        set_label: "Login only — registration isn't supported yet. Passwords \
                                            go straight to your homeserver over HTTPS and are never stored by \
                                            neo unless you remember this device.",
                                        set_wrap: true,
                                        add_css_class: "caption",
                                        add_css_class: "dim-label",
                                    },
                                },
                            },
                        },

                        add_named[Some("rooms")] = &gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,

                            // Space rail: fixed-width, scrolls vertically —
                            // its footprint never grows with the number or
                            // length of the user's spaces, so it can't force
                            // the window wider than a phone screen.
                            gtk::ScrolledWindow {
                                #[watch]
                                set_visible: model.has_spaces,
                                set_hscrollbar_policy: gtk::PolicyType::Never,
                                // GTK4's overlay scrollbar draws over the
                                // rail instead of reserving its own space —
                                // fine for a tall message list, not for a
                                // narrow avatar column. Hidden here; the
                                // rail still scrolls fine with wheel/trackpad.
                                set_vscrollbar_policy: gtk::PolicyType::Never,
                                set_width_request: 64,

                                #[local_ref]
                                space_chip_box -> gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 6,
                                    set_margin_all: 8,
                                },
                            },

                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_spacing: 8,
                                set_margin_all: 12,
                                set_hexpand: true,

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 8,

                                    gtk::Label {
                                        #[watch]
                                        set_label: model.user_id.as_deref().unwrap_or(""),
                                        add_css_class: "dim-label",
                                        set_hexpand: true,
                                        set_halign: gtk::Align::Start,
                                        set_ellipsize: gtk::pango::EllipsizeMode::Middle,
                                    },
                                    gtk::Button {
                                        set_icon_name: "emblem-system-symbolic",
                                        set_tooltip_text: Some("Settings"),
                                        connect_clicked => AppMsg::OpenSettings,
                                    },
                                },

                                gtk::Entry {
                                    set_buffer: &model.room_filter_buf,
                                    set_placeholder_text: Some("Search rooms…"),
                                    set_primary_icon_name: Some("system-search-symbolic"),
                                    connect_changed => AppMsg::FilterChanged,
                                },

                                gtk::ScrolledWindow {
                                    set_vexpand: true,

                                    #[local_ref]
                                    room_list_box -> gtk::ListBox {
                                        add_css_class: "boxed-list",
                                        set_selection_mode: gtk::SelectionMode::None,
                                        set_valign: gtk::Align::Start,
                                    },
                                },

                                gtk::Label {
                                    #[watch]
                                    set_visible: model.rooms_empty_hint.is_some(),
                                    #[watch]
                                    set_label: model.rooms_empty_hint.as_deref().unwrap_or(""),
                                    add_css_class: "dim-label",
                                    set_margin_top: 24,
                                },
                            },
                        },

                        add_named[Some("room")] = &gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 8,
                            set_margin_all: 12,

                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 8,

                                gtk::Button {
                                    set_icon_name: "go-previous-symbolic",
                                    connect_clicked => AppMsg::Back,
                                },
                                gtk::Label {
                                    #[watch]
                                    set_label: &model.active_room_name(),
                                    add_css_class: "title-3",
                                    set_hexpand: true,
                                    set_halign: gtk::Align::Start,
                                    set_ellipsize: gtk::pango::EllipsizeMode::End,
                                },
                                gtk::Button {
                                    set_label: "Load older",
                                    #[watch]
                                    set_sensitive: !model.loading_older,
                                    connect_clicked => AppMsg::LoadOlder,
                                },
                            },

                            gtk::Stack {
                                set_vexpand: true,
                                set_transition_type: gtk::StackTransitionType::Crossfade,

                                add_named[Some("loading")] = &gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 12,
                                    set_valign: gtk::Align::Center,
                                    set_halign: gtk::Align::Center,

                                    gtk::Spinner { set_spinning: true },
                                    gtk::Label {
                                        set_label: "Loading messages…",
                                        add_css_class: "dim-label",
                                    },
                                },

                                #[name = "message_scroller"]
                                add_named[Some("messages")] = &gtk::ScrolledWindow {
                                    #[local_ref]
                                    message_box -> gtk::Box {
                                        set_orientation: gtk::Orientation::Vertical,
                                        set_spacing: 6,
                                        set_valign: gtk::Align::End,
                                        set_margin_top: 8,
                                        set_margin_bottom: 8,
                                    },
                                },

                                // Set only after both named pages above exist
                                // — see the matching comment on the outer
                                // screen Stack.
                                #[watch]
                                set_visible_child_name: if model.loading_messages { "loading" } else { "messages" },
                            },

                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 8,

                                gtk::Entry {
                                    set_buffer: &model.compose_buf,
                                    set_placeholder_text: Some("Message…"),
                                    set_hexpand: true,
                                    connect_activate => AppMsg::Send,
                                },
                                gtk::Button {
                                    set_label: "Send",
                                    add_css_class: "suggested-action",
                                    connect_clicked => AppMsg::Send,
                                },
                            },
                        },

                        add_named[Some("settings")] = &gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 16,
                            set_margin_all: 12,

                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 8,

                                gtk::Button {
                                    set_icon_name: "go-previous-symbolic",
                                    connect_clicked => AppMsg::Back,
                                },
                                gtk::Label { set_label: "Settings", add_css_class: "title-3" },
                            },

                            adw::PreferencesGroup {
                                set_title: "Account",

                                adw::ActionRow {
                                    set_title: "Signed in as",
                                    #[watch]
                                    set_subtitle: model.user_id.as_deref().unwrap_or("—"),
                                },
                                adw::ActionRow {
                                    set_title: "Homeserver",
                                    #[watch]
                                    set_subtitle: model.homeserver.as_deref().unwrap_or("—"),
                                },
                            },

                            adw::PreferencesGroup {
                                set_title: "Appearance",

                                adw::SwitchRow {
                                    set_title: "Dark",
                                    #[watch]
                                    #[block_signal(dark_handler)]
                                    set_active: model.dark,
                                    connect_active_notify[sender] => move |row| {
                                        sender.input(AppMsg::ToggleTheme(row.is_active()));
                                    } @dark_handler,
                                },
                            },

                            gtk::Button {
                                set_label: "Sign out",
                                add_css_class: "destructive-action",
                                connect_clicked => AppMsg::Logout,
                            },

                            gtk::Label {
                                set_label: "neo — a matrix.org client built with Relm4.",
                                add_css_class: "dim-label",
                                set_wrap: true,
                            },
                        },

                        // Set only after every named page above has been
                        // added — evaluating this any earlier (e.g. as the
                        // Stack's first property) fires before the pages
                        // exist and GTK logs "child name not found".
                        #[watch]
                        set_visible_child_name: model.screen.name(),
                    },
                },
            },
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
        let message_factory = FactoryVecDeque::builder().launch_default().detach();

        let model = AppModel {
            cmd_tx,
            screen: Screen::Connect,
            dark: true,
            homeserver_buf: gtk::EntryBuffer::new(Some("matrix.org")),
            username_buf: gtk::EntryBuffer::default(),
            password_buf: gtk::EntryBuffer::default(),
            remember_me: true,
            connection: ConnectionState::Disconnected,
            user_id: None,
            homeserver: None,
            error: None,
            sso_url: None,
            sso_pending: false,
            rooms: Vec::new(),
            room_filter_buf: gtk::EntryBuffer::default(),
            rooms_empty_hint: Some("No rooms yet — joined rooms show up here.".to_owned()),
            loading_rooms: false,
            active_room: None,
            messages: HashMap::new(),
            loading_older: false,
            loading_messages: false,
            space_children: HashMap::new(),
            active_space: None,
            has_spaces: false,
            compose_buf: gtk::EntryBuffer::default(),
            room_factory,
            message_factory,
            space_factory,
            toaster: Toaster::default(),
        };

        let toast_overlay = model.toaster.overlay_widget();
        let room_list_box = model.room_factory.widget();
        let message_box = model.message_factory.widget();
        let space_chip_box = model.space_factory.widget();

        let widgets = view_output!();

        // Keep the timeline pinned to the newest message as it grows, same
        // as the old egui `ScrollArea::stick_to_bottom`.
        let vadj = widgets.message_scroller.vadjustment();
        vadj.connect_changed(move |adj| {
            adj.set_value(adj.upper() - adj.page_size());
        });

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
            }
            AppMsg::OpenRoom(id) => {
                self.active_room = Some(id.clone());
                self.screen = Screen::Room;
                let has_cache = self.messages.get(&id).is_some_and(|m| !m.is_empty());
                self.loading_messages = !has_cache;
                self.sync_messages();
                let _ = self.cmd_tx.send(MatrixCmd::OpenRoom(id));
            }
            AppMsg::OpenSettings => self.screen = Screen::Settings,

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
                if let Some(room_id) = self.active_room.clone() {
                    self.loading_older = true;
                    let _ = self.cmd_tx.send(MatrixCmd::LoadOlder(room_id));
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
    }
}

impl AppModel {
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
                if let Some(active) = &self.active_space {
                    if !space_children.contains_key(active) {
                        self.active_space = None;
                    }
                }
                self.space_children = space_children;
                self.loading_rooms = false;
                self.sync_room_list();
            }
            MatrixEvent::Timeline { room_id, messages } => {
                self.apply_timeline(room_id, messages);
            }
            MatrixEvent::SendFailed { room_id: _, error } => {
                self.toaster.add_toast(adw::Toast::new(&format!("Send failed: {error}")));
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

        let mut guard = self.space_factory.guard();
        guard.clear();
        guard.push_back(SpaceChip {
            id: None,
            label: "Home".to_owned(),
            avatar: None,
            selected: self.active_space.is_none(),
        });
        for space in spaces {
            guard.push_back(SpaceChip {
                id: Some(space.id.clone()),
                label: space.name.clone(),
                avatar: space.avatar.clone(),
                selected: self.active_space.as_deref() == Some(space.id.as_str()),
            });
        }
    }

    fn active_room_summary(&self) -> Option<&RoomSummary> {
        let id = self.active_room.as_ref()?;
        self.rooms.iter().find(|r| &r.id == id)
    }

    fn active_room_name(&self) -> String {
        self.active_room_summary().map(|r| r.name.clone()).unwrap_or_else(|| "Room".to_owned())
    }

    /// Every `MatrixEvent::Timeline` is a full, already-deduplicated snapshot
    /// of the room's live timeline (the bridge re-flattens its
    /// `eyeball_im::Vector` on every diff), so there's no merge/dedup logic
    /// needed here anymore — just replace.
    fn apply_timeline(&mut self, room_id: String, incoming: Vec<ChatMessage>) {
        self.loading_older = false;
        self.messages.insert(room_id.clone(), incoming);

        if self.active_room.as_deref() == Some(room_id.as_str()) {
            self.loading_messages = false;
            self.sync_messages();
        }
    }

    fn sync_messages(&mut self) {
        let mut guard = self.message_factory.guard();
        guard.clear();
        let Some(room_id) = self.active_room.clone() else { return };
        for msg in self.messages.get(&room_id).cloned().unwrap_or_default() {
            guard.push_back(msg);
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
        self.screen = Screen::Connect;
        self.sync_room_list();
        self.sync_space_chips();
        self.sync_messages();
    }
}
