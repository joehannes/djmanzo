//! The filter language smart folders are written in.
//!
//! ```text
//! bpm > 120 and bpm < 130
//! key compatible 8A and genre contains bachata
//! artist = "Juan Luis Guerra" or (year >= 1990 and rating >= 4)
//! not genre contains reggaeton
//! ```
//!
//! # Why a language and not stored SQL
//!
//! A smart folder needs a condition that outlives the session, which means
//! storing it. Storing SQL and executing it would mean the database file is an
//! executable script: anything that can write a playlist row — an importer, a
//! synced library, a corrupted file — could then run arbitrary statements
//! against a DJ's collection. So this parses into a typed tree and *compiles*
//! to a parameterised `WHERE` clause. Every value the user typed leaves as a
//! bound parameter; nothing they type is ever concatenated into SQL.
//!
//! # Why it is this small
//!
//! Every field here is one a DJ sorts or filters by in front of a crowd. The
//! grammar is what fits in a single line of a text box, because that is where
//! it is typed. There is no date arithmetic and no arbitrary nesting of fields
//! — those are report-writing features, and a DJ building a crate is not
//! writing a report.
//!
//! This used to say "no subqueries" as well, and `for` broke that. The rule
//! it was reaching for is still intact and worth restating precisely: **the
//! *grammar* has no subqueries.** A DJ types one flat term and the compiler
//! decides how to answer it. `for is opener` is a subquery underneath because a
//! function is a row in another table rather than a column on `tracks`, and
//! hiding that from the person typing is the point of having a compiler at
//! all. What is still refused is a grammar in which the *user* nests one
//! query inside another.

use dj_core::{Mode, MusicalKey};
use std::fmt;

/// A parsed filter.
#[derive(Debug, Clone, PartialEq)]
pub enum Filter {
    All(Vec<Filter>),
    Any(Vec<Filter>),
    Not(Box<Filter>),
    Compare { field: Field, op: Op, value: Value },
}

/// What a condition is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Bpm,
    Key,
    Year,
    Rating,
    Plays,
    Loudness,
    Title,
    Artist,
    Album,
    Genre,
    Label,
    Comment,
    /// What a record is *for* -- see [`crate::functions`]. Not a column: a row
    /// in `track_functions`, which is why it does not go through the generic
    /// path below.
    Function,
}

impl Field {
    fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "bpm" | "tempo" => Self::Bpm,
            "key" => Self::Key,
            "year" => Self::Year,
            "rating" | "stars" => Self::Rating,
            "plays" | "played" => Self::Plays,
            "loudness" | "lufs" => Self::Loudness,
            "title" | "name" => Self::Title,
            "artist" => Self::Artist,
            "album" => Self::Album,
            "genre" | "style" => Self::Genre,
            "label" => Self::Label,
            "comment" | "comments" => Self::Comment,
            // `for` first because it is what a DJ says out loud -- "what is
            // this for" -- and `function` because it is what the vocabulary is
            // called everywhere else.
            "for" | "function" => Self::Function,
            _ => return None,
        })
    }

    /// The column this reads, for the fields that are one column.
    fn column(self) -> &'static str {
        match self {
            Self::Bpm => "tracks.bpm",
            Self::Year => "tracks.year",
            Self::Rating => "tracks.rating",
            Self::Plays => "tracks.play_count",
            Self::Loudness => "tracks.loudness_lufs",
            Self::Title => "tracks.title",
            Self::Artist => "tracks.artist",
            Self::Album => "tracks.album",
            Self::Genre => "tracks.genre",
            Self::Label => "tracks.label",
            Self::Comment => "tracks.comment",
            // Two columns; never reached, because `Key` is compiled specially.
            Self::Key => "tracks.key_hour",
            // No column at all -- a row in `track_functions`. Never reached,
            // for the same reason `Key` is not.
            Self::Function => "tracks.id",
        }
    }

    fn is_text(self) -> bool {
        matches!(
            self,
            Self::Title
                | Self::Artist
                | Self::Album
                | Self::Genre
                | Self::Label
                | Self::Comment
                // A function is typed as a word and compared as one, so the
                // parser reads its value the same way. What it *compiles* to
                // is a subquery -- see `function_sql`.
                | Self::Function
        )
    }
}

/// How the field is compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    Contains,
    StartsWith,
    EndsWith,
    /// Harmonic neighbours: the same key, its relative major or minor, and one
    /// step either way round the Camelot wheel. The reason a DJ filters by key
    /// at all.
    Compatible,
    // The negated forms. They exist as operators rather than as a `NOT` wrapper
    // so that negation can be pushed down to the leaves -- see
    // [`Filter::negate`]. Nothing parses to these directly.
    NotContains,
    NotStartsWith,
    NotEndsWith,
    NotCompatible,
}

