//! §128: every source djmanzo can use, by what it is for.
//!
//! > add a cogwheel in the navbar and create a configuration
//! > dashboard/screen/view that allows for connecting sources for song
//! > search, song playing, buying, lyrics search, similar song search,
//! > artist background and imagery ... all useful stuff
//!
//! Settings already describes each streaming source, its keys and its
//! honest paragraph (`dj_sources::catalog`, [`crate::sources`]). What a host
//! asks before a night is the other way round — *where do the words come
//! from? where do I buy this? what finds me a song like that one?* — so this
//! is one list of rows, each a source and **one job** it does, built from
//! the tables that already exist rather than a second description of them:
//! the catalog and its live status, the stores (`dj_sources::stores`), and
//! the fixed sources the karaoke features read (a record's own words,
//! LRCLIB, the Cover Art Archive, djmanzo's own analysis). A source that does
//! two jobs is two rows, so each job's heading lists everything that can do
//! it. Nothing here calls a service; it says what each one needs.

use crate::state::AppState;
use dj_secrets::SecretKind;
use serde::Serialize;

/// What a source is used for, in the order the board shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Job {
    /// Finding a song by its name, its artist or its words.
    Search,
    /// Putting it on a deck.
    Play,
    /// Buying a song the collection does not have.
    Buy,
    /// The words for the singers' screen.
    Lyrics,
    /// Songs like this one.
    Similar,
    /// Covers, and who the artist is.
    Imagery,
}

impl Job {
    pub const ALL: [Job; 6] = [
        Job::Search,
        Job::Play,
        Job::Buy,
        Job::Lyrics,
        Job::Similar,
        Job::Imagery,
    ];

    /// The heading a host reads.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Job::Search => "Finding songs",
            Job::Play => "Playing them",
            Job::Buy => "Buying them",
            Job::Lyrics => "Lyrics",
            Job::Similar => "Songs like this one",
            Job::Imagery => "Covers and artists",
        }
    }
}

/// Whether a row works now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Ready {
    /// Works now, nothing to set up.
    Ready,
    /// Works once something is set up — a key, an account, a model.
    NeedsSetup,
    /// Needs a commercial agreement djmanzo does not hold; no setting helps.
    PartnerGated,
    /// Switched off by the DJ.
    Off,
}

/// One source doing one job.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    pub job: Job,
    pub name: String,
    /// What it does for that job, one line.
    pub what: String,
    pub ready: Ready,
    /// What to do, or why not, in a sentence.
    pub detail: String,
    /// Where it is set up: `sources` (Settings' sources), `assistant`
    /// (Settings' AI), or `None` when there is nothing to set.
    pub setup: Option<&'static str>,
}

