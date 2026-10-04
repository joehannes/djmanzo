//! §122 step 4b: the DJ's own MilkDrop presets, read into the form
//! Butterchurn draws. See `docs/VISUALS.md`.
//!
//! Two kinds of file are taken:
//!
//! - **`.milk`**, MilkDrop's own: `key=value` lines under `[preset00]`. The
//!   values become Butterchurn's base values (by the `RENAMED` table, or by
//!   their own name lower-cased); the numbered equation lines — `per_frame_1`,
//!   `per_frame_2`, … — are joined in the order of their numbers, not of the
//!   file; the four custom waves and four custom shapes keep theirs. MilkDrop
//!   2's shaders (`warp_`, `comp_`) are HLSL, which nothing here translates:
//!   they are left out, the preset draws with Butterchurn's default pair, and
//!   the DJ is told so.
//! - **Butterchurn's own JSON**, taken as it is if its equations are in
//!   MilkDrop's language (`*_eqs_eel`). One that carries only JavaScript
//!   (`*_eqs_str`) is refused: djmanzo never evaluates JavaScript, and says
//!   that rather than drawing nothing.
//!
//! Either way the result is made whole — four shapes, four waves, every
//! equation and shader field a string, and no JavaScript left in it — because
//! Butterchurn's WebAssembly path indexes all four of each and throws on a
//! missing one.

use serde_json::{Map, Value, json};

/// The largest preset file taken. MilkDrop's own are a few kilobytes; a
/// file this size is not a preset.
pub const LARGEST: u64 = 512 * 1024;

/// `.milk` names that Butterchurn calls something else, lower-cased. A name
/// not here is Butterchurn's own already. From `milkdrop-preset-utils` (MIT),
/// Butterchurn's own reader, as a table of names.
const RENAMED: [(&str, &str); 37] = [
    ("frating", "rating"),
    ("fgammaadj", "gammaadj"),
    ("fdecay", "decay"),
    ("fvideoechozoom", "echo_zoom"),
    ("fvideoechoalpha", "echo_alpha"),
    ("nvideoechoorientation", "echo_orient"),
    ("nwavemode", "wave_mode"),
    ("badditivewaves", "additivewave"),
    ("bwavedots", "wave_dots"),
    ("bwavethick", "wave_thick"),
    ("bmodwavealphabyvolume", "modwavealphabyvolume"),
    ("bmaximizewavecolor", "wave_brighten"),
    ("btexwrap", "wrap"),
    ("bdarkencenter", "darken_center"),
    ("bredbluestereo", "red_blue"),
    ("bbrighten", "brighten"),
    ("bdarken", "darken"),
    ("bsolarize", "solarize"),
    ("binvert", "invert"),
    ("fwavealpha", "wave_a"),
    ("fwavescale", "wave_scale"),
    ("fwavesmoothing", "wave_smoothing"),
    ("fwaveparam", "wave_mystery"),
    ("fmodwavealphastart", "modwavealphastart"),
    ("fmodwavealphaend", "modwavealphaend"),
    ("fwarpanimspeed", "warpanimspeed"),
    ("fwarpscale", "warpscale"),
    ("fzoomexponent", "zoomexp"),
    ("nmotionvectorsx", "mv_x"),
    ("nmotionvectorsy", "mv_y"),
    ("thick", "thickoutline"),
    ("instances", "num_inst"),
    ("num_instances", "num_inst"),
    ("badditive", "additive"),
    ("busedots", "usedots"),
    ("bspectrum", "spectrum"),
    ("bdrawthick", "thick"),
];

/// How many custom shapes and waves a preset has, in MilkDrop and in
/// Butterchurn alike.
const CUSTOM: usize = 4;

/// A preset ready for Butterchurn, and what the DJ should know about it.
#[derive(Debug, Clone, PartialEq)]
pub struct Read {
    pub preset: Value,
    /// Said beside the preset in the list: shaders left out, for instance.
    pub note: Option<String>,
}

fn renamed(key: &str) -> String {
    let lower = key.to_ascii_lowercase();
    RENAMED
        .iter()
        .find(|(from, _)| *from == lower)
        .map_or(lower, |(_, to)| (*to).to_owned())
}

/// Numbered lines, joined in the order of their numbers.
#[derive(Default)]
struct Lines(Vec<(u32, String)>);

impl Lines {
    fn push(&mut self, number: &str, code: &str) {
        if let Ok(n) = number.parse() {
            self.0.push((n, code.to_owned()));
        }
    }

