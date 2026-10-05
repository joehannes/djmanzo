//! §122 step 7: packs — looks and the MilkDrop presets they draw, made by
//! someone, signed by them, and checked whole before they are installed.
//! See `docs/VISUALS.md`, step 7.
//!
//! A pack is one file, `<name>.djpack`: a zip holding [`MANIFEST`] (name,
//! author, licence, version, the author's Ed25519 key, every other file with
//! its SHA-256, and the looks), [`SIGNATURE`] (the author's signature over the
//! manifest's exact bytes) and the presets under `milkdrop/`. [`check`] is the
//! whole of what installing one means: it is refused — with its reason — unless
//! the signature verifies, every file is there with its hash and nothing is
//! there unlisted, every preset can be drawn, and every look draws only what
//! djmanzo draws from the pack's own presets.
//!
//! The signature says the pack is the one its key's holder made, unchanged;
//! the fingerprint says who that is to a DJ who knows the maker's. Neither
//! says its licence is true: a pack's works are its author's to answer for,
//! as the DJ's own presets are theirs, and djmanzo never sends them anywhere.

use crate::looks::{BLENDS, KINDS, Layer, Look, SIGNALS};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use sha2::Digest as _;
use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::Path;

/// The format this djmanzo reads.
pub const FORMAT: u32 = 1;

/// The manifest's name in a pack.
pub const MANIFEST: &str = "pack.json";

/// The signature's name in a pack.
pub const SIGNATURE: &str = "pack.sig";

/// Where a pack keeps its presets.
pub const PRESETS: &str = "milkdrop/";

/// The largest pack taken, unpacked.
pub const MOST_BYTES: u64 = 20 * 1024 * 1024;

/// The largest file in a pack.
pub const MOST_FILE: u64 = 2 * 1024 * 1024;

/// The most files in a pack.
pub const MOST_FILES: usize = 200;

/// The longest name, author, licence, version or look name, in characters.
const SHORT: usize = 80;

/// The longest description.
const LONG: usize = 600;

/// What `pack.json` says.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: u32,
    pub name: String,
    pub author: String,
    pub licence: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    /// The author's Ed25519 public key, base64.
    pub key: String,
    /// Every file but the manifest and the signature, by its path in the pack,
    /// with its SHA-256 in hex.
    pub files: BTreeMap<String, String>,
    /// The looks, in djmanzo's own form; a MilkDrop layer names its preset by
    /// file name, `"neon"` for `milkdrop/neon.json` or `.milk`.
    pub looks: Vec<Look>,
}

/// A pack's files, as they came out of its archive or its folder.
pub type Files = BTreeMap<String, Vec<u8>>;

/// A pack that has passed [`check`].
#[derive(Debug, Clone, PartialEq)]
pub struct Checked {
    pub manifest: Manifest,
    /// The key, decoded.
    pub key: Vec<u8>,
    /// Every file, the manifest and signature included, as checked.
    pub files: Files,
}

impl Checked {
    /// Its folder's name once installed: its name, made plain, and its key's
    /// fingerprint — never a name the pack chose for itself.
    #[must_use]
    pub fn id(&self) -> String {
        id(&self.manifest.name, &self.key)
    }

    /// The fingerprint of its key.
    #[must_use]
    pub fn fingerprint(&self) -> String {
        fingerprint(&self.key)
    }
}

/// What the DJ is shown before installing a pack.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Inspected {
    pub name: String,
    pub author: String,
    pub licence: String,
    pub version: String,
    pub description: String,
    pub looks: Vec<String>,
    pub presets: usize,
    /// The fingerprint of the key it is signed with.
    pub key: String,
    /// The version installed now, when a pack of this name is.
    pub installed: Option<String>,
    /// Why it cannot be installed, when it cannot.
    pub problem: Option<String>,
}

/// A pack as it is listed once installed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Installed {
    pub id: String,
    pub name: String,
    pub author: String,
    pub licence: String,
    pub version: String,
    pub description: String,
    pub looks: usize,
    pub key: String,
    /// Why it is not offered, when it is not: changed since it was installed.
    pub problem: Option<String>,
}

