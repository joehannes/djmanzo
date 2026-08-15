//! Turning tiles into something an `<img>` can display.
//!
//! # Why PNG rather than raw pixels
//!
//! The whole point of [ADR-0004](../../../docs/adr/0004-waveform-rendering-strategy.md)
//! is that the webview composites rather than draws. An `<img>` element moved by
//! a CSS transform is pure compositor work -- the browser decodes once, off the
//! main thread, and every subsequent frame is a layer translation.
//!
//! Handing over raw RGBA instead would mean `putImageData` into a canvas, which
//! is per-frame JavaScript drawing on the main thread. That is precisely the
//! pattern that collapses under WebKitGTK, and precisely what this design exists
//! to avoid. Paying a few milliseconds to encode once, on a worker, buys a
//! decode path that is someone else's optimised C.

use crate::tile::Tile;

#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error("tile is empty")]
    Empty,
    #[error("png encoding failed: {0}")]
    Png(String),
}

/// Encode a tile as a PNG.
///
/// Worker-thread work, not audio-thread and not UI-thread.
pub fn encode_png(tile: &Tile) -> Result<Vec<u8>, EncodeError> {
    if tile.pixels.is_empty() || tile.spec.width == 0 || tile.spec.height == 0 {
        return Err(EncodeError::Empty);
    }

    let mut out = Vec::with_capacity(tile.pixels.len() / 4);
    {
        let mut encoder = png::Encoder::new(&mut out, tile.spec.width, tile.spec.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        // `Fast` uses a DEFLATE implementation specialised for PNG that still
        // compresses better than other encoders' fastest modes. Tiles are
        // encoded on demand while a track loads, so encode time is felt and a
        // few percent of file size is not -- these never leave the machine.
        encoder.set_compression(png::Compression::Fast);

        let mut writer = encoder
            .write_header()
            .map_err(|e| EncodeError::Png(e.to_string()))?;
        writer
            .write_image_data(&tile.pixels)
            .map_err(|e| EncodeError::Png(e.to_string()))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Palette, TileSpec, WaveformSummary, render_tile};
    use dj_core::SampleRate;
    use std::f32::consts::PI;

    fn tile() -> Tile {
        let samples: Vec<f32> = (0..96_000)
            .flat_map(|n| {
                let v = (2.0 * PI * 440.0 * n as f32 / 48_000.0).sin() * 0.8;
                [v, v]
            })
            .collect();
        let summary = WaveformSummary::analyse(&samples, SampleRate::DEFAULT);
        render_tile(
            &summary,
            &TileSpec {
                width: 512,
                height: 128,
                start_frame: 0.0,
                frames_per_pixel: 128.0,
            },
            &Palette::default(),
        )
    }

    #[test]
    fn encodes_a_valid_png() {
        let bytes = encode_png(&tile()).unwrap();
        // PNG magic number -- proves we produced a real file, not just bytes.
        assert_eq!(
            &bytes[..8],
            &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]
        );
        assert!(bytes.len() > 100);
    }

    #[test]
    fn compression_actually_helps() {
        let tile = tile();
        let encoded = encode_png(&tile).unwrap();
        assert!(
            encoded.len() < tile.pixels.len() / 2,
            "expected real compression: {} raw -> {} encoded",
            tile.pixels.len(),
            encoded.len()
        );
    }

    #[test]
    fn an_empty_tile_is_reported_not_encoded() {
        let empty = Tile {
            spec: TileSpec {
                width: 0,
                height: 0,
                start_frame: 0.0,
                frames_per_pixel: 1.0,
            },
            pixels: Vec::new(),
        };
        assert!(matches!(encode_png(&empty), Err(EncodeError::Empty)));
    }

    #[test]
    fn encoding_is_deterministic() {
        // Tiles are cached by URL; two encodes of the same tile must not differ,
        // or a cache revalidation would swap in visibly different bytes.
        assert_eq!(encode_png(&tile()).unwrap(), encode_png(&tile()).unwrap());
    }
}
