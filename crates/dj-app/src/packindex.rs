//! §122 step 7c: indexes of packs — JSON files anywhere on the web naming
//! packs, where each is, and the key it is signed with. See
//! `docs/VISUALS.md`, step 7.
//!
//! An index is read only when the DJ asks to browse, and only the indexes the
//! DJ keeps in their list. What it says is checked before it is shown —
//! [`parse`] leaves out any entry it cannot vouch for the shape of — and its
//! word is checked again against the pack itself when one is fetched:
//! [`matches()`] refuses a pack whose key or name is not the one the index
//! lists. The fetching is given to [`browse`] and [`download`], so the whole
//! of it is tested with recorded answers.

use crate::packs::{Checked, Installed};
use serde::{Deserialize, Serialize};
use std::future::Future;

/// The format of index this djmanzo reads.
pub const FORMAT: u32 = 1;

/// The community index, in this repository, read from GitHub's raw address.
pub const COMMUNITY: &str =
    "https://raw.githubusercontent.com/joehannes/djmanzo/main/community/packs.json";

/// The largest index read.
pub const MOST_INDEX: usize = 1024 * 1024;

/// The most packs one index lists.
pub const MOST_PACKS: usize = 500;

/// The largest pack downloaded: its unpacked limit, and room for the zip.
pub const MOST_DOWNLOAD: usize = 25 * 1024 * 1024;

/// One answer to a request: its status and as much of its body as was read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    pub bytes: Vec<u8>,
    /// The body was longer than was asked to be read.
    pub cut: bool,
}

/// A pack as an index lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub author: String,
    pub licence: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    /// Where its `.djpack` is.
    pub url: String,
    /// The fingerprint of the key it is signed with, as [`crate::packs::fingerprint`]
    /// writes it.
    pub key: String,
}

/// An index, as read: its name and the entries that could be shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Index {
    pub name: String,
    pub packs: Vec<Entry>,
    /// How many entries were left out, and so not shown.
    pub left_out: usize,
}

/// One index as the browser shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Seen {
    pub url: String,
    pub name: String,
    pub packs: usize,
    pub left_out: usize,
    /// Why it could not be read, when it could not.
    pub problem: Option<String>,
}

/// One pack as the browser lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Listed {
    /// The index it is listed in.
    pub index: String,
    #[serde(flatten)]
    pub entry: Entry,
    /// What the DJ has of it: `new`, `installed`, `update` (same key, another
    /// version) or `other_key` (a pack of its name signed by another key).
    pub state: &'static str,
}

/// What browsing came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct Browsed {
    pub indexes: Vec<Seen>,
    pub packs: Vec<Listed>,
}

/// Whether `url` may be asked for: https, or plain http to the DJ's own
/// machine.
#[must_use]
pub fn allowed(url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(url) else {
        return false;
    };
    match url.scheme() {
        "https" => url.host_str().is_some(),
        "http" => matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")),
        _ => false,
    }
}

/// Whether `key` is written as a fingerprint is: four groups of four hex.
fn is_fingerprint(key: &str) -> bool {
    let groups: Vec<&str> = key.split(' ').collect();
    groups.len() == 4
        && groups
            .iter()
            .all(|g| g.len() == 4 && g.chars().all(|c| c.is_ascii_hexdigit()))
}

/// An entry's fields within the limits a pack's own are held to.
fn sound(entry: &Entry) -> bool {
    let short = |field: &str| !field.trim().is_empty() && field.chars().count() <= 80;
    short(&entry.name)
        && short(&entry.author)
        && short(&entry.licence)
        && short(&entry.version)
        && entry.description.chars().count() <= 600
        && allowed(&entry.url)
        && is_fingerprint(&entry.key)
}

