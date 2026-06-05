use crate::audio::{AudioEngine, active_effect_specs, pluck_delay_samples_for_test, render};
use crate::cli::auto_render_seconds;
use crate::editor;
use crate::editor::apply_runtime_source;
use crate::effects::FilterKind;
use crate::effects::analog::{Exciter, Tape};
use crate::effects::creative::{
    Adsr, Body, Crystal, DcRemove, Doppler, Fade, FirstOrderLowpass, Glitch, Lofi, Maximizer,
    MultibandComp, Octaver, ParallelComp, Radio, Resonator, Shimmer, Sidechain, Spatial, Stutter,
    SubBass, Telephone, Underwater, Vinyl, Wavefolder,
};
use crate::effects::delays::Delay;
use crate::effects::distortions::{Bitcrush, Distortion};
use crate::effects::dynamics::{
    Dbx160, Fairchild, La2a, NoiseGate, SslComp, TransientShaper, Urei1176,
};
use crate::effects::filters::{Biquad, Comb, Formant, Vowel};
use crate::effects::hardware::{
    AmsProgram, AmsReverb, BuchlaLpg, Lexicon224, MarshallAmp, ProphetFilter, PultecEq, SpaceEcho,
    Tc2290, VoxAc30,
};
use crate::effects::modulations::{
    Chorus, Dimension, DimensionD, Flanger, H3000, Phaser, SmallStone, Vibrato,
};
use crate::effects::offline::{
    OfflineEffectSpec, StereoSide, apply_chain, apply_chain_stereo, granular_settings_for_test,
};
use crate::gui_render;
use crate::language::eval_program;
use crate::model::{NoteMode, Runtime};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};

fn sum_abs_samples(engine: &mut AudioEngine, samples: usize) -> f32 {
    let mut sum = 0.0;
    for _ in 0..samples {
        sum += engine.next_sample().abs();
    }
    sum
}

#[test]
fn parses_and_evaluates_track() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 130) (d :a :src :saw-synth :note (p [c3 eb3]) :gate (p [1 0]) :amp 0.2) (start!)",
    )
    .unwrap();
    assert_eq!(runtime.bpm, 130.0);
    assert!(runtime.running);
    assert_eq!(runtime.tracks.len(), 1);
    assert_eq!(runtime.tracks["a"].notes.len(), 2);
}

#[test]
fn render_is_not_silent() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120) (d :a :src :saw-synth :note (p [c3 g3]) :gate 1 :amp 0.25) (start!)",
    )
    .unwrap();
    let mut engine = AudioEngine::new(runtime, 48_000.0);
    let mut sum = 0.0;
    for _ in 0..48_000 {
        sum += engine.next_sample().abs();
    }
    assert!(sum > 100.0);
}

#[test]
fn explicit_empty_gate_pattern_is_silent() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120) (d :a :src :saw-synth :note (p []) :gate (p []) :amp 0.25) (start!)",
    )
    .unwrap();
    let mut engine = AudioEngine::new(runtime, 48_000.0);
    let mut sum = 0.0;
    for _ in 0..48_000 {
        sum += engine.next_sample().abs();
    }
    assert_eq!(sum, 0.0);
}

#[test]
fn audio_engine_emits_step_events_for_gui_live_clock() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120) (d :a :src :sine-synth :note c3 :gate (p [1 0]) :amp 0.2) (start!)",
    )
    .unwrap();
    let (tx, rx) = mpsc::channel();
    let mut engine = AudioEngine::new(runtime, 48_000.0);
    engine.set_step_sender(tx);

    engine.next_frame();

    assert_eq!(rx.try_recv().unwrap(), 0);
}

#[test]
fn audio_engine_does_not_emit_step_events_when_stopped() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120) (d :a :src :sine-synth :note c3 :gate (p [1 0]) :amp 0.2)",
    )
    .unwrap();
    let (tx, rx) = mpsc::channel();
    let mut engine = AudioEngine::new(runtime, 48_000.0);
    engine.set_step_sender(tx);

    engine.next_frame();

    assert!(rx.try_recv().is_err());
}

#[test]
fn finite_scene_completion_emits_transport_stopped_event() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120)
         (scene :intro :steps 1 :repeat 1
           (d :a :src :sine-synth :note c3 :gate 1 :dur 0.01 :amp 0.2))
         (play-scene :intro)",
    )
    .unwrap();
    let (tx, rx) = mpsc::channel();
    let mut engine = AudioEngine::new(runtime, 48_000.0);
    engine.set_step_sender(tx);

    engine.next_frame();
    assert_eq!(rx.try_recv().unwrap(), 0);
    for _ in 0..12_000 {
        engine.next_frame();
    }

    assert!(
        rx.try_iter()
            .any(|step| step == crate::audio::TRANSPORT_STOPPED_STEP)
    );
}

#[test]
fn audio_engine_resets_transport_when_live_runtime_revision_changes() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120) (d :a :src :sine-synth :note c3 :gate (p [1 0]) :amp 0.2) (start!)",
    )
    .unwrap();
    let shared = Arc::new(Mutex::new(runtime));
    let (tx, rx) = mpsc::channel();
    let mut engine = AudioEngine::new_shared(shared.clone(), 8.0);
    engine.set_step_sender(tx);

    for _ in 0..4 {
        engine.next_frame();
    }
    let emitted = rx.try_iter().collect::<Vec<_>>();
    assert!(emitted.contains(&0));
    assert!(emitted.iter().any(|step| *step > 0));

    let mut next_runtime = Runtime::new();
    eval_program(
        &mut next_runtime,
        "(bpm 120) (d :a :src :sine-synth :note c3 :gate (p [1 0]) :amp 0.2) (start!)",
    )
    .unwrap();
    next_runtime.transport_revision = 1;
    *shared.lock().expect("runtime lock poisoned") = next_runtime;

    engine.next_frame();

    assert_eq!(rx.try_recv().unwrap(), 0);
}

#[test]
fn gui_live_source_application_accepts_nulls_and_advances_revision() {
    let mut runtime = Runtime::new();
    let revision = runtime.transport_revision;
    let (running, tracks, scenes) = crate::cli::apply_gui_live_source(
        &mut runtime,
        "(scene :intro :repeat 2
           (d :lead
              :src :sine-synth
              :note c3
              :gate 1
              :dur null
              :amp null
              :fx [(filter :type null :cutoff null :res null)])
           (d :lead2
              :src :tri-synth
              :note c4
              :gate (p [1 0])
              :dur 0.12
              :amp 0.16))
         (play-scene :intro)",
    )
    .unwrap();

    assert!(running);
    assert_eq!(tracks, 2);
    assert_eq!(scenes, 1);
    assert_eq!(runtime.transport_revision, revision.wrapping_add(1));
    assert_eq!(runtime.tracks["lead"].amp, 0.2);
    assert_eq!(runtime.tracks["lead2"].amp, 0.16);
    assert_eq!(runtime.scenes["intro"].repeats, 2);
}

#[test]
fn supports_808_drum_sources() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :kick :src :kick-808 :note c1 :gate (p [1 0 0 0]) :dur 0.42 :amp 0.6)
         (d :snare :src :snare-808 :note c3 :gate (p [0 0 1 0]) :dur 0.16 :amp 0.25)
         (d :hat :src :hat-808 :note c6 :gate (p [0 1 0 1]) :dur 0.035 :amp 0.09)
         (d :cowbell :src :cowbell-808 :note c5 :gate (p [0 0 0 1]) :dur 0.12 :amp 0.08)
         (start!)",
    )
    .unwrap();
    assert_eq!(runtime.tracks.len(), 4);

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    let mut sum = 0.0;
    for _ in 0..48_000 {
        sum += engine.next_sample().abs();
    }
    assert!(sum > 20.0);
}

#[test]
fn hat_808_uses_python_style_post_filter_peak_normalization() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120)
         (d :hat :src :hat-808 :note c6 :gate 1 :dur 0.08 :amp 1.0)
         (start!)",
    )
    .unwrap();

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    let mut peak = 0.0_f32;
    for _ in 0..4_800 {
        peak = peak.max(engine.next_sample().abs());
    }
    assert!(peak > 0.25);
}

#[test]
fn snare_808_normalizes_snappy_noise_before_envelope_like_python() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120)
         (d :snare :src :snare-808 :note c3 :gate 1 :dur 0.16 :amp 1.0)
         (start!)",
    )
    .unwrap();

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    let mut peak = 0.0_f32;
    let mut energy = 0.0_f32;
    for _ in 0..4_800 {
        let sample = engine.next_sample();
        peak = peak.max(sample.abs());
        energy += sample.abs();
    }
    assert!(peak > 0.32);
    assert!(energy > 60.0);
}

#[test]
fn hat_909_uses_python_style_post_filter_peak_normalization() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120)
         (d :hat :src :hat-909 :note c6 :gate 1 :dur 0.08 :amp 1.0)
         (start!)",
    )
    .unwrap();

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    let mut peak = 0.0_f32;
    let mut energy = 0.0_f32;
    for _ in 0..4_800 {
        let sample = engine.next_sample();
        peak = peak.max(sample.abs());
        energy += sample.abs();
    }
    assert!(peak > 0.25);
    assert!(energy > 35.0);
}

#[test]
fn hat_78_keeps_python_unattenuated_bandpassed_metal_level() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120)
         (d :hat :src :hat-78 :note c6 :gate 1 :dur 0.08 :amp 1.0)
         (start!)",
    )
    .unwrap();

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    let mut energy = 0.0_f32;
    for _ in 0..4_800 {
        energy += engine.next_sample().abs();
    }
    assert!(energy > 45.0);
}

#[test]
fn scratch_keeps_python_bandpass_without_unused_extra_lowpass() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120)
         (d :scratch :src :scratch :note c4 :gate 1 :dur 0.16 :amp 1.0)
         (start!)",
    )
    .unwrap();

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    let mut previous = 0.0;
    let mut movement = 0.0_f32;
    for _ in 0..4_800 {
        let sample = engine.next_sample();
        movement += (sample - previous).abs();
        previous = sample;
    }
    assert!(movement > 35.0);
}

#[test]
fn snare_synth_keeps_python_eight_khz_noise_band() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120)
         (d :snare :src :snare :note c3 :gate 1 :dur 0.16 :amp 1.0)
         (start!)",
    )
    .unwrap();

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    let mut previous = 0.0;
    let mut movement = 0.0_f32;
    for _ in 0..4_800 {
        let sample = engine.next_sample();
        movement += (sample - previous).abs();
        previous = sample;
    }
    assert!(movement > 42.0);
}

#[test]
fn additive_uses_python_style_peak_normalized_eight_harmonic_sum() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120)
         (d :add :src :additive :note c3 :gate 1 :dur 0.10 :amp 1.0
            :harmonics [1 1 1 1 1 1 1 1])
         (start!)",
    )
    .unwrap();

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    let mut peak = 0.0_f32;
    for _ in 0..4_800 {
        peak = peak.max(engine.next_sample().abs());
    }
    assert!(peak > 0.25);
}

#[test]
fn bass_slap_uses_python_sized_pluck_transient() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120)
         (d :bass :src :bass-slap :note c2 :gate 1 :dur 0.12 :amp 1.0)
         (start!)",
    )
    .unwrap();

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    let mut peak = 0.0_f32;
    for _ in 0..1_000 {
        peak = peak.max(engine.next_sample().abs());
    }
    assert!(peak < 0.62);
}

