//! neo eframe app — wires the Matrix bridge to the vidya-themed UI.

use std::time::{Duration, Instant};

use eframe::egui;
use tokio::sync::mpsc;
use vidya::{apply_dark, apply_light, Theme};

use crate::matrix_bridge::{MatrixCmd, MatrixEvent};
use crate::state::{AppState, ChatMessage, ConnectionState, Screen};
use crate::ui::{self, ConnectAction, RoomAction, RoomsAction, SettingsAction};

/// How often the UI asks the bridge to re-sync while connected.
const POLL_INTERVAL: Duration = Duration::from_secs(3);

pub struct NeoApp {
    state: AppState,
    cmd_tx: mpsc::UnboundedSender<MatrixCmd>,
    evt_rx: mpsc::UnboundedReceiver<MatrixEvent>,
    theme_dark: bool,
}

impl NeoApp {
    pub fn new(ctx: &egui::Context) -> Self {
        let (cmd_tx, evt_rx) = crate::matrix_bridge::spawn();
        apply_dark(ctx);
        Self { state: AppState::default(), cmd_tx, evt_rx, theme_dark: true }
    }

    fn theme(&self) -> Theme {
        if self.state.dark { Theme::dark() } else { Theme::light() }
    }

    fn send(&self, cmd: MatrixCmd) {
        let _ = self.cmd_tx.send(cmd);
    }

    fn drain_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.evt_rx.try_recv() {
            match event {
                MatrixEvent::Connecting => {
                    self.state.connection = ConnectionState::Connecting;
                    self.state.error = None;
                }
                MatrixEvent::SsoUrl(url) => {
                    // Best-effort auto-open; the connect screen also shows
                    // `url` as a fallback link/"Open again" button in case
                    // there's no default browser configured (e.g. sandboxed
                    // or headless environments).
                    let _ = open::that_detached(&url);
                    self.state.sso_url = Some(url);
                }
                MatrixEvent::LoggedIn { user_id, homeserver } => {
                    self.state.connection = ConnectionState::Connected;
                    self.state.user_id = Some(user_id);
                    self.state.homeserver = Some(homeserver);
                    self.state.error = None;
                    self.state.sso_url = None;
                    self.state.screen = Screen::Rooms;
                    self.state.form_password.clear();
                    self.state.last_poll = Instant::now();
                    self.state.toast("Signed in");
                }
                MatrixEvent::LoginFailed(err) => {
                    self.state.connection = ConnectionState::Disconnected;
                    self.state.sso_url = None;
                    self.state.error = Some(err);
                }
                MatrixEvent::Rooms { rooms, space_children } => {
                    // A space the user switched into may have been left/removed
                    // server-side by the time this poll lands — fall back to Home
                    // instead of showing an empty, unrecoverable filtered list.
                    if let Some(active) = &self.state.active_space {
                        if !space_children.contains_key(active) {
                            self.state.active_space = None;
                        }
                    }
                    self.state.rooms = rooms;
                    self.state.space_children = space_children;
                }
                MatrixEvent::Timeline { room_id, messages, prepend } => {
                    self.apply_timeline(room_id, messages, prepend);
                }
                MatrixEvent::SendFailed { room_id: _, error } => {
                    self.state.toast(format!("Send failed: {error}"));
                }
                MatrixEvent::Error(err) => {
                    self.state.toast(err);
                }
                MatrixEvent::LoggedOut => {
                    self.state.reset_session();
                }
            }
            ctx.request_repaint();
        }
    }

    fn apply_timeline(&mut self, room_id: String, mut incoming: Vec<ChatMessage>, prepend: bool) {
        self.state.loading_older = false;
        let existing = self.state.messages.entry(room_id).or_default();
        if prepend && !existing.is_empty() {
            // Merge on event_id / (sender, ts, body) to avoid duplicate rows
            // across successive fetches of the same recent window.
            let known: std::collections::HashSet<(String, i64, String)> = existing
                .iter()
                .map(|m| (m.sender.clone(), m.ts_millis, m.body.clone()))
                .collect();
            incoming.retain(|m| !known.contains(&(m.sender.clone(), m.ts_millis, m.body.clone())));
            let mut merged = incoming;
            merged.extend(existing.drain(..));
            *existing = merged;
        } else {
            *existing = incoming;
        }
    }

    fn maybe_poll(&mut self) {
        if self.state.connection != ConnectionState::Connected {
            return;
        }
        if self.state.last_poll.elapsed() >= POLL_INTERVAL {
            self.state.last_poll = Instant::now();
            self.send(MatrixCmd::Poll);
        }
    }
}

