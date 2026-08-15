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

pub use host::{AudioHost, HostError};
pub use snapshot::{Snapshot, SnapshotPump};
pub use state::AppState;

use tauri::Manager;

/// Environment variable that forces the headless audio backend.
///
/// Set by CI, and useful on any machine with no working sound device -- the
/// application still starts, so the UI can be exercised.
pub const NULL_BACKEND_ENV: &str = "DJMANZO_NULL_AUDIO";

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

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .setup(move |app| {
            let handle = app.handle().clone();
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
            commands::session_log,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start djmanzo");
}