#[test]
fn parses_oscillator_waveform_sources() {
    let sources = [
        "sine-synth",
        "square-synth",
        "saw-synth",
        "tri-synth",
        "pulse",
        "morph",
        "supersaw",
        "wavetable",
        "fm-op",
        "additive",
        "sync",
        "pwm-sweep",
        "harsh",
        "chip",
        "pluck",
        "strings",
        "brass",
        "organ",
        "bell",
        "glass",
        "vocal",
        "breath",
        "pad-wash",
        "click",
        "kick-synth",
        "snare",
        "hat",
        "kick-808",
        "snare-808",
        "hat-808",
        "cowbell-808",
        "kick-909",
        "snare-909",
        "hat-909",
        "kick-78",
        "snare-78",
        "hat-78",
        "kick-707",
        "snare-707",
        "clap",
        "cymbal-crash",
        "cymbal-ride",
        "tom",
        "rimshot",
        "shaker",
        "woodblock",
        "cowbell",
        "zap",
        "scratch",
        "impact",
        "bass-slap",
        "piano-electric",
        "drone-dark",
        "noise-white",
        "noise-pink",
        "noise-brown",
        "noise-blue",
        "noise-purple",
        "sample",
    ];
    let mut runtime = Runtime::new();
    let source = sources
        .iter()
        .enumerate()
        .map(|(idx, source)| {
            format!(
                "(d :osc{} :src :{} :note c3 :gate 1 :dur 0.02 :amp 0.01)",
                idx, source
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    eval_program(&mut runtime, &source).unwrap();
    assert_eq!(runtime.tracks.len(), sources.len());
}

#[test]
fn sample_waveform_plays_sample_data_without_synth_substitution() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :hit
            :src :sample
            :sample-data [1 0.5 -0.25 0]
            :gate 1
            :dur 0.02
            :amp 0.5)
         (start!)",
    )
    .unwrap();

    assert_eq!(runtime.tracks["hit"].sample_data.len(), 4);
    let mut engine = AudioEngine::new(runtime, 48_000.0);
    let center_pan_gain = 0.5_f32.sqrt();
    for expected in [1.0, 0.5, -0.25, 0.0] {
        let frame = engine.next_frame();
        let raw = expected * 0.5 * center_pan_gain * 0.8;
        let clipped = raw / (1.0_f32 + raw.abs());
        assert!((frame[0] - clipped).abs() < 0.000_1);
        assert!((frame[1] - clipped).abs() < 0.000_1);
    }
    assert_eq!(engine.next_frame(), [0.0, 0.0]);
}

#[test]
fn parses_python_oscillator_parameters() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :osc
            :src :fm-op
            :note c3
            :gate 1
            :pulse-width 0.27
            :morph 0.72
            :gain 1.4
            :detune-cents -5
            :phase 0.25
            :unison 4
            :unison-detune 13
            :unison-spread 0.8
            :fm-ratio 3.5
            :fm-depth 2.25
            :harmonics [1 0.25 0.125 0.0625])",
    )
    .unwrap();
    let params = &runtime.tracks["osc"].oscillator;
    assert_eq!(params.unison, 4);
    assert_eq!(params.pulse_width, 0.27);
    assert_eq!(params.morph_pos, 0.72);
    assert_eq!(params.gain, 1.4);
    assert_eq!(params.detune_cents, -5.0);
    assert_eq!(params.phase, 0.25);
    assert_eq!(params.unison_detune, 13.0);
    assert_eq!(params.unison_spread, 0.8);
    assert_eq!(params.fm_ratio, 3.5);
    assert_eq!(params.fm_depth, 2.25);
    assert_eq!(params.harmonics[2], 0.125);
}

#[test]
fn parses_per_hit_numeric_parameter_patterns() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :osc
            :src :fm-op
            :note c3
            :gate 1
            :dur (p [0.4 0.3])
            :amp (p [0.2 0.5])
            :detune-cents (p [-5 7])
            :phase (p [0.25 1.25])
            :pulse-width (p [0.2 0.8])
            :morph (p [0.1 0.9])
            :gain (p [0.5 1.5])
            :unison (p [1 3])
            :unison-detune (p [4 8])
            :unison-spread (p [0.2 0.7])
            :fm-ratio (p [1.5 3.0])
            :fm-depth (p [2.0 4.0]))
         (d :aliases
            :src :sine-synth
            :note c3
            :gate (p [1 0 1 0])
            :dur (gate-seq [0.5 0.25])
            :amp (gs [0.4 0.3 0.2 0.1])
            :gain (g [1.0 0.8])
            :unison (gs [2 4]))",
    )
    .unwrap();

    let patterns = &runtime.tracks["osc"].param_patterns;
    assert_eq!(patterns.dur_seconds.as_ref().unwrap(), &vec![0.4, 0.3]);
    assert_eq!(patterns.amp.as_ref().unwrap(), &vec![0.2, 0.5]);
    assert_eq!(patterns.detune_cents.as_ref().unwrap(), &vec![-5.0, 7.0]);
    assert_eq!(patterns.phase.as_ref().unwrap(), &vec![0.25, 0.25]);
    assert_eq!(patterns.pulse_width.as_ref().unwrap(), &vec![0.2, 0.8]);
    assert_eq!(patterns.morph_pos.as_ref().unwrap(), &vec![0.1, 0.9]);
    assert_eq!(patterns.gain.as_ref().unwrap(), &vec![0.5, 1.5]);
    assert_eq!(patterns.unison.as_ref().unwrap(), &vec![1, 3]);
    assert_eq!(patterns.unison_detune.as_ref().unwrap(), &vec![4.0, 8.0]);
    assert_eq!(patterns.unison_spread.as_ref().unwrap(), &vec![0.2, 0.7]);
    assert_eq!(patterns.fm_ratio.as_ref().unwrap(), &vec![1.5, 3.0]);
    assert_eq!(patterns.fm_depth.as_ref().unwrap(), &vec![2.0, 4.0]);

    let alias_patterns = &runtime.tracks["aliases"].param_patterns;
    assert_eq!(alias_patterns.dur_seconds.as_ref().unwrap(), &vec![0.5, 0.25]);
    assert_eq!(
        alias_patterns.amp.as_ref().unwrap(),
        &vec![0.4, 0.3, 0.2, 0.1]
    );
    assert_eq!(alias_patterns.gain.as_ref().unwrap(), &vec![1.0, 0.8]);
    assert_eq!(alias_patterns.unison.as_ref().unwrap(), &vec![2, 4]);
}

#[test]
fn parameter_patterns_advance_on_gate_hits_not_rests() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120)
         (d :lead
            :src :sine-synth
            :note c3
            :gate (p [1 0 1])
            :dur 0.03
            :amp (p [1 0]))
         (start!)",
    )
    .unwrap();

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    let first_hit = sum_abs_samples(&mut engine, 6_000);
    let rest = sum_abs_samples(&mut engine, 6_000);
    let second_hit = sum_abs_samples(&mut engine, 6_000);

    assert!(first_hit > 100.0);
    assert!(rest < first_hit * 0.05);
    assert!(second_hit < first_hit * 0.05);
}

#[test]
fn null_track_and_effect_parameters_keep_defaults() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :osc
            :src :sine-synth
            :note c3
            :gate 1
            :dur null
            :amp null
            :detune-cents null
            :phase null
            :pulse-width null
            :morph null
            :gain null
            :unison null
            :unison-detune null
            :unison-spread null
            :fm-ratio null
            :fm-depth null
            :harmonics null
            :fx [(filter :type null :cutoff null :res null)])",
    )
    .unwrap();
    let track = &runtime.tracks["osc"];
    let default = crate::model::OscillatorParams::default();
    assert_eq!(track.amp, 0.2);
    assert_eq!(track.dur_seconds, 0.12);
    assert_eq!(track.oscillator.detune_cents, default.detune_cents);
    assert_eq!(track.oscillator.phase, default.phase);
    assert_eq!(track.oscillator.pulse_width, default.pulse_width);
    assert_eq!(track.oscillator.morph_pos, default.morph_pos);
    assert_eq!(track.oscillator.gain, default.gain);
    assert_eq!(track.oscillator.unison, default.unison);
    assert_eq!(track.oscillator.unison_detune, default.unison_detune);
    assert_eq!(track.oscillator.unison_spread, default.unison_spread);
    assert_eq!(track.oscillator.fm_ratio, default.fm_ratio);
    assert_eq!(track.oscillator.fm_depth, default.fm_depth);
    assert_eq!(track.oscillator.harmonics, default.harmonics);
    assert_eq!(track.effects.len(), 1);

    eval_program(
        &mut runtime,
        "(d :nil-compat :src :sine-synth :note c3 :gate 1 :amp nil)",
    )
    .unwrap();
    assert_eq!(runtime.tracks["nil-compat"].amp, 0.2);
}

#[test]
fn unison_spread_renders_stereo_difference() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :wide
            :src :saw-synth
            :note c3
            :gate 1
            :dur 0.3
            :amp 0.2
            :unison 5
            :unison-detune 12
            :unison-spread 1.0)
         (start!)",
    )
    .unwrap();
    let mut engine = AudioEngine::new(runtime, 48_000.0);
    let mut diff_sum = 0.0;
    for _ in 0..4_000 {
        let frame = engine.next_frame();
        diff_sum += (frame[0] - frame[1]).abs();
    }
    assert!(diff_sum > 1.0);
}

#[test]
fn block_repeats_and_moves_to_next_block() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120)
         (block :intro :steps 7 :repeat 1 :next :drop
           (d :a :src :sine-synth :note c3 :gate 1 :amp 0.1))
         (block :drop :steps 16 :repeat 1
           (d :b :src :sine-synth :note c4 :gate 1 :amp 0.1))
         (play-block :intro)",
    )
    .unwrap();

    assert!(runtime.running);
    assert_eq!(runtime.scenes.len(), 2);
    assert_eq!(runtime.scenes["intro"].steps, 7);
    assert!(runtime.tracks.contains_key("a"));
    assert_eq!(
        runtime
            .scene_state
            .as_ref()
            .map(|state| state.current.as_str()),
        Some("intro")
    );

    let shared = Arc::new(Mutex::new(runtime));
    let mut engine = AudioEngine::new_shared(shared.clone(), 48_000.0);
    for _ in 0..50_000 {
        engine.next_sample();
    }

    let snapshot = shared.lock().unwrap().clone();
    assert!(snapshot.tracks.contains_key("b"));
    assert!(!snapshot.tracks.contains_key("a"));
    assert_eq!(
        snapshot
            .scene_state
            .as_ref()
            .map(|state| state.current.as_str()),
        Some("drop")
    );
}

#[test]
fn scene_infers_steps_from_track_patterns_when_omitted() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(scene :intro :repeat 2
           (d :kick :src :kick-808 :note c1 :gate (p [1 0 0 0 1 0 0 0]) :amp 0.2)
           (d :hat :src :hat-808 :note c6 :gate (p [0 1 0 1]) :amp 0.05))",
    )
    .unwrap();
    assert_eq!(runtime.scenes["intro"].steps, 8);
}

#[test]
fn scene_steps_of_uses_named_track_cycle() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(scene :intro :steps-of :kick :repeat 2
           (d :kick :src :kick-808 :note c1 :gate (p [1 0 0 0 0 0 0 0 1 0 0 0]) :amp 0.2)
           (d :hat :src :hat-808 :note c6 :gate (p [0 1 0 1]) :amp 0.05))",
    )
    .unwrap();
    assert_eq!(runtime.scenes["intro"].steps, 12);
}

