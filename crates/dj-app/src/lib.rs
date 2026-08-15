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

pub mod commands;
pub mod host;
pub mod snapshot;
pub mod state;
pub mod waveform;

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

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            match waveforms.tile_png(key, &dj_render::Palette::default()) {
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
        .manage(state)
        .setup(move |app| {
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

            let pump = SnapshotPump::start(registry, deck_count, move |snapshot| {
                use tauri::Emitter;
                if let Err(error) = handle.emit("snapshot", &snapshot) {
                    tracing::warn!(%error, "failed to emit snapshot");
                }
            });
            // Hand the pump to Tauri so it lives as long as the app does.
            app.manage(pump);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_devices,
            commands::open_device,
            commands::start_audio,
            commands::stop_audio,
            commands::load_track,
            commands::dispatch,
            commands::get_snapshot,
            commands::waveform_info,
            commands::report_bench,
            commands::session_log,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start djmanzo");
}
