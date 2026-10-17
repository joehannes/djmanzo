//! §34 / §36: the floor's other signals, between the room surface and
//! `dj_assistant::floor`.
//!
//! The room surface measures them from the same camera frames and the same
//! window of sound it already reads light, movement and loudness from, and
//! sends them here as numbers — never a frame, never a sample. This pairs the
//! microphone's loudness with the master's level **at the moment it arrives**,
//! which is the half of *the room over the music* only Rust can supply: the
//! interface has no idea how loud the record it is hearing was sent out.
//!
//! The judgement is all `dj_assistant::floor`'s and is tested there. This is
//! the two commands and the line the assistant is told.

use crate::state::AppState;
use dj_assistant::floor::{Floor, Line, Look, Signal, loudness_of_peak};
use serde::Serialize;
use std::time::SystemTime;
use tauri::State;

/// What one look measured, as the surface sends it. Every field optional: a
/// microphone alone measures no picture.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Deserialize)]
pub struct Measured {
    pub low: Option<f32>,
    pub mid: Option<f32>,
    pub high: Option<f32>,
    pub applause: Option<f32>,
    pub uneven: Option<f32>,
    pub together: Option<f32>,
    pub scene: Option<f32>,
    /// The microphone's loudness, on the room surface's own scale.
    pub heard: Option<f32>,
}

impl Measured {
    /// The look these make. `OverMusic` is never sent: it is worked out from
    /// `heard` and the master in [`Floor::saw`].
    #[must_use]
    pub fn look(&self, at: SystemTime) -> Look {
        [
            (Signal::Low, self.low),
            (Signal::Mid, self.mid),
            (Signal::High, self.high),
            (Signal::Applause, self.applause),
            (Signal::Uneven, self.uneven),
            (Signal::Together, self.together),
            (Signal::Scene, self.scene),
        ]
        .into_iter()
        .fold(Look::at(at), |look, (signal, value)| match value {
            Some(value) => look.with(signal, value),
            None => look,
        })
    }
}

/// The master's level, on the microphone's scale: the louder meter, because a
/// mix loud in one channel is still a loud mix.
#[must_use]
pub fn music_level(snapshot: &crate::Snapshot) -> Option<f32> {
    let peak = snapshot.master.peak_left.max(snapshot.master.peak_right);
    peak.is_finite().then(|| loudness_of_peak(peak))
}

/// Take one look into tonight's floor.
pub fn saw(floor: &mut Floor, measured: &Measured, music: Option<f32>, at: SystemTime) {
    floor.saw(measured.look(at), measured.heard, music);
}

/// §34 / §36: what the room surface measured beyond light, movement and
/// loudness.
#[tauri::command]
pub fn floor_saw(state: State<'_, AppState>, measured: Measured) -> Result<(), String> {
    let music = music_level(&crate::commands::snapshot_now(&state));
    let night = state.night();
    let mut floor = night
        .floor()
        .lock()
        .map_err(|_| "the floor's readings are poisoned")?;
    saw(&mut floor, &measured, music, SystemTime::now());
    Ok(())
}

/// One line, as the interface draws it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LineDto {
    pub signal: String,
    pub title: String,
    pub now: Option<f32>,
    /// `lowest` … `highest`, or `None` before it can be placed.
    pub against: Option<String>,
    pub notable: bool,
    /// `not_yet`, `early` or `settled`.
    pub confidence: String,
    pub looks: usize,
    pub says: String,
    pub measures: String,
}

impl From<Line> for LineDto {
    fn from(line: Line) -> Self {
        Self {
            signal: line.signal.name().to_owned(),
            title: line.signal.title().to_owned(),
            now: line.now,
            against: line.against.map(|a| a.name().to_owned()),
            notable: line.against.is_some_and(|a| a.is_notable()),
            confidence: line.confidence.name().to_owned(),
            looks: line.looks,
            says: line.says,
            measures: line.signal.measures().to_owned(),
        }
    }
}

/// Every signal's line, or none while nothing is looking.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FloorDto {
    pub watching: bool,
    pub lines: Vec<LineDto>,
}

