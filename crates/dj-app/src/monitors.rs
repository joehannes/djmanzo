//! Panels on other screens.
//!
//! # Why a second window rather than a second layout
//!
//! A DJ with two screens does not want a different arrangement on each — they
//! want *the same interface, spread out*. The browser is the obvious case: a
//! song list is a list, and a list wants a whole screen while the decks keep
//! the other. So a panel is not moved by changing a layout; it is taken out of
//! the main window and given one of its own, which the operating system's own
//! window management then puts wherever the DJ drags it.
//!
//! That also means djmanzo does not need to know anything about monitors. It
//! never asks how many there are, never positions a window on one, never has to
//! cope with a screen being unplugged mid-set. It opens a window; the desktop
//! decides where windows go. Every attempt to be cleverer than that ends with an
//! application that puts a panel on a projector.
//!
//! # One process, one state
//!
//! A detached window is the same application: same `AppState`, same action bus,
//! same snapshot. Tauri's `emit` reaches every window, so a detached waveform
//! is drawn from exactly the same sixty-times-a-second snapshot the main window
//! draws from, and there is no second path to keep in step.
//!
//! # What is remembered
//!
//! Which panels were detached, and nothing else. Where a window *was* is the
//! desktop's business — a saved position is wrong the moment a screen is
//! unplugged, and restoring one onto a monitor that is no longer there is how
//! a panel ends up invisible with no way to get it back.

use serde::{Deserialize, Serialize};

/// A panel that can be given a window of its own.
///
/// A closed set rather than a free string: the detached window is opened with
/// the panel's name in its URL, and the interface has to know what to render.
/// An unknown name would open a blank window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Panel {
    /// The song list. The one everybody detaches.
    Browser,
    /// The scrolling waveform lane, on its own and large.
    Waveforms,
    /// The effect racks.
    Fx,
    /// The sampler's slots.
    Sampler,
    /// The assistant.
    Assistant,
    /// The living interface's visual, for a screen facing the room.
    Watershed,
}

impl Panel {
    pub const ALL: [Panel; 6] = [
        Panel::Browser,
        Panel::Waveforms,
        Panel::Fx,
        Panel::Sampler,
        Panel::Assistant,
        Panel::Watershed,
    ];

    /// The name used in the window label and in the URL.
    ///
    /// Lower case and hyphenated, because a Tauri window label may only contain
    /// alphanumerics, `-`, `/`, `:` and `_` — a label with a space in it is
    /// refused at runtime, which is a fault that only appears when somebody
    /// clicks the button.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Panel::Browser => "browser",
            Panel::Waveforms => "waveforms",
            Panel::Fx => "fx",
            Panel::Sampler => "sampler",
            Panel::Assistant => "assistant",
            Panel::Watershed => "watershed",
        }
    }

    /// What the window is called, in its title bar and in the desktop's window
    /// list — which is how a DJ finds it again after minimising it.
    ///
    /// ASCII only, and a plain hyphen rather than an em dash. A window title
    /// crosses into X11's `WM_NAME`, and a desktop running in a non-UTF-8
    /// locale converts it down — a title with an em dash in it came back as
    /// "(failure in conversion from UTF8_STRING to ANSI_X3.4-1968)" on the
    /// first run of this. A typographically nicer dash is not worth a taskbar
    /// entry a DJ cannot read.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Panel::Browser => "djmanzo - Browser",
            Panel::Waveforms => "djmanzo - Waveforms",
            Panel::Fx => "djmanzo - Effects",
            Panel::Sampler => "djmanzo - Sampler",
            Panel::Assistant => "djmanzo - Assistant",
            Panel::Watershed => "djmanzo - Watershed",
        }
    }

    /// How big to open it.
    ///
    /// Shapes rather than sizes: a browser is a tall list, a waveform lane is a
    /// wide strip, a watershed fills whatever it is given. Opening every panel
    /// as the same rectangle would mean dragging each one into shape once.
    #[must_use]
    pub fn size(self) -> (f64, f64) {
        match self {
            Panel::Browser => (900.0, 900.0),
            Panel::Waveforms => (1400.0, 420.0),
            Panel::Fx => (520.0, 720.0),
            Panel::Sampler => (620.0, 520.0),
            Panel::Assistant => (480.0, 760.0),
            Panel::Watershed => (1280.0, 720.0),
        }
    }

    #[must_use]
    pub fn parse(slug: &str) -> Option<Panel> {
        Panel::ALL.into_iter().find(|panel| panel.slug() == slug)
    }

    /// The window label Tauri knows it by.
    #[must_use]
    pub fn label(self) -> String {
        format!("panel-{}", self.slug())
    }
}

