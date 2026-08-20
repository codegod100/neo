//! One pill button in the space filter row above the room list. `id: None`
//! is the "Home" chip (every joined room, the pre-Spaces default).

use gtk::prelude::*;
use relm4::factory::{FactoryComponent, FactorySender};
use relm4::gtk;

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
            set_label: &self.label,
            add_css_class: "pill",
            add_css_class: if self.selected { "suggested-action" } else { "flat" },

            connect_clicked[sender, id = self.id.clone()] => move |_| {
                sender.output(SpaceChipOutput::Select(id.clone())).ok();
            },
        }
    }

    fn init_model(init: Self::Init, _index: &relm4::factory::DynamicIndex, _sender: FactorySender<Self>) -> Self {
        init
    }
}
