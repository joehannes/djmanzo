//! K7: the third book — answers to the quiz, from phones.
//!
//! The quiz's questions are the application's (`dj_app::quiz`); what is
//! here is what a phone may do while one is open: give a nickname once a
//! night, and answer each question once. Like the request book and K4's
//! singers' book, a phone can add to it and reach nothing else.
//!
//! # Points
//!
//! A right answer is worth [`WORTH`]: half of it for being right, the other
//! half shrinking to nothing between the question opening and closing. The
//! question closes [`CLOSES_AFTER`] after its clip ends, or when the host
//! shows the answer, whichever is first; an answer after that is refused,
//! not counted late. Points are added when the question is settled, so a
//! phone learns nothing from its total before the answer is shown.

use std::time::{Duration, SystemTime};

/// How long a question stays open after its clip ends.
pub const CLOSES_AFTER: Duration = Duration::from_secs(10);

/// What a right answer given the moment the question opened is worth.
pub const WORTH: u32 = 1000;

/// The longest nickname, in characters: K4's rule for a singer's name.
pub const LONGEST_NICKNAME: usize = crate::stage::LONGEST_NAME;

/// How many the leaderboard shows.
pub const LEADERS: usize = 10;

/// The letters the answers go by, on phones and on the screen.
pub const LETTERS: [&str; 4] = ["A", "B", "C", "D"];

/// One phone's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Answer {
    phone: String,
    letter: usize,
    at: SystemTime,
}

/// The question phones are answering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    /// Which question this is tonight, so a page left open on the last one
    /// cannot answer this one.
    pub id: u64,
    pub answers: [String; 4],
    right: usize,
    /// Counted from one, of how many the round is.
    pub number: usize,
    pub of: usize,
    opened: SystemTime,
    closes: Option<SystemTime>,
    answered: Vec<Answer>,
    settled: bool,
    /// The answer has been shown.
    pub revealed: bool,
}

impl Asked {
    /// The right answer, once it has been shown.
    #[must_use]
    pub fn right(&self) -> Option<usize> {
        self.revealed.then_some(self.right)
    }

    /// Whether answers are still taken at `now`.
    #[must_use]
    pub fn open_at(&self, now: SystemTime) -> bool {
        !self.settled && self.closes.is_none_or(|closes| now <= closes)
    }

    /// How long answers are still taken, once the clip is over; `None`
    /// while it plays, and nothing left once settled.
    #[must_use]
    pub fn closes_in(&self, now: SystemTime) -> Option<Duration> {
        if self.settled {
            return Some(Duration::ZERO);
        }
        self.closes
            .map(|closes| closes.duration_since(now).unwrap_or_default())
    }

    /// How many phones chose each answer.
    #[must_use]
    pub fn counts(&self) -> [usize; 4] {
        let mut counts = [0; 4];
        for answer in &self.answered {
            counts[answer.letter] += 1;
        }
        counts
    }

    /// The letter `phone` chose, if it answered.
    #[must_use]
    pub fn chosen_by(&self, phone: &str) -> Option<usize> {
        self.answered
            .iter()
            .find(|answer| answer.phone == phone)
            .map(|answer| answer.letter)
    }

    /// What an answer at `at` is worth, right, with the question closing at
    /// `closes`: half for being right, half shrinking to nothing.
    fn worth(&self, at: SystemTime, closes: SystemTime) -> u32 {
        let whole = closes.duration_since(self.opened).unwrap_or_default();
        let taken = at.duration_since(self.opened).unwrap_or_default();
        let left = if whole.is_zero() {
            0.0
        } else {
            1.0 - (taken.as_secs_f64() / whole.as_secs_f64()).clamp(0.0, 1.0)
        };
        let half = f64::from(WORTH / 2);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let points = (half + half * left).round() as u32;
        points
    }
}

/// What became of a nickname.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Named {
    Named,
    Empty,
    Long,
    /// Another phone plays by that name tonight.
    Taken,
    /// This phone already has one tonight.
    Already,
}

