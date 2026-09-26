//! Finding a record from a query with a typo in it.
//!
//! §124: *"it might be some fuzzy search, whatever suits best"*.
//!
//! The collection's own search ([`crate::Library::search`]) is a prefix match
//! on every word, which is what makes it feel instant while typing — "bach
//! ros" finds "Bachata Rosa" after four keystrokes. It finds nothing for
//! "bachta" or "guera", and a DJ mid-set types fast and does not stop to
//! correct. This is the second chance: every word of the query within a typo
//! or two of a word of the record's title, artist, album or genre.
//!
//! **How close is close.** A word of up to two letters must be exact — "dj"
//! a typo away is half the collection. Three to five letters may be one edit
//! off, six or more two. An *edit* is a letter added, dropped, changed, or two
//! neighbours swapped ("bahcata"), which is how hands actually miss. A query
//! word may also be the start of a record's word with a typo in it, since
//! the DJ is usually still typing: "bacht" is one edit from "bach", the start
//! of "bachata".
//!
//! **Accents fold away.** "ojala" finds "Ojalá", as the prefix search's
//! tokenizer already does, so the two searches agree on what a letter is.
//!
//! Arithmetic, not a model and not a dependency: a bounded edit distance over
//! words that are rarely longer than a dozen letters.

/// The words of `text`, lower-cased and with accents folded away.
#[must_use]
pub fn words(text: &str) -> Vec<Vec<char>> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| {
            word.chars()
                .flat_map(char::to_lowercase)
                .map(fold)
                .collect()
        })
        .collect()
}

/// A letter without its accent, for the Latin letters a collection's tags
/// carry. Anything else is left as it is.
#[must_use]
pub fn fold(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'ā' => 'a',
        'é' | 'è' | 'ê' | 'ë' | 'ē' => 'e',
        'í' | 'ì' | 'î' | 'ï' | 'ī' => 'i',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ø' | 'ō' => 'o',
        'ú' | 'ù' | 'û' | 'ü' | 'ū' => 'u',
        'ñ' => 'n',
        'ç' => 'c',
        'ý' | 'ÿ' => 'y',
        'š' => 's',
        'ž' => 'z',
        'č' => 'c',
        other => other,
    }
}

/// How many edits a query word of `len` letters may be from what it finds.
#[must_use]
pub const fn allowed(len: usize) -> usize {
    match len {
        0..=2 => 0,
        3..=5 => 1,
        _ => 2,
    }
}

/// Edits between `a` and `b` — insertions, deletions, substitutions and
/// swaps of neighbours (the optimal string alignment distance) — or `None`
/// once it is certainly more than `most`.
#[must_use]
pub fn edits(a: &[char], b: &[char], most: usize) -> Option<usize> {
    if a.len().abs_diff(b.len()) > most {
        return None;
    }
    let width = b.len() + 1;
    // Three rows: the one before last is needed for a swap.
    let mut before: Vec<usize> = vec![0; width];
    let mut last: Vec<usize> = (0..width).collect();
    let mut row: Vec<usize> = vec![0; width];
    for i in 1..=a.len() {
        row[0] = i;
        let mut least = row[0];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (last[j] + 1).min(row[j - 1] + 1).min(last[j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(before[j - 2] + 1);
            }
            row[j] = best;
            least = least.min(best);
        }
        if least > most {
            return None;
        }
        std::mem::swap(&mut before, &mut last);
        std::mem::swap(&mut last, &mut row);
    }
    let distance = last[b.len()];
    (distance <= most).then_some(distance)
}