impl Op {
    /// The comparison that is true exactly when this one is not.
    #[must_use]
    pub const fn negate(self) -> Self {
        match self {
            Self::Eq => Self::Ne,
            Self::Ne => Self::Eq,
            Self::Gt => Self::Le,
            Self::Ge => Self::Lt,
            Self::Lt => Self::Ge,
            Self::Le => Self::Gt,
            Self::Contains => Self::NotContains,
            Self::NotContains => Self::Contains,
            Self::StartsWith => Self::NotStartsWith,
            Self::NotStartsWith => Self::StartsWith,
            Self::EndsWith => Self::NotEndsWith,
            Self::NotEndsWith => Self::EndsWith,
            Self::Compatible => Self::NotCompatible,
            Self::NotCompatible => Self::Compatible,
        }
    }
}

/// The right-hand side.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    Text(String),
    Key(MusicalKey),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterError {
    Empty,
    /// A word where a field name was expected.
    UnknownField(String),
    UnknownOperator(String),
    /// `bpm contains 4`, or `genre > 3`.
    WrongKindOfComparison {
        field: &'static str,
        op: &'static str,
    },
    ExpectedValue,
    ExpectedNumber(String),
    ExpectedKey(String),
    UnclosedParen,
    UnclosedQuote,
    Trailing(String),
}

impl fmt::Display for FilterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "the filter is empty"),
            Self::UnknownField(word) => write!(
                f,
                "{word:?} is not something to filter on. Try bpm, key, artist, \
                 title, album, genre, label, year, rating, plays or for."
            ),
            Self::UnknownOperator(word) => write!(f, "{word:?} is not a comparison"),
            Self::WrongKindOfComparison { field, op } => {
                write!(f, "{field} cannot be compared with {op}")
            }
            Self::ExpectedValue => write!(f, "the comparison has nothing to compare against"),
            Self::ExpectedNumber(word) => write!(f, "{word:?} is not a number"),
            Self::ExpectedKey(word) => {
                write!(
                    f,
                    "{word:?} is not a key. Camelot notation, like 8A or 11B."
                )
            }
            Self::UnclosedParen => write!(f, "a bracket is not closed"),
            Self::UnclosedQuote => write!(f, "a quote is not closed"),
            Self::Trailing(word) => write!(f, "unexpected {word:?} at the end"),
        }
    }
}

impl std::error::Error for FilterError {}

/// Parse a filter.
pub fn parse(input: &str) -> Result<Filter, FilterError> {
    let tokens = tokenise(input)?;
    if tokens.is_empty() {
        return Err(FilterError::Empty);
    }
    let mut parser = Parser { tokens, at: 0 };
    let filter = parser.parse_any()?;
    if let Some(token) = parser.peek() {
        return Err(FilterError::Trailing(token.text.clone()));
    }
    Ok(filter)
}

/// A `WHERE` clause and the parameters it binds.
#[derive(Debug, Clone, PartialEq)]
pub struct Compiled {
    pub sql: String,
    pub params: Vec<Param>,
}

/// One bound value. Never interpolated into the SQL.
#[derive(Debug, Clone, PartialEq)]
pub enum Param {
    Number(f64),
    Text(String),
    Integer(i64),
}

impl Filter {
    /// Compile to a `WHERE` clause with bound parameters.
    ///
    /// `next` numbers the parameters, so the caller can compile a filter into a
    /// larger query that already binds some.
    #[must_use]
    pub fn compile(&self) -> Compiled {
        let mut params = Vec::new();
        let sql = self.write_sql(&mut params);
        Compiled { sql, params }
    }

