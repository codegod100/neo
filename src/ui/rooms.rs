//! Room list ("Chats") screen.

use eframe::egui::{self, RichText};
use vidya::{body, button, dim_label, status_dot, text_field_singleline, title, Theme};

use crate::state::{AppState, ConnectionState};
use crate::ui::RoomsAction;

pub fn rooms_screen(ui: &mut egui::Ui, th: &Theme, state: &mut AppState) -> RoomsAction {
    let mut action = RoomsAction::None;
    let sp = &th.spacing;
    let p = &th.palette;

    // Header: title, live dot, settings gear.
    ui.horizontal(|ui| {
        title(ui, th, "Chats");
        ui.add_space(sp.sm);
        let connected = state.connection == ConnectionState::Connected;
        status_dot(ui, th, connected)
            .on_hover_text(state.connection.label());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if button(ui, th, "Settings").clicked() {
                action = RoomsAction::OpenSettings;
            }
        });
    });
    if let Some(user) = &state.user_id {
        dim_label(ui, th, user);
    }
    ui.add_space(sp.sm);

    let spaces: Vec<_> = state.spaces().into_iter().cloned().collect();
    if !spaces.is_empty() {
        egui::ScrollArea::horizontal().id_salt("space-chips").show(ui, |ui| {
            ui.horizontal(|ui| {
                if space_chip(ui, th, "Home", state.active_space.is_none()).clicked() {
                    state.active_space = None;
                }
                for space in &spaces {
                    let selected = state.active_space.as_deref() == Some(space.id.as_str());
                    if space_chip(ui, th, &space.name, selected).clicked() {
                        state.active_space = if selected { None } else { Some(space.id.clone()) };
                    }
                }
            });
        });
        ui.add_space(sp.sm);
    }

    text_field_singleline(ui, th, &mut state.room_filter);
    ui.add_space(sp.md);

    let rooms: Vec<_> = state.filtered_rooms().into_iter().cloned().collect();
    if rooms.is_empty() {
        ui.add_space(sp.lg);
        ui.vertical_centered(|ui| {
            dim_label(
                ui,
                th,
                if state.rooms.is_empty() { "No rooms yet — joined rooms show up here." } else { "No rooms match your search." },
            );
        });
        return action;
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for room in &rooms {
            let resp = ui.add(
                egui::Button::new("")
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::NONE)
                    .min_size(egui::vec2(ui.available_width(), th.spacing.control_height * 1.6)),
            );
            let row_rect = resp.rect;
            if ui.is_rect_visible(row_rect) {
                let mut row_ui = ui.new_child(egui::UiBuilder::new().max_rect(row_rect.shrink2(egui::vec2(0.0, 2.0))));
                row_ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&room.name).size(th.type_scale.body).strong().color(p.text));
                            if room.encrypted {
                                ui.label(RichText::new("🔒").size(th.type_scale.caption).color(p.text_secondary));
                            }
                        });
                        if !room.preview.is_empty() {
                            body(ui, th, &room.preview);
                        }
                    });
                });
            }
            ui.painter().hline(
                row_rect.x_range(),
                row_rect.bottom(),
                egui::Stroke::new(1.0_f32, p.border_soft),
            );
            if resp.clicked() {
                action = RoomsAction::Open(room.id.clone());
            }
            if let Some(cursor) = ui.visuals().interact_cursor {
                resp.on_hover_cursor(cursor);
            }
        }
    });

    action
}

/// A selectable pill for the space filter row (modeled on vidya's
/// `reaction_chip`, which is styled but not click-toggle-aware).
fn space_chip(ui: &mut egui::Ui, th: &Theme, label: &str, selected: bool) -> egui::Response {
    let p = &th.palette;
    let (fill, border, text_color) = if selected {
        (p.accent, egui::Stroke::new(1.0_f32, p.accent), p.accent_fg)
    } else {
        (p.headerbar_bg, egui::Stroke::new(1.0_f32, p.border_soft), p.text)
    };

    let frame = egui::Frame::new()
        .fill(fill)
        .stroke(border)
        .corner_radius(12.0)
        .inner_margin(egui::Margin::symmetric(10, 4));

    let resp = ui
        .scope(|ui| {
            frame
                .show(ui, |ui| {
                    ui.label(RichText::new(label).size(th.type_scale.caption).color(text_color));
                })
                .response
        })
        .inner;

    let resp = resp.interact(egui::Sense::click());
    if let Some(cursor) = ui.visuals().interact_cursor {
        resp.on_hover_cursor(cursor)
    } else {
        resp
    }
}
