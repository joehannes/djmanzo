//! `djmanzo-listen`: whisper.cpp in a process of its own, for a djmanzo
//! built without it. See the library's documentation for the protocol.

use std::io::{BufRead, Read};
use std::sync::{Arc, Mutex};

fn main() {
    let probe = std::env::args().skip(1).any(|arg| arg == "--probe");
    let output = Arc::new(Mutex::new(std::io::stdout()));
    if probe {
        std::process::exit(dj_whisper::serve(
            true,
            &mut std::io::empty(),
            output,
            dj_whisper::transcribe,
        ));
    }
    // The request is the first line, read under a lock that is let go of at
    // once: the watcher below needs standard input to itself.
    let mut first = String::new();
    let read = std::io::stdin().lock().read_line(&mut first);
    if read.is_ok() {
        // After the request, the pipe staying open is the application still
        // wanting the answer. When it closes nobody is listening for one,
        // and whisper.cpp should not go on working.
        std::thread::spawn(|| {
            let mut rest = Vec::new();
            let _ = std::io::stdin().lock().read_to_end(&mut rest);
            std::process::exit(3);
        });
    }
    std::process::exit(dj_whisper::serve(
        false,
        &mut first.as_bytes(),
        output,
        dj_whisper::transcribe,
    ));
}
