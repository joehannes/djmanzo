//! Whether djmanzo can listen on this machine, and how.
//!
//! Speech recognition — §122's words in time and A2's spoken commands — is
//! whisper.cpp. It is either compiled in, or, in the karaoke build for older
//! Macs, it is a helper program shipped beside the application
//! (`djmanzo-listen`, [`dj_whisper`]) and built for the macOS it needs. Which
//! one, and whether either can run here, is decided once, here, and every
//! panel reads the answer: [`can`].
//!
//! # Checked by running it
//!
//! The owner asked for the karaoke build to find out for itself rather than
//! leave it out for every Mac because one of them is from 2011. The surest
//! way to know whether a program runs on a machine is to run it, so the
//! check is the helper's own `--probe`, which loads nothing and answers in a
//! line: ready, or why not (a processor without AVX2). A macOS too old to
//! start it at all is read first, from `sw_vers`, and said in a sentence
//! rather than as a loader's error. The answer is kept for the run; Settings
//! can ask again.
//!
//! # Automatic, or off
//!
//! On by itself wherever it can run, because that is what the DJ downloaded
//! the models for; off in Settings for a DJ who would rather it never
//! listened, which every panel then says in the same sentence.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

/// How djmanzo listens, where it can.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Engine {
    /// whisper.cpp compiled into this program.
    BuiltIn,
    /// The helper beside it, which this machine has been seen to run.
    Helper(PathBuf),
}

/// The DJ's choice, kept in `listening.json` beside the settings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Setting {
    /// Off in Settings: nothing listens. Automatic otherwise.
    #[serde(default)]
    pub off: bool,
}

/// What every panel says when the DJ has turned it off.
pub const OFF: &str = "speech recognition is off in Settings; lyrics from LRCLIB and the record's own tags still work";

/// The oldest macOS the helper is built for.
pub const HELPER_MACOS: (u32, u32) = (10, 15);

static SWITCHED_OFF: AtomicBool = AtomicBool::new(false);

/// The probe's answer, kept for the run: which helper, and what it said.
static PROBED: Mutex<Option<(PathBuf, Result<(), String>)>> = Mutex::new(None);

/// Turn listening off, or back to automatic. Called with the stored choice
/// once the settings folder is known, and whenever the DJ changes it.
pub fn set_off(off: bool) {
    SWITCHED_OFF.store(off, Ordering::Relaxed);
}

/// Whether the DJ has turned it off.
#[must_use]
pub fn is_off() -> bool {
    SWITCHED_OFF.load(Ordering::Relaxed)
}

/// Forget the probe's answer, so the next question asks the machine again.
pub fn recheck() {
    if let Ok(mut probed) = PROBED.lock() {
        *probed = None;
    }
}

/// The one decision, apart from the machine so it can be tested: the DJ's
/// switch first, then whisper.cpp compiled in (whose processor check is the
/// answer — the helper would be the same whisper.cpp), then the helper, and
/// with neither the sentence the build without whisper.cpp has always said.
///
/// # Errors
/// Why it cannot listen, in a sentence a panel can show as it is.
pub fn decide(
    off: bool,
    built_in: Option<Result<(), String>>,
    helper: Option<(PathBuf, Result<(), String>)>,
) -> Result<Engine, String> {
    if off {
        return Err(OFF.to_owned());
    }
    match (built_in, helper) {
        (Some(Ok(())), _) => Ok(Engine::BuiltIn),
        (Some(Err(why)), _) | (None, Some((_, Err(why)))) => Err(why),
        (None, Some((path, Ok(())))) => Ok(Engine::Helper(path)),
        (None, None) => Err(crate::whispercpp::NOT_BUILT_IN.to_owned()),
    }
}

/// Whether djmanzo can listen here, and how.
///
/// # Errors
/// As [`decide`].
pub fn can() -> Result<Engine, String> {
    let (built_in, helper) = this_machine();
    decide(is_off(), built_in, helper)
}

/// What this machine can do, whatever the switch says — for Settings, which
/// should say whether a DJ who turned it off could turn it back on.
///
/// # Errors
/// As [`decide`], never with [`OFF`].
pub fn machine() -> Result<Engine, String> {
    let (built_in, helper) = this_machine();
    decide(false, built_in, helper)
}

type Found = (
    Option<Result<(), String>>,
    Option<(PathBuf, Result<(), String>)>,
);

/// whisper.cpp compiled in and its processor check, or the helper beside
/// this program and what its probe said.
fn this_machine() -> Found {
    let built_in = dj_whisper::BUILT_IN.then(dj_whisper::processor_can_run);
    let helper = if built_in.is_some() {
        None
    } else {
        helper_path().map(|path| {
            let answer = probed(&path);
            (path, answer)
        })
    };
    (built_in, helper)
}

