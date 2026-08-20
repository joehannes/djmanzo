//! A plugin's controls, read out generically.
//!
//! # Why generically and not through the plugin's own window
//!
//! A CLAP plugin's interface is a native child window, and there is nowhere to
//! put one inside a webview. So djmanzo asks the plugin what it has — name,
//! range, default, whether it steps — and draws the controls itself. Every
//! plugin gets the same treatment and none of them get their own look.
//!
//! That is a real loss for a plugin whose panel *is* the product, and a real
//! gain everywhere else: the controls are in djmanzo's own vocabulary, so a
//! controller can be mapped to one, a preset can save one, and the assistant
//! can move one. A plugin window can do none of those things.
//!
//! # Values are plain, not normalised
//!
//! CLAP parameters carry their own range — a filter cutoff really is in hertz —
//! and the host is told the minimum and maximum. Passing the plain value around
//! rather than a 0..1 fraction means the number shown is the number the plugin
//! uses, and a preset saved at 440 Hz reopens at 440 Hz whatever the plugin
//! decides its range is next version.

use clack_extensions::params::{ParamInfoBuffer, ParamInfoFlags, PluginParams};
use clack_host::prelude::*;

/// One control a plugin exposes.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamInfo {
    /// The plugin's own stable id. Not an index: a plugin may reorder its
    /// parameters between versions, and a preset saved against an index would
    /// silently move a DJ's settings to the wrong knob.
    pub id: u32,
    pub name: String,
    /// Its place in the plugin's own grouping, e.g. `Filter/Cutoff`. Empty when
    /// the plugin does not group.
    pub module: String,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    pub value: f64,
    /// The parameter only takes whole numbers — a mode switch rather than a
    /// knob, and drawn as one.
    pub stepped: bool,
    /// The plugin will not let a host change it. Shown, but not offered.
    pub read_only: bool,
}

impl ParamInfo {
    /// Where the control sits in its own range, 0..=1, for a slider.
    #[must_use]
    pub fn fraction(&self) -> f64 {
        let span = self.max - self.min;
        if span <= 0.0 {
            return 0.0;
        }
        ((self.value - self.min) / span).clamp(0.0, 1.0)
    }

    /// The plain value at a fraction of the range.
    #[must_use]
    pub fn at(&self, fraction: f64) -> f64 {
        let value = self.min + (self.max - self.min) * fraction.clamp(0.0, 1.0);
        if self.stepped { value.round() } else { value }
    }
}

/// Everything the plugin lets a host change.
///
/// Empty when the plugin has no parameters extension, which is legitimate — an
/// effect with nothing to tune is a valid plugin, and it should appear in the
/// chain rather than be refused.
pub fn read<H: HostHandlers>(instance: &mut PluginInstance<H>) -> Vec<ParamInfo> {
    let mut handle = instance.plugin_handle();
    let Some(params) = handle.get_extension::<PluginParams>() else {
        return Vec::new();
    };
    let count = params.count(&mut handle);
    let mut buffer = ParamInfoBuffer::new();
    let mut out = Vec::with_capacity(count as usize);
    for index in 0..count {
        let Some(info) = params.get_info(&mut handle, index, &mut buffer) else {
            continue;
        };
        // Name and module come back as fixed C buffers with the text at the
        // front, so the trailing nul padding has to go or every name in the
        // interface carries a tail of invisible characters.
        let id: u32 = info.id.into();
        let name = c_text(info.name);
        let module = c_text(info.module);
        let min = info.min_value;
        let max = info.max_value;
        let default = info.default_value;
        let stepped = info.flags.contains(ParamInfoFlags::IS_STEPPED);
        let read_only = info.flags.contains(ParamInfoFlags::IS_READONLY);
        let param_id = info.id;
        let value = params.get_value(&mut handle, param_id).unwrap_or(default);
        out.push(ParamInfo {
            id,
            name,
            module,
            min,
            max,
            default,
            value,
            stepped,
            read_only,
        });
    }
    out
}

/// A fixed C buffer as a `String`, without its nul padding.
fn c_text(raw: &[u8]) -> String {
    let end = raw.iter().position(|byte| *byte == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn knob() -> ParamInfo {
        ParamInfo {
            id: 1,
            name: "Cutoff".to_owned(),
            module: String::new(),
            min: 20.0,
            max: 20_000.0,
            default: 1_000.0,
            value: 10_010.0,
            stepped: false,
            read_only: false,
        }
    }

    #[test]
    fn a_fraction_is_where_the_value_sits_in_its_range() {
        assert!((knob().fraction() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn a_fraction_maps_back_to_the_plain_value() {
        let knob = knob();
        assert!((knob.at(0.5) - 10_010.0).abs() < 1e-9);
        assert!((knob.at(0.0) - 20.0).abs() < 1e-9);
        assert!((knob.at(1.0) - 20_000.0).abs() < 1e-9);
    }

    /// A stepped parameter is a mode switch. Handing a plugin 2.4 for a
    /// three-way switch is asking it to guess.
    #[test]
    fn a_stepped_parameter_lands_on_a_whole_number() {
        let switch = ParamInfo {
            min: 0.0,
            max: 3.0,
            stepped: true,
            ..knob()
        };
        assert_eq!(switch.at(0.4), 1.0);
        assert_eq!(switch.at(0.9), 3.0);
    }

    /// A plugin is entitled to expose a parameter with one possible value, and
    /// dividing by its zero-width range must not produce a NaN on a slider.
    #[test]
    fn a_range_of_nothing_does_not_divide_by_zero() {
        let fixed = ParamInfo {
            min: 1.0,
            max: 1.0,
            value: 1.0,
            ..knob()
        };
        assert_eq!(fixed.fraction(), 0.0);
        assert!(fixed.at(0.5).is_finite());
    }

    #[test]
    fn a_fraction_outside_the_range_is_clamped() {
        let knob = knob();
        assert_eq!(knob.at(-3.0), knob.min);
        assert_eq!(knob.at(9.0), knob.max);

        let below = ParamInfo {
            value: -50.0,
            ..knob
        };
        assert_eq!(below.fraction(), 0.0);
    }
}