#[test]
fn scene_bars_are_converted_to_sixteen_step_bars() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(scene :intro :bars 2 :repeat 1
           (d :kick :src :kick-808 :note c1 :gate (p [1 0 0 0]) :amp 0.2))",
    )
    .unwrap();
    assert_eq!(runtime.scenes["intro"].steps, 32);
}

#[test]
fn scene_steps_of_rejects_unknown_track() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(scene :intro :steps-of :missing
           (d :kick :src :kick-808 :note c1 :gate (p [1 0 0 0]) :amp 0.2))",
    )
    .unwrap_err();
    assert!(err.contains("unknown track ':missing'"));
}

#[test]
fn scene_without_steps_infers_track_cycle_and_repeat_extends_render_time() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120)
         (scene :intro :repeat 2
           (d :lead
              :src :sine-synth
              :note (p [c3 eb3 g3 bb3])
              :gate (p [1 0 1 0 0 1 0 1])
              :dur 0.12
              :amp 0.16
              :fx [(formant :vowel :a :mix 0.45)])
           (d :lead2
              :src :tri-synth
              :note (p [c3 eb3 g3 bb3])
              :gate (p [1 0 1 0 0 1 0 1])
              :dur 0.12
              :amp 0.16))
         (play-scene :intro)",
    )
    .unwrap();

    assert_eq!(runtime.scenes["intro"].steps, 8);
    assert_eq!(runtime.scenes["intro"].repeats, 2);
    assert_eq!(runtime.scenes["intro"].tracks.len(), 2);
    assert_eq!(auto_render_seconds(&runtime), Some(4.0));
}

#[test]
fn empty_scene_without_steps_errors_instead_of_defaulting_to_sixteen() {
    let mut runtime = Runtime::new();
    let err = eval_program(&mut runtime, "(scene :intro :repeat 1)").unwrap_err();
    assert!(err.contains("scene has nothing to play"));
}

#[test]
fn scene_without_repeat_defaults_to_infinite_repetition() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 124)
         (scene :intro
           (d :kick
              :src :kick-808
              :note c1
              :gate (p [1 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0])
              :dur 0.36
              :amp 0.42))
         (play-scene :intro)",
    )
    .unwrap();

    assert_eq!(runtime.scenes["intro"].steps, 16);
    assert_eq!(runtime.scenes["intro"].repeats, 0);
    assert_eq!(auto_render_seconds(&runtime), None);
}

#[test]
fn delay_matches_python_iir_comb_wet_path() {
    let mut delay = Delay::new(0.1, 0.5, 1.0, 100.0);
    assert!((delay.process(1.0) - 1.0).abs() < 1e-6);
    for _ in 0..9 {
        assert!(delay.process(0.0).abs() < 1e-6);
    }
    assert!((delay.process(0.0) - 0.5).abs() < 1e-6);
}

#[test]
fn comb_matches_python_feed_forward_filter() {
    let mut comb = Comb::new(10.0, 0.7, 0.5, 100.0);
    assert!((comb.process(1.0) - 1.0).abs() < 1e-6);
    assert!((comb.process(0.0) - 0.7).abs() < 1e-6);
    assert!(comb.process(0.0).abs() < 1e-6);
}

#[test]
fn comb_delay_samples_floor_like_python_int() {
    let comb = Comb::new(1.9, 0.7, 0.5, 1_000.0);
    assert_eq!(comb.delay_samples_for_test(), 1);
}

#[test]
fn auto_render_seconds_follows_finite_scene_chain() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120)
         (scene :intro :steps 7 :repeat 2 :next :drop
           (d :a :src :sine-synth :note c3 :gate 1 :amp 0.1))
         (scene :drop :steps 5 :repeat 1
           (d :b :src :sine-synth :note c4 :gate 1 :amp 0.1))
         (play-scene :intro)",
    )
    .unwrap();

    assert_eq!(auto_render_seconds(&runtime), Some(4.375));
}

#[test]
fn auto_render_seconds_rejects_looping_scene_chain() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120)
         (scene :a :repeat 1 :next :b
           (d :a :src :sine-synth :note c3 :gate 1 :amp 0.1))
         (scene :b :repeat 1 :next :a
           (d :b :src :sine-synth :note c4 :gate 1 :amp 0.1))
         (play-scene :a)",
    )
    .unwrap();

    assert_eq!(auto_render_seconds(&runtime), None);
}

#[test]
fn supports_live_lifecycle_and_euclid_forms() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :kick :src :kick-synth :note c2 :gate (euclid 4 16) :amp 0.4)
             (d :acid :src :saw-synth :note (rev (p [c2 eb2 g2])) :gate (euclid-rot 5 16 2))
             (mute :acid)
             (solo :kick)
             (start!)",
    )
    .unwrap();
    assert!(runtime.running);
    assert_eq!(
        runtime.tracks["kick"]
            .gates
            .iter()
            .filter(|value| **value)
            .count(),
        4
    );
    assert!(runtime.tracks["acid"].muted);
    assert!(runtime.tracks["kick"].solo);
    eval_program(&mut runtime, "(unmute :acid) (unsolo :kick) (clear :kick)").unwrap();
    assert!(!runtime.tracks["acid"].muted);
    assert!(!runtime.tracks.contains_key("kick"));
}

#[test]
fn supports_track_clock_division_and_offset() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :slow :src :sine-synth :note (p [c2 e2]) :gate (p [1 0]) :every 2 :offset 1 :amp 0.2)
             (start!)",
    )
    .unwrap();
    let track = &runtime.tracks["slow"];
    assert_eq!(track.step_every, 2);
    assert_eq!(track.step_offset, 1);

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    let mut sum = 0.0;
    for _ in 0..48_000 {
        sum += engine.next_sample().abs();
    }
    assert!(sum > 10.0);
}

#[test]
fn supports_nested_gate_subdivisions() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :hat :src :hat-909 :note c6 :gate (p [1 0 [1 1 1] 0]) :dur 0.03 :amp 0.4)
             (start!)",
    )
    .unwrap();

    let track = &runtime.tracks["hat"];
    assert_eq!(track.gates, vec![true, false, true, false]);
    assert_eq!(
        track.gate_subdivisions,
        vec![vec![true], vec![false], vec![true, true, true], vec![false]]
    );
}

#[test]
fn supports_gate_holds() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate (p [1 (gate-hold 1) 0 (gate-hold 2) 0 0]) :dur 0.05 :amp 0.2)
             (start!)",
    )
    .unwrap();

    let track = &runtime.tracks["lead"];
    assert_eq!(
        track.gate_subdivisions,
        vec![
            vec![true],
            vec![true],
            vec![false],
            vec![true],
            vec![false],
            vec![false],
        ]
    );
    assert_eq!(
        track.gate_holds,
        vec![vec![0], vec![1], vec![0], vec![2], vec![0], vec![0]]
    );
}

#[test]
fn gate_holds_reject_overlapping_hits() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate (p [1 (gate-hold 2) 1 0]) :amp 0.2)",
    )
    .unwrap_err();
    assert!(err.contains("overlaps another hit"));

    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate (p [1 0 0 (gate-hold 1)]) :amp 0.2)",
    )
    .unwrap_err();
    assert!(err.contains("overlaps another hit"));
}

#[test]
fn supports_gate_subdivisions_nested_inside_subdivisions() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :hat :src :hat-909 :note c6 :gate (p [[1 [1 1] 1] [1 [1 [1 1]]]]) :dur 0.02 :amp 0.4)
             (start!)",
    )
    .unwrap();

    let track = &runtime.tracks["hat"];
    assert_eq!(track.gates, vec![true, true]);
    assert_eq!(
        track.gate_subdivisions,
        vec![
            vec![true, false, true, true, true, false],
            vec![true, false, false, false, true, false, true, true],
        ]
    );
}

#[test]
fn supports_hit_indexed_note_sequences() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120)
         (d :lead
            :src :sine-synth
            :note (s [c3 eb3 g3 bb3])
            :gate (p [1 0 [1 1] 0])
            :dur 0.02
            :amp 0.2)
         (start!)",
    )
    .unwrap();

    let track = &runtime.tracks["lead"];
    assert_eq!(track.note_mode, NoteMode::Hit);

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    for _ in 0..23_990 {
        engine.next_sample();
    }
    assert_eq!(engine.note_cursor_for_test("lead"), 3);
}

#[test]
fn self_looping_scene_does_not_clear_voice_state() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120)
         (scene :intro :steps 4 :repeat 1 :next :intro
           (d :lead
              :src :sine-synth
              :note (s [c3 eb3 g3 bb3])
              :gate (p [1 0 1 0])
              :dur 0.25
              :amp 0.2))
         (play-scene :intro)",
    )
    .unwrap();

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    for _ in 0..48_010 {
        engine.next_sample();
    }
    assert!(engine.note_cursor_for_test("lead") >= 3);
}

#[test]
fn supports_gate_slot_indexed_note_sequences() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120)
         (d :lead
            :src :sine-synth
            :note (gs [c3 eb3 g3 bb3])
            :gate (p [1 0 [1 1] 0])
            :dur 0.02
            :amp 0.2)
         (start!)",
    )
    .unwrap();

    let track = &runtime.tracks["lead"];
    assert_eq!(track.note_mode, NoteMode::Tick);

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    for _ in 0..23_990 {
        engine.next_sample();
    }
    assert_eq!(engine.note_cursor_for_test("lead"), 5);
}

#[test]
fn supports_documented_gate_sequence_note_aliases() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :dash-alias :src :sine-synth :note (gate-seq [c3 eb3]) :gate (p [1 [1 1]]) :amp 0.1)
         (d :underscore-alias :src :sine-synth :note (gate_seq [c3 eb3]) :gate (p [1 [1 1]]) :amp 0.1)",
    )
    .unwrap();

    assert_eq!(runtime.tracks["dash-alias"].note_mode, NoteMode::Tick);
    assert_eq!(runtime.tracks["underscore-alias"].note_mode, NoteMode::Tick);
}

#[test]
fn supports_gated_track_effects_per_gate_hit() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
             :src :saw-synth
             :note c3
             :gate (p [[1 1] 1])
             :fx [(filter :type :lowpass :cutoff 900)
                  (on :gate (p [[0 1] 0])
                      (delay :time 0.12 :feedback 0.3 :mix 0.4))])
         (start!)",
    )
    .unwrap();

    let track = &runtime.tracks["lead"];
    assert_eq!(track.effects.len(), 2);
    assert!(track.effects[0].gate_subdivisions.is_none());
    assert_eq!(
        track.effects[1].gate_subdivisions,
        Some(vec![vec![false, true], vec![false]])
    );

    let first_sub_hit = active_effect_specs(&track.effects, 0, 0);
    let second_sub_hit = active_effect_specs(&track.effects, 0, 1);
    let next_step_hit = active_effect_specs(&track.effects, 1, 0);

    assert_eq!(first_sub_hit.len(), 1);
    assert_eq!(second_sub_hit.len(), 2);
    assert_eq!(next_step_hit.len(), 1);
    assert!(matches!(
        second_sub_hit[1],
        crate::effects::EffectSpec::Delay { .. }
    ));
}

#[test]
fn supports_gated_adsr_effects() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
             :src :saw-synth
             :note c3
             :gate (p [[1 1] 1])
             :dur 0.12
             :fx [(on :gate (p [[0 1] 0])
                      (asdr :a 0.001 :d 0.02 :s 0.4 :r 0.03))])
         (start!)",
    )
    .unwrap();

    let track = &runtime.tracks["lead"];
    assert_eq!(track.effects.len(), 1);
    assert!(matches!(
        active_effect_specs(&track.effects, 0, 1)[0],
        crate::effects::EffectSpec::Adsr { .. }
    ));
    assert!(active_effect_specs(&track.effects, 0, 0).is_empty());
}

