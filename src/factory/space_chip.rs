//! One entry in the vertical space rail to the left of the room list. `id:
//! None` is the "Home" entry (every joined room, the pre-Spaces default).
//!
//! This used to be a horizontal row of text pills above the room list, but
//! that forces the window wide enough to fit every space's full name side
//! by side — a non-starter on a phone-width window. A fixed-size avatar
//! rail scrolls vertically instead, so its width never depends on how many
//! spaces there are or how long their names are.

use adw::prelude::*;
use relm4::factory::{FactoryComponent, FactorySender};
use relm4::{adw, gtk};

#[derive(Debug)]
pub struct SpaceChip {
    pub id: Option<String>,
    pub label: String,
    pub selected: bool,
}

#[derive(Debug)]
pub enum SpaceChipOutput {
    Select(Option<String>),
}

#[relm4::factory(pub)]
impl FactoryComponent for SpaceChip {
    type Init = SpaceChip;
    type Input = ();
    type Output = SpaceChipOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::Box;

    view! {
        #[root]
        gtk::Button {
            add_css_class: "flat",
            add_css_class: if self.selected { "suggested-action" } else { "" },
            set_tooltip_text: Some(&self.label),
            set_halign: gtk::Align::Center,

            adw::Avatar {
                set_size: 40,
                set_text: Some(&self.label),
                // "Home" gets a house icon instead of an initial — it isn't
                // a real space, just the unfiltered room list.
                set_show_initials: self.id.is_some(),
                set_icon_name: self.id.is_none().then_some("go-home-symbolic"),
            },

            connect_clicked[sender, id = self.id.clone()] => move |_| {
                sender.output(SpaceChipOutput::Select(id.clone())).ok();
            },
        }
    }

    fn init_model(init: Self::Init, _index: &relm4::factory::DynamicIndex, _sender: FactorySender<Self>) -> Self {
        init
    }
}
