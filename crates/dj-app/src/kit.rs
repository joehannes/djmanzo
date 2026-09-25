//! §118d: the DJ's own press kit.
//!
//! > have a place for the dj to save his details and all possible details he
//! > might want to pass on quickly in the event: personal details, music,
//! > songs, calender, pricings, job circumstantial fact sheets, how to be
//! > booked, profile, channels, social presences ... documents ...
//! > experience, photos ... the DJ can accumulate his personal stuff in a
//! > orchestrated way and on the occasion has easy and direct access to many
//! > share mechanisms that are prepackaged/optimised for typical practical
//! > occasions/opportunities.
//!
//! # One place, many occasions
//!
//! The kit is one file beside the settings (`kit.json`), with its photos and
//! documents **copied** into `kit/` for the reason the logo is
//! ([`crate::brand`]): a press photo that vanished because a USB stick was
//! pulled would vanish in front of the promoter. The calendar is not kept
//! twice — the dates already booked are the events' own ([`crate::gig`]).
//!
//! What the DJ does with it is an [`Occasion`], each composed here from the
//! same kit: a card for whoever asks at the booth (with a QR code of the
//! contact), an answer to a booking enquiry for a kind of night (with that
//! night's fee and the dates already taken), a post about the next night,
//! and the whole kit as one page for a venue. Each says what the kit still
//! lacks for it — the way an event's steps do — rather than sending a
//! message with a hole in it.
//!
//! # djmanzo prepares, the DJ sends
//!
//! [`crate::share`]'s rule, unchanged: nothing here posts or mails anything.
//! A composer opens with the words in it — the mail client, WhatsApp, a
//! network's own compose address — and the DJ reads, edits and sends. The
//! interface names a way, never an address: the address is built here.
//!
//! # Held to what can be sent
//!
//! [`check`] refuses what would go out wrong: an e-mail address that is not
//! one, a phone number with letters in it, a link that is not a web page (a
//! `javascript:` or `file:` link in a press kit is a link someone else will
//! click), a genre djmanzo does not know, a fee that says nothing.

use crate::setting::Setting;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The kit, as the DJ writes it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Kit {
    /// The name the DJ plays as.
    pub name: String,
    /// One line: what the DJ is.
    pub tagline: String,
    /// A paragraph or two.
    pub bio: String,
    /// Where the DJ is based, and plays.
    pub based: String,
    /// Genre families, by `dj_core::genre` name.
    pub genres: Vec<String>,
    /// One line each: residencies, notable nights, years behind the decks.
    pub experience: Vec<String>,
    pub email: String,
    pub phone: String,
    /// How to book: an agent, a deposit, what the rider asks for.
    pub booking: String,
    pub fees: Vec<Fee>,
    /// The DJ's pages: mixes, profiles, channels.
    pub links: Vec<String>,
    pub photos: Vec<Kept>,
    pub documents: Vec<Kept>,
}

/// What a kind of booking costs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Fee {
    /// The kind of night it is for, when it is for one.
    pub night: Option<Setting>,
    /// What is booked: "Up to five hours, sound included".
    pub what: String,
    /// As the DJ writes it: "€900", "from 600 €".
    pub price: String,
}

/// A photo or a document, copied into the kit's folder.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Kept {
    /// Its name in `kit/`: a plain file name, never a path.
    pub file: String,
    pub caption: String,
}

/// Why the kit cannot be kept as it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    Email(String),
    Phone(String),
    Link(String),
    Genre(String),
    Fee,
    File(String),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::Email(e) => write!(f, "{e:?} is not an e-mail address"),
            Refused::Phone(p) => write!(f, "{p:?} is not a phone number"),
            Refused::Link(l) => write!(f, "{l:?} is not a web address (http:// or https://)"),
            Refused::Genre(g) => write!(f, "djmanzo does not know the genre {g:?}"),
            Refused::Fee => f.write_str("a fee needs to say what is booked"),
            Refused::File(name) => write!(f, "{name:?} is not a file in the kit"),
        }
    }
}

/// Whether `email` looks like an address a mail client will take.
fn is_email(email: &str) -> bool {
    let Some((user, host)) = email.split_once('@') else {
        return false;
    };
    !user.is_empty()
        && host.contains('.')
        && !host.starts_with('.')
        && !host.ends_with('.')
        && !email
            .chars()
            .any(|c| c.is_whitespace() || c == '<' || c == '>')
}

/// Whether `phone` is a phone number: digits and the usual punctuation, and
/// enough digits to dial.
fn is_phone(phone: &str) -> bool {
    phone
        .chars()
        .all(|c| c.is_ascii_digit() || " +-()./".contains(c))
        && phone.chars().filter(char::is_ascii_digit).count() >= 6
}

