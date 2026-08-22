//! Typed access to the application UI compiled from GTK Blueprint.

use adw::prelude::*;
use gtk::glib::{self, prelude::IsA, types::StaticType};
use relm4::{adw, gtk};

fn object<T>(builder: &gtk::Builder, id: &str) -> T
where
    T: IsA<glib::Object> + StaticType,
{
    builder
        .object(id)
        .unwrap_or_else(|| panic!("Blueprint object `{id}` is missing"))
}

#[derive(Clone)]
pub struct AppUi {
    pub root: gtk::Box,
    pub window_title: adw::WindowTitle,
    pub space_rail: gtk::ScrolledWindow,
    pub screen_stack: gtk::Stack,
    pub sso_waiting: gtk::Box,
    pub sso_url: gtk::Label,
    pub login_form: gtk::Box,
    pub login_error: gtk::Label,
    pub password_entry: gtk::Entry,
    pub remember_switch: adw::SwitchRow,
    pub login_button: gtk::Button,
    pub sso_button: gtk::Button,
    pub reopen_sso: gtk::Button,
    pub cancel_sso: gtk::Button,
    pub rooms_user: gtk::Label,
    pub settings_button: gtk::Button,
    pub room_filter: gtk::Entry,
    pub lobby_button: gtk::Button,
    pub rooms_empty: gtk::Label,
    pub lobby_back: gtk::Button,
    pub lobby_stack: gtk::Stack,
    pub lobby_empty: gtk::Label,
    pub room_back: gtk::Button,
    pub room_name: gtk::Label,
    pub room_address: gtk::Label,
    pub message_stack: gtk::Stack,
    pub message_scroller: gtk::ScrolledWindow,
    pub compose_entry: gtk::Entry,
    pub send_button: gtk::Button,
    pub settings_back: gtk::Button,
    pub settings_user: adw::ActionRow,
    pub settings_homeserver: adw::ActionRow,
    pub dark_switch: adw::SwitchRow,
    pub logout_button: gtk::Button,
}

impl AppUi {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        homeserver_buf: &gtk::EntryBuffer,
        username_buf: &gtk::EntryBuffer,
        password_buf: &gtk::EntryBuffer,
        room_filter_buf: &gtk::EntryBuffer,
        compose_buf: &gtk::EntryBuffer,
        space_widget: &gtk::Box,
        room_widget: &gtk::ListBox,
        lobby_widget: &gtk::ListBox,
        message_widget: &gtk::Box,
    ) -> Self {
        let builder = gtk::Builder::from_string(include_str!("ui/app.ui"));

        let homeserver_entry: gtk::Entry = object(&builder, "homeserver_entry");
        let username_entry: gtk::Entry = object(&builder, "username_entry");
        let password_entry: gtk::Entry = object(&builder, "password_entry");
        let room_filter: gtk::Entry = object(&builder, "room_filter");
        let compose_entry: gtk::Entry = object(&builder, "compose_entry");
        homeserver_entry.set_buffer(homeserver_buf);
        username_entry.set_buffer(username_buf);
        password_entry.set_buffer(password_buf);
        room_filter.set_buffer(room_filter_buf);
        compose_entry.set_buffer(compose_buf);

        let space_mount: gtk::Box = object(&builder, "space_mount");
        let room_mount: gtk::Box = object(&builder, "room_mount");
        let lobby_mount: gtk::Box = object(&builder, "lobby_mount");
        let message_mount: gtk::Box = object(&builder, "message_mount");
        space_widget.set_orientation(gtk::Orientation::Vertical);
        space_widget.set_spacing(6);
        space_widget.set_valign(gtk::Align::Start);
        space_mount.append(space_widget);
        room_mount.append(room_widget);
        lobby_mount.append(lobby_widget);
        message_mount.append(message_widget);

        room_widget.add_css_class("boxed-list");
        room_widget.set_selection_mode(gtk::SelectionMode::None);
        room_widget.set_valign(gtk::Align::Start);
        lobby_widget.add_css_class("boxed-list");
        lobby_widget.set_selection_mode(gtk::SelectionMode::None);
        lobby_widget.set_valign(gtk::Align::Start);
        message_widget.set_orientation(gtk::Orientation::Vertical);
        message_widget.set_spacing(6);
        message_widget.set_valign(gtk::Align::End);
        message_widget.set_margin_top(8);
        message_widget.set_margin_bottom(8);

        Self {
            root: object(&builder, "app_content"),
            window_title: object(&builder, "window_title"),
            space_rail: object(&builder, "space_rail"),
            screen_stack: object(&builder, "screen_stack"),
            sso_waiting: object(&builder, "sso_waiting"),
            sso_url: object(&builder, "sso_url"),
            login_form: object(&builder, "login_form"),
            login_error: object(&builder, "login_error"),
            password_entry,
            remember_switch: object(&builder, "remember_switch"),
            login_button: object(&builder, "login_button"),
            sso_button: object(&builder, "sso_button"),
            reopen_sso: object(&builder, "reopen_sso"),
            cancel_sso: object(&builder, "cancel_sso"),
            rooms_user: object(&builder, "rooms_user"),
            settings_button: object(&builder, "settings_button"),
            room_filter,
            lobby_button: object(&builder, "lobby_button"),
            rooms_empty: object(&builder, "rooms_empty"),
            lobby_back: object(&builder, "lobby_back"),
            lobby_stack: object(&builder, "lobby_stack"),
            lobby_empty: object(&builder, "lobby_empty"),
            room_back: object(&builder, "room_back"),
            room_name: object(&builder, "room_name"),
            room_address: object(&builder, "room_address"),
            message_stack: object(&builder, "message_stack"),
            message_scroller: object(&builder, "message_scroller"),
            compose_entry,
            send_button: object(&builder, "send_button"),
            settings_back: object(&builder, "settings_back"),
            settings_user: object(&builder, "settings_user"),
            settings_homeserver: object(&builder, "settings_homeserver"),
            dark_switch: object(&builder, "dark_switch"),
            logout_button: object(&builder, "logout_button"),
        }
    }
}
