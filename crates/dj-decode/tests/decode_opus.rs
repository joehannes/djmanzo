//! §122: Opus, the sound of a WebM from YouTube and of every `.opus` file.
//!
//! These write the two containers it comes in byte by byte, as
//! `decode_matroska.rs` does — an Ogg Opus file with its pages and checksums,
//! and a WebM — around packets from `rusty-opus`'s own encoder, and read them
//! back through [`decode_file`]. What comes out is compared, sample for
//! sample, with the same packets decoded directly: so what is held is exactly
//! what djmanzo adds — which frames are dropped from each end, the gain, the
//! channels — and the codec's own correctness rests where it was measured,
//! on the RFC 8251 vectors (`dj_decode::opus`).

use dj_decode::decode_file;
use rusty_opus::{Application, OpusDecoder, OpusEncoder};
use std::path::Path;

/// 20 ms at 48 kHz: what every packet here holds.
const FRAME: usize = 960;
/// What libopus writes as its pre-skip: the frames a stream starts with that
/// are the encoder looking ahead, not the music.
const PRE_SKIP: usize = 312;

/// One second of a tone, 440 Hz on the left and 660 on the right (or the
/// left alone, for one channel), as Opus packets.
fn packets(channels: usize) -> Vec<Vec<u8>> {
    let mut encoder = OpusEncoder::new(48_000, channels, Application::Audio).expect("encoder");
    encoder.bitrate_bps = 128_000;
    let tone = |hz: f32, i: usize| (i as f32 / 48_000.0 * hz * std::f32::consts::TAU).sin() * 0.5;
    let samples: Vec<f32> = (0..48_000)
        .flat_map(|i| {
            let both = [tone(440.0, i), tone(660.0, i)];
            both.into_iter().take(channels)
        })
        .collect();
    samples
        .chunks(FRAME * channels)
        .map(|frame| {
            let mut packet = [0u8; 1500];
            let len = encoder.encode(frame, FRAME, &mut packet).expect("encode");
            packet[..len].to_vec()
        })
        .collect()
}

/// The packets decoded directly, to stereo, every frame: what the file holds
/// before anything is taken off either end.
fn decoded_directly(packets: &[Vec<u8>]) -> Vec<f32> {
    let mut decoder = OpusDecoder::new(48_000, 2).expect("decoder");
    let mut out = Vec::new();
    let mut pcm = vec![0f32; 5760 * 2];
    for packet in packets {
        let frames = decoder.decode(packet, 5760, &mut pcm).expect("decode");
        out.extend_from_slice(&pcm[..frames * 2]);
    }
    out
}

fn opus_head(channels: u8, gain_q8: i16) -> Vec<u8> {
    let mut out = b"OpusHead".to_vec();
    out.push(1);
    out.push(channels);
    out.extend_from_slice(&(PRE_SKIP as u16).to_le_bytes());
    out.extend_from_slice(&48_000u32.to_le_bytes());
    out.extend_from_slice(&gain_q8.to_le_bytes());
    out.push(0);
    out
}

/// Ogg's checksum: CRC-32 with polynomial 0x04C11DB7, unreflected, from zero.
fn ogg_crc(bytes: &[u8]) -> u32 {
    let mut crc = 0u32;
    for &byte in bytes {
        crc ^= u32::from(byte) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 == 0 {
                crc << 1
            } else {
                (crc << 1) ^ 0x04C1_1DB7
            };
        }
    }
    crc
}

/// One Ogg page carrying one packet.
fn page(kind: u8, granule: u64, sequence: u32, packet: &[u8]) -> Vec<u8> {
    let mut lacing = vec![255u8; packet.len() / 255];
    lacing.push((packet.len() % 255) as u8);
    let mut out = b"OggS".to_vec();
    out.push(0);
    out.push(kind);
    out.extend_from_slice(&granule.to_le_bytes());
    out.extend_from_slice(&0x646a_6d7au32.to_le_bytes());
    out.extend_from_slice(&sequence.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.push(lacing.len() as u8);
    out.extend_from_slice(&lacing);
    out.extend_from_slice(packet);
    let crc = ogg_crc(&out);
    out[22..26].copy_from_slice(&crc.to_le_bytes());
    out
}

/// An Ogg Opus file of `packets`, whose last page says the music is `music`
/// frames long — after the pre-skip, as RFC 7845 counts it.
fn ogg_opus(packets: &[Vec<u8>], head: &[u8], music: usize) -> Vec<u8> {
    let mut tags = b"OpusTags".to_vec();
    tags.extend_from_slice(&7u32.to_le_bytes());
    tags.extend_from_slice(b"djmanzo");
    tags.extend_from_slice(&0u32.to_le_bytes());
    let mut out = page(0x02, 0, 0, head);
    out.extend(page(0, 0, 1, &tags));
    for (n, packet) in packets.iter().enumerate() {
        let last = n + 1 == packets.len();
        let granule = if last {
            PRE_SKIP + music
        } else {
            (n + 1) * FRAME
        };
        out.extend(page(
            if last { 0x04 } else { 0 },
            granule as u64,
            n as u32 + 2,
            packet,
        ));
    }
    out
}

/// One EBML element, its length always eight bytes, as in `decode_matroska.rs`.
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

/// A WebM of `packets`, as a browser or YouTube writes one: the `OpusHead` as
/// the track's private data, and nothing said of the encoder's delay.
fn webm(packets: &[Vec<u8>], head: &[u8], channels: u64) -> Vec<u8> {
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
            element(&[0xB5], &48_000f64.to_be_bytes()),
            uint(&[0x9F], channels),
        ]
        .concat(),
    );
    let track = element(
        &[0xAE],
        &[
            uint(&[0xD7], 1),
            uint(&[0x73, 0xC5], 1),
            uint(&[0x83], 2),
            element(&[0x86], b"A_OPUS"),
            element(&[0x63, 0xA2], head),
            audio,
        ]
        .concat(),
    );
    let tracks = element(&[0x16, 0x54, 0xAE, 0x6B], &track);
    let mut cluster = uint(&[0xE7], 0);
    for (n, packet) in packets.iter().enumerate() {
        let mut body = vec![0x81];
        body.extend_from_slice(&((n * 20) as i16).to_be_bytes());
        body.push(0x80);
        body.extend_from_slice(packet);
        cluster.extend(element(&[0xA3], &body));
    }
    let cluster = element(&[0x1F, 0x43, 0xB6, 0x75], &cluster);
    let segment = element(&[0x18, 0x53, 0x80, 0x67], &[info, tracks, cluster].concat());
    [element(&[0x1A, 0x45, 0xDF, 0xA3], &header), segment].concat()
}

