//! Waveform summarisation and tile rasterisation.
//!
//! Per [ADR-0004](../../../docs/adr/0004-waveform-rendering-strategy.md), the
//! webview never draws the waveform. This crate turns audio into RGBA tiles; the
//! interface lays them end to end and scrolls them with a CSS transform, which
//! is compositor work rather than per-frame JavaScript.
//!
//! # Host-agnostic on purpose
//!
//! Nothing here knows about Tauri, a webview, or a window. It takes samples and
//! returns pixels. That is what makes the escape hatch in ADR-0004 real: if
//! WebKitGTK cannot keep up on Xubuntu, the shell can be replaced with a native
//! `wgpu` window without touching a line of this crate.
//!
//! ```text
//!   samples ─→ WaveformSummary ─→ render_tile ─→ RGBA bytes ─→ (any host)
//!              multi-resolution     column fill
//! ```

pub mod summary;
pub mod tile;

pub use summary::{Bucket, WaveformSummary};
pub use tile::{BYTES_PER_PIXEL, Palette, Theme, Tile, TileSpec, render_tile};

pub mod encode;
pub use encode::{EncodeError, encode_png};