    fn joined(mut self) -> String {
        self.0.sort_by_key(|(n, _)| *n);
        self.0
            .into_iter()
            .map(|(_, code)| code)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Default)]
struct Custom {
    base: Map<String, Value>,
    init: Lines,
    frame: Lines,
    point: Lines,
}

/// `prefix<i>_rest` → (i, rest), for i a custom shape's or wave's number.
fn numbered<'a>(key: &'a str, prefix: &str) -> Option<(usize, &'a str)> {
    let rest = key.strip_prefix(prefix)?;
    let (index, rest) = rest.split_once('_')?;
    let index: usize = index.parse().ok()?;
    (index < CUSTOM).then_some((index, rest))
}

/// A `.milk` file's text, read into Butterchurn's form.
///
/// # Errors
/// The text has no `key=value` lines at all, so it is not a MilkDrop preset.
pub fn from_milk(text: &str) -> Result<Read, String> {
    let mut base = Map::new();
    let (mut init, mut frame, mut pixel) = (Lines::default(), Lines::default(), Lines::default());
    let mut shapes: Vec<Custom> = (0..CUSTOM).map(|_| Custom::default()).collect();
    let mut waves: Vec<Custom> = (0..CUSTOM).map(|_| Custom::default()).collect();
    let mut shaders = false;
    let mut version = 1;
    let mut lines = 0;

    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || key.starts_with('[') {
            continue;
        }
        lines += 1;
        let lower = key.to_ascii_lowercase();
        let code = value.trim();

        if let Some(n) = lower.strip_prefix("per_frame_init_") {
            init.push(n, code);
        } else if let Some(n) = lower.strip_prefix("per_frame_") {
            frame.push(n, code);
        } else if let Some(n) = lower.strip_prefix("per_pixel_") {
            pixel.push(n, code);
        } else if lower.starts_with("warp_") || lower.starts_with("comp_") {
            shaders |= !code.trim_start_matches('`').trim().is_empty();
        } else if let Some((i, rest)) = numbered(&lower, "wavecode_") {
            if let Ok(v) = code.parse::<f64>() {
                waves[i].base.insert(renamed(rest), json!(v));
            }
        } else if let Some((i, rest)) = numbered(&lower, "shapecode_") {
            if let Ok(v) = code.parse::<f64>() {
                shapes[i].base.insert(renamed(rest), json!(v));
            }
        } else if let Some((i, rest)) = numbered(&lower, "wave_") {
            if let Some(n) = rest.strip_prefix("init") {
                waves[i].init.push(n, code);
            } else if let Some(n) = rest.strip_prefix("per_frame") {
                waves[i].frame.push(n, code);
            } else if let Some(n) = rest.strip_prefix("per_point") {
                waves[i].point.push(n, code);
            }
        } else if let Some((i, rest)) = numbered(&lower, "shape_") {
            if let Some(n) = rest.strip_prefix("init") {
                shapes[i].init.push(n, code);
            } else if let Some(n) = rest.strip_prefix("per_frame") {
                shapes[i].frame.push(n, code);
            }
        } else if lower == "milkdrop_preset_version" {
            if code.parse::<u32>().is_ok_and(|v| v >= 200) {
                version = 2;
            }
        } else if lower.starts_with("psversion") {
            // Which shader model the HLSL wanted: nothing here to choose.
        } else if let Ok(v) = code.parse::<f64>() {
            base.insert(renamed(key), json!(v));
        }
    }
    if lines == 0 {
        return Err("it has no MilkDrop settings in it".to_owned());
    }

    let shapes: Vec<Value> = shapes
        .into_iter()
        .map(|s| {
            json!({
                "baseVals": s.base,
                "init_eqs_eel": s.init.joined(),
                "frame_eqs_eel": s.frame.joined(),
            })
        })
        .collect();
    let waves: Vec<Value> = waves
        .into_iter()
        .map(|w| {
            json!({
                "baseVals": w.base,
                "init_eqs_eel": w.init.joined(),
                "frame_eqs_eel": w.frame.joined(),
                "point_eqs_eel": w.point.joined(),
            })
        })
        .collect();
    let preset = json!({
        "version": version,
        "baseVals": base,
        "shapes": shapes,
        "waves": waves,
        "init_eqs_eel": init.joined(),
        "frame_eqs_eel": frame.joined(),
        "pixel_eqs_eel": pixel.joined(),
        "warp": "",
        "comp": "",
    });
    Ok(Read {
        preset: whole(preset)?,
        note: shaders.then(|| {
            "its MilkDrop 2 shaders are left out, so it draws with the default pair".to_owned()
        }),
    })
}

