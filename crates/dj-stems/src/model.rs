//! What an HT-Demucs ONNX export expects, and how a chunk longer than its
//! segment is run through it.
//!
//! # What went wrong, and why this reads the model instead of assuming it
//!
//! The first engine called the model's input `input` and its output
//! `output`, fed it a whole ten-second chunk, and took the stems in
//! djmanzo's own order — vocals first. The export djmanzo's notes point to
//! (StemSplit's `demucs-onnx`, MIT) calls them `mix` and `stems`, fixes the
//! segment at 343,980 samples (7.8 s at 44.1 kHz), and gives the stems as
//! drums, bass, other, vocals. So every chunk failed with *Invalid input
//! name: input*, logged under a crate the application's log filter did not
//! show, and the stem controls moved gains on stems that never arrived.
//!
//! So nothing about the model is assumed that the model can say itself.
//! [`Contract::read`] takes the names and the segment length from the
//! graph's own declaration — `mix` and `stems` when they are there, the
//! only input and output when there is one of each — and the number of
//! stems from the shape of what the model actually returns.
//!
//! # Longer than a segment
//!
//! A model with a fixed segment is run over overlapping segments — a quarter
//! of each shared with the next, cross-faded — which is how Demucs itself
//! runs a whole song: a segment's edges have less context than its middle,
//! and the fade lets the neighbour's middle carry them. The fade never
//! reaches zero, so the first and last samples of a chunk are not divided
//! by nothing. The last segment is padded with silence and cut back.
//!
//! # Six stems into four
//!
//! The six-stem export adds guitar and piano. djmanzo's decks have four
//! faders, so both are folded into *other*, which is where the four-stem
//! model puts them anyway.

use dj_core::Stem;

/// The rate HT-Demucs was trained at, and the only one its export takes.
pub const MODEL_RATE: u32 = 44_100;

/// One stem as a model names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Drums,
    Bass,
    Other,
    Vocals,
    Guitar,
    Piano,
}

impl Source {
    /// HT-Demucs's four, in its order.
    pub const FOUR: [Source; 4] = [Source::Drums, Source::Bass, Source::Other, Source::Vocals];
    /// The six-stem model's.
    pub const SIX: [Source; 6] = [
        Source::Drums,
        Source::Bass,
        Source::Other,
        Source::Vocals,
        Source::Guitar,
        Source::Piano,
    ];

    /// The layout a model giving `count` stems uses, if djmanzo knows one.
    #[must_use]
    pub fn layout(count: usize) -> Option<&'static [Source]> {
        match count {
            4 => Some(&Self::FOUR),
            6 => Some(&Self::SIX),
            _ => None,
        }
    }

    /// Which of djmanzo's four it plays on.
    #[must_use]
    pub const fn stem(self) -> Stem {
        match self {
            Source::Drums => Stem::Drums,
            Source::Bass => Stem::Bass,
            Source::Vocals => Stem::Vocal,
            Source::Other | Source::Guitar | Source::Piano => Stem::Other,
        }
    }
}

/// A tensor as the graph declares it: a name and its dimensions, `-1` where
/// a dimension is left open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    pub name: String,
    pub dims: Vec<i64>,
}

/// What the model takes and gives, read from its declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contract {
    pub input: String,
    pub output: String,
    /// Samples per segment, when the graph fixes it.
    pub segment: Option<usize>,
}

