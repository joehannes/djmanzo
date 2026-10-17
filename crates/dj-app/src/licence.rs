//! §130/§131: what this run of djmanzo may use, and where each thing it
//! offers belongs.
//!
//! One download carries every module (`docs/adr/0011-licence-by-sign-in.md`);
//! the licence decides which this run may use. The parts list is
//! `dj_core::edition`. This is the application's half: the run's [`Licence`],
//! and the one place each command, panel and activity says which modules it
//! needs — [`command_needs`], [`surface_needs`], [`activity_needs`]. An action
//! says it itself (`dj_core::edition::Needs::of_action`), because a pad, a
//! key and a button send the same words.
//!
//! # Refused at the door, hidden from the screen
//!
//! Every command passes [`gate`] before it runs (`lib.rs` wraps the handler),
//! so nothing a module owns can be reached around the interface. The lists
//! the interface is drawn from — panels, activities, arrangements, the
//! palette, the tree under Space, the dashboard — leave out what this run
//! does not carry, so the interface never offers a button that would be
//! refused. A command that only reads how something stands (`*_status`, a
//! catalogue) is the foundation's: the interface can always ask, and a panel
//! that is not drawn never does.
//!
//! # The licence, for now
//!
//! Until signed licences exist (`docs/EDITIONS.md`, step 3), every run is a
//! **development licence**: Full, unless [`EDITION_ENV`] names another
//! edition, with [`MODULES_ENV`] adding modules to it as a Custom licence
//! would. A development affordance, like `DJMANZO_DEMO`.

use dj_core::edition::{Edition, Module, Modules, Needs};

/// Names the edition a development run plays as: a slug of
/// `dj_core::edition::Edition` (`hobbyist`, `pro`, …). Unset is Full.
pub const EDITION_ENV: &str = "DJMANZO_EDITION";

/// Modules added to [`EDITION_ENV`]'s, as a Custom licence lists them:
/// `stems, karaoke`.
pub const MODULES_ENV: &str = "DJMANZO_MODULES";

/// What this run may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Licence {
    pub edition: Edition,
    pub modules: Modules,
    /// A development licence, not one the store signed.
    pub development: bool,
}

impl Licence {
    /// Full, for development: what every run was before editions.
    pub const DEVELOPMENT: Self = Self {
        edition: Edition::Full,
        modules: Modules::all(),
        development: true,
    };

    /// A development licence for `edition`, with `extra` modules on top.
    #[must_use]
    pub const fn development(edition: Edition, extra: Modules) -> Self {
        let mut modules = edition.modules();
        let mut i = 0;
        while i < Module::ALL.len() {
            if extra.has(Module::ALL[i]) {
                modules = modules.with(Module::ALL[i]);
            }
            i += 1;
        }
        Self {
            edition,
            modules,
            development: true,
        }
    }

    /// The development licence the environment names.
    ///
    /// # Errors
    /// A word that names no edition or no module: said rather than read as
    /// Full, because a test run that meant Hobbyist and got Full proves
    /// nothing about Hobbyist.
    pub fn from_environment(edition: Option<&str>, modules: Option<&str>) -> Result<Self, String> {
        let edition = match edition.map(str::trim).filter(|e| !e.is_empty()) {
            None => Edition::Full,
            Some(slug) => Edition::from_slug(slug).ok_or_else(|| {
                let known: Vec<&str> = Edition::ALL.iter().map(|e| e.slug()).collect();
                format!(
                    "{EDITION_ENV}={slug:?} names no edition; one of {}",
                    known.join(", ")
                )
            })?,
        };
        let extra = Modules::parse(modules.unwrap_or_default())
            .map_err(|e| format!("{MODULES_ENV}: {e}"))?;
        Ok(Self::development(edition, extra))
    }

    /// Whether this run may use what `needs` asks for.
    #[must_use]
    pub const fn allows(self, needs: Needs) -> bool {
        needs.met_by(self.modules)
    }

    /// Whether this run carries `module`.
    #[must_use]
    pub const fn has(self, module: Module) -> bool {
        self.modules.has(module)
    }
}

impl Default for Licence {
    fn default() -> Self {
        Self::DEVELOPMENT
    }
}

/// Refuse a command this run does not carry, in the sentence a DJ reads.
///
/// # Errors
/// The refusal, naming what it needs and the cheapest edition that has it.
pub fn gate(licence: Licence, command: &str) -> Result<(), String> {
    let needs = command_needs(command);
    if licence.allows(needs) {
        Ok(())
    } else {
        Err(needs.refusal())
    }
}

use Module::{
    Assistant, Autopilot, Controllers, Crowd, Dvs, Events, Karaoke, Plugins, Replay, Sources,
    Stems, Streaming, Teacher, Video,
};

/// The singers' screen's looks and pictures: karaoke's and the visuals'.
const LOOKS: Needs = Needs::any(&[Karaoke, Video]);
/// The room's page, its requests and its reactions: karaoke's sign-up sheet,
/// and the crowd's voice.
const ROOM_PAGE: Needs = Needs::any(&[Karaoke, Crowd]);
/// Finding a record from a line, a hum or a description: what a singer
/// remembers, and what the assistant is asked.
const FINDING: Needs = Needs::any(&[Karaoke, Assistant]);
/// The night's plan as a sequence: the working DJ's preparation, and what
/// the autopilot plays through.
const PLAN: Needs = Needs::any(&[Events, Autopilot]);
/// The transition as an object, planned and held: what the assistant offers
/// and the autopilot performs.
const TRANSITION: Needs = Needs::any(&[Assistant, Autopilot]);
/// The night read as an arc: the assistant's, the autopilot's and the
/// room's reading of it.
const ARC: Needs = Needs::any(&[Assistant, Autopilot, Crowd]);
/// What a live stream's chat said, and the camera it is filmed on.
const STREAM_ROOM: Needs = Needs::any(&[Crowd, Streaming]);