    /// The filter that matches exactly what this one does not.
    ///
    /// # Why negation is not `NOT (...)`
    ///
    /// A comparison on an analysed field only matches tracks that *have* that
    /// field: `bpm > 120` cannot be true of a track nobody has analysed. But
    /// wrapping that in SQL's `NOT` inverts the guard along with the
    /// comparison, so `not bpm > 120` would match every unanalysed track — and
    /// a DJ filtering for slow tracks would find a 150 BPM record in their
    /// warm-up crate, which is the worst possible time to discover it.
    ///
    /// Pushing the negation down to the comparison instead keeps the guard
    /// where it belongs: `not bpm > 120` compiles as `bpm <= 120`, still
    /// requiring a tempo to exist.
    ///
    /// Text is deliberately the other way round, and it is not an
    /// inconsistency. An absent *tag* is a value — a track with no genre
    /// genuinely is not reggaeton, and `not genre contains reggaeton` should
    /// find it. An absent *analysis* is an unknown, and a filter must not
    /// assert anything about it. The text columns read through `COALESCE`, so
    /// null behaves as empty in both directions.
    #[must_use]
    pub fn negate(&self) -> Filter {
        match self {
            // De Morgan, so the leaves are what carry the negation.
            Self::All(parts) => Self::Any(parts.iter().map(Filter::negate).collect()),
            Self::Any(parts) => Self::All(parts.iter().map(Filter::negate).collect()),
            Self::Not(inner) => (**inner).clone(),
            Self::Compare { field, op, value } => Self::Compare {
                field: *field,
                op: op.negate(),
                value: value.clone(),
            },
        }
    }

    fn write_sql(&self, params: &mut Vec<Param>) -> String {
        match self {
            // An empty group is the identity for its operator. `all of nothing`
            // is true and `any of nothing` is false -- which is what makes
            // `not (...)` on an empty group behave.
            Self::All(parts) if parts.is_empty() => "1".to_owned(),
            Self::Any(parts) if parts.is_empty() => "0".to_owned(),
            Self::All(parts) => join(parts, " AND ", params),
            Self::Any(parts) => join(parts, " OR ", params),
            // Negation is pushed down to the comparisons rather than wrapped
            // around the SQL. See `negate` for why that is not just a tidiness
            // preference.
            Self::Not(inner) => inner.negate().write_sql(params),
            Self::Compare { field, op, value } => compare_sql(*field, *op, value, params),
        }
    }
}

fn join(parts: &[Filter], separator: &str, params: &mut Vec<Param>) -> String {
    let rendered: Vec<String> = parts.iter().map(|p| p.write_sql(params)).collect();
    format!("({})", rendered.join(separator))
}

fn placeholder(params: &mut Vec<Param>, param: Param) -> String {
    params.push(param);
    format!("?{}", params.len())
}

fn compare_sql(field: Field, op: Op, value: &Value, params: &mut Vec<Param>) -> String {
    // Key is two columns and one of the comparisons is a set, so it does not
    // go through the generic path.
    if field == Field::Key {
        return key_sql(op, value, params);
    }
    // A function is a row in another table, so neither.
    if field == Field::Function {
        return function_sql(op, value, params);
    }

    let column = field.column();
    match (op, value) {
        // Text reads through `COALESCE`, so an untagged track behaves as one
        // with an empty tag rather than as SQL's three-valued unknown. That is
        // what makes `not genre contains reggaeton` find it -- see
        // `Filter::negate`.
        //
        // `ESCAPE` so a title containing % or _ is matched literally rather
        // than as a wildcard the DJ did not type.
        (Op::Contains | Op::NotContains, Value::Text(text)) => {
            like(column, op, format!("%{}%", escape_like(text)), params)
        }
        (Op::StartsWith | Op::NotStartsWith, Value::Text(text)) => {
            like(column, op, format!("{}%", escape_like(text)), params)
        }
        (Op::EndsWith | Op::NotEndsWith, Value::Text(text)) => {
            like(column, op, format!("%{}", escape_like(text)), params)
        }
        (_, Value::Text(text)) => {
            let bound = placeholder(params, Param::Text(text.clone()));
            // Case-insensitive: nobody types an artist's capitalisation the way
            // the tag has it.
            format!(
                "COALESCE({column}, '') {} {bound} COLLATE NOCASE",
                sql_op(op)
            )
        }
        (_, Value::Number(number)) => {
            let bound = placeholder(params, Param::Number(*number));
            // The guard that must survive negation: a filter about a tempo can
            // only be true of a track that has one. `Filter::negate` inverts
            // the operator rather than wrapping this in `NOT`, so the guard
            // stays put.
            format!("({column} IS NOT NULL AND {column} {} {bound})", sql_op(op))
        }
        (_, Value::Key(_)) => "0".to_owned(),
    }
}

fn like(column: &str, op: Op, pattern: String, params: &mut Vec<Param>) -> String {
    let bound = placeholder(params, Param::Text(pattern));
    let negated = matches!(op, Op::NotContains | Op::NotStartsWith | Op::NotEndsWith);
    let keyword = if negated { "NOT LIKE" } else { "LIKE" };
    format!("COALESCE({column}, '') {keyword} {bound} ESCAPE '\\'")
}

