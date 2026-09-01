//! The way into the room: a QR code, a URL, and something to print them on.
//!
//! # Why there are two addresses and not one
//!
//! A sticker is printed before the night and stuck to a table. Whatever is on
//! it has to be true in a venue nobody has visited yet — which rules out the
//! laptop's address, because that is handed out by whatever router the venue
//! happens to own and is different every time.
//!
//! So djmanzo offers both, and says which is which:
//!
//! - **[`Kind::Name`]** — `http://djmanzo.local:7331/`. The same every night,
//!   so it can be printed in advance. It works because [`crate::announce`]
//!   answers for that name on the local network. Apple devices resolve
//!   `.local` without being asked to; Android has since 12 and does not always
//!   manage it; a few browsers never will.
//! - **[`Kind::Lan`]** — `http://192.168.1.42:7331/`. Certain to work on the
//!   network it was read from, and worthless on any other. This is the one for
//!   the QR code shown on a screen, or printed at the venue an hour before.
//!
//! Neither is hidden behind the other. A DJ who prints the stable one and
//! finds that half the phones in the room cannot open it needs to know that
//! was a known trade and that the screen QR is the fallback — not to discover
//! it from a shrug.

use std::net::{IpAddr, SocketAddr, UdpSocket};

/// The port djmanzo asks for by default.
///
/// Not a port anything else wants, short enough to type from a sticker, and
/// above 1024 so starting the server never needs privileges.
pub const DEFAULT_PORT: u16 = 7331;

/// The name djmanzo answers to on the local network.
///
/// Without the trailing dot, which is what goes in a URL; [`crate::announce`]
/// adds it back for DNS.
pub const LOCAL_NAME: &str = "djmanzo.local";

/// Which kind of address this is, and therefore what it is good for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The same every night. Printable in advance, resolved by most phones.
    Name,
    /// Tonight's address on tonight's network. Certain, and not portable.
    Lan,
}

impl Kind {
    /// The sentence djmanzo shows next to the address, which is the honest
    /// part: an address with a caveat and no caveat printed is a trap.
    #[must_use]
    pub fn caveat(self) -> &'static str {
        match self {
            Self::Name => {
                "The same at every venue, so it can be printed in advance. \
                 iPhones open it; most Android phones since Android 12 do; \
                 a few browsers will not."
            }
            Self::Lan => {
                "Certain to work — on this network only. Show it on a screen, \
                 or print it at the venue."
            }
        }
    }
}

/// One way in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WayIn {
    pub kind: Kind,
    pub url: String,
}

impl WayIn {
    #[must_use]
    pub fn name(port: u16) -> Self {
        Self {
            kind: Kind::Name,
            url: url_for(LOCAL_NAME, port),
        }
    }

    #[must_use]
    pub fn lan(address: IpAddr, port: u16) -> Self {
        Self {
            kind: Kind::Lan,
            url: url_for(&host_in_url(address), port),
        }
    }
}

/// `http://host/` when the port is the usual one, `http://host:port/` when it
/// is not — because a URL with no port is a URL somebody can retype from a
/// sticker without getting it wrong.
fn url_for(host: &str, port: u16) -> String {
    if port == 80 {
        format!("http://{host}/")
    } else {
        format!("http://{host}:{port}/")
    }
}

/// An IPv6 address needs its brackets before it is a URL.
fn host_in_url(address: IpAddr) -> String {
    match address {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => format!("[{v6}]"),
    }
}

/// This machine's address on the network the phones are on.
///
/// Asked by opening a UDP socket towards an address in `TEST-NET-1` — reserved
/// for documentation, routed nowhere — and reading back which of this
/// machine's addresses the kernel would have used. **Nothing is sent.** A UDP
/// `connect` only fixes a route.
///
/// This is deliberately the address of the *default route*, not a list of
/// every interface. A laptop in a booth has a wifi address, possibly an
/// ethernet one, a docker bridge, and several link-local addresses, and only
/// one of them is the one a phone in the room can reach.
#[must_use]
pub fn lan_address() -> Option<IpAddr> {
    let socket = UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], 0))).ok()?;
    socket.connect(SocketAddr::from(([192, 0, 2, 1], 9))).ok()?;
    let found = socket.local_addr().ok()?.ip();
    // A machine with no route at all reports the unspecified address, which is
    // not somewhere a phone can go.
    (!found.is_unspecified()).then_some(found)
}