/// The fingerprint of a key: the first 64 bits of its SHA-256, in four groups
/// — enough for a DJ to compare with what its maker publishes.
#[must_use]
pub fn fingerprint(key: &[u8]) -> String {
    let digest = hex(&sha2::Sha256::digest(key));
    digest.as_bytes()[..16]
        .chunks(4)
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A pack's folder name: its name in lower-case letters, digits and hyphens,
/// then its key's fingerprint.
#[must_use]
pub fn id(name: &str, key: &[u8]) -> String {
    let mut slug = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug: String = slug.trim_end_matches('-').chars().take(40).collect();
    let print = fingerprint(key).replace(' ', "");
    if slug.is_empty() {
        format!("pack-{}", &print[..8])
    } else {
        format!("{slug}-{}", &print[..8])
    }
}

/// Whether `id` is one [`id`] could have made: what a name from the
/// interface must be before a folder is looked for by it.
#[must_use]
pub fn is_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 60
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.starts_with('-')
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

/// A file's SHA-256, in hex.
#[must_use]
pub fn sha256(bytes: &[u8]) -> String {
    hex(&sha2::Sha256::digest(bytes))
}

/// The files of the pack archive at `path`, within the limits: no file
/// climbing out of the pack, no link, no file twice.
///
/// # Errors
/// Not a zip, too large, too many files, or a name or entry refused.
pub fn read_archive(path: &Path) -> Result<Files, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|_| "it is not a pack: not a zip".to_owned())?;
    let mut files = Files::new();
    let mut total: u64 = 0;
    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|e| format!("it could not be unpacked: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_owned();
        if entry.is_symlink() {
            return Err(format!("{name} is a link, which a pack may not hold"));
        }
        if entry.enclosed_name().is_none() || name.starts_with('/') || name.contains('\\') {
            return Err(format!("{name:?} points outside the pack"));
        }
        if files.len() == MOST_FILES {
            return Err(format!("it holds more than {MOST_FILES} files"));
        }
        // Read no more than the limit, whatever the archive claims.
        let mut bytes = Vec::new();
        (&mut entry)
            .take(MOST_FILE + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("{name} could not be read: {e}"))?;
        if bytes.len() as u64 > MOST_FILE {
            return Err(format!(
                "{name} is larger than {} MB",
                MOST_FILE / 1024 / 1024
            ));
        }
        total += bytes.len() as u64;
        if total > MOST_BYTES {
            return Err(format!("it is larger than {} MB", MOST_BYTES / 1024 / 1024));
        }
        if files.insert(name.clone(), bytes).is_some() {
            return Err(format!("{name} is in it twice"));
        }
    }
    Ok(files)
}

/// Whether `path` is a plain path to a preset: under [`PRESETS`], one name,
/// `.milk` or `.json`, nothing hidden and nothing climbing.
fn is_preset_path(path: &str) -> bool {
    let Some(file) = path.strip_prefix(PRESETS) else {
        return false;
    };
    let lower = file.to_ascii_lowercase();
    !file.is_empty()
        && !file.starts_with('.')
        && !file.contains(['/', '\\', ':'])
        && (lower.ends_with(".milk") || lower.ends_with(".json"))
}

/// A preset's name in its look: its file name without the extension.
fn stem(path: &str) -> &str {
    let file = path.strip_prefix(PRESETS).unwrap_or(path);
    file.rsplit_once('.').map_or(file, |(stem, _)| stem)
}

fn short(field: &str, what: &str) -> Result<(), String> {
    if field.trim().is_empty() {
        return Err(format!("its {what} is empty"));
    }
    if field.chars().count() > SHORT {
        return Err(format!("its {what} is longer than {SHORT} characters"));
    }
    Ok(())
}

/// Whether a layer's colour is one the interface reads: `record`, or
/// `#rrggbb`.
fn colour_is_drawn(colour: &str) -> bool {
    colour == "record"
        || (colour.len() == 7
            && colour.starts_with('#')
            && colour[1..].chars().all(|c| c.is_ascii_hexdigit()))
}