#[test]
fn nested_gate_subdivisions_trigger_extra_hits() {
    fn render_sum(gate: &str) -> f32 {
        let mut runtime = Runtime::new();
        eval_program(
            &mut runtime,
            &format!(
                "(bpm 120)
                     (d :hat :src :hat-909 :note c6 :gate {} :dur 0.025 :amp 0.8)
                     (start!)",
                gate
            ),
        )
        .unwrap();
        let mut engine = AudioEngine::new(runtime, 48_000.0);
        let mut sum = 0.0;
        for _ in 0..24_000 {
            sum += engine.next_sample().abs();
        }
        sum
    }

    let single = render_sum("(p [1 0 0 0])");
    let subdivided = render_sum("(p [[1 1 1] 0 0 0])");
    assert!(subdivided > single * 1.8);
}

#[test]
fn supports_track_effect_chain() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :acid
                :src :saw-synth
                :note (p [c2 eb2 g2 bb1])
                :gate (euclid 7 16)
                :fx [(filter :type :lowpass :cutoff 900 :res 0.6)
                     (distort :type :tanh :drive 0.4)
                     (bitcrush :bits 7 :rate 3)
                     (delay :time 0.12 :feedback 0.35 :mix 0.25)
                     (wavefolder :folds 2 :gain 1.8)
                     (resonator :freq 180 :decay 0.85 :mix 0.25 :harmonics 5)
                     (tremolo :rate 6 :depth 0.3)])
             (start!)",
    )
    .unwrap();
    assert_eq!(runtime.tracks["acid"].effects.len(), 7);

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    let mut sum = 0.0;
    for _ in 0..48_000 {
        sum += engine.next_sample().abs();
    }
    assert!(sum > 50.0);
}

#[test]
fn supports_python_eq_filter_types() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :eq
            :src :saw-synth
            :note c3
            :gate 1
            :fx [(filter :type :peaking :cutoff 900 :res 0.4 :gain-db 6)
                 (filter :type :low-shelf :cutoff 180 :res 0.2 :gain-db 4)
                 (filter :type :high-shelf :cutoff 3000 :res 0.2 :gain-db -3)])
         (start!)",
    )
    .unwrap();
    assert_eq!(runtime.tracks["eq"].effects.len(), 3);
}

#[test]
fn low_shelf_first_impulse_matches_python_rbj_formula() {
    let sample_rate = 48_000.0_f32;
    let cutoff = 180.0_f32;
    let resonance = 0.2_f32;
    let gain_db = 4.0_f32;
    let mut filter = Biquad::new_with_gain(
        FilterKind::LowShelf,
        cutoff,
        resonance,
        gain_db,
        sample_rate,
    );

    let q = 0.5 + resonance * 11.5;
    let w0 = std::f32::consts::TAU * cutoff / sample_rate;
    let alpha = w0.sin() / (2.0 * q);
    let cos_w0 = w0.cos();
    let a = 10.0_f32.powf(gain_db / 40.0);
    let sq = 2.0 * a.sqrt() * alpha;
    let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + sq);
    let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + sq;

    assert!((filter.process(1.0) - b0 / a0).abs() < 0.000_001);
}

#[test]
fn rectify_distortion_uses_python_style_peak_normalization() {
    let mut effect = Distortion::new(crate::effects::DistortionKind::Rectify, 0.0);
    assert!((effect.process(-0.25) - 1.0).abs() < 0.000_001);
    assert!((effect.process(0.125) - 0.5).abs() < 0.000_001);
    assert!((effect.process(-0.5) - 1.0).abs() < 0.000_001);
}

#[test]
fn flanger_feedback_uses_current_delayed_sample() {
    let mut flanger = Flanger::new(0.01, 0.0001, 0.8, 1.0, 1_000.0);
    assert_eq!(flanger.process(1.0, 1_000.0), 0.0);
    assert!(flanger.process(0.0, 1_000.0) > 0.9);
    assert!(flanger.process(0.0, 1_000.0) > 0.6);
}

#[test]
fn modulation_voice_and_stage_counts_floor_like_python_range() {
    let chorus = Chorus::new(1.5, 0.003, 3.9, 0.5, 48_000.0);
    assert_eq!(chorus.voice_count(), 3);

    let phaser = Phaser::new(0.5, 0.5, 5.9, 0.5);
    assert_eq!(phaser.stage_count(), 5);
}

#[test]
fn dimension_modes_floor_like_python_int() {
    let dimension = Dimension::new(2.9, 48_000.0);
    assert_eq!(dimension.mode_params(), (0.8, 0.002));

    let dimension_d = DimensionD::new(3.9, 48_000.0);
    assert_eq!(dimension_d.mode_params(), (0.8, 0.0025, 0.7));
}

#[test]
fn prophet_filter_allows_python_sub_twenty_hz_cutoff() {
    let sample_rate = 48_000.0;
    let cutoff = 10.0;
    let input = 0.5_f32;
    let mut prophet = ProphetFilter::new(cutoff, 0.0, sample_rate);
    let first = prophet.process(input);

    let g = 2.0 * (std::f32::consts::PI * cutoff / sample_rate).sin();
    let mut s = [0.0_f32; 4];
    for idx in 0..4 {
        let stage_input = if idx == 0 { input } else { s[idx - 1] };
        s[idx] += g * (stage_input.tanh() - s[idx].tanh());
    }

    assert!((first - s[3]).abs() < 0.000_001);
}

#[test]
fn fairchild_uses_python_time_constant_floor_and_alpha() {
    let sample_rate = 10_000.0;
    let mut fairchild = Fairchild::new(0.0, -60.0, 3.9, 1.0, sample_rate);
    let first = fairchild.process(1.0);

    let driven = (1.0_f32 * 1.2).tanh() / 1.2;
    let attack_alpha = (-1.0_f32 / (0.0004 * sample_rate).max(1.0)).exp();
    let env = (1.0 - attack_alpha) * driven.abs();
    let threshold = 10.0_f32.powf(-60.0 / 20.0);
    let gain = (threshold / (env + 1e-9)).sqrt();
    let compressed = driven * gain;
    let expected = compressed + 0.02 * compressed.powi(2) - 0.005 * compressed.powi(3);

    assert!((first - expected).abs() < 0.000_001);
}

#[test]
fn wavefolder_uses_python_style_peak_normalization() {
    let mut effect = Wavefolder::new(1.0, 1.0, 1.0);
    assert!((effect.process(0.25) - 1.0).abs() < 0.000_001);
    let quieter = effect.process(0.125);
    assert!(quieter > 0.45 && quieter < 0.55);
}

#[test]
fn resonator_uses_python_style_wet_peak_normalization() {
    let mut resonator = Resonator::new(200.0, 0.98, 1.0, 4.0, 48_000.0);
    let mut peak = 0.0_f32;
    for sample in [1.0, 0.5, -0.25, 0.125, 0.0, 0.0, 0.0, 0.0] {
        peak = peak.max(resonator.process(sample).abs());
    }
    assert!(peak > 0.9);
    assert!(peak <= 1.0);
}

#[test]
fn oscillator_and_creative_counts_floor_like_python_int() {
    assert_eq!(pluck_delay_samples_for_test(48_000.0, 5_250.0), 9);

    let wavefolder = Wavefolder::new(2.9, 1.0, 1.0);
    assert_eq!(wavefolder.fold_count_for_test(), 2);

    let resonator = Resonator::new(200.0, 0.98, 1.0, 4.9, 48_000.0);
    assert_eq!(resonator.band_count_for_test(), 4);
}

#[test]
fn lofi_uses_python_floor_for_hold_and_bit_depth() {
    let lofi = Lofi::new(0.49, 48_000.0);
    let (hold, levels) = lofi.python_integer_settings();
    assert_eq!(hold, 4);
    assert_eq!(levels, 2.0_f32.powi(10));

    let lofi = Lofi::new(0.54, 48_000.0);
    let (hold, levels) = lofi.python_integer_settings();
    assert_eq!(hold, 5);
    assert_eq!(levels, 2.0_f32.powi(9));
}

#[test]
fn lofi_uses_python_fourth_order_butterworth_lowpass_shape() {
    let sample_rate = 48_000.0;
    let cutoff = 15_000.0;
    let mut lofi = Lofi::new(0.0, sample_rate);
    let first = lofi.process(1.0);

    let expected = [0.541_196_1_f32, 1.306_563_f32]
        .into_iter()
        .map(|q| {
            let w0 = std::f32::consts::TAU * cutoff / sample_rate;
            let alpha = w0.sin() / (2.0 * q);
            let cos_w0 = w0.cos();
            let b0 = (1.0 - cos_w0) / 2.0;
            let a0 = 1.0 + alpha;
            b0 / a0
        })
        .product::<f32>();

    assert!((first - expected).abs() < 0.000_001);
}