/// A QR code for `url`, as an SVG element.
///
/// Returned as SVG rather than a PNG because it is going into a page and onto
/// paper: it has to stay sharp at whatever size a printer decides, and it has
/// to survive being embedded in a document with no file alongside it.
///
/// # Errors
/// When the URL is too long to encode, which for a URL of this shape it is not.
pub fn qr_svg(url: &str) -> Result<String, String> {
    use qrcode::render::svg;
    let code = qrcode::QrCode::new(url.as_bytes()).map_err(|why| why.to_string())?;
    let rendered = code
        .render()
        .min_dimensions(180, 180)
        // Black on white, always. A QR reader in a dark room is looking
        // through a camera at paper, and paper is white.
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();

    // The renderer writes a standalone document, prolog and all. Inside an
    // HTML page that prolog is not a declaration, it is a bogus comment -- the
    // HTML parser has no XML mode -- so it is cut here rather than shipped and
    // hoped over.
    let inline = rendered
        .find("<svg")
        .map_or(rendered.as_str(), |start| &rendered[start..]);
    Ok(inline.to_owned())
}

/// What goes on the printed sheet.
#[derive(Debug, Clone)]
pub struct Sticker<'a> {
    /// The night's name, or the DJ's. Blank is fine.
    pub heading: &'a str,
    /// "Request a song", in the room's language.
    pub call: &'a str,
    pub way_in: &'a WayIn,
    /// How many copies to lay out on the sheet.
    pub copies: usize,
}

/// The most stickers one sheet holds.
///
/// Twelve on A4 at three columns, which is a sticker about the size of a beer
/// mat's label — big enough that the QR reads from arm's length across a
/// table, small enough that a sheet is worth printing.
pub const MOST_COPIES: usize = 12;

/// A printable sheet of identical stickers.
///
/// Self-contained, like everything else served here, and laid out for paper:
/// a print stylesheet with an A4 page box, no background colours to drink a
/// cartridge, and a cut border on each.
///
/// # Errors
/// When the URL cannot be encoded as a QR code.
pub fn sheet(sticker: &Sticker) -> Result<String, String> {
    let qr = qr_svg(&sticker.way_in.url)?;
    let copies = sticker.copies.clamp(1, MOST_COPIES);

    let mut html = String::with_capacity(8192);
    html.push_str("<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>");
    html.push_str(&escape(sticker.heading));
    html.push_str(" — djmanzo</title>\n<style>\n");
    html.push_str(SHEET_STYLE);
    html.push_str("</style>\n</head>\n<body>\n<div class=\"sheet\">\n");
    for _ in 0..copies {
        html.push_str("<div class=\"sticker\">\n<div class=\"qr\">");
        html.push_str(&qr);
        html.push_str("</div>\n<p class=\"call\">");
        html.push_str(&escape(sticker.call));
        html.push_str("</p>\n<p class=\"url\">");
        html.push_str(&escape(strip_scheme(&sticker.way_in.url)));
        html.push_str("</p>\n");
        if !sticker.heading.trim().is_empty() {
            html.push_str("<p class=\"who\">");
            html.push_str(&escape(sticker.heading));
            html.push_str("</p>\n");
        }
        html.push_str("</div>\n");
    }
    html.push_str("</div>\n</body>\n</html>\n");
    Ok(html)
}

