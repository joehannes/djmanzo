//! How a platter behaves under the hand.
//!
//! Here rather than in the engine because it is vocabulary: a mapping file, a
//! script, the network API and the interface all name it, and the engine is
//! only one of the things that reads it. See `dj_engine::jog` for what each
//! mode actually does.

use serde::{Deserialize, Serialize};

/// How a platter behaves under the hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum JogMode {
    /// Touching the top stops the record and the wheel drives it directly.
    #[default]
    Vinyl,
    /// Touching does nothing; turning nudges the tempo, like a CD player.
    Cdj,
}

impl JogMode {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            JogMode::Vinyl => "vinyl",
            JogMode::Cdj => "cdj",
        }
    }

    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "vinyl" => Some(JogMode::Vinyl),
            "cdj" => Some(JogMode::Cdj),
            _ => None,
        }
    }
}

impl std::fmt::Display for JogMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::JogMode;

    #[test]
    fn a_mode_round_trips_through_its_name() {
        for mode in [JogMode::Vinyl, JogMode::Cdj] {
            assert_eq!(JogMode::parse(mode.name()), Some(mode));
            assert_eq!(JogMode::parse(&mode.to_string()), Some(mode));
        }
    }

    /// A name nobody meant must not quietly become a mode -- a mapping that
    /// said `jog_mode turntable` would otherwise silently get vinyl.
    #[test]
    fn an_unknown_name_is_refused() {
        assert_eq!(JogMode::parse("turntable"), None);
        assert_eq!(JogMode::parse(""), None);
        assert_eq!(JogMode::parse("VINYL"), None);
    }

    #[test]
    fn vinyl_is_the_default() {
        assert_eq!(JogMode::default(), JogMode::Vinyl);
    }
}
