//! The camera and the microphone, as far as the webview is allowed to reach
//! them.
//!
//! djmanzo has two things that listen or look: the room surface (§34–§39),
//! which reads light, movement and loudness, and the hum in Memory, which reads
//! a tune. Both run in the page and ask for a device with `getUserMedia`. What
//! this module owns is the part below the page — whether the webview hands the
//! request to the operating system at all — which differs by platform and was,
//! until this module existed, **wrong on two of the three**.
//!
//! # What was wrong
//!
//! - **Linux.** WebKitGTK asks its host before it opens a device, through the
//!   `permission-request` signal, and a request nobody answers is *denied*.
//!   Tauri's webview layer answers it on macOS (it grants) and does not connect
//!   the signal on Linux at all. So on Linux a DJ with a working webcam would
//!   have pressed *Look at the room* and been told djmanzo was not allowed —
//!   and sent to system privacy settings that have no such switch.
//! - **macOS.** The operating system will not open a camera or a microphone for
//!   an application whose `Info.plist` does not say why it wants one. The
//!   bundle had no `Info.plist` at all. That is fixed beside `tauri.conf.json`,
//!   not here, because it is a packaging fact rather than a runtime one; this
//!   module's tests hold the file to having both sentences.
//! - **Windows.** WebView2 shows its own prompt, and the page's own error text
//!   covers a refusal. Nothing to do.
//!
//! # Only djmanzo's own page
//!
//! A granted request is a camera turned on, so the grant is not blanket: it is
//! given only while the webview is showing djmanzo's own interface, whichever
//! of the two addresses that is served from ([`is_own_page`]). djmanzo never
//! navigates its window anywhere else — links are handed to the system browser
//! — so this refuses nothing that happens today. It is here so that if that
//! ever changes, a page that is not djmanzo's does not inherit djmanzo's
//! microphone.
//!
//! # Testing without a camera
//!
//! WebKitGTK ships **mock capture devices** — a test-pattern camera and a tone
//! for a microphone — for exactly this. Setting [`MOCK_ENV`] turns them on, so
//! the whole path from the button to `room_saw` can be driven on a machine with
//! neither, in the webview djmanzo actually ships in. They are off unless the
//! variable is set: a DJ must never find a test pattern where their room
//! should be.

/// Set to anything to give the Linux webview WebKitGTK's mock camera and
/// microphone instead of real ones. For testing on a machine with neither.
pub const MOCK_ENV: &str = "DJMANZO_MOCK_CAPTURE";

/// Whether `uri` is djmanzo's own interface, and so may use the camera and
/// microphone.
///
/// Two addresses: `tauri://localhost` (and its `http(s)://tauri.localhost`
/// spelling, which is what the same page is served as on some platforms) for
/// the shipped build, and the loopback development server for a debug one.
/// Anything else — including a loopback address that merely *starts* with the
/// right characters — is not.
#[must_use]
pub fn is_own_page(uri: &str) -> bool {
    let Some((scheme, rest)) = uri.split_once("://") else {
        return false;
    };
    // The host is everything up to the first '/', '?' or '#', less any port.
    // Credentials in front of a host (`http://localhost@elsewhere.example/`,
    // which names elsewhere.example) need no rule of their own: they leave an
    // '@' in what is left, and no name on the list below has one.
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = match authority.rsplit_once(':') {
        Some((host, port)) if port.chars().all(|c| c.is_ascii_digit()) => host,
        _ => authority,
    };
    match scheme {
        "tauri" => host == "localhost",
        "http" | "https" => matches!(
            host,
            "tauri.localhost" | "localhost" | "127.0.0.1" | "[::1]"
        ),
        _ => false,
    }
}