/// Read an index. An entry that is not whole and sound is left out and
/// counted; two entries of one name keep the first.
///
/// # Errors
/// Not JSON, not an index, a newer format, or too large.
pub fn parse(bytes: &[u8]) -> Result<Index, String> {
    if bytes.len() > MOST_INDEX {
        return Err(format!("it is larger than {} MB", MOST_INDEX / 1024 / 1024));
    }
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| "it is not an index: not JSON".to_owned())?;
    let format = value
        .get("format")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "it is not an index of packs".to_owned())?;
    if format != u64::from(FORMAT) {
        return Err(format!(
            "it is in index format {format}, and this djmanzo reads format {FORMAT}: a newer djmanzo may read it"
        ));
    }
    let name = value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(|n| n.trim().chars().take(80).collect::<String>())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "An index".to_owned());
    let listed = value
        .get("packs")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "it lists no packs".to_owned())?;
    if listed.len() > MOST_PACKS {
        return Err(format!("it lists more than {MOST_PACKS} packs"));
    }
    let mut packs: Vec<Entry> = Vec::new();
    let mut left_out = 0;
    for item in listed {
        match serde_json::from_value::<Entry>(item.clone()) {
            Ok(mut entry) if sound(&entry) => {
                entry.key = entry.key.to_ascii_lowercase();
                if packs.iter().any(|p| p.name == entry.name) {
                    left_out += 1;
                } else {
                    packs.push(entry);
                }
            }
            _ => left_out += 1,
        }
    }
    Ok(Index {
        name,
        packs,
        left_out,
    })
}

/// What the DJ has of `entry`, from the packs installed.
#[must_use]
pub fn state(entry: &Entry, installed: &[Installed]) -> &'static str {
    match installed.iter().find(|pack| pack.name == entry.name) {
        None => "new",
        Some(pack) if pack.key != entry.key => "other_key",
        Some(pack) if pack.version == entry.version => "installed",
        Some(_) => "update",
    }
}

/// Read every index in `urls`, in order, with `get` (asked for at most
/// [`MOST_INDEX`] bytes), and list their packs against what is installed.
/// An index that cannot be read says why and lists nothing; one pack name
/// in two indexes is listed from the first.
pub async fn browse<F, Fut>(urls: &[String], installed: &[Installed], get: F) -> Browsed
where
    F: Fn(String, usize) -> Fut,
    Fut: Future<Output = Result<Reply, String>>,
{
    let mut browsed = Browsed::default();
    for url in urls {
        let read = if allowed(url) {
            get(url.clone(), MOST_INDEX).await.and_then(|reply| {
                if !(200..300).contains(&reply.status) {
                    Err(format!("it answered {}", reply.status))
                } else if reply.cut {
                    Err(format!("it is larger than {} MB", MOST_INDEX / 1024 / 1024))
                } else {
                    parse(&reply.bytes)
                }
            })
        } else {
            Err("its address is not https".to_owned())
        };
        match read {
            Ok(index) => {
                browsed.indexes.push(Seen {
                    url: url.clone(),
                    name: index.name,
                    packs: index.packs.len(),
                    left_out: index.left_out,
                    problem: None,
                });
                for entry in index.packs {
                    if browsed.packs.iter().any(|p| p.entry.name == entry.name) {
                        continue;
                    }
                    browsed.packs.push(Listed {
                        index: url.clone(),
                        state: state(&entry, installed),
                        entry,
                    });
                }
            }
            Err(why) => browsed.indexes.push(Seen {
                url: url.clone(),
                name: String::new(),
                packs: 0,
                left_out: 0,
                problem: Some(why),
            }),
        }
    }
    browsed
}

/// Download the `.djpack` `entry` names, with `get` (asked for at most
/// [`MOST_DOWNLOAD`] bytes).
///
/// # Errors
/// Its address is not allowed, it did not answer with success, or it is too
/// large.
pub async fn download<F, Fut>(entry: &Entry, get: F) -> Result<Vec<u8>, String>
where
    F: FnOnce(String, usize) -> Fut,
    Fut: Future<Output = Result<Reply, String>>,
{
    if !allowed(&entry.url) {
        return Err("its address is not https".to_owned());
    }
    let reply = get(entry.url.clone(), MOST_DOWNLOAD).await?;
    if !(200..300).contains(&reply.status) {
        return Err(format!("{} answered {}", entry.url, reply.status));
    }
    if reply.cut {
        return Err(format!(
            "it is larger than {} MB",
            MOST_DOWNLOAD / 1024 / 1024
        ));
    }
    Ok(reply.bytes)
}

/// Whether the pack fetched is the one the index lists: its name, and the key
/// it is signed with.
///
/// # Errors
/// Either differs, said in words a DJ can act on.
pub fn matches(pack: &Checked, entry: &Entry) -> Result<(), String> {
    if pack.fingerprint() != entry.key {
        return Err(format!(
            "it is signed by the key {}, and the index says {} signs it: it is not the pack the index lists",
            pack.fingerprint(),
            entry.key
        ));
    }
    if pack.manifest.name != entry.name {
        return Err(format!(
            "it is called {:?}, and the index lists {:?}: it is not the pack the index lists",
            pack.manifest.name, entry.name
        ));
    }
    Ok(())
}