/// How far one query word is from one word of a record: nothing when the
/// record's word starts with it, otherwise the fewer edits to the whole word
/// or to its start. `None` when it is further than the word is allowed.
#[must_use]
pub fn word_distance(query: &[char], word: &[char]) -> Option<usize> {
    if word.starts_with(query) {
        return Some(0);
    }
    let most = allowed(query.len());
    if most == 0 {
        return None;
    }
    let whole = edits(query, word, most);
    // The start of the word, one letter either side of the query's length,
    // for a word still being typed.
    let start = (query.len().saturating_sub(1)..=query.len() + 1)
        .filter(|&len| len > 0 && len < word.len())
        .filter_map(|len| edits(query, &word[..len], most))
        .min();
    match (whole, start) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// How far a whole query is from a record's words: the sum over the query's
/// words of each one's nearest, or `None` when any word finds nothing close
/// enough. Lower is closer; 0 is what the prefix search would have found.
#[must_use]
pub fn closeness(query: &[Vec<char>], record: &[Vec<char>]) -> Option<usize> {
    if query.is_empty() {
        return None;
    }
    query.iter().try_fold(0, |sum, wanted| {
        record
            .iter()
            .filter_map(|word| word_distance(wanted, word))
            .min()
            .map(|nearest| sum + nearest)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(text: &str) -> Vec<Vec<char>> {
        words(text)
    }

    fn close(query: &str, record: &str) -> Option<usize> {
        closeness(&w(query), &w(record))
    }

    /// **The load-bearing one.** The misses a hand makes mid-set find the
    /// record; a different record is not found for them.
    #[test]
    fn a_query_with_a_typo_finds_the_record_and_nothing_else() {
        let record = "Bachata Rosa Juan Luis Guerra";
        assert_eq!(close("bachata", record), Some(0), "exact");
        assert_eq!(close("bach ros", record), Some(0), "still typing");
        assert_eq!(close("bachta", record), Some(1), "a letter dropped");
        assert_eq!(close("bahcata", record), Some(1), "two letters swapped");
        assert_eq!(close("bachatta", record), Some(1), "a letter doubled");
        assert_eq!(
            close("guera rosa", record),
            Some(1),
            "a typo in one of two words"
        );
        assert_eq!(
            close("bacht", record),
            Some(1),
            "a letter dropped, still typing"
        );
        // Not a near miss: every word must find something.
        assert_eq!(close("bachata merengue", record), None);
        assert_eq!(close("salsa", record), None);
        // Short words are exact, or "dj" a typo away is half the collection.
        assert_eq!(close("xy", "xa"), None);
        assert_eq!(close("xa", "xa"), Some(0));
    }

    #[test]
    fn accents_fold_away_on_either_side() {
        assert_eq!(close("ojala", "Ojalá Que Llueva Café"), Some(0));
        assert_eq!(close("cafe", "Ojalá Que Llueva Café"), Some(0));
        assert_eq!(close("Ojalá", "Ojala"), Some(0));
        assert_eq!(close("pina", "Piña Colada"), Some(0));
    }

    #[test]
    fn longer_words_may_be_further_off() {
        assert_eq!(allowed(2), 0);
        assert_eq!(allowed(4), 1);
        assert_eq!(allowed(8), 2);
        assert_eq!(close("merengeu", "Merengue"), Some(1));
        assert_eq!(
            close("mrnge", "Merengue"),
            None,
            "two edits in five letters"
        );
        assert_eq!(close("mereenguee", "Merengue"), Some(2));
    }

    #[test]
    fn the_distance_is_the_one_a_hand_makes() {
        let e = |a: &str, b: &str| {
            edits(
                &a.chars().collect::<Vec<_>>(),
                &b.chars().collect::<Vec<_>>(),
                5,
            )
        };
        assert_eq!(e("kitten", "sitting"), Some(3));
        assert_eq!(e("ab", "ba"), Some(1), "a swap is one edit");
        assert_eq!(e("", "abc"), Some(3));
        assert_eq!(e("same", "same"), Some(0));
        // Bounded: past `most` it stops and says so.
        assert_eq!(
            edits(
                &"abcdef".chars().collect::<Vec<_>>(),
                &"uvwxyz".chars().collect::<Vec<_>>(),
                2
            ),
            None
        );
    }
}
