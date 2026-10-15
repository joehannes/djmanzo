//! §118d: filling the kit from the DJ's own pages.
//!
//! > a place for the dj to save his details ... profile, channels, social
//! > presences
//!
//! A DJ's one line and bio are usually written already, on the pages the kit
//! links to. *Fill from your pages* reads each of those links **once, on the
//! DJ's press** — never in the background — and takes only what a page
//! publishes for a link preview: `og:title`, `og:description`, `og:image`,
//! the `description` meta and the `<title>`. Nothing behind a login, and
//! nothing of the page's body.
//!
//! # It proposes, never writes
//!
//! Each [`Suggestion`] names the page it came from and the field it would
//! fill; the DJ takes it or leaves it, and taking it is an ordinary edit of
//! the kit. Nothing here changes `kit.json`. A field already filled is
//! offered a replacement — the interface says so — and never overwritten;
//! a suggestion that says what the kit already says is not made at all.
//!
//! # Held to what can be trusted from a stranger's server
//!
//! A page is fetched only at an address [`crate::kit::host`] accepts, the
//! first [`MOST_PAGE`] bytes of it read and the rest never asked for, within
//! [`WAIT`]. Its words are stripped of tags and their entities decoded, the
//! one line capped at [`MOST_LINE`] characters and the bio at [`MOST_BIO`]. A
//! page that answers with no preview — the ones behind a login often do — is
//! named as such ([`Read::said`]) rather than skipped silently. A photo is
//! fetched only when the DJ takes it, at most [`MOST_PHOTO`] bytes, and only
//! a picture djmanzo can show is kept: a photo on a page may be a
//! photographer's to give, which the interface says beside it.

use serde::Serialize;
use std::time::Duration;

/// The most of a page that is read. A preview is in the page's head; a
/// megabyte is far more head than any page has.
pub const MOST_PAGE: usize = 1024 * 1024;

/// The most of a photo that is fetched.
pub const MOST_PHOTO: usize = 10 * 1024 * 1024;

/// How long a page or a photo is waited for.
pub const WAIT: Duration = Duration::from_secs(10);

/// The longest one line the kit is offered, in characters.
pub const MOST_LINE: usize = 140;

/// The longest bio the kit is offered, in characters.
pub const MOST_BIO: usize = 1200;

/// What a page publishes for a link preview, its words plain.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Preview {
    /// `og:title`, or the `<title>`.
    pub title: Option<String>,
    /// `og:description`, or the `description` meta.
    pub description: Option<String>,
    /// `og:image`, as an absolute `http(s)` address.
    pub image: Option<String>,
}

impl Preview {
    /// Whether the page published nothing to preview.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.description.is_none() && self.image.is_none()
    }
}

/// Which of the kit's fields a suggestion would fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Field {
    /// The kit's one line (`tagline`).
    Line,
    /// The bio.
    Bio,
    /// A photo, fetched only when taken.
    Photo,
}

/// One thing a page offers the kit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Suggestion {
    pub field: Field,
    /// The words, or a photo's address.
    pub value: String,
}

/// What one of the kit's links gave.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Read {
    pub link: String,
    /// How the link is named ([`crate::kit::site`]).
    pub site: &'static str,
    /// What the page calls itself, when it says.
    pub title: Option<String>,
    pub suggestions: Vec<Suggestion>,
    /// Why there is nothing to offer, when there is not: no preview, an
    /// answer that was not a page, or no answer.
    pub said: Option<String>,
}

// ---------------------------------------------------------------------------
// Reading a page
// ---------------------------------------------------------------------------

/// The page's head, or as much of the page as there is when it has none
/// marked: a preview is never in the body, and a body's `<meta>` (an
/// embedded post, a quoted page) is somebody else's.
fn head(html: &str) -> &str {
    let lower = html.to_ascii_lowercase();
    let end = lower
        .find("</head")
        .or_else(|| lower.find("<body"))
        .unwrap_or(html.len());
    &html[..end]
}

