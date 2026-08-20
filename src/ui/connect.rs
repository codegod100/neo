//! Connect screen — homeserver + username/password login, freeq-sleek
//! `ui/connect.rs` inspired but for a matrix.org account instead of guest IRC.

use eframe::egui::{self, Key, RichText};
use vidya::{
    body, button, card, checkbox, dim_label, primary_button, text_field_singleline, title,
    title_2, Theme,
};

use crate::state::{AppState, ConnectionState};
use crate::ui::ConnectAction;

pub fn connect_screen(ui: &mut egui::Ui, th: &Theme, state: &mut AppState) -> ConnectAction {
    let mut action = ConnectAction::None;
    let sp = &th.spacing;
    let p = &th.palette;
    let loading = state.connection == ConnectionState::Connecting;

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(sp.xl);
        ui.vertical_centered(|ui| {
            title(ui, th, "neo");
            ui.add_space(sp.xs);
            dim_label(ui, th, "A matrix.org client, dressed in Vidya.");
        });
        ui.add_space(sp.lg);

        if let Some(url) = state.sso_url.clone() {
            card(ui, th, |ui| {
                sso_waiting_panel(ui, th, &url, &mut action);
            });
            return;
        }

        card(ui, th, |ui| {
            title_2(ui, th, "Sign in");
            ui.add_space(sp.sm);
            dim_label(ui, th, "Your homeserver account — the same one Element uses.");
            ui.add_space(sp.lg);

            body(ui, th, "Homeserver");
            ui.add_space(sp.xs);
            text_field_singleline(ui, th, &mut state.form_homeserver);
            ui.add_space(sp.md);

            body(ui, th, "Username");
            ui.add_space(sp.xs);
            text_field_singleline(ui, th, &mut state.form_username);
            ui.add_space(sp.md);

            body(ui, th, "Password");
            ui.add_space(sp.xs);
            let pw_resp = ui.add(
                egui::TextEdit::singleline(&mut state.form_password)
                    .password(true)
                    .margin(th.text_edit_margin())
                    .desired_width(ui.available_width()),
            );
            ui.add_space(sp.md);

            checkbox(ui, th, &mut state.remember_me, "Remember me on this device");
            ui.add_space(sp.md);

            if let Some(err) = &state.error {
                ui.label(RichText::new(err).size(th.type_scale.caption).color(p.destructive));
                ui.add_space(sp.sm);
            }

            let can_submit = !loading
                && !state.form_homeserver.trim().is_empty()
                && !state.form_username.trim().is_empty()
                && !state.form_password.is_empty();

            let enter_pressed = pw_resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));

            ui.horizontal(|ui| {
                ui.set_width(ui.available_width());
                let label = if loading { "Signing in…" } else { "Sign in" };
                let resp = primary_button(ui, th, label);
                if (resp.clicked() || enter_pressed) && can_submit {
                    action = ConnectAction::Login;
                }
            });

            ui.add_space(sp.sm);
            dim_label(
                ui,
                th,
                "Login only — registration isn't supported yet. Passwords go straight to your \
                 homeserver over HTTPS and are never stored by neo unless you remember this device.",
            );
        });

        ui.add_space(sp.lg);
        ui.vertical_centered(|ui| dim_label(ui, th, "— or —"));
        ui.add_space(sp.lg);

        card(ui, th, |ui| {
            title_2(ui, th, "Single sign-on");
            ui.add_space(sp.sm);
            dim_label(
                ui,
                th,
                "For homeservers that sign you in through a browser (Mozilla accounts, GitHub, \
                 your workplace IdP, …) instead of a password. Uses the homeserver above.",
            );
            ui.add_space(sp.lg);

            ui.horizontal(|ui| {
                ui.set_width(ui.available_width());
                let sso_label = if loading { "Signing in…" } else { "Sign in with SSO" };
                let can_sso = !loading && !state.form_homeserver.trim().is_empty();
                ui.add_enabled_ui(can_sso, |ui| {
                    if primary_button(ui, th, sso_label).clicked() {
                        action = ConnectAction::Sso;
                    }
                });
            });
        });
    });

    action
}

/// Shown in place of the login form while an SSO login is in flight: a
/// fallback link (auto-open can fail — sandboxed environments, no default
/// browser configured, …) and a way to bail out back to the form.
fn sso_waiting_panel(ui: &mut egui::Ui, th: &Theme, url: &str, action: &mut ConnectAction) {
    let sp = &th.spacing;

    title_2(ui, th, "Continue in your browser");
    ui.add_space(sp.sm);
    dim_label(ui, th, "We opened your homeserver's sign-in page. Waiting for it to finish…");
    ui.add_space(sp.md);

    ui.horizontal_wrapped(|ui| {
        ui.spinner();
        ui.add_space(sp.xs);
        body(ui, th, "Waiting for sign-in to complete");
    });
    ui.add_space(sp.md);

    ui.add(
        egui::Label::new(RichText::new(url).size(th.type_scale.caption).color(th.palette.text_secondary))
            .selectable(true)
            .wrap(),
    );
    ui.add_space(sp.sm);

    ui.horizontal(|ui| {
        if button(ui, th, "Open again").clicked() {
            let _ = open::that_detached(url);
        }
        if button(ui, th, "Cancel").clicked() {
            *action = ConnectAction::CancelSso;
        }
    });
}