/// Every command a module owns, by module. A command not here is the
/// foundation's — and must be named in [`FOUNDATION_COMMANDS`], so a new one
/// cannot slip in on neither list (`every_command_is_placed`).
pub const MODULE_COMMANDS: &[(&str, Needs)] = &[
    // Stems: sending a deck out in parts (how it stands, `stem_out`, is a
    // read Settings makes on opening, and the foundation's). Separating at load is asked where
    // the load is (`commands::put_on_deck`), and a stem action by its words.
    ("set_stem_out", Needs::one(Stems)),
    // Auto-pilot: the autopilot working through a set.
    ("assistant_set_setlist", Needs::one(Autopilot)),
    ("assistant_step", Needs::one(Autopilot)),
    // Assistant: what to play next, the whisper, staged moves, packs, asking.
    ("ask", Needs::one(Assistant)),
    ("suggest_next", Needs::one(Assistant)),
    ("next_decision", Needs::one(Assistant)),
    ("similar_to", Needs::one(Assistant)),
    ("library_lens", Needs::one(Assistant)),
    ("ghost_preview", Needs::one(Assistant)),
    ("control_suggestions", Needs::one(Assistant)),
    ("whisper_answer", Needs::one(Assistant)),
    ("staged_prepare", Needs::one(Assistant)),
    ("staged_choose", Needs::one(Assistant)),
    ("staged_accept", Needs::one(Assistant)),
    ("staged_reject", Needs::one(Assistant)),
    ("assistant_set_occasion", Needs::one(Assistant)),
    ("assistant_apply_pack", Needs::one(Assistant)),
    ("set_chosen_pack", Needs::one(Assistant)),
    ("voice_strip_set", Needs::one(Assistant)),
    ("guides", Needs::any(&[Assistant, Teacher])),
    ("plan_transition", TRANSITION),
    ("transition_arm", TRANSITION),
    ("transition_adjust", TRANSITION),
    ("transition_replan", TRANSITION),
    ("night_declare", ARC),
    // Teacher: the practice lab and deck, the coach, the learning guide.
    ("practice_live", Needs::one(Teacher)),
    ("practice_live_load", Needs::one(Teacher)),
    ("practice_live_do", Needs::one(Teacher)),
    ("practice_live_eject", Needs::one(Teacher)),
    ("practice_rehearse", Needs::one(Teacher)),
    ("coach_report", Needs::one(Teacher)),
    ("learning_moves", Needs::one(Teacher)),
    ("learning_mark", Needs::one(Teacher)),
    // Karaoke: the singers, their microphones, their screen and the journal.
    ("vocals_open", Needs::one(Karaoke)),
    ("vocal_strip_set", Needs::one(Karaoke)),
    ("vocal_strip_preset", Needs::one(Karaoke)),
    ("vocal_strip_singer", Needs::one(Karaoke)),
    ("vocal_strip_keep", Needs::one(Karaoke)),
    ("vocal_line", Needs::one(Karaoke)),
    ("karaoke_rotation", Needs::one(Karaoke)),
    ("karaoke_ask", Needs::one(Karaoke)),
    ("karaoke_take", Needs::one(Karaoke)),
    ("karaoke_refuse", Needs::one(Karaoke)),
    ("karaoke_line", Needs::one(Karaoke)),
    ("karaoke_bring_in", Needs::one(Karaoke)),
    ("karaoke_sang", Needs::one(Karaoke)),
    ("karaoke_not_here", Needs::one(Karaoke)),
    ("karaoke_move", Needs::one(Karaoke)),
    ("karaoke_leave", Needs::one(Karaoke)),
    ("karaoke_key", Needs::one(Karaoke)),
    ("karaoke_clear", Needs::one(Karaoke)),
    ("karaoke_load", Needs::one(Karaoke)),
    ("karaoke_scoring", Needs::one(Karaoke)),
    ("karaoke_verdict", Needs::one(Karaoke)),
    ("karaoke_versus", Needs::one(Karaoke)),
    ("karaoke_breaks_set", Needs::one(Karaoke)),
    ("karaoke_range_start", Needs::one(Karaoke)),
    ("karaoke_range_take", Needs::one(Karaoke)),
    ("karaoke_range_cancel", Needs::one(Karaoke)),
    ("karaoke_fits", Needs::one(Karaoke)),
    ("karaoke_songs_for", Needs::one(Karaoke)),
    ("karaoke_record_song", Needs::one(Karaoke)),
    ("karaoke_record_stop", Needs::one(Karaoke)),
    ("karaoke_guide_offer", Needs::one(Karaoke)),
    ("sing_rules_set", Needs::one(Karaoke)),
    ("sing_photo_approve", Needs::one(Karaoke)),
    ("sing_photo_refuse", Needs::one(Karaoke)),
    ("singer_lyrics", Needs::one(Karaoke)),
    ("singer_backdrop", Needs::one(Karaoke)),
    ("singer_photo", Needs::one(Karaoke)),
    ("guests_save", Needs::one(Karaoke)),
    ("guests_forget", Needs::one(Karaoke)),
    ("guests_voice", Needs::one(Karaoke)),
    ("guests_song", Needs::one(Karaoke)),
    ("guests_export", Needs::one(Karaoke)),
    ("guests_reveal_voice", Needs::one(Karaoke)),
    ("guests_message", Needs::one(Karaoke)),
    ("guests_whatsapp_here", Needs::one(Karaoke)),
    ("open_suno", Needs::one(Karaoke)),
    ("suno_fill", Needs::one(Karaoke)),
    ("suno_link", Needs::one(Karaoke)),
    ("word_timing_run", Needs::one(Karaoke)),
    ("word_timing_download", Needs::one(Karaoke)),
    ("word_timing_choose", Needs::one(Karaoke)),
    ("quiz_set", Needs::one(Karaoke)),
    ("quiz_next", Needs::one(Karaoke)),
    ("quiz_again", Needs::one(Karaoke)),
    ("quiz_reveal", Needs::one(Karaoke)),
    ("quiz_end", Needs::one(Karaoke)),
    ("find_song", FINDING),
    ("guess_from_description", FINDING),
    ("hum", FINDING),
    ("melody_sweep", FINDING),
    // The singers' screen's looks: karaoke's, and the visuals'.
    ("look_choose", LOOKS),
    ("look_ai", LOOKS),
    ("look_picked", LOOKS),
    ("milkdrop_add", LOOKS),
    ("milkdrop_remove", LOOKS),
    ("artist_pictures", LOOKS),
    // The room's page: karaoke's sign-up sheet and the crowd's voice.
    ("audience_start", ROOM_PAGE),
    ("audience_open", ROOM_PAGE),
    ("audience_settle", ROOM_PAGE),
    ("audience_sheet", ROOM_PAGE),
    ("room_reactions_set", ROOM_PAGE),
    // Reading the room.
    ("room_saw", Needs::one(Crowd)),
    ("room_read", Needs::one(Crowd)),
    ("room_forget", Needs::one(Crowd)),
    ("room_history", Needs::one(Crowd)),
    ("floor_saw", Needs::one(Crowd)),
    ("floor_read", Needs::one(Crowd)),
    ("crowd_add", Needs::one(Crowd)),
    ("crowd_import", Needs::one(Crowd)),
    ("crowd_read_now", Needs::one(Crowd)),
    ("crowd_compare", Needs::one(Crowd)),
    ("crowd_settings_save", Needs::one(Crowd)),
    ("set_camera_wanted", STREAM_ROOM),
    ("phone_camera_set", STREAM_ROOM),
    ("phone_camera_new_link", STREAM_ROOM),
    ("youtube_settings_set", STREAM_ROOM),
    ("youtube_sign_in", STREAM_ROOM),
    // Controllers beyond a shipped mapping, and other things driving djmanzo.
    ("mapping_learn", Needs::one(Controllers)),
    ("mapping_bind", Needs::one(Controllers)),
    ("mapping_unbind", Needs::one(Controllers)),
    ("mapping_rename", Needs::one(Controllers)),
    ("mapping_draft_from", Needs::one(Controllers)),
    ("mapping_save", Needs::one(Controllers)),
    ("open_hid_controller", Needs::one(Controllers)),
    ("start_remote", Needs::one(Controllers)),
    ("start_osc", Needs::one(Controllers)),
    ("start_clock", Needs::one(Controllers)),
    ("follow_clock", Needs::one(Controllers)),
    ("start_peer_sync", Needs::one(Controllers)),
    // Timecode vinyl.
    ("start_timecode", Needs::one(Dvs)),
    ("write_timecode_signal", Needs::one(Dvs)),
    // Going live.
    ("obs_settings_set", Needs::one(Streaming)),
    ("obs_stream", Needs::one(Streaming)),
    ("obs_scene", Needs::one(Streaming)),
    ("obs_moments_set", Needs::one(Streaming)),
    ("set_live", Needs::one(Streaming)),
    ("share_to", Needs::one(Streaming)),
    ("share_to_whatsapp", Needs::one(Streaming)),
    // Plugins.
    ("load_plugin", Needs::one(Plugins)),
    // Online sources: searching, fetching and buying beyond the DJ's files.
    ("search_sources", Needs::one(Sources)),
    ("resolve_source_track", Needs::one(Sources)),
    ("open_store", Needs::one(Sources)),
    ("set_downloads", Needs::one(Sources)),
    // Events: the event, the press kit, and the night's plan.
    ("new_event", Needs::one(Events)),
    ("save_event", Needs::one(Events)),
    ("take_event_idea", Needs::one(Events)),
    ("set_live_event", Needs::one(Events)),
    ("kit_save", Needs::one(Events)),
    ("kit_compose", Needs::one(Events)),
    ("kit_add", Needs::one(Events)),
    ("kit_fill", Needs::one(Events)),
    ("kit_take_photo", Needs::one(Events)),
    ("kit_send", Needs::one(Events)),
    ("setlist_build", PLAN),
    ("setlist_steer", PLAN),
    ("setlist_save", PLAN),
    // Replay: hearing a set back and rendering it again.
    ("session_open", Needs::one(Replay)),
    ("session_render", Needs::one(Replay)),
    ("session_render_mix", Needs::one(Replay)),
    ("session_diff", Needs::one(Replay)),
];