/// Each tag named `name` in `html`: the text between `<name` and `>`, and
/// where the `>` is.
fn tags<'a>(html: &'a str, name: &str) -> Vec<(&'a str, usize)> {
    let lower = html.to_ascii_lowercase();
    let open = format!("<{name}");
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = lower[from..].find(&open) {
        let start = from + at + open.len();
        // `<metadata` is not `<meta`.
        let next = lower[start..].chars().next();
        if !next.is_some_and(|c| c.is_ascii_whitespace() || c == '/' || c == '>') {
            from = start;
            continue;
        }
        let Some(end) = attributes_end(&html[start..]) else {
            break;
        };
        found.push((&html[start..start + end], start + end));
        from = start + end;
    }
    found
}

/// Where a tag's attributes end: the first `>` outside a quoted value.
fn attributes_end(rest: &str) -> Option<usize> {
    let mut quote = None;
    for (i, c) in rest.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            (None, '>') => return Some(i),
            _ => {}
        }
    }
    None
}

/// The attributes of a tag, names in lower case, values as written.
fn attributes(tag: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut chars = tag.char_indices().peekable();
    loop {
        while chars
            .next_if(|(_, c)| c.is_ascii_whitespace() || *c == '/')
            .is_some()
        {}
        let Some(&(start, _)) = chars.peek() else {
            break;
        };
        let mut end = start;
        while let Some((i, c)) =
            chars.next_if(|(_, c)| !c.is_ascii_whitespace() && !"=/>".contains(*c))
        {
            end = i + c.len_utf8();
        }
        let name = tag[start..end].to_ascii_lowercase();
        while chars.next_if(|(_, c)| c.is_ascii_whitespace()).is_some() {}
        let mut value = String::new();
        if chars.next_if(|(_, c)| *c == '=').is_some() {
            while chars.next_if(|(_, c)| c.is_ascii_whitespace()).is_some() {}
            match chars.peek().map(|&(_, c)| c) {
                Some(q @ ('"' | '\'')) => {
                    chars.next();
                    for (_, c) in chars.by_ref() {
                        if c == q {
                            break;
                        }
                        value.push(c);
                    }
                }
                _ => {
                    while let Some((_, c)) = chars.next_if(|(_, c)| !c.is_ascii_whitespace()) {
                        value.push(c);
                    }
                }
            }
        }
        if name.is_empty() {
            // Nothing that reads as a name: step past it rather than loop.
            if chars.next().is_none() {
                break;
            }
            continue;
        }
        out.push((name, value));
    }
    out
}

/// One character reference's character, when it is one djmanzo reads.
fn entity(name: &str) -> Option<char> {
    if let Some(number) = name.strip_prefix('#') {
        let code = match number.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => number.parse().ok()?,
        };
        return char::from_u32(code).filter(|c| !c.is_control() || c.is_whitespace());
    }
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => ' ',
        "ndash" => '–',
        "mdash" => '—',
        "hellip" => '…',
        "lsquo" => '‘',
        "rsquo" => '’',
        "ldquo" => '“',
        "rdquo" => '”',
        "middot" => '·',
        "bull" => '•',
        "copy" => '©',
        "reg" => '®',
        "trade" => '™',
        "eacute" => 'é',
        "aacute" => 'á',
        "iacute" => 'í',
        "oacute" => 'ó',
        "uacute" => 'ú',
        "ntilde" => 'ñ',
        "uuml" => 'ü',
        "ouml" => 'ö',
        "auml" => 'ä',
        "szlig" => 'ß',
        _ => return None,
    })
}