impl eframe::App for NeoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events(ctx);
        self.state.expire_toasts();
        self.maybe_poll();

        if self.theme_dark != self.state.dark {
            self.theme_dark = self.state.dark;
            if self.state.dark { apply_dark(ctx) } else { apply_light(ctx) }
        }
        let theme = self.theme();

        egui::CentralPanel::default().frame(theme.page_frame()).show(ctx, |ui| match self.state.screen {
            Screen::Connect => {
                let action = ui::connect::connect_screen(ui, &theme, &mut self.state);
                self.handle_connect_action(action);
            }
            Screen::Rooms => {
                let action = ui::rooms::rooms_screen(ui, &theme, &mut self.state);
                self.handle_rooms_action(action);
            }
            Screen::Room => {
                let action = ui::room::room_screen(ui, &theme, &mut self.state);
                self.handle_room_action(action);
            }
            Screen::Settings => {
                let action = ui::settings::settings_screen(ui, &theme, &mut self.state);
                self.handle_settings_action(action);
            }
        });

        self.toasts_overlay(ctx, &theme);

        if self.state.connection == ConnectionState::Connected {
            ctx.request_repaint_after(POLL_INTERVAL);
        }
    }
}

impl NeoApp {
    fn handle_connect_action(&mut self, action: ConnectAction) {
        match action {
            ConnectAction::None => {}
            ConnectAction::Login => {
                self.state.connection = ConnectionState::Connecting;
                self.state.error = None;
                self.send(MatrixCmd::Login {
                    homeserver: self.state.form_homeserver.trim().to_owned(),
                    username: self.state.form_username.trim().to_owned(),
                    password: self.state.form_password.clone(),
                    remember: self.state.remember_me,
                });
            }
            ConnectAction::Sso => {
                self.state.connection = ConnectionState::Connecting;
                self.state.error = None;
                self.state.sso_url = None;
                self.send(MatrixCmd::LoginSso {
                    homeserver: self.state.form_homeserver.trim().to_owned(),
                    remember: self.state.remember_me,
                });
            }
            ConnectAction::CancelSso => {
                // The bridge task may still complete in the background (it
                // isn't cancelled) — if it does, the resulting LoggedIn event
                // is honored normally. This just stops the UI from waiting.
                self.state.connection = ConnectionState::Disconnected;
                self.state.sso_url = None;
            }
        }
    }

    fn handle_rooms_action(&mut self, action: RoomsAction) {
        match action {
            RoomsAction::None => {}
            RoomsAction::Open(room_id) => {
                self.state.active_room = Some(room_id.clone());
                self.state.screen = Screen::Room;
                self.send(MatrixCmd::OpenRoom(room_id));
            }
            RoomsAction::OpenSettings => self.state.screen = Screen::Settings,
        }
    }

    fn handle_room_action(&mut self, action: RoomAction) {
        match action {
            RoomAction::None => {}
            RoomAction::Back => self.state.screen = Screen::Rooms,
            RoomAction::Send(text) => {
                if let Some(room_id) = self.state.active_room.clone() {
                    self.send(MatrixCmd::Send { room_id, text });
                }
            }
            RoomAction::LoadOlder => {
                if let Some(room_id) = self.state.active_room.clone() {
                    self.state.loading_older = true;
                    self.send(MatrixCmd::LoadOlder(room_id));
                }
            }
        }
    }

    fn handle_settings_action(&mut self, action: SettingsAction) {
        match action {
            SettingsAction::None => {}
            SettingsAction::Back => self.state.screen = Screen::Rooms,
            SettingsAction::ToggleTheme => self.state.dark = !self.state.dark,
            SettingsAction::Logout => self.send(MatrixCmd::Logout),
        }
    }

    fn toasts_overlay(&self, ctx: &egui::Context, theme: &Theme) {
        if self.state.toasts.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("neo-toasts"))
            .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -theme.spacing.page))
            .show(ctx, |ui| {
                for toast in &self.state.toasts {
                    theme.card_frame().show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(&toast.text)
                                .size(theme.type_scale.body)
                                .color(theme.palette.text),
                        );
                    });
                    ui.add_space(theme.spacing.xs);
                }
            });
    }
}