/// What a command needs: its row in [`MODULE_COMMANDS`], or the foundation.
#[must_use]
pub fn command_needs(command: &str) -> Needs {
    MODULE_COMMANDS
        .iter()
        .find(|(name, _)| *name == command)
        .map_or(Needs::FOUNDATION, |(_, needs)| *needs)
}

/// What a panel needs. Panels not named are the foundation's.
#[must_use]
pub fn surface_needs(surface: &str) -> Needs {
    match surface {
        "next" | "assistant" => Needs::one(Assistant),
        "pair" => TRANSITION,
        "plan" => PLAN,
        "night" => ARC,
        "room" | "crowd" => Needs::one(Crowd),
        "camera" => STREAM_ROOM,
        "requests" => ROOM_PAGE,
        "karaoke" => Needs::one(Karaoke),
        "memory" => FINDING,
        "stems" => Needs::one(Stems),
        "practice" => Needs::one(Teacher),
        "event" | "kit" => Needs::one(Events),
        "video" => Needs::one(Video),
        _ => Needs::FOUNDATION,
    }
}

/// What a waveform layer needs: the stems' presence is drawn from the
/// separation, which runs for the stems or karaoke's vocals out; the
/// suggestion is the assistant's and the crowd's response the room's. The
/// rest are read off the record itself.
#[must_use]
pub fn layer_needs(layer: &str) -> Needs {
    match layer {
        "stems" | "vocal" => Needs::any(&[Stems, Karaoke]),
        "suggestion" => Needs::one(Assistant),
        "crowd" => Needs::one(Crowd),
        _ => Needs::FOUNDATION,
    }
}

