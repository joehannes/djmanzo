//! Writing a WAV file, incrementally.
//!
//! # Why this is here rather than a crate
//!
//! A WAV file is a 44-byte header and then the samples. Writing one is less
//! code than the entry it would need in [`docs/RESEARCH.md`](../../../docs/RESEARCH.md)
//! under [ADR-0002](../../../docs/adr/0002-clean-room-permissive-licensing.md),
//! and it keeps a dependency out of the path a recording has to survive.
//!
//! # Why the sizes are patched at the end
//!
//! Two header fields hold byte counts that are not known until the recording
//! stops. They are written as zero up front and seeked back to on close, which
//! is what every WAV writer does. The consequence worth knowing: **a recording
//! that is never closed — a crash, a power cut — leaves a file whose header
//! says it holds no audio, even though the samples are all on disk.**
//! [`Wav::repair`] exists for exactly that, and the size is also rewritten
//! every few seconds while recording so a lost file loses seconds rather than
//! everything.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Bytes before the sample data: `RIFF` chunk, `fmt ` chunk, `data` header.
const HEADER_BYTES: u64 = 44;
/// Offset of the RIFF chunk's size field.
const RIFF_SIZE_AT: u64 = 4;
/// Offset of the data chunk's size field.
const DATA_SIZE_AT: u64 = 40;

/// 16-bit signed PCM. Not float, which would sidestep the dither question and
/// double the size: a recorded set is something a DJ uploads, and every tool
/// that will touch it reads 16-bit. The dither happens before the samples get
/// here — see `crate::setrec`.
const BITS: u16 = 16;
const CHANNELS: u16 = 2;

#[derive(Debug, thiserror::Error)]
pub enum WavError {
    #[error("could not write {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// A WAV file open for appending.
#[derive(Debug)]
pub struct Wav {
    file: BufWriter<File>,
    path: PathBuf,
    /// Sample *values* written, not frames and not bytes.
    written: u64,
    sample_rate: u32,
}

impl Wav {
    /// Create the file and write a header with the sizes left at zero.
    pub fn create(path: impl AsRef<Path>, sample_rate: u32) -> Result<Self, WavError> {
        let path = path.as_ref().to_path_buf();
        let file = File::create(&path).map_err(|source| WavError::Io {
            path: path.clone(),
            source,
        })?;
        let mut wav = Self {
            file: BufWriter::with_capacity(1 << 16, file),
            path,
            written: 0,
            sample_rate,
        };
        wav.write_header()?;
        Ok(wav)
    }

    fn io(&self, source: std::io::Error) -> WavError {
        WavError::Io {
            path: self.path.clone(),
            source,
        }
    }

    fn write_header(&mut self) -> Result<(), WavError> {
        let byte_rate = self.sample_rate * u32::from(CHANNELS) * u32::from(BITS / 8);
        let block_align = CHANNELS * (BITS / 8);
        let mut header = Vec::with_capacity(HEADER_BYTES as usize);
        header.extend_from_slice(b"RIFF");
        header.extend_from_slice(&0u32.to_le_bytes()); // patched on close
        header.extend_from_slice(b"WAVE");
        header.extend_from_slice(b"fmt ");
        header.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
        header.extend_from_slice(&1u16.to_le_bytes()); // 1 = integer PCM
        header.extend_from_slice(&CHANNELS.to_le_bytes());
        header.extend_from_slice(&self.sample_rate.to_le_bytes());
        header.extend_from_slice(&byte_rate.to_le_bytes());
        header.extend_from_slice(&block_align.to_le_bytes());
        header.extend_from_slice(&BITS.to_le_bytes());
        header.extend_from_slice(b"data");
        header.extend_from_slice(&0u32.to_le_bytes()); // patched on close
        debug_assert_eq!(header.len() as u64, HEADER_BYTES);
        self.file
            .write_all(&header)
            .map_err(|source| self.io_owned(source))
    }

    fn io_owned(&self, source: std::io::Error) -> WavError {
        self.io(source)
    }

    /// Append interleaved 16-bit samples.
    pub fn write(&mut self, samples: &[i16]) -> Result<(), WavError> {
        // One `write_all` rather than one per sample: this runs on the writer
        // thread, but a syscall per sample would still be forty thousand a
        // second doing nothing useful.
        let mut bytes = Vec::with_capacity(samples.len() * 2);
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        self.file
            .write_all(&bytes)
            .map_err(|source| self.io_owned(source))?;
        self.written += samples.len() as u64;
        Ok(())
    }

    /// Frames written so far.
    #[must_use]
    pub fn frames(&self) -> u64 {
        self.written / u64::from(CHANNELS)
    }

