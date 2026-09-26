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
    /// The model loads but is not one djmanzo can separate with: its inputs
    /// and outputs are not a stereo mix in and stems out, or trying it on a
    /// segment failed.
    Unsuited {
        /// What the model declares, or what it said when tried.
        reason: String,
    },
}

/// Each message names the cause and stops there.
///
/// It used to end "so stems are unavailable", which was true when this was the
/// only outcome and stopped being true the day a built-in harmonic/percussive
/// separator was added as the fallback. The interface then read
/// "Using the built-in separator — ONNX Runtime could not be loaded, so stems
/// are unavailable", which contradicts itself in one sentence. The consequence
/// belongs to whoever knows whether there is a fallback, and that is the
/// caller, never this enum.
impl std::fmt::Display for Unavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Runtime { library, reason } => {
                write!(f, "ONNX Runtime ({library}) could not be loaded: {reason}")
            }
            Self::Model { path } => {
                write!(f, "there is no separation model at {}", path.display())
            }
            Self::Session { reason } => {
                write!(f, "the separation model would not load: {reason}")
            }
            Self::Unsuited { reason } => {
                write!(f, "the separation model does not fit djmanzo: {reason}")
            }
        }
    }
}

impl std::error::Error for Unavailable {}

/// The library name `ort` will try to open, given the value of
/// `ORT_DYLIB_PATH`, when no runtime was packaged with djmanzo.
///
/// Split out from the environment lookup so it can be tested without setting a
/// process-wide variable that every other test in the binary would see.
/// Mirrors `ort`'s own resolution, including treating an empty variable as
/// unset -- a shell that exports `ORT_DYLIB_PATH=` should behave like a shell
/// that never mentioned it.
#[must_use]
pub fn resolve_dylib_name(configured: Option<&str>) -> String {
    choose_library(configured, None)
}

/// Where the Linux packages install the ONNX Runtime they carry, under the
/// prefix the executable is installed in: `/usr/bin/djmanzo` finds
/// `/usr/lib/djmanzo/libonnxruntime.so.1`, and the same layout holds inside an
/// AppImage. `scripts/fetch-onnxruntime.sh` stages the file and
/// `crates/dj-app/tauri.linux.conf.json` installs it there.
///
/// A private directory, as Debian policy puts a library no other package
/// links against, and by its SONAME, the name the library gives itself.
pub const BUNDLED: [&str; 3] = ["lib", "djmanzo", "libonnxruntime.so.1"];

/// Where the Windows installers put it: in an `onnxruntime` folder beside
/// `djmanzo.exe`, which is Tauri's resource directory on Windows.
///
/// By full path rather than by name, because Windows 11 carries an older
/// `onnxruntime.dll` of its own in System32 for Windows ML, which `ort`
/// refuses as too old -- and a search by name can find that one first.
/// `scripts/fetch-onnxruntime.cjs` stages it and
/// `crates/dj-app/tauri.onnxruntime.conf.json` packages it.
pub const BUNDLED_WINDOWS: [&str; 2] = ["onnxruntime", "onnxruntime.dll"];

/// Where the Apple Silicon app carries it: `djmanzo.app/Contents/Resources/
/// onnxruntime/`, the bundle's resource directory, reached from
/// `Contents/MacOS/djmanzo`. The Intel build carries none: Microsoft stopped
/// publishing ONNX Runtime for Intel Macs after 1.23, older than this build
/// accepts, so there the built-in separator does the work.
pub const BUNDLED_MACOS: [&str; 3] = ["Resources", "onnxruntime", "libonnxruntime.1.dylib"];

/// Which package's layout to look for a carried runtime in.
///
/// A value rather than a `cfg!` in [`bundled_in`], so each layout is tested
/// on every machine the tests run on, not only the one that ships it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// The `.deb`, the `.rpm` and the AppImage: [`BUNDLED`].
    Linux,
    /// The `.msi` and the `.exe` installer: [`BUNDLED_WINDOWS`].
    Windows,
    /// The `.app`, inside a `.dmg` or not: [`BUNDLED_MACOS`].
    MacOs,
    /// Anywhere else, where no package carries one.
    Unpackaged,
}

impl Layout {
    /// The layout of the package this build is.
    #[must_use]
    pub const fn here() -> Self {
        if cfg!(target_os = "linux") {
            Self::Linux
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Unpackaged
        }
    }
}

/// Where a packaged runtime would be for an executable at `exe`, in the
/// package layout `layout`. `None` where no package carries one, and for an
/// executable too near the root for the layout to hold.
#[must_use]
pub fn bundled_in(layout: Layout, exe: &Path) -> Option<PathBuf> {
    let (base, parts): (&Path, &[&str]) = match layout {
        Layout::Linux => (exe.parent()?.parent()?, &BUNDLED),
        Layout::Windows => (exe.parent()?, &BUNDLED_WINDOWS),
        Layout::MacOs => (exe.parent()?.parent()?, &BUNDLED_MACOS),
        Layout::Unpackaged => return None,
    };
    Some(
        parts
            .iter()
            .fold(base.to_path_buf(), |path, part| path.join(part)),
    )
}