/// `for is opener` -- does this track carry that function.
///
/// An `EXISTS` rather than a join, so a track carrying three functions still
/// appears once. A join would multiply rows and `for:opener or for:peak` would
/// list a record twice, which is the sort of thing that looks like a
/// duplicate-detection bug rather than a filter bug.
///
/// **Negation is `NOT EXISTS`, not an inverted comparison**, and that is the
/// right side of the line `Filter::negate` draws. An absent function is a
/// *value*, like an absent genre: a record nobody has tagged genuinely is not
/// an opener, and `not for:opener` should find it. That is the opposite of an
/// absent tempo, which is an unknown a filter must not assert anything about.
///
/// The slug is bound as a parameter and never interpolated, like every other
/// value here. An unknown slug simply matches nothing rather than being
/// refused -- `for is warmup` is a DJ guessing at the vocabulary, and an empty
/// result says "not a thing" more usefully than an error does.
fn function_sql(op: Op, value: &Value, params: &mut Vec<Param>) -> String {
    let Value::Text(slug) = value else {
        return "0".to_owned();
    };
    let bound = placeholder(params, Param::Text(slug.clone()));
    let exists = format!(
        "EXISTS (SELECT 1 FROM track_functions
                 WHERE track_functions.track_id = tracks.id
                   AND track_functions.function = {bound} COLLATE NOCASE)"
    );
    match op {
        // The negated forms `Filter::negate` produces, plus `!=` typed
        // directly. Everything else -- a `>` on a function, say -- is
        // meaningless rather than wrong, and matches nothing.
        Op::Ne | Op::NotContains | Op::NotStartsWith | Op::NotEndsWith | Op::NotCompatible => {
            format!("NOT {exists}")
        }
        Op::Eq | Op::Contains | Op::StartsWith | Op::EndsWith => exists,
        _ => "0".to_owned(),
    }
}

fn key_sql(op: Op, value: &Value, params: &mut Vec<Param>) -> String {
    let Value::Key(key) = value else {
        return "0".to_owned();
    };
    let one = |key: MusicalKey, params: &mut Vec<Param>| {
        let hour = placeholder(params, Param::Integer(i64::from(key.hour())));
        let mode = placeholder(
            params,
            Param::Text(
                match key.mode() {
                    Mode::Minor => "minor",
                    Mode::Major => "major",
                }
                .to_owned(),
            ),
        );
        format!("(tracks.key_hour = {hour} AND tracks.key_mode = {mode})")
    };

    // Every form requires the track to have a key at all, including the
    // negated ones: a track nobody has analysed is not "in a different key",
    // it is unknown.
    let has_key = "tracks.key_hour IS NOT NULL";

    match op {
        Op::Compatible | Op::NotCompatible => {
            // The key itself plus its neighbours. `compatible()` returns the
            // four a DJ can mix into; the track's own key is the fifth and the
            // most obvious, so it goes in too.
            let mut clauses = vec![one(*key, params)];
            clauses.extend(key.compatible().iter().map(|k| one(*k, params)));
            let set = clauses.join(" OR ");
            if op == Op::NotCompatible {
                format!("({has_key} AND NOT ({set}))")
            } else {
                format!("({set})")
            }
        }
        Op::Ne => format!("({has_key} AND NOT {})", one(*key, params)),
        _ => one(*key, params),
    }
}

fn sql_op(op: Op) -> &'static str {
    match op {
        Op::Eq | Op::Compatible => "=",
        Op::Ne | Op::NotCompatible => "!=",
        Op::Gt => ">",
        Op::Ge => ">=",
        Op::Lt => "<",
        Op::Le => "<=",
        // Handled by `like` before this is reached.
        Op::Contains | Op::StartsWith | Op::EndsWith => "LIKE",
        Op::NotContains | Op::NotStartsWith | Op::NotEndsWith => "NOT LIKE",
    }
}

/// Make `%` and `_` literal inside a `LIKE` pattern.
fn escape_like(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

// -- lexer -----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
struct Token {
    text: String,
    /// True when it came from a quoted string, so `and` in a title is a word
    /// rather than an operator.
    quoted: bool,
}

fn tokenise(input: &str) -> Result<Vec<Token>, FilterError> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '"' || c == '\'' {
            let quote = c;
            chars.next();
            let mut text = String::new();
            loop {
                match chars.next() {
                    Some(c) if c == quote => break,
                    Some(c) => text.push(c),
                    None => return Err(FilterError::UnclosedQuote),
                }
            }
            tokens.push(Token { text, quoted: true });
        } else if c == '(' || c == ')' {
            chars.next();
            tokens.push(Token {
                text: c.to_string(),
                quoted: false,
            });
        } else if "<>=!".contains(c) {
            let mut text = String::new();
            while let Some(&c) = chars.peek() {
                if "<>=!".contains(c) {
                    text.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            tokens.push(Token {
                text,
                quoted: false,
            });
        } else {
            let mut text = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() || "()<>=!\"'".contains(c) {
                    break;
                }
                text.push(c);
                chars.next();
            }
            tokens.push(Token {
                text,
                quoted: false,
            });
        }
    }
    Ok(tokens)
}