/// Words as a reader sees them: tags taken out, entities decoded, every run
/// of white space one space, and nothing that is not printable.
#[must_use]
pub fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        let opens_tag = rest[1..]
            .chars()
            .next()
            .is_some_and(|n| n.is_ascii_alphabetic() || n == '/' || n == '!');
        if c == '<' && opens_tag {
            // A tag in a value (`<b>`, or a `<script>` somebody hoped would
            // run in a preview) is taken out, not shown; a `<` that opens
            // nothing (`I <3 bachata`) is a word.
            match attributes_end(&rest[1..]) {
                Some(end) => {
                    rest = &rest[end + 2..];
                    out.push(' ');
                    continue;
                }
                None => break,
            }
        }
        if c == '&'
            && let Some(end) = rest[1..].find(';').filter(|&e| e <= 10)
            && let Some(decoded) = entity(&rest[1..=end])
        {
            out.push(decoded);
            rest = &rest[end + 2..];
            continue;
        }
        out.push(if c.is_whitespace() || c.is_control() {
            ' '
        } else {
            c
        });
        rest = &rest[c.len_utf8()..];
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `src` as an absolute address, read against the page it was found on.
/// `None` when it cannot be one djmanzo would fetch.
#[must_use]
pub fn absolute(page: &str, src: &str) -> Option<String> {
    let src = src.trim();
    let address = if crate::kit::host(src).is_some() {
        src.to_owned()
    } else {
        let scheme = if page.starts_with("https://") {
            "https:"
        } else {
            "http:"
        };
        let host = crate::kit::host(page)?;
        let port = page
            .split_once("://")
            .map(|(_, rest)| rest)
            .and_then(|rest| rest.split(['/', '?', '#']).next())
            .and_then(|authority| authority.split_once(':'))
            .map(|(_, port)| format!(":{port}"))
            .unwrap_or_default();
        if let Some(rest) = src.strip_prefix("//") {
            format!("{scheme}//{rest}")
        } else if src.starts_with('/') {
            format!("{scheme}//{host}{port}{src}")
        } else {
            return None;
        }
    };
    crate::kit::host(&address).is_some().then_some(address)
}

/// What `html`, found at `page`, publishes for a link preview.
#[must_use]
pub fn read(page: &str, html: &str) -> Preview {
    let head = head(html);
    let mut og_title = None;
    let mut og_description = None;
    let mut og_image = None;
    let mut description = None;
    for (tag, _) in tags(head, "meta") {
        let attrs = attributes(tag);
        let get = |name: &str| {
            attrs
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.as_str())
        };
        let key = get("property")
            .or_else(|| get("name"))
            .unwrap_or_default()
            .to_ascii_lowercase();
        let Some(content) = get("content") else {
            continue;
        };
        let slot = match key.as_str() {
            "og:title" => &mut og_title,
            "og:description" => &mut og_description,
            "og:image" | "og:image:url" | "og:image:secure_url" => &mut og_image,
            "description" => &mut description,
            _ => continue,
        };
        if slot.is_none() {
            *slot = Some(content.to_owned());
        }
    }
    let title = tags(head, "title").first().map(|&(_, close)| {
        let at = close + 1;
        let lower = head[at..].to_ascii_lowercase();
        let end = lower.find("</title").unwrap_or(lower.len());
        head[at..at + end].to_owned()
    });
    let words = |s: Option<String>| s.map(|s| plain(&s)).filter(|s| !s.is_empty());
    Preview {
        title: words(og_title).or_else(|| words(title)),
        description: words(og_description).or_else(|| words(description)),
        image: og_image.and_then(|src| absolute(page, &plain(&src))),
    }
}

/// `text` cut to at most `most` characters, at a word, with an ellipsis
/// saying it was cut.
#[must_use]
pub fn cap(text: &str, most: usize) -> String {
    if text.chars().count() <= most {
        return text.to_owned();
    }
    let cut: String = text.chars().take(most.saturating_sub(1)).collect();
    let at_word = cut
        .rfind(' ')
        .filter(|&i| i > cut.len() / 2)
        .unwrap_or(cut.len());
    format!(
        "{}…",
        cut[..at_word].trim_end_matches([' ', ',', ';', ':', '-', '—'])
    )
}