#[test]
fn sub_bass_uses_python_sixth_order_butterworth_lowpass() {
    let sample_rate = 48_000.0;
    let mut sub_bass = SubBass::new(1.0, sample_rate);
    let first = sub_bass.process_sub_probe(1.0);

    let expected = [
        0.517_638_1_f32,
        std::f32::consts::FRAC_1_SQRT_2,
        1.931_851_6_f32,
    ]
    .into_iter()
    .map(|q| {
        let w0 = std::f32::consts::TAU * 150.0 / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();
        let b0 = (1.0 - cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        b0 / a0
    })
    .product::<f32>();

    assert!((first - expected).abs() < 0.000_001);
}

#[test]
fn vinyl_wow_does_not_turn_python_read_ahead_half_cycle_into_extra_delay() {
    let sample_rate = 48_000.0;
    let mut vinyl = Vinyl::new(0.0, 0.0, 1.0, sample_rate);
    vinyl.process(0.25, sample_rate);
    vinyl.process(0.5, sample_rate);
    vinyl.set_wow_phase_for_test(0.75);

    let out = vinyl.process(1.0, sample_rate);
    assert!((out - 1.0).abs() < 0.000_001);
}

#[test]
fn vinyl_hiss_uses_python_third_order_butterworth_lowpass() {
    let sample_rate = 48_000.0;
    let cutoff = 8_000.0;
    let mut vinyl = Vinyl::new(0.0, 1.0, 0.0, sample_rate);
    let first = vinyl.process_hiss_probe(1.0);

    let k = (std::f32::consts::PI * cutoff / sample_rate).tan();
    let first_order_b0 = k / (1.0 + k);
    let w0 = std::f32::consts::TAU * cutoff / sample_rate;
    let alpha = w0.sin() / 2.0;
    let cos_w0 = w0.cos();
    let second_order_b0 = ((1.0 - cos_w0) / 2.0) / (1.0 + alpha);

    assert!((first - first_order_b0 * second_order_b0).abs() < 0.000_001);
}

#[test]
fn stutter_uses_python_floor_for_grain_and_repeats() {
    let stutter = Stutter::new(1.9, 2.9, 1.0, 1_000.0);
    assert_eq!(stutter.python_integer_settings(), (1, 2));

    let stutter = Stutter::new(5.5, 3.1, 1.0, 2_000.0);
    assert_eq!(stutter.python_integer_settings(), (11, 3));
}

#[test]
fn telephone_keeps_voice_band_above_high_frequency_hash() {
    let sample_rate = 48_000.0;
    let mut voice_band = Telephone::new(0.5, sample_rate);
    let mut high_band = Telephone::new(0.5, sample_rate);
    let mut voice_sum = 0.0_f32;
    let mut high_sum = 0.0_f32;
    for idx in 0..4_800 {
        let t = idx as f32 / sample_rate;
        let voice = (std::f32::consts::TAU * 1_000.0 * t).sin() * 0.5;
        let high = (std::f32::consts::TAU * 8_000.0 * t).sin() * 0.5;
        voice_sum += voice_band.process(voice).abs();
        high_sum += high_band.process(high).abs();
    }
    assert!(voice_sum > high_sum * 4.0);
}

#[test]
fn telephone_uses_python_floor_for_codec_bit_depth() {
    let telephone = Telephone::new(0.3, 48_000.0);
    assert_eq!(telephone.python_levels(), 2.0_f32.powi(6));

    let telephone = Telephone::new(0.95, 48_000.0);
    assert_eq!(telephone.python_levels(), 2.0_f32.powi(4));
}

#[test]
fn radio_uses_butterworth_style_band_edges() {
    let sample_rate = 48_000.0;
    let low = 500.0;
    let high = 5_000.0;
    let mut radio = Radio::new(0.0, sample_rate);
    let first = radio.process(1.0, sample_rate);

    let hp = [0.541_196_1_f32, 1.306_563_f32]
        .into_iter()
        .map(|q| {
            let w0 = std::f32::consts::TAU * low / sample_rate;
            let alpha = w0.sin() / (2.0 * q);
            let cos_w0 = w0.cos();
            let b0 = (1.0 + cos_w0) / 2.0;
            let a0 = 1.0 + alpha;
            b0 / a0
        })
        .product::<f32>();
    let lp = [0.541_196_1_f32, 1.306_563_f32]
        .into_iter()
        .map(|q| {
            let w0 = std::f32::consts::TAU * high / sample_rate;
            let alpha = w0.sin() / (2.0 * q);
            let cos_w0 = w0.cos();
            let b0 = (1.0 - cos_w0) / 2.0;
            let a0 = 1.0 + alpha;
            b0 / a0
        })
        .product::<f32>();
    let expected = (hp * lp).tanh();

    assert!((first - expected).abs() < 0.000_001);
}

#[test]
fn underwater_strongly_suppresses_bright_signal() {
    let sample_rate = 48_000.0;
    let mut low_effect = Underwater::new(0.75, sample_rate);
    let mut high_effect = Underwater::new(0.75, sample_rate);
    let mut low_sum = 0.0_f32;
    let mut high_sum = 0.0_f32;
    for idx in 0..9_600 {
        let t = idx as f32 / sample_rate;
        let low = (std::f32::consts::TAU * 180.0 * t).sin() * 0.5;
        let high = (std::f32::consts::TAU * 4_000.0 * t).sin() * 0.5;
        low_sum += low_effect.process(low, sample_rate).abs();
        high_sum += high_effect.process(high, sample_rate).abs();
    }
    assert!(low_sum > high_sum * 6.0);
}

#[test]
fn underwater_uses_python_lowpass_order_and_zero_initial_mod_delay() {
    let sample_rate = 48_000.0;
    let depth = 0.5;
    let cutoff = 2_000.0 - depth * 1_800.0;
    let resonance_cutoff = cutoff * 0.8;
    let mut underwater = Underwater::new(depth, sample_rate);
    let first = underwater.process(1.0, sample_rate);

    let lowpass_b0 = |cutoff: f32, q: f32| {
        let w0 = std::f32::consts::TAU * cutoff / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();
        let b0 = (1.0 - cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        b0 / a0
    };
    let sixth_order = [
        0.517_638_1_f32,
        std::f32::consts::FRAC_1_SQRT_2,
        1.931_851_6_f32,
    ]
    .into_iter()
    .map(|q| lowpass_b0(cutoff, q))
    .product::<f32>();
    let expected = sixth_order * lowpass_b0(resonance_cutoff, std::f32::consts::FRAC_1_SQRT_2);

    assert!((first - expected).abs() < 0.000_001);
}

#[test]
fn crystal_feedback_comb_keeps_normalized_sparkle() {
    let mut crystal = Crystal::new(1.0, 0.8, 48_000.0);
    let mut peak = 0.0_f32;
    for idx in 0..256 {
        let sample = if idx == 0 { 1.0 } else { 0.0 };
        peak = peak.max(crystal.process(sample).abs());
    }
    assert!(peak > 0.25);
    assert!(peak <= 1.3);
}

#[test]
fn crystal_comb_delay_floors_like_python_int() {
    let crystal = Crystal::new(0.5, 0.3, 48_000.0);
    assert_eq!(crystal.comb_delay_samples_for_test(), 8);
}

#[test]
fn sidechain_starts_cycle_at_python_phase_zero() {
    let mut sidechain = Sidechain::new(2.0, 0.7, 0.5);
    let first = sidechain.process(1.0, 48_000.0);
    assert!((first - 0.3).abs() < 0.000_001);
}

#[test]
fn dc_remove_uses_python_second_order_butterworth_highpass() {
    let sample_rate = 48_000.0;
    let cutoff = 10.0;
    let mut dc_remove = DcRemove::new(sample_rate);
    let first = dc_remove.process(1.0);

    let w0 = std::f32::consts::TAU * cutoff / sample_rate;
    let alpha = w0.sin() / (2.0 * std::f32::consts::FRAC_1_SQRT_2);
    let cos_w0 = w0.cos();
    let b0 = (1.0 + cos_w0) / 2.0;
    let a0 = 1.0 + alpha;

    assert!((first - b0 / a0).abs() < 0.000_001);
}

#[test]
fn body_uses_python_style_wet_peak_normalization() {
    let mut body = Body::new(0.55, 0.5, 1.0, 48_000.0);
    let mut peak = 0.0_f32;
    for sample in [1.0, 0.5, -0.25, 0.125, 0.0, 0.0, 0.0, 0.0] {
        peak = peak.max(body.process(sample).abs());
    }
    assert!(peak > 0.9);
    assert!(peak <= 1.0);
}

#[test]
fn warmth_lowpass_matches_python_first_order_butterworth_start() {
    let sample_rate = 48_000.0_f32;
    let cutoff = 150.0_f32;
    let mut filter = FirstOrderLowpass::new(cutoff, sample_rate);
    let k = (std::f32::consts::PI * cutoff / sample_rate).tan();
    let expected = k / (1.0 + k);
    assert!((filter.process(1.0) - expected).abs() < 0.000_001);
}

#[test]
fn spatial_uses_python_equal_power_pan_fold_down() {
    let mut left = Spatial::new(0.0, 0.0, 0.0, 48_000.0);
    let mut center = Spatial::new(0.0, 0.5, 0.0, 48_000.0);
    assert!((left.process(1.0) - 0.5).abs() < 0.000_001);
    let expected_center = std::f32::consts::FRAC_1_SQRT_2;
    assert!((center.process(1.0) - expected_center).abs() < 0.000_001);
}

#[test]
fn octaver_uses_pitch_taps_for_up_and_down_layers() {
    let mut octaver = Octaver::new(1.0, 1.0, 48_000.0);
    octaver.process(0.25);
    let (up, down) = octaver.pitch_read_positions();
    assert!((up - 2.0).abs() < 0.000_001);
    assert!((down - 0.5).abs() < 0.000_001);
}

#[test]
fn h3000_mono_uses_python_upshift_without_predelay_feedback() {
    let mut h3000 = H3000::new(1_200.0, 15.0, 0.5, 1.0, 1_000.0);
    assert_eq!(h3000.process(1.0), 1.0);
    assert!((h3000.pitch_read_position() - 2.0).abs() < 0.000_001);
}

#[test]
fn doppler_starts_with_python_approach_pitch_ramp() {
    let mut doppler = Doppler::new(1.0, 0.3, 48_000.0);
    doppler.process(0.25, 48_000.0);
    assert!((doppler.read_position() - 1.3).abs() < 0.000_001);
}

#[test]
fn shimmer_waits_for_python_delay_window_before_wet_tail() {
    let mut shimmer = Shimmer::new(12.0, 0.3, 1.0, 1_000.0);
    for _ in 0..80 {
        assert_eq!(shimmer.process(1.0), 0.0);
    }
    assert!(shimmer.process(1.0).abs() > 0.0);
}

#[test]
fn glitch_reverse_mode_uses_python_slice_reversal() {
    let mut glitch = Glitch::new(1.0, 4.0, 1_000.0);
    glitch.force_mode_for_test(1);
    assert_eq!(
        [1.0, 2.0, 3.0, 4.0].map(|sample| glitch.process(sample)),
        [1.0, 2.0, 3.0, 4.0]
    );
    assert_eq!(
        [5.0, 6.0, 7.0, 8.0].map(|sample| glitch.process(sample)),
        [4.0, 3.0, 2.0, 1.0]
    );
}

#[test]
fn glitch_slice_size_floors_like_python_int() {
    let glitch = Glitch::new(1.0, 1.9, 1_000.0);
    assert_eq!(glitch.slice_samples_for_test(), 1);

    let glitch = Glitch::new(1.0, 5.5, 2_000.0);
    assert_eq!(glitch.slice_samples_for_test(), 11);
}

#[test]
fn haas_delay_samples_floor_like_python_int() {
    let audio = (0..6)
        .map(|idx| [0.0, (idx + 1) as f32])
        .collect::<Vec<_>>();
    let out = apply_chain_stereo(
        audio,
        &[OfflineEffectSpec::Haas {
            delay_ms: 1.9,
            side: StereoSide::Right,
        }],
        1_000.0,
    );

    let right = out.iter().map(|frame| frame[1]).collect::<Vec<_>>();
    assert_eq!(right, vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
}

#[test]
fn tape_stop_uses_python_normalized_read_curve() {
    let audio = (0..10).map(|value| value as f32).collect::<Vec<_>>();
    let out = apply_chain(
        audio,
        &[OfflineEffectSpec::TapeStop { duration_pct: 0.5 }],
        1_000.0,
    );
    let left = out.iter().map(|frame| frame[0]).collect::<Vec<_>>();
    let expected = [0.0, 1.0, 2.0, 3.0, 4.0, 6.6, 5.85, 4.3, 2.25, 0.0];
    for (actual, expected) in left.iter().zip(expected) {
        assert!((actual - expected).abs() < 0.000_01);
    }
}

#[test]
fn granular_stretch_uses_python_output_length_without_extra_tail_grain() {
    let audio = vec![1.0; 300];
    let out = apply_chain(
        audio,
        &[OfflineEffectSpec::GranularStretch {
            rate: 2.0,
            grain_ms: 128.0,
        }],
        1_000.0,
    );
    assert_eq!(out.len(), 300);
    assert!(out[200][0] > 0.0);
    assert_eq!(out[280][0], 0.0);
}

#[test]
fn granular_pitch_spread_resamples_grain_length_like_python() {
    let mut audio = vec![0.0; 1_000];
    audio[87] = 1.0;
    let processed = apply_chain(
        audio,
        &[OfflineEffectSpec::Granular {
            grain_ms: 100.0,
            density: 0.04,
            spray: 0.0,
            pitch_spread: 1.0,
        }],
        1_000.0,
    );
    assert!(processed[131][0].abs() > 0.01);
}

#[test]
fn granular_uses_python_floor_for_grain_size_and_count() {
    assert_eq!(
        granular_settings_for_test(1_000, 64.9, 0.37, 1_000.0),
        (64, 16)
    );
    assert_eq!(
        granular_settings_for_test(1_000, 65.9, 0.37, 1_000.0),
        (65, 16)
    );
}

#[test]
fn noise_gate_filters_python_binary_gate_target_without_extra_gain_smoothing() {
    let mut gate = NoiseGate::new(-40.0, 0.001, 0.05, 1_000.0);
    let mut gain_filter = Biquad::new(FilterKind::Lowpass, 200.0, 0.2, 1_000.0);
    let expected = gain_filter.process(1.0);
    assert!((gate.process(1.0) - expected).abs() < 0.000_001);
}

#[test]
fn transient_shaper_uses_transient_peak_for_python_sustain_mask() {
    let mut shaper = TransientShaper::new(1.5, 0.8, 0.01, 1_000.0);
    let first = shaper.process(1.0);
    assert!((first - 1.0).abs() < 0.000_001);
}

#[test]
fn tape_allows_python_zero_delay_first_saturated_sample() {
    let mut tape = Tape::new(0.5, 0.0, 0.0, 48_000.0);
    let expected = (0.5_f32 * (1.0 + 0.5 * 5.0)).tanh() * (0.8 / (1.0 + 0.5));
    assert!((tape.process(0.5, 48_000.0) - expected).abs() < 0.000_001);
}

#[test]
fn vibrato_zero_delay_reads_current_sample_like_python_interp() {
    let mut vibrato = Vibrato::new(750.0, 0.001, 1_000.0);
    assert_eq!(vibrato.process(1.0, 1_000.0), 0.0);
    assert!((vibrato.process(2.0, 1_000.0) - 2.0).abs() < 0.000_001);
}

#[test]
fn delay_less_than_one_sample_bypasses_like_python() {
    let mut delay = Delay::new(0.0004, 0.5, 1.0, 1_000.0);
    assert_eq!(delay.process(0.25), 0.25);
    assert_eq!(delay.process(-0.5), -0.5);
}

#[test]
fn bitcrush_parameters_floor_like_python_int_conversion() {
    let mut crush = Bitcrush::new(3.9, 2.9);
    assert_eq!(crush.process(0.3), 0.25);
    assert_eq!(crush.process(0.6), 0.25);
    assert_eq!(crush.process(0.6), 0.625);
}

#[test]
fn formant_matches_python_wet_sum_even_when_mix_is_zero() {
    let mut formant = Formant::new(Vowel::A, 0.0, 48_000.0);
    assert!(formant.process(1.0).abs() > 0.0);
}

#[test]
fn exciter_uses_high_order_python_style_highpass_stages() {
    let mut exciter = Exciter::new(1.0, 3_000.0, 48_000.0);
    let mut low_energy = 0.0;
    let mut phase = 0.0_f32;
    for idx in 0..9_600 {
        let sample = (phase * std::f32::consts::TAU).sin() * 0.5;
        phase = (phase + 100.0 / 48_000.0) % 1.0;
        let added = (exciter.process(sample) - sample).abs();
        if idx >= 4_800 {
            low_energy += added;
        }
    }
    assert!(low_energy < 0.01);
}

#[test]
fn small_stone_uses_python_state_cascade_for_first_impulse() {
    let sample_rate = 48_000.0_f32;
    let depth = 0.7_f32;
    let mut phaser = SmallStone::new(0.4, depth, 0.6, false);
    let sweep = 200.0 + 0.5 * depth * 6_000.0;
    let w = (std::f32::consts::PI * sweep / sample_rate).tan();
    let coeff = ((1.0 - w) / (1.0 + w)).clamp(-0.98, 0.98);
    let expected = 0.5 + 0.5 * coeff;
    assert!((phaser.process(1.0, sample_rate) - expected).abs() < 0.000_001);
}

#[test]
fn dimension_mono_starts_without_extra_base_delay_like_python() {
    let mut dimension = Dimension::new(2.0, 48_000.0);
    assert!((dimension.process(1.0, 48_000.0) - 1.0).abs() < 0.000_001);
}

#[test]
fn space_echo_spring_uses_python_delayed_taps_without_immediate_comb_signal() {
    let mut echo = SpaceEcho::new(0.1, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1_000.0);
    assert_eq!(echo.spring_probe(1.0), 0.0);
    for _ in 1..29 {
        assert_eq!(echo.spring_probe(0.0), 0.0);
    }
    assert!((echo.spring_probe(0.0) - 0.15).abs() < 0.000_001);
}

#[test]
fn space_echo_tape_delay_and_modulation_floor_like_python_int() {
    let mut echo = SpaceEcho::new(0.0209, 0.0, 0.0, 0.0, 0.5, 0.0, 1.0, 1_000.0);
    assert_eq!(echo.tape_settings_for_test(), (20, 30));

    echo.set_tape_probe_for_test(10, &[0.0, 0.0, 0.0, 0.25, 0.75]);
    assert_eq!(echo.tape_read_probe_for_test(7.9), 0.25);
}

#[test]
fn tc2290_delay_and_modulation_floor_like_python_int() {
    let mut delay = Tc2290::new(20.9, 0.0, 0.0, 0.0029, 1.0, 1_000.0);
    assert_eq!(delay.settings_for_test(), (20, 122));

    delay.set_probe_for_test(10, &[0.0, 0.0, 0.0, 0.25, 0.75]);
    assert!((delay.read_probe_for_test(7.5) - 0.5).abs() < 0.000_001);
}

#[test]
fn lexicon_delay_lengths_floor_like_python_int_and_zero_predelay_bypasses() {
    let lexicon = Lexicon224::new(0.65, 2.2, 0.4, 0.0, 0.3, 1_000.0);
    let (pre_delay, diffusers, lines) = lexicon.delay_lengths_for_test();
    assert_eq!(pre_delay, 0);
    assert_eq!(diffusers, vec![92, 153, 246, 326]);
    assert_eq!(lines, vec![1012, 1256, 1502, 1775]);
}

#[test]
fn ams_reverb_delays_and_nonlin_gate_floor_like_python_int() {
    let mut ams = AmsReverb::new(0.603, 0.45, AmsProgram::Nonlin, 0.3, 1_000.0);
    assert_eq!(ams.line_lengths_for_test(), vec![29, 37, 43, 53, 67, 79]);

    assert!((ams.nonlin_envelope_for_test() - 1.0).abs() < 0.000_001);
    ams.set_age_for_test(602);
    assert!((ams.nonlin_envelope_for_test() - 0.2).abs() < 0.000_001);
    ams.set_age_for_test(603);
    assert_eq!(ams.nonlin_envelope_for_test(), 0.0);
}

#[test]
fn ssl_comp_uses_python_db_gain_formula_without_double_dividing() {
    let mut ssl = SslComp::new(-20.0, 4.0, 0.01, 100.0, 0.0, 1_000.0);
    let out = ssl.process(1.0);
    let threshold = 10.0_f32.powf(-20.0 / 20.0);
    let alpha = (-1.0_f32).exp();
    let env = (1.0 - alpha) * 1.0;
    let over_db = 20.0 * (env / threshold + 1.0e-12).log10();
    let expected = 10.0_f32.powf((over_db / 4.0 - over_db) / 20.0);
    assert!((out - expected).abs() < 0.000_001);
}

#[test]
fn dbx160_uses_python_db_gain_formula_without_double_dividing() {
    let mut dbx = Dbx160::new(-15.0, 6.0, 1_000.0);
    let out = dbx.process(1.0);
    let threshold = 10.0_f32.powf(-15.0 / 20.0);
    let alpha = (-1.0_f32).exp();
    let env = (1.0 - alpha) * 1.0;
    let over_db = 20.0 * (env / threshold + 1.0e-12).log10();
    let gain = 10.0_f32.powf((over_db / 6.0 - over_db) / 20.0);
    let makeup = 10.0_f32.powf((-15.0 * (1.0 - 1.0 / 6.0)) / 40.0);
    assert!((out - gain * makeup).abs() < 0.000_001);
}

#[test]
fn urei1176_uses_python_db_gain_formula_without_double_dividing() {
    let mut fet = Urei1176::new(0.5, 4.0, 0.0, 0.5, 1_000.0);
    let sample = 0.2_f32;
    let out = fet.process(sample);
    let driven = sample * (1.0 + 0.5 * 3.0);
    let alpha = (-1.0_f32 / 1.0).exp();
    let env = (1.0 - alpha) * driven;
    let threshold = 0.25_f32;
    let over_db = 20.0 * (env / threshold + 1.0e-12).log10();
    let gain = 10.0_f32.powf((over_db / 4.0 - over_db) / 20.0);
    let compressed = driven * gain;
    let expected = (compressed + 0.03 * compressed.powi(3)).clamp(-1.0, 1.0);
    assert!((out - expected).abs() < 0.000_001);
}

#[test]
fn maximizer_uses_python_release_time_floor() {
    let mut maximizer = Maximizer::new(-6.0, 0.0, 1.0, 50.0);
    let ceiling = 10.0_f32.powf(-6.0 / 20.0);
    assert!((maximizer.process(1.0) - ceiling).abs() < 0.000_001);
    let out = maximizer.process(0.25);
    assert!((out - 0.25).abs() < 0.000_001);
}

#[test]
fn fade_uses_python_linspace_endpoints() {
    let mut fade = Fade::new(4.0, 4.0, 0.008, 1_000.0);
    let out = (0..8).map(|_| fade.process(1.0)).collect::<Vec<_>>();
    let expected = [
        0.0,
        1.0 / 3.0,
        2.0 / 3.0,
        1.0,
        1.0,
        2.0 / 3.0,
        1.0 / 3.0,
        0.0,
    ];
    for (actual, expected) in out.iter().zip(expected) {
        assert!((actual - expected).abs() < 0.000_001);
    }
}

#[test]
fn adsr_shapes_attack_decay_sustain_and_release() {
    let mut adsr = Adsr::new(0.002, 0.002, 0.5, 0.002, 0.006, 1_000.0);
    let out = (0..9).map(|_| adsr.process(1.0)).collect::<Vec<_>>();
    let expected = [0.0, 0.5, 1.0, 0.75, 0.5, 0.5, 0.5, 0.25, 0.0];
    for (actual, expected) in out.iter().zip(expected) {
        assert!((actual - expected).abs() < 0.000_001);
    }
}

#[test]
fn multiband_comp_uses_python_style_steeper_crossover_filters() {
    let mut comp = MultibandComp::new(-20.0, -18.0, -15.0, 200.0, 4_000.0, 48_000.0);
    let mut phase = 0.0_f32;
    let mut low_leakage = 0.0;
    for idx in 0..9_600 {
        let sample = (phase * std::f32::consts::TAU).sin();
        phase = (phase + 8_000.0 / 48_000.0) % 1.0;
        let (low, _, _) = comp.band_probe(sample);
        if idx >= 4_800 {
            low_leakage += low.abs();
        }
    }
    assert!(low_leakage < 0.01);
}

#[test]
fn parallel_comp_uses_python_style_peak_makeup_without_soft_clip() {
    let mut comp = ParallelComp::new(-25.0, 8.0, 1.0, 1_000.0);
    assert!((comp.process(1.0) - 0.9).abs() < 0.000_001);
}

#[test]
fn marshall_mid_scoop_uses_python_peaking_eq_coefficients() {
    let sample_rate = 48_000.0_f32;
    let mut amp = MarshallAmp::new(0.0, 0.0, 0.0, sample_rate);
    let w0 = std::f32::consts::TAU * 600.0 / sample_rate;
    let alpha = w0.sin() / (2.0 * 1.5);
    let a_gain = 10.0_f32.powf(-3.0 / 40.0);
    let expected = (1.0 + alpha * a_gain) / (1.0 + alpha / a_gain);
    assert!((amp.mid_scoop_probe(1.0) - expected).abs() < 0.000_001);
}

#[test]
fn vox_ac30_top_boost_uses_python_peaking_eq_coefficients() {
    let sample_rate = 48_000.0_f32;
    let treble = 0.6_f32;
    let mut amp = VoxAc30::new(0.0, treble, 0.0, sample_rate);
    let w0 = std::f32::consts::TAU * 2_500.0 / sample_rate;
    let alpha = w0.sin() / (2.0 * 1.5);
    let a_gain = 10.0_f32.powf((treble * 6.0) / 40.0);
    let expected = (1.0 + alpha * a_gain) / (1.0 + alpha / a_gain);
    assert!((amp.top_boost_probe(1.0) - expected).abs() < 0.000_001);
}

#[test]
fn pultec_low_bump_uses_python_peaking_eq_coefficients() {
    let sample_rate = 48_000.0_f32;
    let low_boost = 0.8_f32;
    let low_freq = 60.0_f32;
    let mut eq = PultecEq::new(low_boost, 0.5, low_freq, 0.0, 0.0, 8_000.0, sample_rate);
    let bump_freq = low_freq * 1.5;
    let w0 = std::f32::consts::TAU * bump_freq / sample_rate;
    let alpha = w0.sin() / (2.0 * 2.0);
    let a_gain = 10.0_f32.powf((low_boost * 3.0).min(4.0) / 40.0);
    let expected = (1.0 + alpha * a_gain) / (1.0 + alpha / a_gain);
    assert!((eq.low_bump_probe(1.0) - expected).abs() < 0.000_001);
}

#[test]
fn la2a_uses_python_optical_timing_floor() {
    let mut la2a = La2a::new(0.5, false, 50.0);
    let sample = 1.0_f32;
    let out = la2a.process(sample);
    let threshold = 10.0_f32.powf((-20.0 + (1.0 - 0.5) * 15.0) / 20.0);
    let alpha = (-1.0_f32).exp();
    let env = (1.0 - alpha) * sample;
    let over = env / threshold;
    let gain = 1.0 / (1.0 + (over - 1.0) * (3.0 - 1.0) / 3.0);
    assert!((out - gain).abs() < 0.000_001);
}

#[test]
fn buchla_lpg_uses_python_double_exponential_vactrol_filter_start() {
    let mut lpg = BuchlaLpg::new(0.7, 0.3, 0.2, 1_000.0);
    let env = 0.7_f32;
    let cutoff = 50.0 + env * (450.0 - 50.0);
    let g = 2.0 * (std::f32::consts::PI * cutoff / 1_000.0).sin();
    let expected = g * g * env;
    assert!((lpg.process(1.0) - expected).abs() < 0.000_001);
}

#[test]
fn supports_modular_effect_families() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :wide
                :src :saw-synth
                :note (p [c2 eb2 g2 bb2])
                :gate (euclid 7 16)
                :amp 0.18
                :fx [(comb :delay-ms 4 :feedback 0.65 :mix 0.35)
                     (formant :vowel :a :mix 0.4)
                     (chorus :rate 0.8 :depth 0.004 :voices 3 :mix 0.35)
                     (ensemble :voices 5 :depth 0.004 :rate 0.8)
                     (ce1-chorus :rate 0.5 :intensity 0.45)
                     (re301-chorus :rate 0.6 :depth 0.45 :tone 0.55)
                     (dimension :mode 2)
                     (dimension-d :mode 3)
                     (flanger :rate 0.25 :depth 0.002 :feedback 0.4 :mix 0.25)
                     (phaser :rate 0.5 :depth 0.6 :stages 4 :mix 0.4)
                     (small-stone :rate 0.4 :depth 0.7 :feedback 0.6 :color :on)
                     (vibrato :rate 5 :depth 0.002)
                     (ring-mod :freq 90 :mix 0.2)
                     (compressor :threshold -18 :ratio 3 :attack 0.005 :release 0.08)
                     (dbx160 :threshold -15 :ratio 6)
                     (1176 :input-gain 0.45 :ratio 8 :attack 0.25 :release 0.45)
                     (la2a :peak-reduction 0.5 :mode :compress)
                     (gate :threshold -55 :attack 0.001 :release 0.04)
                     (transient :attack-gain 1.5 :sustain-gain 0.8 :sensitivity 0.01)
                     (reverb :decay 0.25 :mix 0.15)
                     (spring-reverb :decay 1.2 :tone 0.55 :mix 0.18 :drip 0.35)
                     (emt-plate :decay 1.8 :damping 0.45 :mix 0.12 :pre-delay-ms 12)
                     (lexicon-224 :size 0.65 :decay 2.2 :damping 0.4 :pre-delay-ms 10 :mix 0.12)
                     (ams-reverb :decay 0.6 :damping 0.45 :program :nonlin :mix 0.1)
                     (tube :drive 0.35 :asymmetry 0.15)
                     (exciter :amount 0.2 :cutoff 3000)
                     (moog :cutoff 1200 :res 0.45 :drive 0.15)
                     (sem-filter :type :bandpass :cutoff 900 :res 0.25)
                     (ms20 :cutoff 1500 :res 0.35)])
             (start!)",
    )
    .unwrap();
    assert_eq!(runtime.tracks["wide"].effects.len(), 29);

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    let mut sum = 0.0;
    for _ in 0..48_000 {
        sum += engine.next_sample().abs();
    }
    assert!(sum > 0.1);
}

