//! Where to buy a record djmanzo does not have — §111.
//!
//! > investigate if there's legal ways to use some kind of streaming or
//! > downloading service for music to live stem/out the vocals for karaoke ...
//! > download/buy mp3s and manipulate them legally ... integrate with some
//! > kind of music stores if useful ... maybe just as direct links?
//!
//! What the research found (sources in `docs/RESEARCH.md`):
//!
//! - **Streaming you may stem live exists only through a partnership.**
//!   Beatport allows stems on streamed tracks inside the DJ applications it
//!   has agreements with; TIDAL only with its paid *DJ Extension*; Spotify and
//!   Apple Music allow no stems anywhere. None of them offers a public
//!   streaming API. That is [`crate::partner`]'s "ready, waiting on an
//!   agreement", and nothing here pretends otherwise.
//! - **A bought download is a file you own**, and separating it — taking the
//!   voice out, `dj_dsp::karaoke` or the stems — is yours to do. Playing it
//!   to a room is a different right: a venue's performance licence, and for
//!   a karaoke night the licence to show lyrics on a screen. djmanzo says so
//!   beside the links rather than burying it.
//!
//! So the integration is the honest one: a link to the store's own search for
//! the song, opened in the DJ's browser, where they buy it on the store's
//! terms. The file lands in their downloads folder, and `dj_app::downloads`
//! files it into the collection. djmanzo never sees a payment or a password.
//!
//! **Only search addresses that were checked go in as searches.** A store
//! whose search address could not be confirmed is linked at the page that
//! was, and says "search there" — a guessed address is a DJ looking at an
//! error page mid-request.

/// How a store is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Link {
    /// The store's own search, with `{query}` where the words go.
    Search(&'static str),
    /// A page on the store, where the DJ searches for themselves.
    Page(&'static str),
}

/// One store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Store {
    /// The word a command names it by.
    pub slug: &'static str,
    pub name: &'static str,
    /// What it sells, in a DJ's words.
    pub sells: &'static str,
    pub link: Link,
    /// Whether it is where a karaoke host looks first: licensed backing
    /// tracks, or a cappellas and stems a host may use.
    pub karaoke: bool,
}

/// Every store djmanzo links to, in the order a DJ would try them.
pub const STORES: [Store; 7] = [
    Store {
        slug: "bandcamp",
        name: "Bandcamp",
        sells: "downloads straight from artists and labels, MP3 to FLAC and WAV",
        link: Link::Search("https://bandcamp.com/search?q={query}"),
        karaoke: false,
    },
    Store {
        slug: "beatport",
        name: "Beatport",
        sells: "DJ downloads in MP3, WAV and AIFF; Beatsource's catalogue is part of it now",
        link: Link::Search("https://www.beatport.com/search?q={query}"),
        karaoke: false,
    },
    Store {
        slug: "traxsource",
        name: "Traxsource",
        sells: "house and soulful DJ downloads, MP3 and lossless",
        link: Link::Search("https://www.traxsource.com/search?term={query}"),
        karaoke: false,
    },
    Store {
        slug: "amazon",
        name: "Amazon Music",
        sells: "MP3 downloads of most commercial releases",
        link: Link::Search("https://www.amazon.com/s?k={query}&i=digital-music"),
        karaoke: false,
    },
    Store {
        slug: "apple",
        name: "Apple (iTunes Store)",
        sells: "downloads bought in Apple's Music app; the web search finds the song",
        link: Link::Search("https://music.apple.com/search?term={query}"),
        karaoke: false,
    },
    Store {
        slug: "karaoke-version",
        name: "Karaoke Version",
        sells: "re-recorded backing tracks with the master rights licensed; search there",
        link: Link::Page("https://www.karaoke-version.com/"),
        karaoke: true,
    },
    Store {
        slug: "ccmixter",
        name: "ccMixter",
        sells: "a cappellas and stems under Creative Commons, free; mind each one's licence",
        link: Link::Page("https://ccmixter.org/media/docs/pellbrowser"),
        karaoke: true,
    },
];

/// What a DJ is told beside the links. One sentence, always shown.
pub const WHAT_BUYING_COVERS: &str = "A bought download is yours to play and to take the voice out of; playing it to a room needs the venue's performance licence, and lyrics on a screen at a karaoke night need the venue's licence for those too.";

impl Store {
    /// The address for a song, built here and never taken from elsewhere.
    #[must_use]
    pub fn url_for(&self, artist: &str, title: &str) -> String {
        match self.link {
            Link::Page(page) => page.to_owned(),
            Link::Search(template) => {
                let words = [artist.trim(), title.trim()]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                template.replace("{query}", &urlencoding::encode(&words))
            }
        }
    }

    /// Whether the link searches for the song, or only reaches the store.
    #[must_use]
    pub const fn searches(&self) -> bool {
        matches!(self.link, Link::Search(_))
    }

    /// A store by its word.
    #[must_use]
    pub fn by_slug(slug: &str) -> Option<&'static Store> {
        STORES.iter().find(|store| store.slug == slug)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The song's words reach the store's search, encoded** — spaces,
    /// accents and an ampersand in a title included, so a title cannot add a
    /// parameter of its own to the address.
    #[test]
    fn a_song_becomes_a_search_on_the_store() {
        let bandcamp = Store::by_slug("bandcamp").unwrap();
        assert_eq!(
            bandcamp.url_for("Juan Luis Guerra", "Bachata Rosa"),
            "https://bandcamp.com/search?q=Juan%20Luis%20Guerra%20Bachata%20Rosa"
        );
        let traxsource = Store::by_slug("traxsource").unwrap();
        let url = traxsource.url_for("Kerri Chandler", "Rain & Sun?");
        assert_eq!(
            url,
            "https://www.traxsource.com/search?term=Kerri%20Chandler%20Rain%20%26%20Sun%3F"
        );
        let amazon = Store::by_slug("amazon").unwrap();
        assert!(
            amazon
                .url_for("", "Corazón")
                .starts_with("https://www.amazon.com/s?k=Coraz%C3%B3n&i=digital-music")
        );
    }

    /// Every address is https on the store's own domain, every slug is its
    /// own, and a store whose search was not confirmed is a page, not a
    /// guessed search.
    #[test]
    fn every_store_is_a_safe_known_address() {
        let mut slugs = std::collections::BTreeSet::new();
        for store in &STORES {
            assert!(slugs.insert(store.slug), "{} twice", store.slug);
            let url = store.url_for("An Artist", "A Song");
            assert!(url.starts_with("https://"), "{url}");
            assert!(!url.contains(' '), "{url}");
            assert!(store.sells.len() > 20, "{} says too little", store.slug);
        }
        assert!(!Store::by_slug("karaoke-version").unwrap().searches());
        assert!(Store::by_slug("nowhere").is_none());
    }

    /// A karaoke host is pointed first at what a karaoke night may use.
    #[test]
    fn the_karaoke_stores_are_marked() {
        let karaoke: Vec<&str> = STORES
            .iter()
            .filter(|s| s.karaoke)
            .map(|s| s.slug)
            .collect();
        assert_eq!(karaoke, vec!["karaoke-version", "ccmixter"]);
    }
}