/// The first sentence of `text`, when it has one short enough to be a line.
fn first_sentence(text: &str) -> Option<&str> {
    let end = text
        .char_indices()
        .find(|&(i, c)| {
            matches!(c, '.' | '!' | '?' | '|' | '•')
                && text[i + c.len_utf8()..]
                    .chars()
                    .next()
                    .is_none_or(char::is_whitespace)
        })
        .map_or(text.len(), |(i, c)| {
            if c == '|' || c == '•' {
                i
            } else {
                i + c.len_utf8()
            }
        });
    let line = text[..end].trim();
    (!line.is_empty() && line.chars().count() <= MOST_LINE).then_some(line)
}

/// What a page offers `kit`: the description as the bio, its first
/// sentence as the one line, and its image as a photo. Nothing the kit
/// already says is offered again.
#[must_use]
pub fn suggest(kit: &crate::kit::Kit, preview: &Preview) -> Vec<Suggestion> {
    let mut out = Vec::new();
    if let Some(description) = &preview.description {
        let line = first_sentence(description);
        if let Some(line) = line
            && line != kit.tagline
        {
            out.push(Suggestion {
                field: Field::Line,
                value: line.to_owned(),
            });
        }
        // A description that is one short sentence is a line, not a bio.
        let bio = cap(description, MOST_BIO);
        if bio != kit.bio && Some(bio.as_str()) != line {
            out.push(Suggestion {
                field: Field::Bio,
                value: bio,
            });
        }
    }
    if let Some(image) = &preview.image {
        out.push(Suggestion {
            field: Field::Photo,
            value: image.clone(),
        });
    }
    out
}

/// What one link's answer gives `kit`: `status`, its content type and the
/// first bytes of its body.
#[must_use]
pub fn answer(
    kit: &crate::kit::Kit,
    link: &str,
    status: u16,
    mime: Option<&str>,
    body: &[u8],
) -> Read {
    let site = crate::kit::site(link);
    let nothing = |said: String| Read {
        link: link.to_owned(),
        site,
        title: None,
        suggestions: Vec::new(),
        said: Some(said),
    };
    if !(200..300).contains(&status) {
        return nothing(format!("{site} answered {status}"));
    }
    if mime.is_some_and(|m| {
        let m = m.to_ascii_lowercase();
        !(m.starts_with("text/html") || m.starts_with("application/xhtml"))
    }) {
        return nothing(format!("{link} is not a page"));
    }
    let preview = read(link, &String::from_utf8_lossy(body));
    if preview.is_empty() {
        return nothing(format!(
            "{site} gave no preview — a page behind a login often does not"
        ));
    }
    let suggestions = suggest(kit, &preview);
    let said = suggestions
        .is_empty()
        .then(|| "Nothing the kit does not already say".to_owned());
    Read {
        link: link.to_owned(),
        site,
        title: preview.title,
        suggestions,
        said,
    }
}

// ---------------------------------------------------------------------------
// Fetching
// ---------------------------------------------------------------------------

/// How djmanzo introduces itself to a page it reads.
pub const USER_AGENT: &str = concat!(
    "djmanzo/",
    env!("CARGO_PKG_VERSION"),
    " (a DJ's press kit reading its own links)"
);

/// An answer: its status, its content type and at most the bytes asked for.
#[derive(Debug, Clone)]
pub struct Fetched {
    pub status: u16,
    pub mime: Option<String>,
    pub bytes: Vec<u8>,
    /// Whether there was more than was read.
    pub more: bool,
}