#[test]
fn supports_distinct_hardware_alias_ports() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :curtis
                :src :saw-synth
                :note (p [c2 eb2 g2 bb2])
                :gate (euclid 8 16)
                :amp 0.16
                :fx [(prophet-filter :cutoff 1800 :res 0.35)
                     (obxa-filter :type :bandpass :cutoff 1000 :res 0.25)])
             (d :wasp
                :src :square-synth
                :note (p [c2 g2])
                :gate (euclid 5 16)
                :amp 0.13
                :fx [(wasp-filter :cutoff 1600 :res 0.45)
                     (arp-ring-mod :freq 110 :depth 0.28 :diode-curve 0.3)])
             (start!)",
    )
    .unwrap();
    assert_eq!(runtime.tracks["curtis"].effects.len(), 2);
    assert_eq!(runtime.tracks["wasp"].effects.len(), 2);

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    let mut sum = 0.0;
    for _ in 0..48_000 {
        sum += engine.next_sample().abs();
    }
    assert!(sum > 5.0);
}

#[test]
fn parses_every_legacy_python_effect_name() {
    let live_effects = [
        "(1176)",
        "(303-filter)",
        "(adsr)",
        "(ams-reverb)",
        "(arp-ring-mod)",
        "(bitcrush)",
        "(body)",
        "(buchla-lpg)",
        "(ce1-chorus)",
        "(chorus)",
        "(comb)",
        "(compressor)",
        "(crystal)",
        "(dbx160)",
        "(dc-remove)",
        "(delay)",
        "(dimension)",
        "(dimension-d)",
        "(distortion)",
        "(doppler)",
        "(emt-plate)",
        "(ensemble)",
        "(exciter)",
        "(fade)",
        "(fairchild)",
        "(fender-twin)",
        "(filter)",
        "(flanger)",
        "(formant)",
        "(gate)",
        "(glitch)",
        "(h3000)",
        "(harmonic-enhance)",
        "(harmonizer)",
        "(juno-hpf)",
        "(la2a)",
        "(lexicon-224)",
        "(limiter)",
        "(lofi)",
        "(marshall-amp)",
        "(maximizer)",
        "(moog-ladder)",
        "(ms20-filter)",
        "(multiband-comp)",
        "(neve-preamp)",
        "(obxa-filter)",
        "(octaver)",
        "(parallel-comp)",
        "(phaser)",
        "(pitch-shift)",
        "(prophet-filter)",
        "(pultec-eq)",
        "(radio)",
        "(re301-chorus)",
        "(resonator)",
        "(reverb)",
        "(ring-mod)",
        "(sem-filter)",
        "(shimmer)",
        "(sidechain)",
        "(small-stone)",
        "(space-echo)",
        "(spatial)",
        "(spring-reverb)",
        "(ssl-comp)",
        "(studer-tape)",
        "(stutter)",
        "(sub-bass)",
        "(tape)",
        "(tc2290)",
        "(telephone)",
        "(transient-shaper)",
        "(tremolo)",
        "(tube-saturation)",
        "(underwater)",
        "(vibrato)",
        "(vinyl)",
        "(vox-ac30)",
        "(warmth)",
        "(wasp-filter)",
        "(wavefolder)",
    ];
    let post_effects = [
        "(autopan)",
        "(freq-shift)",
        "(granular)",
        "(granular-stretch)",
        "(haas)",
        "(ping-pong-delay)",
        "(reverse)",
        "(spectral-freeze)",
        "(stereo-imager)",
        "(stereo-widen)",
        "(tape-stop)",
        "(width-enhance)",
    ];

    let mut runtime = Runtime::new();
    let source = format!(
        "(d :coverage :src :sine-synth :note c3 :gate 1 :fx [{}]) (post-fx [{}])",
        live_effects.join(" "),
        post_effects.join(" ")
    );
    eval_program(&mut runtime, &source).unwrap();
    assert_eq!(runtime.tracks["coverage"].effects.len(), live_effects.len());
    assert_eq!(runtime.post_effects.len(), post_effects.len());
}

