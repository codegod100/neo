//! One chat bubble in the room timeline.

use gtk::prelude::*;
use relm4::factory::{FactoryComponent, FactorySender};
use relm4::gtk;

use crate::state::ChatMessage;

#[derive(Debug)]
pub struct MessageRow {
    pub msg: ChatMessage,
}

#[relm4::factory(pub)]
impl FactoryComponent for MessageRow {
    type Init = ChatMessage;
    type Input = ();
    type Output = ();
    type CommandOutput = ();
    type ParentWidget = gtk::Box;

    view! {
        #[root]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 2,
            set_halign: if self.msg.own { gtk::Align::End } else { gtk::Align::Start },

            gtk::Label {
                set_visible: !self.msg.own,
                set_label: &self.msg.sender,
                add_css_class: "caption-heading",
                add_css_class: "accent",
                set_halign: gtk::Align::Start,
            },

            gtk::Box {
                add_css_class: "neo-bubble",
                add_css_class: if self.msg.own { "neo-bubble-own" } else { "neo-bubble-other" },
                set_orientation: gtk::Orientation::Vertical,

                gtk::Label {
                    set_label: &self.msg.body,
                    set_wrap: true,
                    set_wrap_mode: gtk::pango::WrapMode::WordChar,
                    set_max_width_chars: 42,
                    set_xalign: 0.0,
                    set_selectable: true,
                },
            },

            gtk::Label {
                set_visible: self.msg.pending,
                set_label: "sending…",
                add_css_class: "caption",
                add_css_class: "dim-label",
                set_halign: if self.msg.own { gtk::Align::End } else { gtk::Align::Start },
            },
        }
    }

    fn init_model(msg: Self::Init, _index: &relm4::factory::DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self { msg }
    }
}
