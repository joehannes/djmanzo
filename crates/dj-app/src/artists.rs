//! §122 step 5: pictures of the artist playing, behind the words and on the
//! projector, each freely licensed and credited on the screen while it shows.
//! See `docs/VISUALS.md`.
//!
//! > Where may artist images come from, for a screen at a paid event? →
//! > Wikimedia Commons plus TheAudioDB, licensed images only, each TheAudioDB
//! > image used only when its record says Creative Commons, with the credit on
//! > screen. (`docs/DIRECTIVE.md`, decided 27 September)
//!
//! **Wikimedia Commons**, found through Wikidata: the artist's item — by the
//! MusicBrainz artist id the record was tagged with, else by a name that is
//! one of the item's own names, among items that are MusicBrainz artists at
//! all — gives its main picture and its Commons category. The category's
//! photographs whose file names name the artist come after the main picture.
//! A file is taken only under CC BY, CC BY-SA or CC0, as a JPEG of a size a
//! projector needs, and with an author to credit.
//!
//! Commons' "public domain" files are left out: many are public domain only
//! in the United States, and the screen may be anywhere. So are files under
//! a non-commercial or no-derivatives licence (Commons does not host them, but
//! the check does not rely on that), the GFDL and the Free Art License.
//!
//! **TheAudioDB** is not asked. The tag the decision rests on,
//! `strCreativeCommons`, which its terms say to check, is in none of its
//! artist records (looked for on 4 October across search and lookup), so no
//! TheAudioDB picture can meet the decision; its free key is also for
//! development only. `docs/VISUALS.md` says what would change that.
//!
//! **Asked once per artist and kept**, as K1 keeps covers: the pictures and
//! their credits in `artists/<key>/`, or that there are none. An answer that
//! did not come — no network, Wikimedia asking to wait (429) — is not kept,
//! so it is asked again later. The asking itself is [`ask`], given the
//! means to fetch, so the whole of it is tested with recorded answers.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Who is asking, as Wikimedia's policy wants: the program and where to reach
/// its makers. Requests without one are limited to ten a minute.
pub const USER_AGENT: &str = concat!(
    "djmanzo/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/joehannes/djmanzo) reqwest"
);

/// The most pictures kept of one artist.
pub const MOST: usize = 6;

/// The narrowest original taken: a projector fills a wall with it.
pub const NARROWEST: u32 = 800;

/// The width asked of Commons, one of the sizes it keeps ready.
pub const WIDTH: u32 = 1280;

/// The largest picture taken; a 1280-wide JPEG is a fraction of this.
pub const MOST_BYTES: usize = 4 * 1024 * 1024;

const WIKIDATA: &str = "https://www.wikidata.org/w/api.php";
const COMMONS: &str = "https://commons.wikimedia.org/w/api.php";

/// Who the pictures are of: the record's artist, and the MusicBrainz artist
/// it was tagged with, if it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artist {
    pub name: String,
    pub id: Option<String>,
}

impl Artist {
    /// The folder the answer is kept in: the MusicBrainz id (already a UUID,
    /// `dj_library::tags::artist_id`), else a hash of the name as written
    /// any which way — never the name itself, which could name a path.
    #[must_use]
    pub fn key(&self) -> String {
        match &self.id {
            Some(id) => id.clone(),
            None => format!("name-{:016x}", fnv(&same(&self.name))),
        }
    }
}

/// One answer from a server: its status, its content type and its body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    pub mime: Option<String>,
    pub bytes: Vec<u8>,
}

/// One picture as kept, and as the interface credits it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    /// Who made it, as Commons credits them.
    pub author: String,
    /// The licence's short name, e.g. `CC BY-SA 4.0`.
    pub licence: String,
    /// The file's page on Commons, where its whole credit is.
    pub page: String,
    /// The line drawn in a corner of the screen while it shows.
    pub credit: String,
}

/// What was found for an artist: the pictures, or none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Found {
    pub artist: String,
    /// The Wikidata item they were found as, if they were.
    pub entity: Option<String>,
    pub pictures: Vec<Picture>,
}

/// A file Commons offered, before it is chosen.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub title: String,
    /// The picture at [`WIDTH`], or the original where it is narrower.
    pub url: String,
    pub mime: String,
    /// The original's width, which says whether it is big enough.
    pub original: u32,
    pub width: u32,
    pub height: u32,
    pub licence: Option<String>,
    pub author: String,
    pub page: String,
}

/// The licence a file may be shown under at a paid event, by its short name,
/// or none. Commons gives a code (`cc-by-sa-4.0`) and a short name (`CC BY-SA
/// 4.0`); the code is read first, the name if there is no code.
///
/// CC BY and CC BY-SA, any version or port, and CC0. Not public domain, which
/// on Commons is often the United States' alone; not anything non-commercial
/// or without derivatives; not the GFDL, the Free Art License, or a licence
/// unsaid.
#[must_use]
pub fn usable_licence(code: Option<&str>, short: Option<&str>) -> Option<String> {
    let written = code
        .filter(|c| !c.trim().is_empty())
        .or(short)?
        .split(',')
        .next()?
        .trim()
        .to_lowercase()
        .replace([' ', '_'], "-");
    let parts: Vec<&str> = written.split('-').filter(|p| !p.is_empty()).collect();
    let zero = matches!(
        parts.as_slice(),
        ["cc0", ..] | ["cc", "zero", ..] | ["cc", "0", ..]
    );
    let by = matches!(parts.as_slice(), ["cc", "by", ..])
        && parts[2..].iter().all(|p| {
            *p == "sa"
                || p.chars().all(|c| c.is_ascii_digit() || c == '.')
                || (p.len() == 2 && p.chars().all(|c| c.is_ascii_alphabetic()))
        });
    if !(zero || by) || parts.iter().any(|p| matches!(*p, "nc" | "nd")) {
        return None;
    }
    let name = short.map(str::trim).filter(|s| !s.is_empty()).map_or_else(
        || {
            if zero {
                "CC0".to_owned()
            } else {
                let version = parts[2..]
                    .iter()
                    .filter(|p| *p != &"sa")
                    .copied()
                    .collect::<Vec<_>>()
                    .join(" ");
                let sa = if parts.contains(&"sa") { "-SA" } else { "" };
                format!("CC BY{sa} {version}").trim().to_owned()
            }
        },
        str::to_owned,
    );
    Some(name)
}