// -- parser ----------------------------------------------------------------

struct Parser {
    tokens: Vec<Token>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.at).cloned();
        if token.is_some() {
            self.at += 1;
        }
        token
    }

    /// Is the next token this keyword, unquoted?
    fn at_keyword(&self, word: &str) -> bool {
        self.peek()
            .is_some_and(|t| !t.quoted && t.text.eq_ignore_ascii_case(word))
    }

    fn parse_any(&mut self) -> Result<Filter, FilterError> {
        let mut parts = vec![self.parse_all()?];
        while self.at_keyword("or") {
            self.at += 1;
            parts.push(self.parse_all()?);
        }
        Ok(if parts.len() == 1 {
            parts.pop().unwrap_or(Filter::All(Vec::new()))
        } else {
            Filter::Any(parts)
        })
    }

    fn parse_all(&mut self) -> Result<Filter, FilterError> {
        let mut parts = vec![self.parse_term()?];
        // `and` is optional. "bpm > 120 bpm < 130" reads fine and is what
        // somebody types when they are in a hurry, so juxtaposition means the
        // same thing.
        while self.at_keyword("and") || self.starts_a_term() {
            if self.at_keyword("and") {
                self.at += 1;
            }
            parts.push(self.parse_term()?);
        }
        Ok(if parts.len() == 1 {
            parts.pop().unwrap_or(Filter::All(Vec::new()))
        } else {
            Filter::All(parts)
        })
    }

    fn starts_a_term(&self) -> bool {
        match self.peek() {
            None => false,
            Some(token) if token.quoted => false,
            Some(token) => {
                token.text == "("
                    || token.text.eq_ignore_ascii_case("not")
                    || Field::parse(&token.text.to_ascii_lowercase()).is_some()
            }
        }
    }

    fn parse_term(&mut self) -> Result<Filter, FilterError> {
        if self.at_keyword("not") {
            self.at += 1;
            return Ok(Filter::Not(Box::new(self.parse_term()?)));
        }
        if self.peek().is_some_and(|t| !t.quoted && t.text == "(") {
            self.at += 1;
            let inner = self.parse_any()?;
            match self.next() {
                Some(token) if token.text == ")" => return Ok(inner),
                _ => return Err(FilterError::UnclosedParen),
            }
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Filter, FilterError> {
        let name = self.next().ok_or(FilterError::ExpectedValue)?;
        let field = Field::parse(&name.text.to_ascii_lowercase())
            .ok_or_else(|| FilterError::UnknownField(name.text.clone()))?;

        let op_token = self.next().ok_or(FilterError::ExpectedValue)?;
        let op = parse_op(&op_token.text)
            .ok_or_else(|| FilterError::UnknownOperator(op_token.text.clone()))?;

        let value_token = self.next().ok_or(FilterError::ExpectedValue)?;
        let value = self.parse_value(field, op, &value_token)?;
        Ok(Filter::Compare { field, op, value })
    }

    fn parse_value(&self, field: Field, op: Op, token: &Token) -> Result<Value, FilterError> {
        if field == Field::Key {
            if !matches!(op, Op::Eq | Op::Ne | Op::Compatible) {
                return Err(FilterError::WrongKindOfComparison {
                    field: "key",
                    op: op_name(op),
                });
            }
            return parse_camelot(&token.text)
                .map(Value::Key)
                .ok_or_else(|| FilterError::ExpectedKey(token.text.clone()));
        }

        if field.is_text() {
            if matches!(op, Op::Gt | Op::Ge | Op::Lt | Op::Le) {
                return Err(FilterError::WrongKindOfComparison {
                    field: text_field_name(field),
                    op: op_name(op),
                });
            }
            return Ok(Value::Text(token.text.clone()));
        }

        // Numeric field.
        if matches!(op, Op::Contains | Op::StartsWith | Op::EndsWith) {
            return Err(FilterError::WrongKindOfComparison {
                field: numeric_field_name(field),
                op: op_name(op),
            });
        }
        token
            .text
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .map(Value::Number)
            .ok_or_else(|| FilterError::ExpectedNumber(token.text.clone()))
    }
}

fn parse_op(word: &str) -> Option<Op> {
    Some(match word.to_ascii_lowercase().as_str() {
        "=" | "==" | "is" => Op::Eq,
        "!=" | "<>" | "isnt" => Op::Ne,
        ">" => Op::Gt,
        ">=" => Op::Ge,
        "<" => Op::Lt,
        "<=" => Op::Le,
        "contains" | "has" | "~" => Op::Contains,
        "starts" | "startswith" => Op::StartsWith,
        "ends" | "endswith" => Op::EndsWith,
        "compatible" | "harmonic" => Op::Compatible,
        _ => return None,
    })
}

fn op_name(op: Op) -> &'static str {
    match op {
        Op::Eq => "=",
        Op::Ne => "!=",
        Op::Gt => ">",
        Op::Ge => ">=",
        Op::Lt => "<",
        Op::Le => "<=",
        Op::Contains => "contains",
        Op::StartsWith => "starts",
        Op::EndsWith => "ends",
        Op::Compatible => "compatible",
        Op::NotContains => "not contains",
        Op::NotStartsWith => "not starts",
        Op::NotEndsWith => "not ends",
        Op::NotCompatible => "not compatible",
    }
}