/// What a panel in a window of its own needs: the singers' screen is
/// karaoke's, and the rest are the panel they draw ([`surface_needs`]).
#[must_use]
pub fn panel_needs(panel: crate::monitors::Panel) -> Needs {
    match panel {
        crate::monitors::Panel::Singers => Needs::one(Karaoke),
        other => other.surface().map_or(Needs::FOUNDATION, surface_needs),
    }
}

/// What a reading on the mission bar needs: the night's phase is the arc's,
/// the occasion and how much the assistant does are the assistant's (the
/// autopilot's too, for how much it does), and the room is the crowd's. The
/// clock, the tempo, the recording, the sound card and the machine's health
/// are the foundation's.
#[must_use]
pub fn mission_needs(slug: &str) -> Needs {
    match slug {
        "phase" => ARC,
        "occasion" => Needs::one(Assistant),
        "posture" => TRANSITION,
        "room" => Needs::one(Crowd),
        _ => Needs::FOUNDATION,
    }
}

/// What an activity needs: the ones that exist for a module. The rest open
/// on whatever of their arrangement this run has.
#[must_use]
pub fn activity_needs(slug: &str) -> Needs {
    match slug {
        "autopilot" => Needs::one(Autopilot),
        "practice" => Needs::one(Teacher),
        "karaoke" => Needs::one(Karaoke),
        "event" => Needs::one(Events),
        "requests" => ROOM_PAGE,
        _ => Needs::FOUNDATION,
    }
}

/// A workspace brought into what the cockpit can draw **and this run
/// carries**: `crate::cockpit::resolve`, after the decks are held to two
/// without [`Module::FourDecks`] and the panels of modules not carried are
/// left out — each said in a note, as the resolver says every correction.
///
/// Every path that keeps or draws an arrangement comes through here, so a
/// workspace file made in Full opens in the Hobbyist edition as the most of
/// it that edition has, not as an error.
#[must_use]
pub fn resolve(
    workspace: &crate::cockpit::Workspace,
    licence: Licence,
) -> crate::cockpit::Resolved {
    let mut held = workspace.clone();
    let mut notes = Vec::new();
    let decks = Needs::one(Module::FourDecks);
    if held.decks > 2 && !licence.allows(decks) {
        notes.push(format!(
            "{} decks became two: {}",
            held.decks,
            decks.refusal()
        ));
        held.decks = 2;
    }
    held.surfaces.retain(|placement| {
        let needs = surface_needs(&placement.surface);
        let kept = licence.allows(needs);
        if !kept {
            notes.push(format!(
                "`{}` was left out: {}",
                placement.surface,
                needs.refusal()
            ));
        }
        kept
    });
    let mut resolved = crate::cockpit::resolve(&held);
    notes.append(&mut resolved.notes);
    resolved.notes = notes;
    resolved
}

/// Whether this run offers a shipped or kept arrangement: all of its panels
/// and its decks are carried. One that would open as something else is not
/// offered under its name; [`resolve`] still opens it, as the most of it this
/// run has, when a file asks for it.
#[must_use]
pub fn offers_workspace(licence: Licence, workspace: &crate::cockpit::Workspace) -> bool {
    (workspace.decks <= 2 || licence.has(Module::FourDecks))
        && workspace
            .surfaces
            .iter()
            .all(|placement| licence.allows(surface_needs(&placement.surface)))
}

