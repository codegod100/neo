//! One row in a space's Lobby directory (`AppMsg::OpenLobby`) — every room
//! the space advertises, joined or not. A joined row opens like a normal
//! room; an unjoined one offers a Join button instead.

use adw::prelude::*;
use relm4::factory::{FactoryComponent, FactorySender};
use relm4::{adw, gtk};

use crate::state::LobbyRoom;

#[derive(Debug)]
pub struct LobbyRoomRow {
    pub room: LobbyRoom,
}

#[derive(Debug)]
pub enum LobbyRoomRowOutput {
    Open(String),
    Join(String),
}

#[relm4::factory(pub)]
impl FactoryComponent for LobbyRoomRow {
    type Init = LobbyRoom;
    type Input = ();
    type Output = LobbyRoomRowOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        #[root]
        adw::ActionRow {
            set_title: &self.room.name,
            set_subtitle: if self.room.joined { "Joined" } else { "Not joined" },
            set_activatable: self.room.joined,
            set_use_markup: false,
            add_css_class: "neo-lobby-row",

            connect_activated[sender, id = self.room.id.clone()] => move |_| {
                sender.output(LobbyRoomRowOutput::Open(id.clone())).ok();
            },

            add_suffix = &gtk::Button {
                set_label: "Join",
                set_valign: gtk::Align::Center,
                add_css_class: "suggested-action",
                set_visible: !self.room.joined,
                connect_clicked[sender, id = self.room.id.clone()] => move |_| {
                    sender.output(LobbyRoomRowOutput::Join(id.clone())).ok();
                },
            },
        }
    }

    fn init_model(room: Self::Init, _index: &relm4::factory::DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self { room }
    }
}
