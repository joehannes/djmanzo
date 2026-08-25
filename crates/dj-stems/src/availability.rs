//! Whether separation can run at all, and -- when it cannot -- why.
//!
//! # Why this module exists
//!
//! `ort` loads ONNX Runtime dynamically, and when the library is missing it
//! does not return an error: it panics inside a `Once`, which poisons `ort`'s
//! own environment mutex. The poisoned mutex is then touched again by an
//! `atexit` handler that is declared `extern "C"` and therefore cannot unwind,
//! so the *process aborts on exit* -- long after, and far away from, the code
//! that asked for a stem.
//!
//! That failure mode is not recoverable and not catchable: `catch_unwind`
//! around the first call still leaves the mutex poisoned for the exit handler.
//! The only safe move is to never reach it. So this module answers "can
//! separation run?" without touching `ort` at all, by resolving the library
//! exactly the way `ort` would and opening it ourselves.
//!
//! A DJ on a laptop with no ONNX Runtime should get a mixer with the stem
//! controls greyed out and a sentence saying why -- not a crash, and not a
//! silent no-op.

use std::path::{Path, PathBuf};

/// The environment variable `ort` reads to find ONNX Runtime.
const DYLIB_PATH_VAR: &str = "ORT_DYLIB_PATH";

/// The symbol `ort` looks up once the library is open. Present in every real
/// ONNX Runtime; absent from anything else that happens to share the name.
const ENTRY_SYMBOL: &[u8] = b"OrtGetApiBase\0";

/// Why stem separation is not available.
///
/// Each variant is a different job for whoever reads it: the runtime is a
/// packaging problem, the model is a download, and a session failure is a
/// broken or mismatched file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unavailable {
    /// ONNX Runtime could not be loaded.
    Runtime {
        /// What we tried to open, after resolving `ORT_DYLIB_PATH`.
        library: String,
        /// What the loader said.
        reason: String,
    },
    /// The runtime is here, but there is no model to run.
    Model {
        /// Where we looked.
        path: PathBuf,
    },
    /// Both are here, but the model would not load.
    Session {
        /// What ONNX Runtime said.
        reason: String,
    },
}

impl std::fmt::Display for Unavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Runtime { library, reason } => write!(
                f,
                "ONNX Runtime ({library}) could not be loaded, so stems are unavailable: {reason}"
            ),
            Self::Model { path } => write!(
                f,
                "no separation model at {}, so stems are unavailable",
                path.display()
            ),
            Self::Session { reason } => {
                write!(f, "the separation model would not load: {reason}")
            }
        }
    }
}

impl std::error::Error for Unavailable {}

/// The library name `ort` will try to open, given the value of
/// `ORT_DYLIB_PATH`.
///
/// Split out from the environment lookup so it can be tested without setting a
/// process-wide variable that every other test in the binary would see.
/// Mirrors `ort`'s own resolution, including treating an empty variable as
/// unset -- a shell that exports `ORT_DYLIB_PATH=` should behave like a shell
/// that never mentioned it.
#[must_use]
pub fn resolve_dylib_name(configured: Option<&str>) -> String {
    match configured {
        Some(path) if !path.is_empty() => path.to_owned(),
        _ => default_dylib_name().to_owned(),
    }
}

/// The platform's ONNX Runtime file name, as `ort` spells it.
#[must_use]
pub const fn default_dylib_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "onnxruntime.dll"
    }
    #[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
    {
        "libonnxruntime.so"
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        "libonnxruntime.dylib"
    }
}

/// Can ONNX Runtime be loaded?
///
/// Opens the library and looks up the entry symbol, which is the pair of steps
/// `ort` panics on. Doing them here means a missing or broken runtime comes
/// back as an `Err` instead of taking the process down at exit.
///
/// The library handle is dropped immediately. That is deliberate: this asks a
/// question, it does not take ownership of the answer. `ort` opens the library
/// again itself, and on every platform we target `dlopen` of an already-loaded
/// library is refcounted and cheap.
pub fn probe_runtime() -> Result<(), Unavailable> {
    let library = resolve_dylib_name(std::env::var(DYLIB_PATH_VAR).ok().as_deref());
    probe_named_runtime(&library)
}

/// [`probe_runtime`] against a name chosen by the caller, so a test can ask
/// about a library it knows is not there.
pub fn probe_named_runtime(library: &str) -> Result<(), Unavailable> {
    // SAFETY: opening a shared library runs its initialisers, which is exactly
    // what `ort` is about to do anyway. We look up one symbol and drop the
    // handle without calling anything.
    let opened = unsafe { libloading::Library::new(library) };
    let handle = opened.map_err(|error| Unavailable::Runtime {
        library: library.to_owned(),
        reason: error.to_string(),
    })?;

    // A file with the right name is not necessarily ONNX Runtime, and `ort`
    // panics on the missing symbol just as hard as on the missing file.
    let symbol: Result<libloading::Symbol<'_, unsafe extern "C" fn() -> *const ()>, _> =
        unsafe { handle.get(ENTRY_SYMBOL) };
    symbol.map(|_| ()).map_err(|error| Unavailable::Runtime {
        library: library.to_owned(),
        reason: error.to_string(),
    })
}