/// What became of an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answered {
    Taken,
    /// No nickname yet: one first.
    Nameless,
    /// The question closed, or is not the one open.
    Closed,
    /// This phone answered this question already.
    Again,
}

/// The night's quiz, as phones play it.
#[derive(Debug, Clone, Default)]
pub struct QuizBook {
    /// A round is running: the room's page offers the quiz.
    on: bool,
    nicknames: Vec<(String, String)>,
    question: Option<Asked>,
    /// Each phone's points tonight.
    points: Vec<(String, u32)>,
    asked: u64,
}

impl QuizBook {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a round is running.
    #[must_use]
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// The question open, or the last one shown.
    #[must_use]
    pub fn question(&self) -> Option<&Asked> {
        self.question.as_ref()
    }

    /// Ask a question: the last one settled first. Answers are taken from
    /// `now` until its clip has ended and [`CLOSES_AFTER`] more.
    pub fn ask(
        &mut self,
        answers: [String; 4],
        right: usize,
        number: usize,
        of: usize,
        now: SystemTime,
    ) -> u64 {
        self.settle(now);
        self.on = true;
        self.asked += 1;
        self.question = Some(Asked {
            id: self.asked,
            answers,
            right: right.min(3),
            number,
            of,
            opened: now,
            closes: None,
            answered: Vec::new(),
            settled: false,
            revealed: false,
        });
        self.asked
    }

    /// The clip is over: answers are taken for [`CLOSES_AFTER`] more.
    pub fn clip_over(&mut self, now: SystemTime) {
        if let Some(question) = self.question.as_mut() {
            question.closes.get_or_insert(now + CLOSES_AFTER);
        }
    }

    /// The clip played again: answers are taken until it ends again, if the
    /// question has not been settled.
    pub fn again(&mut self) {
        if let Some(question) = self.question.as_mut().filter(|q| !q.settled) {
            question.closes = None;
        }
    }

    /// How many phones play tonight.
    #[must_use]
    pub fn players(&self) -> usize {
        self.nicknames.len()
    }

    /// Give `phone` its nickname for the night.
    pub fn name(&mut self, phone: &str, nickname: &str) -> Named {
        let nickname = nickname.trim();
        if self.nickname(phone).is_some() {
            return Named::Already;
        }
        if nickname.is_empty() {
            return Named::Empty;
        }
        if nickname.chars().count() > LONGEST_NICKNAME {
            return Named::Long;
        }
        if self
            .nicknames
            .iter()
            .any(|(_, held)| held.to_lowercase() == nickname.to_lowercase())
        {
            return Named::Taken;
        }
        self.nicknames.push((phone.to_owned(), nickname.to_owned()));
        Named::Named
    }

    /// The nickname `phone` plays by tonight.
    #[must_use]
    pub fn nickname(&self, phone: &str) -> Option<&str> {
        self.nicknames
            .iter()
            .find(|(held, _)| held == phone)
            .map(|(_, nickname)| nickname.as_str())
    }

    /// `phone` answers question `id` with `letter` (0 to 3).
    pub fn answer(&mut self, phone: &str, id: u64, letter: usize, now: SystemTime) -> Answered {
        if self.nickname(phone).is_none() {
            return Answered::Nameless;
        }
        let Some(question) = self
            .question
            .as_mut()
            .filter(|question| question.id == id && letter < 4)
        else {
            return Answered::Closed;
        };
        if !question.open_at(now) {
            return Answered::Closed;
        }
        if question.chosen_by(phone).is_some() {
            return Answered::Again;
        }
        question.answered.push(Answer {
            phone: phone.to_owned(),
            letter,
            at: now,
        });
        Answered::Taken
    }