/// A Butterchurn JSON preset's text, taken as djmanzo can draw it.
///
/// # Errors
/// Not JSON, not an object, or equations only in JavaScript.
pub fn from_json(text: &str) -> Result<Read, String> {
    let preset: Value =
        serde_json::from_str(text).map_err(|e| format!("it is not a preset ({e})"))?;
    if !preset.is_object() {
        return Err("it is not a preset".to_owned());
    }
    if preset.get("init_eqs_eel").is_none() {
        return Err(if preset.get("init_eqs_str").is_some() {
            "its equations are only in JavaScript, which djmanzo does not run; a copy with \
             MilkDrop's own equations (`*_eqs_eel`) will work"
                .to_owned()
        } else {
            "it has no MilkDrop equations".to_owned()
        });
    }
    let shaders = ["warp", "comp"]
        .iter()
        .any(|f| preset[*f].as_str().is_some_and(|s| !s.trim().is_empty()));
    Ok(Read {
        preset: whole(preset)?,
        note: shaders.then(|| "it brings its own shaders".to_owned()),
    })
}

/// Read a preset file by its extension.
///
/// # Errors
/// Neither `.milk` nor `.json`, or what [`from_milk`] and [`from_json`] refuse.
pub fn read(name: &str, text: &str) -> Result<Read, String> {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".milk") {
        from_milk(text)
    } else if lower.ends_with(".json") {
        from_json(text)
    } else {
        Err("only .milk and Butterchurn .json presets are taken".to_owned())
    }
}

/// Make a preset whole for Butterchurn's WebAssembly path: four shapes and
/// four waves, each with its base values and equations as strings; every
/// top-level equation and shader a string; no JavaScript anywhere.
fn whole(mut preset: Value) -> Result<Value, String> {
    let Some(object) = preset.as_object_mut() else {
        return Err("it is not a preset".to_owned());
    };
    strip_javascript(object);
    if !object.get("baseVals").is_some_and(Value::is_object) {
        object.insert("baseVals".to_owned(), json!({}));
    }
    for field in [
        "init_eqs_eel",
        "frame_eqs_eel",
        "pixel_eqs_eel",
        "warp",
        "comp",
    ] {
        if !object.get(field).is_some_and(Value::is_string) {
            object.insert(field.to_owned(), json!(""));
        }
    }
    for (list, fields) in [
        ("shapes", &["init_eqs_eel", "frame_eqs_eel"][..]),
        (
            "waves",
            &["init_eqs_eel", "frame_eqs_eel", "point_eqs_eel"][..],
        ),
    ] {
        let mut items: Vec<Value> = object
            .get(list)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        items.truncate(CUSTOM);
        items.resize(CUSTOM, json!({ "baseVals": { "enabled": 0 } }));
        for item in &mut items {
            if !item.is_object() {
                *item = json!({ "baseVals": { "enabled": 0 } });
            }
            let Some(item) = item.as_object_mut() else {
                continue;
            };
            strip_javascript(item);
            if !item.get("baseVals").is_some_and(Value::is_object) {
                item.insert("baseVals".to_owned(), json!({ "enabled": 0 }));
            }
            for field in fields {
                if !item.get(*field).is_some_and(Value::is_string) {
                    item.insert((*field).to_owned(), json!(""));
                }
            }
        }
        object.insert(list.to_owned(), Value::Array(items));
    }
    Ok(preset)
}

