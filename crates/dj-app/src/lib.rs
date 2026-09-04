//! The djmanzo desktop application.
//!
//! This crate is wiring, not logic. It owns the pieces and connects them:
//!
//! ```text
//!   UI (webview)
//!     │  invoke("dispatch", "deck 1 play")      ▲  emit("snapshot", {...})
//!     ▼                                         │
//!   commands ──→ ActionBus ──→ [rtrb] ──→ Engine ──→ ParameterRegistry
//!                                  ▲                        │
//!                             AudioHost thread              │
//!                        (opens devices, frees              │
//!                         retired track buffers)      SnapshotPump @60Hz
//! ```
//!
//! Note what does *not* appear: no path from the UI straight to the engine.
//! Everything is an [`dj_core::Action`] on the bus, which is what makes the same
//! surface reachable from a controller, a script or the network later, and what
//! makes a session replayable. See
//! `docs/adr/0003-action-bus-and-parameter-registry.md`.

pub mod analysis;
pub mod assistant;
pub mod audience;
pub mod automix;
pub mod autopilot;
pub mod brand;
pub mod clock;
pub mod cockpit;
pub mod commands;
pub mod context;
pub mod control;
pub mod grid;
pub mod host;
pub mod layout;
pub mod library;
pub mod memory;
pub mod monitors;
pub mod peersync;
pub mod persist;
pub mod plan;
pub mod plugins;
pub mod presets;
pub mod rackcapture;
pub mod remote;
pub mod replay;
pub mod session;
pub mod setrec;
pub mod share;
pub mod snapshot;
pub mod sources;
pub mod state;
pub mod timecode;
pub mod transition;
pub mod wav;
pub mod waveform;
pub mod widgets;
pub mod world;

pub use host::{AudioHost, HostError};
pub use snapshot::{Snapshot, SnapshotPump};
pub use state::AppState;
pub use waveform::WaveformStore;

use std::sync::Arc;
use tauri::Manager;
use tauri::http;

/// Environment variable that forces the headless audio backend.
///
/// Set by CI, and useful on any machine with no working sound device -- the
/// application still starts, so the UI can be exercised.
pub const NULL_BACKEND_ENV: &str = "DJMANZO_NULL_AUDIO";

/// Set to a track path to run the waveform benchmark on startup.
///
/// This exists because the question ADR-0004 gates on -- can WebKitGTK
/// composite a scrolling waveform at 60 fps on four decks -- can only be
/// answered by measuring inside a real webview on the target platform. A number
/// from a synthetic harness would not settle it.
pub const BENCH_ENV: &str = "DJMANZO_BENCH";

/// Set to anything to run the rendering-strategy benchmark on startup.
///
/// Answers a different question from [`BENCH_ENV`]: not whether the waveform
/// strip composites, but whether a continuously animating interface is cheaper
/// as DOM, as a 2D canvas, or as WebGL on this machine. See ADR-0004 for why
/// the answer is not obvious and `ui/src/renderbench.ts` for what it measures.
pub const RENDER_BENCH_ENV: &str = "DJMANZO_RENDERBENCH";

/// Set to a folder of audio to load and play two decks on startup.
///
/// A development affordance, not a feature: the interface can only be judged
/// with something actually playing, and on a headless machine there is nobody
/// to click Load. Never set in a normal session.
pub const DEMO_ENV: &str = "DJMANZO_DEMO";