    /// Close the open question and add its points, once.
    pub fn settle(&mut self, now: SystemTime) {
        let Some(question) = self.question.as_mut().filter(|q| !q.settled) else {
            return;
        };
        let closes = question.closes.map_or(now, |closes| closes.min(now));
        question.closes = Some(closes);
        question.settled = true;
        let earned: Vec<(String, u32)> = question
            .answered
            .iter()
            .filter(|answer| answer.letter == question.right)
            .map(|answer| (answer.phone.clone(), question.worth(answer.at, closes)))
            .collect();
        for (phone, points) in earned {
            match self.points.iter_mut().find(|(held, _)| *held == phone) {
                Some((_, total)) => *total += points,
                None => self.points.push((phone, points)),
            }
        }
    }

    /// Show the answer: the question closed, its points added.
    pub fn reveal(&mut self, now: SystemTime) {
        self.settle(now);
        if let Some(question) = self.question.as_mut() {
            question.revealed = true;
        }
    }

    /// The round over. The night's points and nicknames are kept.
    pub fn end_round(&mut self, now: SystemTime) {
        self.settle(now);
        self.question = None;
        self.on = false;
    }

    /// A new night: nothing of tonight's is kept.
    pub fn clear(&mut self) {
        *self = Self {
            asked: self.asked,
            ..Self::default()
        };
    }

    /// `phone`'s points tonight.
    #[must_use]
    pub fn points_of(&self, phone: &str) -> u32 {
        self.points
            .iter()
            .find(|(held, _)| held == phone)
            .map_or(0, |(_, points)| *points)
    }