/// Commons' HTML credit as plain words: tags dropped, entities read, spaces
/// closed up, and no longer than a corner of a screen holds.
#[must_use]
pub fn plain(html: &str) -> String {
    let mut text = String::new();
    let mut inside = false;
    for c in html.chars() {
        match c {
            '<' => inside = true,
            '>' if inside => {
                inside = false;
                text.push(' ');
            }
            _ if !inside => text.push(c),
            _ => {}
        }
    }
    let text = entities(&text);
    let words = text.split_whitespace().collect::<Vec<_>>().join(" ");
    const LONGEST: usize = 80;
    if words.chars().count() > LONGEST {
        let cut: String = words.chars().take(LONGEST - 1).collect();
        format!("{}…", cut.trim_end())
    } else {
        words
    }
}

fn entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest[..rest.len().min(12)].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..end];
        let read = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ => name
                .strip_prefix("#x")
                .or_else(|| name.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| name.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match read {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// A name as compared: lower case, spaces closed up.
fn same(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Letters and digits only, lower case.
fn squeezed(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Whether a file's name names the artist — how a photograph in the artist's
/// category is told from the album covers, logos and stray uploads there.
/// A leading "The" may be left off the file's name.
#[must_use]
pub fn names_them(title: &str, artist: &str) -> bool {
    let title = squeezed(title.strip_prefix("File:").unwrap_or(title));
    let lower = same(artist);
    let name = squeezed(lower.strip_prefix("the ").unwrap_or(&lower));
    name.chars().count() >= 2 && title.contains(&name)
}

/// The record's lead artist, for a second search when the whole credit finds
/// no one: "A feat. B", "A & B", "A x B", "A, B" are A.
#[must_use]
pub fn lead(name: &str) -> &str {
    const JOINS: [&str; 9] = [
        " feat. ",
        " feat ",
        " ft. ",
        " featuring ",
        " & ",
        " x ",
        ", ",
        " vs. ",
        " vs ",
    ];
    // ASCII lower case keeps every byte where it was, so an offset found in
    // it is one in the name.
    let lower = name.to_ascii_lowercase();
    let cut = JOINS
        .iter()
        .filter_map(|join| lower.find(join))
        .min()
        .unwrap_or(name.len());
    name[..cut].trim()
}

fn url(base: &str, params: &[(&str, &str)]) -> Result<String, String> {
    reqwest::Url::parse_with_params(base, params)
        .map(String::from)
        .map_err(|e| e.to_string())
}

/// Wikidata's search for the artist, among items that are MusicBrainz
/// artists: by their id, exactly, or by name.
///
/// # Errors
/// Only if the address cannot be written, which a constant base rules out.
pub fn search_url(artist: &Artist, name: &str) -> Result<String, String> {
    let query = match &artist.id {
        Some(id) => format!("haswbstatement:P434={id}"),
        None => format!("{name} haswbstatement:P434"),
    };
    url(
        WIKIDATA,
        &[
            ("action", "query"),
            ("list", "search"),
            ("srsearch", &query),
            ("srlimit", "5"),
            ("srprop", ""),
            ("format", "json"),
            ("formatversion", "2"),
        ],
    )
}

/// The items a search found, best first.
#[must_use]
pub fn search_hits(json: &Value) -> Vec<String> {
    json["query"]["search"]
        .as_array()
        .map(|hits| {
            hits.iter()
                .filter_map(|hit| hit["title"].as_str())
                .filter(|title| title.starts_with('Q'))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Every name of each item — labels and aliases, in every language.
///
/// # Errors
/// As [`search_url`].
pub fn names_url(items: &[String]) -> Result<String, String> {
    url(
        WIKIDATA,
        &[
            ("action", "wbgetentities"),
            ("ids", &items.join("|")),
            ("props", "labels|aliases"),
            ("format", "json"),
            ("formatversion", "2"),
        ],
    )
}

/// The first of `items` (in the search's order) one of whose own names is
/// `name`: a search for "Adele" also finds singers merely called Adele
/// somewhere in their item, and only the one called Adele is her.
#[must_use]
pub fn pick(json: &Value, items: &[String], name: &str) -> Option<String> {
    let wanted = same(name);
    items
        .iter()
        .find(|item| {
            let entity = &json["entities"][item.as_str()];
            let labels = entity["labels"]
                .as_object()
                .into_iter()
                .flat_map(|labels| labels.values());
            let aliases = entity["aliases"]
                .as_object()
                .into_iter()
                .flat_map(|aliases| aliases.values())
                .filter_map(Value::as_array)
                .flatten();
            labels
                .chain(aliases)
                .filter_map(|named| named["value"].as_str())
                .any(|named| same(named) == wanted)
        })
        .cloned()
}

/// The item's statements, for its picture and its Commons category.
///
/// # Errors
/// As [`search_url`].
pub fn claims_url(item: &str) -> Result<String, String> {
    url(
        WIKIDATA,
        &[
            ("action", "wbgetentities"),
            ("ids", item),
            ("props", "claims"),
            ("format", "json"),
            ("formatversion", "2"),
        ],
    )
}

/// The item's pictures (P18), deprecated ones left out, and its Commons
/// category (P373).
#[must_use]
pub fn claims(json: &Value, item: &str) -> (Vec<String>, Option<String>) {
    let claims = &json["entities"][item]["claims"];
    let values = |property: &str| -> Vec<String> {
        claims[property]
            .as_array()
            .map(|said| {
                said.iter()
                    .filter(|claim| claim["rank"].as_str() != Some("deprecated"))
                    .filter_map(|claim| claim["mainsnak"]["datavalue"]["value"].as_str())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    };
    (values("P18"), values("P373").into_iter().next())
}

const METADATA: &str = "License|LicenseShortName|Artist";

/// Commons' description of the named files, at [`WIDTH`], with each one's
/// licence and author.
///
/// # Errors
/// As [`search_url`].
pub fn files_url(files: &[String]) -> Result<String, String> {
    let titles = files
        .iter()
        .map(|file| format!("File:{file}"))
        .collect::<Vec<_>>()
        .join("|");
    let width = WIDTH.to_string();
    url(
        COMMONS,
        &[
            ("action", "query"),
            ("titles", &titles),
            ("prop", "imageinfo"),
            ("iiprop", "url|mime|size|extmetadata"),
            ("iiurlwidth", &width),
            ("iiextmetadatafilter", METADATA),
            ("format", "json"),
            ("formatversion", "2"),
        ],
    )
}

/// The same for the files in a Commons category.
///
/// # Errors
/// As [`search_url`].
pub fn category_url(category: &str) -> Result<String, String> {
    let title = format!("Category:{category}");
    let width = WIDTH.to_string();
    url(
        COMMONS,
        &[
            ("action", "query"),
            ("generator", "categorymembers"),
            ("gcmtitle", &title),
            ("gcmtype", "file"),
            ("gcmlimit", "50"),
            ("prop", "imageinfo"),
            ("iiprop", "url|mime|size|extmetadata"),
            ("iiurlwidth", &width),
            ("iiextmetadatafilter", METADATA),
            ("format", "json"),
            ("formatversion", "2"),
        ],
    )
}

/// The files Commons described, in its order.
#[must_use]
pub fn candidates(json: &Value) -> Vec<Candidate> {
    let Some(pages) = json["query"]["pages"].as_array() else {
        return Vec::new();
    };
    pages
        .iter()
        .filter_map(|page| {
            let info = &page["imageinfo"][0];
            let meta = &info["extmetadata"];
            let said = |key: &str| meta[key]["value"].as_str();
            let number = |key: &str| {
                info[key]
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .unwrap_or(0)
            };
            let original = number("width");
            let (url, width, height) = match info["thumburl"].as_str() {
                Some(thumb) => (thumb, number("thumbwidth"), number("thumbheight")),
                None => (info["url"].as_str()?, original, number("height")),
            };
            Some(Candidate {
                title: page["title"].as_str()?.to_owned(),
                url: url.to_owned(),
                mime: info["mime"].as_str().unwrap_or("").to_owned(),
                original,
                width,
                height,
                licence: usable_licence(said("License"), said("LicenseShortName")),
                author: said("Artist").map(plain).unwrap_or_default(),
                page: info["descriptionurl"].as_str().unwrap_or("").to_owned(),
            })
        })
        .collect()
}

/// Whether a file may be shown: under a usable licence, with someone to
/// credit, a JPEG — a photograph, where logos and drawings are PNG and SVG —
/// and wide enough for a projector.
#[must_use]
pub fn showable(candidate: &Candidate) -> bool {
    candidate.licence.is_some()
        && !candidate.author.is_empty()
        && candidate.mime == "image/jpeg"
        && candidate.original >= NARROWEST
        && candidate.width > 0
        && candidate.height > 0
        && candidate.url.starts_with("https://")
}

/// The pictures to keep: the item's own first, then the category's that name
/// the artist, each once, every one showable, at most [`MOST`].
#[must_use]
pub fn choose(own: Vec<Candidate>, category: Vec<Candidate>, artist: &str) -> Vec<Candidate> {
    let mut chosen: Vec<Candidate> = Vec::new();
    let named = category
        .into_iter()
        .filter(|c| names_them(&c.title, artist));
    for candidate in own.into_iter().chain(named) {
        if chosen.len() == MOST {
            break;
        }
        if showable(&candidate) && !chosen.iter().any(|c| c.title == candidate.title) {
            chosen.push(candidate);
        }
    }
    chosen
}

/// The line drawn in the corner while a picture shows.
#[must_use]
pub fn credit(candidate: &Candidate) -> String {
    format!(
        "Photo: {} · {} · Wikimedia Commons",
        candidate.author,
        candidate.licence.as_deref().unwrap_or("")
    )
}

/// Fetch `url` as Wikimedia asks to be asked: with [`USER_AGENT`], one
/// address at a time, and when told to wait (429 or 503 with `Retry-After`),
/// waiting as long as it says — up to half a minute, twice — before asking
/// again. Told to wait longer, the answer is the refusal.
///
/// # Errors
/// No answer: no network, or none within twenty seconds.
pub async fn fetch(client: &reqwest::Client, url: String) -> Result<Reply, String> {
    let mut waited = 0;
    loop {
        let response = client
            .get(&url)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .timeout(Duration::from_secs(20))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let header = |name| {
            response
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        };
        if matches!(status, 429 | 503) && waited < 2 {
            let seconds = header(reqwest::header::RETRY_AFTER)
                .and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(5);
            if seconds <= 30 {
                waited += 1;
                let pause = Duration::from_secs(seconds);
                tauri::async_runtime::spawn_blocking(move || std::thread::sleep(pause))
                    .await
                    .map_err(|e| e.to_string())?;
                continue;
            }
        }
        let mime = header(reqwest::header::CONTENT_TYPE);
        let bytes = response.bytes().await.map_err(|e| e.to_string())?;
        return Ok(Reply {
            status,
            mime,
            bytes: bytes.to_vec(),
        });
    }
}

/// What an answer from Wikimedia is: its JSON, or why not — a status that is
/// not success (429 is Wikimedia asking to wait), or a body that is not JSON.
fn json(reply: Reply) -> Result<Value, String> {
    if !(200..300).contains(&reply.status) {
        return Err(format!("Wikimedia answered {}", reply.status));
    }
    serde_json::from_slice(&reply.bytes).map_err(|e| format!("Wikimedia's answer: {e}"))
}

/// What a picture's download is: the JPEG, none (gone: 404, or not a JPEG of
/// a sane size — that file is skipped), or an error, which stops the asking
/// so it is tried again later.
///
/// # Errors
/// Any status but success or 404.
pub fn image(reply: Reply) -> Result<Option<Vec<u8>>, String> {
    if reply.status == 404 {
        return Ok(None);
    }
    if !(200..300).contains(&reply.status) {
        return Err(format!("Wikimedia answered {}", reply.status));
    }
    let jpeg = reply
        .mime
        .as_deref()
        .map(|m| m.split(';').next().unwrap_or("").trim())
        == Some("image/jpeg");
    let sane = !reply.bytes.is_empty() && reply.bytes.len() <= MOST_BYTES;
    Ok((jpeg && sane && reply.bytes.starts_with(&[0xFF, 0xD8])).then_some(reply.bytes))
}

/// Ask Wikidata and Commons for `artist`'s pictures, through `get` — which
/// fetches one address with [`USER_AGENT`], or in a test answers from a
/// recording. The pictures come back with their bytes, in order.
///
/// One request at a time, as Wikimedia asks: two searches at most, the
/// item's names and statements, two descriptions, and the pictures.
///
/// # Errors
/// Any request that failed — no network, Wikimedia asking to wait — so that
/// nothing is kept and the artist is asked for again later. Finding no one,
/// or no picture that may be shown, is not an error: it is kept.
pub async fn ask<F, Fut>(artist: &Artist, get: F) -> Result<(Found, Vec<Vec<u8>>), String>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<Reply, String>>,
{
    let none = |entity: Option<String>| {
        Ok((
            Found {
                artist: artist.name.clone(),
                entity,
                pictures: Vec::new(),
            },
            Vec::new(),
        ))
    };
    let mut entity = None;
    // The name the category's files are checked against: the one the item
    // was found by, or by an id, the lead artist's — a credit with its
    // guests names no photograph.
    let mut called = lead(&artist.name);
    let mut names = vec![artist.name.trim()];
    if artist.id.is_none() && lead(&artist.name) != artist.name.trim() {
        names.push(lead(&artist.name));
    }
    for name in names {
        let hits = search_hits(&json(get(search_url(artist, name)?).await?)?);
        entity = if artist.id.is_some() {
            hits.into_iter().next()
        } else if hits.is_empty() {
            None
        } else {
            pick(&json(get(names_url(&hits)?).await?)?, &hits, name)
        };
        if entity.is_some() && artist.id.is_none() {
            called = name;
        }
        if entity.is_some() || artist.id.is_some() {
            break;
        }
    }
    let Some(item) = entity else {
        return none(None);
    };
    let (own, category) = claims(&json(get(claims_url(&item)?).await?)?, &item);
    let own = if own.is_empty() {
        Vec::new()
    } else {
        candidates(&json(get(files_url(&own)?).await?)?)
    };
    let category = match category {
        Some(category) => candidates(&json(get(category_url(&category)?).await?)?),
        None => Vec::new(),
    };
    let mut pictures = Vec::new();
    let mut bytes = Vec::new();
    for chosen in choose(own, category, called) {
        if let Some(jpeg) = image(get(chosen.url.clone()).await?)? {
            pictures.push(Picture {
                width: chosen.width,
                height: chosen.height,
                author: chosen.author.clone(),
                licence: chosen.licence.clone().unwrap_or_default(),
                page: chosen.page.clone(),
                credit: credit(&chosen),
            });
            bytes.push(jpeg);
        }
    }
    if pictures.is_empty() {
        return none(Some(item));
    }
    Ok((
        Found {
            artist: artist.name.clone(),
            entity: Some(item),
            pictures,
        },
        bytes,
    ))
}

/// The artist's pictures: as kept, or asked for through `get` and kept.
///
/// # Errors
/// As [`ask`], or the answer could not be kept.
pub async fn find<F, Fut>(dir: &Path, artist: &Artist, get: F) -> Result<Found, String>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<Reply, String>>,
{
    let key = artist.key();
    if let Some(found) = kept(dir, &key) {
        return Ok(found);
    }
    let (found, pictures) = ask(artist, get).await?;
    keep(dir, &key, &found, &pictures)?;
    Ok(found)
}

/// What the interface is told of a deck's artist.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Shown {
    pub artist: Option<String>,
    /// What [`picture`] reads their pictures by.
    pub key: Option<String>,
    /// `found`; `none` (asked: nothing that may be shown); `asking`;
    /// `waiting` (an answer did not come, and is asked for again after
    /// [`WAIT`]); `unknown` (the record names no artist).
    pub state: &'static str,
    pub pictures: Vec<Picture>,
    /// Why an answer did not come.
    pub why: Option<String>,
}

impl Shown {
    #[must_use]
    pub fn of(artist: Option<&Artist>, state: &'static str) -> Self {
        Self {
            artist: artist.map(|a| a.name.clone()),
            key: artist.map(Artist::key),
            state,
            pictures: Vec::new(),
            why: None,
        }
    }

    /// What was kept, told as found or none.
    #[must_use]
    pub fn kept(artist: &Artist, found: Found) -> Self {
        let state = if found.pictures.is_empty() {
            "none"
        } else {
            "found"
        };
        Self {
            pictures: found.pictures,
            ..Self::of(Some(artist), state)
        }
    }
}

/// How long after an answer did not come it is asked for again: long enough
/// that a night without a network, or Wikimedia asking to wait, is not asked
/// every few seconds by every screen.
pub const WAIT: Duration = Duration::from_secs(10 * 60);

#[derive(Debug)]
enum Ask {
    Now,
    Failed(Instant, String),
}

/// Who is being asked for, and whose answer did not come lately — so two
/// screens following the same deck ask once between them.
#[derive(Debug, Default)]
pub struct Asking {
    asks: Mutex<HashMap<String, Ask>>,
}

impl Asking {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Begin asking for `artist`, or what to tell the interface instead:
    /// that it is being asked for, or is waited on after a failure.
    ///
    /// # Errors
    /// The [`Shown`] to answer with rather than asking.
    pub fn begin(&self, artist: &Artist, now: Instant) -> Result<(), Shown> {
        let key = artist.key();
        let mut asks = self
            .asks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match asks.get(&key) {
            Some(Ask::Now) => return Err(Shown::of(Some(artist), "asking")),
            Some(Ask::Failed(when, why)) if now.duration_since(*when) < WAIT => {
                return Err(Shown {
                    why: Some(why.clone()),
                    ..Shown::of(Some(artist), "waiting")
                });
            }
            _ => {}
        }
        asks.insert(key, Ask::Now);
        Ok(())
    }

    /// The asking for `artist` is over: kept, or failed for `why`.
    pub fn end(&self, artist: &Artist, outcome: Result<(), String>, now: Instant) {
        let mut asks = self
            .asks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match outcome {
            Ok(()) => asks.remove(&artist.key()),
            Err(why) => asks.insert(artist.key(), Ask::Failed(now, why)),
        };
    }
}

const FOUND: &str = "found.json";

/// The answer kept for `key` in `dir`, if it was asked: a [`Found`] with no
/// pictures is an artist asked for who has none.
#[must_use]
pub fn kept(dir: &Path, key: &str) -> Option<Found> {
    let text = std::fs::read_to_string(dir.join(key).join(FOUND)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Keep the answer for `key` in `dir`: each picture as `<n>.jpg`, then what
/// was found — last, so a folder whose writing was cut off is asked again.
///
/// # Errors
/// The file system's own sentence.
pub fn keep(dir: &Path, key: &str, found: &Found, pictures: &[Vec<u8>]) -> Result<PathBuf, String> {
    let folder = dir.join(key);
    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    for (n, bytes) in pictures.iter().enumerate() {
        std::fs::write(folder.join(format!("{n}.jpg")), bytes).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(found).map_err(|e| e.to_string())?;
    let path = folder.join(FOUND);
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(path)
}

/// The `n`th kept picture of `key`, as JPEG bytes. `key` must be one
/// [`Artist::key`] makes and `n` one [`kept`] lists, so no name the interface
/// sends reaches outside `dir`.
#[must_use]
pub fn picture(dir: &Path, key: &str, n: usize) -> Option<Vec<u8>> {
    let made = key.strip_prefix("name-").map_or_else(
        || is_uuid(key),
        |hash| hash.len() == 16 && hash.chars().all(|c| c.is_ascii_hexdigit()),
    );
    if !made || n >= kept(dir, key)?.pictures.len() {
        return None;
    }
    std::fs::read(dir.join(key).join(format!("{n}.jpg"))).ok()
}

fn is_uuid(text: &str) -> bool {
    let parts: Vec<&str> = text.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(part, len)| part.len() == len && part.chars().all(|c| c.is_ascii_hexdigit()))
}

/// FNV-1a, 64 bits: a name's folder, the same on every run and machine.
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::RefCell;
    use std::collections::HashMap;

    const DAFT_PUNK: &str = "056e4f3e-d505-4dad-8ec1-d04f521cbb56";

    /// A file as Commons describes it — the shape of its real answers (4
    /// October), down to the thumbnail claiming 1280 wide for an original
    /// narrower than that.
    fn page(
        title: &str,
        mime: &str,
        width: u32,
        height: u32,
        licence: (&str, &str),
        artist: &str,
    ) -> Value {
        let file = title.trim_start_matches("File:").replace(' ', "_");
        json!({
            "ns": 6,
            "title": title,
            "imageinfo": [{
                "width": width,
                "height": height,
                "thumburl": format!("https://upload.wikimedia.org/wikipedia/commons/thumb/x/xx/{file}/1280px-{file}"),
                "thumbwidth": 1280,
                "thumbheight": 1280 * height / width,
                "url": format!("https://upload.wikimedia.org/wikipedia/commons/x/xx/{file}"),
                "descriptionurl": format!("https://commons.wikimedia.org/wiki/File:{file}"),
                "extmetadata": {
                    "Artist": { "value": artist, "source": "commons-desc-page" },
                    "LicenseShortName": { "value": licence.1, "source": "commons-desc-page" },
                    "License": { "value": licence.0, "source": "commons-templates" },
                },
                "mime": mime,
            }],
        })
    }

    fn statement(value: &str, rank: &str) -> Value {
        json!({
            "mainsnak": { "snaktype": "value", "datavalue": { "value": value, "type": "string" } },
            "type": "statement",
            "rank": rank,
        })
    }

    fn found(items: &[&str]) -> Value {
        json!({ "query": { "search": items.iter().map(|q| json!({ "ns": 0, "title": q })).collect::<Vec<_>>() } })
    }

    fn reply(json: &Value) -> Reply {
        Reply {
            status: 200,
            mime: Some("application/json; charset=utf-8".to_owned()),
            bytes: json.to_string().into_bytes(),
        }
    }

    fn jpeg(tag: u8) -> Reply {
        Reply {
            status: 200,
            mime: Some("image/jpeg".to_owned()),
            bytes: vec![0xFF, 0xD8, 0xFF, 0xE0, tag],
        }
    }

    /// The Daft Punk category as Commons answered it, cut down to a file of
    /// each kind that must be told apart.
    fn category() -> Value {
        let by = ("cc-by-2.0", "CC BY 2.0");
        json!({ "query": { "pages": [
            // Too small, and not named for them.
            page("File:Hani.jpg", "image/jpeg", 429, 427, ("cc-by-sa-2.5", "CC BY-SA 2.5"), "AVF"),
            page("File:Daft punk at coachella.jpg", "image/jpeg", 1024, 768, by,
                 "<a rel=\"nofollow\" class=\"external text\" href=\"https://www.flickr.com/people/11868335@N00\">Marcelo Teson</a> from Los Angeles, CA, USA"),
            // Public domain, here the United States' alone.
            page("File:Daft Punk @ Oxegen 07.jpg", "image/jpeg", 1632, 1224, ("pd", "Public domain"), "The original uploader"),
            // An icon and a single's cover: not photographs.
            page("File:Toicon-icon-fandom-disguise.svg", "image/svg+xml", 64, 64, ("cc-by-4.0", "CC BY 4.0"), "Carol Liao"),
            page("File:Daft Punk Instant Crush.png", "image/png", 640, 640, ("pd", "Public domain"), "Sony Music"),
            page("File:DaftPunk NeverEverLand Sydney 2007.JPG", "image/jpeg", 3191, 2393, ("cc-by-sa-3.0", "CC BY-SA 3.0"), "Bonsaichop"),
            // The item's own picture, again.
            page("File:Daft Punk in 2013.jpg", "image/jpeg", 3332, 2500, ("cc-by-4.0", "CC BY 4.0"), "Sony Music Entertainment"),
            // Someone's artwork filed under them: not named for them.
            page("File:Oeuvre Rest In Legend.jpg", "image/jpeg", 1200, 1600, ("cc-by-sa-4.0", "CC BY-SA 4.0"), "Bertrand"),
            // Wide enough and free, but no one to credit.
            page("File:Daft Punk 2008.jpg", "image/jpeg", 2000, 1500, ("cc-by-2.0", "CC BY 2.0"), ""),
        ] } })
    }

    /// Wikimedia as recorded: each address the asking may make, and its answer.
    fn wikimedia(artist: &Artist) -> HashMap<String, Reply> {
        let mut answers = HashMap::new();
        answers.insert(
            search_url(artist, &artist.name).unwrap(),
            reply(&found(&["Q185828"])),
        );
        answers.insert(
            claims_url("Q185828").unwrap(),
            reply(
                &json!({ "entities": { "Q185828": { "id": "Q185828", "claims": {
                "P18": [
                    statement("Daft Punk in 2013.jpg", "normal"),
                    // Marked wrong on Wikidata: never asked for.
                    statement("Thomas Bangalter 1999.jpg", "deprecated"),
                ],
                "P373": [statement("Daft Punk", "normal")],
            } } } }),
            ),
        );
        answers.insert(
            files_url(&["Daft Punk in 2013.jpg".to_owned()]).unwrap(),
            reply(&json!({ "query": { "pages": [
                page("File:Daft Punk in 2013.jpg", "image/jpeg", 3332, 2500, ("cc-by-4.0", "CC BY 4.0"), "Sony Music Entertainment"),
            ] } })),
        );
        answers.insert(category_url("Daft Punk").unwrap(), reply(&category()));
        for (n, candidate) in candidates(&category()).iter().enumerate() {
            answers.insert(candidate.url.clone(), jpeg(n as u8));
        }
        answers
    }

    /// A `get` answering from `answers`, writing down every address asked.
    fn recorded<'a>(
        answers: &'a RefCell<HashMap<String, Reply>>,
        asked: &'a RefCell<Vec<String>>,
    ) -> impl Fn(String) -> std::future::Ready<Result<Reply, String>> + 'a {
        move |url: String| {
            asked.borrow_mut().push(url.clone());
            std::future::ready(
                answers
                    .borrow()
                    .get(&url)
                    .cloned()
                    .ok_or_else(|| format!("nothing recorded for {url}")),
            )
        }
    }

    #[test]
    fn only_creative_commons_a_paid_event_may_show_is_taken() {
        let ok = |code: Option<&str>, short: Option<&str>| usable_licence(code, short);
        assert_eq!(
            ok(Some("cc-by-sa-4.0"), Some("CC BY-SA 4.0")).as_deref(),
            Some("CC BY-SA 4.0")
        );
        assert_eq!(ok(Some("cc-by-2.0"), None).as_deref(), Some("CC BY 2.0"));
        assert_eq!(
            ok(Some("cc-by-sa-3.0-de"), None).as_deref(),
            Some("CC BY-SA 3.0 de")
        );
        assert_eq!(
            ok(Some("cc-by-sa-3.0,2.5,2.0,1.0"), None).as_deref(),
            Some("CC BY-SA 3.0")
        );
        assert_eq!(ok(Some("cc0"), Some("CC0")).as_deref(), Some("CC0"));
        assert_eq!(ok(Some("cc-zero"), None).as_deref(), Some("CC0"));
        assert_eq!(
            ok(None, Some("CC BY-SA 3.0")).as_deref(),
            Some("CC BY-SA 3.0")
        );
        for refused in [
            "cc-by-nc-2.0",
            "cc-by-nd-4.0",
            "cc-by-nc-sa-4.0",
            "cc-by-nc-nd-3.0",
            "pd",
            "pd-old-70",
            "gfdl",
            "fal",
            "attribution",
            "cc-sa-1.0",
            "",
        ] {
            assert_eq!(ok(Some(refused), None), None, "{refused}");
        }
        assert_eq!(ok(None, Some("Public domain")), None);
        assert_eq!(ok(None, Some("CC BY-NC 2.0")), None);
        assert_eq!(ok(None, None), None);
    }

    #[test]
    fn a_credit_is_plain_words_a_corner_holds() {
        assert_eq!(
            plain(
                "<a rel=\"nofollow\" class=\"external text\" href=\"https://www.flickr.com/people/1\">Marcelo Teson</a> from Los Angeles, CA, USA"
            ),
            "Marcelo Teson from Los Angeles, CA, USA"
        );
        assert_eq!(
            plain("Ren&#233;e &amp; <b>Jo</b>&nbsp;&#x4E;"),
            "Renée & Jo N"
        );
        assert_eq!(plain("AT&T & co;"), "AT&T & co;");
        let long = plain(&"word ".repeat(40));
        assert!(long.chars().count() <= 80 && long.ends_with('…'), "{long}");
        let candidate = &candidates(&category())[1];
        assert_eq!(
            credit(candidate),
            "Photo: Marcelo Teson from Los Angeles, CA, USA · CC BY 2.0 · Wikimedia Commons"
        );
    }

    /// **Only photographs of them, freely licensed and creditable, are
    /// shown**: from a real category's mix of concert photographs, covers,
    /// icons, artwork, public-domain uploads and strays, the item's own
    /// picture comes first and then the photographs named for them, each
    /// once.
    #[test]
    fn photographs_are_told_from_covers_logos_and_strays() {
        let own = candidates(&json!({ "query": { "pages": [
            page("File:Daft Punk in 2013.jpg", "image/jpeg", 3332, 2500, ("cc-by-4.0", "CC BY 4.0"), "Sony Music Entertainment"),
        ] } }));
        let chosen = choose(own, candidates(&category()), "Daft Punk");
        let titles: Vec<&str> = chosen.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "File:Daft Punk in 2013.jpg",
                "File:Daft punk at coachella.jpg",
                "File:DaftPunk NeverEverLand Sydney 2007.JPG",
            ]
        );
        assert_eq!(chosen[0].width, 1280);
        assert_eq!(chosen[0].height, 960);

        // At most six.
        let many: Vec<Candidate> = (0..10)
            .map(|n| {
                candidates(&json!({ "query": { "pages": [
                    page(&format!("File:Daft Punk live {n}.jpg"), "image/jpeg", 2000, 1300, ("cc-by-2.0", "CC BY 2.0"), "Someone"),
                ] } }))
                .remove(0)
            })
            .collect();
        assert_eq!(choose(Vec::new(), many, "Daft Punk").len(), MOST);
    }

    #[test]
    fn a_file_names_them_or_it_is_not_of_them() {
        assert!(names_them("File:DaftPunk NeverEverLand.JPG", "Daft Punk"));
        assert!(names_them("File:Beatles 1964.jpg", "The Beatles"));
        assert!(names_them(
            "File:The Beatles in America.JPG",
            "the  beatles"
        ));
        assert!(names_them("File:Sigur Rós Iceland.jpg", "Sigur Rós"));
        assert!(!names_them("File:Brassft punk.jpg", "Daft Punk"));
        assert!(!names_them("File:Anything.jpg", "!"));
    }

    /// A name search finds singers merely *called* Adele somewhere in their
    /// item; only the one whose own name — a label or an alias, in any
    /// language — is Adele is her.
    #[test]
    fn the_one_called_by_the_name_is_picked() {
        let names = json!({ "entities": {
            "Q470429": { "labels": { "en": { "value": "Emeli Sandé" } },
                         "aliases": { "en": [ { "value": "Adele Sandé" } ] } },
            "Q105941": { "labels": { "en": { "value": "Myrna Loy" } },
                         "aliases": { "en": [ { "value": "Myrna Adele Williams" } ] } },
            "Q23215":  { "labels": { "mul": { "value": "Adele" } },
                         "aliases": { "en": [ { "value": "Adele Adkins" } ] } },
        } });
        let hits = ["Q470429", "Q105941", "Q23215"].map(str::to_owned);
        assert_eq!(pick(&names, &hits, "  adele ").as_deref(), Some("Q23215"));
        assert_eq!(
            pick(&names, &hits, "Adele Adkins").as_deref(),
            Some("Q23215")
        );
        assert_eq!(pick(&names, &hits, "Adel"), None);
    }

    #[test]
    fn the_lead_artist_is_the_credit_before_its_guests() {
        assert_eq!(lead("Daft Punk feat. Pharrell Williams"), "Daft Punk");
        assert_eq!(lead("Simon & Garfunkel"), "Simon");
        assert_eq!(lead("Beyoncé x Jay-Z"), "Beyoncé");
        assert_eq!(lead("İlhan FEAT. Someone"), "İlhan");
        assert_eq!(lead("AC/DC"), "AC/DC");
        assert_eq!(lead("Daft Punk"), "Daft Punk");
    }

    /// **Asked once per artist and kept**, as K1 keeps covers: the whole
    /// asking from recorded answers — by the MusicBrainz id the record was
    /// tagged with, the item's picture and category, a deprecated picture
    /// never asked for — kept with its credits, and the second time nothing
    /// is asked at all.
    #[test]
    fn asked_once_per_artist_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let artist = Artist {
            name: "Daft Punk".to_owned(),
            id: Some(DAFT_PUNK.to_owned()),
        };
        let answers = RefCell::new(wikimedia(&artist));
        let asked = RefCell::new(Vec::new());
        let get = recorded(&answers, &asked);

        let first = tauri::async_runtime::block_on(find(dir.path(), &artist, &get)).unwrap();
        assert_eq!(first.entity.as_deref(), Some("Q185828"));
        let credits: Vec<&str> = first.pictures.iter().map(|p| p.credit.as_str()).collect();
        assert_eq!(
            credits,
            [
                "Photo: Sony Music Entertainment · CC BY 4.0 · Wikimedia Commons",
                "Photo: Marcelo Teson from Los Angeles, CA, USA · CC BY 2.0 · Wikimedia Commons",
                "Photo: Bonsaichop · CC BY-SA 3.0 · Wikimedia Commons",
            ]
        );
        assert!(asked.borrow()[0].contains("haswbstatement%3AP434%3D056e4f3e"));
        assert!(
            !asked.borrow().iter().any(|url| url.contains("Bangalter")),
            "a deprecated picture was asked for"
        );
        // Searched, the item, its picture, its category, three pictures.
        assert_eq!(asked.borrow().len(), 7, "{:#?}", asked.borrow());

        // Kept, each picture under its number and no more.
        let key = artist.key();
        assert_eq!(kept(dir.path(), &key), Some(first.clone()));
        let coachella = candidates(&category())[1].url.clone();
        assert_eq!(
            picture(dir.path(), &key, 1),
            Some(jpeg(1).bytes),
            "{coachella}"
        );
        assert_eq!(picture(dir.path(), &key, 3), None);

        asked.borrow_mut().clear();
        let again = tauri::async_runtime::block_on(find(dir.path(), &artist, &get)).unwrap();
        assert_eq!(again, first);
        assert!(
            asked.borrow().is_empty(),
            "asked again: {:?}",
            asked.borrow()
        );
    }

    /// An answer that did not come is not kept, so the artist is asked for
    /// again — Wikimedia asking to wait, here halfway through. Finding no one
    /// *is* kept.
    #[test]
    fn a_refusal_is_asked_again_and_no_one_found_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let artist = Artist {
            name: "Daft Punk".to_owned(),
            id: Some(DAFT_PUNK.to_owned()),
        };
        let mut recorded_answers = wikimedia(&artist);
        let category = category_url("Daft Punk").unwrap();
        let whole = recorded_answers.insert(
            category.clone(),
            Reply {
                status: 429,
                mime: Some("text/plain".to_owned()),
                bytes: b"You are making too many requests to the API.".to_vec(),
            },
        );
        let answers = RefCell::new(recorded_answers);
        let asked = RefCell::new(Vec::new());
        let get = recorded(&answers, &asked);
        let refused = tauri::async_runtime::block_on(find(dir.path(), &artist, &get));
        assert_eq!(refused, Err("Wikimedia answered 429".to_owned()));
        assert_eq!(kept(dir.path(), &artist.key()), None);

        answers.borrow_mut().insert(category, whole.unwrap());
        let found = tauri::async_runtime::block_on(find(dir.path(), &artist, &get)).unwrap();
        assert_eq!(found.pictures.len(), 3);

        // No one by that name among MusicBrainz artists: kept as none.
        let nobody = Artist {
            name: "The Unknown Openers".to_owned(),
            id: None,
        };
        answers.borrow_mut().insert(
            search_url(&nobody, &nobody.name).unwrap(),
            reply(&found_nothing()),
        );
        asked.borrow_mut().clear();
        let none = tauri::async_runtime::block_on(find(dir.path(), &nobody, &get)).unwrap();
        assert!(none.pictures.is_empty());
        assert_eq!(asked.borrow().len(), 1);
        assert_eq!(kept(dir.path(), &nobody.key()), Some(none));
    }

    fn found_nothing() -> Value {
        found(&[])
    }

    /// By name: a search among MusicBrainz artists, the one so called picked
    /// from what it finds, and the lead artist searched for when the whole
    /// credit — guests and all — finds no one.
    #[test]
    fn by_name_the_lead_artist_is_found_when_the_whole_credit_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let artist = Artist {
            name: "Daft Punk feat. Pharrell Williams".to_owned(),
            id: None,
        };
        let by_id = Artist {
            name: "Daft Punk".to_owned(),
            id: Some(DAFT_PUNK.to_owned()),
        };
        let mut recorded_answers = wikimedia(&by_id);
        recorded_answers.insert(
            search_url(&artist, &artist.name).unwrap(),
            reply(&found_nothing()),
        );
        recorded_answers.insert(
            search_url(&artist, "Daft Punk").unwrap(),
            reply(&found(&["Q1000", "Q185828"])),
        );
        recorded_answers.insert(
            names_url(&["Q1000".to_owned(), "Q185828".to_owned()]).unwrap(),
            reply(&json!({ "entities": {
                "Q1000": { "labels": { "en": { "value": "Daft Punk tribute band" } } },
                "Q185828": { "labels": { "en": { "value": "Daft Punk" } } },
            } })),
        );
        let answers = RefCell::new(recorded_answers);
        let asked = RefCell::new(Vec::new());
        let get = recorded(&answers, &asked);
        let found = tauri::async_runtime::block_on(find(dir.path(), &artist, &get)).unwrap();
        assert_eq!(found.entity.as_deref(), Some("Q185828"));
        assert_eq!(found.artist, "Daft Punk feat. Pharrell Williams");
        assert_eq!(found.pictures.len(), 3);
        assert!(asked.borrow()[0].contains("haswbstatement%3AP434"));
        assert!(
            !asked.borrow()[0].contains("%3D"),
            "a name search is not by id"
        );
    }

    #[test]
    fn a_download_must_be_a_jpeg_and_a_refusal_stops_the_asking() {
        assert_eq!(image(jpeg(7)).unwrap(), Some(jpeg(7).bytes));
        let gone = Reply {
            status: 404,
            mime: None,
            bytes: Vec::new(),
        };
        assert_eq!(image(gone).unwrap(), None);
        let page = Reply {
            status: 200,
            mime: Some("text/html".to_owned()),
            bytes: b"<html>".to_vec(),
        };
        assert_eq!(image(page).unwrap(), None);
        let lying = Reply {
            status: 200,
            mime: Some("image/jpeg".to_owned()),
            bytes: b"GIF89a".to_vec(),
        };
        assert_eq!(image(lying).unwrap(), None);
        let huge = Reply {
            status: 200,
            mime: Some("image/jpeg".to_owned()),
            bytes: [vec![0xFF, 0xD8], vec![0; MOST_BYTES]].concat(),
        };
        assert_eq!(image(huge).unwrap(), None);
        let wait = Reply {
            status: 429,
            mime: None,
            bytes: Vec::new(),
        };
        assert!(image(wait).is_err());
    }

    /// **No name from the interface reaches outside the folder**: an artist's
    /// folder is their MusicBrainz id or a hash of their name, and a picture
    /// is read only by a key of that form and a number that was kept.
    #[test]
    fn no_name_from_the_interface_reaches_outside_the_folder() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("artists");
        std::fs::create_dir_all(&dir).unwrap();
        let slashed = Artist {
            name: "../../etc/AC/DC".to_owned(),
            id: None,
        };
        let key = slashed.key();
        assert!(key.starts_with("name-") && key.len() == 21, "{key}");
        assert_eq!(
            key,
            Artist {
                name: "../../ETC/ac/dc ".to_owned(),
                id: None
            }
            .key()
        );
        let found = Found {
            artist: slashed.name.clone(),
            entity: None,
            pictures: vec![Picture {
                width: 4,
                height: 3,
                author: "A".to_owned(),
                licence: "CC0".to_owned(),
                page: String::new(),
                credit: "Photo: A · CC0 · Wikimedia Commons".to_owned(),
            }],
        };
        keep(&dir, &key, &found, &[jpeg(1).bytes]).unwrap();
        assert_eq!(picture(&dir, &key, 0), Some(jpeg(1).bytes));

        // The same files, reached by a key the interface made up.
        let outside = root.path().join("secret");
        std::fs::create_dir_all(&outside).unwrap();
        keep(&outside, "x", &found, &[b"secret".to_vec()]).unwrap();
        assert_eq!(picture(&dir, "../secret/x", 0), None);
        assert_eq!(picture(&dir, &format!("{key}/../{key}"), 0), None);
        assert_eq!(picture(&dir, "name-zzzzzzzzzzzzzzzz", 0), None);
        assert_eq!(picture(&dir, &key, 1), None);
    }

    /// Two screens following one deck ask once between them, and an
    /// answer that did not come is waited on — said why — before it is
    /// asked for again.
    #[test]
    fn asked_by_one_screen_at_a_time_and_waited_on_after_a_failure() {
        let asking = Asking::new();
        let artist = Artist {
            name: "Daft Punk".to_owned(),
            id: None,
        };
        let now = Instant::now();
        assert_eq!(asking.begin(&artist, now), Ok(()));
        let busy = asking.begin(&artist, now).unwrap_err();
        assert_eq!(busy.state, "asking");
        assert_eq!(busy.key, Some(artist.key()));

        asking.end(&artist, Err("Wikimedia answered 429".to_owned()), now);
        let waiting = asking.begin(&artist, now + WAIT / 2).unwrap_err();
        assert_eq!(waiting.state, "waiting");
        assert_eq!(waiting.why.as_deref(), Some("Wikimedia answered 429"));
        assert_eq!(asking.begin(&artist, now + WAIT), Ok(()));
        asking.end(&artist, Ok(()), now + WAIT);
        assert_eq!(asking.begin(&artist, now + WAIT), Ok(()));
    }
}