/// Whether a look arrived recently enough to call this live: the room
/// surface's own rule, so the two never disagree about whether anyone is
/// looking.
fn watching(floor: &Floor, now: SystemTime) -> bool {
    floor.last_seen().is_some_and(|at| {
        now.duration_since(at)
            .is_ok_and(|since| since < crate::commands::STILL_WATCHING)
    })
}

/// The floor as it reads now.
#[must_use]
pub fn read(floor: &Floor, now: SystemTime) -> FloorDto {
    let watching = watching(floor, now);
    FloorDto {
        watching,
        lines: if watching {
            floor.lines().into_iter().map(LineDto::from).collect()
        } else {
            Vec::new()
        },
    }
}

/// §34 / §36: every signal on its own line.
#[tauri::command]
pub fn floor_read(state: State<'_, AppState>) -> Result<FloorDto, String> {
    let night = state.night();
    let floor = night
        .floor()
        .lock()
        .map_err(|_| "the floor's readings are poisoned")?;
    Ok(read(&floor, SystemTime::now()))
}

/// What the assistant is told: the notable lines, only while somebody is
/// looking — the room's rule in §40's briefing — and nothing at all otherwise,
/// so a camera switched off an hour ago is not still describing the floor.
#[must_use]
pub fn briefing(state: &AppState) -> Option<String> {
    let night = state.night();
    let notes = {
        let floor = night.floor().lock().ok()?;
        if !watching(&floor, SystemTime::now()) {
            return None;
        }
        floor.notes()
    };
    Some(if notes.is_empty() {
        "the floor's other signals: nothing out of the ordinary tonight".to_owned()
    } else {
        format!("the floor's other signals: {}", notes.join("; "))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// **What the surface sends becomes a look, and the room over the music
    /// is worked out here, never taken from the interface.**
    #[test]
    fn a_measurement_becomes_a_look_with_the_music_paired_in_rust() {
        let measured = Measured {
            low: Some(0.3),
            high: Some(1.4),
            scene: Some(f32::NAN),
            heard: Some(0.6),
            ..Measured::default()
        };
        let look = measured.look(SystemTime::UNIX_EPOCH);
        assert_eq!(look.get(Signal::Low), Some(0.3));
        assert_eq!(look.get(Signal::High), Some(1.0), "clamped on the way in");
        assert_eq!(look.get(Signal::Scene), None, "a broken number is dropped");
        assert_eq!(look.get(Signal::Mid), None);
        assert_eq!(look.get(Signal::OverMusic), None);

        let mut floor = Floor::new();
        let start = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        for i in 0..200u64 {
            #[allow(clippy::cast_precision_loss)]
            let music = 0.4 + 0.05 * (i % 6) as f32;
            let measured = Measured {
                heard: Some(music * 0.9 + 0.05),
                ..Measured::default()
            };
            saw(
                &mut floor,
                &measured,
                Some(music),
                start + Duration::from_secs(i * 2),
            );
        }
        let line = floor.line(Signal::OverMusic);
        assert!(line.now.is_some(), "{}", line.says);
    }

    /// **Nothing is said while nothing is looking.**
    #[test]
    fn the_floor_is_silent_once_nobody_is_looking() {
        let mut floor = Floor::new();
        let start = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        saw(
            &mut floor,
            &Measured {
                high: Some(0.4),
                ..Measured::default()
            },
            None,
            start,
        );
        let live = read(&floor, start + Duration::from_secs(1));
        assert!(live.watching);
        assert_eq!(live.lines.len(), Signal::ALL.len());
        assert!(live.lines.iter().all(|l| l.confidence == "not_yet"));

        let gone = read(&floor, start + Duration::from_secs(60));
        assert!(!gone.watching);
        assert!(gone.lines.is_empty());

        let state = AppState::new(true);
        assert_eq!(briefing(&state), None);
    }

    #[test]
    fn the_master_is_read_on_the_microphones_scale() {
        let mut snapshot = crate::commands::snapshot_now(&AppState::new(true));
        snapshot.master.peak_left = 0.1;
        snapshot.master.peak_right = 0.01;
        let level = music_level(&snapshot).expect("a level");
        assert!((level - 40.0 / 60.0).abs() < 1e-4, "{level}");
    }
}