/// Which panels are on screens of their own.
///
/// Deliberately a set of *what*, never a record of *where*. See the module
/// note: a saved position is wrong the moment a screen is unplugged.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Detached {
    panels: Vec<Panel>,
}

impl Detached {
    #[must_use]
    pub fn contains(&self, panel: Panel) -> bool {
        self.panels.contains(&panel)
    }

    /// Note that a panel now has its own window. Idempotent.
    pub fn add(&mut self, panel: Panel) {
        if !self.contains(panel) {
            self.panels.push(panel);
        }
    }

    pub fn remove(&mut self, panel: Panel) {
        self.panels.retain(|p| *p != panel);
    }

    #[must_use]
    pub fn panels(&self) -> &[Panel] {
        &self.panels
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.panels.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Tauri window label may only contain alphanumerics, `-`, `/`, `:` and
    /// `_`. A label with anything else in it is refused at runtime — a fault
    /// that appears only when somebody clicks the button, which is exactly the
    /// kind that ships.
    #[test]
    fn every_label_is_one_tauri_will_accept() {
        for panel in Panel::ALL {
            let label = panel.label();
            assert!(
                label
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-/:_".contains(c)),
                "`{label}` is not a valid window label"
            );
        }
    }

    /// The slug goes into a URL and comes back out. A panel whose name did not
    /// survive that would open a blank window.
    #[test]
    fn every_slug_round_trips() {
        for panel in Panel::ALL {
            assert_eq!(Panel::parse(panel.slug()), Some(panel));
        }
        assert_eq!(Panel::parse("nonsense"), None);
        assert_eq!(Panel::parse(""), None);
    }

    /// Two panels sharing a slug would share a window: detaching one would
    /// silently reuse the other's, and closing it would take both.
    #[test]
    fn no_two_panels_share_a_name() {
        let mut slugs: Vec<&str> = Panel::ALL.iter().map(|p| p.slug()).collect();
        slugs.sort_unstable();
        let count = slugs.len();
        slugs.dedup();
        assert_eq!(slugs.len(), count, "two panels share a slug");
    }

    /// A window title crosses into X11's `WM_NAME`, and a desktop in a
    /// non-UTF-8 locale converts it down. An em dash in the title came back as
    /// a conversion-failure string on the first run of this, which is what a
    /// DJ would then see in their taskbar.
    #[test]
    fn every_title_survives_a_narrow_locale() {
        for panel in Panel::ALL {
            let title = panel.title();
            assert!(
                title.is_ascii(),
                "`{title}` is not ASCII and may not survive WM_NAME"
            );
        }
    }

    #[test]
    fn every_panel_has_a_usable_size() {
        for panel in Panel::ALL {
            let (width, height) = panel.size();
            assert!(width >= 320.0 && height >= 240.0, "{panel:?} is too small");
        }
    }

    #[test]
    fn detaching_is_idempotent() {
        let mut detached = Detached::default();
        assert!(detached.is_empty());
        detached.add(Panel::Browser);
        detached.add(Panel::Browser);
        assert_eq!(detached.panels(), &[Panel::Browser]);
        assert!(detached.contains(Panel::Browser));
        assert!(!detached.contains(Panel::Fx));
    }

    #[test]
    fn attaching_a_panel_that_was_never_detached_is_harmless() {
        let mut detached = Detached::default();
        detached.remove(Panel::Fx);
        assert!(detached.is_empty());
    }

    #[test]
    fn a_detached_set_survives_being_written_down() {
        let mut detached = Detached::default();
        detached.add(Panel::Browser);
        detached.add(Panel::Watershed);
        let text = serde_json::to_string(&detached).unwrap();
        assert_eq!(
            serde_json::from_str::<Detached>(&text).unwrap(),
            detached,
            "the set did not survive: {text}"
        );
        // Kebab-case in the file, so a DJ editing it by hand sees the same
        // names the interface uses.
        assert!(text.contains("browser"), "{text}");
    }
}