/// Fetch `url`, reading at most `most` bytes of it within `wait`. No cookie
/// is sent and none is kept.
///
/// # Errors
/// An address djmanzo will not fetch, no answer, or none within `wait`.
pub async fn fetch(
    client: &reqwest::Client,
    url: &str,
    most: usize,
    wait: Duration,
) -> Result<Fetched, String> {
    if crate::kit::host(url).is_none() {
        return Err(format!("{url} is not a web page"));
    }
    // reqwest's own timeout runs until the body is finished, so a page that
    // answers and then trickles is given up on too.
    let late = |e: reqwest::Error| {
        if e.is_timeout() {
            format!("no answer from {url} within {} seconds", wait.as_secs_f32())
        } else {
            e.to_string()
        }
    };
    let mut response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .timeout(wait)
        .send()
        .await
        .map_err(late)?;
    let status = response.status().as_u16();
    let mime = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let mut bytes = Vec::new();
    let mut more = false;
    while let Some(chunk) = response.chunk().await.map_err(late)? {
        let room = most - bytes.len();
        if chunk.len() > room {
            bytes.extend_from_slice(&chunk[..room]);
            more = true;
            break;
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(Fetched {
        status,
        mime,
        bytes,
        more,
    })
}

/// Read each of the kit's links once, in the kit's order.
pub async fn read_all(client: &reqwest::Client, kit: &crate::kit::Kit) -> Vec<Read> {
    let mut out: Vec<Read> = Vec::new();
    for link in &kit.links {
        if out.iter().any(|r| &r.link == link) {
            continue;
        }
        let read = match fetch(client, link, MOST_PAGE, WAIT).await {
            Ok(page) => answer(kit, link, page.status, page.mime.as_deref(), &page.bytes),
            Err(e) => Read {
                link: link.clone(),
                site: crate::kit::site(link),
                title: None,
                suggestions: Vec::new(),
                said: Some(e),
            },
        };
        out.push(read);
    }
    out
}

/// A photo taken from a page: its bytes when they are a picture djmanzo can
/// show and keep, and the name to keep it under.
///
/// # Errors
/// An answer that is not success, larger than [`MOST_PHOTO`], or not a PNG,
/// JPEG, GIF or WebP — a picture as an SVG from a stranger can carry a
/// script, and is not taken.
pub fn photo(url: &str, fetched: &Fetched) -> Result<(String, Vec<u8>), String> {
    if !(200..300).contains(&fetched.status) {
        return Err(format!("the photo's page answered {}", fetched.status));
    }
    if fetched.more {
        return Err(format!(
            "the photo is larger than {} MB",
            MOST_PHOTO / 1024 / 1024
        ));
    }
    let ext = match crate::brand::content_type(&fetched.bytes) {
        Some("image/png") => "png",
        Some("image/jpeg") => "jpg",
        Some("image/gif") => "gif",
        Some("image/webp") => "webp",
        _ => return Err("that is not a photo djmanzo can keep".to_owned()),
    };
    let path = url.split(['?', '#']).next().unwrap_or_default();
    let stem = path
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .rsplit_once('.')
        .map_or(path.rsplit('/').next().unwrap_or_default(), |(stem, _)| {
            stem
        });
    let stem: String = stem
        .chars()
        .filter(|c| c.is_alphanumeric() || "-_".contains(*c))
        .take(40)
        .collect();
    let stem = if stem.is_empty() {
        "photo".to_owned()
    } else {
        stem
    };
    Ok((format!("{stem}.{ext}"), fetched.bytes.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::Kit;

    /// A profile page as a music network serves it, saved: entities, an
    /// `og:` set and a body full of other people's tags.
    const PROFILE: &str = r#"<!DOCTYPE html>
<html lang="en"><head>
<meta charset="utf-8">
<title>DJ Rosa | Free Listening on SoundCloud</title>
<meta property="og:title" content="DJ Rosa">
<meta property="og:description" content="Bachata &amp; merengue from Santo Domingo. Resident at La Terraza since 2019; weddings, clubs and the occasional sunrise.">
<meta property='og:image' content='/images/rosa-500x500.jpg'>
<meta name="description" content="Not this one: og:description wins.">
</head><body>
<meta property="og:description" content="An embedded post's, which is not hers.">
<p>Tracks</p></body></html>"#;

    /// **The load-bearing one.** A saved page gives the one line, the bio
    /// and the photo — the head's and nobody else's, its words plain, its
    /// image an address — and each is a suggestion naming its field.
    #[test]
    fn a_saved_page_gives_the_line_the_bio_and_the_photo() {
        let page = "https://soundcloud.com/djrosa";
        let preview = read(page, PROFILE);
        assert_eq!(preview.title.as_deref(), Some("DJ Rosa"));
        assert_eq!(
            preview.description.as_deref(),
            Some(
                "Bachata & merengue from Santo Domingo. Resident at La Terraza since 2019; \
                 weddings, clubs and the occasional sunrise."
            )
        );
        assert_eq!(
            preview.image.as_deref(),
            Some("https://soundcloud.com/images/rosa-500x500.jpg")
        );

        let offered = suggest(&Kit::default(), &preview);
        assert_eq!(
            offered,
            vec![
                Suggestion {
                    field: Field::Line,
                    value: "Bachata & merengue from Santo Domingo.".to_owned()
                },
                Suggestion {
                    field: Field::Bio,
                    value: preview.description.clone().expect("a description")
                },
                Suggestion {
                    field: Field::Photo,
                    value: "https://soundcloud.com/images/rosa-500x500.jpg".to_owned()
                },
            ]
        );

        let read = answer(
            &Kit::default(),
            page,
            200,
            Some("text/html; charset=utf-8"),
            PROFILE.as_bytes(),
        );
        assert_eq!(read.site, "SoundCloud");
        assert_eq!(read.title.as_deref(), Some("DJ Rosa"));
        assert_eq!(read.suggestions, offered);
        assert_eq!(read.said, None);
    }

    /// What the kit already says is not offered again; what it says
    /// otherwise is offered beside it (the interface says it would replace).
    #[test]
    fn what_the_kit_already_says_is_not_offered() {
        let preview = read("https://soundcloud.com/djrosa", PROFILE);
        let kit = Kit {
            tagline: "Bachata & merengue from Santo Domingo.".to_owned(),
            bio: "Something else she wrote.".to_owned(),
            ..Kit::default()
        };
        let fields: Vec<Field> = suggest(&kit, &preview).iter().map(|s| s.field).collect();
        assert_eq!(fields, vec![Field::Bio, Field::Photo]);

        let all = Kit {
            tagline: kit.tagline.clone(),
            bio: preview.description.clone().expect("a description"),
            ..Kit::default()
        };
        let no_image = Preview {
            image: None,
            ..preview
        };
        assert!(suggest(&all, &no_image).is_empty());
        let read = answer(&all, "https://soundcloud.com/djrosa", 200, None, br#"<head><meta property="og:description" content="Bachata &amp; merengue from Santo Domingo. Resident at La Terraza since 2019; weddings, clubs and the occasional sunrise."></head>"#);
        assert_eq!(
            read.said.as_deref(),
            Some("Nothing the kit does not already say")
        );
    }

    /// A page behind a login, a refusal, something that is not a page: each
    /// is named, never skipped silently.
    #[test]
    fn a_page_with_nothing_to_offer_says_why() {
        let wall = "<html><head><script>login()</script></head><body>Log in</body></html>";
        let kit = Kit::default();
        let read = answer(
            &kit,
            "https://www.instagram.com/djrosa",
            200,
            Some("text/html"),
            wall.as_bytes(),
        );
        assert!(read.suggestions.is_empty());
        assert_eq!(
            read.said.as_deref(),
            Some("Instagram gave no preview — a page behind a login often does not")
        );
        let gone = answer(
            &kit,
            "https://mixcloud.com/djrosa",
            404,
            Some("text/html"),
            b"",
        );
        assert_eq!(gone.said.as_deref(), Some("Mixcloud answered 404"));
        let pdf = answer(
            &kit,
            "https://example.com/rider.pdf",
            200,
            Some("application/pdf"),
            PROFILE.as_bytes(),
        );
        assert_eq!(
            pdf.said.as_deref(),
            Some("https://example.com/rider.pdf is not a page")
        );
    }

    /// A stranger's words are plain words: tags out, entities decoded,
    /// white space one space; capped at a word with an ellipsis.
    #[test]
    fn a_strangers_words_are_plain_and_capped() {
        assert_eq!(
            plain("Rosa<script>alert(1)</script> &lt;3 &#233;&#x00F1;\n\t&nbsp;&bogus; &amp;amp;"),
            "Rosa alert(1) <3 éñ &bogus; &amp;"
        );
        assert_eq!(plain("I <3 bachata < merengue"), "I <3 bachata < merengue");
        let long = "word ".repeat(400);
        let capped = cap(long.trim(), MOST_BIO);
        assert!(capped.chars().count() <= MOST_BIO);
        assert!(capped.ends_with("word…"));
        assert_eq!(cap("short", 140), "short");

        let page = format!(
            r#"<head><meta property="og:description" content="{}"></head>"#,
            "Long. ".repeat(10) + &"x".repeat(2000)
        );
        let offered = suggest(&Kit::default(), &read("https://example.com", &page));
        let bio = offered
            .iter()
            .find(|s| s.field == Field::Bio)
            .expect("a bio");
        assert!(bio.value.chars().count() <= MOST_BIO);
        let line = offered
            .iter()
            .find(|s| s.field == Field::Line)
            .expect("a line");
        assert_eq!(line.value, "Long.");
    }

    /// A description with no short first sentence offers no line; a title
    /// alone names the page and offers nothing.
    #[test]
    fn no_line_without_a_short_sentence() {
        let page = format!(
            r#"<head><meta name="description" content="{}"><title>Rosa</title></head>"#,
            "a".repeat(200)
        );
        let preview = read("https://example.com", &page);
        assert_eq!(preview.title.as_deref(), Some("Rosa"));
        let fields: Vec<Field> = suggest(&Kit::default(), &preview)
            .iter()
            .map(|s| s.field)
            .collect();
        assert_eq!(fields, vec![Field::Bio]);

        let title_only = answer(
            &Kit::default(),
            "https://example.com",
            200,
            None,
            b"<title>Rosa &amp; co</title>",
        );
        assert_eq!(title_only.title.as_deref(), Some("Rosa & co"));
        assert_eq!(
            title_only.said.as_deref(),
            Some("Nothing the kit does not already say")
        );
        assert_eq!(
            first_sentence("DJ Rosa | Bachata, merengue"),
            Some("DJ Rosa"),
            "a title's bar ends a line"
        );
    }

    /// Only an address djmanzo would fetch: relative ones read against the
    /// page, `javascript:` and `data:` and a path with no root refused.
    #[test]
    fn an_image_is_an_address_djmanzo_would_fetch() {
        let page = "https://rosa.example.com:8443/about/";
        assert_eq!(
            absolute(page, "/a.jpg").as_deref(),
            Some("https://rosa.example.com:8443/a.jpg")
        );
        assert_eq!(
            absolute(page, "//cdn.example.com/a.jpg").as_deref(),
            Some("https://cdn.example.com/a.jpg")
        );
        assert_eq!(
            absolute(page, "https://cdn.example.com/a.jpg").as_deref(),
            Some("https://cdn.example.com/a.jpg")
        );
        for refused in [
            "javascript:alert(1)",
            "data:image/png;base64,AAAA",
            "a.jpg",
            "file:///etc/passwd",
            "https://user@evil/x",
            // Protocol-relative is read against the page, and then held to
            // the same rule as any other address.
            "//user@evil.example/x",
            "// spaced.example/x",
        ] {
            assert_eq!(absolute(page, refused), None, "{refused}");
        }
        let head = r#"<meta property="og:image" content="javascript:alert(1)"><meta property="og:image" content="/second.jpg">"#;
        assert_eq!(
            read(page, head).image,
            None,
            "the first og:image is the page's; a bad one is not replaced by a later one"
        );
    }

    /// The tag reader: quoted `>` inside a value, unquoted values, `<metadata>`
    /// that is not `<meta>`, and attributes in any order and case.
    #[test]
    fn the_tag_reader_reads_what_pages_write() {
        let head = r#"<metadata name="description" content="no">
<META CONTENT="a > b" NAME=Description>
<meta content=solo property=og:title>"#;
        let preview = read("https://example.com", head);
        assert_eq!(preview.description.as_deref(), Some("a > b"));
        assert_eq!(preview.title.as_deref(), Some("solo"));
        assert_eq!(read("https://example.com", "<meta").description, None);
        // A `<meta>` in the body is an embedded post's, not the page's.
        let embedded = r#"<head><title>Rosa</title></head><body><meta property="og:description" content="Somebody else's"></body>"#;
        assert_eq!(read("https://example.com", embedded).description, None);
        assert_eq!(
            read(
                "https://example.com",
                "<meta name=\"description\" content=\"never closed"
            )
            .description,
            None
        );
    }

    /// A photo is kept only as a picture djmanzo can show, under a plain name.
    #[test]
    fn a_photo_is_a_picture_under_a_plain_name() {
        let jpeg = Fetched {
            status: 200,
            mime: None,
            bytes: vec![0xff, 0xd8, 0xff, 0xe0, 0, 0],
            more: false,
        };
        let (name, bytes) =
            photo("https://cdn.example.com/p/../Rosa%20Live.JPG?w=500", &jpeg).expect("a photo");
        assert_eq!(name, "Rosa20Live.jpg");
        assert_eq!(bytes, jpeg.bytes);
        assert_eq!(
            photo("https://cdn.example.com/", &jpeg).expect("a photo").0,
            "photo.jpg"
        );
        let svg = Fetched {
            bytes: b"<svg onload=\"alert(1)\"></svg>".to_vec(),
            ..jpeg.clone()
        };
        assert!(photo("https://cdn.example.com/a.svg", &svg).is_err());
        let large = Fetched {
            more: true,
            ..jpeg.clone()
        };
        assert_eq!(
            photo("https://x.example.com/a.jpg", &large),
            Err("the photo is larger than 10 MB".to_owned())
        );
        let gone = Fetched {
            status: 404,
            ..jpeg
        };
        assert!(photo("https://x.example.com/a.jpg", &gone).is_err());
    }

    /// A page served from here: the first megabyte is read and no more,
    /// a page that does not answer is given up on, and a preview past the
    /// megabyte is not found.
    #[test]
    fn a_page_is_read_to_a_megabyte_and_no_longer_than_it_is_waited_for() {
        use std::io::{Read as _, Write as _};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
        let port = listener.local_addr().expect("an address").port();
        let server = std::thread::spawn(move || {
            for body in [
                format!("{PROFILE}{}", "x".repeat(3 * MOST_PAGE)),
                format!("<html>{}{PROFILE}", " ".repeat(MOST_PAGE)),
            ] {
                let (mut s, _) = listener.accept().expect("a client");
                let mut ask = [0u8; 1024];
                let _ = s.read(&mut ask);
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                );
                let _ = s.write_all(body.as_bytes());
            }
            // The third is never answered.
            let (s, _) = listener.accept().expect("a client");
            std::thread::sleep(Duration::from_millis(1500));
            drop(s);
        });
        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{port}/rosa");
        tauri::async_runtime::block_on(async {
            let first = fetch(&client, &url, MOST_PAGE, WAIT)
                .await
                .expect("an answer");
            assert_eq!(first.bytes.len(), MOST_PAGE);
            assert!(first.more);
            let kit = Kit::default();
            let read = answer(
                &kit,
                &url,
                first.status,
                first.mime.as_deref(),
                &first.bytes,
            );
            assert_eq!(read.title.as_deref(), Some("DJ Rosa"));

            let second = fetch(&client, &url, MOST_PAGE, WAIT)
                .await
                .expect("an answer");
            let read = answer(
                &kit,
                &url,
                second.status,
                second.mime.as_deref(),
                &second.bytes,
            );
            assert!(
                read.suggestions.is_empty(),
                "the preview was past the megabyte"
            );

            let third = fetch(&client, &url, MOST_PAGE, Duration::from_millis(300)).await;
            assert!(third.expect_err("no answer").contains("within"));
        });
        server.join().expect("the server");
        let refused =
            tauri::async_runtime::block_on(fetch(&client, "file:///etc/passwd", MOST_PAGE, WAIT));
        assert!(refused.is_err());
    }
}
