//! A binding that switches what the screen is doing, rather than the mix.
//!
//! §109 asks for the activity -- Dig, Mix, Perform, Karaoke -- to be switched
//! from wherever the DJ's hands are, and a controller is where they are. An
//! activity is not an engine action: the engine has no idea what a panel is,
//! and `dj_app::uiop` explains why that knowledge stays out of
//! [`dj_core::Action`]. So a binding may say
//!
//! ```text
//! press = "interface switch activity mix"
//! ```
//!
//! -- `interface ` and then exactly what a leaf of the Space tree runs.
//!
//! # Still nothing a mapping can invent
//!
//! This crate checks the **shape**: one of the five kinds of leaf the tree has
//! (`switch`, `surface`, `lift`, `ui`, `uiop`), something after it, nothing that is
//! not plain text, and no `{value}` -- a fader position is not an activity.
//! It does not know which activities or panels exist; the application checks
//! the **name** against what its own tree offers before anything happens, so
//! a line naming something djmanzo does not have is refused there, logged, and
//! does nothing. A shared mapping still cannot do anything a key under Space
//! cannot.

/// What starts an interface line.
pub const PREFIX: &str = "interface ";

/// The kinds of leaf an interface line may run, as the Space tree names them.
pub const KINDS: [&str; 5] = ["switch", "surface", "lift", "ui", "uiop"];

/// What an interface line runs, or `None` when the line is not one.
#[must_use]
pub fn run(line: &str) -> Option<&str> {
    let run = line.trim().strip_prefix(PREFIX)?.trim();
    let (kind, rest) = run.split_once(' ')?;
    let plain = run.chars().all(|c| !c.is_control()) && !run.contains("{value}");
    (KINDS.contains(&kind) && !rest.trim().is_empty() && plain).then_some(run)
}

/// Whether a line a binding produces is one the loader accepts: an engine
/// action, or an interface line.
///
/// # Errors
/// The parse error, when it is neither.
pub fn check(line: &str) -> Result<(), String> {
    if run(line).is_some() {
        return Ok(());
    }
    if line.trim().starts_with(PREFIX) {
        return Err(format!(
            "an interface line runs one of {} and then what it names",
            KINDS.join(", ")
        ));
    }
    // `{value}` is substituted at dispatch time, so it has to be stood in for
    // to check the rest of the line.
    let probe = line.replace("{value}", "0.5");
    dj_core::Action::parse(&probe)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **An interface line is what the Space tree runs, and only that.** An
    /// activity, a panel, one of the interface's verbs; not a fader position,
    /// not a kind the tree does not have, not an empty one.
    #[test]
    fn an_interface_line_is_what_the_space_tree_runs() {
        assert_eq!(
            run("interface switch activity mix"),
            Some("switch activity mix")
        );
        assert_eq!(run("  interface ui back "), Some("ui back"));
        assert_eq!(run("interface surface library"), Some("surface library"));
        assert_eq!(run("interface uiop show next"), Some("uiop show next"));
        for refused in [
            "interface",
            "interface ui",
            "interface ui ",
            "interface action deck 1 play",
            "interface shell rm -rf /",
            "interface switch activity {value}",
            "interface ui back\u{7}",
            "switch activity mix",
        ] {
            assert_eq!(run(refused), None, "{refused:?}");
        }
    }

    /// The loader's check: engine actions as before, interface lines as
    /// above, and a line that tried to be one and is not says so.
    #[test]
    fn the_loader_takes_actions_and_interface_lines_and_nothing_else() {
        assert!(check("deck 1 play_pause").is_ok());
        assert!(check("deck 1 volume {value}").is_ok());
        assert!(check("interface switch activity karaoke").is_ok());
        assert!(check("deck 1 levitate").is_err());
        let said = check("interface action deck 1 play").unwrap_err();
        assert!(said.contains("switch, surface, lift, ui, uiop"), "{said}");
        assert!(check("interface lift library").is_ok());
    }
}