    /// The top [`LEADERS`], by nickname, most points first; a tie in the
    /// order they first scored.
    #[must_use]
    pub fn leaders(&self) -> Vec<(String, u32)> {
        let mut board: Vec<(String, u32)> = self
            .points
            .iter()
            .filter_map(|(phone, points)| Some((self.nickname(phone)?.to_owned(), *points)))
            .collect();
        board.sort_by_key(|entry| std::cmp::Reverse(entry.1));
        board.truncate(LEADERS);
        board
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: f64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000) + Duration::from_secs_f64(seconds)
    }

    fn answers() -> [String; 4] {
        ["Ana", "Bo", "Cy", "Di"].map(str::to_owned)
    }

    fn named(book: &mut QuizBook, phones: &[&str]) {
        for (n, phone) in phones.iter().enumerate() {
            assert_eq!(book.name(phone, &format!("player {n}")), Named::Named);
        }
    }

    /// **A right answer is worth a thousand at once and half at the close;
    /// a wrong one nothing; points arrive only when settled.** The clip
    /// lasts ten seconds and the question closes ten after: twenty in all.
    #[test]
    fn points_are_half_for_right_and_half_for_quick() {
        let mut book = QuizBook::new();
        named(&mut book, &["quick", "slow", "wrong", "late"]);
        let id = book.ask(answers(), 2, 1, 10, at(0.0));
        assert_eq!(book.answer("quick", id, 2, at(0.0)), Answered::Taken);
        assert_eq!(book.answer("wrong", id, 1, at(1.0)), Answered::Taken);
        assert_eq!(
            book.question().unwrap().closes_in(at(5.0)),
            None,
            "while the clip plays"
        );
        book.clip_over(at(10.0));
        assert_eq!(
            book.question().unwrap().closes_in(at(13.0)),
            Some(Duration::from_secs(7))
        );
        assert_eq!(book.answer("slow", id, 2, at(15.0)), Answered::Taken);
        assert_eq!(
            book.points_of("quick"),
            0,
            "points before the answer is shown"
        );
        assert_eq!(book.answer("late", id, 2, at(20.5)), Answered::Closed);
        let mut replayed = book.clone();
        replayed.again();
        assert_eq!(
            replayed.answer("late", id, 2, at(20.5)),
            Answered::Taken,
            "played again, open again"
        );
        book.reveal(at(30.0));
        assert_eq!(book.points_of("quick"), 1000);
        assert_eq!(book.points_of("slow"), 625);
        assert_eq!(book.points_of("wrong"), 0);
        assert_eq!(book.points_of("late"), 0);
        assert_eq!(book.question().unwrap().counts(), [0, 1, 2, 0]);
        assert_eq!(book.question().unwrap().right(), Some(2));
        assert_eq!(
            book.question().unwrap().closes_in(at(30.0)),
            Some(Duration::ZERO)
        );
    }

    /// **Shown early, the question closes then**, and the clock for the
    /// shrinking half is the time it was actually open.
    #[test]
    fn showing_the_answer_closes_the_question() {
        let mut book = QuizBook::new();
        named(&mut book, &["a", "b"]);
        let id = book.ask(answers(), 0, 1, 5, at(0.0));
        assert_eq!(book.answer("a", id, 0, at(2.0)), Answered::Taken);
        book.reveal(at(4.0));
        assert_eq!(book.answer("b", id, 0, at(4.5)), Answered::Closed);
        assert_eq!(book.points_of("a"), 750);
        book.again();
        assert_eq!(
            book.answer("b", id, 0, at(5.0)),
            Answered::Closed,
            "settled stays closed"
        );
        book.reveal(at(9.0));
        assert_eq!(book.points_of("a"), 750, "settled twice");
    }

    /// **One answer a phone a question, a nickname first, and a page left
    /// on the last question cannot answer this one.**
    #[test]
    fn one_answer_each_and_only_to_the_open_question() {
        let mut book = QuizBook::new();
        let first = book.ask(answers(), 0, 1, 5, at(0.0));
        assert_eq!(book.answer("a", first, 0, at(1.0)), Answered::Nameless);
        named(&mut book, &["a"]);
        assert_eq!(book.answer("a", first, 0, at(0.0)), Answered::Taken);
        assert_eq!(book.answer("a", first, 1, at(2.0)), Answered::Again);
        let second = book.ask(answers(), 0, 2, 5, at(30.0));
        assert_eq!(book.answer("a", first, 0, at(31.0)), Answered::Closed);
        assert_eq!(
            book.answer("a", second, 4, at(31.0)),
            Answered::Closed,
            "a fifth letter"
        );
        assert_eq!(book.answer("a", second, 0, at(31.0)), Answered::Taken);
        assert_eq!(
            book.points_of("a"),
            1000,
            "the first settled when the second was asked"
        );
    }

    /// **A nickname once a night, unique, forty characters at most.**
    #[test]
    fn a_nickname_once_a_night() {
        let mut book = QuizBook::new();
        assert_eq!(book.name("a", "  "), Named::Empty);
        assert_eq!(
            book.name("a", &"n".repeat(LONGEST_NICKNAME + 1)),
            Named::Long
        );
        assert_eq!(book.name("a", " Ana "), Named::Named);
        assert_eq!(book.nickname("a"), Some("Ana"));
        assert_eq!(book.name("a", "Other"), Named::Already);
        assert_eq!(book.name("b", "ANA"), Named::Taken);
        book.clear();
        assert_eq!(book.nickname("a"), None);
    }

    /// **The leaderboard is the top ten by nickname, kept across rounds
    /// and forgotten by a new night.**
    #[test]
    fn the_leaderboard_is_the_top_ten() {
        let mut book = QuizBook::new();
        let phones: Vec<String> = (0..12).map(|n| format!("phone {n}")).collect();
        let names: Vec<&str> = phones.iter().map(String::as_str).collect();
        named(&mut book, &names);
        let id = book.ask(answers(), 1, 1, 5, at(0.0));
        book.clip_over(at(0.0));
        for (n, phone) in phones.iter().enumerate() {
            book.answer(phone, id, 1, at(n as f64 * 0.5));
        }
        book.end_round(at(20.0));
        let board = book.leaders();
        assert_eq!(board.len(), LEADERS);
        assert_eq!(board[0], ("player 0".to_owned(), 1000));
        assert!(
            board.windows(2).all(|pair| pair[0].1 >= pair[1].1),
            "{board:?}"
        );
        assert!(!book.is_on() && book.question().is_none());
        let again = book.ask(answers(), 0, 1, 5, at(60.0));
        assert!(again > id, "a new round's question is a new question");
        assert_eq!(book.points_of("phone 0"), 1000, "kept across rounds");
        book.clear();
        assert!(book.leaders().is_empty());
    }
}
