//! The host, exercised against a real CLAP plugin.
//!
//! The plugin itself is `dj_clap::testplug` — see that module for why it is a
//! shared fixture rather than a local one.
//!
//! What these deliberately do not test is dynamic loading itself: `dlopen` on
//! a file that does not exist is the operating system's job, not ours.

#![cfg(feature = "test-plugin")]

use dj_clap::plugin::CHANNELS;
use dj_clap::testplug::{GAIN_ID, bundle};

/// A block of interleaved stereo, every sample the same.
fn block(frames: usize, level: f32) -> Vec<f32> {
    vec![level; frames * CHANNELS]
}

#[test]
fn a_bundle_lists_what_is_inside_it() {
    let plugins = bundle().unwrap().plugins();
    assert_eq!(plugins.len(), 1, "{plugins:?}");
    assert_eq!(plugins[0].id, "dev.djmanzo.testgain");
    assert_eq!(plugins[0].name, "Test Gain");
    assert_eq!(plugins[0].vendor, "djmanzo");
}

#[test]
fn a_plugin_can_be_instantiated_by_id() {
    let bundle = bundle().unwrap();
    assert!(bundle.instantiate(Some("dev.djmanzo.testgain")).is_ok());
    // And a bundle with one plugin in it does not need to be told which.
    assert!(bundle.instantiate(None).is_ok());
}

#[test]
fn asking_for_a_plugin_that_is_not_there_says_so() {
    let bundle = bundle().unwrap();
    let Err(error) = bundle.instantiate(Some("com.example.nope")) else {
        panic!("a plugin that is not there was instantiated");
    };
    assert!(
        matches!(error, dj_clap::ClapError::NoSuchPlugin { .. }),
        "{error:?}"
    );
}

/// Parameters are read out generically: name, range, default and current
/// value, addressed by the plugin's own id.
#[test]
fn the_hosts_reads_a_plugins_parameters() {
    let bundle = bundle().unwrap();
    let mut loaded = bundle.instantiate(None).unwrap();
    let params = loaded.params();

    assert_eq!(params.len(), 1, "{params:?}");
    let gain = &params[0];
    assert_eq!(gain.id, GAIN_ID, "the id was not the plugin's own");
    assert_eq!(gain.name, "Gain");
    assert_eq!(gain.module, "Amp/Gain");
    assert_eq!(gain.min, 0.0);
    assert_eq!(gain.max, 2.0);
    assert_eq!(gain.default, 1.0);
    assert_eq!(gain.value, 1.0);
    assert!(!gain.stepped);
}

/// **The whole point.** Audio goes in, the plugin's audio comes out.
#[test]
fn audio_goes_through_the_plugin() {
    let bundle = bundle().unwrap();
    let mut loaded = bundle.instantiate(None).unwrap();
    let mut processor = loaded.activate(48_000.0, 256).unwrap();

    let mut buffer = block(256, 0.5);
    processor.process(&mut buffer);
    // Unity gain: it comes back as it went in, which proves the buffers are
    // wired the right way round rather than the plugin being skipped.
    assert!(
        buffer.iter().all(|s| (*s - 0.5).abs() < 1e-6),
        "first sample {}",
        buffer[0]
    );

    loaded.deactivate(processor);
}

/// A parameter change reaches the plugin and changes what comes out.
#[test]
fn setting_a_parameter_changes_the_audio() {
    let bundle = bundle().unwrap();
    let mut loaded = bundle.instantiate(None).unwrap();
    let mut processor = loaded.activate(48_000.0, 256).unwrap();

    processor.set_param(GAIN_ID, 0.25);
    let mut buffer = block(256, 0.8);
    processor.process(&mut buffer);
    assert!(
        (buffer[0] - 0.2).abs() < 1e-6,
        "the parameter did not reach the plugin: {}",
        buffer[0]
    );

    // And it stays changed on the next block, rather than the event being
    // re-delivered or forgotten.
    let mut buffer = block(256, 0.8);
    processor.process(&mut buffer);
    assert!((buffer[0] - 0.2).abs() < 1e-6, "{}", buffer[0]);

    loaded.deactivate(processor);
}