/// The host of an `http(s)` address, or `None` when it is not one.
#[must_use]
pub fn host(link: &str) -> Option<&str> {
    let rest = link
        .strip_prefix("https://")
        .or_else(|| link.strip_prefix("http://"))?;
    if link.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return None;
    }
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    let host = authority.split(':').next().unwrap_or_default();
    // Letters, digits, dots and dashes only -- which also refuses the `@`
    // of `user@host`, how a link pretends to go somewhere it does not.
    let fine = host.contains('.')
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    fine.then_some(host)
}

/// What a link is, by where it goes: the name a reader recognises.
#[must_use]
pub fn site(link: &str) -> &'static str {
    let Some(host) = host(link) else {
        return "Link";
    };
    let host = host.trim_start_matches("www.").trim_start_matches("m.");
    let known: [(&str, &str); 16] = [
        ("instagram.com", "Instagram"),
        ("soundcloud.com", "SoundCloud"),
        ("mixcloud.com", "Mixcloud"),
        ("youtube.com", "YouTube"),
        ("youtu.be", "YouTube"),
        ("tiktok.com", "TikTok"),
        ("spotify.com", "Spotify"),
        ("bandcamp.com", "Bandcamp"),
        ("facebook.com", "Facebook"),
        ("x.com", "X"),
        ("twitter.com", "X"),
        ("bsky.app", "Bluesky"),
        ("threads.net", "Threads"),
        ("beatport.com", "Beatport"),
        ("ra.co", "Resident Advisor"),
        ("twitch.tv", "Twitch"),
    ];
    known
        .iter()
        .find(|(domain, _)| host == *domain || host.ends_with(&format!(".{domain}")))
        .map_or("Website", |(_, name)| name)
}

/// Whether `name` is a plain file name: no folder, no way out of `kit/`.
fn is_plain(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\'])
        && !name.starts_with('.')
}

/// Hold a kit to what can be sent: trimmed, blank lines and links dropped,
/// genres spelled djmanzo's way.
///
/// # Errors
/// See [`Refused`].
pub fn check(mut kit: Kit) -> Result<Kit, Refused> {
    for field in [
        &mut kit.name,
        &mut kit.tagline,
        &mut kit.bio,
        &mut kit.based,
        &mut kit.email,
        &mut kit.phone,
        &mut kit.booking,
    ] {
        *field = field.trim().to_owned();
    }
    if !kit.email.is_empty() && !is_email(&kit.email) {
        return Err(Refused::Email(kit.email));
    }
    if !kit.phone.is_empty() && !is_phone(&kit.phone) {
        return Err(Refused::Phone(kit.phone));
    }
    for list in [&mut kit.experience, &mut kit.links] {
        *list = list
            .iter()
            .map(|line| line.trim().to_owned())
            .filter(|line| !line.is_empty())
            .collect();
    }
    if let Some(bad) = kit.links.iter().find(|l| host(l).is_none()) {
        return Err(Refused::Link(bad.clone()));
    }
    for name in &mut kit.genres {
        let family =
            dj_core::genre::family_for(name).ok_or_else(|| Refused::Genre(name.clone()))?;
        family.name.clone_into(name);
    }
    let mut seen = Vec::new();
    kit.genres.retain(|g| {
        let fresh = !seen.contains(g);
        seen.push(g.clone());
        fresh
    });
    for fee in &mut kit.fees {
        fee.what = fee.what.trim().to_owned();
        fee.price = fee.price.trim().to_owned();
    }
    kit.fees
        .retain(|f| !(f.what.is_empty() && f.price.is_empty()));
    if kit.fees.iter().any(|f| f.what.is_empty()) {
        return Err(Refused::Fee);
    }
    for kept in kit.photos.iter().chain(&kit.documents) {
        if !is_plain(&kept.file) {
            return Err(Refused::File(kept.file.clone()));
        }
    }
    Ok(kit)
}

/// What a kit is used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Occasion {
    /// Someone at the booth asks who you are.
    Card,
    /// Someone asks what a night would cost.
    Enquiry,
    /// Telling people where you play next.
    Next,
    /// A venue or a promoter asks for the kit.
    Page,
}

impl Occasion {
    pub const ALL: [Occasion; 4] = [
        Occasion::Card,
        Occasion::Enquiry,
        Occasion::Next,
        Occasion::Page,
    ];

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Occasion::Card => "Your card",
            Occasion::Enquiry => "A booking enquiry",
            Occasion::Next => "Where you play next",
            Occasion::Page => "The whole kit",
        }
    }

    #[must_use]
    pub const fn when(self) -> &'static str {
        match self {
            Occasion::Card => "Someone at the booth asks who you are.",
            Occasion::Enquiry => "Someone asks what a night would cost.",
            Occasion::Next => "Telling people where to find you next.",
            Occasion::Page => "A venue or a promoter asks for your press kit.",
        }
    }

    /// The ways it can be handed over, in the order they are offered.
    #[must_use]
    pub const fn ways(self) -> &'static [Way] {
        match self {
            Occasion::Card => &[Way::Copy, Way::WhatsApp, Way::Email],
            Occasion::Enquiry => &[Way::Email, Way::WhatsApp, Way::Copy],
            Occasion::Next => &[Way::Copy, Way::WhatsApp, Way::X, Way::Bluesky, Way::Threads],
            Occasion::Page => &[Way::Save, Way::Email, Way::Copy],
        }
    }
}

