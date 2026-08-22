//! One row in the room timeline: either a chat bubble or a day separator
//! dropped between messages sent on different calendar days.

use adw::prelude::*;
use chrono::Datelike;
use gtk::{gdk, glib};
use relm4::factory::{FactoryComponent, FactorySender};
use relm4::{adw, gtk};

use crate::state::ChatMessage;

/// What a single [`MessageRow`] renders. Kept as one factory component
/// (rather than two factories interleaved) so day separators stay in their
/// natural position in the single scrolling list.
#[derive(Debug, Clone)]
pub enum TimelineRow {
    Message(ChatMessage),
    /// Pre-formatted label, e.g. `"Today"` or `"Wednesday, August 20, 2026"`.
    DaySeparator(String),
}

#[derive(Debug)]
pub struct MessageRow {
    pub item: TimelineRow,
}

#[relm4::factory(pub)]
impl FactoryComponent for MessageRow {
    type Init = TimelineRow;
    type Input = ();
    type Output = ();
    type CommandOutput = ();
    type ParentWidget = gtk::Box;

    view! {
        #[root]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 2,
            set_halign: self.halign(),

            gtk::Box {
                set_visible: self.is_separator(),
                set_halign: gtk::Align::Center,
                set_margin_top: 12,
                set_margin_bottom: 4,

                gtk::Label {
                    set_label: self.separator_label(),
                    add_css_class: "caption-heading",
                    add_css_class: "dim-label",
                },
            },

            gtk::Box {
                set_visible: !self.is_separator(),
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 2,
                set_halign: self.halign(),

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 6,
                    set_halign: self.halign(),

                    adw::Avatar {
                        set_visible: !self.own(),
                        set_size: 20,
                        set_text: Some(self.display_name()),
                        set_show_initials: true,
                        set_custom_image: self.texture().as_ref(),
                    },

                    gtk::Label {
                        set_visible: !self.own(),
                        set_label: self.display_name(),
                        add_css_class: "caption-heading",
                        add_css_class: "accent",
                        set_halign: gtk::Align::Start,
                    },

                    gtk::Label {
                        set_label: &self.timestamp_label(),
                        add_css_class: "caption",
                        add_css_class: "dim-label",
                        set_halign: gtk::Align::Start,
                    },
                },

                gtk::Box {
                    add_css_class: "neo-bubble",
                    add_css_class: if self.own() { "neo-bubble-own" } else { "neo-bubble-other" },
                    set_orientation: gtk::Orientation::Vertical,

                    gtk::Picture {
                        set_visible: self.media_preview().is_some(),
                        set_paintable: self.media_texture().as_ref(),
                        set_content_fit: gtk::ContentFit::Contain,
                        set_can_shrink: true,
                        set_width_request: 320,
                        set_height_request: 240,
                        set_alternative_text: Some(self.body()),
                    },

                    gtk::Label {
                        set_label: &self.body_markup(),
                        set_use_markup: true,
                        set_wrap: true,
                        set_wrap_mode: gtk::pango::WrapMode::WordChar,
                        set_max_width_chars: 42,
                        set_xalign: 0.0,
                        set_selectable: true,
                    },
                },

                gtk::Label {
                    set_visible: self.pending(),
                    set_label: "sending…",
                    add_css_class: "caption",
                    add_css_class: "dim-label",
                    set_halign: self.halign(),
                },
            },
        }
    }

    fn init_model(item: Self::Init, _index: &relm4::factory::DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self { item }
    }
}

impl MessageRow {
    fn message(&self) -> Option<&ChatMessage> {
        match &self.item {
            TimelineRow::Message(msg) => Some(msg),
            TimelineRow::DaySeparator(_) => None,
        }
    }

    fn is_separator(&self) -> bool {
        matches!(self.item, TimelineRow::DaySeparator(_))
    }

    fn separator_label(&self) -> &str {
        match &self.item {
            TimelineRow::DaySeparator(label) => label,
            TimelineRow::Message(_) => "",
        }
    }

    fn own(&self) -> bool {
        self.message().is_some_and(|m| m.own)
    }

    fn pending(&self) -> bool {
        self.message().is_some_and(|m| m.pending)
    }

