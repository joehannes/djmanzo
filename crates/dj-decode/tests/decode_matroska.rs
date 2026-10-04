//! §122: a video's sound, in the container a video comes in.
//!
//! Matroska (and WebM, which is Matroska) carries the sound of most video
//! files that are not MP4, and it says nothing of how long a packet of
//! uncompressed sound is — which Symphonia's PCM decoder needs told. These
//! write a Matroska file byte by byte, as `decode_wav.rs` writes WAVs, and read
//! it back.

use dj_decode::decode_file;
use std::path::Path;

/// One EBML element: its id, its length as a variable-length integer (always
/// eight bytes here, which every reader accepts), and its body.
fn element(id: &[u8], body: &[u8]) -> Vec<u8> {
    let mut out = id.to_vec();
    out.push(0x01);
    out.extend_from_slice(&(body.len() as u64).to_be_bytes()[1..]);
    out.extend_from_slice(body);
    out
}

fn uint(id: &[u8], value: u64) -> Vec<u8> {
    element(id, &value.to_be_bytes())
}

/// A Matroska file of interleaved 32-bit float sound at `rate`, `channels`
/// wide, in blocks of `per_block` frames.
fn matroska(samples: &[f32], rate: f64, channels: u64, per_block: usize) -> Vec<u8> {
    let header = [
        uint(&[0x42, 0x86], 1),
        uint(&[0x42, 0xF7], 1),
        uint(&[0x42, 0xF2], 4),
        uint(&[0x42, 0xF3], 8),
        element(&[0x42, 0x82], b"webm"),
        uint(&[0x42, 0x87], 4),
        uint(&[0x42, 0x85], 2),
    ]
    .concat();
    let info = element(
        &[0x15, 0x49, 0xA9, 0x66],
        &[
            uint(&[0x2A, 0xD7, 0xB1], 1_000_000),
            element(&[0x4D, 0x80], b"djmanzo test"),
            element(&[0x57, 0x41], b"djmanzo test"),
        ]
        .concat(),
    );
    let audio = element(
        &[0xE1],
        &[
            element(&[0xB5], &rate.to_be_bytes()),
            uint(&[0x9F], channels),
            uint(&[0x62, 0x64], 32),
        ]
        .concat(),
    );
    let track = element(
        &[0xAE],
        &[
            uint(&[0xD7], 1),
            uint(&[0x73, 0xC5], 1),
            uint(&[0x83], 2),
            element(&[0x86], b"A_PCM/FLOAT/IEEE"),
            audio,
        ]
        .concat(),
    );
    let tracks = element(&[0x16, 0x54, 0xAE, 0x6B], &track);

    let width = channels as usize;
    let mut cluster = uint(&[0xE7], 0);
    for (n, block) in samples.chunks(per_block * width).enumerate() {
        // Track 1, the block's time from the cluster's in milliseconds, a
        // keyframe, then the samples.
        let at = ((n * per_block) as f64 / rate * 1000.0) as i16;
        let mut body = vec![0x81];
        body.extend_from_slice(&at.to_be_bytes());
        body.push(0x80);
        for sample in block {
            body.extend_from_slice(&sample.to_le_bytes());
        }
        cluster.extend(element(&[0xA3], &body));
    }
    let cluster = element(&[0x1F, 0x43, 0xB6, 0x75], &cluster);
    let segment = element(&[0x18, 0x53, 0x80, 0x67], &[info, tracks, cluster].concat());
    [element(&[0x1A, 0x45, 0xDF, 0xA3], &header), segment].concat()
}

fn write(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).expect("write the file");
}

/// **A video's uncompressed sound is read, every frame of it, as recorded.**
/// What a browser's own recorder writes, and what failed outright before:
/// "pcm: maximum frames per packet is required".
#[test]
fn uncompressed_sound_in_matroska_is_read_whole() {
    let rate = 48_000.0;
    let frames = 24_000;
    let samples: Vec<f32> = (0..frames)
        .flat_map(|i| {
            let t = i as f32 / 48_000.0;
            let left = (t * 440.0 * std::f32::consts::TAU).sin() * 0.5;
            [left, -left]
        })
        .collect();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clip.webm");
    write(&path, &matroska(&samples, rate, 2, 480));

    let track = decode_file(&path).expect("a video's PCM sound is read");
    assert_eq!(track.buffer.sample_rate().get(), 48_000);
    let got = track.buffer.as_interleaved();
    assert_eq!(got.len(), samples.len(), "every frame, none dropped");
    for (i, (a, b)) in got.iter().zip(&samples).enumerate() {
        assert!((a - b).abs() < 1e-6, "sample {i}: {a} against {b}");
    }
}

/// A block far longer than a browser writes — two seconds in one packet —
/// is read whole too: the bound the decoder is given is a generous one.
#[test]
fn a_long_block_is_not_cut() {
    let frames = 2 * 44_100;
    let samples: Vec<f32> = (0..frames * 2).map(|i| (i % 100) as f32 / 100.0).collect();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("long.mkv");
    write(&path, &matroska(&samples, 44_100.0, 2, frames));
    let track = decode_file(&path).expect("read");
    assert_eq!(track.buffer.as_interleaved().len(), samples.len());
}
