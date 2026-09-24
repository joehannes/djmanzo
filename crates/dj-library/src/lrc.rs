//! Timed lyrics, read: LRC and its word-level extension.
//!
//! §107's singers' screen puts the words up in time with the record, and the
//! words it has are what LRCLIB returns in `syncedLyrics` — the LRC format:
//! a timestamp in square brackets at the start of each line,
//! `[01:02.50]and the line`. Two extensions are common enough to read:
//!
//! - **Several timestamps on one line**, `[00:40.00][01:40.00]the chorus`,
//!   where a repeated line is written once.
//! - **Word timings** ("Enhanced LRC", also called A2): an angle-bracketed
//!   stamp before each word, `[00:12.00]<00:12.00>so <00:12.40>long`. A line
//!   with them can be wiped word by word; a line without is wiped evenly.
//!
//! And one tag: `[offset:+250]`, milliseconds, positive meaning the words come
//! *sooner*. Every other `[tag:value]` — artist, title, album, who made the
//! file — is metadata about the lyric and is skipped.
//!
//! Lenient where a file is merely untidy and strict where it would be wrong:
//! a line without a timestamp is not timed and is dropped rather than guessed
//! at, and a timestamp that does not parse drops its line. A singer reading a
//! line at the wrong moment is worse than a gap.

/// One word and when it is sung, in seconds from the start of the record.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub at: f64,
    pub text: String,
}

/// One line and when it starts, in seconds from the start of the record.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub at: f64,
    /// The line as it is sung, word stamps removed. Empty for a marked gap —
    /// an instrumental stretch, which LRC writes as a timestamp with nothing
    /// after it.
    pub text: String,
    /// When each word is sung, where the file says. Empty where it does not,
    /// and then the line is wiped evenly.
    pub words: Vec<Word>,
}

/// Read an LRC text into lines, in the order they are sung.
#[must_use]
pub fn parse(text: &str) -> Vec<Line> {
    let offset = text.lines().find_map(offset_of).unwrap_or(0.0);
    let mut lines = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut stamps = Vec::new();
        while let Some(after) = rest.strip_prefix('[') {
            let Some(end) = after.find(']') else {
                break;
            };
            match timestamp(&after[..end]) {
                Some(at) => stamps.push(at),
                // A tag (`[ar:...]`), or a broken stamp: either way not a
                // time, and a line whose only brackets are not times is not
                // a sung line.
                None => {
                    stamps.clear();
                    break;
                }
            }
            rest = after[end + 1..].trim_start();
        }
        if stamps.is_empty() {
            continue;
        }
        let (text, words) = words_of(rest, offset);
        for at in stamps {
            lines.push(Line {
                at: (at - offset).max(0.0),
                text: text.clone(),
                words: words.clone(),
            });
        }
    }
    lines.sort_by(|a, b| a.at.total_cmp(&b.at));
    lines
}

/// `[offset:+250]` as seconds, or `None` for any other line.
fn offset_of(line: &str) -> Option<f64> {
    let inner = line.trim().strip_prefix('[')?.strip_suffix(']')?;
    let (tag, value) = inner.split_once(':')?;
    if !tag.trim().eq_ignore_ascii_case("offset") {
        return None;
    }
    let milliseconds: f64 = value.trim().trim_start_matches('+').parse().ok()?;
    milliseconds.is_finite().then_some(milliseconds / 1000.0)
}

/// `mm:ss`, `mm:ss.x`, `mm:ss.xx` or `mm:ss.xxx` as seconds.
fn timestamp(stamp: &str) -> Option<f64> {
    let (minutes, seconds) = stamp.split_once(':')?;
    if minutes.is_empty() || !minutes.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (whole, fraction) = seconds.split_once('.').unwrap_or((seconds, ""));
    if whole.len() != 2
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
        || fraction.len() > 3
    {
        return None;
    }
    let minutes: f64 = minutes.parse().ok()?;
    let whole: f64 = whole.parse().ok()?;
    if whole >= 60.0 {
        return None;
    }
    let fraction = if fraction.is_empty() {
        0.0
    } else {
        fraction.parse::<f64>().ok()? / 10f64.powi(fraction.len() as i32)
    };
    Some(minutes * 60.0 + whole + fraction)
}

