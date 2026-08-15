//! The user's own logo, in place of the wordmark.
//!
//! A DJ's name is their business, and a booth screen that says someone else's
//! product name is a small daily insult. So the title area takes an image, and
//! the application steps out of the way.
//!
//! The image is **copied** into the app's config directory rather than
//! referenced where it was picked from. Referencing would mean the logo
//! vanishing because a USB stick was pulled or a Downloads folder was tidied —
//! and it would vanish in front of an audience, which is the worst moment for
//! anything to.
//!
//! Serving it uses the same trick as waveform tiles: a custom URI scheme, so
//! the webview loads it as an ordinary image instead of it being base64'd
//! through IPC on every render.

use std::path::{Path, PathBuf};
use tauri::Manager;

/// The URI scheme the logo is served on.
pub const SCHEME: &str = "brand";

/// Filename inside the config directory. Extension-less on purpose: the stored
/// bytes keep whatever format they arrived in, and the content type is sniffed
/// when serving rather than trusted from a name.
const LOGO_FILE: &str = "logo.img";

/// Refuse anything larger than this.
///
/// A logo is a logo. A 40 MB camera original in the title bar would be decoded
/// on every window resize, and the person who picked it would have no idea why
/// the application had become slow.
pub const MAX_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum BrandError {
    #[error("{0}")]
    Io(String),
    #[error("that file is {size} MB; a logo must be under {max} MB")]
    TooLarge { size: u64, max: u64 },
    #[error("that does not look like an image (expected PNG, JPEG, GIF, WebP or SVG)")]
    NotAnImage,
}

/// Where the logo lives, given the app's config directory.
#[must_use]
pub fn logo_path(config_dir: &Path) -> PathBuf {
    config_dir.join(LOGO_FILE)
}

/// Copy `source` in as the user's logo.
///
/// # Errors
/// If the file cannot be read, is too large, or is not an image.
pub fn set_logo(config_dir: &Path, source: &Path) -> Result<(), BrandError> {
    let metadata = std::fs::metadata(source).map_err(|e| BrandError::Io(e.to_string()))?;
    if metadata.len() > MAX_BYTES {
        return Err(BrandError::TooLarge {
            size: metadata.len() / (1024 * 1024),
            max: MAX_BYTES / (1024 * 1024),
        });
    }

    let bytes = std::fs::read(source).map_err(|e| BrandError::Io(e.to_string()))?;
    // Sniff rather than trust the extension. A renamed file would otherwise be
    // stored happily and then fail to render, with nothing saying why.
    if content_type(&bytes).is_none() {
        return Err(BrandError::NotAnImage);
    }

    std::fs::create_dir_all(config_dir).map_err(|e| BrandError::Io(e.to_string()))?;
    std::fs::write(logo_path(config_dir), &bytes).map_err(|e| BrandError::Io(e.to_string()))?;
    Ok(())
}

/// Go back to the wordmark.
pub fn clear_logo(config_dir: &Path) -> Result<(), BrandError> {
    match std::fs::remove_file(logo_path(config_dir)) {
        Ok(()) => Ok(()),
        // Already absent is the desired state, not a failure.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(BrandError::Io(e.to_string())),
    }
}

/// Read the stored logo, if there is one.
#[must_use]
pub fn read_logo(config_dir: &Path) -> Option<(Vec<u8>, &'static str)> {
    let bytes = std::fs::read(logo_path(config_dir)).ok()?;
    let mime = content_type(&bytes)?;
    Some((bytes, mime))
}

/// Identify an image from its leading bytes.
///
/// Deliberately a short list of what a browser will actually render, checked by
/// signature. Anything else is refused at the point the user picks it, when
/// there is still a person present to be told why.
#[must_use]
pub fn content_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Some("image/png");
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    // SVG is text, so look for the tag near the front, past any XML
    // declaration, BOM or licence comment.
    let head = &bytes[..bytes.len().min(1024)];
    if let Ok(text) = std::str::from_utf8(head)
        && text.contains("<svg")
    {
        return Some("image/svg+xml");
    }
    None
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn config_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path().app_config_dir().map_err(|e| e.to_string())
}

