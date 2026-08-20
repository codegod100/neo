//! Single room ("Chat") screen — message stream + compose bar.

use eframe::egui::{self, Align, Key, Layout, RichText, ScrollArea};
use vidya::{button, dim_label, primary_button, title_2, Theme};

use crate::state::AppState;
use crate::ui::RoomAction;

pub fn room_screen(ui: &mut egui::Ui, th: &Theme, state: &mut AppState) -> RoomAction {
    let mut action = RoomAction::None;
    let sp = &th.spacing;
    let p = &th.palette;

    let Some(room) = state.active_room_summary().cloned() else {
        dim_label(ui, th, "No room selected.");
        if button(ui, th, "Back").clicked() {
            action = RoomAction::Back;
        }
        return action;
    };

    ui.horizontal(|ui| {
        if button(ui, th, "←").clicked() {
            action = RoomAction::Back;
        }
        ui.add_space(sp.sm);
        title_2(ui, th, &room.name);
    });
    ui.add_space(sp.sm);
    ui.separator();

    let messages = state.messages.get(&room.id).cloned().unwrap_or_default();

    let compose_height = th.spacing.control_height + sp.page * 2.0;
    let history_height = (ui.available_height() - compose_height).max(80.0);

    ScrollArea::vertical().stick_to_bottom(true).max_height(history_height).show(ui, |ui| {
        if messages.is_empty() {
            ui.add_space(sp.lg);
            dim_label(ui, th, "No messages yet — say hello.");
        } else {
            if button(ui, th, "Load older").clicked() && !state.loading_older {
                action = RoomAction::LoadOlder;
            }
            ui.add_space(sp.sm);
            for msg in &messages {
                ui.horizontal(|ui| {
                    ui.set_width(ui.available_width());
                    let align = if msg.own { Align::Max } else { Align::Min };
                    ui.with_layout(Layout::top_down(align), |ui| {
                        ui.horizontal(|ui| {
                            if !msg.own {
                                ui.label(
                                    RichText::new(&msg.sender)
                                        .size(th.type_scale.caption)
                                        .strong()
                                        .color(p.accent),
                                );
                            }
                            if msg.pending {
                                ui.label(RichText::new("sending…").size(th.type_scale.caption).color(p.text_secondary));
                            }
                        });
                        let bubble_fill = if msg.own { p.accent } else { p.card_bg };
                        let text_color = if msg.own { p.accent_fg } else { p.text };
                        egui::Frame::new()
                            .fill(bubble_fill)
                            .corner_radius(th.spacing.radius_md)
                            .inner_margin(egui::Margin::symmetric(sp.md as i8, sp.sm as i8))
                            .show(ui, |ui| {
                                ui.set_max_width(ui.available_width().min(480.0));
                                ui.label(RichText::new(&msg.body).size(th.type_scale.body).color(text_color));
                            });
                    });
                });
                ui.add_space(sp.xs);
            }
        }
    });

    ui.add_space(sp.sm);
    ui.separator();
    ui.add_space(sp.sm);

    ui.horizontal(|ui| {
        let field_width = ui.available_width() - th.spacing.control_height * 2.0 - sp.sm;
        let resp = ui.add(
            egui::TextEdit::singleline(&mut state.compose)
                .hint_text("Message…")
                .margin(th.text_edit_margin())
                .desired_width(field_width.max(80.0)),
        );
        let enter_pressed = resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));

        let text = state.compose.trim().to_owned();
        let can_send = !text.is_empty();
        let send_clicked = primary_button(ui, th, "Send").clicked();
        if (send_clicked || enter_pressed) && can_send {
            action = RoomAction::Send(text);
            state.compose.clear();
        }
    });

    action
}