/// Is there a model file to run?
///
/// Checked separately from the runtime because they fail for different reasons
/// and are fixed in different ways.
pub fn probe_model(path: &Path) -> Result<(), Unavailable> {
    if path.is_file() {
        Ok(())
    } else {
        Err(Unavailable::Model {
            path: path.to_path_buf(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unset_variable_falls_back_to_the_platform_name() {
        assert_eq!(resolve_dylib_name(None), default_dylib_name());
    }

    /// `ort` treats `ORT_DYLIB_PATH=` as unset. If we disagreed with it we
    /// would probe one library and then let `ort` panic loading another.
    #[test]
    fn an_empty_variable_is_the_same_as_an_unset_one() {
        assert_eq!(resolve_dylib_name(Some("")), default_dylib_name());
    }

    #[test]
    fn a_configured_path_wins() {
        assert_eq!(
            resolve_dylib_name(Some("/opt/onnx/libonnxruntime.so")),
            "/opt/onnx/libonnxruntime.so"
        );
    }

    /// The default has to be the name `ort` uses, or the probe answers a
    /// different question from the one that panics.
    #[test]
    fn the_default_name_matches_the_platform() {
        let name = default_dylib_name();
        assert!(name.contains("onnxruntime"), "{name}");
        #[cfg(target_os = "linux")]
        assert_eq!(name, "libonnxruntime.so");
        #[cfg(target_os = "macos")]
        assert_eq!(name, "libonnxruntime.dylib");
        #[cfg(target_os = "windows")]
        assert_eq!(name, "onnxruntime.dll");
    }

    #[test]
    fn a_library_that_is_not_there_is_reported_not_panicked() {
        let error = probe_named_runtime("libdefinitely-not-onnxruntime-97f3.so")
            .expect_err("nothing by that name can exist");
        match error {
            Unavailable::Runtime { library, .. } => {
                assert_eq!(library, "libdefinitely-not-onnxruntime-97f3.so");
            }
            other => panic!("expected a runtime failure, got {other:?}"),
        }
    }

    /// A real library without ONNX Runtime's entry point must be refused too.
    /// `ort` looks the symbol up with `.expect()`, so accepting the file here
    /// would only move the panic later.
    #[test]
    fn a_library_without_the_entry_symbol_is_refused() {
        // libc is present wherever these tests run and certainly is not ONNX
        // Runtime. If it cannot be opened by this name the check is vacuous,
        // so skip rather than assert something the platform did not answer.
        let candidates = ["libc.so.6", "libSystem.B.dylib", "kernel32.dll"];
        let Some(opened) = candidates
            .into_iter()
            .find(|name| unsafe { libloading::Library::new(*name) }.is_ok())
        else {
            return;
        };
        let error = probe_named_runtime(opened)
            .expect_err("libc does not export ONNX Runtime's entry point");
        assert!(matches!(error, Unavailable::Runtime { .. }), "{error:?}");
    }

    #[test]
    fn a_missing_model_names_the_path_it_looked_for() {
        let path = Path::new("/nowhere/htdemucs.onnx");
        let error = probe_model(path).expect_err("that path does not exist");
        assert_eq!(
            error,
            Unavailable::Model {
                path: path.to_path_buf()
            }
        );
        assert!(error.to_string().contains("/nowhere/htdemucs.onnx"));
    }

    #[test]
    fn a_model_that_is_there_passes() {
        let dir = std::env::temp_dir().join("djmanzo-stems-probe-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("model.onnx");
        std::fs::write(&path, b"not really a model, but it is a file").expect("write");
        assert_eq!(probe_model(&path), Ok(()));
        let _ = std::fs::remove_file(&path);
    }

    /// A directory is not a model, and `is_file` is what makes that true.
    #[test]
    fn a_directory_is_not_a_model() {
        let dir = std::env::temp_dir();
        assert!(
            probe_model(&dir).is_err(),
            "{} is a directory",
            dir.display()
        );
    }

    /// Each message has to name the thing the reader has to go and fix.
    #[test]
    fn every_reason_says_what_to_do_about_it() {
        let runtime = Unavailable::Runtime {
            library: "libonnxruntime.so".to_owned(),
            reason: "no such file".to_owned(),
        };
        assert!(runtime.to_string().contains("libonnxruntime.so"));
        assert!(runtime.to_string().contains("no such file"));

        let session = Unavailable::Session {
            reason: "opset 18 unsupported".to_owned(),
        };
        assert!(session.to_string().contains("opset 18 unsupported"));
    }
}