fn text_field_name(field: Field) -> &'static str {
    match field {
        Field::Title => "title",
        Field::Artist => "artist",
        Field::Album => "album",
        Field::Genre => "genre",
        Field::Label => "label",
        Field::Function => "for",
        _ => "comment",
    }
}

fn numeric_field_name(field: Field) -> &'static str {
    match field {
        Field::Bpm => "bpm",
        Field::Year => "year",
        Field::Rating => "rating",
        Field::Plays => "plays",
        _ => "loudness",
    }
}

/// Camelot notation: an hour 1..=12 and a ring, `A` for minor, `B` for major.
fn parse_camelot(word: &str) -> Option<MusicalKey> {
    let word = word.trim();
    let (digits, ring) = word.split_at(word.len().checked_sub(1)?);
    let hour: u8 = digits.parse().ok()?;
    let mode = match ring.to_ascii_uppercase().as_str() {
        "A" => Mode::Minor,
        "B" => Mode::Major,
        _ => return None,
    };
    MusicalKey::new(hour, mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(hour: u8, mode: Mode) -> MusicalKey {
        MusicalKey::new(hour, mode).unwrap()
    }

    #[test]
    fn a_single_comparison_parses() {
        assert_eq!(
            parse("bpm > 120").unwrap(),
            Filter::Compare {
                field: Field::Bpm,
                op: Op::Gt,
                value: Value::Number(120.0),
            }
        );
    }

    #[test]
    fn and_or_and_not_nest_the_way_they_read() {
        let filter = parse("bpm > 120 and genre contains bachata or rating >= 4").unwrap();
        // `and` binds tighter than `or`, as everywhere else.
        let Filter::Any(parts) = filter else {
            panic!("expected an OR at the top, got {filter:?}");
        };
        assert_eq!(parts.len(), 2);
        assert!(matches!(parts[0], Filter::All(_)));
    }

    #[test]
    fn brackets_override_precedence() {
        let filter = parse("(bpm > 120 or rating >= 4) and genre contains latin").unwrap();
        let Filter::All(parts) = filter else {
            panic!("expected an AND at the top, got {filter:?}");
        };
        assert!(matches!(parts[0], Filter::Any(_)));
    }

    /// Somebody in a hurry types conditions without `and` between them.
    #[test]
    fn juxtaposition_means_and() {
        assert_eq!(
            parse("bpm > 120 bpm < 130").unwrap(),
            parse("bpm > 120 and bpm < 130").unwrap()
        );
    }

    #[test]
    fn a_quoted_value_keeps_its_spaces_and_keywords() {
        let filter = parse(r#"artist = "Earth, Wind and Fire""#).unwrap();
        assert_eq!(
            filter,
            Filter::Compare {
                field: Field::Artist,
                op: Op::Eq,
                value: Value::Text("Earth, Wind and Fire".to_owned()),
            },
            "the `and` inside a quoted name is part of the name"
        );
    }

    #[test]
    fn field_and_operator_aliases_work() {
        assert_eq!(parse("tempo > 120").unwrap(), parse("bpm > 120").unwrap());
        assert_eq!(
            parse("genre has latin").unwrap(),
            parse("style contains latin").unwrap()
        );
    }

    #[test]
    fn keys_parse_in_camelot_notation() {
        assert_eq!(
            parse("key = 8A").unwrap(),
            Filter::Compare {
                field: Field::Key,
                op: Op::Eq,
                value: Value::Key(key(8, Mode::Minor)),
            }
        );
        assert_eq!(parse("key = 11b").unwrap(), parse("key = 11B").unwrap());
    }

    // -- what the errors have to catch -------------------------------------

    #[test]
    fn an_unknown_field_says_what_can_be_filtered_on() {
        let error = parse("colour = red").unwrap_err();
        assert_eq!(error, FilterError::UnknownField("colour".to_owned()));
        let message = error.to_string();
        assert!(message.contains("bpm") && message.contains("artist"));
    }

    #[test]
    fn comparing_the_wrong_kinds_is_refused() {
        assert!(matches!(
            parse("bpm contains 12"),
            Err(FilterError::WrongKindOfComparison { .. })
        ));
        assert!(matches!(
            parse("artist > 5"),
            Err(FilterError::WrongKindOfComparison { .. })
        ));
        assert!(matches!(
            parse("key > 8A"),
            Err(FilterError::WrongKindOfComparison { .. })
        ));
    }

    #[test]
    fn a_malformed_filter_is_reported_rather_than_half_understood() {
        assert!(matches!(parse(""), Err(FilterError::Empty)));
        assert!(matches!(parse("   "), Err(FilterError::Empty)));
        assert!(matches!(parse("bpm >"), Err(FilterError::ExpectedValue)));
        assert!(matches!(
            parse("bpm > abc"),
            Err(FilterError::ExpectedNumber(_))
        ));
        assert!(matches!(
            parse("key = 99Z"),
            Err(FilterError::ExpectedKey(_))
        ));
        assert!(matches!(
            parse("(bpm > 120"),
            Err(FilterError::UnclosedParen)
        ));
        assert!(matches!(
            parse("artist = \"open"),
            Err(FilterError::UnclosedQuote)
        ));
        assert!(matches!(
            parse("bpm ? 120"),
            Err(FilterError::UnknownOperator(_))
        ));
    }

    #[test]
    fn a_key_hour_outside_the_wheel_is_not_a_key() {
        assert!(matches!(
            parse("key = 13A"),
            Err(FilterError::ExpectedKey(_))
        ));
        assert!(matches!(
            parse("key = 0A"),
            Err(FilterError::ExpectedKey(_))
        ));
    }

    // -- compilation -------------------------------------------------------

    /// The property the whole design rests on: nothing the user typed appears
    /// in the SQL.
    #[test]
    fn user_text_never_reaches_the_sql() {
        let filter = parse(r#"artist = "Robert'); DROP TABLE tracks;--""#).unwrap();
        let compiled = filter.compile();
        assert!(
            !compiled.sql.contains("DROP"),
            "the value must be bound, not written into the statement: {}",
            compiled.sql
        );
        assert_eq!(
            compiled.params,
            vec![Param::Text("Robert'); DROP TABLE tracks;--".to_owned())]
        );
    }

    #[test]
    fn a_numeric_comparison_binds_its_number() {
        let compiled = parse("bpm > 120").unwrap().compile();
        assert!(compiled.sql.contains("tracks.bpm > ?1"));
        assert_eq!(compiled.params, vec![Param::Number(120.0)]);
    }

    /// A null column must not match, *and* negating the comparison must not
    /// silently drop every unanalysed track -- which is what `NOT (NULL > 120)`
    /// does in SQL, because it is NULL rather than true.
    #[test]
    fn a_missing_value_never_matches_a_number_comparison_either_way() {
        let compiled = parse("bpm > 120").unwrap().compile();
        assert!(compiled.sql.contains("IS NOT NULL"), "got {}", compiled.sql);
    }

    #[test]
    fn contains_becomes_a_wildcarded_like() {
        let compiled = parse("genre contains bachata").unwrap().compile();
        assert!(compiled.sql.contains("LIKE ?1"));
        assert_eq!(compiled.params, vec![Param::Text("%bachata%".to_owned())]);
    }

    /// A percent sign the DJ typed is a percent sign, not "match anything".
    #[test]
    fn like_wildcards_in_the_users_text_are_escaped() {
        let compiled = parse("title contains 100%").unwrap().compile();
        assert_eq!(compiled.params, vec![Param::Text("%100\\%%".to_owned())]);
        assert!(compiled.sql.contains("ESCAPE"));
    }

    #[test]
    fn starts_and_ends_anchor_the_right_end() {
        assert_eq!(
            parse("artist starts Juan").unwrap().compile().params,
            vec![Param::Text("Juan%".to_owned())]
        );
        assert_eq!(
            parse("artist ends Guerra").unwrap().compile().params,
            vec![Param::Text("%Guerra".to_owned())]
        );
    }

    /// The reason a DJ filters by key at all.
    #[test]
    fn compatible_matches_the_key_and_its_harmonic_neighbours() {
        let compiled = parse("key compatible 8A").unwrap().compile();
        // The key itself plus the four `MusicalKey::compatible` returns.
        assert_eq!(
            compiled.params.len(),
            10,
            "five keys, two parameters each: {}",
            compiled.sql
        );
        assert!(compiled.params.contains(&Param::Integer(8)));
        // Its neighbours on the wheel.
        assert!(compiled.params.contains(&Param::Integer(7)));
        assert!(compiled.params.contains(&Param::Integer(9)));
    }

    #[test]
    fn an_exact_key_matches_only_that_key() {
        let compiled = parse("key = 8A").unwrap().compile();
        assert_eq!(
            compiled.params,
            vec![Param::Integer(8), Param::Text("minor".to_owned())]
        );
    }

    #[test]
    fn parameters_are_numbered_in_the_order_they_are_bound() {
        let compiled = parse("bpm > 120 and genre contains latin and year >= 1990")
            .unwrap()
            .compile();
        assert!(compiled.sql.contains("?1"));
        assert!(compiled.sql.contains("?2"));
        assert!(compiled.sql.contains("?3"));
        assert_eq!(compiled.params.len(), 3);
    }

    #[test]
    fn not_becomes_the_opposite_comparison_rather_than_a_wrapper() {
        let compiled = parse("not genre contains reggaeton").unwrap().compile();
        assert!(
            compiled.sql.contains("NOT LIKE"),
            "negation is pushed to the leaf, not wrapped: {}",
            compiled.sql
        );
    }

    /// The guard that must survive negation. Wrapping in SQL's `NOT` would
    /// invert it along with the comparison and put every unanalysed track into
    /// a filter for slow ones.
    #[test]
    fn negating_a_tempo_comparison_still_requires_a_tempo() {
        let compiled = parse("not bpm > 120").unwrap().compile();
        assert!(compiled.sql.contains("IS NOT NULL"), "got {}", compiled.sql);
        assert!(compiled.sql.contains("<="), "got {}", compiled.sql);
        assert!(!compiled.sql.contains("NOT ("), "got {}", compiled.sql);
    }

    #[test]
    fn negation_distributes_over_and_and_or() {
        // not (a and b) is (not a) or (not b).
        assert_eq!(
            parse("not (bpm > 120 and rating >= 4)").unwrap(),
            Filter::Not(Box::new(parse("bpm > 120 and rating >= 4").unwrap()))
        );
        // Once: an AND of two comparisons becomes an OR of their opposites.
        let once = parse("bpm > 120 and rating >= 4").unwrap().negate();
        assert_eq!(
            once,
            Filter::Any(vec![
                parse("bpm <= 120").unwrap(),
                parse("rating < 4").unwrap(),
            ])
        );
        // Twice: back where it started.
        assert_eq!(once.negate(), parse("bpm > 120 and rating >= 4").unwrap());
    }

    #[test]
    fn every_operator_has_an_inverse_that_round_trips() {
        for op in [
            Op::Eq,
            Op::Ne,
            Op::Gt,
            Op::Ge,
            Op::Lt,
            Op::Le,
            Op::Contains,
            Op::StartsWith,
            Op::EndsWith,
            Op::Compatible,
            Op::NotContains,
            Op::NotStartsWith,
            Op::NotEndsWith,
            Op::NotCompatible,
        ] {
            assert_eq!(op.negate().negate(), op, "{op:?} does not round trip");
            assert_ne!(op.negate(), op);
        }
    }

    /// An absent tag is a value. A track with no genre is genuinely not
    /// reggaeton, and a DJ excluding reggaeton wants it.
    #[test]
    fn text_reads_an_absent_tag_as_empty_in_both_directions() {
        for query in ["genre contains latin", "not genre contains latin"] {
            let compiled = parse(query).unwrap().compile();
            assert!(
                compiled.sql.contains("COALESCE"),
                "{query} must treat a missing tag as empty: {}",
                compiled.sql
            );
        }
    }

    /// ...but an absent key is an unknown, not "some other key".
    #[test]
    fn negating_a_key_comparison_still_requires_a_key() {
        for query in ["not key = 8A", "not key compatible 8A"] {
            let compiled = parse(query).unwrap().compile();
            assert!(
                compiled.sql.contains("key_hour IS NOT NULL"),
                "{query}: {}",
                compiled.sql
            );
        }
    }
}