/// Fetch `url` with `client`, reading at most `most` bytes of its body.
///
/// # Errors
/// No answer within twenty seconds a part, or the connection failing.
pub async fn fetch(client: &reqwest::Client, url: String, most: usize) -> Result<Reply, String> {
    let mut response = client
        .get(&url)
        .header(reqwest::header::USER_AGENT, crate::artists::USER_AGENT)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    let mut bytes = Vec::new();
    let mut cut = false;
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > most {
            cut = true;
            break;
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(Reply { status, bytes, cut })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::looks::Looks;
    use std::collections::HashMap;
    use std::path::Path;

    fn entry(name: &str, key: &str, url: &str) -> serde_json::Value {
        serde_json::json!({
            "name": name, "author": "A Maker", "licence": "CC BY 4.0",
            "version": "1.0.0", "description": "Neon.", "url": url, "key": key,
        })
    }

    fn index(packs: Vec<serde_json::Value>) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"format": 1, "name": "Test packs", "packs": packs}))
            .unwrap()
    }

    fn ok(bytes: Vec<u8>) -> Result<Reply, String> {
        Ok(Reply {
            status: 200,
            bytes,
            cut: false,
        })
    }

    /// Answers recorded by address; anything else is "no network".
    fn recorded(
        answers: HashMap<String, Result<Reply, String>>,
    ) -> impl Fn(String, usize) -> std::future::Ready<Result<Reply, String>> {
        move |url, _| {
            std::future::ready(
                answers
                    .get(&url)
                    .cloned()
                    .unwrap_or_else(|| Err("no network".to_owned())),
            )
        }
    }

    const KEY: &str = "0a1b 2c3d 4e5f 6a7b";

    /// **Only what an index can vouch for the shape of is shown**: an entry
    /// missing a field, with an address that is not https, a key that is not
    /// a fingerprint, or a name already listed is left out and counted.
    #[test]
    fn an_index_shows_only_whole_entries() {
        let read = parse(&index(vec![
            entry("Neon Nights", KEY, "https://example.org/neon.djpack"),
            entry("Plain", KEY, "http://example.org/plain.djpack"),
            entry("Local", KEY, "http://127.0.0.1:8080/local.djpack"),
            entry("Badkey", "not a key", "https://example.org/b.djpack"),
            entry("Neon Nights", KEY, "https://example.org/again.djpack"),
            serde_json::json!({"name": "Half"}),
            entry("Script", KEY, "javascript:alert(1)"),
        ]))
        .unwrap();
        assert_eq!(read.name, "Test packs");
        assert_eq!(
            read.packs
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            ["Neon Nights", "Local"]
        );
        assert_eq!(read.left_out, 5);

        assert!(parse(b"<html>").unwrap_err().contains("not JSON"));
        assert!(
            parse(br#"{"format":2,"packs":[]}"#)
                .unwrap_err()
                .contains("newer")
        );
        assert!(
            parse(br#"{"packs":[]}"#)
                .unwrap_err()
                .contains("not an index")
        );
        assert!(
            parse(&vec![b' '; MOST_INDEX + 1])
                .unwrap_err()
                .contains("larger")
        );
    }

    /// **Browsing reads the DJ's indexes in order** and lists each pack once,
    /// against what the DJ has; an index that cannot be read says why and
    /// lists nothing, and an address that is not allowed is never asked.
    #[test]
    fn browsing_lists_each_pack_against_what_is_installed() {
        let installed = |name: &str, key: &str, version: &str| Installed {
            id: "x".to_owned(),
            name: name.to_owned(),
            author: String::new(),
            licence: String::new(),
            version: version.to_owned(),
            description: String::new(),
            looks: 1,
            key: key.to_owned(),
            problem: None,
        };
        let have = [
            installed("Same", KEY, "1.0.0"),
            installed("Older", KEY, "0.9.0"),
            installed("Theirs", "ffff ffff ffff ffff", "1.0.0"),
        ];
        let a = "https://a.example/index.json".to_owned();
        let b = "https://b.example/index.json".to_owned();
        let down = "https://down.example/index.json".to_owned();
        let asked = std::sync::Mutex::new(Vec::new());
        let answers: HashMap<String, Result<Reply, String>> = [
            (
                a.clone(),
                ok(index(vec![
                    entry("New", KEY, "https://a.example/new.djpack"),
                    entry("Same", KEY, "https://a.example/same.djpack"),
                    entry("Older", KEY, "https://a.example/older.djpack"),
                    entry("Theirs", KEY, "https://a.example/theirs.djpack"),
                ])),
            ),
            (
                b.clone(),
                ok(index(vec![entry(
                    "New",
                    KEY,
                    "https://b.example/new.djpack",
                )])),
            ),
            (
                down.clone(),
                Ok(Reply {
                    status: 404,
                    bytes: Vec::new(),
                    cut: false,
                }),
            ),
        ]
        .into();
        let get = recorded(answers);
        let urls = [
            a.clone(),
            b.clone(),
            down.clone(),
            "ftp://c.example/index.json".to_owned(),
        ];
        let browsed = futures_lite(browse(&urls, &have, |url, most| {
            asked.lock().unwrap().push(url.clone());
            get(url, most)
        }));
        assert_eq!(
            browsed
                .packs
                .iter()
                .map(|p| (p.entry.name.as_str(), p.state, p.index.as_str()))
                .collect::<Vec<_>>(),
            [
                ("New", "new", a.as_str()),
                ("Same", "installed", a.as_str()),
                ("Older", "update", a.as_str()),
                ("Theirs", "other_key", a.as_str()),
            ]
        );
        assert_eq!(browsed.indexes.len(), 4);
        assert_eq!(browsed.indexes[0].packs, 4);
        assert!(
            browsed.indexes[2]
                .problem
                .as_deref()
                .unwrap()
                .contains("404")
        );
        assert!(
            browsed.indexes[3]
                .problem
                .as_deref()
                .unwrap()
                .contains("https")
        );
        assert_eq!(*asked.lock().unwrap(), [a, b, down], "ftp never asked");
    }

    /// Drive a future that never waits on anything (recorded answers).
    fn futures_lite<F: Future>(future: F) -> F::Output {
        tauri::async_runtime::block_on(future)
    }

    fn signed_pack(dir: &Path, name: &str) -> (std::path::PathBuf, String) {
        let home = dir.join(format!("maker-{name}"));
        let looks = Looks::new();
        looks.start(&home);
        let elsewhere = dir.join(format!("presets-{name}"));
        std::fs::create_dir_all(&elsewhere).unwrap();
        let preset = elsewhere.join("glow.json");
        std::fs::write(&preset, crate::looks::PRESETS[0].1).unwrap();
        assert!(looks.add(&[preset]).refused.is_empty());
        let made = looks
            .make_pack(
                &crate::packs::Making {
                    name: name.to_owned(),
                    author: "A Maker".to_owned(),
                    licence: "CC BY 4.0".to_owned(),
                    version: "1.0.0".to_owned(),
                    description: String::new(),
                    presets: vec!["glow".to_owned()],
                    theirs: true,
                },
                &dir.join(format!("{name}.djpack")),
            )
            .unwrap();
        (made.path.into(), made.key)
    }

    /// **The index's word is checked against the pack**: a pack signed by
    /// another key than the index lists, or called by another name, is
    /// refused; one that matches downloads and is shown as any pack is; a
    /// download too large or refused by its host is not taken.
    #[test]
    fn a_pack_from_an_index_must_be_the_one_it_lists() {
        let dir = tempfile::tempdir().unwrap();
        let (path, key) = signed_pack(dir.path(), "Neon Nights");
        let bytes = std::fs::read(&path).unwrap();
        let pack = crate::packs::read_archive(&path)
            .and_then(crate::packs::check)
            .unwrap();
        let url = "https://example.org/neon.djpack";
        let listed: Entry = serde_json::from_value(entry("Neon Nights", &key, url)).unwrap();
        assert_eq!(matches(&pack, &listed), Ok(()));

        let forged: Entry = serde_json::from_value(entry("Neon Nights", KEY, url)).unwrap();
        assert!(
            matches(&pack, &forged)
                .unwrap_err()
                .contains("signed by the key")
        );
        let renamed: Entry = serde_json::from_value(entry("Other", &key, url)).unwrap();
        assert!(matches(&pack, &renamed).unwrap_err().contains("is called"));

        let got = futures_lite(download(
            &listed,
            recorded([(url.to_owned(), ok(bytes.clone()))].into()),
        ));
        assert_eq!(got.unwrap(), bytes);
        let too_big = futures_lite(download(
            &listed,
            recorded(
                [(
                    url.to_owned(),
                    Ok(Reply {
                        status: 200,
                        bytes: Vec::new(),
                        cut: true,
                    }),
                )]
                .into(),
            ),
        ));
        assert!(too_big.unwrap_err().contains("larger"));
        let gone = futures_lite(download(
            &listed,
            recorded(
                [(
                    url.to_owned(),
                    Ok(Reply {
                        status: 404,
                        bytes: Vec::new(),
                        cut: false,
                    }),
                )]
                .into(),
            ),
        ));
        assert!(gone.unwrap_err().contains("404"));
        let mut plain = listed.clone();
        plain.url = "http://example.org/neon.djpack".to_owned();
        assert!(futures_lite(download(&plain, recorded(HashMap::new()))).is_err());
    }

    /// **The DJ's list of indexes, and a pack got through one.** The list
    /// starts with the community index, takes only https, keeps each once,
    /// and is kept between runs; the community index can be removed. A pack
    /// is fetched only by the index and name the last browse listed it
    /// under, shown before anything is installed — refused when it is not
    /// the pack the index lists — and installed from where it was kept.
    #[test]
    fn the_djs_indexes_and_a_pack_got_through_one() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("dj");
        let looks = Looks::new();
        looks.start(&home);
        assert_eq!(looks.pack_indexes(), [COMMUNITY]);
        assert!(looks.add_pack_index("http://example.org/i.json").is_err());
        assert!(looks.add_pack_index("file:///etc/passwd").is_err());
        let own = "https://example.org/index.json";
        assert_eq!(looks.add_pack_index(own).unwrap(), [COMMUNITY, own]);
        assert!(looks.add_pack_index(own).unwrap_err().contains("already"));
        let later = Looks::new();
        later.start(&home);
        assert_eq!(later.pack_indexes(), [COMMUNITY, own]);
        assert_eq!(later.remove_pack_index(COMMUNITY).unwrap(), [own]);
        assert!(later.remove_pack_index(COMMUNITY).is_err());

        let (path, key) = signed_pack(dir.path(), "Neon Nights");
        let bytes = std::fs::read(&path).unwrap();
        let listed: Entry = serde_json::from_value(entry(
            "Neon Nights",
            &key,
            "https://example.org/neon.djpack",
        ))
        .unwrap();
        assert!(later.listed(own, "Neon Nights").is_err(), "not browsed yet");
        later.remember_browsed(&Browsed {
            indexes: Vec::new(),
            packs: vec![Listed {
                index: own.to_owned(),
                entry: listed.clone(),
                state: "new",
            }],
        });
        assert_eq!(later.listed(own, "Neon Nights").unwrap(), listed);
        assert!(later.listed(COMMUNITY, "Neon Nights").is_err());

        // Listed under another key: shown, refused, nothing installed.
        let mut forged = listed.clone();
        forged.key = KEY.to_owned();
        let (_, seen) = later.downloaded(&forged, &bytes).unwrap();
        assert!(
            seen.problem
                .as_deref()
                .unwrap()
                .contains("signed by the key")
        );
        assert!(later.packs().is_empty());

        let (kept, seen) = later.downloaded(&listed, &bytes).unwrap();
        assert_eq!(seen.problem, None, "{seen:?}");
        assert_eq!(seen.licence, "CC BY 4.0");
        assert!(later.packs().is_empty(), "shown, not installed");
        let installed = later.install_pack(&kept).unwrap();
        assert_eq!(installed[0].key, key);
        assert_eq!(state(&listed, &installed), "installed");
        // The download is not taken for a pack.
        let again = Looks::new();
        again.start(&home);
        assert_eq!(again.packs().len(), 1);
    }

    /// The community index this repository serves is one this djmanzo reads.
    #[test]
    fn the_community_index_is_readable() {
        let bytes =
            std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../community/packs.json"))
                .unwrap();
        let read = parse(&bytes).unwrap();
        assert_eq!(read.left_out, 0, "every entry in it can be shown");
        assert!(allowed(COMMUNITY));
    }
}