/// The helper beside this program's own executable, if it was shipped.
#[must_use]
pub fn helper_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let path = exe.with_file_name(dj_whisper::HELPER);
    path.is_file().then_some(path)
}

/// The probe's answer for `path`, asked once a run.
fn probed(path: &Path) -> Result<(), String> {
    if let Ok(probed) = PROBED.lock()
        && let Some((asked, answer)) = probed.as_ref()
        && asked == path
    {
        return answer.clone();
    }
    let answer = probe(path);
    if let Ok(mut probed) = PROBED.lock() {
        *probed = Some((path.to_path_buf(), answer.clone()));
    }
    answer
}

/// Ask the helper whether it can run here, by running it.
fn probe(path: &Path) -> Result<(), String> {
    if let Some(version) = macos() {
        too_old(version)?;
    }
    let output = std::process::Command::new(path)
        .arg("--probe")
        .stdin(std::process::Stdio::null())
        .output();
    read_probe(output)
}

/// A macOS older than the helper is built for, said as itself rather than
/// as the loader's error.
///
/// # Errors
/// That macOS, and what is needed.
pub fn too_old((major, minor): (u32, u32)) -> Result<(), String> {
    if (major, minor) < HELPER_MACOS {
        return Err(format!(
            "this Mac runs macOS {major}.{minor}, and djmanzo's speech recognition needs {}.{} or later; lyrics from LRCLIB and the record's own tags still work",
            HELPER_MACOS.0, HELPER_MACOS.1
        ));
    }
    Ok(())
}

/// The probe's output as an answer.
///
/// # Errors
/// The helper's own reason, or why it did not answer.
pub fn read_probe(output: std::io::Result<std::process::Output>) -> Result<(), String> {
    let output =
        output.map_err(|e| format!("the speech recognition helper would not start: {e}"))?;
    let said = String::from_utf8_lossy(&output.stdout);
    match said
        .lines()
        .find_map(|line| serde_json::from_str::<dj_whisper::Line>(line.trim()).ok())
    {
        Some(dj_whisper::Line::Ready) => Ok(()),
        Some(dj_whisper::Line::Refused(why)) => Err(why),
        _ => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let first = stderr
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or("it gave no answer");
            Err(format!(
                "the speech recognition helper would not start here: {first}"
            ))
        }
    }
}

/// This Mac's macOS version, from `sw_vers`; none elsewhere.
fn macos() -> Option<(u32, u32)> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let output = std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()?;
    version_of(&String::from_utf8_lossy(&output.stdout))
}

/// `10.13.6` as `(10, 13)`.
#[must_use]
pub fn version_of(text: &str) -> Option<(u32, u32)> {
    let mut parts = text.trim().split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().map_or(Some(0), |minor| minor.parse().ok())?;
    Some((major, minor))
}

/// Listen through the helper: the audio into a file beside nothing else,
/// the request on its input, progress and the answer from its output.
///
/// # Errors
/// The helper's refusal, or why it stopped before it answered.
pub fn through_helper(
    helper: &Path,
    ask: dj_whisper::Ask,
    audio: &[f32],
    mut progress: impl FnMut(i32),
) -> Result<dj_whisper::Raw, String> {
    static TAKES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let take = TAKES.fetch_add(1, Ordering::Relaxed);
    let file =
        std::env::temp_dir().join(format!("djmanzo-listen-{}-{take}.f32", std::process::id()));
    std::fs::write(&file, dj_whisper::bytes_of(audio))
        .map_err(|e| format!("{}: {e}", file.display()))?;
    let answer = converse(helper, ask, &file, &mut progress);
    let _ = std::fs::remove_file(&file);
    answer
}

