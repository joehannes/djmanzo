//! Lua, for mappings that need logic rather than a table.
//!
//! # The promise this must not break
//!
//! djmanzo's mappings are safe to accept from a stranger, and the reason is
//! stated plainly in [`crate::mapping`]: **a mapping cannot do anything the
//! interface cannot.** Every action in a TOML file is parsed through
//! `Action::parse` when the file loads, so the worst a malicious mapping can do
//! is play the wrong track.
//!
//! A scripting language is exactly how that promise gets lost. So:
//!
//! - **Nothing that reaches outside the process.** `io`, `os`, `package`,
//!   `require` and `debug` are never loaded, and `dofile`, `loadfile` and
//!   `load` are removed. A mapping that could open a file, spawn a process or
//!   load native code is a mapping nobody should download.
//!
//!   Asking for [`StdLib::NONE`] is **not enough**, which is worth knowing:
//!   mlua installs the base library regardless, so `dofile` and `loadfile`
//!   were reachable from a script until they were explicitly taken away. A
//!   test enumerates them by name rather than trusting the flag.
//! - **Actions still go through the parser.** A script returns *text*, and that
//!   text is `Action::parse`d exactly as a TOML binding's is. There is no path
//!   from Lua to the engine that skips the vocabulary.
//! - **Bounded execution.** A script runs on the MIDI thread, where a `while
//!   true do end` would take the controller down with it. Lua's instruction
//!   hook stops it after [`STEP_LIMIT`] steps.
//!
//! # What a script is for
//!
//! The things a table cannot express. A shift button that changes what eight
//! pads do; a jog wheel whose sensitivity depends on whether the deck is
//! playing; a single knob that sweeps a filter one way and an echo the other.
//! Each of those is a *decision*, and a decision needs an `if`.
//!
//! # What a script is handed
//!
//! One control event, and the ability to read any parameter by the same stable
//! name the interface and the network API use. It returns nothing, one action,
//! or several.

use dj_control::ParameterRegistry;
use dj_core::ParamId;
use mlua::{Lua, StdLib, Value};
use std::sync::Arc;

/// How many Lua instructions one control event may take.
///
/// A hundred thousand is far more than any real mapping needs — the examples
/// in the documentation are a dozen lines — and small enough that a runaway
/// script costs a fraction of a millisecond rather than the controller.
const STEP_LIMIT: u32 = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScriptError {
    #[error("the script could not be loaded: {0}")]
    Broken(String),
    #[error("the script has no `on_control` function to call")]
    NoEntryPoint,
    #[error("the script failed while handling {0}: {1}")]
    Failed(String, String),
    #[error("the script ran too long and was stopped")]
    TooLong,
    #[error("{0:?} is not something djmanzo can do: {1}")]
    BadAction(String, String),
}

/// What happened to a control, as a script sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Press,
    Release,
    /// A fader, knob or wheel moved. The value is 0..=1 as the binding scaled
    /// it, so a script sees what a `move` action would have received.
    Move,
}

impl Event {
    fn name(self) -> &'static str {
        match self {
            Event::Press => "press",
            Event::Release => "release",
            Event::Move => "move",
        }
    }
}

/// A loaded mapping script.
pub struct Script {
    lua: Lua,
    /// Kept for error messages: a stack trace nobody can place is no help.
    name: String,
}

impl std::fmt::Debug for Script {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Script").field("name", &self.name).finish()
    }
}

impl Script {
    /// Load `source`, with `registry` readable from inside it.
    ///
    /// # Errors
    /// When the script does not compile, or has no `on_control` function.
    pub fn load(
        name: &str,
        source: &str,
        registry: Arc<ParameterRegistry>,
    ) -> Result<Self, ScriptError> {
        // The empty standard library. Not "the standard library minus the
        // dangerous parts" -- *none of it* -- and then the safe pieces are
        // added back deliberately below. A deny-list would grow a hole every
        // time Lua gained a function.
        let lua = Lua::new_with(StdLib::NONE, mlua::LuaOptions::default())
            .map_err(|e| ScriptError::Broken(e.to_string()))?;

        Self::furnish(&lua, registry)?;

        lua.load(source)
            .set_name(name)
            .exec()
            .map_err(|e| ScriptError::Broken(e.to_string()))?;

        let has_entry = lua
            .globals()
            .get::<Value>("on_control")
            .map(|v| v.is_function())
            .unwrap_or(false);
        if !has_entry {
            return Err(ScriptError::NoEntryPoint);
        }

        Ok(Self {
            lua,
            name: name.to_owned(),
        })
    }

