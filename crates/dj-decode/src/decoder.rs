//! File decoding via Symphonia.

use crate::buffer::{AudioBuffer, CHANNELS};
use dj_core::{SampleRate, TrackId};
use std::fs::File;
use std::path::{Path, PathBuf};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, StandardTagKey};
use symphonia::core::probe::Hint;

/// A decoded file: the audio plus whatever the tags said.
#[derive(Debug, Clone)]
pub struct DecodedTrack {
    pub id: TrackId,
    pub path: PathBuf,
    pub buffer: AudioBuffer,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
}

impl DecodedTrack {
    #[must_use]
    pub fn display_title(&self) -> String {
        self.title.clone().unwrap_or_else(|| {
            self.path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Untitled".to_owned())
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("cannot open {path}: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("unrecognised or unsupported format: {0}")]
    Unsupported(String),
    #[error("no audio track in file")]
    NoAudioTrack,
    #[error("stream has no usable sample rate")]
    BadSampleRate,
    #[error("decode failed: {0}")]
    Decode(String),
    #[error("file contains no audio")]
    Empty,
}

/// Decode an entire file into memory.
///
/// Blocking and potentially slow -- minutes of audio, a full content hash. Never
/// call this from the audio thread or from a UI event handler; it belongs on a
/// worker.
pub fn decode_file(path: impl AsRef<Path>) -> Result<DecodedTrack, DecodeError> {
    let path = path.as_ref();
    let file = File::open(path).map_err(|source| DecodeError::Open {
        path: path.to_path_buf(),
        source,
    })?;

    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(extension);
    }

    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions {
                enable_gapless: true,
                ..Default::default()
            },
            &MetadataOptions::default(),
        )
        .map_err(|e| DecodeError::Unsupported(e.to_string()))?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(DecodeError::NoAudioTrack)?;
    let track_id = track.id;

    let source_rate = track
        .codec_params
        .sample_rate
        .and_then(SampleRate::new)
        .ok_or(DecodeError::BadSampleRate)?;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| DecodeError::Unsupported(e.to_string()))?;

    // Pre-size from the declared duration where the container gives one. Saves a
    // long chain of reallocations on a full-length track.
    let mut interleaved: Vec<f32> = match track.codec_params.n_frames {
        Some(frames) => Vec::with_capacity((frames as usize).saturating_mul(CHANNELS)),
        None => Vec::new(),
    };
    let mut sample_buffer: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            // Symphonia signals a clean end of stream as an IO error of kind
            // UnexpectedEof rather than a dedicated variant.
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(SymphoniaError::ResetRequired) => break,
            Err(e) => return Err(DecodeError::Decode(e.to_string())),
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                let buffer = sample_buffer.get_or_insert_with(|| {
                    SampleBuffer::<f32>::new(decoded.capacity() as u64, spec)
                });
                buffer.copy_interleaved_ref(decoded);
                append_as_stereo(&mut interleaved, buffer.samples(), spec.channels.count());
            }
            // A corrupt packet in the middle of a set should skip, not abort the
            // load. Symphonia marks exactly these two as recoverable.
            Err(SymphoniaError::DecodeError(_) | SymphoniaError::IoError(_)) => continue,
            Err(e) => return Err(DecodeError::Decode(e.to_string())),
        }
    }

    if interleaved.is_empty() {
        return Err(DecodeError::Empty);
    }

    let (title, artist, album) = read_tags(&mut format);
    let id = hash_audio(&interleaved);

    Ok(DecodedTrack {
        id,
        path: path.to_path_buf(),
        buffer: AudioBuffer::from_interleaved(interleaved, source_rate),
        title,
        artist,
        album,
    })
}

/// Fold any channel layout down to interleaved stereo.
///
/// Mono is duplicated so it sits centre rather than hard left. Anything wider
/// than stereo keeps its first two channels, which for standard layouts is L/R.
fn append_as_stereo(out: &mut Vec<f32>, samples: &[f32], channels: usize) {
    match channels {
        0 => {}
        1 => {
            for &sample in samples {
                out.push(sample);
                out.push(sample);
            }
        }
        2 => out.extend_from_slice(samples),
        n => {
            for frame in samples.chunks_exact(n) {
                out.push(frame[0]);
                out.push(frame[1]);
            }
        }
    }
}