fn converse(
    helper: &Path,
    ask: dj_whisper::Ask,
    audio: &Path,
    progress: &mut dyn FnMut(i32),
) -> Result<dj_whisper::Raw, String> {
    let request = serde_json::to_string(&dj_whisper::Request {
        ask,
        audio: audio.to_path_buf(),
    })
    .map_err(|e| e.to_string())?;
    let mut child = std::process::Command::new(helper)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("the speech recognition helper would not start: {e}"))?;
    // Held open until the answer is in: the helper leaves when it closes.
    let mut input = child.stdin.take();
    if let Some(input) = input.as_mut() {
        writeln!(input, "{request}").map_err(|e| e.to_string())?;
        input.flush().map_err(|e| e.to_string())?;
    }
    // Drained apart, so a helper that writes much there cannot stall on a
    // full pipe while the answer waits.
    let errors = child.stderr.take().map(|stderr| {
        std::thread::spawn(move || {
            let mut kept = String::new();
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if kept.len() < 4096 {
                    kept.push_str(&line);
                    kept.push('\n');
                }
            }
            kept
        })
    });
    let mut answer = None;
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            match serde_json::from_str::<dj_whisper::Line>(line.trim()) {
                Ok(dj_whisper::Line::Progress(percent)) => progress(percent),
                Ok(dj_whisper::Line::Heard(raw)) => answer = Some(Ok(raw)),
                Ok(dj_whisper::Line::Refused(why)) => answer = Some(Err(why)),
                Ok(dj_whisper::Line::Ready) | Err(_) => {}
            }
        }
    }
    drop(input);
    let status = child.wait().map_err(|e| e.to_string())?;
    let errors = errors
        .and_then(|thread| thread.join().ok())
        .unwrap_or_default();
    answer.unwrap_or_else(|| {
        let last = errors
            .lines()
            .map(str::trim)
            .rfind(|line| !line.is_empty())
            .unwrap_or("nothing said");
        Err(format!(
            "speech recognition stopped before it answered ({status}): {last}"
        ))
    })
}

/// What Settings shows: the switch, and what the machine was found to do
/// — whatever the switch says, so a DJ who turned it off can see whether
/// turning it back on would do anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Status {
    pub off: bool,
    /// `built-in` or `helper` where this machine can, none where it cannot.
    pub how: Option<&'static str>,
    /// Why this machine cannot, when it cannot.
    pub why: Option<String>,
}

impl Status {
    /// The status for a switch and what the machine was found to do.
    #[must_use]
    pub fn of(off: bool, machine: Result<Engine, String>) -> Self {
        Self {
            off,
            how: machine.as_ref().ok().map(|engine| match engine {
                Engine::BuiltIn => "built-in",
                Engine::Helper(_) => "helper",
            }),
            why: machine.err(),
        }
    }
}

