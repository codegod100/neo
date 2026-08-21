//! One entry in the vertical space rail to the left of the room list. `id:
//! None` is the "Home" entry (every joined room, the pre-Spaces default).
//!
//! This used to be a horizontal row of text pills above the room list, but
//! that forces the window wide enough to fit every space's full name side
//! by side — a non-starter on a phone-width window. A fixed-size avatar
//! rail scrolls vertically instead, so its width never depends on how many
//! spaces there are or how long their names are.

use adw::prelude::*;
use gtk::{gdk, glib};
use relm4::factory::{FactoryComponent, FactorySender};
use relm4::{adw, gtk};

#[derive(Debug)]
pub struct SpaceChip {
    pub id: Option<String>,
    pub label: String,
    /// Raw avatar thumbnail bytes for the space, if it has one set. `None`
    /// for the "Home" chip and for spaces without an avatar.
    pub avatar: Option<Vec<u8>>,
    pub selected: bool,
}

#[derive(Debug)]
pub enum SpaceChipOutput {
    Select(Option<String>),
}

/// Toggles which chip is highlighted without touching anything else about
/// it. Sent in place of rebuilding the chip list so the GTK button you just
/// clicked keeps keyboard focus instead of being destroyed and recreated
/// (see `AppModel::sync_space_chips`).
#[derive(Debug)]
pub enum SpaceChipInput {
    SetSelected(bool),
}

#[relm4::factory(pub)]
impl FactoryComponent for SpaceChip {
    type Init = SpaceChip;
    type Input = SpaceChipInput;
    type Output = SpaceChipOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::Box;

    view! {
        #[root]
        gtk::Button {
            #[watch]
            set_css_classes: if self.selected {
                &["flat", "suggested-action"]
            } else {
                &["flat"]
            },
            set_tooltip_text: Some(&self.label),
            set_halign: gtk::Align::Center,

            adw::Avatar {
                set_size: 40,
                set_text: Some(&self.label),
                // "Home" gets a house icon instead of initials — it isn't a
                // real space, just the unfiltered room list. A decoded
                // avatar thumbnail wins over both.
                set_show_initials: self.id.is_some(),
                set_icon_name: self.id.is_none().then_some("go-home-symbolic"),
                set_custom_image: self.texture().as_ref(),
            },

            connect_clicked[sender, id = self.id.clone()] => move |_| {
                sender.output(SpaceChipOutput::Select(id.clone())).ok();
            },
        }
    }

    fn init_model(init: Self::Init, _index: &relm4::factory::DynamicIndex, _sender: FactorySender<Self>) -> Self {
        init
    }

    fn update(&mut self, msg: Self::Input, _sender: FactorySender<Self>) {
        match msg {
            SpaceChipInput::SetSelected(selected) => self.selected = selected,
        }
    }
}

impl SpaceChip {
    /// Decodes the space's avatar thumbnail into a paintable, if it has one.
    fn texture(&self) -> Option<gdk::Texture> {
        let bytes = self.avatar.as_ref()?;
        gdk::Texture::from_bytes(&glib::Bytes::from(bytes)).ok()
    }
}