/// Whether a preset may be applied: every action it would run is allowed.
/// Read for deck one, the representative deck its own check uses; a deck
/// past the second is the decks' question, asked when it is applied.
#[must_use]
pub fn allows_preset(licence: Licence, preset: &dj_presets::Preset) -> bool {
    preset.resolve(1).is_ok_and(|actions| {
        actions
            .iter()
            .all(|action| licence.allows(Needs::of_action(action)))
    })
}

/// A board, when this run carries every part of it — the rule
/// [`offers_workspace`] holds an arrangement to. Not trimmed: a board is
/// named for its work, and the Karaoke board with its karaoke taken out
/// would be the library under karaoke's name.
#[must_use]
pub fn board_for(licence: Licence, board: crate::boards::Board) -> Option<crate::boards::Board> {
    board
        .widgets
        .iter()
        .all(|widget| licence.allows(surface_needs(widget.name)))
        .then_some(board)
}

/// What this run offers, for reading the lines a button, a key under Space,
/// a palette entry or a dashboard tile runs.
#[derive(Debug, Clone, Copy)]
pub struct Offer<'a> {
    pub licence: Licence,
    /// The arrangements a `switch workspace` line may name.
    pub workspaces: &'a [crate::cockpit::Workspace],
    /// The packs a `switch preset` line may name.
    pub packs: &'a [dj_presets::Pack],
    /// The boards this run has, already through [`board_for`].
    pub boards: &'a [crate::boards::Board],
}

impl Offer<'_> {
    /// Whether a line is offered, however it is written: bare or as the
    /// tree's `action …`; `ui …`; `surface`, `lift` or `board` and a name;
    /// `switch activity`, `workspace`, `preset` or `theme`. A line in none of
    /// these forms is not this file's to refuse.
    #[must_use]
    pub fn line(&self, line: &str) -> bool {
        let licence = self.licence;
        let line = line.trim();
        if let Some(action) = line.strip_prefix("action ") {
            return self.line(action);
        }
        if line.starts_with("ui ")
            && let Ok(op) = crate::uiop::UiOp::parse(line)
        {
            return op
                .surface()
                .is_none_or(|surface| licence.allows(surface_needs(surface)));
        }
        if let Some(name) = line
            .strip_prefix("surface ")
            .or_else(|| line.strip_prefix("lift "))
        {
            return licence.allows(surface_needs(name.trim()));
        }
        if let Some(slug) = line.strip_prefix("board ") {
            return self.boards.iter().any(|board| board.slug == slug.trim());
        }
        if let Some(slug) = line.strip_prefix("switch activity ") {
            return licence.allows(activity_needs(slug.trim()));
        }
        if let Some(name) = line.strip_prefix("switch workspace ") {
            return self
                .workspaces
                .iter()
                .find(|w| w.name == name.trim())
                .is_none_or(|w| offers_workspace(licence, w));
        }
        if let Some(rest) = line.strip_prefix("switch preset ") {
            let id = rest.split_whitespace().next().unwrap_or_default();
            return self
                .packs
                .iter()
                .flat_map(|pack| &pack.presets)
                .find(|preset| preset.id == id)
                .is_none_or(|preset| allows_preset(licence, preset));
        }
        dj_core::Action::parse(line)
            .map_or(true, |action| licence.allows(Needs::of_action(&action)))
    }

    /// The tree under Space, without what is not offered: a leaf whose line
    /// is refused goes, and a group left with nothing goes with it.
    #[must_use]
    pub fn prune(&self, node: crate::leader::Node) -> Option<crate::leader::Node> {
        if let Some(run) = &node.run {
            return self.line(run).then_some(node);
        }
        let had = node.children.len();
        let children: Vec<_> = node
            .children
            .into_iter()
            .filter_map(|child| self.prune(child))
            .collect();
        (had == 0 || !children.is_empty()).then_some(crate::leader::Node { children, ..node })
    }
}