/// `http://x/` shown as `x` — the scheme is noise on paper, and every phone
/// camera and browser puts it back.
fn strip_scheme(url: &str) -> &str {
    url.strip_prefix("http://")
        .unwrap_or(url)
        .strip_suffix('/')
        .unwrap_or_else(|| url.strip_prefix("http://").unwrap_or(url))
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

const SHEET_STYLE: &str = "\
@page{size:A4;margin:10mm}
*{box-sizing:border-box}
body{margin:0;background:#fff;color:#000;
 font:12px/1.35 system-ui,-apple-system,'Segoe UI',Roboto,sans-serif}
.sheet{display:grid;grid-template-columns:repeat(3,1fr);gap:4mm;padding:6mm}
.sticker{border:1px dashed #999;border-radius:3mm;padding:4mm 3mm;text-align:center;
 break-inside:avoid;page-break-inside:avoid}
.qr svg{width:100%;height:auto;max-width:34mm;display:block;margin:0 auto}
.call{margin:2.5mm 0 1mm;font-size:11px;font-weight:600}
.url{margin:0;font-size:10px;font-family:ui-monospace,'SFMono-Regular',Menlo,monospace;
 overflow-wrap:anywhere}
.who{margin:1.5mm 0 0;font-size:9px;color:#555}
@media screen{body{background:#f4f4f5;padding:8mm}
 .sheet{background:#fff;max-width:210mm;margin:0 auto;box-shadow:0 1px 6px rgba(0,0,0,.2)}}
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_url_hides_the_port_only_when_it_is_the_default_one() {
        assert_eq!(WayIn::name(DEFAULT_PORT).url, "http://djmanzo.local:7331/");
        assert_eq!(WayIn::name(80).url, "http://djmanzo.local/");
    }

    /// **An IPv6 address is bracketed, or it is not a URL.**
    #[test]
    fn an_ipv6_address_gets_its_brackets() {
        let six: IpAddr = "fe80::1".parse().expect("address");
        assert_eq!(WayIn::lan(six, 7331).url, "http://[fe80::1]:7331/");
        let four: IpAddr = "192.168.1.42".parse().expect("address");
        assert_eq!(WayIn::lan(four, 7331).url, "http://192.168.1.42:7331/");
    }

    /// **Both ways in say what they are good for.**
    #[test]
    fn every_way_in_carries_its_caveat() {
        for kind in [Kind::Name, Kind::Lan] {
            assert!(!kind.caveat().trim().is_empty(), "{kind:?}");
        }
        assert_ne!(Kind::Name.caveat(), Kind::Lan.caveat());
    }

    /// **The QR encodes the URL and is an SVG.**
    #[test]
    fn a_qr_code_is_an_svg_that_scales() {
        let svg = qr_svg("http://192.168.1.42:7331/").expect("qr");
        // No XML prolog: this goes inside an HTML document, where a prolog is
        // parsed as a stray comment rather than as anything at all.
        assert!(svg.starts_with("<svg"), "{}", &svg[..60.min(svg.len())]);
        assert!(!svg.contains("<?xml"), "the prolog survived");
        assert!(
            svg.contains("viewBox"),
            "a QR that cannot scale is a bitmap"
        );
        // A QR of this URL is at least version 2; a blank render would not be.
        assert!(svg.len() > 500, "suspiciously small: {} bytes", svg.len());
    }

    /// **The sheet prints the number of copies asked for, and no more.**
    #[test]
    fn a_sheet_lays_out_the_copies_it_was_asked_for() {
        let way_in = WayIn::name(DEFAULT_PORT);
        let sticker = Sticker {
            heading: "Sábado en La Guácara",
            call: "Pide una canción",
            way_in: &way_in,
            copies: 6,
        };
        let html = sheet(&sticker).expect("sheet");
        assert_eq!(html.matches("class=\"sticker\"").count(), 6);
        assert!(html.contains("djmanzo.local:7331"));
        assert!(html.contains("Pide una canción"));
        assert!(html.contains("@page"), "the sheet has no page box");
    }

    /// **A ridiculous number of copies is clamped, not obeyed.**
    #[test]
    fn the_sheet_will_not_print_a_thousand() {
        let way_in = WayIn::name(DEFAULT_PORT);
        let mut sticker = Sticker {
            heading: "",
            call: "Request a song",
            way_in: &way_in,
            copies: 1_000,
        };
        assert_eq!(
            sheet(&sticker)
                .expect("sheet")
                .matches("class=\"sticker\"")
                .count(),
            MOST_COPIES
        );
        sticker.copies = 0;
        assert_eq!(
            sheet(&sticker)
                .expect("sheet")
                .matches("class=\"sticker\"")
                .count(),
            1
        );
    }

    /// **A blank heading leaves no empty line on the paper.**
    #[test]
    fn a_sticker_with_no_name_has_no_name_on_it() {
        let way_in = WayIn::lan("10.0.0.5".parse().expect("address"), 7331);
        let html = sheet(&Sticker {
            heading: "   ",
            call: "Request a song",
            way_in: &way_in,
            copies: 1,
        })
        .expect("sheet");
        assert!(!html.contains("class=\"who\""), "{html}");
    }

    /// **A venue's name cannot become markup on the printed page.**
    #[test]
    fn the_heading_is_escaped_on_paper_too() {
        let way_in = WayIn::name(DEFAULT_PORT);
        let html = sheet(&Sticker {
            heading: "<script>bad()</script>",
            call: "Request a song",
            way_in: &way_in,
            copies: 1,
        })
        .expect("sheet");
        assert!(!html.contains("<script>"), "{html}");
    }

    #[test]
    fn the_scheme_comes_off_for_reading() {
        assert_eq!(
            strip_scheme("http://djmanzo.local:7331/"),
            "djmanzo.local:7331"
        );
        assert_eq!(strip_scheme("http://10.0.0.5/"), "10.0.0.5");
    }

    /// **Asking for this machine's address sends nothing and answers something.**
    ///
    /// In a container with one interface this is that interface; the assertion
    /// is only that it is a real address rather than the unspecified one,
    /// because a machine with no route is a machine no phone can reach and the
    /// interface has to say so instead of printing `0.0.0.0` on a sticker.
    #[test]
    fn this_machines_address_is_a_real_one_or_none() {
        if let Some(found) = lan_address() {
            assert!(!found.is_unspecified(), "{found}");
        }
    }
}
