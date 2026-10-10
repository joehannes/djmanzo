//! Opus, the one codec a record commonly comes in that Symphonia 0.5 cannot
//! decode (§122).
//!
//! It is the sound of almost every WebM from YouTube and of every `.opus`
//! file. Symphonia reads both containers — Ogg's mapping parses the `OpusHead`
//! and Matroska passes it on as the track's private data — and then has no
//! decoder to hand the packets to. This is that decoder, registered beside
//! Symphonia's own in [`crate::decoder`], over `rusty-opus` (BSD-3-Clause, pure
//! Rust).
//!
//! # Why that decoder
//!
//! Two pure-Rust decoders were run against the twelve RFC 8251 conformance
//! vectors and scored with Xiph's `opus_compare`, judged as Xiph's own
//! `run_vectors.sh` judges them. Decoding to stereo, both passed all twelve.
//! Decoding to mono, each failed several — so this decoder always decodes to
//! stereo, which is what djmanzo plays anyway, and a mono stream comes out in
//! both channels as Opus itself duplicates it. On the vectors coded with CELT,
//! `opus-decoder` took 4–11 s each where `rusty-opus` took 0.03–0.2 s; a
//! record is decoded whole before it plays, so that difference is a DJ waiting.
//!
//! # What is done to the sound
//!
//! - **The encoder's delay comes off the start.** Every Opus stream begins with
//!   a few milliseconds the encoder needed to look ahead (`OpusHead`'s
//!   *pre-skip*, 312 frames from libopus). This decoder drops them, in either
//!   container: Symphonia 0.5's Ogg reader records the pre-skip as the
//!   track's delay and then never trims by it, and Matroska says nothing at
//!   all. (Ogg's *end* is Symphonia's: it cuts the last packet at the last
//!   page's granule position, which counts the pre-skip, and so is right.)
//!   Left in, a record's first beat would sit 6.5 ms late against its own
//!   grid, and a WebM's sound 6.5 ms behind its picture.
//! - **The header's output gain is applied**, as RFC 7845 asks of a player: it
//!   is how a stream is made louder or quieter without re-encoding it.
//!
//! # What is not read
//!
//! Surround Opus — more than two channels, coded as several streams — is
//! refused with that reason. A record in it is rare, and folding it to stereo
//! properly is its own piece of work.

use symphonia::core::audio::{
    AsAudioBufferRef, AudioBuffer, AudioBufferRef, Channels, Signal, SignalSpec,
};
use symphonia::core::codecs::{
    CODEC_TYPE_OPUS, CodecDescriptor, CodecParameters, Decoder, DecoderOptions, FinalizeResult,
};
use symphonia::core::errors::{Result, decode_error, unsupported_error};
use symphonia::core::formats::Packet;
use symphonia::core::support_codec;

/// Opus always decodes at 48 kHz; the rate in its header is only what the
/// source was, for an encoder's information.
const RATE: u32 = 48_000;

/// The most one packet can hold: 120 ms at 48 kHz.
const MOST: usize = 5760;

/// What `OpusHead` says, of what a player needs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Head {
    pub channels: u8,
    /// Frames at 48 kHz to drop from the start.
    pub pre_skip: u16,
    /// The output gain, as a factor.
    pub gain: f32,
}

/// Read an `OpusHead` (RFC 7845 §5.1), or say why it cannot be played.
pub(crate) fn head(bytes: &[u8]) -> std::result::Result<Head, &'static str> {
    if bytes.len() < 19 || &bytes[..8] != b"OpusHead" {
        return Err("opus: the stream has no OpusHead");
    }
    // Only the major version (the top four bits) changes the layout.
    if bytes[8] >> 4 != 0 {
        return Err("opus: an OpusHead of a version this decoder does not know");
    }
    let channels = bytes[9];
    let pre_skip = u16::from_le_bytes([bytes[10], bytes[11]]);
    let gain_q8 = i16::from_le_bytes([bytes[16], bytes[17]]);
    let family = bytes[18];
    // Family 0 is one stream of one or two channels. Family 1 can say the
    // same thing at greater length — one stream, at most two channels — and
    // is read when it does; anything else is several streams.
    let one_stream = match family {
        0 => true,
        _ => bytes.get(19) == Some(&1),
    };
    if channels == 0 || channels > 2 || !one_stream {
        return Err("opus: surround Opus (more than two channels) is not read yet");
    }
    Ok(Head {
        channels,
        pre_skip,
        // Q7.8 decibels.
        gain: 10f32.powf(f32::from(gain_q8) / 256.0 / 20.0),
    })
}

/// Symphonia's view of an Opus decoder.
pub struct OpusDecoder {
    params: CodecParameters,
    inner: rusty_opus::OpusDecoder,
    gain: f32,
    /// Frames of the pre-skip still to drop from the start.
    skip: usize,
    interleaved: Vec<f32>,
    buf: AudioBuffer<f32>,
}

