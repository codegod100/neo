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

/// A chip's selection state. Kept as its own enum — rather than a bare
/// `bool` with the CSS classes worked out inline wherever it's read — so
/// there's exactly one place (`css_classes`) that decides what a state
/// looks like. `.flat` zeroes out a button's resting-state background in
/// Adwaita, and that rule wins over `.suggested-action`'s background when
/// both classes are present on a button at rest; only the transient
/// hover/press pseudo-classes paint over `.flat` regardless, so a chip
/// combining both classes flashed color on click and faded right back.
/// Routing every transition through here means that combination can only
/// be reintroduced by editing this one function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipState {
    Unselected,
    Selected,
}

impl ChipState {
    fn css_classes(self) -> &'static [&'static str] {
        match self {
            ChipState::Unselected => &["flat"],
            ChipState::Selected => &["suggested-action"],
        }
    }
}

impl From<bool> for ChipState {
    fn from(selected: bool) -> Self {
        if selected { ChipState::Selected } else { ChipState::Unselected }
    }
}

#[derive(Debug)]
pub struct SpaceChip {
    pub id: Option<String>,
    pub label: String,
    /// Raw avatar thumbnail bytes for the space, if it has one set. `None`
    /// for the "Home" chip and for spaces without an avatar.
    pub avatar: Option<Vec<u8>>,
    pub state: ChipState,
}

#[derive(Debug)]
pub enum SpaceChipOutput {
    Select(Option<String>),
}

/// Transitions this chip's `ChipState` without touching anything else about
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
            set_css_classes: self.state.css_classes(),
            set_tooltip_text: Some(&self.label),
            set_halign: gtk::Align::Center,

            adw::Avatar {
                set_size: 48,
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
            SpaceChipInput::SetSelected(selected) => self.state = selected.into(),
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