/// Start the application.
///
/// # Panics
///
/// Panics if Tauri cannot create the main window, which is unrecoverable.
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "dj_app=info,dj_engine=info".into()),
        )
        .init();

    let use_null_backend = std::env::var(NULL_BACKEND_ENV).is_ok();
    if use_null_backend {
        tracing::info!("using the null audio backend ({NULL_BACKEND_ENV} is set)");
    }

    let state = AppState::new(use_null_backend);
    let registry = state.registry();
    let deck_count = state.deck_count();
    let waveforms = Arc::clone(state.waveforms());
    let bridge_handle = state.bridge_handle();
    let analysis = Arc::clone(state.analysis());
    let deck_tracks = state.deck_tracks();
    let sample_names = state.sample_names();
    let recording_state = state.recording_state();
    // What the context engine has made of the night, read by the pump on every
    // frame and written once a record. See `crate::context`.
    let night = Arc::clone(state.night());
    // The snapshot pump is where a hot cue change becomes visible to the host:
    // the engine sets cues at a playhead quantize may have moved, so the only
    // reliable reading is the one the audio thread publishes. See
    // `crate::persist`.
    let watched_tracks = state.deck_tracks();
    let cue_watcher = state.cue_watcher();
    let play_watcher = state.play_watcher();
    let session_id = state.session_id();
    let library_writer = state.library_writer();
    let pump_audience = Arc::clone(state.audience());

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // Handing a URL to the operating system, so the WhatsApp handoff
        // reaches WhatsApp instead of opening inside our own webview.
        .plugin(tauri_plugin_opener::init())
        // Tiles are served as images rather than pushed over IPC, so the webview
        // decodes once off the main thread and every subsequent frame is a
        // compositor translation. See docs/adr/0004.
        .register_uri_scheme_protocol(waveform::SCHEME, move |_ctx, request| {
            let path = request.uri().path().to_owned();
            let Some(key) = waveform::parse_tile_path(&path) else {
                return http::Response::builder()
                    .status(400)
                    .body(Vec::new())
                    .unwrap_or_default();
            };
            match waveforms.tile_png(key) {
                Some(png) => http::Response::builder()
                    .status(200)
                    .header("Content-Type", "image/png")
                    // Tiles are deterministic and invalidated by key, so they can
                    // be cached hard. Without this the webview refetches every
                    // tile on every scroll, which would defeat the whole design.
                    .header("Cache-Control", "public, max-age=31536000, immutable")
                    .header("Access-Control-Allow-Origin", "*")
                    .body(png.as_ref().clone())
                    .unwrap_or_default(),
                None => http::Response::builder()
                    .status(404)
                    .body(Vec::new())
                    .unwrap_or_default(),
            }
        })
        // The user's own logo, served the same way tiles are -- as an image the
        // webview loads, not bytes pushed through IPC.
        .register_uri_scheme_protocol(brand::SCHEME, move |ctx, _request| {
            let logo = ctx
                .app_handle()
                .path()
                .app_config_dir()
                .ok()
                .and_then(|dir| brand::read_logo(&dir));
            match logo {
                Some((bytes, mime)) => http::Response::builder()
                    .status(200)
                    .header("Content-Type", mime)
                    // Unlike tiles, a logo is replaced in place at the same URL.
                    // Caching it would mean the old one staying on screen until
                    // a restart, which reads as the change having failed.
                    .header("Cache-Control", "no-store")
                    .body(bytes)
                    .unwrap_or_default(),
                None => http::Response::builder()
                    .status(404)
                    .body(Vec::new())
                    .unwrap_or_default(),
            }
        })
        .manage(state)
        .setup(move |app| {
            // The assistant's own loop. Started here because it needs the
            // handle to reach the managed state, and it must outlive this
            // closure.
            commands::start_assistant_tick(app.handle().clone());

            let handle = app.handle().clone();
            if let Ok(path) = std::env::var(BENCH_ENV) {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    // Give the webview a moment to mount before asking it to
                    // load four decks and start measuring.
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    use tauri::Emitter;
                    let _ = handle.emit("bench", path);
                });
            }

            if let Ok(value) = std::env::var(RENDER_BENCH_ENV) {
                // The value is the shape count, so the scaling law can be
                // measured rather than assumed. Anything unparseable means "the
                // default", since a typo should still run the benchmark.
                let shapes: u32 = value.trim().parse().unwrap_or(0);
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    use tauri::Emitter;
                    let _ = handle.emit("renderbench", shapes);
                });
            }

            // The analysis cache lives under the app cache directory, which is
            // only knowable once Tauri has resolved it. Until this runs the
            // store is memory-only, which is correct rather than a fallback: a
            // machine with no writable cache directory should still analyse.
            if let Ok(dir) = app.path().app_cache_dir() {
                analysis.set_cache_dir(crate::analysis::cache_subdir(&dir));
            }

            // The library goes in the *config* directory rather than the cache
            // one. A cache is something the system may delete to reclaim space;
            // a DJ's cues, corrected grids and play history are not that.
            match app.path().app_config_dir() {
                Ok(dir) => {
                    let state: tauri::State<'_, AppState> = handle.state();
                    state.open_library(&dir.join("library.db"));
                }
                Err(error) => {
                    tracing::warn!(%error, "no config directory; the library stays in memory");
                }
            }

            // A DJ's own layouts sit beside their presets, in the config
            // directory: they are things the DJ made, not things the system
            // may delete to reclaim space.
            if let Ok(dir) = app.path().app_config_dir() {
                let state: tauri::State<'_, AppState> = handle.state();
                // Separation looks for its model here too, and reports why it
                // cannot run rather than refusing to start. It has to happen
                // after the directory is known and before the interface asks
                // whether stems are available.
                state.open_stems(&dir);
                // The editor saves here, so the loader has to look here too --
                // otherwise a mapping a DJ just made would not be in the list
                // they can open until the next restart.
                let problems = state.control().load_user_mappings(&dir.join("mappings"));
                for problem in problems {
                    tracing::warn!(%problem, "a user mapping could not be loaded");
                }
                state.set_config_dir(dir);
            }

            let pump = SnapshotPump::start_with_bridge(
                registry,
                deck_count,
                crate::snapshot::Sources {
                    bridge: Some(bridge_handle),
                    analysis: Some(Arc::clone(&analysis)),
                    tracks: Some(deck_tracks),
                    samples: Some(sample_names),
                    recording: Some(recording_state),
                    night: Some(night),
                },
                move |snapshot| {
                    use tauri::Emitter;
                    // The automix rides the same pump the interface does, so it
                    // sees exactly what the DJ sees and there is no second view
                    // of the decks to keep in step. It does nothing at all
                    // until it is switched on.
                    {
                        let state: tauri::State<'_, AppState> = handle.state();
                        let plan = state.automix().lock().ok().map(|mut mix| {
                            // The mix a DJ set up, if there is one. Handed in
                            // on every tick so an adjustment made in the pair
                            // view is in force on the next frame -- see
                            // `automix::Automix::follow`.
                            mix.follow(crate::commands::automix_setup(&state));
                            let plan = mix.tick(&crate::commands::automix_view(&state));
                            crate::commands::publish_automix(&state, &mix);
                            plan
                        });
                        if let Some(plan) =
                            plan.filter(|p| !p.actions.is_empty() || p.load.is_some())
                        {
                            crate::commands::run_automix_plan(&state, plan);
                        }
                    }
                    save_changed_cues(&snapshot, &watched_tracks, &cue_watcher, &library_writer);
                    // The night's evidence arrives one record at a time, which
                    // is also the only moment its shape can change -- see
                    // `AppState::record_played`.
                    for record in record_plays(
                        &snapshot,
                        &watched_tracks,
                        &play_watcher,
                        &session_id,
                        &library_writer,
                        &pump_audience,
                    ) {
                        let state: tauri::State<'_, AppState> = handle.state();
                        state.record_played(record);
                    }
                    // A fixture for the interface's layout budget, captured
                    // from the running application rather than rebuilt from a
                    // bare parameter registry.
                    //
                    // The difference is not academic. A fresh `ParameterRegistry`
                    // is all zeros, and the engine seeds it as it starts: stem
                    // volumes go to one, a deck's gain to its unity point, and
                    // so on. A snapshot built from the bare registry therefore
                    // describes every stem as muted, which the interface reads
                    // as "the DJ is using stems" and unfolds a 359 px module
                    // nobody opened. It looks like a snapshot and it is not one
                    // this application ever sends.
                    dump_snapshot(&snapshot);
                    if let Err(error) = handle.emit("snapshot", &snapshot) {
                        tracing::warn!(%error, "failed to emit snapshot");
                    }
                },
            );
            // Hand the pump to Tauri so it lives as long as the app does.
            app.manage(pump);

            // Controllers. The drain thread needs a handle, so it cannot start
            // until here; the queue itself exists from the moment the state
            // does, so a controller opened before this point still has
            // somewhere to post.
            {
                let state: tauri::State<'_, AppState> = app.state();
                if let Ok(dir) = app.path().app_config_dir() {
                    let dir = dir.join("mappings");
                    for problem in state.control().load_user_mappings(&dir) {
                        tracing::warn!(%problem, "a mapping file could not be read");
                    }
                }
                if let Some(inbox) = state.take_control_inbox() {
                    crate::control::drain(app.handle().clone(), inbox);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_devices,
            commands::list_inputs,
            commands::list_panels,
            commands::detach_panel,
            commands::attach_panel,
            commands::list_plugins,
            commands::plugin_state,
            commands::stems_status,
            commands::start_peer_sync,
            commands::stop_peer_sync,
            commands::peer_status,
            commands::stem_out,
            commands::set_stem_out,
            commands::set_deck_out,
            commands::load_plugin,
            commands::clear_plugin,
            commands::open_mic,
            commands::close_mic,
            commands::open_device,
            commands::active_device,
            commands::start_audio,
            commands::stop_audio,
            commands::load_track,
            commands::load_sample,
            commands::dispatch,
            commands::control_status,
            commands::control_mappings,
            commands::mapping_learn,
            commands::mapping_draft,
            commands::mapping_rename,
            commands::mapping_bind,
            commands::mapping_unbind,
            commands::mapping_draft_from,
            commands::mapping_save,
            commands::keyboard_keys,
            commands::set_keyboard_enabled,
            commands::open_controller,
            commands::close_controller,
            commands::open_hid_controller,
            commands::close_hid_controller,
            commands::midi_outputs,
            commands::clock_status,
            commands::start_clock,
            commands::stop_clock,
            commands::follow_clock,
            commands::unfollow_clock,
            commands::remote_status,
            commands::audience_start,
            commands::audience_stop,
            commands::audience_status,
            commands::audience_open,
            commands::audience_settings,
            commands::audience_languages,
            commands::audience_waiting,
            commands::audience_all,
            commands::audience_settle,
            commands::audience_sheet,
            commands::room_saw,
            commands::room_read,
            commands::room_forget,
            commands::words_search,
            commands::words_progress,
            commands::words_fetch,
            commands::guess_from_description,
            commands::hum,
            commands::melody_sweep,
            commands::melody_progress,
            commands::start_remote,
            commands::stop_remote,
            commands::start_osc,
            commands::stop_osc,
            commands::get_snapshot,
            commands::waveform_info,
            commands::report_bench,
            commands::session_log,
            commands::library_status,
            commands::palette,
            commands::track_functions,
            commands::functions_of,
            commands::set_track_functions,
            commands::suggest_next,
            commands::plan_transition,
            commands::transition_arm,
            commands::transition_current,
            commands::transition_adjust,
            commands::transition_drag,
            commands::transition_replan,
            commands::transition_clear,
            commands::session_read,
            commands::session_save,
            commands::session_open,
            commands::session_diff,
            commands::session_render,
            commands::setlist_build,
            commands::genre_families,
            commands::assistant_packs,
            commands::assistant_conduct,
            commands::setlist_steer,
            commands::setlist_save,
            commands::similar_to,
            commands::learned_taste,
            commands::coach_report,
            commands::note_add,
            commands::note_write,
            commands::note_delete,
            commands::notes,
            commands::note_counts,
            commands::current_session,
            commands::assistant_set_posture,
            commands::assistant_set_occasion,
            commands::assistant_apply_pack,
            commands::assistant_take_over,
            commands::assistant_hand_back,
            commands::assistant_set_setlist,
            commands::assistant_step,
            commands::library_add_folder,
            commands::library_remove_folder,
            commands::library_rescan,
            commands::library_search,
            commands::list_playlists,
            commands::create_playlist,
            commands::rename_playlist,
            commands::delete_playlist,
            commands::move_playlist,
            commands::playlist_tracks,
            commands::add_to_playlist,
            commands::remove_from_playlist,
            commands::reorder_playlist,
            commands::play_history,
            commands::set_playlist_query,
            commands::check_filter,
            commands::smart_playlist_tracks,
            commands::import_library,
            commands::edit_tracks,
            commands::clear_track_field,
            commands::find_duplicates,
            commands::forget_track_path,
            commands::list_sessions,
            commands::export_session,
            commands::open_signup_link,
            commands::share_preview,
            commands::share_to_whatsapp,
            commands::sidelist,
            commands::sidelist_add,
            commands::sidelist_remove,
            commands::sidelist_clear,
            commands::list_layouts,
            commands::pad_pages,
            commands::layout_folder,
            commands::world,
            commands::demo_folder,
            commands::watershed,
            commands::set_watershed,
            commands::chosen_layout,
            commands::choose_layout,
            commands::widget_catalog,
            commands::layout_vocabulary,
            commands::layout_tree,
            commands::cockpit_surfaces,
            commands::cockpit_workspaces,
            commands::density_bands,
            commands::cockpit_workspace,
            commands::set_cockpit_workspace,
            sources::list_sources,
            sources::set_secret,
            sources::clear_secret,
            sources::secrets_persist,
            sources::add_music_folder,
            sources::default_music_folder,
            sources::remove_music_folder,
            sources::music_library,
            sources::search_sources,
            sources::resolve_source_track,
            timecode::timecode_formats,
            timecode::timecode_status,
            timecode::start_timecode,
            timecode::stop_timecode,
            timecode::write_timecode_signal,
            brand::set_brand_logo,
            brand::clear_brand_logo,
            brand::has_brand_logo,
            assistant::list_llm_providers,
            assistant::list_llm_models,
            assistant::assistant_state,
            assistant::set_assistant_model,
            assistant::set_spend_cap,
            assistant::reset_spend,
            assistant::ask,
            presets::list_presets,
            presets::apply_preset,
            presets::preset_folder,
            presets::save_rack_preset,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start djmanzo");
}

/// Note each deck's hot cues, saving any that moved.
///
/// Runs on the snapshot thread, which already only fires when something
/// changed. The watcher compares and the writer does the I/O, so this costs one
/// comparison of eight optional floats per deck on a quiet tick.
fn save_changed_cues(
    snapshot: &Snapshot,
    tracks: &snapshot::DeckTracks,
    watcher: &std::sync::Mutex<persist::CueWatcher>,
    writer: &persist::LibraryWriter,
) {
    let Ok(mut watcher) = watcher.lock() else {
        return;
    };
    let names = tracks.lock().ok();
    for deck in &snapshot.decks {
        let track = names
            .as_ref()
            .and_then(|map| map.get(&deck.number))
            .map(|loaded| loaded.id);
        watcher.observe(deck.number, track, &deck.hot_cues, writer);
    }
}

/// When the application started, for the play watcher's monotonic deltas.
static START: std::sync::LazyLock<std::time::Instant> =
    std::sync::LazyLock::new(std::time::Instant::now);

/// Record any deck whose track has now been played far enough to count.
///
/// Runs beside the cue watcher on the snapshot thread, for the same reason:
/// the playhead is the audio thread's to report, and this is where the host
/// reads it. The threshold and the reasoning are in
/// [`persist::PlayWatcher`].
fn record_plays(
    snapshot: &Snapshot,
    tracks: &snapshot::DeckTracks,
    watcher: &std::sync::Mutex<persist::PlayWatcher>,
    session: &str,
    writer: &persist::LibraryWriter,
    audience: &Arc<audience::Audience>,
) -> Vec<context::Played> {
    let mut played_now = Vec::new();
    let Ok(mut watcher) = watcher.lock() else {
        return played_now;
    };
    // One reading for the whole pass, so every deck measures the same instant
    // and two decks cannot disagree about how long a frame took. Monotonic, so
    // a clock adjustment mid-set cannot invent or erase playback.
    let now = std::time::Instant::now()
        .duration_since(*START)
        .as_secs_f64();
    let names = tracks.lock().ok();
    for deck in &snapshot.decks {
        // The track, whether or not it is playing. Reporting `None` while
        // paused would make the watcher forget it, and every pause and resume
        // would be another row.
        let track = names
            .as_ref()
            .and_then(|map| map.get(&deck.number))
            .map(|loaded| loaded.id)
            .filter(|_| deck.loaded);

        if let Some(played) = watcher.observe(
            deck.number,
            track,
            deck.playing,
            f64::from(deck.position_seconds),
            f64::from(deck.length_seconds),
            now,
        ) {
            writer.send(persist::Write::Play {
                track: played,
                at: library::now_seconds(),
                session: Some(session.to_owned()),
            });
            // The moment a track counts as played is the moment to tick off
            // the request that wanted it. Doing this here rather than in the
            // panel means a DJ who never opens the panel still hands the room
            // a list that is true: a song asked for and played does not sit at
            // the top of the waiting list all night.
            let name = names
                .as_ref()
                .and_then(|map| map.get(&deck.number))
                .map(track_name);
            if let Some(id) = name
                .as_deref()
                .and_then(|name| audience.front().played(name))
            {
                tracing::debug!(id, ?name, "a request was played");
            }
            // And the moment it becomes evidence about the night. What the
            // analyser found about this record is already on the snapshot, so
            // the context engine is fed from the same reading the interface is
            // drawing rather than from a second trip to the library.
            played_now.push(context::Played {
                bpm: deck.analysis.as_ref().and_then(|a| a.bpm).map(f64::from),
                lufs: deck.analysis.as_ref().and_then(|a| a.lufs).map(f64::from),
            });
        }
    }
    played_now
}

/// How a loaded track is named when it is matched against what the room asked
/// for.
///
/// Artist and title, in the order somebody types them into a request box. The
/// matching folds this to nothing but letters and digits anyway, so the
/// separator is for reading rather than for matching.
pub(crate) fn track_name(loaded: &state::LoadedTrackInfo) -> String {
    match &loaded.artist {
        Some(artist) if !artist.trim().is_empty() => format!("{artist} - {}", loaded.title),
        _ => loaded.title.clone(),
    }
}

/// Write one snapshot to the path in `DJMANZO_SNAPSHOT_OUT`, then never again.
///
/// Development only, and off unless the variable is set. It exists so
/// `ui/e2e/`'s layout budget can be measured against a state the application
/// actually produced -- see `crates/dj-app/tests/e2e_fixture.rs`, which checks
/// the committed file still fits the current `Snapshot` type.
///
/// Written once rather than every frame: this runs on the pump's thread at
/// 60 Hz, and a file write there sixty times a second would be a benchmark of
/// the disk.
fn dump_snapshot(snapshot: &snapshot::Snapshot) {
    use std::sync::Once;
    static DONE: Once = Once::new();

    let Some(path) = std::env::var_os("DJMANZO_SNAPSHOT_OUT") else {
        return;
    };
    // Wait for a state worth capturing. The first frame of a run has a zero
    // sample rate and empty decks -- it is the "Waiting for the engine…"
    // screen, which has no crossfader on it, and a budget measured against it
    // would pass while every control was off the bottom of the window.
    if snapshot.master.sample_rate <= 0.0
        || snapshot.decks.iter().filter(|deck| deck.loaded).count() < 2
    {
        return;
    }
    DONE.call_once(|| match serde_json::to_string_pretty(snapshot) {
        Ok(text) => {
            if let Err(error) = std::fs::write(&path, format!("{text}\n")) {
                tracing::warn!(%error, ?path, "could not write the snapshot fixture");
            } else {
                tracing::info!(?path, "wrote a snapshot fixture");
            }
        }
        Err(error) => tracing::warn!(%error, "could not serialise the snapshot fixture"),
    });
}