/// Where a packaged runtime would be for an executable at `exe`, in this
/// build's own layout. See [`bundled_in`].
#[must_use]
pub fn bundled_beside(exe: &Path) -> Option<PathBuf> {
    bundled_in(Layout::here(), exe)
}

/// Which library to open, in order: `ORT_DYLIB_PATH` when it is set, so a DJ
/// or a developer can always point at another; the runtime the package
/// carries, when it is there; and otherwise the platform's name, for the
/// loader to search for -- which is what a development build does.
#[must_use]
pub fn choose_library(configured: Option<&str>, bundled: Option<&Path>) -> String {
    match (configured, bundled) {
        (Some(path), _) if !path.is_empty() => path.to_owned(),
        (_, Some(bundled)) => bundled.display().to_string(),
        _ => default_dylib_name().to_owned(),
    }
}

/// The library this process will load ONNX Runtime from. See
/// [`choose_library`].
#[must_use]
pub fn runtime_library() -> String {
    let bundled = std::env::current_exe()
        .ok()
        .and_then(|exe| bundled_beside(&exe))
        .filter(|path| path.is_file());
    choose_library(
        std::env::var(DYLIB_PATH_VAR).ok().as_deref(),
        bundled.as_deref(),
    )
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
    probe_named_runtime(&runtime_library())
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

    /// **A packaged runtime is found where the package put it**, from the
    /// executable's own path — `/usr/bin` and inside an AppImage alike, the
    /// Windows install folder, the app bundle — without `LD_LIBRARY_PATH`, a
    /// symlink or a system copy; and `ORT_DYLIB_PATH` still wins over it when
    /// a DJ sets one.
    #[test]
    fn a_packaged_runtime_is_found_beside_the_executable() {
        assert_eq!(
            bundled_in(Layout::Linux, Path::new("/usr/bin/djmanzo")),
            Some(PathBuf::from("/usr/lib/djmanzo/libonnxruntime.so.1"))
        );
        assert_eq!(
            bundled_in(
                Layout::Linux,
                Path::new("/tmp/.mount_djmanzo/usr/bin/djmanzo")
            ),
            Some(PathBuf::from(
                "/tmp/.mount_djmanzo/usr/lib/djmanzo/libonnxruntime.so.1"
            ))
        );
        // Forward slashes, which both platforms read as separators, so the
        // Windows layout is checked on every machine.
        assert_eq!(
            bundled_in(
                Layout::Windows,
                Path::new("C:/Program Files/djmanzo/djmanzo.exe")
            ),
            Some(PathBuf::from(
                "C:/Program Files/djmanzo/onnxruntime/onnxruntime.dll"
            ))
        );
        assert_eq!(
            bundled_in(
                Layout::MacOs,
                Path::new("/Applications/djmanzo.app/Contents/MacOS/djmanzo")
            ),
            Some(PathBuf::from(
                "/Applications/djmanzo.app/Contents/Resources/onnxruntime/libonnxruntime.1.dylib"
            ))
        );
        assert_eq!(
            bundled_in(Layout::Unpackaged, Path::new("/usr/bin/djmanzo")),
            None
        );
        #[cfg(target_os = "linux")]
        assert_eq!(Layout::here(), Layout::Linux);
        #[cfg(target_os = "windows")]
        assert_eq!(Layout::here(), Layout::Windows);
        #[cfg(target_os = "macos")]
        assert_eq!(Layout::here(), Layout::MacOs);
        let packaged = Path::new("/usr/lib/djmanzo/libonnxruntime.so.1");
        assert_eq!(
            choose_library(None, Some(packaged)),
            "/usr/lib/djmanzo/libonnxruntime.so.1"
        );
        assert_eq!(
            choose_library(Some(""), Some(packaged)),
            packaged.display().to_string()
        );
        assert_eq!(
            choose_library(Some("/opt/ort/libonnxruntime.so"), Some(packaged)),
            "/opt/ort/libonnxruntime.so"
        );
        assert_eq!(choose_library(None, None), default_dylib_name());
    }

    /// **The file the application looks for is the file the packages
    /// install, of a version `ort` accepts.** Three places name it — this
    /// crate, the Linux bundle configuration and the script that fetches it —
    /// and a package that installs it anywhere else, or a script that pins an
    /// ONNX Runtime older than the C API this build asks for, ships a stem
    /// separator that cannot load.
    #[test]
    fn the_packaged_runtime_is_the_one_looked_for() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let config = std::fs::read_to_string(root.join("crates/dj-app/tauri.linux.conf.json"))
            .expect("the Linux bundle configuration");
        let installed = format!(
            "\"/usr/{}\": \"onnxruntime/{}\"",
            BUNDLED.join("/"),
            BUNDLED[2]
        );
        assert_eq!(
            config.matches(&installed).count(),
            3,
            "the .deb, the .rpm and the AppImage each install {installed}"
        );

        let script = std::fs::read_to_string(root.join("scripts/fetch-onnxruntime.sh"))
            .expect("the script that stages ONNX Runtime");
        let staged = format!("\"$stage/{}\"", BUNDLED[2]);
        assert!(
            script
                .lines()
                .any(|line| line.starts_with("cp ") && line.ends_with(&staged)),
            "the script does not copy the library to {staged}"
        );
        let version = script
            .lines()
            .find_map(|line| line.strip_prefix("VERSION="))
            .expect("a pinned version");
        let minor: u32 = version
            .split('.')
            .nth(1)
            .and_then(|m| m.parse().ok())
            .expect("1.x.y");
        assert!(
            version.starts_with("1.") && minor >= ort::MINOR_VERSION,
            "ONNX Runtime {version} is older than the C API 1.{} this build asks for",
            ort::MINOR_VERSION
        );
    }

    /// **The Windows and Apple Silicon packages carry the file looked for,
    /// of the version the Linux ones carry.** The script that stages them
    /// names each file, the configuration packages the folder it stages
    /// into, and this crate looks for it there; the three drifting apart is a
    /// package whose separator cannot load, found by a DJ.
    #[test]
    fn the_windows_and_mac_runtimes_are_the_ones_looked_for() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let script = std::fs::read_to_string(root.join("scripts/fetch-onnxruntime.cjs"))
            .expect("the script that stages the Windows and macOS runtimes");
        for name in [BUNDLED_WINDOWS[1], BUNDLED_MACOS[2]] {
            assert!(
                script
                    .lines()
                    .any(|line| line.trim_end().ends_with(&format!(": \"{name}\","))),
                "the script stages nothing as {name}"
            );
        }
        // Staged into the folder both layouts put in the package.
        assert_eq!(BUNDLED_WINDOWS[0], "onnxruntime");
        assert_eq!(BUNDLED_MACOS[1], "onnxruntime");
        assert!(script.contains(r#""../crates/dj-app/onnxruntime""#));
        let config =
            std::fs::read_to_string(root.join("crates/dj-app/tauri.onnxruntime.conf.json"))
                .expect("the configuration the release passes to those builds");
        assert!(
            config.contains(r#""resources": ["onnxruntime/*"]"#),
            "{config}"
        );
        assert!(
            config.contains("node ../scripts/fetch-onnxruntime.cjs"),
            "{config}"
        );

        // One version everywhere, the one `ort` accepts.
        let linux = std::fs::read_to_string(root.join("scripts/fetch-onnxruntime.sh"))
            .expect("the Linux script");
        let pinned = |source: &str, prefix: &str| {
            source
                .lines()
                .find_map(|line| line.strip_prefix(prefix))
                .map(|rest| rest.trim_matches(|c| c == '"' || c == ';').to_owned())
                .expect("a pinned version")
        };
        assert_eq!(
            pinned(&script, "const VERSION = "),
            pinned(&linux, "VERSION=")
        );

        // And the release gives the configuration to exactly the builds it
        // has a runtime for.
        let release = std::fs::read_to_string(root.join(".github/workflows/release.yml"))
            .expect("the release workflow");
        let carrying: Vec<&str> = release
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .filter(|line| line.contains("--config tauri.onnxruntime.conf.json"))
            .collect();
        assert_eq!(carrying.len(), 2, "{carrying:#?}");
        assert!(
            carrying
                .iter()
                .any(|line| line.contains("aarch64-apple-darwin"))
        );
        assert!(carrying.iter().any(|line| line.contains("windows-latest")));
        assert!(
            !carrying
                .iter()
                .any(|line| line.contains("x86_64-apple-darwin"))
        );
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

    /// **A cause, never a consequence.**
    ///
    /// These sentences are shown in two places that mean opposite things: one
    /// where separation really is off, and one where the built-in separator
    /// took over and the controls all work. A message that decides the
    /// consequence for itself is wrong in one of them, and it was: the
    /// fallback line read "Using the built-in separator — ONNX Runtime could
    /// not be loaded, so stems are unavailable".
    #[test]
    fn no_message_claims_stems_are_unavailable() {
        let messages = [
            Unavailable::Runtime {
                library: "libonnxruntime.so".to_owned(),
                reason: "no such file".to_owned(),
            }
            .to_string(),
            Unavailable::Model {
                path: PathBuf::from("/models/htdemucs.onnx"),
            }
            .to_string(),
            Unavailable::Session {
                reason: "opset 18 unsupported".to_owned(),
            }
            .to_string(),
        ];
        for message in messages {
            assert!(
                !message.contains("unavailable"),
                "the cause decided the consequence: {message}"
            );
        }
    }
}