fn describe(declared: &[Declared]) -> String {
    declared
        .iter()
        .map(|d| {
            let dims: Vec<String> = d
                .dims
                .iter()
                .map(|n| {
                    if *n < 0 {
                        "?".to_owned()
                    } else {
                        n.to_string()
                    }
                })
                .collect();
            format!("{} [{}]", d.name, dims.join(", "))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The one named `wanted`, or the only one.
fn pick<'a>(declared: &'a [Declared], wanted: &str) -> Option<&'a Declared> {
    declared
        .iter()
        .find(|d| d.name == wanted)
        .or(match declared {
            [only] => Some(only),
            _ => None,
        })
}

impl Contract {
    /// Read the contract off a model's declared inputs and outputs.
    ///
    /// # Errors
    /// A sentence saying what the model declares and what djmanzo needs,
    /// when it is not a stereo mix in and stems out.
    pub fn read(inputs: &[Declared], outputs: &[Declared]) -> Result<Self, String> {
        let needs =
            "djmanzo needs a stereo mix in, [1, 2, samples], and stems out, [1, stems, 2, samples]";
        let input = pick(inputs, "mix")
            .ok_or_else(|| format!("it takes {} and {needs}", describe(inputs)))?;
        let output = pick(outputs, "stems")
            .ok_or_else(|| format!("it gives {} and {needs}", describe(outputs)))?;
        let fits = |dims: &[i64], at: usize, want: i64| dims[at] < 0 || dims[at] == want;
        if input.dims.len() != 3 || !fits(&input.dims, 0, 1) || !fits(&input.dims, 1, 2) {
            return Err(format!("it takes {} and {needs}", describe(inputs)));
        }
        if output.dims.len() != 4 || !fits(&output.dims, 0, 1) || !fits(&output.dims, 2, 2) {
            return Err(format!("it gives {} and {needs}", describe(outputs)));
        }
        let segment = usize::try_from(input.dims[2]).ok().filter(|n| *n > 0);
        Ok(Self {
            input: input.name.clone(),
            output: output.name.clone(),
            segment,
        })
    }
}

/// djmanzo's four stems, each as two channels.
pub type Planar = [[Vec<f32>; 2]; Stem::COUNT];

/// What one run of the model gave back: how many stems, and the values
/// `[stems][2][samples]`, flattened.
#[derive(Debug, Clone, PartialEq)]
pub struct Ran {
    pub stems: usize,
    pub values: Vec<f32>,
}

/// The cross-fade over a segment of `len` sharing `fade` samples with each
/// neighbour: rising over the first `fade`, falling over the last, never
/// reaching zero.
fn window(len: usize, fade: usize) -> Vec<f32> {
    (0..len)
        .map(|i| {
            if fade == 0 {
                return 1.0;
            }
            let rise = (i + 1) as f32 / (fade + 1) as f32;
            let fall = (len - i) as f32 / (fade + 1) as f32;
            rise.min(fall).min(1.0)
        })
        .collect()
}

/// Run `run` over two channels at the model's rate, in segments of
/// `segment` samples when the model fixes one (the whole of it when it does
/// not), and fold what comes back into djmanzo's four stems.
///
/// `run` is handed `[2][segment]` flattened — left, then right — and gives
/// back [`Ran`].
///
/// # Errors
/// Whatever `run` says, or a sentence when what it gave back is not
/// `[stems][2][segment]` for a number of stems djmanzo knows.
pub fn separate<F>(
    left: &[f32],
    right: &[f32],
    segment: Option<usize>,
    mut run: F,
) -> Result<Planar, String>
where
    F: FnMut(&[f32]) -> Result<Ran, String>,
{
    let total = left.len().min(right.len());
    if total == 0 {
        return Err("there is nothing to separate".to_owned());
    }
    let seg = segment.unwrap_or(total);
    let (count, stride, fade) = if total <= seg {
        (1, seg, 0)
    } else {
        let fade = seg / 4;
        let stride = seg - fade;
        (1 + (total - seg).div_ceil(stride), stride, fade)
    };
    let weights = window(seg, fade);

    let mut out: Planar = std::array::from_fn(|_| [vec![0.0; total], vec![0.0; total]]);
    let mut weight = vec![0.0f32; total];
    let mut piece = vec![0.0f32; 2 * seg];

    for index in 0..count {
        let start = index * stride;
        let end = (start + seg).min(total);
        let len = end - start;
        piece.fill(0.0);
        piece[..len].copy_from_slice(&left[start..end]);
        piece[seg..seg + len].copy_from_slice(&right[start..end]);

        let ran = run(&piece)?;
        let layout = Source::layout(ran.stems).ok_or_else(|| {
            format!(
                "it gave {} stems, and djmanzo knows the four- and six-stem HT-Demucs layouts",
                ran.stems
            )
        })?;
        if ran.values.len() != ran.stems * 2 * seg {
            return Err(format!(
                "it gave {} values for a segment of {seg} samples, where {} stems would be {}",
                ran.values.len(),
                ran.stems,
                ran.stems * 2 * seg
            ));
        }
        for (row, source) in layout.iter().enumerate() {
            let stem = source.stem().index();
            for (channel, into) in out[stem].iter_mut().enumerate() {
                let from = &ran.values[(row * 2 + channel) * seg..][..len];
                for ((sample, value), w) in into[start..end].iter_mut().zip(from).zip(&weights) {
                    *sample += value * w;
                }
            }
        }
        for (sum, w) in weight[start..end].iter_mut().zip(&weights) {
            *sum += w;
        }
    }

    for stem in &mut out {
        for channel in stem.iter_mut() {
            for (sample, w) in channel.iter_mut().zip(&weight) {
                *sample /= *w;
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declared(name: &str, dims: &[i64]) -> Declared {
        Declared {
            name: name.to_owned(),
            dims: dims.to_vec(),
        }
    }

    /// **The export's own names, and the length it fixes**, are what the
    /// contract says — and a model with one input and one output of other
    /// names is read by position.
    #[test]
    fn the_contract_is_read_from_the_model() {
        let demucs = Contract::read(
            &[declared("mix", &[1, 2, 343_980])],
            &[declared("stems", &[1, 4, 2, 343_980])],
        )
        .unwrap();
        assert_eq!(demucs.input, "mix");
        assert_eq!(demucs.output, "stems");
        assert_eq!(demucs.segment, Some(343_980));

        let other = Contract::read(
            &[declared("input", &[-1, 2, -1])],
            &[declared("output", &[-1, -1, 2, -1])],
        )
        .unwrap();
        assert_eq!(other.input, "input");
        assert_eq!(other.segment, None);

        let mono = Contract::read(
            &[declared("mix", &[1, 1, 44_100])],
            &[declared("stems", &[1, 4, 1, 44_100])],
        )
        .unwrap_err();
        assert!(mono.contains("mix [1, 1, 44100]"), "{mono}");
        assert!(mono.contains("stereo"), "{mono}");

        let two = Contract::read(
            &[declared("a", &[1, 2, 10]), declared("b", &[1, 2, 10])],
            &[declared("stems", &[1, 4, 2, 10])],
        )
        .unwrap_err();
        assert!(two.contains("a [1, 2, 10], b [1, 2, 10]"), "{two}");
    }

    /// A stand-in model: each of its stems is the segment times a gain, in
    /// HT-Demucs's order. Counts its runs.
    fn gains<'a>(
        gains: &'static [f32],
        runs: &'a mut usize,
    ) -> impl FnMut(&[f32]) -> Result<Ran, String> + 'a {
        move |piece: &[f32]| {
            *runs += 1;
            let values = gains
                .iter()
                .flat_map(|g| piece.iter().map(move |x| x * g))
                .collect();
            Ok(Ran {
                stems: gains.len(),
                values,
            })
        }
    }

    fn signal(len: usize, freq: f32) -> Vec<f32> {
        (0..len)
            .map(|n| (n as f32 * freq).sin() * 0.5 + (n as f32 * 0.001).cos() * 0.1)
            .collect()
    }

    fn worst(a: &[f32], b: &[f32]) -> f32 {
        assert_eq!(a.len(), b.len());
        a.iter()
            .zip(b)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f32::max)
    }

    /// **Every stem lands on djmanzo's stem and at its own time**, across
    /// segment seams, the padded last segment, and the first and last
    /// samples. A stand-in whose stems are the mix times drums 0.1, bass
    /// 0.2, other 0.3, vocals 0.4 must give back exactly that, sample for
    /// sample: a stem in the wrong slot, a seam not cross-faded back to
    /// unity, or a segment placed at the wrong time all break it.
    #[test]
    fn the_stems_come_back_in_place_and_in_order() {
        let segment = 1_000;
        for total in [700, 1_000, 1_001, 2_600, 9_999] {
            let left = signal(total, 0.03);
            let right = signal(total, 0.05);
            let mut runs = 0;
            let out = separate(
                &left,
                &right,
                Some(segment),
                gains(&[0.1, 0.2, 0.3, 0.4], &mut runs),
            )
            .unwrap();
            for (stem, gain) in [
                (Stem::Drums, 0.1),
                (Stem::Bass, 0.2),
                (Stem::Other, 0.3),
                (Stem::Vocal, 0.4),
            ] {
                let expect_l: Vec<f32> = left.iter().map(|x| x * gain).collect();
                let expect_r: Vec<f32> = right.iter().map(|x| x * gain).collect();
                assert!(
                    worst(&out[stem.index()][0], &expect_l) < 1e-5,
                    "{stem} left, {total}"
                );
                assert!(
                    worst(&out[stem.index()][1], &expect_r) < 1e-5,
                    "{stem} right, {total}"
                );
            }
            let expected_runs = if total <= segment {
                1
            } else {
                1 + (total - segment).div_ceil(750)
            };
            assert_eq!(runs, expected_runs, "{total} samples");
        }
    }

    /// Guitar and piano are folded into *other*; a model with no fixed
    /// segment is run once over the whole of it.
    #[test]
    fn six_stems_fold_into_four_and_an_open_length_runs_once() {
        let left = signal(5_000, 0.02);
        let right = signal(5_000, 0.07);
        let mut runs = 0;
        let out = separate(
            &left,
            &right,
            None,
            gains(&[0.1, 0.2, 0.3, 0.25, 0.1, 0.05], &mut runs),
        )
        .unwrap();
        assert_eq!(runs, 1);
        let other: Vec<f32> = left.iter().map(|x| x * 0.45).collect();
        assert!(worst(&out[Stem::Other.index()][0], &other) < 1e-5);
        let vocal: Vec<f32> = right.iter().map(|x| x * 0.25).collect();
        assert!(worst(&out[Stem::Vocal.index()][1], &vocal) < 1e-5);
    }

    /// What does not fit is refused with a sentence, never sliced blindly.
    #[test]
    fn what_does_not_fit_is_said() {
        let left = signal(100, 0.1);
        let three = separate(&left, &left, Some(100), |piece: &[f32]| {
            Ok(Ran {
                stems: 3,
                values: piece.repeat(3),
            })
        })
        .unwrap_err();
        assert!(three.contains("gave 3 stems"), "{three}");
        let short = separate(&left, &left, Some(100), |_: &[f32]| {
            Ok(Ran {
                stems: 4,
                values: vec![0.0; 10],
            })
        })
        .unwrap_err();
        assert!(short.contains("gave 10 values"), "{short}");
        assert!(separate(&[], &[], Some(100), |_: &[f32]| unreachable!()).is_err());
    }
}