/// Let this window's page open a camera and a microphone.
///
/// Called for the main window when djmanzo starts and for every panel as it is
/// detached, because the room surface can be either. A no-op off Linux: see
/// the module documentation for why.
pub fn permit(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "linux")]
    {
        let mock = std::env::var_os(MOCK_ENV).is_some();
        let outcome = window.with_webview(move |webview| {
            use webkit2gtk::glib::prelude::*;
            use webkit2gtk::{PermissionRequestExt, SettingsExt, UserMediaPermissionRequest, WebViewExt};

            let view = webview.inner();
            if let Some(settings) = WebViewExt::settings(&view) {
                settings.set_enable_media_stream(true);
                if mock {
                    settings.set_enable_mock_capture_devices(true);
                    tracing::warn!("{MOCK_ENV} is set: the camera and microphone are WebKit's mock devices");
                }
            }
            view.connect_permission_request(|view, request| {
                // Only a camera or microphone request is ours to answer. Any
                // other kind is left to WebKit's default, which refuses.
                if !request.is::<UserMediaPermissionRequest>() {
                    return false;
                }
                let own = view.uri().is_some_and(|uri| is_own_page(&uri));
                if own {
                    request.allow();
                } else {
                    tracing::warn!(uri = ?view.uri(), "refused a camera or microphone request from a page that is not djmanzo's");
                    request.deny();
                }
                true
            });
        });
        if let Err(error) = outcome {
            tracing::warn!(%error, "could not reach the webview to allow the camera and microphone");
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = window;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The load-bearing one: the grant is djmanzo's page and nothing else.**
    ///
    /// Every spelling djmanzo's own interface is served under is let in, and
    /// the lookalikes are not — a loopback address with credentials in front of
    /// another host, a subdomain that ends in `localhost`, a scheme that is not
    /// one of the three. A mistake in this function is a camera switched on
    /// for a page that is not ours.
    #[test]
    fn only_djmanzos_own_page_may_use_the_camera() {
        for own in [
            "tauri://localhost",
            "tauri://localhost/",
            "tauri://localhost/index.html?panel=room",
            "http://tauri.localhost/",
            "https://tauri.localhost/index.html",
            "http://localhost:5173/",
            "http://127.0.0.1:5173/?panel=room",
            "http://[::1]:5173/",
        ] {
            assert!(is_own_page(own), "{own} is djmanzo's own page");
        }
        for other in [
            "",
            "localhost",
            "tauri://elsewhere.example/",
            "https://elsewhere.example/",
            "http://localhost@elsewhere.example/",
            "http://localhost.elsewhere.example/",
            "http://evil-localhost/",
            "http://tauri.localhost.example/",
            "file:///home/dj/index.html",
            "ftp://localhost/",
            "http://localhost:notaport@elsewhere.example/",
        ] {
            assert!(!is_own_page(other), "{other} is not djmanzo's page");
        }
    }

    /// **The macOS half: the bundle says why it wants each device.**
    ///
    /// macOS refuses the camera and the microphone to an application whose
    /// `Info.plist` does not carry a reason for each, and djmanzo shipped with
    /// no `Info.plist` at all. Tauri merges the file beside `tauri.conf.json`
    /// into the bundle's, so this holds that file to both keys and to a
    /// sentence a DJ would recognise in the system prompt. A reason that is
    /// empty is refused the same as a missing one.
    #[test]
    fn the_mac_bundle_says_why_it_wants_the_camera_and_microphone() {
        let plist = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Info.plist"))
            .expect("crates/dj-app/Info.plist is what macOS reads the reasons from");
        for key in ["NSCameraUsageDescription", "NSMicrophoneUsageDescription"] {
            let at = plist
                .find(&format!("<key>{key}</key>"))
                .unwrap_or_else(|| panic!("{key} is missing"));
            let rest = &plist[at..];
            let open = rest.find("<string>").expect("a reason follows the key") + "<string>".len();
            let close = rest.find("</string>").expect("the reason is closed");
            let reason = rest[open..close].trim();
            assert!(
                reason.len() > 20,
                "{key} needs a real sentence, not {reason:?}"
            );
            assert!(
                reason.contains("djmanzo"),
                "{key} should say who is asking: {reason:?}"
            );
        }
    }
}