/// Replace the wordmark with the user's own image.
#[tauri::command]
pub fn set_brand_logo(app: tauri::AppHandle, path: String) -> Result<(), String> {
    set_logo(&config_dir(&app)?, Path::new(&path)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_brand_logo(app: tauri::AppHandle) -> Result<(), String> {
    clear_logo(&config_dir(&app)?).map_err(|e| e.to_string())
}

/// Whether a logo is set, so the header knows whether to draw one.
#[tauri::command]
pub fn has_brand_logo(app: tauri::AppHandle) -> bool {
    config_dir(&app).is_ok_and(|dir| logo_path(&dir).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("djmanzo-brand-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn png() -> Vec<u8> {
        let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        bytes.extend_from_slice(&[0u8; 64]);
        bytes
    }

    #[test]
    fn each_supported_format_is_recognised() {
        assert_eq!(content_type(&png()), Some("image/png"));
        assert_eq!(content_type(&[0xff, 0xd8, 0xff, 0xe0]), Some("image/jpeg"));
        assert_eq!(content_type(b"GIF89a....."), Some("image/gif"));

        let mut webp = b"RIFF".to_vec();
        webp.extend_from_slice(&[0u8; 4]);
        webp.extend_from_slice(b"WEBPVP8 ");
        assert_eq!(content_type(&webp), Some("image/webp"));

        assert_eq!(
            content_type(br#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg"/>"#),
            Some("image/svg+xml")
        );
    }

    #[test]
    fn something_that_is_not_an_image_is_refused() {
        assert_eq!(content_type(b"just some text"), None);
        assert_eq!(content_type(&[]), None);
        // A truncated RIFF header must not be read past its end.
        assert_eq!(content_type(b"RIFF"), None);
    }

    /// The point of copying rather than referencing: the logo must survive the
    /// original being moved or deleted.
    #[test]
    fn the_logo_is_copied_not_referenced() {
        let dir = temp_dir("copy");
        let source = dir.join("mylogo.png");
        std::fs::write(&source, png()).unwrap();

        let config = dir.join("config");
        set_logo(&config, &source).unwrap();
        std::fs::remove_file(&source).unwrap();

        let (bytes, mime) = read_logo(&config).expect("logo should have survived");
        assert_eq!(mime, "image/png");
        assert_eq!(bytes, png());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_logo_reads_as_nothing_rather_than_failing() {
        let dir = temp_dir("absent");
        assert!(read_logo(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn clearing_restores_the_wordmark_and_is_idempotent() {
        let dir = temp_dir("clear");
        let source = dir.join("logo.png");
        std::fs::write(&source, png()).unwrap();
        let config = dir.join("config");

        set_logo(&config, &source).unwrap();
        assert!(read_logo(&config).is_some());

        clear_logo(&config).unwrap();
        assert!(read_logo(&config).is_none());
        // Clearing twice must not error: the desired state is already true.
        clear_logo(&config).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A renamed file is the common mistake, and it has to be caught while
    /// there is still a person present to be told about it.
    #[test]
    fn a_renamed_non_image_is_refused_at_the_point_of_choosing() {
        let dir = temp_dir("renamed");
        let source = dir.join("definitely.png");
        std::fs::write(&source, b"this is a text file").unwrap();

        let config = dir.join("config");
        assert!(matches!(
            set_logo(&config, &source),
            Err(BrandError::NotAnImage)
        ));
        assert!(read_logo(&config).is_none(), "a bad file was stored anyway");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_enormous_file_is_refused_with_its_size() {
        let dir = temp_dir("huge");
        let source = dir.join("huge.png");
        let mut bytes = png();
        bytes.resize(MAX_BYTES as usize + 1024, 0);
        std::fs::write(&source, &bytes).unwrap();

        let error = set_logo(&dir.join("config"), &source).unwrap_err();
        assert!(matches!(error, BrandError::TooLarge { .. }));
        // The message has to say how big it was, or "too large" is unactionable.
        assert!(error.to_string().contains("8 MB"), "{error}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn replacing_a_logo_leaves_only_the_new_one() {
        let dir = temp_dir("replace");
        let config = dir.join("config");

        let first = dir.join("a.png");
        std::fs::write(&first, png()).unwrap();
        set_logo(&config, &first).unwrap();

        let second = dir.join("b.jpg");
        std::fs::write(&second, [0xff, 0xd8, 0xff, 0xe0, 1, 2, 3]).unwrap();
        set_logo(&config, &second).unwrap();

        let (_, mime) = read_logo(&config).unwrap();
        assert_eq!(mime, "image/jpeg", "the old logo survived a replacement");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_source_file_reports_io_rather_than_panicking() {
        let dir = temp_dir("missing");
        assert!(matches!(
            set_logo(&dir, Path::new("/nowhere/logo.png")),
            Err(BrandError::Io(_))
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