/// Every command that is the foundation's, named so that placing a new
/// command is a decision rather than a default.
pub const FOUNDATION_COMMANDS: &[&str] = &[
    // The sound and the decks.
    "list_devices",
    "list_inputs",
    "open_device",
    "active_device",
    "start_audio",
    "stop_audio",
    "load_track",
    "load_sample",
    "sample_outline",
    "dispatch",
    "get_snapshot",
    "waveform_info",
    "melody_line",
    "rhythm_line",
    "open_mic",
    "close_mic",
    "set_deck_out",
    "stems_status",
    "stem_out",
    "deck_queue",
    "deck_queue_set",
    "deck_queue_load",
    "audition",
    "at_hand",
    "waveform_moves",
    "phrase_grid",
    "waveform_layers",
    "chosen_layers",
    "set_chosen_layers",
    "waveform_colourings",
    "waveform_colouring",
    "set_waveform_colouring",
    // The library and the DJ's own files.
    "library_status",
    "library_add_folder",
    "library_remove_folder",
    "library_rescan",
    "library_search",
    "list_playlists",
    "create_playlist",
    "rename_playlist",
    "delete_playlist",
    "move_playlist",
    "playlist_tracks",
    "add_to_playlist",
    "remove_from_playlist",
    "reorder_playlist",
    "play_history",
    "set_playlist_query",
    "check_filter",
    "smart_playlist_tracks",
    "import_library",
    "edit_tracks",
    "clear_track_field",
    "find_duplicates",
    "forget_track_path",
    "list_sessions",
    "export_session",
    "sidelist",
    "sidelist_add",
    "sidelist_remove",
    "sidelist_clear",
    "library_columns",
    "chosen_columns",
    "set_chosen_columns",
    "library_sort",
    "set_library_sort",
    "track_functions",
    "functions_of",
    "set_track_functions",
    "words_search",
    "words_progress",
    "words_fetch",
    "melody_progress",
    "add_music_folder",
    "default_music_folder",
    "remove_music_folder",
    "music_library",
    "list_sources",
    "sources_board",
    "set_secret",
    "clear_secret",
    "secrets_persist",
    "downloads",
    "store_links",
    // The interface: windows, layouts, the cockpit, themes, the keyboard.
    "list_panels",
    "detach_panel",
    "attach_panel",
    "window_ask",
    "interface_density",
    "interface_density_set",
    "palette",
    "leader_tree",
    "dashboard",
    "boards",
    "board_arrange",
    "board_reset",
    "board_make",
    "board_forget",
    "interface_settings",
    "set_toolbars",
    "used_tile",
    "leader_mine",
    "keep_mnemonic",
    "forget_mnemonic",
    "world",
    "demo_folder",
    "watershed",
    "set_watershed",
    "pad_pages",
    "list_layouts",
    "layout_folder",
    "chosen_layout",
    "choose_layout",
    "widget_catalog",
    "layout_vocabulary",
    "layout_tree",
    "cockpit_surfaces",
    "density_bands",
    "phase_priorities",
    "cockpit_workspaces",
    "cockpit_workspace",
    "set_cockpit_workspace",
    "my_workspaces",
    "keep_workspace",
    "forget_workspace",
    "cockpit_locks",
    "semantic_roles",
    "favourite_pad_pages",
    "set_favourite_pad_pages",
    "rail_controls",
    "kept_controls",
    "set_kept_controls",
    "activities",
    "set_activity_mode",
    "keep_activity",
    "forget_activity",
    "activity_suggestion",
    "ui_vocabulary",
    "ui_do",
    "theme_now",
    "theme_lock",
    "theme_chosen",
    "themes",
    "set_brand_logo",
    "clear_brand_logo",
    "has_brand_logo",
    "report_bench",
    "under_load",
    "room_poll_ms",
    "edition",
    "keyboard_keys",
    "set_keyboard_enabled",
    // Controllers through a shipped mapping, and how things stand.
    "control_status",
    "control_mappings",
    "mapping_draft",
    "open_controller",
    "close_controller",
    "close_hid_controller",
    "controller_hands",
    "controller_lights",
    "control_handles",
    "midi_outputs",
    "clock_status",
    "stop_clock",
    "unfollow_clock",
    "remote_status",
    "stop_remote",
    "stop_osc",
    "peer_status",
    "stop_peer_sync",
    "controller_standin_press",
    // Effects, presets, the sampler, recording, and the mixes they made.
    "list_presets",
    "apply_preset",
    "preset_folder",
    "save_rack_preset",
    "setups",
    "apply_setup",
    "transition_styles",
    "transition_current",
    "transition_clear",
    "session_mixes",
    "keep_mix",
    "session_log",
    "session_save",
    "current_session",
    "recording_chapters",
    "share_preview",
    "share_channels",
    "open_signup_link",
    "list_plugins",
    "plugin_state",
    "clear_plugin",
    // What djmanzo knows about the DJ and the night, shown; settings for it.
    "mission_bar",
    "dj_context",
    "night_read",
    "night_now",
    "night_setting",
    "night_settings",
    "night_fits",
    "phase_asks",
    "learned_tendencies",
    "learned_profiles",
    "learned_persona",
    "answer_persona",
    "learned_taste",
    "profile_tonight",
    "remembered",
    "standing",
    "adaptation_levels",
    "set_adaptation_level",
    "authority_matrix",
    "authority_set",
    "authority_reset",
    "assistant_sight",
    "assistant_appetite",
    "assistant_conduct",
    "assistant_set_posture",
    "assistant_take_over",
    "assistant_hand_back",
    "assistant_packs",
    "knowledge_packs",
    "chosen_pack",
    "genre_families",
    "staged_current",
    "list_llm_providers",
    "list_llm_models",
    "assistant_state",
    "set_assistant_model",
    "set_spend_cap",
    "reset_spend",
    "listening_status",
    "listening_set",
    "listening_check",
    "voice_state",
    "welcome_state",
    "welcome_save",
    "welcome_plan",
    "welcome_apply",
    // Notes, the night's journal.
    "note_add",
    "note_write",
    "note_delete",
    "notes",
    "note_counts",
    // How each module stands, read: a panel not drawn never asks.
    "vocals_close",
    "vocals_state",
    "sing_rules",
    "sing_photos",
    "guests",
    "word_timing",
    "word_timing_speed",
    "word_timing_remove",
    "word_timing_forget_whisperx",
    "karaoke_breaks",
    "quiz",
    "quiz_screen",
    "room_reactions",
    "audience_stop",
    "audience_status",
    "audience_settings",
    "audience_languages",
    "audience_waiting",
    "audience_all",
    "crowd_view",
    "camera_wanted",
    "phone_camera_status",
    "youtube_status",
    "youtube_settings",
    "youtube_sign_out",
    "obs_status",
    "obs_settings",
    "live_status",
    "looks_catalogue",
    "look_chosen",
    "milkdrop_preset",
    "milkdrop_presets",
    "deck_pictures",
    "artist_picture",
    "timecode_formats",
    "timecode_status",
    "stop_timecode",
    "list_events",
    "event_view",
    "forget_event",
    "event_options",
    "event_tonight",
    "live_event",
    "kit_view",
    "kit_forget",
    // The packs djmanzo reads looks from: the marketplace's foundation.
    "pack_inspect",
    "pack_install",
    "packs_installed",
    "pack_remove",
    "pack_key",
    "pack_make",
    "pack_indexes",
    "pack_index_add",
    "pack_index_remove",
    "packs_browse",
    "pack_get",
];