    #[must_use]
    pub fn seconds(&self) -> f64 {
        if self.sample_rate == 0 {
            return 0.0;
        }
        self.frames() as f64 / f64::from(self.sample_rate)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Write the two size fields, leaving the file playable.
    ///
    /// Called periodically as well as on close, so a recording lost to a crash
    /// is short by seconds rather than entirely unreadable.
    pub fn flush_sizes(&mut self) -> Result<(), WavError> {
        let data_bytes = self.written * u64::from(BITS / 8);
        // A WAV size field is 32 bits, so the format itself runs out at 4 GiB —
        // about six hours of a stereo set. Saturating rather than wrapping: a
        // too-large file should report the largest size it can rather than a
        // small one, which is the difference between a player reading most of
        // it and a player reading none.
        let data = u32::try_from(data_bytes).unwrap_or(u32::MAX);
        let riff = u32::try_from(data_bytes + HEADER_BYTES - 8).unwrap_or(u32::MAX);

        self.file.flush().map_err(|source| self.io_owned(source))?;
        let file = self.file.get_mut();
        file.seek(SeekFrom::Start(RIFF_SIZE_AT))
            .and_then(|_| file.write_all(&riff.to_le_bytes()))
            .and_then(|_| file.seek(SeekFrom::Start(DATA_SIZE_AT)))
            .and_then(|_| file.write_all(&data.to_le_bytes()))
            .and_then(|_| file.seek(SeekFrom::End(0)))
            .map_err(|source| WavError::Io {
                path: self.path.clone(),
                source,
            })?;
        Ok(())
    }

    /// Finish the file.
    pub fn close(mut self) -> Result<PathBuf, WavError> {
        self.flush_sizes()?;
        self.file.flush().map_err(|source| self.io_owned(source))?;
        Ok(self.path)
    }

    /// Rewrite the sizes of a file whose recording never closed.
    ///
    /// Takes the byte length on disk as the truth, because that is the one
    /// thing a crash cannot have lied about.
    pub fn repair(path: impl AsRef<Path>) -> Result<u64, WavError> {
        let path = path.as_ref().to_path_buf();
        let io = |source| WavError::Io {
            path: path.clone(),
            source,
        };
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(io)?;
        let len = file.metadata().map_err(io)?.len();
        if len <= HEADER_BYTES {
            return Ok(0);
        }
        let data_bytes = len - HEADER_BYTES;
        let data = u32::try_from(data_bytes).unwrap_or(u32::MAX);
        let riff = u32::try_from(len - 8).unwrap_or(u32::MAX);
        file.seek(SeekFrom::Start(RIFF_SIZE_AT)).map_err(io)?;
        file.write_all(&riff.to_le_bytes()).map_err(io)?;
        file.seek(SeekFrom::Start(DATA_SIZE_AT)).map_err(io)?;
        file.write_all(&data.to_le_bytes()).map_err(io)?;
        Ok(data_bytes / u64::from(CHANNELS * (BITS / 8)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read a whole file back so a test can assert on the bytes it produced.
    fn bytes(path: &Path) -> Vec<u8> {
        std::fs::read(path).expect("the file should exist")
    }

    fn field(raw: &[u8], at: usize) -> u32 {
        u32::from_le_bytes([raw[at], raw[at + 1], raw[at + 2], raw[at + 3]])
    }

    fn temp(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("djmanzo-wav-{name}-{}.wav", std::process::id()));
        path
    }

    #[test]
    fn a_closed_file_says_how_much_audio_it_holds() {
        let path = temp("closed");
        let mut wav = Wav::create(&path, 48_000).unwrap();
        wav.write(&[1, -1, 2, -2]).unwrap();
        assert_eq!(wav.frames(), 2);
        let done = wav.close().unwrap();

        let raw = bytes(&done);
        assert_eq!(&raw[..4], b"RIFF");
        assert_eq!(&raw[8..12], b"WAVE");
        assert_eq!(&raw[36..40], b"data");
        assert_eq!(field(&raw, DATA_SIZE_AT as usize), 8, "four 16-bit samples");
        assert_eq!(
            field(&raw, RIFF_SIZE_AT as usize),
            8 + HEADER_BYTES as u32 - 8
        );
        assert_eq!(raw.len() as u64, HEADER_BYTES + 8);
        let _ = std::fs::remove_file(&done);
    }

    #[test]
    fn the_header_describes_the_format_it_actually_wrote() {
        let path = temp("format");
        let wav = Wav::create(&path, 44_100).unwrap();
        let done = wav.close().unwrap();
        let raw = bytes(&done);

        assert_eq!(u16::from_le_bytes([raw[20], raw[21]]), 1, "integer PCM");
        assert_eq!(u16::from_le_bytes([raw[22], raw[23]]), 2, "stereo");
        assert_eq!(field(&raw, 24), 44_100);
        // Byte rate and block align have to agree with the rest, or players
        // that trust them play at the wrong speed rather than refusing.
        assert_eq!(field(&raw, 28), 44_100 * 2 * 2);
        assert_eq!(u16::from_le_bytes([raw[32], raw[33]]), 4);
        assert_eq!(u16::from_le_bytes([raw[34], raw[35]]), 16);
        let _ = std::fs::remove_file(&done);
    }

    /// Samples go down little-endian and in the order given, because a channel
    /// swap or a byte swap both produce a file that plays and is wrong.
    #[test]
    fn samples_land_in_the_order_and_byte_order_they_were_given() {
        let path = temp("order");
        let mut wav = Wav::create(&path, 48_000).unwrap();
        wav.write(&[0x0102, i16::MIN, i16::MAX]).unwrap();
        let done = wav.close().unwrap();

        let raw = bytes(&done);
        let data = &raw[HEADER_BYTES as usize..];
        assert_eq!(&data[0..2], &[0x02, 0x01]);
        assert_eq!(&data[2..4], &i16::MIN.to_le_bytes());
        assert_eq!(&data[4..6], &i16::MAX.to_le_bytes());
        let _ = std::fs::remove_file(&done);
    }

    #[test]
    fn writing_in_pieces_is_the_same_as_writing_at_once() {
        let one = temp("one");
        let many = temp("many");
        let all: Vec<i16> = (0..1_000).map(|n| n as i16).collect();

        let mut a = Wav::create(&one, 48_000).unwrap();
        a.write(&all).unwrap();
        let a = a.close().unwrap();

        let mut b = Wav::create(&many, 48_000).unwrap();
        for chunk in all.chunks(37) {
            b.write(chunk).unwrap();
        }
        let b = b.close().unwrap();

        assert_eq!(bytes(&a), bytes(&b));
        let _ = std::fs::remove_file(&a);
        let _ = std::fs::remove_file(&b);
    }

    /// **The failure this is designed around.** A recording that never closes
    /// leaves the sizes at zero, so the samples are on disk and no player will
    /// touch them. Periodic size flushes keep it playable as it goes.
    #[test]
    fn a_flushed_file_is_playable_before_it_is_closed() {
        let path = temp("flushed");
        let mut wav = Wav::create(&path, 48_000).unwrap();
        wav.write(&[7; 100]).unwrap();
        wav.flush_sizes().unwrap();

        let raw = bytes(&path);
        assert_eq!(field(&raw, DATA_SIZE_AT as usize), 200);

        // And writing continues from the end rather than over the header.
        wav.write(&[9; 100]).unwrap();
        let done = wav.close().unwrap();
        let raw = bytes(&done);
        assert_eq!(field(&raw, DATA_SIZE_AT as usize), 400);
        assert_eq!(raw.len() as u64, HEADER_BYTES + 400);
        let _ = std::fs::remove_file(&done);
    }

    /// And a file that was never flushed at all can still be rescued from the
    /// one fact a crash cannot have falsified: its length on disk.
    #[test]
    fn a_file_left_open_by_a_crash_can_be_repaired() {
        let path = temp("crashed");
        {
            let mut wav = Wav::create(&path, 48_000).unwrap();
            wav.write(&[3; 500]).unwrap();
            // Dropped without closing, exactly as a crash would leave it.
        }
        let raw = bytes(&path);
        assert_eq!(field(&raw, DATA_SIZE_AT as usize), 0, "the header is empty");

        let frames = Wav::repair(&path).unwrap();
        assert_eq!(frames, 250, "five hundred samples is 250 stereo frames");
        let raw = bytes(&path);
        assert_eq!(field(&raw, DATA_SIZE_AT as usize), 1_000);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn repairing_an_empty_file_finds_nothing_rather_than_failing() {
        let path = temp("empty");
        let wav = Wav::create(&path, 48_000).unwrap();
        drop(wav);
        assert_eq!(Wav::repair(&path).unwrap(), 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn elapsed_time_follows_the_frames_written() {
        let path = temp("seconds");
        let mut wav = Wav::create(&path, 48_000).unwrap();
        assert_eq!(wav.seconds(), 0.0);
        wav.write(&[0; 96_000]).unwrap();
        assert!((wav.seconds() - 1.0).abs() < 1e-9);
        let done = wav.close().unwrap();
        let _ = std::fs::remove_file(&done);
    }
}