/// Content hash of the decoded audio.
///
/// Keying on decoded audio rather than file bytes means the same track keeps its
/// cues and stem cache across a container change, and a re-encode correctly does
/// not. FNV-1a over the sample bits, widened to 32 bytes -- adequate for cache
/// keying, and swappable for BLAKE3 when the library lands in M3.
fn hash_audio(samples: &[f32]) -> TrackId {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    let mut lanes = [OFFSET; 4];
    for (i, &sample) in samples.iter().enumerate() {
        let lane = &mut lanes[i % 4];
        for byte in sample.to_bits().to_le_bytes() {
            *lane ^= u64::from(byte);
            *lane = lane.wrapping_mul(PRIME);
        }
    }
    // Mix the length in so two files differing only by trailing silence differ.
    lanes[0] ^= samples.len() as u64;
    lanes[0] = lanes[0].wrapping_mul(PRIME);

    let mut bytes = [0u8; 32];
    for (i, lane) in lanes.iter().enumerate() {
        bytes[i * 8..(i + 1) * 8].copy_from_slice(&lane.to_le_bytes());
    }
    TrackId::from_bytes(bytes)
}

type Tags = (Option<String>, Option<String>, Option<String>);

fn read_tags(format: &mut Box<dyn symphonia::core::formats::FormatReader>) -> Tags {
    let mut title = None;
    let mut artist = None;
    let mut album = None;

    let mut take = |tags: &[symphonia::core::meta::Tag]| {
        for tag in tags {
            let value = tag.value.to_string();
            if value.trim().is_empty() {
                continue;
            }
            match tag.std_key {
                Some(StandardTagKey::TrackTitle) if title.is_none() => title = Some(value),
                Some(StandardTagKey::Artist) if artist.is_none() => artist = Some(value),
                Some(StandardTagKey::Album) if album.is_none() => album = Some(value),
                _ => {}
            }
        }
    };

    // Tags can live in the container or in a leading metadata block; check both.
    if let Some(metadata) = format.metadata().current() {
        take(metadata.tags());
    }
    if let Some(mut metadata) = format
        .metadata()
        .skip_to_latest()
        .map(|m| m.tags().to_vec())
    {
        take(&mut metadata);
    }

    (title, artist, album)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_is_duplicated_to_both_channels() {
        let mut out = Vec::new();
        append_as_stereo(&mut out, &[0.5, -0.5], 1);
        assert_eq!(out, vec![0.5, 0.5, -0.5, -0.5]);
    }

    #[test]
    fn stereo_passes_through_untouched() {
        let mut out = Vec::new();
        append_as_stereo(&mut out, &[1.0, 2.0, 3.0, 4.0], 2);
        assert_eq!(out, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn surround_keeps_the_front_pair() {
        let mut out = Vec::new();
        // Two 6-channel frames; only channels 0 and 1 survive.
        append_as_stereo(
            &mut out,
            &[
                1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0,
            ],
            6,
        );
        assert_eq!(out, vec![1.0, 2.0, 7.0, 8.0]);
    }

    #[test]
    fn zero_channels_produces_nothing() {
        let mut out = Vec::new();
        append_as_stereo(&mut out, &[1.0, 2.0], 0);
        assert!(out.is_empty());
    }

    #[test]
    fn identical_audio_hashes_identically() {
        let a = vec![0.1, 0.2, 0.3, 0.4];
        let b = vec![0.1, 0.2, 0.3, 0.4];
        assert_eq!(hash_audio(&a), hash_audio(&b));
    }

    #[test]
    fn different_audio_hashes_differently() {
        assert_ne!(hash_audio(&[0.1, 0.2]), hash_audio(&[0.1, 0.3]));
    }

    #[test]
    fn reordered_samples_hash_differently() {
        // A hash that ignored order would collide constantly on real music.
        assert_ne!(
            hash_audio(&[0.1, 0.2, 0.3, 0.4]),
            hash_audio(&[0.4, 0.3, 0.2, 0.1])
        );
    }

    #[test]
    fn trailing_silence_changes_the_hash() {
        let base = vec![0.1, 0.2];
        let padded = vec![0.1, 0.2, 0.0, 0.0];
        assert_ne!(hash_audio(&base), hash_audio(&padded));
    }

    #[test]
    fn missing_file_reports_the_path() {
        let err = decode_file("/definitely/not/here.flac").unwrap_err();
        assert!(matches!(err, DecodeError::Open { .. }));
        assert!(err.to_string().contains("not/here.flac"));
    }
}