/// Check a pack whole. See the module's documentation for what that is.
///
/// # Errors
/// The first thing that does not hold, in words a DJ can act on.
pub fn check(files: Files) -> Result<Checked, String> {
    let manifest_bytes = files
        .get(MANIFEST)
        .ok_or_else(|| format!("it has no {MANIFEST}: it is not a djmanzo pack"))?;
    let manifest: Manifest = serde_json::from_slice(manifest_bytes)
        .map_err(|e| format!("its {MANIFEST} could not be read: {e}"))?;
    if manifest.format != FORMAT {
        return Err(format!(
            "it is in pack format {}, and this djmanzo reads format {FORMAT}: a newer djmanzo may read it",
            manifest.format
        ));
    }
    short(&manifest.name, "name")?;
    short(&manifest.author, "author")?;
    short(&manifest.licence, "licence")?;
    short(&manifest.version, "version")?;
    if manifest.description.chars().count() > LONG {
        return Err(format!("its description is longer than {LONG} characters"));
    }

    // The signature, before anything the manifest says is believed.
    let key = base64::engine::general_purpose::STANDARD
        .decode(manifest.key.trim())
        .map_err(|_| "its key is not base64".to_owned())?;
    if key.len() != 32 {
        return Err("its key is not an Ed25519 key".to_owned());
    }
    let signature_text = files
        .get(SIGNATURE)
        .ok_or_else(|| format!("it is not signed: there is no {SIGNATURE}"))?;
    let signature = base64::engine::general_purpose::STANDARD
        .decode(String::from_utf8_lossy(signature_text).trim())
        .map_err(|_| "its signature is not base64".to_owned())?;
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, &key)
        .verify(manifest_bytes, &signature)
        .map_err(|_| {
            "its signature does not match: it was changed after it was signed, or signed with another key"
                .to_owned()
        })?;

    // Every file listed, with its hash; nothing unlisted.
    for (path, hash) in &manifest.files {
        if !is_preset_path(path) {
            return Err(format!(
                "it lists {path:?}, and a pack holds only presets under {PRESETS}"
            ));
        }
        let bytes = files
            .get(path)
            .ok_or_else(|| format!("{path} is listed and not in it"))?;
        if !sha256(bytes).eq_ignore_ascii_case(hash.trim()) {
            return Err(format!("{path} is not the file that was signed"));
        }
    }
    if let Some(extra) = files
        .keys()
        .find(|name| *name != MANIFEST && *name != SIGNATURE && !manifest.files.contains_key(*name))
    {
        return Err(format!("{extra} is in it and not listed, so not signed"));
    }

    // Every preset drawable; no two answering to one name.
    let mut stems: BTreeMap<String, &str> = BTreeMap::new();
    for path in manifest.files.keys() {
        let bytes = &files[path];
        let file = path.strip_prefix(PRESETS).unwrap_or(path);
        crate::milk::read(file, &String::from_utf8_lossy(bytes))
            .map_err(|why| format!("{path} cannot be drawn: {why}"))?;
        if stems.insert(stem(path).to_owned(), path).is_some() {
            return Err(format!("two presets are called {:?}", stem(path)));
        }
    }

    // Every look draws only what djmanzo draws, from the pack's own presets.
    if manifest.looks.is_empty() {
        return Err("it has no looks".to_owned());
    }
    let mut names: Vec<&str> = Vec::new();
    for look in &manifest.looks {
        short(&look.name, "look's name")?;
        if names.contains(&look.name.as_str()) {
            return Err(format!("two of its looks are called {:?}", look.name));
        }
        names.push(&look.name);
        if look.description.chars().count() > LONG {
            return Err(format!("{}'s description is too long", look.name));
        }
        if look.layers.is_empty() {
            return Err(format!("{} draws nothing", look.name));
        }
        for layer in &look.layers {
            let Layer {
                kind,
                signal,
                colour,
                amount,
                blend,
                preset,
            } = layer;
            if !KINDS.contains(&kind.as_str()) {
                return Err(format!(
                    "{} draws a {kind:?} layer, which this djmanzo does not draw",
                    look.name
                ));
            }
            if !SIGNALS.contains(&signal.as_str()) || !BLENDS.contains(&blend.as_str()) {
                return Err(format!(
                    "{} has a layer this djmanzo cannot draw",
                    look.name
                ));
            }
            if !colour_is_drawn(colour) || !(0.0..=1.0).contains(amount) {
                return Err(format!("{} has a layer out of range", look.name));
            }
            match (kind.as_str(), preset) {
                ("milkdrop", Some(preset)) if stems.contains_key(preset) => {}
                ("milkdrop", Some(preset)) => {
                    return Err(format!(
                        "{} draws the preset {preset:?}, which is not in the pack",
                        look.name
                    ));
                }
                ("milkdrop", None) => {
                    return Err(format!(
                        "{} has a MilkDrop layer naming no preset",
                        look.name
                    ));
                }
                (_, Some(_)) => {
                    return Err(format!(
                        "{} names a preset on a layer that draws none",
                        look.name
                    ));
                }
                (_, None) => {}
            }
        }
    }
    Ok(Checked {
        manifest,
        key,
        files,
    })
}