/// Remove every JavaScript equation (`*_eqs_str`) and the flag asking for it.
fn strip_javascript(object: &mut Map<String, Value>) {
    object.retain(|key, _| !key.ends_with("_eqs_str") && key != "useJS");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A MilkDrop 2 preset of djmanzo's own making, with what real ones
    /// have: renamed and plain settings, equation lines past nine (so the
    /// order of their numbers matters), a custom wave and shape, and HLSL.
    const MILK: &str = "\
[preset00]
fRating=3.000000
fGammaAdj=1.700000
fDecay=0.955000
nWaveMode=7
bAdditiveWaves=1
zoom=1.010000
rot=0.000000
MILKDROP_PRESET_VERSION=201
PSVERSION=2
wavecode_0_enabled=1
wavecode_0_bSpectrum=1
wavecode_0_samples=256
wave_0_per_point2=y = y + 0.1;
wave_0_per_point1=x = sample;
shapecode_1_enabled=1
shapecode_1_sides=6
shapecode_1_thickOutline=1
shape_1_per_frame1=ang = time;
per_frame_init_1=q1 = 0;
per_frame_10=wave_b = 0.9;
per_frame_2=wave_g = 0.5;
per_frame_1=wave_r = 0.5 + 0.5*sin(time);
per_pixel_1=zoom = zoom + 0.01*rad;
warp_1=`shader_body {
warp_2=`  ret = tex2D(sampler_main, uv).xyz;
comp_1=`shader_body { ret = 1; }
";

    #[test]
    fn a_milk_file_becomes_butterchurns_own_preset() {
        let read = from_milk(MILK).unwrap();
        let p = &read.preset;
        assert_eq!(p["version"], 2);
        assert_eq!(p["baseVals"]["gammaadj"], 1.7);
        assert_eq!(p["baseVals"]["decay"], 0.955);
        assert_eq!(p["baseVals"]["wave_mode"], 7.0);
        assert_eq!(p["baseVals"]["additivewave"], 1.0);
        assert_eq!(p["baseVals"]["zoom"], 1.01);
        assert!(p["baseVals"].get("milkdrop_preset_version").is_none());
        // The order of the numbers, not of the file: 1, 2, 10.
        assert_eq!(
            p["frame_eqs_eel"],
            "wave_r = 0.5 + 0.5*sin(time);\nwave_g = 0.5;\nwave_b = 0.9;"
        );
        assert_eq!(p["init_eqs_eel"], "q1 = 0;");
        assert_eq!(p["pixel_eqs_eel"], "zoom = zoom + 0.01*rad;");
        assert_eq!(p["waves"][0]["baseVals"]["enabled"], 1.0);
        assert_eq!(p["waves"][0]["baseVals"]["spectrum"], 1.0);
        assert_eq!(p["waves"][0]["point_eqs_eel"], "x = sample;\ny = y + 0.1;");
        assert_eq!(p["shapes"][1]["baseVals"]["sides"], 6.0);
        assert_eq!(p["shapes"][1]["baseVals"]["thickoutline"], 1.0);
        assert_eq!(p["shapes"][1]["frame_eqs_eel"], "ang = time;");
        // HLSL left out, and said.
        assert_eq!(p["warp"], "");
        assert_eq!(p["comp"], "");
        assert!(read.note.unwrap().contains("shaders are left out"));
    }

    #[test]
    fn a_preset_is_made_whole_for_butterchurn() {
        for read in [
            from_milk("fDecay=0.9\n").unwrap(),
            from_json(r#"{"init_eqs_eel":"","shapes":[{"baseVals":{"enabled":1}}, 7],"waves":[]}"#)
                .unwrap(),
        ] {
            let p = &read.preset;
            assert_eq!(p["shapes"].as_array().unwrap().len(), 4);
            assert_eq!(p["waves"].as_array().unwrap().len(), 4);
            for shape in p["shapes"].as_array().unwrap() {
                assert!(shape["baseVals"].is_object());
                assert!(shape["init_eqs_eel"].is_string() && shape["frame_eqs_eel"].is_string());
            }
            for wave in p["waves"].as_array().unwrap() {
                assert!(wave["point_eqs_eel"].is_string(), "{wave}");
            }
            for field in [
                "init_eqs_eel",
                "frame_eqs_eel",
                "pixel_eqs_eel",
                "warp",
                "comp",
            ] {
                assert!(p[field].is_string(), "{field}");
            }
        }
        // A MilkDrop 1 preset: no version line.
        assert_eq!(from_milk("fDecay=0.9\n").unwrap().preset["version"], 1);
    }

    #[test]
    fn javascript_is_refused_or_stripped_never_kept() {
        let only_js = r#"{"init_eqs_str":"a.q1=0;","frame_eqs_str":""}"#;
        assert!(from_json(only_js).unwrap_err().contains("JavaScript"));
        let both = r#"{"init_eqs_eel":"q1=0;","init_eqs_str":"a.q1=0;","useJS":true,
            "shapes":[{"baseVals":{},"init_eqs_str":"x"}]}"#;
        let read = from_json(both).unwrap();
        let text = read.preset.to_string();
        assert!(
            !text.contains("_eqs_str") && !text.contains("useJS"),
            "{text}"
        );
        assert!(read.note.is_none());
        assert!(from_json("[1, 2]").is_err());
        assert!(from_json("not json").is_err());
    }

    #[test]
    fn only_milk_and_json_are_taken_and_a_milk_must_say_something() {
        assert!(read("x.MILK", "fDecay=0.9").is_ok());
        assert!(read("x.txt", "fDecay=0.9").unwrap_err().contains(".milk"));
        assert!(
            from_milk("just words\n\n")
                .unwrap_err()
                .contains("no MilkDrop")
        );
    }
}