#[test]
fn supports_creative_algorithm_ports() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :creative
                :src :square-synth
                :note (p [c2 eb2 g2 bb2])
                :gate (euclid 9 16)
                :amp 0.16
                :fx [(lofi :amount 0.45)
                     (vinyl :crackle 0.25 :hiss 0.08 :wow 0.18)
                     (sub-bass :mix 0.25)
                     (sidechain :rate 2 :depth 0.45 :shape 0.5)
                     (radio :intensity 0.25)
                     (telephone :quality 0.3)
                     (underwater :depth 0.25)
                     (crystal :brightness 0.5 :decay 0.25)])
             (start!)",
    )
    .unwrap();
    assert_eq!(runtime.tracks["creative"].effects.len(), 8);

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    let mut sum = 0.0;
    for _ in 0..48_000 {
        sum += engine.next_sample().abs();
    }
    assert!(sum > 10.0);
}

#[test]
fn supports_hardware_algorithm_ports() {
    let mut runtime = Runtime::new();
    eval_program(
            &mut runtime,
            "(d :hardware
                :src :saw-synth
                :note (p [c2 eb2 g2 bb1])
                :gate (euclid 7 16)
                :amp 0.17
                :fx [(tape :saturation 0.35 :wow 0.2 :flutter 0.15)
                     (studer-tape :input-level 0.45 :speed 1 :bias 0.55)
                     (tb-303 :cutoff 900 :res 0.65 :env-mod 0.6 :accent 0.4 :decay 0.25)
                     (space-echo :time 0.12 :feedback 0.35 :wow 0.2 :flutter 0.12 :tone 0.45 :spring-mix 0.15 :mix 0.25)
                     (fairchild :input-gain 0.4 :threshold -22 :time-constant 3 :mix 0.7)
                     (ssl-comp :threshold -16 :ratio 4 :attack-ms 10 :release-ms 120 :makeup-db 1)
                     (neve-preamp :gain 0.35 :warmth 0.45)
                     (marshall-amp :gain 0.45 :tone 0.55 :presence 0.35)
                     (vox-ac30 :gain 0.35 :treble 0.65 :cut 0.3)
                     (fender-twin :volume 0.35 :treble 0.55 :bass 0.45 :reverb-mix 0.16)
                     (pultec-eq :low-boost 0.35 :low-atten 0.18 :low-freq 60 :high-boost 0.24 :high-atten 0.08 :high-freq 8000)
                     (tc2290 :time-ms 180 :feedback 0.32 :mod-rate 0.4 :mod-depth 0.002 :mix 0.22)
                     (h3000 :detune-cents 10 :delay-ms 14 :feedback 0.08 :mix 0.22)
                     (juno-hpf :cutoff 220 :res 0.2)
                     (buchla-lpg :strike 0.8 :decay 0.24 :res 0.25)])
             (start!)",
        )
        .unwrap();
    assert_eq!(runtime.tracks["hardware"].effects.len(), 15);

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    let mut sum = 0.0;
    for _ in 0..48_000 {
        sum += engine.next_sample().abs();
    }
    assert!(sum > 10.0);
}