/// How a composed occasion leaves djmanzo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Way {
    /// Onto the clipboard, which the interface does.
    Copy,
    /// The mail client, with the subject and the words in it.
    Email,
    WhatsApp,
    X,
    Bluesky,
    Threads,
    /// Written as a page in the kit's folder.
    Save,
}

impl Way {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Way::Copy => "Copy",
            Way::Email => "E-mail",
            Way::WhatsApp => "WhatsApp",
            Way::X => "X",
            Way::Bluesky => "Bluesky",
            Way::Threads => "Threads",
            Way::Save => "Save as a page",
        }
    }

    /// The network this is, for the ways [`crate::share`] already builds.
    const fn channel(self) -> Option<crate::share::Channel> {
        match self {
            Way::WhatsApp => Some(crate::share::Channel::WhatsApp),
            Way::X => Some(crate::share::Channel::X),
            Way::Bluesky => Some(crate::share::Channel::Bluesky),
            Way::Threads => Some(crate::share::Channel::Threads),
            _ => None,
        }
    }

    /// The address that opens a composer with `subject` and `text` in it;
    /// `None` for the ways that are not an address.
    #[must_use]
    pub fn address(self, subject: &str, text: &str) -> Option<String> {
        if self == Way::Email {
            return Some(format!(
                "mailto:?subject={}&body={}",
                urlencoding::encode(subject),
                urlencoding::encode(text)
            ));
        }
        self.channel().map(|channel| channel.compose_url(text))
    }

    /// Whether `text` can be handed over this way whole.
    #[must_use]
    pub fn fits(self, text: &str) -> bool {
        self.channel().is_none_or(|channel| channel.fits(text))
    }
}

/// A night already booked, from the events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Booked {
    pub date: String,
    pub title: String,
    pub place: String,
    pub starts: String,
}

/// The events' dates from `today` on, soonest first.
#[must_use]
pub fn booked(gigs: &[crate::gig::Gig], today: &str) -> Vec<Booked> {
    let mut out: Vec<Booked> = gigs
        .iter()
        .filter(|g| !g.date.is_empty() && g.date.as_str() >= today)
        .map(|g| Booked {
            date: g.date.clone(),
            title: g.title.clone(),
            place: g.place.clone(),
            starts: g.starts.clone(),
        })
        .collect();
    out.sort_by(|a, b| (&a.date, &a.starts).cmp(&(&b.date, &b.starts)));
    out
}

/// An occasion, composed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Composed {
    pub occasion: Occasion,
    pub title: &'static str,
    pub when: &'static str,
    /// The subject line, for the ways that have one.
    pub subject: String,
    pub text: String,
    /// What the kit still lacks for this occasion. Empty is ready.
    pub missing: Vec<&'static str>,
    pub ways: Vec<WayOffered>,
}

/// A way to hand an occasion over, and whether the text fits it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WayOffered {
    pub way: Way,
    pub name: &'static str,
    pub fits: bool,
}