    fn halign(&self) -> gtk::Align {
        if self.own() { gtk::Align::End } else { gtk::Align::Start }
    }

    fn body(&self) -> &str {
        self.message().map(|m| m.body.as_str()).unwrap_or_default()
    }

    /// The message body as Pango markup, with any `http(s)://` URLs turned
    /// into clickable links. GTK's label widget opens `<a href>` links
    /// itself, so no click handling is needed beyond `set_use_markup`.
    fn body_markup(&self) -> String {
        linkify(self.body())
    }

    /// The sender's room display name, falling back to their raw MXID when
    /// the member event hasn't been seen yet.
    fn display_name(&self) -> &str {
        self.message()
            .map(|m| m.display_name.as_deref().unwrap_or(&m.sender))
            .unwrap_or_default()
    }

    /// Decodes the sender's avatar thumbnail into a paintable, if one was
    /// fetched for this message.
    fn texture(&self) -> Option<gdk::Texture> {
        let bytes = self.message()?.avatar.as_ref()?;
        gdk::Texture::from_bytes(&glib::Bytes::from(bytes)).ok()
    }

    fn media_preview(&self) -> Option<&[u8]> {
        self.message()?.media_preview.as_deref()
    }

    fn media_texture(&self) -> Option<gdk::Texture> {
        let bytes = self.media_preview()?;
        gdk::Texture::from_bytes(&glib::Bytes::from(bytes)).ok()
    }

    fn timestamp_label(&self) -> String {
        self.message().map(|m| format_timestamp(m.ts_millis)).unwrap_or_default()
    }
}

/// Escapes `text` as Pango markup, wrapping any `http://` or `https://` URL
/// in an `<a href>` tag so GTK renders it as a clickable link. URLs are
/// detected by scanning for the scheme and extending to the next
/// whitespace, then trimming trailing punctuation (e.g. a sentence-ending
/// period or a closing paren with no matching open paren) that's unlikely
/// to be part of the URL itself.
fn linkify(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;

    loop {
        let start = ["http://", "https://"]
            .iter()
            .filter_map(|scheme| rest.find(scheme))
            .min();

        let Some(idx) = start else {
            out.push_str(&glib::markup_escape_text(rest));
            break;
        };

        out.push_str(&glib::markup_escape_text(&rest[..idx]));

        let candidate = &rest[idx..];
        let end = candidate.find(char::is_whitespace).unwrap_or(candidate.len());
        let mut url = &candidate[..end];
        while let Some(last) = url.chars().last() {
            let trim = match last {
                '.' | ',' | '!' | '?' | ':' | ';' | '\'' | '"' => true,
                ')' => url.matches('(').count() <= url.matches(')').count() - 1,
                _ => false,
            };
            if !trim {
                break;
            }
            url = &url[..url.len() - last.len_utf8()];
        }

        let escaped = glib::markup_escape_text(url);
        out.push_str(&format!(r#"<a href="{escaped}">{escaped}</a>"#));
        rest = &candidate[url.len()..];
    }

    out
}

/// Render a `origin_server_ts` (millis since the Unix epoch) as a short
/// local time, e.g. `"2:32 PM"`.
fn format_timestamp(ts_millis: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ts_millis)
        .map(|dt| dt.with_timezone(&chrono::Local).format("%-I:%M %p").to_string())
        .unwrap_or_default()
}

/// A message timestamp's local calendar day, used to detect when a day
/// separator needs to be inserted between two consecutive messages.
pub fn local_day(ts_millis: i64) -> Option<chrono::NaiveDate> {
    chrono::DateTime::from_timestamp_millis(ts_millis).map(|dt| dt.with_timezone(&chrono::Local).date_naive())
}

/// Render a local calendar day as a separator label: `"Today"`,
/// `"Yesterday"`, or e.g. `"Wednesday, August 20"` (with the year appended
/// when it isn't the current one).
pub fn format_day_label(day: chrono::NaiveDate) -> String {
    let today = chrono::Local::now().date_naive();
    match (today - day).num_days() {
        0 => "Today".to_owned(),
        1 => "Yesterday".to_owned(),
        _ if day.year() == today.year() => day.format("%A, %B %-d").to_string(),
        _ => day.format("%A, %B %-d, %Y").to_string(),
    }
}