fn read(path: &Path, bytes: &[u8]) -> Vec<f32> {
    std::fs::write(path, bytes).expect("write the file");
    let track = decode_file(path).expect("an Opus record is read");
    assert_eq!(track.buffer.sample_rate().get(), 48_000);
    track.buffer.as_interleaved().to_vec()
}

fn assert_same(got: &[f32], expected: &[f32]) {
    assert_eq!(
        got.len() / 2,
        expected.len() / 2,
        "frames read against frames expected"
    );
    for (i, (a, b)) in got.iter().zip(expected).enumerate() {
        assert!(
            (a - b).abs() < 1e-6,
            "sample {i} (frame {}): {a} against {b}",
            i / 2
        );
    }
}

/// How often the left channel crosses zero going up, per second.
fn left_hz(samples: &[f32]) -> f32 {
    let left: Vec<f32> = samples.iter().step_by(2).copied().collect();
    let ups = left
        .windows(2)
        .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
        .count();
    ups as f32 / (left.len() as f32 / 48_000.0)
}

/// **An `.opus` file is read from its first sound to its last.**
///
/// Its first 312 frames are the encoder looking ahead, and its last page
/// says where the music stops, inside the last packet. Both come off: left
/// in, a record's first beat sits 6.5 ms late against its grid, and its end
/// carries a tail that was never recorded.
#[test]
fn an_opus_file_is_read_from_its_first_sound_to_its_last() {
    let packets = packets(2);
    let whole = decoded_directly(&packets);
    let music = whole.len() / 2 - PRE_SKIP - 700;
    let dir = tempfile::tempdir().unwrap();
    let got = read(
        &dir.path().join("tone.opus"),
        &ogg_opus(&packets, &opus_head(2, 0), music),
    );
    assert_same(&got, &whole[PRE_SKIP * 2..(PRE_SKIP + music) * 2]);
    // And it is the tone: 440 Hz on the left, at the level it was made.
    let hz = left_hz(&got);
    assert!((hz - 440.0).abs() < 5.0, "{hz} Hz");
}

/// **A WebM's Opus sound starts without the encoder's delay.**
///
/// What YouTube serves, and what failed outright before: "core (codec):
/// unsupported codec". Matroska says nothing of the delay; the `OpusHead`
/// does, and the decoder takes it.
#[test]
fn a_webms_opus_sound_starts_without_the_encoders_delay() {
    let packets = packets(2);
    let whole = decoded_directly(&packets);
    let dir = tempfile::tempdir().unwrap();
    let got = read(
        &dir.path().join("video.webm"),
        &webm(&packets, &opus_head(2, 0), 2),
    );
    assert_same(&got, &whole[PRE_SKIP * 2..]);
}

/// **A mono Opus record plays in both ears**, as every mono record does.
#[test]
fn a_mono_opus_record_plays_in_both_ears() {
    let packets = packets(1);
    let whole = decoded_directly(&packets);
    let dir = tempfile::tempdir().unwrap();
    let got = read(
        &dir.path().join("mono.webm"),
        &webm(&packets, &opus_head(1, 0), 1),
    );
    assert_same(&got, &whole[PRE_SKIP * 2..]);
    assert!(got.chunks_exact(2).all(|pair| pair[0] == pair[1]));
    assert!(got.iter().any(|s| s.abs() > 0.3), "a tone, not silence");
}

/// **The stream's own gain is applied**, as RFC 7845 asks of a player: it is
/// how a stream is made quieter without encoding it again.
#[test]
fn the_streams_own_gain_is_applied() {
    let packets = packets(2);
    let whole = decoded_directly(&packets);
    // Minus 6.02 dB, in the header's Q7.8: half.
    let half: Vec<f32> = whole[PRE_SKIP * 2..]
        .iter()
        .map(|s| s * 10f32.powf(-1541.0 / 256.0 / 20.0))
        .collect();
    let dir = tempfile::tempdir().unwrap();
    let got = read(
        &dir.path().join("quiet.webm"),
        &webm(&packets, &opus_head(2, -1541), 2),
    );
    assert_same(&got, &half);
}

/// **Surround Opus is refused with its reason**, not decoded as something it
/// is not.
#[test]
fn surround_opus_is_refused_with_its_reason() {
    let mut head = opus_head(6, 0);
    head[18] = 1;
    head.extend_from_slice(&[4, 2, 0, 4, 1, 2, 3, 5]);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("surround.webm");
    std::fs::write(&path, webm(&packets(2), &head, 6)).unwrap();
    let why = decode_file(&path).expect_err("refused").to_string();
    assert!(why.contains("surround"), "{why}");
}