/// The line's text with any word stamps taken out, and the words they time.
fn words_of(line: &str, offset: f64) -> (String, Vec<Word>) {
    if !line.contains('<') {
        return (line.trim().to_owned(), Vec::new());
    }
    let mut words = Vec::new();
    let mut plain = String::new();
    let mut rest = line;
    while let Some(open) = rest.find('<') {
        plain.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('>') else {
            plain.push_str(&rest[open..]);
            rest = "";
            break;
        };
        let stamp = timestamp(&after[..close]);
        let next = &after[close + 1..];
        let word_end = next.find('<').unwrap_or(next.len());
        let word = &next[..word_end];
        if let Some(at) = stamp
            && !word.trim().is_empty()
        {
            words.push(Word {
                at: (at - offset).max(0.0),
                text: word.trim().to_owned(),
            });
        }
        plain.push_str(word);
        rest = &next[word_end..];
    }
    plain.push_str(rest);
    let plain = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    (plain, words)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The plain case: each line at its time, in order, tags skipped.**
    #[test]
    fn lines_come_at_their_times() {
        let lines = parse(
            "[ar:Somebody]\n[ti:A Song]\n[00:12.50]First line\n[00:15.00]Second line\n[01:02.345]Third",
        );
        let at: Vec<f64> = lines.iter().map(|line| line.at).collect();
        assert_eq!(at, vec![12.5, 15.0, 62.345]);
        assert_eq!(lines[0].text, "First line");
        assert!(lines.iter().all(|line| line.words.is_empty()));
    }

    /// A chorus written once with every time it is sung is every one of
    /// those lines, in the record's order rather than the file's.
    #[test]
    fn a_line_with_several_times_is_sung_at_each() {
        let lines = parse("[00:40.00][01:40.00]The chorus\n[01:00.00]The verse");
        let said: Vec<(f64, &str)> = lines.iter().map(|l| (l.at, l.text.as_str())).collect();
        assert_eq!(
            said,
            vec![
                (40.0, "The chorus"),
                (60.0, "The verse"),
                (100.0, "The chorus")
            ]
        );
    }

    /// **Word timings, where the file has them**: the text is the words, and
    /// each word keeps its own time.
    #[test]
    fn enhanced_lines_time_each_word() {
        let lines = parse("[00:12.00]<00:12.00>So <00:12.40>long <00:13.10>my friend");
        assert_eq!(lines[0].text, "So long my friend");
        let words: Vec<(f64, &str)> = lines[0]
            .words
            .iter()
            .map(|w| (w.at, w.text.as_str()))
            .collect();
        assert_eq!(
            words,
            vec![(12.0, "So"), (12.4, "long"), (13.1, "my friend")]
        );
    }

    /// `[offset:+500]` brings every line and word half a second sooner.
    #[test]
    fn the_offset_moves_everything() {
        let lines = parse("[offset:+500]\n[00:10.00]<00:10.00>Hey <00:11.00>you");
        assert_eq!(lines[0].at, 9.5);
        assert_eq!(lines[0].words[1].at, 10.5);
        let later = parse("[offset:-250]\n[00:10.00]Hey");
        assert_eq!(later[0].at, 10.25);
    }

    /// An empty timed line is a gap — an instrumental — and kept as one; a
    /// line with no time, or a time that does not parse, is dropped.
    #[test]
    fn gaps_are_kept_and_untimed_lines_are_not() {
        let lines = parse(
            "[00:05.00]Sung\n[00:09.00]\nno stamp here\n[0x:10.00]broken\n[00:61.00]too many seconds",
        );
        let said: Vec<(f64, &str)> = lines.iter().map(|l| (l.at, l.text.as_str())).collect();
        assert_eq!(said, vec![(5.0, "Sung"), (9.0, "")]);
    }

    /// Nothing timed at all — plain words, or nothing — is no lines, not a
    /// panic and not a guess.
    #[test]
    fn untimed_text_is_no_lines() {
        assert!(parse("Just words\nwith no times").is_empty());
        assert!(parse("").is_empty());
    }
}