/// A module this run does not carry, as the interface shows it in its place.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WithinReach {
    pub module: &'static str,
    pub title: &'static str,
    /// The cheapest edition that has it, by its product name.
    pub edition: &'static str,
}

/// What this run is, for the interface.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EditionDto {
    pub edition: &'static str,
    pub product_name: &'static str,
    /// The modules carried, by slug.
    pub modules: Vec<&'static str>,
    /// The modules not carried, with where each can be had.
    pub missing: Vec<WithinReach>,
    pub development: bool,
}

impl From<Licence> for EditionDto {
    fn from(licence: Licence) -> Self {
        Self {
            edition: licence.edition.slug(),
            product_name: licence.edition.product_name(),
            modules: licence.modules.iter().map(Module::slug).collect(),
            missing: licence
                .modules
                .missing()
                .iter()
                .map(|module| WithinReach {
                    module: module.slug(),
                    title: module.title(),
                    edition: Edition::cheapest_with(module).product_name(),
                })
                .collect(),
            development: licence.development,
        }
    }
}

/// §130/§131: which edition this run is, what it carries, and what is
/// within reach.
#[tauri::command]
#[must_use]
pub fn edition(state: tauri::State<'_, crate::state::AppState>) -> EditionDto {
    state.licence().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every command `lib.rs` hands Tauri, read from its source.
    fn registered() -> Vec<String> {
        let source = include_str!("lib.rs");
        let start = source
            .find("tauri::generate_handler![")
            .expect("the handler list");
        let end = start + source[start..].find("])").expect("its end");
        source[start..end]
            .lines()
            .skip(1)
            .map(str::trim)
            .filter(|line| !line.starts_with("//") && !line.is_empty())
            .filter_map(|line| line.trim_end_matches(',').rsplit("::").next())
            .map(str::to_owned)
            .collect()
    }

    /// **Every command is placed, once.** In a module's row or among the
    /// foundation's — never both, never neither, never a name that is not a
    /// command. A command added without deciding where it belongs fails here,
    /// which is the point: an edition is only as true as this table.
    #[test]
    fn every_command_is_placed() {
        let registered = registered();
        assert!(registered.len() > 400, "read {} commands", registered.len());
        for command in &registered {
            let in_module = MODULE_COMMANDS.iter().filter(|(n, _)| n == command).count();
            let in_foundation = FOUNDATION_COMMANDS.iter().filter(|n| *n == command).count();
            assert_eq!(
                in_module + in_foundation,
                1,
                "`{command}` is placed {} times: put it in MODULE_COMMANDS or \
                 FOUNDATION_COMMANDS, once",
                in_module + in_foundation
            );
        }
        for (name, needs) in MODULE_COMMANDS {
            assert!(
                registered.iter().any(|c| c == name),
                "`{name}` is not a command"
            );
            assert_ne!(
                *needs,
                Needs::FOUNDATION,
                "`{name}` is a module's row needing nothing"
            );
        }
        for name in FOUNDATION_COMMANDS {
            assert!(
                registered.iter().any(|c| c == name),
                "`{name}` is not a command"
            );
        }
    }

    /// **The gate refuses what the licence lacks, in the DJ's words, and
    /// lets the foundation through in every edition.**
    #[test]
    fn the_gate_refuses_what_the_licence_lacks() {
        let hobbyist = Licence::development(Edition::Hobbyist, Modules::NONE);
        assert_eq!(gate(hobbyist, "load_track"), Ok(()));
        assert_eq!(
            gate(hobbyist, "karaoke_take"),
            Err("Karaoke is not in this edition of djmanzo: it comes with djmanzo Karaoke".into())
        );
        assert!(gate(hobbyist, "start_timecode").is_err());
        assert_eq!(
            gate(
                Licence::development(Edition::Karaoke, Modules::NONE),
                "karaoke_take"
            ),
            Ok(())
        );
        // Any one of several will do: the looks are karaoke's and the visuals'.
        assert_eq!(
            gate(
                Licence::development(Edition::ProDj, Modules::NONE),
                "look_choose"
            ),
            Ok(())
        );
        assert!(
            gate(
                Licence::development(Edition::Apprentice, Modules::NONE),
                "look_choose"
            )
            .is_err()
        );
        // A Custom licence carries exactly what it lists.
        let custom = Licence::development(Edition::Custom, Modules::of(&[Module::Dvs]));
        assert_eq!(gate(custom, "start_timecode"), Ok(()));
        assert!(gate(custom, "load_plugin").is_err());
        for (name, _) in MODULE_COMMANDS {
            assert_eq!(
                gate(Licence::DEVELOPMENT, name),
                Ok(()),
                "Full refused {name}"
            );
        }
    }

    /// **The environment names a development licence, and a wrong word is
    /// said rather than read as Full.**
    #[test]
    fn the_environment_names_a_development_licence() {
        assert_eq!(
            Licence::from_environment(None, None),
            Ok(Licence::DEVELOPMENT)
        );
        let hobbyist = Licence::from_environment(Some("hobbyist"), None).expect("hobbyist");
        assert_eq!(hobbyist.modules, Modules::NONE);
        let custom = Licence::from_environment(Some("custom"), Some("stems, dvs")).expect("custom");
        assert!(custom.has(Module::Stems) && custom.has(Module::Dvs));
        assert!(!custom.has(Module::Karaoke));
        assert!(
            Licence::from_environment(Some("platinum"), None)
                .unwrap_err()
                .contains("names no edition")
        );
        assert!(
            Licence::from_environment(None, Some("lasers"))
                .unwrap_err()
                .contains("lasers")
        );
    }

    /// **Every panel and activity named here exists**, and each one a module
    /// owns is left out of an edition without it.
    /// **Every way a line is written is read**: bare, as the tree's
    /// `action …` and `ui …`, a panel, an activity and an arrangement.
    #[test]
    fn every_way_a_line_is_written_is_read() {
        let hobbyist = Licence::development(Edition::Hobbyist, Modules::NONE);
        let workspaces = crate::cockpit::workspaces();
        let packs = dj_presets::PresetLibrary::builtin().packs().to_vec();
        let boards = |licence| -> Vec<_> {
            crate::boards::all()
                .into_iter()
                .filter_map(|b| board_for(licence, b))
                .collect()
        };
        let (hobbyist_boards, full_boards) = (boards(hobbyist), boards(Licence::DEVELOPMENT));
        let at = |licence: Licence, boards: &[crate::boards::Board], line: &str| {
            Offer {
                licence,
                workspaces: &workspaces,
                packs: &packs,
                boards,
            }
            .line(line)
        };
        for refused in [
            "deck 1 stem_mute drums",
            "action deck 1 stem_mute drums",
            "action deck 3 play",
            "action automix on",
            "surface karaoke",
            "ui show karaoke",
            "switch activity practice",
            "switch workspace 4 Deck",
            "lift karaoke",
            "board karaoke",
            "switch preset karaoke-vocals-out 1",
            "switch preset karaoke-guide-vocal 2",
        ] {
            assert!(
                !at(hobbyist, &hobbyist_boards, refused),
                "offered `{refused}`"
            );
            assert!(
                at(Licence::DEVELOPMENT, &full_boards, refused),
                "Full refused `{refused}`"
            );
        }
        for offered in [
            "action deck 1 play",
            "action crossfader 0",
            "ui palette",
            "ui show library",
            "surface library",
            "switch activity mix",
            "switch workspace Classic DJ",
            "switch theme aurora",
            "lift library",
            "board music",
            "switch preset prep-cue 1",
            // An EQ move, whatever its name says: the foundation's.
            "switch preset move-isolate-vocal 2",
        ] {
            assert!(
                at(hobbyist, &hobbyist_boards, offered),
                "refused `{offered}`"
            );
        }
    }

    #[test]
    fn panels_and_activities_are_placed_on_things_that_exist() {
        let surfaces: Vec<&str> = crate::cockpit::surfaces().iter().map(|s| s.name).collect();
        for name in [
            "next",
            "assistant",
            "pair",
            "plan",
            "night",
            "room",
            "crowd",
            "camera",
            "requests",
            "karaoke",
            "memory",
            "stems",
            "practice",
            "event",
            "kit",
            "video",
        ] {
            assert!(surfaces.contains(&name), "no panel `{name}`");
            assert_ne!(surface_needs(name), Needs::FOUNDATION, "`{name}`");
        }
        let activities: Vec<String> = crate::activity::shipped()
            .into_iter()
            .map(|a| a.slug)
            .collect();
        for slug in ["autopilot", "practice", "karaoke", "event", "requests"] {
            assert!(activities.iter().any(|a| a == slug), "no activity `{slug}`");
            assert_ne!(activity_needs(slug), Needs::FOUNDATION);
        }
        let hobbyist = Licence::development(Edition::Hobbyist, Modules::NONE);
        for slug in ["phase", "occasion", "posture", "room"] {
            assert!(!hobbyist.allows(mission_needs(slug)), "{slug}");
            assert!(Licence::DEVELOPMENT.allows(mission_needs(slug)), "{slug}");
        }
        assert!(hobbyist.allows(mission_needs("tempo")));
        let layers: Vec<&str> = dj_render::layers().iter().map(|l| l.name).collect();
        for layer in ["stems", "vocal", "suggestion", "crowd"] {
            assert!(layers.contains(&layer), "no layer `{layer}`");
            assert!(!hobbyist.allows(layer_needs(layer)), "{layer}");
        }
        assert!(hobbyist.allows(layer_needs("beats")));
        assert!(!hobbyist.allows(panel_needs(crate::monitors::Panel::Singers)));
        assert!(hobbyist.allows(panel_needs(crate::monitors::Panel::Browser)));
        assert!(!hobbyist.allows(panel_needs(
            crate::monitors::Panel::parse("assistant").expect("a panel")
        )));
        assert!(!hobbyist.allows(surface_needs("karaoke")));
        assert!(hobbyist.allows(surface_needs("library")));
        assert!(
            Licence::development(Edition::Karaoke, Modules::NONE).allows(surface_needs("requests"))
        );
    }
}