/// The look a pack's look is offered as: called after its pack, its
/// presets named where djmanzo finds them.
#[must_use]
pub fn offered(id: &str, pack: &str, look: &Look) -> Look {
    Look {
        name: format!("{pack}: {}", look.name),
        description: look.description.clone(),
        layers: look
            .layers
            .iter()
            .map(|layer| Layer {
                preset: layer
                    .preset
                    .as_ref()
                    .map(|preset| format!("pack/{id}/{preset}")),
                ..layer.clone()
            })
            .collect(),
    }
}

/// A pack's files as kept in its folder: the manifest, the signature, and
/// each file it lists — read only by the names it lists, never by a name
/// joined to the folder that it did not.
///
/// # Errors
/// A file missing or unreadable.
pub fn read_folder(folder: &Path) -> Result<Files, String> {
    let mut files = Files::new();
    let manifest = std::fs::read(folder.join(MANIFEST)).map_err(|e| format!("{MANIFEST}: {e}"))?;
    let listed: Manifest = serde_json::from_slice(&manifest)
        .map_err(|e| format!("its {MANIFEST} could not be read: {e}"))?;
    files.insert(
        SIGNATURE.to_owned(),
        std::fs::read(folder.join(SIGNATURE)).map_err(|e| format!("{SIGNATURE}: {e}"))?,
    );
    for path in listed.files.keys() {
        if !is_preset_path(path) {
            return Err(format!("it lists {path:?}"));
        }
        let bytes = std::fs::read(folder.join(path)).map_err(|e| format!("{path}: {e}"))?;
        files.insert(path.clone(), bytes);
    }
    files.insert(MANIFEST.to_owned(), manifest);
    Ok(files)
}