fn contact(kit: &Kit) -> String {
    [kit.email.as_str(), kit.phone.as_str()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

fn fee_for(kit: &Kit, night: Option<Setting>) -> Option<&Fee> {
    kit.fees
        .iter()
        .find(|f| night.is_some() && f.night == night)
        .or_else(|| kit.fees.iter().find(|f| f.night.is_none()))
}

/// Compose `occasion` from the kit. `night` is the kind of night an enquiry
/// is for; `booked` the dates already taken, soonest first.
#[must_use]
pub fn compose(
    kit: &Kit,
    occasion: Occasion,
    night: Option<Setting>,
    booked: &[Booked],
) -> Composed {
    let name = if kit.name.is_empty() {
        "DJ"
    } else {
        kit.name.as_str()
    };
    let mut missing = Vec::new();
    if kit.name.is_empty() {
        missing.push("your name");
    }
    let reach = contact(kit);
    let first_link = kit.links.first().cloned().unwrap_or_default();
    let (subject, text) = match occasion {
        Occasion::Card => {
            if reach.is_empty() {
                missing.push("an e-mail or a phone number");
            }
            let mut lines = vec![name.to_owned()];
            if !kit.tagline.is_empty() {
                lines.push(kit.tagline.clone());
            }
            if !first_link.is_empty() {
                lines.push(first_link.clone());
            }
            if !reach.is_empty() {
                lines.push(format!("Bookings: {reach}"));
            }
            (name.to_owned(), lines.join("\n"))
        }
        Occasion::Enquiry => {
            if kit.bio.is_empty() {
                missing.push("a bio");
            }
            if reach.is_empty() {
                missing.push("an e-mail or a phone number");
            }
            let fee = fee_for(kit, night);
            if fee.is_none() {
                missing.push("a fee");
            }
            let mut parts = vec!["Hello, and thank you for getting in touch.".to_owned()];
            if !kit.bio.is_empty() {
                parts.push(kit.bio.clone());
            }
            if !kit.genres.is_empty() {
                parts.push(format!("What I play: {}.", kit.genres.join(", ")));
            }
            if !kit.experience.is_empty() {
                parts.push(kit.experience.join("\n"));
            }
            if let Some(fee) = fee {
                let what = match (night, fee.night) {
                    (Some(n), Some(f)) if n == f => {
                        format!("For a {} night", n.title().to_lowercase())
                    }
                    _ => "For a night".to_owned(),
                };
                let price = if fee.price.is_empty() {
                    String::new()
                } else {
                    format!(": {}", fee.price)
                };
                parts.push(format!("{what}: {}{price}.", fee.what));
            }
            if !booked.is_empty() {
                let dates: Vec<&str> = booked.iter().map(|b| b.date.as_str()).collect();
                parts.push(format!("Already booked: {}.", dates.join(", ")));
            }
            if !kit.booking.is_empty() {
                parts.push(kit.booking.clone());
            }
            let mut sign = vec![name.to_owned()];
            if !reach.is_empty() {
                sign.push(reach.clone());
            }
            sign.extend(kit.links.iter().cloned());
            parts.push(sign.join("\n"));
            (format!("{name} — booking"), parts.join("\n\n"))
        }
        Occasion::Next => {
            let next = booked.first();
            if next.is_none() {
                missing.push("an event with a date");
            }
            let text = match next {
                Some(b) => {
                    let mut line = format!("Next: {}", b.title);
                    if !b.place.is_empty() {
                        line.push_str(&format!(", {}", b.place));
                    }
                    line.push_str(&format!(" — {}", b.date));
                    if !b.starts.is_empty() {
                        line.push_str(&format!(" from {}", b.starts));
                    }
                    line.push('.');
                    if !first_link.is_empty() {
                        line.push_str(&format!(" {first_link}"));
                    }
                    line
                }
                None => String::new(),
            };
            (format!("{name} — next"), text)
        }
        Occasion::Page => {
            if kit.bio.is_empty() {
                missing.push("a bio");
            }
            if reach.is_empty() {
                missing.push("an e-mail or a phone number");
            }
            if kit.photos.is_empty() {
                missing.push("a photo");
            }
            (format!("{name} — press kit"), plain(kit, booked))
        }
    };
    let ways = occasion
        .ways()
        .iter()
        .map(|&way| WayOffered {
            way,
            name: way.name(),
            fits: way.fits(&text),
        })
        .collect();
    Composed {
        occasion,
        title: occasion.title(),
        when: occasion.when(),
        subject,
        text,
        missing,
        ways,
    }
}

/// The whole kit as text: what an e-mail carries when the page cannot go.
fn plain(kit: &Kit, booked: &[Booked]) -> String {
    let mut parts = Vec::new();
    let mut head = vec![kit.name.clone()];
    if !kit.tagline.is_empty() {
        head.push(kit.tagline.clone());
    }
    if !kit.based.is_empty() {
        head.push(kit.based.clone());
    }
    parts.push(head.join("\n"));
    if !kit.bio.is_empty() {
        parts.push(kit.bio.clone());
    }
    if !kit.genres.is_empty() {
        parts.push(format!("Music: {}", kit.genres.join(", ")));
    }
    if !kit.experience.is_empty() {
        parts.push(format!("Experience:\n{}", kit.experience.join("\n")));
    }
    if !kit.fees.is_empty() {
        let fees: Vec<String> = kit
            .fees
            .iter()
            .map(|f| {
                let night = f
                    .night
                    .map(|n| format!("{}: ", n.title()))
                    .unwrap_or_default();
                if f.price.is_empty() {
                    format!("{night}{}", f.what)
                } else {
                    format!("{night}{} — {}", f.what, f.price)
                }
            })
            .collect();
        parts.push(format!("Fees:\n{}", fees.join("\n")));
    }
    if !booked.is_empty() {
        let dates: Vec<&str> = booked.iter().map(|b| b.date.as_str()).collect();
        parts.push(format!("Already booked: {}", dates.join(", ")));
    }
    let mut book = Vec::new();
    if !kit.booking.is_empty() {
        book.push(kit.booking.clone());
    }
    let reach = contact(kit);
    if !reach.is_empty() {
        book.push(reach);
    }
    if !book.is_empty() {
        parts.push(format!("Bookings:\n{}", book.join("\n")));
    }
    if !kit.links.is_empty() {
        let links: Vec<String> = kit
            .links
            .iter()
            .map(|l| format!("{}: {l}", site(l)))
            .collect();
        parts.push(links.join("\n"));
    }
    parts.join("\n\n")
}

/// Escape text for HTML.
fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The whole kit as one page, its photos beside it in `kit/`.
///
/// A plain document with no scripts and nothing fetched: it opens from a
/// folder, from a USB stick, or attached to a mail, and prints as it looks.
#[must_use]
pub fn page(kit: &Kit, booked: &[Booked]) -> String {
    let mut body = String::new();
    body.push_str(&format!("<h1>{}</h1>\n", esc(&kit.name)));
    if !kit.tagline.is_empty() {
        body.push_str(&format!("<p class=\"tag\">{}</p>\n", esc(&kit.tagline)));
    }
    if !kit.based.is_empty() {
        body.push_str(&format!("<p class=\"based\">{}</p>\n", esc(&kit.based)));
    }
    if !kit.photos.is_empty() {
        body.push_str("<div class=\"photos\">\n");
        for photo in &kit.photos {
            body.push_str(&format!(
                "<figure><img src=\"{}\" alt=\"{}\"><figcaption>{}</figcaption></figure>\n",
                esc(&urlencoding::encode(&photo.file)),
                esc(&photo.caption),
                esc(&photo.caption)
            ));
        }
        body.push_str("</div>\n");
    }
    for paragraph in kit.bio.split("\n\n").filter(|p| !p.trim().is_empty()) {
        body.push_str(&format!("<p>{}</p>\n", esc(paragraph.trim())));
    }
    if !kit.genres.is_empty() {
        body.push_str(&format!(
            "<h2>Music</h2>\n<p>{}</p>\n",
            esc(&kit.genres.join(", "))
        ));
    }
    let list = |title: &str, items: &[String]| -> String {
        if items.is_empty() {
            return String::new();
        }
        let li: String = items
            .iter()
            .map(|i| format!("<li>{}</li>", esc(i)))
            .collect();
        format!("<h2>{title}</h2>\n<ul>{li}</ul>\n")
    };
    body.push_str(&list("Experience", &kit.experience));
    let fees: Vec<String> = kit
        .fees
        .iter()
        .map(|f| {
            let night = f
                .night
                .map(|n| format!("{}: ", n.title()))
                .unwrap_or_default();
            if f.price.is_empty() {
                format!("{night}{}", f.what)
            } else {
                format!("{night}{} — {}", f.what, f.price)
            }
        })
        .collect();
    body.push_str(&list("Fees", &fees));
    let dates: Vec<String> = booked
        .iter()
        .map(|b| {
            if b.place.is_empty() {
                format!("{} — {}", b.date, b.title)
            } else {
                format!("{} — {}, {}", b.date, b.title, b.place)
            }
        })
        .collect();
    body.push_str(&list("Already booked", &dates));
    let mut book = String::new();
    if !kit.booking.is_empty() {
        book.push_str(&format!("<p>{}</p>", esc(&kit.booking)));
    }
    if !kit.email.is_empty() {
        book.push_str(&format!(
            "<p><a href=\"mailto:{}\">{}</a></p>",
            esc(&kit.email),
            esc(&kit.email)
        ));
    }
    if !kit.phone.is_empty() {
        book.push_str(&format!("<p>{}</p>", esc(&kit.phone)));
    }
    if !book.is_empty() {
        body.push_str(&format!("<h2>Bookings</h2>\n{book}\n"));
    }
    if !kit.links.is_empty() {
        let li: String = kit
            .links
            .iter()
            .map(|l| format!("<li><a href=\"{}\">{}</a></li>", esc(l), esc(site(l))))
            .collect();
        body.push_str(&format!("<h2>Find me</h2>\n<ul>{li}</ul>\n"));
    }
    if !kit.documents.is_empty() {
        let li: String = kit
            .documents
            .iter()
            .map(|d| {
                let label = if d.caption.is_empty() {
                    &d.file
                } else {
                    &d.caption
                };
                format!(
                    "<li><a href=\"{}\">{}</a></li>",
                    esc(&urlencoding::encode(&d.file)),
                    esc(label)
                )
            })
            .collect();
        body.push_str(&format!("<h2>Documents</h2>\n<ul>{li}</ul>\n"));
    }
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{} — press kit</title>\n<style>\n{}\n</style>\n</head>\n<body>\n<main>\n{body}</main>\n</body>\n</html>\n",
        esc(&kit.name),
        PAGE_STYLE
    )
}

const PAGE_STYLE: &str = "body{font-family:system-ui,sans-serif;margin:0;background:#fff;color:#111;line-height:1.5}\
main{max-width:46rem;margin:0 auto;padding:2rem 1.25rem}\
h1{font-size:2.2rem;margin:0}\
.tag{font-size:1.2rem;margin:.2rem 0}\
.based{color:#555;margin:0 0 1rem}\
.photos{display:grid;grid-template-columns:repeat(auto-fit,minmax(12rem,1fr));gap:.75rem;margin:1rem 0}\
figure{margin:0}img{width:100%;height:auto;display:block;border-radius:.25rem}\
figcaption{font-size:.85rem;color:#555}\
h2{font-size:1.1rem;margin:1.5rem 0 .4rem}\
ul{padding-left:1.2rem}a{color:#0645ad}";

/// The contact as a vCard, for the card's QR code: a phone that scans it
/// offers to save the contact.
#[must_use]
pub fn vcard(kit: &Kit) -> String {
    // vCard's own escapes for text values.
    let v = |s: &str| {
        s.replace('\\', "\\\\")
            .replace(',', "\\,")
            .replace(';', "\\;")
            .replace('\n', "\\n")
    };
    let mut lines = vec![
        "BEGIN:VCARD".to_owned(),
        "VERSION:3.0".to_owned(),
        format!("FN:{}", v(&kit.name)),
    ];
    if !kit.phone.is_empty() {
        lines.push(format!("TEL:{}", v(&kit.phone)));
    }
    if !kit.email.is_empty() {
        lines.push(format!("EMAIL:{}", v(&kit.email)));
    }
    if let Some(link) = kit.links.first() {
        lines.push(format!("URL:{}", v(link)));
    }
    if !kit.tagline.is_empty() {
        lines.push(format!("NOTE:{}", v(&kit.tagline)));
    }
    lines.push("END:VCARD".to_owned());
    lines.join("\r\n")
}

/// A kit's first words, from what the welcome was told: the name and the
/// genres, so the DJ does not say them twice.
#[must_use]
pub fn begun(answers: &crate::welcome::Answers) -> Kit {
    Kit {
        name: answers.name.clone(),
        genres: answers.genres.clone(),
        ..Kit::default()
    }
}

/// `kit.json` beside the settings.
#[must_use]
pub fn file(config: &Path) -> PathBuf {
    config.join("kit.json")
}

/// The folder its photos and documents are copied into.
#[must_use]
pub fn folder(config: &Path) -> PathBuf {
    config.join("kit")
}

/// The kit kept, if one was.
#[must_use]
pub fn load(config: &Path) -> Option<Kit> {
    let text = std::fs::read_to_string(file(config)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Keep a kit, after [`check`].
///
/// # Errors
/// The refusal, or the file system's own sentence.
pub fn save(config: &Path, kit: Kit) -> Result<Kit, String> {
    let kit = check(kit).map_err(|refused| refused.to_string())?;
    for kept in kit.photos.iter().chain(&kit.documents) {
        if !folder(config).join(&kept.file).is_file() {
            return Err(Refused::File(kept.file.clone()).to_string());
        }
    }
    std::fs::create_dir_all(config).map_err(|e| format!("{}: {e}", config.display()))?;
    let text = serde_json::to_string_pretty(&kit).map_err(|e| e.to_string())?;
    let partial = config.join(".kit.json.partial");
    std::fs::write(&partial, text).map_err(|e| format!("{}: {e}", partial.display()))?;
    let path = file(config);
    std::fs::rename(&partial, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(kit)
}

/// The largest photo or document the kit copies in.
pub const MAX_BYTES: u64 = 32 * 1024 * 1024;

/// Copy `source` into the kit's folder under a name no other file there has,
/// and answer that name.
///
/// # Errors
/// A file that is not there, too large, or the file system's own sentence.
pub fn copy_in(config: &Path, source: &Path) -> Result<String, String> {
    let meta = std::fs::metadata(source).map_err(|e| format!("{}: {e}", source.display()))?;
    if !meta.is_file() {
        return Err(format!("{} is not a file", source.display()));
    }
    if meta.len() > MAX_BYTES {
        return Err(format!(
            "{} is larger than {} MB",
            source.display(),
            MAX_BYTES / 1024 / 1024
        ));
    }
    let dir = folder(config);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let original = source
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| {
            n.chars()
                .map(|c| {
                    if c.is_alphanumeric() || ".-_ ".contains(c) {
                        c
                    } else {
                        '_'
                    }
                })
                .collect::<String>()
        })
        .map(|n| n.trim_start_matches('.').to_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "file".to_owned());
    let (stem, ext) = match original.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_owned(), format!(".{ext}")),
        _ => (original.clone(), String::new()),
    };
    let mut name = original.clone();
    let mut n = 2;
    while dir.join(&name).exists() {
        name = format!("{stem} {n}{ext}");
        n += 1;
    }
    std::fs::copy(source, dir.join(&name)).map_err(|e| format!("{}: {e}", source.display()))?;
    Ok(name)
}

/// Read one of the kit's files for the interface, by its plain name only.
#[must_use]
pub fn read(config: &Path, name: &str) -> Option<(Vec<u8>, &'static str)> {
    if !is_plain(name) {
        return None;
    }
    let bytes = std::fs::read(folder(config).join(name)).ok()?;
    let mime = crate::brand::content_type(&bytes).unwrap_or("application/octet-stream");
    Some((bytes, mime))
}

/// The URI scheme the kit's photos are served on.
pub const SCHEME: &str = "kit";

/// Write the whole kit as `press-kit.html` beside its photos, and answer
/// where.
///
/// # Errors
/// The file system's own sentence.
pub fn write_page(config: &Path, kit: &Kit, booked: &[Booked]) -> Result<PathBuf, String> {
    let dir = folder(config);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join("press-kit.html");
    std::fs::write(&path, page(kit, booked)).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dj() -> Kit {
        Kit {
            name: " DJ Rosa ".to_owned(),
            tagline: "Latin and disco for rooms that dance".to_owned(),
            bio: "Ten years of weddings and beach bars.".to_owned(),
            based: "Vienna".to_owned(),
            genres: vec!["Disco".to_owned(), "disco".to_owned(), "salsa".to_owned()],
            experience: vec!["Resident at Sun Bar since 2019".to_owned(), "  ".to_owned()],
            email: "rosa@example.com".to_owned(),
            phone: "+43 660 123 4567".to_owned(),
            booking: "A deposit of 30% holds the date.".to_owned(),
            fees: vec![
                Fee {
                    night: Some(Setting::Wedding),
                    what: "Up to five hours, sound included".to_owned(),
                    price: "€900".to_owned(),
                },
                Fee {
                    night: None,
                    what: "A club night".to_owned(),
                    price: "€400".to_owned(),
                },
            ],
            links: vec!["https://soundcloud.com/djrosa".to_owned(), " ".to_owned()],
            photos: Vec::new(),
            documents: Vec::new(),
        }
    }

    /// **Held to what can be sent.** A kit is trimmed and spelled
    /// djmanzo's way, and what would go out wrong is refused: an address
    /// that is not one, a phone with letters, a link that is not a web page
    /// -- `javascript:`, `file:`, or one with credentials in it -- a genre
    /// djmanzo does not know, a fee that says nothing, and a file that
    /// reaches out of the kit's folder.
    #[test]
    fn a_kit_is_held_to_what_can_be_sent() {
        let kept = check(dj()).expect("the kit is usable");
        assert_eq!(kept.name, "DJ Rosa");
        assert_eq!(kept.experience, ["Resident at Sun Bar since 2019"]);
        assert_eq!(kept.links, ["https://soundcloud.com/djrosa"]);
        assert_eq!(kept.genres.len(), 2, "{:?}", kept.genres);

        let refused = |change: fn(&mut Kit)| {
            let mut kit = dj();
            change(&mut kit);
            check(kit).expect_err("refused")
        };
        assert!(matches!(
            refused(|k| k.email = "rosa at example".to_owned()),
            Refused::Email(_)
        ));
        assert!(matches!(
            refused(|k| k.phone = "call me".to_owned()),
            Refused::Phone(_)
        ));
        assert!(matches!(
            refused(|k| k.links = vec!["javascript:alert(1)".to_owned()]),
            Refused::Link(_)
        ));
        assert!(matches!(
            refused(|k| k.links = vec!["file:///etc/passwd".to_owned()]),
            Refused::Link(_)
        ));
        assert!(matches!(
            refused(|k| k.links = vec!["https://soundcloud.com@evil.example/x".to_owned()]),
            Refused::Link(_)
        ));
        assert!(matches!(
            refused(|k| k.genres = vec!["polka-step".to_owned()]),
            Refused::Genre(_)
        ));
        assert!(matches!(
            refused(|k| k.fees = vec![Fee {
                night: None,
                what: String::new(),
                price: "€5".to_owned()
            }]),
            Refused::Fee
        ));
        assert!(matches!(
            refused(|k| k.photos = vec![Kept {
                file: "../settings.json".to_owned(),
                caption: String::new()
            }]),
            Refused::File(_)
        ));
        // Unsaid is not wrong.
        assert_eq!(check(Kit::default()), Ok(Kit::default()));
    }

    /// A link says where it goes by the name a reader knows.
    #[test]
    fn a_link_is_named_by_where_it_goes() {
        assert_eq!(site("https://soundcloud.com/djrosa"), "SoundCloud");
        assert_eq!(site("https://www.instagram.com/djrosa/"), "Instagram");
        assert_eq!(site("https://m.youtube.com/@djrosa"), "YouTube");
        assert_eq!(site("https://djrosa.example"), "Website");
        assert_eq!(
            site("https://notinstagram.com/x"),
            "Website",
            "a suffix is not the site"
        );
    }

    /// **Each occasion says what it still lacks**, and composes from the
    /// kit: the card with the contact, the enquiry with that night's own fee
    /// and the dates already taken, the next night from the soonest event.
    #[test]
    fn each_occasion_composes_from_the_kit_and_says_what_it_lacks() {
        let kit = check(dj()).unwrap();
        let booked = vec![Booked {
            date: "2026-10-03".to_owned(),
            title: "Anna and Ben".to_owned(),
            place: "Schloss Hof".to_owned(),
            starts: "18:00".to_owned(),
        }];

        let card = compose(&kit, Occasion::Card, None, &booked);
        assert!(card.missing.is_empty(), "{:?}", card.missing);
        assert!(
            card.text.contains("DJ Rosa") && card.text.contains("rosa@example.com"),
            "{}",
            card.text
        );

        let wedding = compose(&kit, Occasion::Enquiry, Some(Setting::Wedding), &booked);
        assert!(wedding.missing.is_empty(), "{:?}", wedding.missing);
        assert!(
            wedding.text.contains("€900"),
            "the wedding's own fee: {}",
            wedding.text
        );
        assert!(!wedding.text.contains("€400"));
        assert!(wedding.text.contains("Already booked: 2026-10-03"));
        let club = compose(&kit, Occasion::Enquiry, Some(Setting::Club), &booked);
        assert!(
            club.text.contains("€400"),
            "no club fee: the general one: {}",
            club.text
        );

        let next = compose(&kit, Occasion::Next, None, &booked);
        assert!(
            next.text
                .starts_with("Next: Anna and Ben, Schloss Hof — 2026-10-03 from 18:00."),
            "{}",
            next.text
        );
        assert!(
            compose(&kit, Occasion::Next, None, &[])
                .missing
                .contains(&"an event with a date")
        );

        let page = compose(&kit, Occasion::Page, None, &booked);
        assert_eq!(page.missing, ["a photo"]);

        let bare = compose(&Kit::default(), Occasion::Card, None, &[]);
        assert_eq!(bare.missing, ["your name", "an e-mail or a phone number"]);
    }

    /// The ways out: the mail client with the subject and the words, a
    /// network's own composer, and whether the words fit it.
    #[test]
    fn the_ways_out_are_composers_with_the_words_in_them() {
        let mail = Way::Email
            .address("DJ Rosa — booking", "Hello & welcome")
            .unwrap();
        assert_eq!(
            mail,
            "mailto:?subject=DJ%20Rosa%20%E2%80%94%20booking&body=Hello%20%26%20welcome"
        );
        assert!(
            Way::WhatsApp
                .address("", "hi")
                .unwrap()
                .starts_with("https://wa.me/?text=")
        );
        assert_eq!(Way::Copy.address("", "hi"), None);
        assert_eq!(Way::Save.address("", "hi"), None);
        let long = "x".repeat(400);
        assert!(!Way::X.fits(&long));
        assert!(Way::Email.fits(&long));
    }

    /// **The page is safe to send**: what the DJ wrote is text, never
    /// markup, and a photo's name is a relative link that stays in the
    /// folder.
    #[test]
    fn the_page_is_the_kit_as_text_never_as_markup() {
        let mut kit = check(dj()).unwrap();
        kit.bio = "<script>alert(1)</script> & more".to_owned();
        kit.photos = vec![Kept {
            file: "rosa on stage.jpg".to_owned(),
            caption: "At \"Sun Bar\"".to_owned(),
        }];
        let html = page(&kit, &[]);
        assert!(!html.contains("<script>"), "{html}");
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt; &amp; more"));
        assert!(html.contains("src=\"rosa%20on%20stage.jpg\""));
        assert!(html.contains("At &quot;Sun Bar&quot;"));
        assert!(html.contains("<a href=\"https://soundcloud.com/djrosa\">SoundCloud</a>"));
    }

    /// The card's QR is a contact a phone offers to save.
    #[test]
    fn the_card_is_a_contact_a_phone_can_save() {
        let card = vcard(&check(dj()).unwrap());
        assert!(
            card.starts_with("BEGIN:VCARD\r\nVERSION:3.0\r\nFN:DJ Rosa\r\n"),
            "{card}"
        );
        assert!(card.contains("TEL:+43 660 123 4567") && card.contains("EMAIL:rosa@example.com"));
        assert!(card.ends_with("END:VCARD"));
        assert!(dj_net::sticker::qr_svg(&card).is_ok(), "fits in a QR code");
    }

    /// Photos are copied in, under a name nothing else in the folder has,
    /// and read back only by that plain name.
    #[test]
    fn photos_are_copied_in_and_read_only_by_name() {
        let dir = std::env::temp_dir().join(format!("djmanzo-kit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("rosa.png");
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        png.extend_from_slice(&[0u8; 16]);
        std::fs::write(&source, &png).unwrap();
        let config = dir.join("config");
        assert_eq!(copy_in(&config, &source).unwrap(), "rosa.png");
        assert_eq!(copy_in(&config, &source).unwrap(), "rosa 2.png");
        std::fs::remove_file(&source).unwrap();
        assert_eq!(
            read(&config, "rosa.png").map(|(_, m)| m),
            Some("image/png"),
            "copied, not referenced"
        );
        assert_eq!(read(&config, "../kit.json"), None);

        let mut kit = check(dj()).unwrap();
        kit.photos = vec![Kept {
            file: "rosa.png".to_owned(),
            caption: String::new(),
        }];
        assert!(save(&config, kit.clone()).is_ok());
        kit.photos[0].file = "missing.png".to_owned();
        assert!(
            save(&config, kit).is_err(),
            "a photo that is not in the folder"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