impl std::fmt::Debug for OpusDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpusDecoder")
            .field("gain", &self.gain)
            .field("skip", &self.skip)
            .finish_non_exhaustive()
    }
}

fn fresh() -> Result<rusty_opus::OpusDecoder> {
    // Two channels always: see the module's note on mono.
    rusty_opus::OpusDecoder::new(RATE as i32, 2)
        .or_else(|_| unsupported_error("opus: the decoder could not be made"))
}

impl Decoder for OpusDecoder {
    fn try_new(params: &CodecParameters, _: &DecoderOptions) -> Result<Self> {
        if params.codec != CODEC_TYPE_OPUS {
            return unsupported_error("opus: not an Opus stream");
        }
        let Some(extra) = params.extra_data.as_deref() else {
            return unsupported_error("opus: the stream has no OpusHead");
        };
        let head = match head(extra) {
            Ok(head) => head,
            Err(why) => return unsupported_error(why),
        };
        // Neither container trims it (see the module's note), so it is
        // always this decoder's to take.
        let skip = usize::from(head.pre_skip);
        Ok(Self {
            params: params.clone(),
            inner: fresh()?,
            gain: head.gain,
            skip,
            interleaved: vec![0.0; MOST * 2],
            buf: AudioBuffer::new(
                MOST as u64,
                SignalSpec::new(RATE, Channels::FRONT_LEFT | Channels::FRONT_RIGHT),
            ),
        })
    }

    fn supported_codecs() -> &'static [CodecDescriptor] {
        &[support_codec!(CODEC_TYPE_OPUS, "opus", "Opus")]
    }

    fn reset(&mut self) {
        // `rusty-opus` has no reset of its own; a new decoder is one.
        if let Ok(inner) = fresh() {
            self.inner = inner;
        }
    }

    fn codec_params(&self) -> &CodecParameters {
        &self.params
    }

    fn decode(&mut self, packet: &Packet) -> Result<AudioBufferRef<'_>> {
        self.buf.clear();
        let Ok(frames) = self.inner.decode(packet.buf(), MOST, &mut self.interleaved) else {
            return decode_error("opus: a packet that could not be decoded");
        };
        self.buf.render_reserved(Some(frames));
        let gain = self.gain;
        let (left, right) = self.buf.chan_pair_mut(0, 1);
        for ((l, r), pair) in left
            .iter_mut()
            .zip(right.iter_mut())
            .zip(self.interleaved.chunks_exact(2))
        {
            *l = pair[0] * gain;
            *r = pair[1] * gain;
        }
        let own = self.skip.min(frames);
        self.skip -= own;
        self.buf.trim(
            packet.trim_start() as usize + own,
            packet.trim_end() as usize,
        );
        Ok(self.buf.as_audio_buffer_ref())
    }

    fn finalize(&mut self) -> FinalizeResult {
        FinalizeResult::default()
    }

    fn last_decoded(&self) -> AudioBufferRef<'_> {
        self.buf.as_audio_buffer_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opus_head(channels: u8, pre_skip: u16, gain_q8: i16, family: u8) -> Vec<u8> {
        let mut out = b"OpusHead".to_vec();
        out.push(1);
        out.push(channels);
        out.extend_from_slice(&pre_skip.to_le_bytes());
        out.extend_from_slice(&44_100u32.to_le_bytes());
        out.extend_from_slice(&gain_q8.to_le_bytes());
        out.push(family);
        out
    }

    #[test]
    fn a_header_says_its_delay_and_its_gain() {
        let read = head(&opus_head(2, 312, -6 * 256, 0)).expect("a stereo stream");
        assert_eq!(read.channels, 2);
        assert_eq!(read.pre_skip, 312);
        assert!((read.gain - 0.501).abs() < 0.001, "{}", read.gain);
        assert_eq!(head(&opus_head(1, 0, 0, 0)).map(|h| h.gain), Ok(1.0));
    }

    /// **Surround is refused with its reason, not decoded as stereo.**
    #[test]
    fn surround_and_what_is_not_opus_are_refused() {
        let mut six = opus_head(6, 312, 0, 1);
        six.extend_from_slice(&[4, 2, 0, 4, 1, 2, 3, 5]);
        assert!(head(&six).unwrap_err().contains("surround"));
        // Family 1 that is one stereo stream is only stereo, at length.
        let mut two = opus_head(2, 312, 0, 1);
        two.extend_from_slice(&[1, 1, 0, 1]);
        assert!(head(&two).is_ok());
        assert!(head(b"OpusTags........xxx").is_err());
        assert!(head(&opus_head(2, 312, 0, 0)[..18]).is_err());
        let mut next = opus_head(2, 312, 0, 0);
        next[8] = 0x10;
        assert!(head(&next).is_err());
    }
}