/// The status, asked now.
#[must_use]
pub fn status() -> Status {
    Status::of(is_off(), machine())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The decision, in its order** — the DJ's switch over everything,
    /// whisper.cpp compiled in over the helper, the helper where the machine
    /// ran it, its own reason where it would not, and the old sentence where
    /// there is neither.
    #[test]
    fn listening_is_decided_in_one_order() {
        let helper =
            PathBuf::from("/Applications/djmanzo Karaoke.app/Contents/MacOS/djmanzo-listen");
        let ran = || Some((helper.clone(), Ok(())));
        assert_eq!(decide(true, Some(Ok(())), None), Err(OFF.to_owned()));
        assert_eq!(decide(true, None, ran()), Err(OFF.to_owned()));
        assert_eq!(decide(false, Some(Ok(())), None), Ok(Engine::BuiltIn));
        assert_eq!(
            decide(false, Some(Err("no AVX2".to_owned())), ran()),
            Err("no AVX2".to_owned()),
            "the helper is the same whisper.cpp, on the same processor"
        );
        assert_eq!(
            decide(false, None, ran()),
            Ok(Engine::Helper(helper.clone()))
        );
        assert_eq!(
            decide(false, None, Some((helper, Err("macOS 10.13".to_owned())))),
            Err("macOS 10.13".to_owned())
        );
        assert_eq!(
            decide(false, None, None),
            Err(crate::whispercpp::NOT_BUILT_IN.to_owned())
        );
    }

    /// **Settings says what the machine can do whatever the switch says**:
    /// a DJ who turned it off sees whether turning it back on would listen.
    #[test]
    fn settings_say_what_the_machine_can_whatever_the_switch() {
        let helper = PathBuf::from("/x/djmanzo-listen");
        let off_but_able = Status::of(true, decide(false, None, Some((helper, Ok(())))));
        assert_eq!(
            (off_but_able.off, off_but_able.how, off_but_able.why),
            (true, Some("helper"), None)
        );
        let unable = Status::of(false, decide(false, None, None));
        assert_eq!(unable.how, None);
        assert_eq!(unable.why.as_deref(), Some(crate::whispercpp::NOT_BUILT_IN));
    }

    /// **A Mac too old for the helper is told so in words**, before the
    /// loader is asked: 10.13 and 10.14 cannot, 10.15 and every later one
    /// can — including the two-part numbers macOS has used since 11.
    #[test]
    fn a_macos_too_old_is_said_as_itself() {
        assert_eq!(version_of("10.13.6\n"), Some((10, 13)));
        assert_eq!(version_of("12.7.4"), Some((12, 7)));
        assert_eq!(version_of("15"), Some((15, 0)));
        assert_eq!(version_of("unknown"), None);
        let said = too_old((10, 13)).expect_err("a 2011 Mac's last macOS");
        assert!(said.contains("10.13") && said.contains("10.15"), "{said}");
        assert!(too_old((10, 14)).is_err());
        assert_eq!(too_old((10, 15)), Ok(()));
        assert_eq!(too_old((11, 0)), Ok(()));
        assert_eq!(too_old((15, 2)), Ok(()));
    }

    #[cfg(unix)]
    fn output(stdout: &str, stderr: &str) -> std::io::Result<std::process::Output> {
        use std::os::unix::process::ExitStatusExt;
        Ok(std::process::Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        })
    }

    /// A stand-in helper: a script that reads the request's line and says
    /// what it is given to say, as the real one would.
    #[cfg(unix)]
    fn stand_in(name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("djmanzo-listening-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a folder");
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\nread request\n{body}\n")).expect("written");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("runnable");
        path
    }

    #[cfg(unix)]
    fn ask() -> dj_whisper::Ask {
        dj_whisper::Ask {
            mode: dj_whisper::Mode::Words,
            model: PathBuf::from("/models/ggml-base-q5_1.bin"),
            id: "base".to_owned(),
            name: "Base".to_owned(),
            language: None,
            prompt: None,
            threads: 2,
        }
    }

    /// **The helper's answer reaches the panel as it was given** — progress
    /// as it comes, then whisper's pieces; a refusal as its own sentence; a
    /// helper that dies before answering as that, with the last thing it
    /// said, never as an empty success. And the audio file is gone after.
    #[cfg(unix)]
    #[test]
    fn the_helper_is_listened_to_as_it_answers() {
        let raw = dj_whisper::Raw {
            language: "es".to_owned(),
            pieces: vec![dj_whisper::Piece {
                start: 0.5,
                end: 2.0,
                text: String::new(),
                tokens: vec![dj_whisper::Token {
                    text: " Vivir".to_owned(),
                    special: false,
                    at: 0.6,
                }],
            }],
            load_seconds: 0.1,
            listen_seconds: 0.9,
        };
        let heard = serde_json::to_string(&dj_whisper::Line::Heard(raw.clone())).expect("json");
        let answers = stand_in(
            "answers",
            &format!("echo '{{\"progress\":40}}'\necho '{{\"progress\":100}}'\necho '{heard}'"),
        );
        let mut told = Vec::new();
        assert_eq!(
            through_helper(&answers, ask(), &[0.0; 160], |percent| told.push(percent)),
            Ok(raw)
        );
        assert_eq!(told, [40, 100]);

        let refuses = stand_in("refuses", "echo '{\"refused\":\"Base would not load\"}'");
        assert_eq!(
            through_helper(&refuses, ask(), &[0.0; 16], |_| {}),
            Err("Base would not load".to_owned())
        );

        let dies = stand_in("dies", "echo 'ggml: out of memory' >&2\nexit 9");
        let said = through_helper(&dies, ask(), &[0.0; 16], |_| {}).expect_err("no answer");
        assert!(
            said.contains("stopped before it answered") && said.contains("out of memory"),
            "{said}"
        );

        let left: Vec<_> = std::fs::read_dir(std::env::temp_dir())
            .expect("the temporary folder")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(&format!("djmanzo-listen-{}-", std::process::id()))
            })
            .collect();
        assert!(left.is_empty(), "audio left behind: {left:?}");
    }

    /// **The probe's answer is the helper's own**: ready, its reason, or —
    /// where it said nothing, as a binary the loader refuses says nothing —
    /// the loader's first line, never silence.
    #[cfg(unix)]
    #[test]
    fn the_probe_is_read_as_the_helper_answered() {
        assert_eq!(read_probe(output("\"ready\"\n", "")), Ok(()));
        assert_eq!(
            read_probe(output("{\"refused\":\"this processor lacks AVX2\"}\n", "")),
            Err("this processor lacks AVX2".to_owned())
        );
        let refused = read_probe(output(
            "",
            "dyld: Symbol not found: __ZNSt3__14__fs10filesystem\n  Referenced from: djmanzo-listen\n",
        ))
        .expect_err("a helper the loader would not start");
        assert!(refused.contains("Symbol not found"), "{refused}");
        assert!(
            read_probe(Err(std::io::Error::other("no such file")))
                .expect_err("no helper")
                .contains("no such file")
        );
    }
}
