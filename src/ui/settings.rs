//! Settings screen — account info, theme toggle, sign out.

use eframe::egui;
use vidya::{body, card, checkbox, destructive_button, dim_label, button, title_2, Theme};

use crate::state::AppState;
use crate::ui::SettingsAction;

pub fn settings_screen(ui: &mut egui::Ui, th: &Theme, state: &mut AppState) -> SettingsAction {
    let mut action = SettingsAction::None;
    let sp = &th.spacing;

    ui.horizontal(|ui| {
        if button(ui, th, "←").clicked() {
            action = SettingsAction::Back;
        }
        ui.add_space(sp.sm);
        title_2(ui, th, "Settings");
    });
    ui.add_space(sp.lg);

    card(ui, th, |ui| {
        body(ui, th, "Account");
        ui.add_space(sp.xs);
        dim_label(ui, th, state.user_id.as_deref().unwrap_or("—"));
        ui.add_space(sp.sm);
        dim_label(ui, th, state.homeserver.as_deref().unwrap_or("—"));
    });
    ui.add_space(sp.md);

    card(ui, th, |ui| {
        body(ui, th, "Appearance");
        ui.add_space(sp.sm);
        let mut dark = state.dark;
        if checkbox(ui, th, &mut dark, "Dark shell").changed() {
            action = SettingsAction::ToggleTheme;
        }
    });
    ui.add_space(sp.md);

    card(ui, th, |ui| {
        body(ui, th, "Session");
        ui.add_space(sp.sm);
        if destructive_button(ui, th, "Sign out").clicked() {
            action = SettingsAction::Logout;
        }
    });
    ui.add_space(sp.md);

    dim_label(ui, th, "neo — a matrix.org client styled with Vidya, built on sleek's shell.");

    action
}