    /// Names the base library provides that a mapping must not have.
    ///
    /// A removal list rather than an allow-list, and not by choice: mlua
    /// installs the base library whatever `StdLib` asks for, so these are
    /// present until they are taken away. Enumerated by name in a test, so a
    /// future mlua that adds one is a failing test rather than a hole.
    ///
    /// - `dofile`, `loadfile` read a file off the disk and run it.
    /// - `load` builds code at run time, which defeats reading a mapping to
    ///   know what it does — the property that makes a stranger's file safe to
    ///   *review*, as distinct from safe to run.
    /// - `collectgarbage` lets a script stall the VM without looping, which
    ///   the step limit would not catch.
    /// - `print` and `warn` write to a stream no DJ is looking at, and a MIDI
    ///   callback firing a thousand times a second is a way to make djmanzo
    ///   slow. A script says what it means by returning actions.
    /// - `_G` is the globals table itself; leaving it reachable would let a
    ///   script put back what this took away.
    ///
    /// Everything else the base library offers -- `pcall`, `type`, `pairs`,
    /// `setmetatable` and the rest -- stays. None of it can reach outside the
    /// process once `io`, `os` and `package` are absent, and a mapping that
    /// wants a metatable is entitled to one.
    const FORBIDDEN: &'static [&'static str] = &[
        "dofile",
        "loadfile",
        "load",
        "collectgarbage",
        "print",
        "warn",
        "_G",
    ];

    /// Put the safe pieces back, one at a time and on purpose.
    fn furnish(lua: &Lua, registry: Arc<ParameterRegistry>) -> Result<(), ScriptError> {
        let globals = lua.globals();
        let broken = |e: mlua::Error| ScriptError::Broken(e.to_string());

        // Arithmetic and string handling, because a mapping computes numbers
        // and builds action text. Neither can reach outside the process.
        lua.load_std_libs(StdLib::MATH | StdLib::STRING | StdLib::TABLE)
            .map_err(broken)?;

        // And the base library's dangerous half, which arrived uninvited.
        for name in Self::FORBIDDEN {
            globals.set(*name, Value::Nil).map_err(broken)?;
        }

        // Reading a parameter by the same stable name the interface and the
        // network API use: `deck.1.playing`, `master_bpm`. A script that can
        // ask "is the deck playing" can make a decision; one that cannot is a
        // table with extra syntax.
        let param = lua
            .create_function(move |_, name: String| {
                Ok(ParamId::all()
                    .find(|id| id.name() == name)
                    .map(|id| f64::from(registry.get(id))))
            })
            .map_err(broken)?;
        globals.set("parameter", param).map_err(broken)?;

        Ok(())
    }

    /// Ask the script what a control event means.
    ///
    /// `control` is the `on = "..."` text of the binding, so a script can tell
    /// its pads apart. `value` is 0..=1 for a move and unused otherwise.
    ///
    /// Returns the action text the script asked for — **already checked
    /// against the vocabulary**, so what comes back is known to be something
    /// djmanzo can do.
    ///
    /// # Errors
    /// When the script fails, runs too long, or asks for an action that does
    /// not exist.
    pub fn on_control(
        &self,
        control: &str,
        event: Event,
        value: f32,
    ) -> Result<Vec<String>, ScriptError> {
        // Re-armed per event: the budget is per decision, not per session.
        //
        // A hook that could not be installed is not a small problem: the
        // script would then run unbounded on the MIDI thread. Refusing to run
        // it at all is the only safe answer.
        let steps = std::cell::Cell::new(0u32);
        let armed = self.lua.set_hook(
            mlua::HookTriggers::new().every_nth_instruction(1_000),
            move |_lua, _debug| {
                let used = steps.get() + 1_000;
                steps.set(used);
                if used > STEP_LIMIT {
                    Err(mlua::Error::RuntimeError("too long".into()))
                } else {
                    Ok(mlua::VmState::Continue)
                }
            },
        );
        if let Err(why) = armed {
            return Err(ScriptError::Failed(
                control.to_owned(),
                format!("the step limit could not be armed, so the script was not run: {why}"),
            ));
        }

        let entry: mlua::Function = self
            .lua
            .globals()
            .get("on_control")
            .map_err(|_| ScriptError::NoEntryPoint)?;

        let outcome: mlua::Result<Value> =
            entry.call((control.to_owned(), event.name(), f64::from(value)));
        self.lua.remove_hook();

        let returned = outcome.map_err(|e| {
            let text = e.to_string();
            if text.contains("too long") {
                ScriptError::TooLong
            } else {
                ScriptError::Failed(control.to_owned(), text)
            }
        })?;

        let mut actions = Vec::new();
        match returned {
            Value::Nil => {}
            Value::String(one) => actions.push(one.to_string_lossy()),
            Value::Table(many) => {
                for entry in many.sequence_values::<String>() {
                    actions.push(
                        entry
                            .map_err(|e| ScriptError::Failed(control.to_owned(), e.to_string()))?,
                    );
                }
            }
            other => {
                return Err(ScriptError::Failed(
                    control.to_owned(),
                    format!(
                        "returned a {} where an action was expected",
                        other.type_name()
                    ),
                ));
            }
        }

        // **The promise.** Every string goes through the same parser a TOML
        // binding's does, so there is no path from Lua to the engine that
        // skips the vocabulary.
        for action in &actions {
            dj_core::Action::parse(action)
                .map_err(|e| ScriptError::BadAction(action.clone(), e.to_string()))?;
        }
        Ok(actions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(source: &str) -> Result<Script, ScriptError> {
        Script::load("test", source, Arc::new(ParameterRegistry::new()))
    }

    const SHIFT: &str = r#"
        local shifted = false
        function on_control(control, event, value)
          if control == "note 1 0x3F" then
            shifted = (event == "press")
            return nil
          end
          if event ~= "press" then return nil end
          if shifted then
            return "deck 1 hotcue_set 1"
          else
            return "deck 1 hotcue 1"
          end
        end
    "#;

    /// **What a script is for.** A shift button changes what a pad does, which
    /// is a decision, and a decision needs an `if`. A table cannot express it.
    #[test]
    fn a_shift_button_changes_what_a_pad_does() {
        let script = script(SHIFT).expect("it loads");

        assert_eq!(
            script.on_control("note 1 0x01", Event::Press, 0.0).unwrap(),
            vec!["deck 1 hotcue 1"]
        );

        assert!(
            script
                .on_control("note 1 0x3F", Event::Press, 0.0)
                .unwrap()
                .is_empty(),
            "the shift button itself did something"
        );
        assert_eq!(
            script.on_control("note 1 0x01", Event::Press, 0.0).unwrap(),
            vec!["deck 1 hotcue_set 1"],
            "shift was held and the pad did the unshifted thing"
        );

        script
            .on_control("note 1 0x3F", Event::Release, 0.0)
            .unwrap();
        assert_eq!(
            script.on_control("note 1 0x01", Event::Press, 0.0).unwrap(),
            vec!["deck 1 hotcue 1"],
            "shift was released and the pad stayed shifted"
        );
    }

    /// A script may return several actions, which is the other thing a table
    /// cannot do: one pad, two things.
    #[test]
    fn a_script_may_return_several_actions() {
        let script = script(
            r#"function on_control(c, e, v)
                 return { "deck 1 play", "deck 2 play" }
               end"#,
        )
        .expect("it loads");
        assert_eq!(
            script.on_control("note 1 0x01", Event::Press, 0.0).unwrap(),
            vec!["deck 1 play", "deck 2 play"]
        );
    }

    /// A move carries its value, so a script can compute with it.
    #[test]
    fn a_move_carries_its_value() {
        let script = script(
            r#"function on_control(c, e, v)
                 if e ~= "move" then return nil end
                 return string.format("deck 1 volume %.3f", v * 0.5)
               end"#,
        )
        .expect("it loads");
        assert_eq!(
            script.on_control("cc 1 0x08", Event::Move, 0.8).unwrap(),
            vec!["deck 1 volume 0.400"]
        );
    }

    /// **The promise the whole module rests on.** A script returns text, and
    /// that text goes through `Action::parse` exactly as a TOML binding's
    /// does. There is no path from Lua to the engine that skips the
    /// vocabulary.
    #[test]
    fn an_action_a_script_invents_is_refused_like_any_other() {
        let script = script(r#"function on_control(c, e, v) return "deck 1 levitate" end"#)
            .expect("it loads");
        let why = script
            .on_control("note 1 0x01", Event::Press, 0.0)
            .expect_err("levitate is not an action");
        assert!(
            matches!(why, ScriptError::BadAction(ref text, _) if text == "deck 1 levitate"),
            "wrong error: {why}"
        );
    }

    /// **Nothing reaches outside the process.** A mapping that could open a
    /// file, spawn a process or load native code is a mapping nobody should
    /// download, and djmanzo's mappings are meant to be safe to accept from a
    /// stranger.
    ///
    /// Enumerated by name rather than trusting `StdLib::NONE`, because that
    /// flag is not enough: mlua installs the base library regardless, and
    /// `dofile` was reachable from a script until this test said so.
    #[test]
    fn a_script_cannot_reach_outside_the_process() {
        for forbidden in [
            // Never loaded.
            "io",
            "os",
            "package",
            "require",
            "debug",
            "loadstring",
            "rawequal_",
            // Present until taken away. See `Script::FORBIDDEN`.
            "dofile",
            "loadfile",
            "load",
            "collectgarbage",
            "print",
            "warn",
            "_G",
        ] {
            let source = format!(
                "function on_control(c, e, v)\n  if {forbidden} == nil then return nil end\n  \
                 return \"deck 1 play\"\nend"
            );
            let script = script(&source)
                .unwrap_or_else(|e| panic!("{forbidden}: the script did not load: {e}"));
            let out = script.on_control("note 1 0x01", Event::Press, 0.0);
            assert_eq!(
                out.as_deref(),
                Ok(&[][..]),
                "`{forbidden}` exists inside a script"
            );
        }
    }

    /// The other half of the rule: what is *kept* is kept on purpose. A
    /// mapping that wants `pcall` or a metatable is entitled to one, and
    /// removing them would be security theatre once `io` and `os` are gone.
    #[test]
    fn the_harmless_half_of_the_base_library_stays() {
        for kept in [
            "pcall",
            "type",
            "pairs",
            "ipairs",
            "tostring",
            "tonumber",
            "setmetatable",
        ] {
            let source = format!(
                "function on_control(c, e, v)\n  if {kept} == nil then return nil end\n  \
                 return \"deck 1 play\"\nend"
            );
            let script = script(&source).unwrap_or_else(|e| panic!("{kept}: {e}"));
            assert_eq!(
                script.on_control("note 1 0x01", Event::Press, 0.0).unwrap(),
                vec!["deck 1 play"],
                "`{kept}` was taken away; nothing needed it gone"
            );
        }
    }

    /// Arithmetic and strings are put back on purpose, because a mapping
    /// computes numbers and builds action text.
    #[test]
    fn the_safe_pieces_are_there() {
        let script = script(
            r#"function on_control(c, e, v)
                 local n = math.floor(v * 4) + 1
                 return string.format("deck 1 hotcue %d", n)
               end"#,
        )
        .expect("it loads");
        assert_eq!(
            script.on_control("cc 1 0x01", Event::Move, 0.5).unwrap(),
            vec!["deck 1 hotcue 3"]
        );
    }

    /// A script can ask what the engine is doing, by the same stable names the
    /// interface and the network API use. Without that it is a table with
    /// extra syntax.
    #[test]
    fn a_script_can_read_a_parameter_by_name() {
        let registry = Arc::new(ParameterRegistry::new());
        registry.set(
            ParamId::Deck(
                dj_core::DeckId::from_human(1).unwrap(),
                dj_core::DeckParam::Playing,
            ),
            1.0,
        );
        let script = Script::load(
            "test",
            r#"function on_control(c, e, v)
                 if parameter("deck.1.playing") == 1.0 then
                   return "deck 1 pause"
                 end
                 return "deck 1 play"
               end"#,
            registry,
        )
        .expect("it loads");
        assert_eq!(
            script.on_control("note 1 0x0B", Event::Press, 0.0).unwrap(),
            vec!["deck 1 pause"]
        );
    }

    /// A parameter that does not exist is `nil`, not a crash, so a typo in a
    /// script is a decision that does not fire rather than a dead controller.
    #[test]
    fn an_unknown_parameter_is_nothing_rather_than_an_error() {
        let script = script(
            r#"function on_control(c, e, v)
                 if parameter("deck.1.levitating") == nil then return "deck 1 play" end
                 return nil
               end"#,
        )
        .expect("it loads");
        assert_eq!(
            script.on_control("note 1 0x01", Event::Press, 0.0).unwrap(),
            vec!["deck 1 play"]
        );
    }

    /// **A script runs on the MIDI thread.** `while true do end` would take the
    /// controller down with it, so it is stopped instead.
    #[test]
    fn a_runaway_script_is_stopped_rather_than_taking_the_controller_with_it() {
        let script =
            script(r#"function on_control(c, e, v) while true do end end"#).expect("it loads");
        let started = std::time::Instant::now();
        let why = script
            .on_control("note 1 0x01", Event::Press, 0.0)
            .expect_err("an endless loop should be stopped");
        assert_eq!(why, ScriptError::TooLong);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "it took {:?} to give up",
            started.elapsed()
        );
    }

    /// And the budget is per decision: a script stopped once still works the
    /// next time a pad is pressed.
    #[test]
    fn the_budget_is_per_event_rather_than_per_session() {
        let script = script(
            r#"local first = true
               function on_control(c, e, v)
                 if first then first = false; while true do end end
                 return "deck 1 play"
               end"#,
        )
        .expect("it loads");
        assert_eq!(
            script.on_control("note 1 0x01", Event::Press, 0.0),
            Err(ScriptError::TooLong)
        );
        assert_eq!(
            script.on_control("note 1 0x01", Event::Press, 0.0).unwrap(),
            vec!["deck 1 play"],
            "the script was written off after one bad event"
        );
    }

    /// A script that does not compile says so when the mapping is chosen, not
    /// when a pad is pressed — the same promise the TOML side makes.
    #[test]
    fn a_broken_script_is_refused_when_it_loads() {
        assert!(matches!(
            script("function on_control( this is not lua"),
            Err(ScriptError::Broken(_))
        ));
    }

    /// A script with nothing to call is refused too, rather than silently
    /// doing nothing for the rest of the night.
    #[test]
    fn a_script_with_no_entry_point_is_refused() {
        assert_eq!(script("local x = 1").err(), Some(ScriptError::NoEntryPoint));
    }

    /// A script returning a number where an action belongs is a mistake worth
    /// naming, not something to coerce into text.
    #[test]
    fn a_script_returning_the_wrong_kind_of_thing_says_so() {
        let script = script(r#"function on_control(c, e, v) return 42 end"#).expect("it loads");
        assert!(matches!(
            script.on_control("note 1 0x01", Event::Press, 0.0),
            Err(ScriptError::Failed(_, _))
        ));
    }
}