#[test]
fn supports_time_pitch_glitch_ports() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :time
                :src :tri-synth
                :note (p [c3 eb3 g3 bb2])
                :gate (euclid 7 16)
                :dur 0.16
                :amp 0.18
                :fx [(dc-remove)
                     (pitch-shift :semitones 7 :mix 0.25)
                     (harmonizer :interval 5 :mix 0.22)
                     (octaver :octave-up 0.15 :octave-down 0.25)
                     (shimmer :shift-semitones 12 :feedback 0.25 :mix 0.22)
                     (stutter :grain-ms 35 :repeats 2 :mix 0.3)
                     (glitch :density 0.2 :slice-ms 25)
                     (fade :fade-in-ms 8 :fade-out-ms 60 :duration 0.16)
                     (doppler :speed 0.8 :depth 0.2)])
             (start!)",
    )
    .unwrap();
    assert_eq!(runtime.tracks["time"].effects.len(), 9);

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    let mut sum = 0.0;
    for _ in 0..48_000 {
        sum += engine.next_sample().abs();
    }
    assert!(sum > 5.0);
}

#[test]
fn supports_mastering_body_ports() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :master
                :src :saw-synth
                :note (p [c2 eb2 g2 bb1])
                :gate (euclid 8 16)
                :dur 0.12
                :amp 0.16
                :fx [(maximizer :ceiling -0.5 :warmth 0.3 :release-ms 45)
                     (multiband-comp :low-thresh -20 :mid-thresh -18 :high-thresh -15)
                     (harmonic-enhance :low-harmonics 0.25 :high-harmonics 0.18 :air 0.12)
                     (parallel-comp :threshold -24 :ratio 8 :mix 0.35)
                     (body :size 0.55 :tone 0.55 :mix 0.25)
                     (warmth :amount 0.35)
                     (spatial :room-size 0.45 :position 0.45 :height 0.25)])
             (start!)",
    )
    .unwrap();
    assert_eq!(runtime.tracks["master"].effects.len(), 7);

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    let mut sum = 0.0;
    for _ in 0..48_000 {
        sum += engine.next_sample().abs();
    }
    assert!(sum > 10.0);
}

#[test]
fn distortion_drive_above_one_is_not_silently_clamped_to_one() {
    let mut drive_one = Distortion::new(crate::effects::DistortionKind::Tanh, 1.0);
    let mut drive_ten = Distortion::new(crate::effects::DistortionKind::Tanh, 10.2);

    let one = drive_one.process(0.1);
    let ten = drive_ten.process(0.1);

    assert!((one - ten).abs() > 0.1);
}

#[test]
fn track_fx_distortion_drive_changes_rendered_signal() {
    fn render_abs_sum(drive: f32) -> f32 {
        let mut runtime = Runtime::new();
        eval_program(
            &mut runtime,
            &format!(
                "(bpm 124)
                 (d :track-id
                    :src :brass
                    :note (p [c3 d3 d4 c6])
                    :gate (p [1 1 1 1 1 1 1 1])
                    :dur 0.6
                    :amp 0.7
                    :detune-cents null
                    :phase null
                    :pulse-width null
                    :morph null
                    :gain null
                    :unison 3
                    :unison-detune null
                    :unison-spread null
                    :fm-ratio 3.6
                    :fm-depth null
                    :harmonics null
                    :fx [(distort :type tanh :drive {})])
                 (start!)",
                drive
            ),
        )
        .unwrap();
        let mut engine = AudioEngine::new(runtime, 48_000.0);
        let mut sum = 0.0;
        for _ in 0..24_000 {
            sum += engine.next_sample().abs();
        }
        sum
    }

    let drive_one = render_abs_sum(1.0);
    let drive_ten = render_abs_sum(10.2);

    assert!((drive_one - drive_ten).abs() > drive_one * 0.2);
}

#[test]
fn supports_post_render_effects() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :post
                :src :saw-synth
                :note (p [c2 eb2 g2 bb1])
                :gate (euclid 8 16)
                :dur 0.10
                :amp 0.16)
             (post-fx [(reverse :mix 0.25)
                       (tape-stop :duration-pct 0.3)
                       (granular :grain-ms 35 :density 0.35 :spray 0.2 :pitch-spread 0.1)
                       (granular-stretch :rate 0.8 :grain-ms 45)
                       (spectral-freeze :freeze-pos 0.35 :sustain 0.6 :mix 0.25)])
             (start!)",
    )
    .unwrap();
    assert_eq!(runtime.post_effects.len(), 5);

    let path = PathBuf::from(format!(
        "/tmp/glitchlisp-post-fx-test-{}.wav",
        std::process::id()
    ));
    let stats = render(runtime, 1.0, path).unwrap();
    assert_eq!(stats.frames, 48_000);
    assert!(stats.rms > 0.001);
}

#[test]
fn post_fx_accepts_live_track_effects() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :post
                :src :saw-synth
                :note c3
                :gate 1
                :dur 0.10
                :amp 0.16)
             (post-fx [(tape :saturation 0.35 :wow 0.08 :flutter 0.05)
                       (filter :type :lowpass :cutoff 1800 :res 0.25)
                       (delay :time 0.04 :feedback 0.2 :mix 0.2)])
             (start!)",
    )
    .unwrap();
    assert_eq!(runtime.post_effects.len(), 3);

    let path = PathBuf::from(format!(
        "/tmp/glitchlisp-live-post-fx-test-{}.wav",
        std::process::id()
    ));
    let stats = render(runtime, 1.0, path).unwrap();
    assert_eq!(stats.frames, 48_000);
    assert!(stats.rms > 0.001);
}

#[test]
fn supports_stereo_post_render_effects() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :wide
                :src :square-synth
                :note (p [c2 g2 bb2 eb3])
                :gate (euclid 10 16)
                :dur 0.08
                :amp 0.12
                :fx [(filter :type :lowpass :cutoff 1400 :res 0.35)])
             (post-fx [(haas :delay-ms 12 :side :right)
                       (stereo-widen :width 0.85)
                       (stereo-imager :width 1.35 :bass-mono-freq 220)
                       (width-enhance :low-width 0.7 :high-width 1.55 :crossover 900)
                       (freq-shift :shift-hz 30 :mix 0.25)
                       (autopan :rate 1.4 :depth 0.75)
                       (ping-pong-delay :time 0.08 :feedback 0.45 :mix 0.5)])
             (start!)",
    )
    .unwrap();
    assert_eq!(runtime.post_effects.len(), 7);

    let path = PathBuf::from(format!(
        "/tmp/glitchlisp-stereo-post-fx-test-{}.wav",
        std::process::id()
    ));
    let stats = render(runtime, 1.0, path).unwrap();
    assert_eq!(stats.frames, 48_000);
    assert!(stats.rms > 0.001);
}

#[test]
fn freq_shift_uses_single_sideband_hilbert_path() {
    let sample_rate = 4_096.0;
    let input_hz = 512.0;
    let shift_hz = 128.0;
    let audio = (0..8_192)
        .map(|idx| (std::f32::consts::TAU * input_hz * idx as f32 / sample_rate).sin())
        .collect::<Vec<_>>();

    let shifted = apply_chain(
        audio,
        &[OfflineEffectSpec::FreqShift { shift_hz, mix: 1.0 }],
        sample_rate,
    );
    let mono = shifted
        .iter()
        .map(|frame| (frame[0] + frame[1]) * 0.5)
        .collect::<Vec<_>>();

    let upper = tone_energy(&mono, input_hz + shift_hz, sample_rate, 2_048, 6_144);
    let lower = tone_energy(&mono, input_hz - shift_hz, sample_rate, 2_048, 6_144);
    assert!(upper > lower * 8.0);
}

#[test]
fn spectral_freeze_uses_fft_magnitudes_instead_of_repeating_source_slice() {
    let mut audio = vec![0.0; 8_192];
    audio[1_024] = 1.0;

    let frozen = apply_chain(
        audio,
        &[OfflineEffectSpec::SpectralFreeze {
            freeze_pos: 0.0,
            sustain: 1.0,
            mix: 1.0,
        }],
        48_000.0,
    );

    let dense_samples = frozen[..2_048]
        .iter()
        .filter(|frame| ((frame[0] + frame[1]) * 0.5).abs() > 0.0001)
        .count();
    assert!(dense_samples > 512);
}

fn tone_energy(audio: &[f32], freq: f32, sample_rate: f32, start: usize, end: usize) -> f32 {
    let mut re = 0.0;
    let mut im = 0.0;
    for (offset, sample) in audio[start..end].iter().enumerate() {
        let phase = std::f32::consts::TAU * freq * offset as f32 / sample_rate;
        re += sample * phase.cos();
        im -= sample * phase.sin();
    }
    re * re + im * im
}

#[test]
fn edits_buffer_and_evaluates_source() {
    let mut buffer = editor::EditorBuffer::empty(None);
    buffer.append("(bpm 110)");
    buffer.append("(start!)");
    buffer
        .insert(
            2,
            "(d :a :src :sine-synth :note (p [c3 g3]) :gate 1 :amp 0.2)",
        )
        .unwrap();
    buffer.replace(1, "(bpm 111)").unwrap();
    assert_eq!(buffer.lines.len(), 3);

    let runtime = Arc::new(Mutex::new(Runtime::new()));
    let snapshot = apply_runtime_source(&runtime, &buffer.source()).unwrap();
    assert_eq!(snapshot.bpm, 111.0);
    assert!(snapshot.running);
    assert!(snapshot.tracks.contains_key("a"));

    buffer.delete(2).unwrap();
    assert_eq!(buffer.lines.len(), 2);
    let block = buffer
        .range_source(editor::LineRange { start: 1, end: 2 })
        .unwrap();
    assert!(block.contains("(bpm 111)"));
}

#[test]
fn block_range_rejects_invalid_bounds() {
    let buffer = editor::EditorBuffer::empty(None);
    assert!(
        buffer
            .range_source(editor::LineRange { start: 1, end: 1 })
            .is_err()
    );
}

#[test]
fn gui_render_names_wav_after_selected_gl_file() {
    let output = gui_render::wav_output_path(
        &PathBuf::from("/tmp/session/live-acid.gl"),
        &PathBuf::from("/tmp/rendered"),
    )
    .unwrap();
    assert_eq!(output, PathBuf::from("/tmp/rendered/live-acid.wav"));
}