/// Write a checked pack into `folder`, as it came.
///
/// # Errors
/// The file system's own sentence.
pub fn write_folder(folder: &Path, pack: &Checked) -> Result<(), String> {
    for (path, bytes) in &pack.files {
        let target = folder.join(path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&target, bytes).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// What the DJ is shown of a checked pack.
#[must_use]
pub fn inspected(pack: &Checked, installed: Option<String>) -> Inspected {
    let manifest = &pack.manifest;
    Inspected {
        name: manifest.name.clone(),
        author: manifest.author.clone(),
        licence: manifest.licence.clone(),
        version: manifest.version.clone(),
        description: manifest.description.clone(),
        looks: manifest
            .looks
            .iter()
            .map(|look| look.name.clone())
            .collect(),
        presets: manifest.files.len(),
        key: pack.fingerprint(),
        installed,
        problem: None,
    }
}

/// Sign `manifest` with the PKCS#8 Ed25519 key `pkcs8`: the bytes of
/// [`SIGNATURE`]. Step 7b makes packs with it; the tests make theirs.
///
/// # Errors
/// The key is not an Ed25519 key.
pub fn sign(manifest: &[u8], pkcs8: &[u8]) -> Result<Vec<u8>, String> {
    let pair = ring::signature::Ed25519KeyPair::from_pkcs8(pkcs8)
        .map_err(|_| "that is not an Ed25519 key".to_owned())?;
    Ok(base64::engine::general_purpose::STANDARD
        .encode(pair.sign(manifest).as_ref())
        .into_bytes())
}

/// The public half of the PKCS#8 Ed25519 key `pkcs8`, base64.
///
/// # Errors
/// The key is not an Ed25519 key.
pub fn public_key(pkcs8: &[u8]) -> Result<String, String> {
    use ring::signature::KeyPair as _;
    let pair = ring::signature::Ed25519KeyPair::from_pkcs8(pkcs8)
        .map_err(|_| "that is not an Ed25519 key".to_owned())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(pair.public_key().as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::looks::{Looks, Screen};
    use std::io::Write as _;

    /// A new Ed25519 key, as PKCS#8.
    fn key() -> Vec<u8> {
        ring::signature::Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new())
            .expect("a key")
            .as_ref()
            .to_vec()
    }

    /// A preset djmanzo can draw: one it ships.
    fn drawable() -> Vec<u8> {
        crate::looks::PRESETS[0].1.as_bytes().to_vec()
    }

    fn look(name: &str, preset: Option<&str>) -> Look {
        Look {
            name: name.to_owned(),
            description: format!("{name}, for testing."),
            layers: vec![Layer {
                kind: if preset.is_some() { "milkdrop" } else { "wash" }.to_owned(),
                signal: "loudness".to_owned(),
                colour: "record".to_owned(),
                amount: 0.9,
                blend: "over".to_owned(),
                preset: preset.map(str::to_owned),
            }],
        }
    }

    /// A manifest for `files`, signed by `pkcs8`, with its looks.
    fn manifest(
        name: &str,
        version: &str,
        pkcs8: &[u8],
        files: &Files,
        looks: Vec<Look>,
    ) -> Manifest {
        Manifest {
            format: FORMAT,
            name: name.to_owned(),
            author: "A Maker".to_owned(),
            licence: "CC BY 4.0".to_owned(),
            version: version.to_owned(),
            description: "Neon colours for late nights.".to_owned(),
            key: public_key(pkcs8).expect("a public key"),
            files: files
                .iter()
                .map(|(path, bytes)| (path.clone(), sha256(bytes)))
                .collect(),
            looks,
        }
    }

    /// The pack's files: its presets, its manifest and its signature.
    fn signed(manifest: &Manifest, presets: &Files, pkcs8: &[u8]) -> Files {
        let bytes = serde_json::to_vec_pretty(manifest).expect("json");
        let mut files = presets.clone();
        files.insert(SIGNATURE.to_owned(), sign(&bytes, pkcs8).expect("signed"));
        files.insert(MANIFEST.to_owned(), bytes);
        files
    }

    /// The files as a `.djpack` in `dir`.
    fn archive(dir: &Path, name: &str, files: &Files) -> std::path::PathBuf {
        let path = dir.join(name);
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).expect("a file"));
        for (name, bytes) in files {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .expect("an entry");
            zip.write_all(bytes).expect("written");
        }
        zip.finish().expect("finished");
        path
    }

    fn neon(pkcs8: &[u8], version: &str) -> Files {
        let presets: Files = [("milkdrop/neon.json".to_owned(), drawable())].into();
        let manifest = manifest(
            "Neon Nights",
            version,
            pkcs8,
            &presets,
            vec![look("Neon", Some("neon")), look("Dim", None)],
        );
        signed(&manifest, &presets, pkcs8)
    }

    /// **A signed pack is installed whole, its licence shown first.** Its
    /// looks are offered after it, its preset drawn from its own files,
    /// and it is the same when djmanzo starts again.
    #[test]
    fn a_signed_pack_is_shown_installed_and_offered() {
        let home = tempfile::tempdir().unwrap();
        let looks = Looks::new();
        looks.start(home.path());
        let author = key();
        let path = archive(home.path(), "neon.djpack", &neon(&author, "1.0.0"));

        let seen = looks.inspect_pack(&path);
        assert_eq!(seen.problem, None);
        assert_eq!(seen.name, "Neon Nights");
        assert_eq!(seen.licence, "CC BY 4.0");
        assert_eq!(seen.looks, ["Neon", "Dim"]);
        assert_eq!(seen.presets, 1);
        assert_eq!(seen.key.len(), 19, "four groups of four: {}", seen.key);
        assert_eq!(seen.installed, None);
        assert!(looks.packs().is_empty(), "inspecting installed something");

        let installed = looks.install_pack(&path).unwrap();
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].version, "1.0.0");
        assert_eq!(installed[0].problem, None);
        let id = installed[0].id.clone();
        assert!(id.starts_with("neon-nights-"), "{id}");

        let catalogue = looks.catalogue();
        let offered = catalogue
            .iter()
            .find(|look| look.name == "Neon Nights: Neon")
            .expect("its look offered");
        let preset = offered.layers[0].preset.clone().unwrap();
        assert_eq!(preset, format!("pack/{id}/neon"));
        assert!(looks.preset(&preset).is_ok(), "its preset is not drawn");
        assert!(catalogue.iter().any(|look| look.name == "Neon Nights: Dim"));

        let again = Looks::new();
        again.start(home.path());
        assert_eq!(again.packs(), installed);
        assert!(again.preset(&preset).is_ok());
        assert_eq!(
            again.inspect_pack(&path).installed.as_deref(),
            Some("1.0.0")
        );
    }

    /// **Nothing unchecked is installed**: each way a pack can be other than
    /// its maker signed it is refused, with its reason.
    #[test]
    fn a_pack_that_is_not_as_signed_is_refused_with_why() {
        let author = key();
        let good = neon(&author, "1.0.0");
        assert!(check(good.clone()).is_ok());
        let refused = |files: Files| check(files).expect_err("refused");

        // The manifest changed after signing: a licence edited, say.
        let mut edited = good.clone();
        let text = String::from_utf8(edited[MANIFEST].clone())
            .unwrap()
            .replace("CC BY 4.0", "CC0");
        edited.insert(MANIFEST.to_owned(), text.into_bytes());
        assert!(refused(edited).contains("signature does not match"));

        // Signed by someone else's key, its own put in the manifest.
        let mut forged = good.clone();
        let other = key();
        forged.insert(SIGNATURE.to_owned(), sign(&good[MANIFEST], &other).unwrap());
        assert!(refused(forged).contains("signature does not match"));

        // Unsigned.
        let mut unsigned = good.clone();
        unsigned.remove(SIGNATURE);
        assert!(refused(unsigned).contains("not signed"));

        // A preset swapped after signing.
        let mut swapped = good.clone();
        let mut bytes = drawable();
        bytes.extend_from_slice(b" ");
        swapped.insert("milkdrop/neon.json".to_owned(), bytes);
        assert!(refused(swapped).contains("not the file that was signed"));

        // A file slipped in, unlisted.
        let mut extra = good.clone();
        extra.insert("milkdrop/extra.json".to_owned(), drawable());
        assert!(refused(extra).contains("not listed"));

        // A listed file missing.
        let mut missing = good.clone();
        missing.remove("milkdrop/neon.json");
        assert!(refused(missing).contains("listed and not in it"));

        // A preset with only JavaScript, signed honestly.
        let script: Files = [(
            "milkdrop/neon.json".to_owned(),
            br#"{"baseVals":{},"init_eqs_str":"a.zoom = 1;","frame_eqs_str":"","pixel_eqs_str":"","shapes":[],"waves":[]}"#.to_vec(),
        )]
        .into();
        let m = manifest(
            "Script",
            "1",
            &author,
            &script,
            vec![look("S", Some("neon"))],
        );
        assert!(refused(signed(&m, &script, &author)).contains("cannot be drawn"));

        // A look drawing what djmanzo does not draw.
        let presets: Files = [("milkdrop/neon.json".to_owned(), drawable())].into();
        let mut laser = look("Laser", None);
        laser.layers[0].kind = "laser".to_owned();
        let m = manifest("Lasers", "1", &author, &presets, vec![laser]);
        assert!(refused(signed(&m, &presets, &author)).contains("does not draw"));

        // A look drawing a preset the pack does not hold.
        let m = manifest(
            "Elsewhere",
            "1",
            &author,
            &presets,
            vec![look("E", Some("gone"))],
        );
        assert!(refused(signed(&m, &presets, &author)).contains("not in the pack"));

        // A path that is not a preset's.
        let odd: Files = [("scripts/run.sh".to_owned(), b"#!/bin/sh".to_vec())].into();
        let m = manifest("Odd", "1", &author, &odd, vec![look("O", None)]);
        assert!(refused(signed(&m, &odd, &author)).contains("holds only presets"));

        // A newer format.
        let mut newer = manifest(
            "Newer",
            "1",
            &author,
            &presets,
            vec![look("N", Some("neon"))],
        );
        newer.format = FORMAT + 1;
        assert!(refused(signed(&newer, &presets, &author)).contains("newer djmanzo"));
    }

    #[test]
    fn an_archive_is_read_within_its_limits_and_never_outside_itself() {
        let dir = tempfile::tempdir().unwrap();
        let not_zip = dir.path().join("x.djpack");
        std::fs::write(&not_zip, b"hello").unwrap();
        assert!(read_archive(&not_zip).unwrap_err().contains("not a zip"));

        let climbing: Files = [("../../outside.json".to_owned(), drawable())].into();
        let path = archive(dir.path(), "climb.djpack", &climbing);
        assert!(
            read_archive(&path)
                .unwrap_err()
                .contains("outside the pack")
        );

        let big: Files = [(
            "milkdrop/big.json".to_owned(),
            vec![b' '; usize::try_from(MOST_FILE).unwrap() + 1],
        )]
        .into();
        let path = archive(dir.path(), "big.djpack", &big);
        assert!(read_archive(&path).unwrap_err().contains("larger than"));
    }

    /// **A pack's name keeps its key.** Its maker's next version replaces
    /// it; a pack of the same name signed by another key is refused, and
    /// says whose is installed.
    #[test]
    fn a_name_keeps_its_key() {
        let home = tempfile::tempdir().unwrap();
        let looks = Looks::new();
        looks.start(home.path());
        let author = key();
        looks
            .install_pack(&archive(home.path(), "a.djpack", &neon(&author, "1.0.0")))
            .unwrap();
        let update = archive(home.path(), "b.djpack", &neon(&author, "1.1.0"));
        assert_eq!(
            looks.inspect_pack(&update).installed.as_deref(),
            Some("1.0.0")
        );
        let now = looks.install_pack(&update).unwrap();
        assert_eq!(now.len(), 1);
        assert_eq!(now[0].version, "1.1.0");

        let impostor = archive(home.path(), "c.djpack", &neon(&key(), "9.9.9"));
        let seen = looks.inspect_pack(&impostor);
        assert!(
            seen.problem.as_deref().unwrap().contains("another key"),
            "{seen:?}"
        );
        assert!(looks.install_pack(&impostor).is_err());
        assert_eq!(looks.packs()[0].version, "1.1.0");
    }

    /// A pack changed on disk after it was installed is listed with why, and
    /// its looks are not offered; removing a pack clears a screen showing
    /// one of its looks.
    #[test]
    fn a_pack_changed_since_is_not_offered_and_removing_clears_its_screen() {
        let home = tempfile::tempdir().unwrap();
        let looks = Looks::new();
        looks.start(home.path());
        let installed = looks
            .install_pack(&archive(home.path(), "a.djpack", &neon(&key(), "1.0.0")))
            .unwrap();
        let id = installed[0].id.clone();
        looks
            .choose(Screen::Singers, Some("Neon Nights: Neon".to_owned()))
            .unwrap();

        // An install cut off part-way leaves its incoming folder: not a pack.
        let cut_off = home.path().join("packs").join(format!(".incoming-{id}"));
        std::fs::create_dir_all(cut_off.join("milkdrop")).unwrap();
        std::fs::write(cut_off.join("pack.json"), b"{}").unwrap();

        let preset = home
            .path()
            .join("packs")
            .join(&id)
            .join("milkdrop/neon.json");
        let mut bytes = std::fs::read(&preset).unwrap();
        bytes.push(b'\n');
        std::fs::write(&preset, bytes).unwrap();
        let later = Looks::new();
        later.start(home.path());
        assert_eq!(later.packs().len(), 1, "{:?}", later.packs());
        assert!(
            later.packs()[0]
                .problem
                .as_deref()
                .unwrap()
                .contains("changed")
        );
        assert!(
            !later
                .catalogue()
                .iter()
                .any(|look| look.name.starts_with("Neon Nights"))
        );
        assert_eq!(
            later.chosen().singers,
            None,
            "a look no longer offered is forgotten"
        );
        // Drawn before the restart, it is checked again when drawn.
        assert!(
            looks
                .preset(&format!("pack/{id}/neon"))
                .unwrap_err()
                .contains("changed")
        );

        assert!(looks.remove_pack(&id).unwrap().is_empty());
        assert_eq!(looks.chosen().singers, None);
        assert!(!home.path().join("packs").join(&id).exists());
    }

    /// No name from the interface reaches outside the packs folder.
    #[test]
    fn no_pack_name_reaches_outside_the_folder() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join("packs")).unwrap();
        let outside = home.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        let looks = Looks::new();
        looks.start(home.path());
        assert!(looks.remove_pack("../outside").is_err());
        assert!(outside.exists());
        assert!(looks.preset("pack/../outside/x").is_err());
        assert!(!is_id("../outside"));
        assert!(!is_id("a/b"));
        assert!(is_id("neon-nights-0a1b2c3d"));
    }
}
