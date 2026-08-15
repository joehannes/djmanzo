//! End-to-end decoding against real files on disk.
//!
//! The unit tests in `decoder.rs` cover the helpers -- channel folding, hashing.
//! They do not prove that Symphonia is wired up correctly, that a container is
//! parsed, or that the samples come out in the right order and the right scale.
//! This does, by writing WAV files byte by byte and decoding them back.

use dj_decode::{DecodeError, decode_file};
use std::io::Write;
use std::path::PathBuf;

/// Build a minimal PCM WAV. No dependencies: the header is 44 bytes and writing
/// it by hand is less trouble than pulling in an encoder.
fn write_wav(path: &PathBuf, channels: u16, sample_rate: u32, samples: &[i16]) {
    let bits = 16u16;
    let block_align = channels * bits / 8;
    let byte_rate = sample_rate * u32::from(block_align);
    let data_len = (samples.len() * 2) as u32;

    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");

    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // format: PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());

    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }

    let mut file = std::fs::File::create(path).expect("create wav");
    file.write_all(&out).expect("write wav");
}

/// A scratch directory that cleans up after itself.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "djmanzo-decode-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn decodes_a_stereo_wav() {
    let dir = TempDir::new("stereo");
    let path = dir.file("tone.wav");

    // 1000 frames of stereo: left at +half scale, right at -half scale, so
    // channel order is verifiable rather than merely plausible.
    let samples: Vec<i16> = (0..1000).flat_map(|_| [16384i16, -16384i16]).collect();
    write_wav(&path, 2, 44_100, &samples);

    let track = decode_file(&path).expect("decode should succeed");

    assert_eq!(track.buffer.len_frames(), 1000);
    assert_eq!(track.buffer.sample_rate().get(), 44_100);

    let [left, right] = track.buffer.frame(0);
    assert!(
        (left - 0.5).abs() < 0.01,
        "left channel should be +0.5, got {left}"
    );
    assert!(
        (right + 0.5).abs() < 0.01,
        "right channel should be -0.5, got {right}"
    );
}

#[test]
fn mono_is_widened_to_stereo() {
    let dir = TempDir::new("mono");
    let path = dir.file("mono.wav");
    let samples: Vec<i16> = vec![8192; 500];
    write_wav(&path, 1, 48_000, &samples);

    let track = decode_file(&path).expect("decode should succeed");

    // 500 mono samples become 500 stereo frames, not 250.
    assert_eq!(track.buffer.len_frames(), 500);
    let [left, right] = track.buffer.frame(10);
    assert_eq!(left, right, "mono must be centred, not hard left");
    assert!((left - 0.25).abs() < 0.01);
}

#[test]
fn duration_matches_the_source() {
    let dir = TempDir::new("duration");
    let path = dir.file("two-seconds.wav");
    // Exactly two seconds of stereo at 48 kHz.
    let samples: Vec<i16> = vec![0; 48_000 * 2 * 2];
    write_wav(&path, 2, 48_000, &samples);

    let track = decode_file(&path).expect("decode should succeed");
    assert!(
        (track.buffer.duration_seconds() - 2.0).abs() < 0.01,
        "expected 2s, got {}",
        track.buffer.duration_seconds()
    );
}

#[test]
fn identical_audio_yields_an_identical_id() {
    let dir = TempDir::new("hash");
    let samples: Vec<i16> = (0..500).flat_map(|n| [n as i16, -(n as i16)]).collect();

    let first = dir.file("a.wav");
    let second = dir.file("b.wav");
    write_wav(&first, 2, 44_100, &samples);
    write_wav(&second, 2, 44_100, &samples);

    // Content addressing: the same audio at a different path is the same track,
    // which is what keeps cues and analysis attached when files move.
    assert_eq!(
        decode_file(&first).unwrap().id,
        decode_file(&second).unwrap().id
    );
}

#[test]
fn different_audio_yields_a_different_id() {
    let dir = TempDir::new("hash-differs");
    let first = dir.file("a.wav");
    let second = dir.file("b.wav");
    write_wav(&first, 2, 44_100, &vec![1000i16; 400]);
    write_wav(&second, 2, 44_100, &vec![2000i16; 400]);

    assert_ne!(
        decode_file(&first).unwrap().id,
        decode_file(&second).unwrap().id
    );
}

#[test]
fn untagged_file_falls_back_to_its_filename() {
    let dir = TempDir::new("title");
    let path = dir.file("Some Great Track.wav");
    write_wav(&path, 2, 44_100, &vec![0i16; 200]);

    let track = decode_file(&path).unwrap();
    assert_eq!(track.display_title(), "Some Great Track");
}

#[test]
fn a_missing_file_is_reported_clearly() {
    let error = decode_file("/nonexistent/nope.wav").unwrap_err();
    assert!(matches!(error, DecodeError::Open { .. }));
}

#[test]
fn a_file_that_is_not_audio_is_rejected() {
    let dir = TempDir::new("garbage");
    let path = dir.file("not-audio.wav");
    std::fs::write(&path, b"this is definitely not a wav file").unwrap();

    // Must fail cleanly rather than panicking or producing noise.
    assert!(decode_file(&path).is_err());
}

#[test]
fn an_empty_file_is_rejected() {
    let dir = TempDir::new("empty");
    let path = dir.file("empty.wav");
    write_wav(&path, 2, 44_100, &[]);

    assert!(decode_file(&path).is_err());
}
