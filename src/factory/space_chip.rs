//! One pill button in the space filter row above the room list. `id: None`
//! is the "Home" chip (every joined room, the pre-Spaces default).

use gtk::prelude::*;
use gtk::{gdk, glib};
use relm4::factory::{FactoryComponent, FactorySender};
use relm4::gtk;

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
            add_css_class: "pill",
            add_css_class: if self.selected { "suggested-action" } else { "flat" },
            set_tooltip_text: Some(&self.label),

            connect_clicked[sender, id = self.id.clone()] => move |_| {
                sender.output(SpaceChipOutput::Select(id.clone())).ok();
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 6,

                gtk::Image {
                    set_visible: self.texture().is_some(),
                    set_pixel_size: 20,
                    add_css_class: "neo-space-avatar",
                    set_paintable: self.texture().as_ref(),
                },

                gtk::Label {
                    set_label: &self.label,
                },
            },
        }
    }

    fn init_model(init: Self::Init, _index: &relm4::factory::DynamicIndex, _sender: FactorySender<Self>) -> Self {
        init
    }
}

impl SpaceChip {
    /// Decodes the space's avatar thumbnail into a paintable, if it has one.
    fn texture(&self) -> Option<gdk::Texture> {
        let bytes = self.avatar.as_ref()?;
        gdk::Texture::from_bytes(&glib::Bytes::from(bytes)).ok()
    }
}
