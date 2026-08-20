//! One row in the room list — a `libadwaita` `ActionRow` showing the room
//! name, an optional encrypted badge, and the last-known preview text.

use adw::prelude::*;
use relm4::factory::{FactoryComponent, FactorySender};
use relm4::{adw, gtk};

use crate::state::RoomSummary;

#[derive(Debug)]
pub struct RoomRow {
    pub room: RoomSummary,
}

#[derive(Debug)]
pub enum RoomRowOutput {
    Open(String),
}

#[relm4::factory(pub)]
impl FactoryComponent for RoomRow {
    type Init = RoomSummary;
    type Input = ();
    type Output = RoomRowOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        #[root]
        adw::ActionRow {
            set_title: &self.room.name,
            set_subtitle: &self.room.preview,
            set_activatable: true,
            set_use_markup: false,
            add_css_class: "neo-room-row",

            connect_activated[sender, id = self.room.id.clone()] => move |_| {
                sender.output(RoomRowOutput::Open(id.clone())).ok();
            },

            add_suffix = &gtk::Image {
                set_icon_name: Some("channel-secure-symbolic"),
                set_visible: self.room.encrypted,
                set_tooltip_text: Some("Encrypted"),
            },
        }
    }

    fn init_model(room: Self::Init, _index: &relm4::factory::DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self { room }
    }
}
