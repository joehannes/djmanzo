//! The real `djmanzo-listen`, run as the application runs it.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

const HELPER: &str = env!("CARGO_BIN_EXE_djmanzo-listen");

/// **The probe is the program itself answering**: the binary that would
/// ship starts, loads nothing, and says ready — or, on a processor it
/// cannot run on, why.
#[test]
fn the_helper_says_whether_this_machine_can_run_it() {
    let output = Command::new(HELPER)
        .arg("--probe")
        .stdin(Stdio::null())
        .output()
        .expect("the helper starts");
    assert!(output.status.success(), "{output:?}");
    let said: dj_whisper::Line =
        serde_json::from_slice(output.stdout.trim_ascii()).expect("a line of the protocol");
    match dj_whisper::processor_can_run() {
        Ok(()) => assert_eq!(said, dj_whisper::Line::Ready),
        Err(why) => assert_eq!(said, dj_whisper::Line::Refused(why)),
    }
}

/// Real speech, heard by a real model through the helper. Ignored: it needs
/// a model and a recording. Run with
/// `DJMANZO_WHISPER_MODEL=…/ggml-tiny-q5_1.bin DJMANZO_SPEECH=…/say.f32
/// DJMANZO_SPEECH_SAYS=moonlight cargo test -p dj-whisper -- --ignored`.
#[test]
#[ignore = "needs a Whisper model and a recording of speech"]
fn real_speech_is_heard_through_the_helper() {
    let model = std::env::var("DJMANZO_WHISPER_MODEL").expect("DJMANZO_WHISPER_MODEL");
    let audio = std::env::var("DJMANZO_SPEECH").expect("DJMANZO_SPEECH");
    let says = std::env::var("DJMANZO_SPEECH_SAYS").expect("DJMANZO_SPEECH_SAYS");
    let mut child = Command::new(HELPER)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("the helper starts");
    let request = serde_json::to_string(&dj_whisper::Request {
        ask: dj_whisper::Ask {
            mode: dj_whisper::Mode::Words,
            model: model.into(),
            id: "tiny".to_owned(),
            name: "Tiny".to_owned(),
            language: Some("en".to_owned()),
            prompt: None,
            threads: 4,
        },
        audio: audio.into(),
    })
    .expect("json");
    let mut input = child.stdin.take().expect("its input");
    writeln!(input, "{request}").expect("asked");
    let mut heard = None;
    for line in BufReader::new(child.stdout.take().expect("its output")).lines() {
        match serde_json::from_str(&line.expect("a line")).expect("the protocol") {
            dj_whisper::Line::Heard(raw) => {
                heard = Some(raw);
                break;
            }
            dj_whisper::Line::Refused(why) => panic!("{why}"),
            _ => {}
        }
    }
    drop(input);
    let _ = child.wait();
    let raw = heard.expect("an answer");
    let text: String = raw
        .pieces
        .iter()
        .flat_map(|piece| &piece.tokens)
        .filter(|token| !token.special)
        .map(|token| token.text.as_str())
        .collect();
    assert!(text.to_lowercase().contains(&says), "{text}");
}

/// **A helper whose application has gone stops working**: its input
/// closing mid-song is the application no longer wanting the answer, and it
/// leaves at once rather than finishing a transcription for nobody. Ignored
/// for the same reason as the one above, with the same variables; the
/// recording is repeated to a few minutes so there is a song to stop.
#[test]
#[ignore = "needs a Whisper model and a recording of speech"]
fn the_helper_stops_when_nobody_is_listening() {
    let model = std::env::var("DJMANZO_WHISPER_MODEL").expect("DJMANZO_WHISPER_MODEL");
    let speech = std::fs::read(std::env::var("DJMANZO_SPEECH").expect("DJMANZO_SPEECH"))
        .expect("the recording");
    let long = std::env::temp_dir().join(format!("dj-whisper-long-{}.f32", std::process::id()));
    std::fs::write(&long, speech.repeat(20)).expect("written");
    let mut child = Command::new(HELPER)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("the helper starts");
    let request = serde_json::to_string(&dj_whisper::Request {
        ask: dj_whisper::Ask {
            mode: dj_whisper::Mode::Words,
            model: model.into(),
            id: "tiny".to_owned(),
            name: "Tiny".to_owned(),
            language: Some("en".to_owned()),
            prompt: None,
            threads: 2,
        },
        audio: long.clone(),
    })
    .expect("json");
    let mut input = child.stdin.take().expect("its input");
    writeln!(input, "{request}").expect("asked");
    let mut first = String::new();
    BufReader::new(child.stdout.take().expect("its output"))
        .read_line(&mut first)
        .expect("it begins");
    assert!(first.contains("progress"), "{first}");
    let closed = std::time::Instant::now();
    drop(input);
    let status = child.wait().expect("it ends");
    let _ = std::fs::remove_file(long);
    assert_eq!(status.code(), Some(3), "{status}");
    assert!(
        closed.elapsed().as_secs_f64() < 2.0,
        "{:?}",
        closed.elapsed()
    );
}