/// A parameter addressed by the wrong id must not move the right one.
#[test]
fn a_parameter_that_does_not_exist_changes_nothing() {
    let bundle = bundle().unwrap();
    let mut loaded = bundle.instantiate(None).unwrap();
    let mut processor = loaded.activate(48_000.0, 256).unwrap();

    processor.set_param(GAIN_ID + 1, 0.0);
    let mut buffer = block(256, 0.5);
    processor.process(&mut buffer);
    assert!(
        (buffer[0] - 0.5).abs() < 1e-6,
        "an unknown id moved the gain: {}",
        buffer[0]
    );

    loaded.deactivate(processor);
}

/// A device may hand over fewer frames than the maximum on any given callback.
/// The scratch is sized for the maximum and this must not reallocate, panic,
/// or process stale samples from the end of a previous, longer block.
#[test]
fn a_short_block_is_processed_and_nothing_stale_comes_with_it() {
    let bundle = bundle().unwrap();
    let mut loaded = bundle.instantiate(None).unwrap();
    let mut processor = loaded.activate(48_000.0, 512).unwrap();

    // A full block at one level...
    processor.set_param(GAIN_ID, 1.0);
    let mut long = block(512, 0.9);
    processor.process(&mut long);

    // ...then a short one at another. Only the short block's own frames may
    // come back, and they must carry the short block's audio.
    let mut short = block(64, 0.1);
    processor.process(&mut short);
    assert_eq!(short.len(), 64 * CHANNELS, "the buffer changed length");
    assert!(
        short.iter().all(|s| (*s - 0.1).abs() < 1e-6),
        "stale audio came back: {:?}",
        &short[..4]
    );

    loaded.deactivate(processor);
}

/// A block longer than the plugin was activated for is truncated rather than
/// panicking on the audio thread.
#[test]
fn an_oversized_block_does_not_panic() {
    let bundle = bundle().unwrap();
    let mut loaded = bundle.instantiate(None).unwrap();
    let mut processor = loaded.activate(48_000.0, 64).unwrap();

    let mut buffer = block(256, 0.5);
    processor.process(&mut buffer);
    // The first 64 frames went through the plugin; the rest are untouched.
    assert!((buffer[0] - 0.5).abs() < 1e-6);
    assert_eq!(buffer.len(), 256 * CHANNELS);

    loaded.deactivate(processor);
}

/// An empty block is a thing a device can produce, and it must not divide by
/// zero or index off the end.
#[test]
fn an_empty_block_is_harmless() {
    let bundle = bundle().unwrap();
    let mut loaded = bundle.instantiate(None).unwrap();
    let mut processor = loaded.activate(48_000.0, 256).unwrap();
    let mut nothing: Vec<f32> = Vec::new();
    processor.process(&mut nothing);
    assert!(nothing.is_empty());
    loaded.deactivate(processor);
}

/// The processor is the half that travels to the audio thread. If it stopped
/// being `Send` the engine could not take it, and the failure would be a
/// compile error in a different crate weeks later.
#[test]
fn the_processor_can_be_sent_to_the_audio_thread() {
    fn assert_send<T: Send>() {}
    assert_send::<dj_clap::Processor>();

    let bundle = bundle().unwrap();
    let mut loaded = bundle.instantiate(None).unwrap();
    let mut processor = loaded.activate(48_000.0, 128).unwrap();

    // Actually send it, process there, and bring it home — which is the round
    // trip the engine makes on every load and unload.
    let processor = std::thread::spawn(move || {
        let mut buffer = block(128, 0.5);
        processor.process(&mut buffer);
        assert!((buffer[0] - 0.5).abs() < 1e-6);
        processor
    })
    .join()
    .unwrap();

    loaded.deactivate(processor);
}