/// The board: each job's title, and its rows.
#[derive(Debug, Clone, Serialize)]
pub struct Board {
    pub jobs: Vec<JobDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobDto {
    pub job: Job,
    pub title: &'static str,
    pub rows: Vec<Row>,
}

/// A streaming source, as the catalog and its live status have it.
#[derive(Debug, Clone)]
pub struct Streaming {
    pub label: String,
    pub summary: String,
    pub can_search: bool,
    /// Its audio can be put on a deck (directly, or from the DJ's own file).
    pub can_play: bool,
    pub ready: Ready,
    pub detail: String,
}

/// The rows, from what is known: the streaming sources' state and whether an
/// AI provider is set up. Pure, so the board's shape is tested without a
/// keychain.
#[must_use]
pub fn rows(streaming: &[Streaming], ai_ready: bool) -> Vec<Row> {
    let mut rows = Vec::new();
    let fixed = |job, name: &str, what: &str, detail: &str| Row {
        job,
        name: name.to_owned(),
        what: what.to_owned(),
        ready: Ready::Ready,
        detail: detail.to_owned(),
        setup: None,
    };
    let assistant = |job, what: &str| Row {
        job,
        name: "Your AI assistant".to_owned(),
        what: what.to_owned(),
        ready: if ai_ready {
            Ready::Ready
        } else {
            Ready::NeedsSetup
        },
        detail: if ai_ready {
            "Set up".to_owned()
        } else {
            "Needs a provider's key, or a model on this machine".to_owned()
        },
        setup: Some("assistant"),
    };

    for source in streaming {
        for (job, does) in [
            (Job::Search, source.can_search),
            (Job::Play, source.can_play),
        ] {
            if does {
                rows.push(Row {
                    job,
                    name: source.label.clone(),
                    what: source.summary.clone(),
                    ready: source.ready,
                    detail: source.detail.clone(),
                    setup: (source.ready != Ready::Ready).then_some("sources"),
                });
            }
        }
    }
    rows.push(fixed(
        Job::Search,
        "Lyrics in your collection",
        "A line of the words finds the record",
        "Works for every record whose words djmanzo has fetched",
    ));
    rows.push(assistant(
        Job::Search,
        "A description — \"that bachata with the piano\" — turned into names",
    ));
    for store in &dj_sources::stores::STORES {
        rows.push(Row {
            job: Job::Buy,
            name: store.name.to_owned(),
            what: store.sells.to_owned(),
            ready: Ready::Ready,
            detail: "Opens the store; buying is yours".to_owned(),
            setup: None,
        });
    }
    rows.push(fixed(
        Job::Lyrics,
        "The record's own words",
        "Lyrics in the file's tags, or a .lrc beside it — read first",
        "Nothing to set up; works without a network",
    ));
    rows.push(fixed(
        Job::Lyrics,
        "LRCLIB",
        "An open database of timed lyrics, asked for the whole collection",
        "No key; needs the network once, then kept",
    ));
    rows.push(fixed(
        Job::Similar,
        "djmanzo's Next rail",
        "Ranked by key, tempo, energy and what has worked before",
        "Your own analysis; nothing to set up",
    ));
    rows.push(assistant(
        Job::Similar,
        "Songs like a song, an artist, a genre or a night, by name",
    ));
    rows.push(fixed(
        Job::Imagery,
        "The record's own cover",
        "The picture in the file's tags",
        "Nothing to set up",
    ));
    rows.push(fixed(
        Job::Imagery,
        "Cover Art Archive",
        "The release's front cover, for records tagged with it (MusicBrainz)",
        "No key; kept for nights without a network",
    ));
    rows.push(assistant(Job::Imagery, "Who the artist is, in a few lines"));
    rows
}

/// Grouped under their jobs, in [`Job::ALL`]'s order.
#[must_use]
pub fn board(rows: Vec<Row>) -> Board {
    Board {
        jobs: Job::ALL
            .iter()
            .map(|&job| JobDto {
                job,
                title: job.title(),
                rows: rows.iter().filter(|row| row.job == job).cloned().collect(),
            })
            .collect(),
    }
}

/// The AI providers djmanzo can be given a key for.
const AI_KEYS: [SecretKind; 5] = [
    SecretKind::OpenRouter,
    SecretKind::Anthropic,
    SecretKind::OpenAi,
    SecretKind::GoogleAi,
    SecretKind::Groq,
];

/// Whether any AI provider has a key: what the assistant's rows say.
#[must_use]
pub fn ai_ready(secrets: &dyn dj_secrets::SecretStore) -> bool {
    AI_KEYS.iter().any(|kind| secrets.get(*kind).is_ok())
}

/// Every source, by what it is for.
#[tauri::command]
pub fn sources_board(state: tauri::State<'_, AppState>) -> Board {
    let streaming: Vec<Streaming> = crate::sources::list_sources(state.clone())
        .into_iter()
        .map(|source| Streaming {
            label: source.label.to_owned(),
            summary: source.summary.to_owned(),
            can_search: source.can_search,
            can_play: source.audio != "none",
            ready: match source.status {
                "ready" => Ready::Ready,
                "partner_gated" => Ready::PartnerGated,
                "disabled" => Ready::Off,
                _ => Ready::NeedsSetup,
            },
            detail: source.status_detail,
        })
        .collect();
    board(rows(&streaming, ai_ready(state.secrets().as_ref())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn streaming(label: &str, search: bool, play: bool, ready: Ready) -> Streaming {
        Streaming {
            label: label.to_owned(),
            summary: format!("{label}'s summary"),
            can_search: search,
            can_play: play,
            ready,
            detail: "detail".to_owned(),
        }
    }

    /// **Every job has somewhere to go, and a source is listed under each
    /// job it does and no other.** A source that searches and plays is under
    /// both; one that only searches is not offered for playing; every store
    /// is under buying; and each of the six jobs has at least one row that
    /// works with nothing set up.
    #[test]
    fn each_job_lists_what_does_it_and_has_something_that_works_now() {
        let board = board(rows(
            &[
                streaming("Local library", true, true, Ready::Ready),
                streaming("Spotify", true, false, Ready::NeedsSetup),
                streaming("Beatport", true, true, Ready::PartnerGated),
            ],
            false,
        ));
        assert_eq!(
            board.jobs.iter().map(|job| job.job).collect::<Vec<_>>(),
            Job::ALL
        );
        let names = |job: Job| -> Vec<String> {
            board
                .jobs
                .iter()
                .find(|j| j.job == job)
                .unwrap()
                .rows
                .iter()
                .map(|row| row.name.clone())
                .collect()
        };
        assert!(names(Job::Search).contains(&"Spotify".to_owned()));
        assert!(
            !names(Job::Play).contains(&"Spotify".to_owned()),
            "Spotify plays nothing"
        );
        assert!(names(Job::Play).contains(&"Beatport".to_owned()));
        assert_eq!(
            names(Job::Buy).len(),
            dj_sources::stores::STORES.len(),
            "every store, and only stores"
        );
        for job in &board.jobs {
            assert!(
                job.rows.iter().any(|row| row.ready == Ready::Ready),
                "{} has nothing that works now",
                job.title
            );
        }
    }

    /// **The assistant's rows say whether it is set up, and where to do
    /// it**; a fixed source has nothing to set; a streaming source that is
    /// not ready points at the sources' settings, one that is ready points
    /// nowhere.
    #[test]
    fn a_row_says_what_it_needs_and_where() {
        use dj_secrets::SecretStore as _;
        let unset = rows(
            &[streaming("Spotify", true, false, Ready::NeedsSetup)],
            false,
        );
        let set = rows(
            &[streaming("Local library", true, true, Ready::Ready)],
            true,
        );
        let assistant = |rows: &[Row]| {
            rows.iter()
                .filter(|row| row.name == "Your AI assistant")
                .map(|row| (row.ready, row.setup))
                .collect::<Vec<_>>()
        };
        assert_eq!(assistant(&unset).len(), 3, "search, similar, imagery");
        assert!(
            assistant(&unset)
                .iter()
                .all(|&(ready, setup)| ready == Ready::NeedsSetup && setup == Some("assistant"))
        );
        assert!(
            assistant(&set)
                .iter()
                .all(|&(ready, _)| ready == Ready::Ready)
        );
        let spotify = unset.iter().find(|row| row.name == "Spotify").unwrap();
        assert_eq!(spotify.setup, Some("sources"));
        let local = set.iter().find(|row| row.name == "Local library").unwrap();
        assert_eq!(local.setup, None);
        // Any provider's key makes the assistant ready; a Spotify key does not.
        let secrets = dj_secrets::MemoryStore::new();
        assert!(!ai_ready(&secrets));
        secrets
            .set(
                SecretKind::SpotifyClientId,
                &dj_secrets::Secret::new("abcd"),
            )
            .unwrap();
        assert!(!ai_ready(&secrets), "Spotify is not an AI provider");
        secrets
            .set(SecretKind::Groq, &dj_secrets::Secret::new("abcd"))
            .unwrap();
        assert!(ai_ready(&secrets));
        let lrclib = unset.iter().find(|row| row.name == "LRCLIB").unwrap();
        assert_eq!((lrclib.ready, lrclib.setup), (Ready::Ready, None));
    }
}
