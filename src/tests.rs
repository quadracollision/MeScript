use crate::audio::{
    AudioEngine, active_effect_specs, playback_step_for_runtime, pluck_delay_samples_for_test,
    render,
};
use crate::cli::{self, auto_render_seconds};
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
use crate::language::{
    compile_source_for_runtime, compile_source_for_runtime_with_base, eval_program, load_runtime,
    source_needs_compiler,
};
use crate::model::{NoteMode, Runtime, SceneState};
use crate::sequencer;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
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
fn workstation_about_text_shows_current_version_date() {
    let source = fs::read_to_string("src/main.clj").unwrap();
    assert!(
        source.contains("MeScript v0.33\\nJune 8, 2026"),
        "about text should show current version/date"
    );
}

#[test]
fn supports_reverse_aliases() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
             :src :sine-synth
             :note (reverse (p [c3 d3 e3]))
             :gate (reverse (p [1 0 0]))
             :amp (reverse (p [0.1 0.2 0.3])))
         (start!)",
    )
    .unwrap();

    let track = &runtime.tracks["lead"];
    assert!(runtime.running);
    assert!(track.notes[0] > track.notes[1]);
    assert!(track.notes[1] > track.notes[2]);
    assert_eq!(track.gates, vec![false, false, true]);
    assert_eq!(track.param_patterns.amp, Some(vec![0.3, 0.2, 0.1]));
}

#[test]
fn nested_note_pattern_steps_play_as_chords() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :chord
             :src :sine-synth
             :note (p [[c3 eb3 g3 bb3]])
             :gate (p [1])
             :dur 0.2
             :amp 0.15)
         (start!)",
    )
    .unwrap();

    let track = &runtime.tracks["chord"];
    assert_eq!(track.note_chords.len(), 1);
    assert_eq!(track.note_chords[0].len(), 4);

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    engine.next_sample();
    assert_eq!(engine.active_voice_count_for_test(), 4);
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

    assert_eq!(rx.try_recv().unwrap().step, 0);
}

#[test]
fn gui_live_step_event_coalescing_keeps_latest_cursor_position() {
    let (tx, rx) = mpsc::channel();
    tx.send(crate::audio::StepEvent {
        step: 1,
        scene: Some("intro".to_string()),
    })
    .unwrap();
    tx.send(crate::audio::StepEvent {
        step: 2,
        scene: Some("drop".to_string()),
    })
    .unwrap();

    let event = cli::coalesced_step_event(
        crate::audio::StepEvent {
            step: 0,
            scene: Some("intro".to_string()),
        },
        &rx,
    );

    assert_eq!(
        event,
        crate::audio::StepEvent {
            step: 2,
            scene: Some("drop".to_string())
        }
    );
    assert!(rx.try_recv().is_err());
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
    assert_eq!(rx.try_recv().unwrap().step, 0);
    for _ in 0..12_000 {
        engine.next_frame();
    }

    assert!(
        rx.try_iter()
            .any(|event| event.step == crate::audio::TRANSPORT_STOPPED_STEP)
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
    assert!(emitted.iter().any(|event| event.step == 0));
    assert!(emitted.iter().any(|event| event.step > 0));

    let mut next_runtime = Runtime::new();
    eval_program(
        &mut next_runtime,
        "(bpm 120) (d :a :src :sine-synth :note c3 :gate (p [1 0]) :amp 0.2) (start!)",
    )
    .unwrap();
    next_runtime.transport_revision = 1;
    *shared.lock().expect("runtime lock poisoned") = next_runtime;

    engine.next_frame();

    assert_eq!(rx.try_recv().unwrap().step, 0);
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

    let summary = crate::cli::gui_live_ok_summary(&runtime);
    assert!(
        summary.contains("OK bpm=")
            && summary.contains("running=true")
            && summary.contains("tracks=2")
            && summary.contains("scenes=1"),
        "gui-live OK summary should keep bridge counts: {}",
        summary
    );
    assert!(
        summary.contains("scene=:intro") && summary.contains("cycle=1/2"),
        "gui-live OK summary should expose active scene and cycle: {}",
        summary
    );

    runtime.running = false;
    runtime.scene_state = None;
    let stopped_summary = crate::cli::gui_live_ok_summary(&runtime);
    assert!(
        stopped_summary.contains("running=false")
            && stopped_summary.contains("tracks=2")
            && stopped_summary.contains("scenes=1"),
        "gui-live stopped summary should preserve bridge counts: {}",
        stopped_summary
    );
    assert!(
        stopped_summary.contains("scene=-") && stopped_summary.contains("cycle=-"),
        "gui-live stopped summary should hide stale scene state: {}",
        stopped_summary
    );
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
fn drum_sources_respond_to_note_pitch() {
    fn render_head(source: &str, note: &str) -> Vec<f32> {
        let mut runtime = Runtime::new();
        eval_program(
            &mut runtime,
            &format!(
                "(bpm 120)
                 (d :drum :src :{} :note {} :gate 1 :dur 0.08 :amp 1.0)
                 (start!)",
                source, note
            ),
        )
        .unwrap();
        let mut engine = AudioEngine::new(runtime, 48_000.0);
        (0..2_400).map(|_| engine.next_sample()).collect()
    }

    for source in [
        "hat",
        "hat-808",
        "hat-909",
        "hat-78",
        "snare-78",
        "snare-707",
        "cymbal-crash",
        "cymbal-ride",
        "rimshot",
        "shaker",
    ] {
        let low = render_head(source, "c3");
        let high = render_head(source, "c6");
        let diff: f32 = low
            .iter()
            .zip(high.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(diff > 5.0, "{} did not respond to note pitch", source);
    }
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
fn sample_form_defines_sample_track_from_wav_path() {
    let path = std::env::temp_dir().join("mescript-sample-form-test.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    {
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        writer.write_sample(16_384_i16).unwrap();
        writer.write_sample(-16_384_i16).unwrap();
        writer.finalize().unwrap();
    }

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        &format!(
            "(sample :hit {:?} :gate (p [1 0]) :dur 0.1 :amp 0.4)
             (start!)",
            path.to_string_lossy()
        ),
    )
    .unwrap();

    let track = &runtime.tracks["hit"];
    assert_eq!(track.sample_data.len(), 2);
    assert_eq!(track.gates, vec![true, false]);
    assert_eq!(track.dur_seconds, 0.1);
    assert_eq!(track.amp, 0.4);
    let _ = std::fs::remove_file(path);
}

#[test]
fn sample_form_rejects_empty_wav_files() {
    let path = std::env::temp_dir().join("mescript-empty-sample-form-test.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    {
        let writer = hound::WavWriter::create(&path, spec).unwrap();
        writer.finalize().unwrap();
    }

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        &format!(
            "(sample :hit {:?} :gate (p [1 0]) :dur 0.1 :amp 0.4)",
            path.to_string_lossy()
        ),
    )
    .unwrap_err();
    assert_eq!(err, "sample-data requires at least one value");
    let _ = std::fs::remove_file(path);
}

#[test]
fn sample_form_accepts_inline_sample_data() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(sample :hit :sample-data [1 0.5 -0.25 0] :gate (p [1 0]) :dur 0.1 :amp 0.4)
         (start!)",
    )
    .unwrap();

    let track = &runtime.tracks["hit"];
    assert_eq!(track.sample_data, vec![1.0, 0.5, -0.25, 0.0]);
    assert_eq!(track.gates, vec![true, false]);
    assert_eq!(track.dur_seconds, 0.1);
    assert_eq!(track.amp, 0.4);
}

#[test]
fn sample_data_rejects_invalid_cells() {
    let mut runtime = Runtime::new();
    let err = eval_program(&mut runtime, "(sample :hit :sample-data [1 bad])").unwrap_err();
    assert!(err.contains("unknown symbol 'bad'"), "{}", err);

    let mut runtime = Runtime::new();
    let err = eval_program(&mut runtime, "(sample :hit :sample-data [1 :bad])").unwrap_err();
    assert!(err.contains("expected number or note"), "{}", err);
}

#[test]
fn sample_form_reports_its_own_argument_errors() {
    let mut runtime = Runtime::new();
    let err = eval_program(&mut runtime, "(sample hit \"kick.wav\")").unwrap_err();
    assert_eq!(err, "sample track id must be a keyword");

    let mut runtime = Runtime::new();
    let err = eval_program(&mut runtime, "(sample :hit \"kick.wav\" 1)").unwrap_err();
    assert_eq!(err, "sample options must be keyword/value pairs");

    let mut runtime = Runtime::new();
    let err = eval_program(&mut runtime, "(sample :hit \"kick.wav\" :amp)").unwrap_err();
    assert_eq!(err, "sample :amp requires a value");

    let mut runtime = Runtime::new();
    let err = eval_program(&mut runtime, "(sample :hit :sample-data [])").unwrap_err();
    assert_eq!(err, "sample-data requires at least one value");

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(sample :hit :sample_data [1 0 -1] :gate 1)
         (start!)",
    )
    .unwrap();
    assert_eq!(runtime.tracks["hit"].sample_data, vec![1.0, 0.0, -1.0]);
}

#[test]
fn compiler_sample_form_accepts_inline_sample_data() {
    let compiled = compile_source_for_runtime(
        "(def hit
           (sample :hit :sample-data [1 0.5 -0.25 0] :gate (p [1 0])))
         (scene :intro :loop true hit)",
    )
    .unwrap();

    assert!(compiled.contains(":src :sample"));
    assert!(compiled.contains(":sample-data [1 0.5 -0.25 0]"));
    assert!(compiled.contains("(scene :intro"));
}

#[test]
fn compiler_sample_form_reports_its_own_argument_errors() {
    let err = compile_source_for_runtime("(def hit (sample hit \"kick.wav\"))").unwrap_err();
    assert_eq!(err, "sample track id must be a keyword");

    let err = compile_source_for_runtime("(def hit (sample :hit \"kick.wav\" 1))").unwrap_err();
    assert_eq!(err, "sample options must be keyword/value pairs");

    let err = compile_source_for_runtime("(def hit (sample :hit \"kick.wav\" :amp))").unwrap_err();
    assert_eq!(err, "sample :amp requires a value");

    let err = compile_source_for_runtime("(def hit (sample :hit 123))").unwrap_err();
    assert_eq!(err, "expected string");

    let err = compile_source_for_runtime("(def hit (sample :hit :sample-path 123))").unwrap_err();
    assert_eq!(err, "expected string");

    let err = compile_source_for_runtime("(def hit (sample :hit :sample 123))").unwrap_err();
    assert_eq!(err, "expected string");

    let err = compile_source_for_runtime("(def hit (sample :hit :sample-data []))").unwrap_err();
    assert_eq!(err, "sample-data requires at least one value");

    let err =
        compile_source_for_runtime("(def hit (sample :hit :sample-data [1 bad]))").unwrap_err();
    assert_eq!(err, "unknown symbol 'bad'");

    let err =
        compile_source_for_runtime("(def hit (sample :hit :sample-data [1 :bad]))").unwrap_err();
    assert_eq!(err, "expected number or note");

    let err = compile_source_for_runtime("(def hit (sample :hit :sample-data 1))").unwrap_err();
    assert_eq!(err, "sample-data must be a vector");

    let compiled = compile_source_for_runtime(
        "(def hit (sample :hit :sample-data (repeat 2 [1 0]) :gate 1))
             (scene :intro hit)",
    )
    .unwrap();
    assert!(compiled.contains(":sample-data [1 0 1 0]"), "{}", compiled);

    let compiled = compile_source_for_runtime(
        "(def hit (sample :hit :sample-path null :gate 1))
             (scene :intro hit)",
    )
    .unwrap();
    assert!(compiled.contains(":sample-path null"), "{}", compiled);

    let compiled = compile_source_for_runtime(
        "(def hit (sample :hit \"kick.wav\" :amp 0.5 :gate 1))
             (scene :intro hit)",
    )
    .unwrap();
    assert!(compiled.contains("(d :hit"), "{}", compiled);
    assert!(
        compiled.contains(":sample-path \"kick.wav\""),
        "{}",
        compiled
    );
    assert!(compiled.contains(":amp 0.5"), "{}", compiled);
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
fn harmonics_reject_out_of_range_values_instead_of_clamping() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :osc :src :additive :note c3 :gate 1 :harmonics [1 -0.1])",
    )
    .unwrap_err();
    assert!(
        err.contains("harmonics must be between 0 and 2, got -0.1"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :osc :src :additive :note c3 :gate 1 :harmonics [1 2.5])",
    )
    .unwrap_err();
    assert!(
        err.contains("harmonics must be between 0 and 2, got 2.5"),
        "{}",
        err
    );
}

#[test]
fn harmonics_reject_extra_values_instead_of_ignoring_them() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :osc :src :additive :note c3 :gate 1 :harmonics [1 1 1 1 1 1 1 1 1])",
    )
    .unwrap_err();
    assert!(
        err.contains("harmonics accepts at most 8 values, got 9"),
        "{}",
        err
    );
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
    assert_eq!(
        alias_patterns.dur_seconds.as_ref().unwrap(),
        &vec![0.5, 0.25]
    );
    assert_eq!(
        alias_patterns.amp.as_ref().unwrap(),
        &vec![0.4, 0.3, 0.2, 0.1]
    );
    assert_eq!(alias_patterns.gain.as_ref().unwrap(), &vec![1.0, 0.8]);
    assert_eq!(alias_patterns.unison.as_ref().unwrap(), &vec![2, 4]);
}

#[test]
fn integer_track_parameter_patterns_reject_fractional_values() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :osc
            :src :fm-op
            :note c3
            :gate 1
            :unison (p [1.5 3]))",
    )
    .unwrap_err();
    assert!(
        err.contains(":unison unison must be a non-negative integer"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :osc
            :src :fm-op
            :note c3
            :gate 1
            :unison (p [-1 3]))",
    )
    .unwrap_err();
    assert!(
        err.contains(":unison unison must be a non-negative integer"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :osc
            :src :fm-op
            :note c3
            :gate 1
            :unison (p [1 10]))",
    )
    .unwrap();
    assert_eq!(
        runtime.tracks["osc"]
            .param_patterns
            .unison
            .as_ref()
            .unwrap(),
        &vec![1, 10]
    );
}

#[test]
fn unison_rejects_out_of_range_values_instead_of_clamping() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :osc :src :fm-op :note c3 :gate 1 :unison 0)",
    )
    .unwrap_err();
    assert!(
        err.contains("unison must be between 1 and 10, got 0"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :osc :src :fm-op :note c3 :gate 1 :unison (p [1 11]))",
    )
    .unwrap_err();
    assert!(
        err.contains(":unison unison must be between 1 and 10, got 11"),
        "{}",
        err
    );
}

#[test]
fn every_rejects_zero_instead_of_coercing_to_one() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :slow :src :sine-synth :note c3 :gate 1 :every 0)",
    )
    .unwrap_err();
    assert!(err.contains("every must be greater than zero"), "{}", err);
}

#[test]
fn offset_rejects_negative_and_fractional_values() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :slow :src :sine-synth :note c3 :gate 1 :offset -1)",
    )
    .unwrap_err();
    assert!(
        err.contains("offset must be a non-negative integer"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :slow :src :sine-synth :note c3 :gate 1 :offset 1.5)",
    )
    .unwrap_err();
    assert!(
        err.contains("offset must be a non-negative integer"),
        "{}",
        err
    );
}

#[test]
fn detune_and_phase_reject_invalid_numeric_values() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate 1 :detune-cents :wide)",
    )
    .unwrap_err();
    assert!(
        err.contains(":detune-cents expected number or note"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate 1 :phase (p [0.1 :late]))",
    )
    .unwrap_err();
    assert!(
        err.contains(":phase expected numeric pattern value"),
        "{}",
        err
    );
}

#[test]
fn amp_rejects_out_of_range_values_instead_of_clamping() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate 1 :amp 3)",
    )
    .unwrap_err();
    assert!(
        err.contains(":amp amp must be between 0 and 1, got 3"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate 1 :amp (p [0.5 -0.1]))",
    )
    .unwrap_err();
    assert!(
        err.contains(":amp amp must be between 0 and 1, got -0.1"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate 1 :amp (p [0 1]))",
    )
    .unwrap();
    assert_eq!(
        runtime.tracks["lead"].param_patterns.amp.as_ref().unwrap(),
        &vec![0.0, 1.0]
    );
}

#[test]
fn dur_rejects_out_of_range_values_instead_of_clamping() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate 1 :dur 0)",
    )
    .unwrap_err();
    assert!(
        err.contains(":dur dur must be between 0.005 and 4, got 0"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate 1 :dur (p [0.1 5]))",
    )
    .unwrap_err();
    assert!(
        err.contains(":dur dur must be between 0.005 and 4, got 5"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate 1 :dur (p [0.005 4]))",
    )
    .unwrap();
    assert_eq!(
        runtime.tracks["lead"]
            .param_patterns
            .dur_seconds
            .as_ref()
            .unwrap(),
        &vec![0.005, 4.0]
    );
}

#[test]
fn oscillator_params_reject_out_of_range_values_instead_of_clamping() {
    for (param, value, message) in [
        (
            ":pulse-width",
            "1",
            ":pulse-width pulse-width must be between 0.01 and 0.99, got 1",
        ),
        (
            ":morph",
            "-0.1",
            ":morph morph must be between 0 and 1, got -0.1",
        ),
        (":gain", "3", ":gain gain must be between 0 and 2, got 3"),
        (
            ":unison-detune",
            "120",
            ":unison-detune unison-detune must be between 0 and 100, got 120",
        ),
        (
            ":unison-spread",
            "-0.1",
            ":unison-spread unison-spread must be between 0 and 1, got -0.1",
        ),
        (
            ":fm-ratio",
            "0",
            ":fm-ratio fm-ratio must be at least 0.01, got 0",
        ),
        (
            ":fm-depth",
            "33",
            ":fm-depth fm-depth must be between 0 and 32, got 33",
        ),
    ] {
        let mut runtime = Runtime::new();
        let err = eval_program(
            &mut runtime,
            &format!("(d :lead :src :fm-op :note c3 :gate 1 {} {})", param, value),
        )
        .unwrap_err();
        assert!(err.contains(message), "{}", err);
    }

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :fm-op :note c3 :gate 1 :gain (p [0.5 3]))",
    )
    .unwrap_err();
    assert!(
        err.contains(":gain gain must be between 0 and 2, got 3"),
        "{}",
        err
    );
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

fn render_mono_head(source: &str, samples: usize) -> Vec<f32> {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        &format!(
            "(d :osc
                :src :{}
                :note c3
                :gate 1
                :dur 0.2
                :amp 0.4
                :morph 0.5)
             (start!)",
            source
        ),
    )
    .unwrap_or_else(|err| panic!("source '{}' failed: {}", source, err));
    let mut engine = AudioEngine::new(runtime, 48_000.0);
    (0..samples).map(|_| engine.next_sample()).collect()
}

fn render_mono_head_with_detune(source: &str, cents: f32, samples: usize) -> Vec<f32> {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        &format!(
            "(d :osc
                :src :{}
                :note c3
                :gate 1
                :dur 0.2
                :amp 0.4
                :morph 0.5
                :detune-cents {})
             (start!)",
            source, cents
        ),
    )
    .unwrap_or_else(|err| panic!("source '{}' failed: {}", source, err));
    let mut engine = AudioEngine::new(runtime, 48_000.0);
    (0..samples).map(|_| engine.next_sample()).collect()
}

#[test]
fn detune_affects_raw_phase_synth_oscillators() {
    for source in [
        "square-synth",
        "pulse",
        "morph",
        "wavetable",
        "fm-op",
        "sync",
        "pwm-sweep",
    ] {
        let plain = render_mono_head(source, 4_000);
        let detuned = render_mono_head_with_detune(source, 1_200.0, 4_000);
        let diff = plain
            .iter()
            .zip(detuned.iter())
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>();
        assert!(diff > 1.0, "{} ignored detune, diff={}", source, diff);
    }
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
fn scene_loop_by_uses_named_track_cycle_times_count() {
    let mut runtime = Runtime::new();
    let source = compile_source_for_runtime(
        "(bpm 100)
         (def click
           (d :click
              :src :click
              :note (p [e4])
              :gate (p [1 0 0 0])
              :dur 0.05
              :amp 0.6))
         (scene :intro :loop-by :click 4
           click)
         (play-scene :intro)",
    )
    .unwrap();
    eval_program(&mut runtime, &source).unwrap();
    assert_eq!(runtime.scenes["intro"].steps, 16);
    assert_eq!(runtime.scenes["intro"].repeats, 0);
    assert_eq!(runtime.scene_state.as_ref().unwrap().current, "intro");
}

#[test]
fn scene_loop_by_with_next_advances_after_counted_track_cycles() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120)
         (scene :intro :loop-by :breath 4 :next :hams
           (d :breath :src :breath :note c3 :gate (p [1]) :dur 0.01 :amp 0.1))
         (scene :hams :loop-by :breath 4 :next :hams
           (d :breath :src :breath :note c3 :gate (p [1]) :dur 0.01 :amp 0.1))
         (play-scene :intro)",
    )
    .unwrap();

    assert_eq!(runtime.scenes["intro"].steps, 4);
    assert_eq!(runtime.scenes["intro"].repeats, 1);
    assert_eq!(
        runtime
            .scene_state
            .as_ref()
            .map(|state| state.current.as_str()),
        Some("intro")
    );

    let shared = Arc::new(Mutex::new(runtime));
    let mut engine = AudioEngine::new_shared(shared.clone(), 48_000.0);
    for _ in 0..24_010 {
        engine.next_sample();
    }

    let snapshot = shared.lock().unwrap().clone();
    assert_eq!(
        snapshot
            .scene_state
            .as_ref()
            .map(|state| state.current.as_str()),
        Some("hams")
    );
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
fn scene_bars_can_use_custom_bar_steps() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(scene :intro :bars 2 :bar-steps 8 :repeat 1
           (d :kick :src :kick-808 :note c1 :gate (p [1 0 0 0]) :amp 0.2))",
    )
    .unwrap();
    assert_eq!(runtime.scenes["intro"].steps, 16);
}

#[test]
fn scene_bars_can_use_track_cycle_as_bar_steps() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(scene :intro :bars 3 :bar-steps-of :kick :repeat 1
           (d :kick :src :kick-808 :note c1 :gate (p [1 0 0]) :amp 0.2))",
    )
    .unwrap();
    assert_eq!(runtime.scenes["intro"].steps, 9);
}

#[test]
fn scene_lengths_reject_zero_and_fractional_values() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(scene :intro :steps 0
           (d :kick :src :kick-808 :note c1 :gate (p [1 0 0 0]) :amp 0.2))",
    )
    .unwrap_err();
    assert!(err.contains("steps must be greater than zero"), "{}", err);

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(scene :intro :bars 0
           (d :kick :src :kick-808 :note c1 :gate (p [1 0 0 0]) :amp 0.2))",
    )
    .unwrap_err();
    assert!(err.contains("bars must be greater than zero"), "{}", err);

    let err = eval_program(
        &mut runtime,
        "(scene :intro :bar-steps 8
           (d :kick :src :kick-808 :note c1 :gate (p [1 0 0 0]) :amp 0.2))",
    )
    .unwrap_err();
    assert!(err.contains("scene :bar-steps requires :bars"), "{}", err);

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(scene :intro :steps 1.5
           (d :kick :src :kick-808 :note c1 :gate (p [1 0 0 0]) :amp 0.2))",
    )
    .unwrap_err();
    assert!(
        err.contains("steps must be a non-negative integer"),
        "{}",
        err
    );
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
fn scene_loop_by_rejects_unknown_track_and_bad_count() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(scene :intro :loop-by :missing 4
           (d :kick :src :kick-808 :note c1 :gate (p [1 0 0 0]) :amp 0.2))",
    )
    .unwrap_err();
    assert!(err.contains("scene :loop-by references unknown track ':missing'"));

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(scene :intro :loop-by :kick 0
           (d :kick :src :kick-808 :note c1 :gate (p [1 0 0 0]) :amp 0.2))",
    )
    .unwrap_err();
    assert!(err.contains("loop-by must be greater than zero"), "{}", err);
}

#[test]
fn play_scene_rejects_missing_reachable_next_scene() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(scene :intro :steps 4 :repeat 1 :next :drop
           (d :kick :src :kick-808 :note c1 :gate 1 :amp 0.2))
         (play-scene :intro)",
    )
    .unwrap_err();
    assert!(
        err.contains("scene ':intro' :next references unknown scene ':drop'"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(scene :intro :steps 4 :repeat 1 :next :drop
           (d :kick :src :kick-808 :note c1 :gate 1 :amp 0.2))
         (scene :drop :steps 4 :repeat 1
           (d :hat :src :hat-808 :note c6 :gate 1 :amp 0.2))
         (play-scene :intro)",
    )
    .unwrap();
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
fn scene_loop_true_is_clear_alias_for_infinite_repeat() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(scene :intro :loop true
           (d :kick
              :src :kick-808
              :note c1
              :gate (p [1 0 0 0])))
         (play-scene :intro)",
    )
    .unwrap();

    assert_eq!(runtime.scenes["intro"].repeats, 0);
    assert_eq!(auto_render_seconds(&runtime), None);
}

#[test]
fn scene_loop_false_errors_instead_of_doing_nothing() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(scene :intro :loop false
           (d :lead :src :sine-synth :note c3 :gate 1))",
    )
    .unwrap_err();
    assert!(
        err.contains("scene :loop only accepts true; use :repeat N for finite scenes"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(scene :intro :loop 0
           (d :lead :src :sine-synth :note c3 :gate 1))",
    )
    .unwrap_err();
    assert!(
        err.contains("scene :loop only accepts true; use :repeat N for finite scenes"),
        "{}",
        err
    );
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
    let mut comb = Comb::new(10.0, 0.7, 1.0, 100.0);
    assert!((comb.process(1.0) - 1.0).abs() < 1e-6);
    assert!((comb.process(0.0) - 0.7).abs() < 1e-6);
    assert!(comb.process(0.0).abs() < 1e-6);
}

#[test]
fn comb_mix_blends_dry_and_feed_forward_signal() {
    let mut dry = Comb::new(10.0, 0.7, 0.0, 100.0);
    assert!((dry.process(1.0) - 1.0).abs() < 1e-6);
    assert!(dry.process(0.0).abs() < 1e-6);

    let mut half = Comb::new(10.0, 0.7, 0.5, 100.0);
    assert!((half.process(1.0) - 1.0).abs() < 1e-6);
    assert!((half.process(0.0) - 0.35).abs() < 1e-6);
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
fn gate_then_times_plays_intro_then_loops_final_stage() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate (p (then
                      (times 2 [0 0 0 1 0 0])
                      [1 0 1 0]))
            :amp 0.2)",
    )
    .unwrap();

    let track = &runtime.tracks["lead"];
    assert_eq!(
        track.gates,
        vec![
            false, false, false, true, false, false, false, false, false, true, false, false, true,
            false, true, false,
        ]
    );
    assert_eq!(track.gate_loop_start, 12);

    let played: Vec<bool> = (0..24)
        .map(|step| sequencer::pattern_bool_with_loop(&track.gates, track.gate_loop_start, step))
        .collect();
    assert_eq!(
        played,
        vec![
            false, false, false, true, false, false, false, false, false, true, false, false, true,
            false, true, false, true, false, true, false, true, false, true, false,
        ]
    );
}

#[test]
fn gate_then_long_times_chain_preserves_stage_counts() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :kick
            :src :kick-synth
            :note c2
            :gate (p (then
                      (times 8 [1 0 0 0])
                      (times 4 [1 1 1 1])
                      (times 6 [1 0 0 0])
                      (times 1 [1 1 0 1])
                      (times 3 [1 1 1 1])
                      (times 6 [1 0 0 0])
                      (times 4 [1 1 1 1])
                      (times 6 [1 0 0 0])
                      (times 4 [1 1 1 1])
                      (times 6 [1 0 0 0])))
            :amp 0.1)",
    )
    .unwrap();

    let track = &runtime.tracks["kick"];
    assert_eq!(track.gates.len(), 192);
    assert_eq!(track.gate_loop_start, 0);

    assert_eq!(
        &track.gates[152..168],
        &[
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true,
        ]
    );
    assert_eq!(
        &track.gates[168..192],
        &[
            true, false, false, false, true, false, false, false, true, false, false, false, true,
            false, false, false, true, false, false, false, true, false, false, false,
        ]
    );
    let looped: Vec<bool> = (192..200)
        .map(|step| sequencer::pattern_bool_with_loop(&track.gates, track.gate_loop_start, step))
        .collect();
    assert_eq!(
        looped,
        vec![true, false, false, false, true, false, false, false]
    );
}

#[test]
fn gate_then_starts_from_prefix_when_track_enters_in_later_scene() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(scene :intro :steps 8 :repeat 1 :next :help
           (d :click :src :kick-synth :note c2 :gate 1 :amp 0.1))
         (scene :help :steps 4 :repeat 0
           (d :lead3
              :src :kick-synth
              :note c2
              :gate (p (then
                        (times 2 [0])
                        [1]))
              :amp 0.1))
         (play-scene :intro)",
    )
    .unwrap();

    let help = runtime.scenes["help"].clone();
    runtime.tracks = help.tracks;
    runtime.scene_state = Some(SceneState {
        current: "help".to_string(),
        cycle: 0,
        start_step: 8,
    });

    let track = &runtime.tracks["lead3"];
    assert_eq!(playback_step_for_runtime(&runtime, 8), 0);
    assert!(!sequencer::pattern_bool_with_loop(
        &track.gates,
        track.gate_loop_start,
        playback_step_for_runtime(&runtime, 8)
    ));
    assert!(!sequencer::pattern_bool_with_loop(
        &track.gates,
        track.gate_loop_start,
        playback_step_for_runtime(&runtime, 9)
    ));
    assert!(sequencer::pattern_bool_with_loop(
        &track.gates,
        track.gate_loop_start,
        playback_step_for_runtime(&runtime, 10)
    ));
}

#[test]
fn gate_then_starts_at_zero_when_next_scene_start_is_not_aligned_to_new_scene_length() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 100)
         (scene :intro :steps 8 :repeat 4 :next :help
           (d :click :src :click :note e4 :gate 1 :amp 0.1))
         (scene :help :repeat 0
           (d :kick
              :src :kick-synth
              :note c2
              :gate (p (then
                        (times 8 [1 0 0 0])
                        [1]))
              :amp 0.1))
         (play-scene :intro)",
    )
    .unwrap();

    let shared = Arc::new(Mutex::new(runtime));
    let mut engine = AudioEngine::new_shared(shared.clone(), 48_000.0);
    let step_samples = 7_200;
    for _ in 0..(32 * step_samples) {
        engine.next_sample();
    }

    let snapshot = shared.lock().unwrap().clone();
    assert_eq!(
        snapshot
            .scene_state
            .as_ref()
            .map(|state| (state.current.as_str(), state.start_step)),
        Some(("help", 32))
    );
    assert_eq!(playback_step_for_runtime(&snapshot, 32), 0);
}

#[test]
fn scene_advance_uses_scene_local_elapsed_steps() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 100)
         (scene :intro :steps 8 :repeat 4 :next :help
           (d :click :src :click :note e4 :gate 1 :amp 0.1))
         (scene :help :steps 192 :repeat 1 :next :done
           (d :kick :src :kick-synth :note c2 :gate 1 :amp 0.1))
         (scene :done :steps 4 :repeat 0
           (d :done :src :click :note e5 :gate 1 :amp 0.1))
         (play-scene :intro)",
    )
    .unwrap();

    let shared = Arc::new(Mutex::new(runtime));
    let mut engine = AudioEngine::new_shared(shared.clone(), 48_000.0);
    let step_samples = 7_200;
    for _ in 0..(192 * step_samples) {
        engine.next_sample();
    }

    let snapshot = shared.lock().unwrap().clone();
    assert_eq!(
        snapshot
            .scene_state
            .as_ref()
            .map(|state| (state.current.as_str(), state.start_step)),
        Some(("help", 32))
    );

    drop(snapshot);
    for _ in 0..(32 * step_samples) {
        engine.next_sample();
    }

    let snapshot = shared.lock().unwrap().clone();
    assert_eq!(
        snapshot
            .scene_state
            .as_ref()
            .map(|state| state.current.as_str()),
        Some("done")
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
fn gate_holds_can_overlap_later_hits() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate (p [1 (gate-hold 2) 1 0]) :amp 0.2)",
    )
    .unwrap();

    assert_eq!(
        runtime.tracks["lead"].gate_holds,
        vec![vec![0], vec![2], vec![0], vec![0]]
    );
}

#[test]
fn gate_hold_rejects_zero_instead_of_adding_a_noop_hit() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate (p [(gate-hold 0)]))",
    )
    .unwrap_err();
    assert!(
        err.contains("gate-hold must be greater than zero"),
        "{}",
        err
    );
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
fn bare_note_vectors_advance_on_hits_across_gate_loops() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(bpm 120)
         (d :lead
            :src :sine-synth
            :note [c3 d3 e3 f3 g3 a3 b3 c4]
            :gate (p [1 1 1 1])
            :dur 0.01
            :amp 0.1)
         (start!)",
    )
    .unwrap();

    let track = &runtime.tracks["lead"];
    assert_eq!(track.note_mode, NoteMode::Hit);

    let mut engine = AudioEngine::new(runtime, 48_000.0);
    for _ in 0..42_010 {
        engine.next_sample();
    }
    assert_eq!(engine.note_cursor_for_test("lead"), 8);
}

#[test]
fn explicit_p_note_patterns_remain_step_indexed() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note (p [c3 d3 e3 f3 g3 a3 b3 c4])
            :gate (p [1 1 1 1])
            :dur 0.01
            :amp 0.1)
         (start!)",
    )
    .unwrap();

    assert_eq!(runtime.tracks["lead"].note_mode, NoteMode::Step);
}

#[test]
fn note_patterns_support_then_times_chord_stages() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :pad
            :src :pad-wash
            :note (p (then
                      (times 3 [[c3 eb3 g3 bb3]])
                      (times 4 [[c3 e4]])))
            :gate (p [1 0 0 0 1 0 0 0])
            :dur 1.5
            :amp 0.16)
         (start!)",
    )
    .unwrap();

    let track = &runtime.tracks["pad"];
    assert_eq!(track.note_mode, NoteMode::Step);
    assert_eq!(track.note_chords.len(), 7);
    assert_eq!(track.note_chords[0].len(), 4);
    assert_eq!(track.note_chords[3].len(), 2);
}

#[test]
fn numeric_parameter_patterns_support_then_times_stages() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :dur (p (then
                     (times 2 [0.05 0.1])
                     [0.2]))
            :amp (g (then
                     (times 2 [0.2 0.4])
                     [0.6])))",
    )
    .unwrap();

    let patterns = &runtime.tracks["lead"].param_patterns;
    assert_eq!(
        patterns.dur_seconds.as_ref().unwrap(),
        &vec![0.05, 0.1, 0.05, 0.1, 0.2]
    );
    assert_eq!(
        patterns.amp.as_ref().unwrap(),
        &vec![0.2, 0.4, 0.2, 0.4, 0.6]
    );
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
        "(d :short-alias :src :sine-synth :note (g [c3 eb3]) :gate (p [1 [1 1]]) :amp 0.1)
         (d :dash-alias :src :sine-synth :note (gate-seq [c3 eb3]) :gate (p [1 [1 1]]) :amp 0.1)
         (d :underscore-alias :src :sine-synth :note (gate_seq [c3 eb3]) :gate (p [1 [1 1]]) :amp 0.1)",
    )
    .unwrap();

    assert_eq!(runtime.tracks["short-alias"].note_mode, NoteMode::Tick);
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
fn all_null_effect_forms_do_not_change_track_sound() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
             :src :additive
             :note c3
             :gate 1
             :fx [(filter :type null :cutoff null :res null)
                  (delay :time null :feedback null :mix null)])
         (post-fx [(reverse :mix null)])
         (start!)",
    )
    .unwrap();

    assert!(runtime.tracks["lead"].effects.is_empty());
    assert!(runtime.post_effects.is_empty());

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
             :src :additive
             :note c3
             :gate 1
             :fx [(filter :bad null)])
         (start!)",
    )
    .unwrap_err();
    assert!(err.contains("unknown filter parameter ':bad'"), "{}", err);
}

#[test]
fn gated_effect_gate_accepts_pattern_aliases() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :gated-g
             :src :saw-synth
             :note c3
             :gate 1
             :fx [(on :gate (g [1 0 1])
                      (filter :cutoff 900))])
         (d :gated-s
             :src :saw-synth
             :note c3
             :gate 1
             :fx [(on :gate (s [0 1])
                      (delay :mix 0.2))])
         (start!)",
    )
    .unwrap();

    assert_eq!(
        runtime.tracks["gated-g"].effects[0].gate_subdivisions,
        Some(vec![vec![true], vec![false], vec![true]])
    );
    assert_eq!(
        runtime.tracks["gated-s"].effects[0].gate_subdivisions,
        Some(vec![vec![false], vec![true]])
    );
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
fn formant_mix_blends_dry_and_vowel_filter_signal() {
    let mut dry = Formant::new(Vowel::A, 0.0, 48_000.0);
    assert!((dry.process(1.0) - 1.0).abs() < 1e-6);

    let mut wet = Formant::new(Vowel::A, 1.0, 48_000.0);
    let wet_sample = wet.process(1.0);
    assert!((wet_sample - 1.0).abs() > 1e-3);

    let mut half = Formant::new(Vowel::A, 0.5, 48_000.0);
    let half_sample = half.process(1.0);
    assert!((half_sample - ((1.0 + wet_sample) * 0.5)).abs() < 1e-6);
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
                     (h3000 :detune-cents 10 :mix 0.22)
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
fn distortion_drive_rejects_out_of_range_values_instead_of_clamping() {
    for (source, message) in [
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(distort :drive -0.1)])
             (start!)",
            "distort :drive drive must be between 0 and 10, got -0.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(distortion :drive 10.2)])
             (start!)",
            "distortion :drive drive must be between 0 and 10, got 10.2",
        ),
    ] {
        let mut runtime = Runtime::new();
        let err = eval_program(&mut runtime, source).unwrap_err();
        assert!(err.contains(message), "{}", err);
    }

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(distort :drive 0)
                 (distortion :drive 10)
                 (distort :drive null)])
         (start!)",
    )
    .unwrap();
}

#[test]
fn analog_normalized_parameters_reject_out_of_range_values_instead_of_clamping() {
    for (source, message) in [
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(tube :gain 1.1)])
             (start!)",
            "tube :gain gain must be between 0 and 1, got 1.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(tube-saturation :asymmetry -0.1)])
             (start!)",
            "tube-saturation :asymmetry asymmetry must be between 0 and 1, got -0.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(tape :input-level 1.2)])
             (start!)",
            "tape :input-level input-level must be between 0 and 1, got 1.2",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(tape :flutter -0.1)])
             (start!)",
            "tape :flutter flutter must be between 0 and 1, got -0.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(studer-tape :bias 1.1)])
             (start!)",
            "studer-tape :bias bias must be between 0 and 1, got 1.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(studer-tape :speed 3)])
             (start!)",
            "studer-tape :speed speed must be between 0 and 2, got 3",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(exciter :amount -0.1)])
             (start!)",
            "exciter :amount amount must be between 0 and 1, got -0.1",
        ),
    ] {
        let mut runtime = Runtime::new();
        let err = eval_program(&mut runtime, source).unwrap_err();
        assert!(err.contains(message), "{}", err);
    }

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(tube :drive 0 :asymmetry 1)
                 (tube-saturation :gain null :asymmetry null)
                 (tape :saturation 0 :wow 1 :flutter null)
                 (studer-tape :input-level 0 :speed 2 :bias 1)
                 (exciter :amount 1)])
         (start!)",
    )
    .unwrap();
}

#[test]
fn dynamics_bounded_parameters_reject_out_of_range_values_instead_of_clamping() {
    for (source, message) in [
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(fairchild :input-gain 1.1)])
             (start!)",
            "fairchild :input-gain input-gain must be between 0 and 1, got 1.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(fairchild :time-constant 7)])
             (start!)",
            "fairchild :time-constant time-constant must be between 1 and 6, got 7",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(la2a :peak-reduction -0.1)])
             (start!)",
            "la2a :peak-reduction peak-reduction must be between 0 and 1, got -0.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(1176 :attack 1.1)])
             (start!)",
            "1176 :attack attack must be between 0 and 1, got 1.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(urei-1176 :release -0.1)])
             (start!)",
            "urei-1176 :release release must be between 0 and 1, got -0.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(transient :attack-gain 9)])
             (start!)",
            "transient :attack-gain attack-gain must be between 0 and 8, got 9",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(transient-shaper :sustain-gain 5)])
             (start!)",
            "transient-shaper :sustain-gain sustain-gain must be between 0 and 4, got 5",
        ),
    ] {
        let mut runtime = Runtime::new();
        let err = eval_program(&mut runtime, source).unwrap_err();
        assert!(err.contains(message), "{}", err);
    }

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(fairchild :input-gain 0 :time-constant 1 :mix null)
                 (fairchild :input-gain 1 :time-constant 6)
                 (la2a :peak-reduction 0)
                 (la2a :peak-reduction 1)
                 (1176 :input-gain 0 :attack 1 :release null)
                 (urei-1176 :input-gain 1 :attack 0 :release 1)
                 (transient :attack-gain 0 :sustain-gain 4)
                 (transient-shaper :attack-gain 8 :sustain-gain 0)])
         (start!)",
    )
    .unwrap();
}

#[test]
fn hardware_filter_normalized_parameters_reject_out_of_range_values_instead_of_clamping() {
    for (source, message) in [
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(moog :drive 1.1)])
             (start!)",
            "moog :drive drive must be between 0 and 1, got 1.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(moog-ladder :drive -0.1)])
             (start!)",
            "moog-ladder :drive drive must be between 0 and 1, got -0.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(tb-303 :env-mod 1.1)])
             (start!)",
            "tb-303 :env-mod env-mod must be between 0 and 1, got 1.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(303-filter :accent -0.1)])
             (start!)",
            "303-filter :accent accent must be between 0 and 1, got -0.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(buchla-lpg :strike 1.2)])
             (start!)",
            "buchla-lpg :strike strike must be between 0 and 1, got 1.2",
        ),
    ] {
        let mut runtime = Runtime::new();
        let err = eval_program(&mut runtime, source).unwrap_err();
        assert!(err.contains(message), "{}", err);
    }

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(moog :drive 0)
                 (moog-ladder :drive 1)
                 (tb-303 :env-mod 0 :accent 1)
                 (tb303 :env-mod null :accent null)
                 (buchla-lpg :strike 0)
                 (lpg :strike 1)])
         (start!)",
    )
    .unwrap();
}

#[test]
fn amp_and_eq_normalized_parameters_reject_out_of_range_values_instead_of_clamping() {
    for (source, message) in [
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(neve-preamp :gain 1.1)])
             (start!)",
            "neve-preamp :gain gain must be between 0 and 1, got 1.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(neve-preamp :warmth -0.1)])
             (start!)",
            "neve-preamp :warmth warmth must be between 0 and 1, got -0.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(marshall-amp :presence 1.1)])
             (start!)",
            "marshall-amp :presence presence must be between 0 and 1, got 1.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(vox-ac30 :cut -0.1)])
             (start!)",
            "vox-ac30 :cut cut must be between 0 and 1, got -0.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(fender-twin :reverb-mix 1.1)])
             (start!)",
            "fender-twin :reverb-mix reverb-mix must be between 0 and 1, got 1.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(pultec-eq :low-boost -0.1)])
             (start!)",
            "pultec-eq :low-boost low-boost must be between 0 and 1, got -0.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(pultec :high-atten 1.1)])
             (start!)",
            "pultec :high-atten high-atten must be between 0 and 1, got 1.1",
        ),
    ] {
        let mut runtime = Runtime::new();
        let err = eval_program(&mut runtime, source).unwrap_err();
        assert!(err.contains(message), "{}", err);
    }

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(neve-preamp :gain 0 :warmth 1)
                 (marshall-amp :gain 1 :tone 0 :presence null)
                 (vox-ac30 :gain 0 :treble 1 :cut null)
                 (fender-twin :volume 0 :treble 1 :bass 0 :reverb-mix 1)
                 (pultec-eq :low-boost 0 :low-atten 1 :high-boost 0 :high-atten 1)
                 (pultec :low-boost null :low-atten null :high-boost null :high-atten null)])
         (start!)",
    )
    .unwrap();
}

#[test]
fn reverb_and_delay_bounded_parameters_reject_out_of_range_values_instead_of_clamping() {
    for (source, message) in [
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(reverb :decay 1.1)])
             (start!)",
            "reverb :decay decay must be between 0 and 1, got 1.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(spring-reverb :drip -0.1)])
             (start!)",
            "spring-reverb :drip drip must be between 0 and 1, got -0.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(emt-plate :decay 0.05)])
             (start!)",
            "emt-plate :decay decay must be between 0.1 and 5, got 0.05",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(lexicon-224 :size 0.1)])
             (start!)",
            "lexicon-224 :size size must be between 0.2 and 2, got 0.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(ams-reverb :damping 1.1)])
             (start!)",
            "ams-reverb :damping damping must be between 0 and 1, got 1.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(space-echo :time 0.01)])
             (start!)",
            "space-echo :time time must be between 0.02 and 2, got 0.01",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(re201 :spring-mix 1.1)])
             (start!)",
            "re201 :spring-mix spring-mix must be between 0 and 1, got 1.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(tc2290 :mod-depth 0.06)])
             (start!)",
            "tc2290 :mod-depth mod-depth must be between 0 and 0.05, got 0.06",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(tc-2290 :time-ms 0.5)])
             (start!)",
            "tc-2290 :time-ms time-ms must be between 1 and 2000, got 0.5",
        ),
    ] {
        let mut runtime = Runtime::new();
        let err = eval_program(&mut runtime, source).unwrap_err();
        assert!(err.contains(message), "{}", err);
    }

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(reverb :decay 0)
                 (reverb :decay 1)
                 (spring-reverb :decay 0 :tone 1 :drip null)
                 (spring-reverb :decay 4 :tone 0 :drip 1)
                 (emt-plate :decay 0.1 :damping 0)
                 (emt-plate :decay 5 :damping 1)
                 (lexicon-224 :size 0.2 :decay 0.1 :damping 0)
                 (lexicon-224 :size 2 :decay 8 :damping 1)
                 (ams-reverb :decay 0.1 :damping 0)
                 (ams-reverb :decay 5 :damping 1)
                 (space-echo :time 0.02 :wow 0 :flutter 1 :tone null :spring-mix 0)
                 (re-201 :time 2 :wow 1 :flutter 0 :tone 1 :spring-mix 1)
                 (tc2290 :time-ms 1 :mod-rate 0 :mod-depth 0)
                 (tc-2290 :time-ms 2000 :mod-rate 20 :mod-depth 0.05)])
         (start!)",
    )
    .unwrap();
}

#[test]
fn time_pitch_glitch_bounded_parameters_reject_out_of_range_values_instead_of_clamping() {
    for (source, message) in [
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(octaver :octave-up 1.1)])
             (start!)",
            "octaver :octave-up octave-up must be between 0 and 1, got 1.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(stutter :grain-ms 0.5)])
             (start!)",
            "stutter :grain-ms grain-size-ms must be between 1 and 500, got 0.5",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(granular-stutter :repeats 17)])
             (start!)",
            "granular-stutter :repeats repeats must be between 1 and 16, got 17",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(glitch :density -0.1)])
             (start!)",
            "glitch :density density must be between 0 and 1, got -0.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(glitch :slice-ms 501)])
             (start!)",
            "glitch :slice-ms slice-ms must be between 1 and 500, got 501",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(fade :fade-in-ms -1)])
             (start!)",
            "fade :fade-in-ms fade-in-ms must be at least 0, got -1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(fade :duration 0)])
             (start!)",
            "fade :duration duration must be at least 0.001, got 0",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(adsr :sustain 1.1)])
             (start!)",
            "adsr :sustain sustain must be between 0 and 1, got 1.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(asdr :a -0.1)])
             (start!)",
            "asdr :a attack must be at least 0, got -0.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(doppler :speed 0)])
             (start!)",
            "doppler :speed speed must be between 0.01 and 8, got 0",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(doppler :depth 1.1)])
             (start!)",
            "doppler :depth depth must be between 0 and 1, got 1.1",
        ),
    ] {
        let mut runtime = Runtime::new();
        let err = eval_program(&mut runtime, source).unwrap_err();
        assert!(err.contains(message), "{}", err);
    }

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(octaver :octave-up 0 :octave-down 1)
                 (octaver :octave-up null :octave-down null)
                 (stutter :grain-size-ms 1 :repeats 1)
                 (granular-stutter :grain-ms 500 :repeats 16)
                 (glitch :density 0 :slice-ms 1)
                 (glitch :density 1 :slice-ms 500)
                 (fade :fade-in-ms 0 :fade-out-ms 0 :duration 0.001)
                 (fade :fade-in-ms null :fade-out-ms null :duration null)
                 (adsr :attack 0 :decay 0 :sustain 0 :release 0 :duration 0.001)
                 (asdr :a null :d null :s null :r null :duration null)
                 (doppler :speed 0.01 :depth 0)
                 (doppler :speed 8 :depth 1)])
         (start!)",
    )
    .unwrap();
}

#[test]
fn creative_body_bounded_parameters_reject_out_of_range_values_instead_of_clamping() {
    for (source, message) in [
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(crystal :decay 1)])
             (start!)",
            "crystal :decay decay must be between 0 and 0.95, got 1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(wavefolder :folds 0)])
             (start!)",
            "wavefolder :folds folds must be between 1 and 8, got 0",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(fold :gain 0)])
             (start!)",
            "fold :gain gain must be between 0.1 and 12, got 0",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(wavefolder :symmetry 2.1)])
             (start!)",
            "wavefolder :symmetry symmetry must be between 0.1 and 2, got 2.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(resonator :freq 19)])
             (start!)",
            "resonator :freq freq must be at least 20, got 19",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(resonator :decay 1.1)])
             (start!)",
            "resonator :decay decay must be between 0 and 1, got 1.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(resonator :harmonics 17)])
             (start!)",
            "resonator :harmonics harmonics must be between 1 and 16, got 17",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(maximizer :warmth -0.1)])
             (start!)",
            "maximizer :warmth warmth must be between 0 and 1, got -0.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(maximizer :release-ms 0)])
             (start!)",
            "maximizer :release-ms release-ms must be at least 1, got 0",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(harmonic-enhance :air 1.1)])
             (start!)",
            "harmonic-enhance :air air must be between 0 and 1, got 1.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(body :tone -0.1)])
             (start!)",
            "body :tone tone must be between 0 and 1, got -0.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(warmth :amount 1.1)])
             (start!)",
            "warmth :amount amount must be between 0 and 1, got 1.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(spatial :height -0.1)])
             (start!)",
            "spatial :height height must be between 0 and 1, got -0.1",
        ),
    ] {
        let mut runtime = Runtime::new();
        let err = eval_program(&mut runtime, source).unwrap_err();
        assert!(err.contains(message), "{}", err);
    }

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(crystal :brightness 0 :decay 0)
                 (crystal :brightness 1 :decay 0.95)
                 (crystal :brightness null :decay null)
                 (wavefolder :folds 1 :gain 0.1 :symmetry 0.1)
                 (fold :folds 8 :gain 12 :symmetry 2)
                 (wavefolder :folds null :gain null :symmetry null)
                 (resonator :freq 20 :decay 0 :harmonics 1)
                 (resonator :freq null :decay 1 :harmonics 16)
                 (maximizer :warmth 0 :release-ms 1)
                 (maximizer :warmth 1 :release-ms null)
                 (harmonic-enhance :low-harmonics 0 :high-harmonics 1 :air null)
                 (body :size 0 :tone 1 :mix null)
                 (warmth :amount 0)
                 (warmth :amount 1)
                 (spatial :room-size 0 :position 1 :height null)
                 (spatial :room-size 1 :position 0 :height 1)])
         (start!)",
    )
    .unwrap();
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
    let drive_ten = render_abs_sum(10.0);

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
fn native_editor_preview_infers_obvious_playback() {
    let track = editor::editor_preview_source("(d :a :src :click :gate 1)");
    assert!(track.contains("(start!)"));

    let scene =
        editor::editor_preview_source("(scene :intro :loop true\n  (d :a :src :click :gate 1))");
    assert!(scene.contains("(play-scene :intro)"));

    let explicit = "(d :a :src :click :gate 1)\n(start!)";
    assert_eq!(editor::editor_preview_source(explicit), explicit);

    let def_only = "(def click\n  (d :click :src :click :gate 1))";
    assert_eq!(editor::editor_preview_source(def_only), def_only);

    let commented_playback = "(d :a :src :click :gate 1)\n; (start!)";
    let commented_preview = editor::editor_preview_source(commented_playback);
    assert!(
        commented_preview != commented_playback && commented_preview.ends_with("\n\n(start!)"),
        "commented playback command should not suppress preview start"
    );

    let string_playback = "(d :a :src :click :gate 1)\n\"(play-scene :intro)\"";
    let string_preview = editor::editor_preview_source(string_playback);
    assert!(
        string_preview != string_playback && string_preview.ends_with("\n\n(start!)"),
        "string playback command should not suppress preview start"
    );

    let vector_nested = "[(d :nested :src :click :gate 1)]";
    assert_eq!(editor::editor_preview_source(vector_nested), vector_nested);

    let map_nested = "{:track (d :nested :src :click :gate 1)}";
    assert_eq!(editor::editor_preview_source(map_nested), map_nested);

    let runtime = Arc::new(Mutex::new(Runtime::new()));
    let snapshot = apply_runtime_source(&runtime, &scene).unwrap();
    assert!(snapshot.running);
    assert_eq!(
        snapshot
            .scene_state
            .as_ref()
            .map(|state| state.current.as_str()),
        Some("intro")
    );
}

#[test]
fn native_editor_scene_commands_infer_only_unambiguous_scene() {
    let single_scene = vec![
        "(bpm 100)".to_string(),
        "(def click (d :click :src :click :gate 1))".to_string(),
        "(scene :intro :loop true".to_string(),
        "  click)".to_string(),
        "(play-scene :intro)".to_string(),
    ];
    assert_eq!(
        editor::scene_name_for_cursor(&single_scene, 0).as_deref(),
        Some("intro")
    );

    let multiple_scenes = vec![
        "(scene :intro :repeat 1 :next :drop".to_string(),
        "  click)".to_string(),
        "(scene :drop :loop true".to_string(),
        "  kick)".to_string(),
        "(play-scene :intro)".to_string(),
    ];
    assert_eq!(
        editor::scene_name_for_cursor(&multiple_scenes, 3).as_deref(),
        Some("drop")
    );
    assert_eq!(
        editor::scene_name_for_cursor(&multiple_scenes, 4).as_deref(),
        Some("intro")
    );
    assert_eq!(editor::scene_name_for_cursor(&multiple_scenes, 10), None);

    let playback_line = vec!["(play-scene :intro)".to_string()];
    assert_eq!(
        editor::scene_name_for_cursor(&playback_line, 0).as_deref(),
        Some("intro")
    );

    let nested_scene_data = vec![
        "(def data".to_string(),
        "  (scene :fake :loop true".to_string(),
        "    click))".to_string(),
        "(scene :real :loop true".to_string(),
        "  click)".to_string(),
    ];
    assert_eq!(
        editor::scene_name_for_cursor(&nested_scene_data, 1).as_deref(),
        Some("real")
    );

    let deep_scene_body = vec![
        "(scene :intro :loop true".to_string(),
        "  (d :lead".to_string(),
        "     :src :click".to_string(),
        "     :gate 1))".to_string(),
        "(scene :drop :loop true".to_string(),
        "  kick)".to_string(),
    ];
    assert_eq!(
        editor::scene_name_for_cursor(&deep_scene_body, 3).as_deref(),
        Some("intro")
    );
}

#[test]
fn native_file_loading_accepts_compiler_forms() {
    let path = PathBuf::from(format!(
        "/tmp/glitchlisp-native-compiler-forms-{}.gl",
        std::process::id()
    ));
    std::fs::write(
        &path,
        "(bpm 100)
         (def click
           (d :click :src :click :gate (p [1 0 0 0]) :dur 0.02 :amp 0.2))
         (scene :intro :loop true
           click)
         (play-scene :intro)",
    )
    .unwrap();

    let runtime = load_runtime(path.to_str().unwrap()).unwrap();
    assert!(runtime.running);
    assert_eq!(
        runtime
            .scene_state
            .as_ref()
            .map(|state| state.current.as_str()),
        Some("intro")
    );
    assert!(runtime.tracks.contains_key("click"));

    let _ = std::fs::remove_file(path);
}

#[test]
fn native_file_loading_expands_relative_includes() {
    let dir = PathBuf::from(format!("/tmp/glitchlisp-include-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("parts")).unwrap();
    let instruments = dir.join("parts/instruments.gl");
    let song = dir.join("song.gl");
    std::fs::write(
        &instruments,
        "(def click
           (d :click :src :click :gate (p [1 0]) :dur 0.02 :amp 0.2))",
    )
    .unwrap();
    std::fs::write(
        &song,
        "(include \"parts/instruments.gl\")
         (scene :intro :loop true
           click)
         (play-scene :intro)",
    )
    .unwrap();

    let runtime = load_runtime(song.to_str().unwrap()).unwrap();
    assert!(runtime.running);
    assert!(runtime.tracks.contains_key("click"));
    assert_eq!(
        runtime
            .scene_state
            .as_ref()
            .map(|state| state.current.as_str()),
        Some("intro")
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn include_cycles_report_clear_error() {
    let dir = PathBuf::from(format!(
        "/tmp/glitchlisp-include-cycle-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.gl");
    let b = dir.join("b.gl");
    std::fs::write(&a, "(include \"b.gl\")").unwrap();
    std::fs::write(&b, "(include \"a.gl\")").unwrap();

    let source = std::fs::read_to_string(&a).unwrap();
    let err = compile_source_for_runtime_with_base(&source, Some(&a)).unwrap_err();
    assert!(err.contains("include cycle detected"), "{}", err);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn native_interactive_eval_accepts_compiler_forms() {
    let mut runtime = Runtime::new();
    cli::eval_interactive_source(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note (p (scale c3 :minor 3))
            :gate (p :repeat 2 [1 0])
            :dur 0.1
            :amp 0.2)
         (start!)",
    )
    .unwrap();

    assert!(runtime.running);
    let track = &runtime.tracks["lead"];
    assert_eq!(track.notes.len(), 3);
    assert_eq!(track.gates, vec![true, false, true, false]);
}

#[test]
fn compiler_bridge_skips_plain_runtime_source() {
    let source = "(d :click :src :click :gate (p [1 0]) :dur 0.02)\n(start!)";
    assert!(!source_needs_compiler(source));
    assert_eq!(compile_source_for_runtime(source).unwrap(), source);

    let compiler_source = "(def click (d :click :src :click :gate (p [1 0])))
                           (scene :intro :loop true click)";
    assert!(source_needs_compiler(compiler_source));
    assert!(
        compile_source_for_runtime(compiler_source)
            .unwrap()
            .contains("(d :click")
    );
}

#[test]
fn compiler_bridge_ignores_helper_names_in_comments_and_strings() {
    let source = "(d :click :src :click :gate 1)\n; def scale chord\n(play-note \"def\")";
    assert!(!source_needs_compiler(source));
}

#[test]
fn compiler_generator_options_reject_typos_and_missing_values() {
    let err = compile_source_for_runtime("(p (choose :cout 2 [1 0]))").unwrap_err();
    assert_eq!(err, "unknown choose option :cout");

    let err = compile_source_for_runtime("(p (choose :count [1 0]))").unwrap_err();
    assert!(err.contains("choose :count requires a value"), "{}", err);

    let err = compile_source_for_runtime("(p (choose :count 2 :count 4 [1 0]))").unwrap_err();
    assert_eq!(err, "duplicate choose option :count");

    let err = compile_source_for_runtime("(p (rand-range :maxx 1))").unwrap_err();
    assert!(err.contains("unknown rand-range option :maxx"), "{}", err);

    let err = compile_source_for_runtime("(p (rand-range :seed 1 :seed 2))").unwrap_err();
    assert_eq!(err, "duplicate rand-range option :seed");

    let err = compile_source_for_runtime("(p (rand-range :min 2 :max 1))").unwrap_err();
    assert_eq!(err, "rand-range :max must be greater than or equal to :min");

    let compiled = compile_source_for_runtime("(p (rand-range :count 2 :min 1 :max 1))").unwrap();
    assert_eq!(compiled, "(p [1.0 1.0])");

    let compiled = compile_source_for_runtime("(p (choose :count 2 :seed 1 [1 0]))").unwrap();
    assert!(compiled.contains("(p [1 1])"), "{}", compiled);
}

#[test]
fn compiler_repeat_helpers_are_detected_and_validate_arity() {
    assert!(source_needs_compiler("(p :repeat 2 [1 0])"));
    assert!(!source_needs_compiler("(scene :intro :repeat 2)"));

    let compiled = compile_source_for_runtime("(p :repeat 2 [1 0])").unwrap();
    assert_eq!(compiled, "(p [1 0 1 0])");

    let compiled = compile_source_for_runtime("(p (repeat 2 [1 0]))").unwrap();
    assert_eq!(compiled, "(p [1 0 1 0])");

    let err = compile_source_for_runtime("(p (repeat 2))").unwrap_err();
    assert_eq!(err, "repeat expects count and one value");

    let err = compile_source_for_runtime("(p :repeat 2 [1] [0])").unwrap_err();
    assert_eq!(err, "p :repeat expects count and one vector pattern");

    let err = compile_source_for_runtime("(p :repeat -1 [1 0])").unwrap_err();
    assert_eq!(err, "p :repeat count must be non-negative");
}

#[test]
fn compiler_p_wrapper_rejects_malformed_arity_like_runtime() {
    assert!(!source_needs_compiler("(p [1 0])"));
    assert!(source_needs_compiler("(p)"));
    assert!(source_needs_compiler("(p [1 0] [0 1])"));
    assert!(source_needs_compiler("(p [1 0] then [0 1])"));

    let err = compile_source_for_runtime("(p)").unwrap_err();
    assert_eq!(err, "p requires a pattern");

    let err = compile_source_for_runtime("(p [1 0] [0 1])").unwrap_err();
    assert_eq!(err, "p expects one pattern");

    let err = compile_source_for_runtime("(p [1 0] then [0 1])").unwrap_err();
    assert_eq!(
        err,
        "p wraps exactly one pattern; use (p (then A B)) instead of (p A then B)"
    );
}

#[test]
fn compiler_times_and_then_reject_malformed_arity_like_runtime() {
    assert!(!source_needs_compiler("(times 2 [1 0])"));
    assert!(!source_needs_compiler("(then [1 0] [0 1])"));
    assert!(source_needs_compiler("(times)"));
    assert!(source_needs_compiler("(times 2)"));
    assert!(source_needs_compiler("(times 2 [1 0] [0 1])"));
    assert!(source_needs_compiler("(times 0 [1 0])"));
    assert!(source_needs_compiler("(then [1 0])"));

    let err = compile_source_for_runtime("(times)").unwrap_err();
    assert_eq!(err, "times requires a count");

    let err = compile_source_for_runtime("(times 2)").unwrap_err();
    assert_eq!(err, "times requires a pattern");

    let err = compile_source_for_runtime("(times 2 [1 0] [0 1])").unwrap_err();
    assert_eq!(err, "times expects count and one pattern");

    let err = compile_source_for_runtime("(times 0 [1 0])").unwrap_err();
    assert_eq!(err, "times must be greater than zero");

    let err = compile_source_for_runtime("(then [1 0])").unwrap_err();
    assert_eq!(err, "then expects at least two patterns");
}

#[test]
fn compiler_every_n_rejects_missing_hit_and_zero_count() {
    let err = compile_source_for_runtime("(p (every-n 4))").unwrap_err();
    assert_eq!(err, "every-n expects n, hit value, and optional rest value");

    let err = compile_source_for_runtime("(p (every-n 0 1))").unwrap_err();
    assert_eq!(err, "every-n count must be greater than zero");

    let compiled = compile_source_for_runtime("(p (every-n 4 1 0))").unwrap();
    assert_eq!(compiled, "(p [1 0 0 0])");
}

#[test]
fn compiler_count_arguments_reject_fractional_values() {
    let err = compile_source_for_runtime("(p (repeat 2.5 [1 0]))").unwrap_err();
    assert_eq!(err, "repeat count must be a whole number");

    let err = compile_source_for_runtime("(p (take 2.5 [1 0 1]))").unwrap_err();
    assert_eq!(err, "take count must be a whole number");

    let err = compile_source_for_runtime("(p (every-n 2.5 1 0))").unwrap_err();
    assert_eq!(err, "every-n count must be a whole number");

    let err = compile_source_for_runtime("(p (choose :count 2.5 [1 0]))").unwrap_err();
    assert_eq!(err, "choose count must be a whole number");

    let err = compile_source_for_runtime("(p :repeat 2.5 [1 0])").unwrap_err();
    assert_eq!(err, "p :repeat count must be a whole number");
}

#[test]
fn compiler_integer_arguments_reject_fractional_values() {
    let err = compile_source_for_runtime("(p (rotate 1.5 [1 0 0]))").unwrap_err();
    assert_eq!(err, "rotate amount must be a whole number");

    let err = compile_source_for_runtime("(p (transpose c3 1.5))").unwrap_err();
    assert_eq!(err, "transpose semitones must be a whole number");

    let err = compile_source_for_runtime("(p (range c3 c4 1.5))").unwrap_err();
    assert_eq!(err, "range note step must be a whole number");

    let compiled = compile_source_for_runtime("(p (rotate 1 [1 0 0]))").unwrap();
    assert_eq!(compiled, "(p [0 0 1])");
}

#[test]
fn compiler_seed_options_reject_fractional_values() {
    let err = compile_source_for_runtime("(p (choose :seed 1.5 :count 2 [1 0]))").unwrap_err();
    assert_eq!(err, "choose seed must be a whole number");

    let err = compile_source_for_runtime("(p (rand-range :seed 1.5 :count 2))").unwrap_err();
    assert_eq!(err, "rand-range seed must be a whole number");

    let compiled = compile_source_for_runtime("(p (choose :seed 1 :count 2 [1 0]))").unwrap();
    assert!(compiled.contains("(p [1 1])"), "{}", compiled);
}

#[test]
fn compiler_generators_report_missing_arguments_by_form() {
    let err = compile_source_for_runtime("(p (scale c3 :major))").unwrap_err();
    assert_eq!(err, "scale expects root, scale name, and count");

    let err = compile_source_for_runtime("(p (shape [c3 e3 g3]))").unwrap_err();
    assert_eq!(
        err,
        "shape expects a vector and a vector of 1-based positions"
    );

    let err = compile_source_for_runtime("(p (transpose c3))").unwrap_err();
    assert_eq!(err, "transpose expects value and semitones");

    let err = compile_source_for_runtime("(p (take 2))").unwrap_err();
    assert_eq!(err, "take expects count and one vector");

    let err = compile_source_for_runtime("(p (rotate 1))").unwrap_err();
    assert_eq!(err, "rotate expects amount and one vector");

    let err = compile_source_for_runtime("(p (chord c3))").unwrap_err();
    assert_eq!(err, "chord expects root and chord name or interval vector");
}

#[test]
fn compiler_map_rejects_mismatched_vector_lengths() {
    let err = compile_source_for_runtime("(p (map and [1 0 1] [1 1]))").unwrap_err();
    assert_eq!(err, "map vector sources must have the same length");

    let compiled = compile_source_for_runtime("(p (map transpose [c3 d3] 12))").unwrap();
    assert!(
        compiled.contains("(p [c4 d4])"),
        "scalar map source should still broadcast across vector source: {}",
        compiled
    );
}

#[test]
fn compiler_interleave_rejects_mismatched_vector_lengths() {
    let err = compile_source_for_runtime("(p (interleave [1 1 1] [0]))").unwrap_err();
    assert_eq!(err, "interleave vectors must have the same length");

    let compiled = compile_source_for_runtime("(p (interleave [1 1] [0 0]))").unwrap();
    assert_eq!(compiled, "(p [1 0 1 0])");
}

#[test]
fn native_compile_command_writes_compiled_source() {
    let input = PathBuf::from(format!(
        "/tmp/glitchlisp-native-compile-command-{}.gl",
        std::process::id()
    ));
    let output = PathBuf::from(format!(
        "/tmp/glitchlisp-native-compile-command-{}.compiled.gl",
        std::process::id()
    ));
    std::fs::write(
        &input,
        "(def click
           (d :click :src :click :gate (p [1 0 0 0]) :dur 0.02 :amp 0.2))
         (scene :intro :loop true
           click)",
    )
    .unwrap();

    let args = vec![
        "glitchlisp-native".to_string(),
        "compile".to_string(),
        input.to_string_lossy().into_owned(),
        output.to_string_lossy().into_owned(),
    ];
    cli::run_with_args(&args).unwrap();

    let compiled = std::fs::read_to_string(&output).unwrap();
    assert!(compiled.contains("(d :click"));
    assert!(compiled.contains("(scene :intro"));
    assert!(!compiled.contains("(def click"));
    assert!(compiled.ends_with('\n'));

    let _ = std::fs::remove_file(input);
    let _ = std::fs::remove_file(output);
}

#[test]
fn native_compile_command_rejects_runtime_invalid_compiled_source() {
    let input = PathBuf::from(format!(
        "/tmp/glitchlisp-native-compile-invalid-{}.gl",
        std::process::id()
    ));
    let output = PathBuf::from(format!(
        "/tmp/glitchlisp-native-compile-invalid-{}.compiled.gl",
        std::process::id()
    ));
    std::fs::write(
        &input,
        "(d :lead
           :src :sine-synth
           :note c3
           :gate (times 2 (p [1 0 0 0] then [1 1 1 1])))
         (start!)",
    )
    .unwrap();

    let args = vec![
        "glitchlisp-native".to_string(),
        "compile".to_string(),
        input.to_string_lossy().into_owned(),
        output.to_string_lossy().into_owned(),
    ];
    let err = cli::run_with_args(&args).unwrap_err();
    assert!(
        err.contains("compiled source failed runtime validation"),
        "{}",
        err
    );
    assert!(
        err.contains("use (p (then A B)) instead of (p A then B)"),
        "{}",
        err
    );
    assert!(
        !output.exists(),
        "compile should not write output for runtime-invalid source"
    );

    let _ = std::fs::remove_file(input);
    let _ = std::fs::remove_file(output);
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

#[test]
fn gui_render_infers_obvious_scene_playback() {
    let input = PathBuf::from(format!(
        "/tmp/glitchlisp-gui-render-scene-{}.gl",
        std::process::id()
    ));
    let output = PathBuf::from(format!(
        "/tmp/glitchlisp-gui-render-scene-{}.wav",
        std::process::id()
    ));
    std::fs::write(
        &input,
        "(def click
           (d :click :src :click :gate 1 :dur 0.02 :amp 0.5))
         (scene :intro :loop true
           click)",
    )
    .unwrap();

    let stats = gui_render::render_selected_file(&input, 0.2, output.clone()).unwrap();
    assert_eq!(stats.frames, 9_600);
    assert!(stats.rms > 0.001);

    let _ = std::fs::remove_file(input);
    let _ = std::fs::remove_file(output);
}

#[test]
fn swing_render_preview_inference_uses_top_level_forms_only() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (let [def-only "(def click\n  (d :click :src :click :gate 1))"
            commented "(d :a :src :click :gate 1)\n; (start!)"
            stringed "(d :a :src :click :gate 1)\n\"(play-scene :intro)\""
            scene-comment "; (scene :fake :loop true)\n(scene :real :loop true\n  (d :a :src :click :gate 1))"
            nested-playback "(def commands\n  (play-scene :data))\n(scene :real :loop true\n  (d :a :src :click :gate 1))\n(play-scene :old)"
            vector-playback "[(play-scene :data)]\n(scene :real :loop true\n  (d :a :src :click :gate 1))\n(play-scene :old)"
            map-playback "{:cue (play-scene :data)}\n(scene :real :loop true\n  (d :a :src :click :gate 1))\n(play-scene :old)"
            hold-token-scene "(scene :real :loop true\n  (d :a :src :click :gate (p [1 0 1_3 0])))\n(play-scene :real)"
            preview-commented (glitchlisp.swing.render/preview-source commented)
            preview-stringed (glitchlisp.swing.render/preview-source stringed)
            preview-scene (glitchlisp.swing.render/preview-source scene-comment)
            preview-hold-token (glitchlisp.swing.render/preview-source hold-token-scene)
            recued (glitchlisp.swing.render/source-with-cue nested-playback "real")
            recued-vector (glitchlisp.swing.render/source-with-cue vector-playback "real")
            recued-map (glitchlisp.swing.render/source-with-cue map-playback "real")]
        (println (= def-only (glitchlisp.swing.render/preview-source def-only)))
        (println (clojure.string/ends-with? preview-commented "\n\n(start!)\n"))
        (println (clojure.string/ends-with? preview-stringed "\n\n(start!)\n"))
        (println (clojure.string/includes? preview-scene "(play-scene :real)"))
        (println (not (clojure.string/includes? preview-scene "(play-scene :fake)")))
        (println (= preview-hold-token hold-token-scene))
        (println (clojure.string/includes? recued "(play-scene :data)"))
        (println (not (clojure.string/includes? recued "(play-scene :old)")))
        (println (clojure.string/includes? recued "(play-scene :real)"))
        (println (clojure.string/includes? recued-vector "(play-scene :data)"))
        (println (not (clojure.string/includes? recued-vector "(play-scene :old)")))
        (println (clojure.string/includes? recued-map "(play-scene :data)"))
        (println (not (clojure.string/includes? recued-map "(play-scene :old)"))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing render preview smoke");
    assert!(
        output.status.success(),
        "swing render preview smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec![
            "true", "true", "true", "true", "true", "true", "true", "true", "true", "true", "true",
            "true", "true",
        ],
        "unexpected preview inference results: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_counts_sample_tracks_like_tracks() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (let [top-level "(sample :hit \"kick.wav\" :gate (p [1 0 0 0]))\n(start!)"
            scene "(scene :intro :loop true\n  (sample :hit \"kick.wav\" :gate (p [1 0 0 0])))\n(play-scene :intro)"
            every "(sample :hat \"hat.wav\" :gate (p [1 0]) :every 3)\n(start!)"]
        (println (= 4 (glitchlisp.swing.render/inferred-loop-steps top-level)))
        (println (= 4 (glitchlisp.swing.render/inferred-loop-steps scene)))
        (println (= 6 (glitchlisp.swing.render/inferred-loop-steps every))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing sample loop inference smoke");
    assert!(
        output.status.success(),
        "swing sample loop inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true"],
        "unexpected sample loop inference results: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_follows_finite_scene_chains() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (let [chain "(scene :intro :steps 7 :repeat 2 :next :drop\n  (d :a :src :click :gate 1))\n(scene :drop :steps 5 :repeat 1\n  (d :b :src :click :gate 1))\n(play-scene :intro)"
            looped (glitchlisp.swing.render/loop-render-source chain)
            cyclic "(scene :a :steps 4 :repeat 1 :next :b\n  (d :a :src :click :gate 1))\n(scene :b :steps 6 :repeat 1 :next :a\n  (d :b :src :click :gate 1))\n(play-scene :a)"]
        (println (= 19 (glitchlisp.swing.render/inferred-loop-steps chain)))
        (println (clojure.string/includes? looped "(scene :drop :steps 5 :repeat 1 :next :intro"))
        (println (= 10 (glitchlisp.swing.render/inferred-loop-steps cyclic))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing scene chain loop inference smoke");
    assert!(
        output.status.success(),
        "swing scene chain loop inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false")
        .rev()
        .take(2)
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true"],
        "unexpected scene chain loop inference results: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_missing_next_scene_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (try
        (glitchlisp.swing.render/inferred-loop-steps
          "(scene :intro :steps 4 :repeat 1 :next :drop\n  (d :kick :src :click :gate 1))\n(play-scene :intro)")
        (println "missing-error")
        (catch Exception ex
          (println (.getMessage ex))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing missing next scene inference smoke");
    assert!(
        output.status.success(),
        "swing missing next scene inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout
            .lines()
            .any(|line| line == "scene ':intro' :next references unknown scene ':drop'"),
        "loop inference should reject missing :next target before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_zero_count_times_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (try
        (glitchlisp.swing.render/inferred-loop-steps
          "(d :kick :src :click :gate (p (then (times 0 [1 0]) [1 1])))\n(start!)")
        (println "missing-error")
        (catch Exception ex
          (println (.getMessage ex))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing zero-times loop inference smoke");
    assert!(
        output.status.success(),
        "swing zero-times loop inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout
            .lines()
            .any(|line| line == "times must be greater than zero"),
        "loop inference should reject zero-count times before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_invalid_gate_holds_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (letfn [(check [source expected]
                (try
                  (glitchlisp.swing.render/inferred-loop-steps source)
                  (println "missing-error")
                  (catch Exception ex
                    (println (= expected (.getMessage ex))))))]
        (check "(d :lead :src :click :gate (p [(gate-hold 0)]))\n(start!)"
               "gate-hold must be greater than zero")
        (check "(d :lead :src :click :gate (p [(gate-hold 1.5)]))\n(start!)"
               "gate-hold must be a non-negative integer")
        (check "(d :lead :src :click :gate (p [(gate-hold 1 2)]))\n(start!)"
               "gate-hold expects zero or one amount")
        (println (= 4 (glitchlisp.swing.render/inferred-loop-steps
                       "(d :lead :src :click :gate (p [1 0 1_3 0]))\n(start!)"))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing invalid gate-hold smoke");
    assert!(
        output.status.success(),
        "swing invalid gate-hold smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false" || *line == "missing-error")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true", "true"],
        "loop inference should reject invalid gate-hold cells before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_non_numeric_gate_cells_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (letfn [(check [source]
                (try
                  (glitchlisp.swing.render/inferred-loop-steps source)
                  (println "missing-error")
                  (catch Exception ex
                    (println (= "expected numeric pattern value" (.getMessage ex))))))]
        (check "(d :lead :src :click :gate (p [bad]))\n(start!)")
        (check "(d :lead :src :click :gate (p [:bad]))\n(start!)")
        (check "(d :lead :src :click :gate (p [(unknown 1)]))\n(start!)"))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing non-numeric gate cell smoke");
    assert!(
        output.status.success(),
        "swing non-numeric gate cell smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false" || *line == "missing-error")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true"],
        "loop inference should reject non-numeric gate cells before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_invalid_note_cells_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (letfn [(check [source expected]
                (try
                  (glitchlisp.swing.render/inferred-loop-steps source)
                  (println "missing-error")
                  (catch Exception ex
                    (println (= expected (.getMessage ex))))))]
        (check "(d :lead :src :click :note (p [bad]) :gate 1)\n(start!)"
               "unknown symbol 'bad'")
        (check "(d :lead :src :click :note (p [:bad]) :gate 1)\n(start!)"
               "expected number or note")
        (check "(d :lead :src :click :note (p [(unknown 1)]) :gate 1)\n(start!)"
               "expected number or note")
        (check "(d :lead :src :click :note (p [[c3 :bad]]) :gate 1)\n(start!)"
               "expected number or note"))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing invalid note cell smoke");
    assert!(
        output.status.success(),
        "swing invalid note cell smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false" || *line == "missing-error")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true", "true"],
        "loop inference should reject invalid note cells before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_zero_every_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (try
        (glitchlisp.swing.render/inferred-loop-steps
          "(d :kick :src :click :gate 1 :every 0)\n(start!)")
        (println "missing-error")
        (catch Exception ex
          (println (.getMessage ex))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing zero-every loop inference smoke");
    assert!(
        output.status.success(),
        "swing zero-every loop inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout
            .lines()
            .any(|line| line == "every must be greater than zero"),
        "loop inference should reject zero :every before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_fractional_every_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (try
        (glitchlisp.swing.render/inferred-loop-steps
          "(d :kick :src :click :gate 1 :every 2.5)\n(start!)")
        (println "missing-error")
        (catch Exception ex
          (println (.getMessage ex))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing fractional-every loop inference smoke");
    assert!(
        output.status.success(),
        "swing fractional-every loop inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout
            .lines()
            .any(|line| line == "every must be a non-negative integer"),
        "loop inference should reject fractional :every before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_invalid_euclid_gates_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (letfn [(check [source expected]
                (try
                  (glitchlisp.swing.render/inferred-loop-steps source)
                  (println "missing-error")
                  (catch Exception ex
                    (println (= expected (.getMessage ex))))))]
        (check "(d :lead :src :click :gate (euclid 4 16 99))\n(start!)"
               "euclid expects pulses and steps")
        (check "(d :lead :src :click :gate (euclid 4.5 16))\n(start!)"
               "euclid pulses must be a non-negative integer")
        (check "(d :lead :src :click :gate (euclid 4 0))\n(start!)"
               "euclid steps must be greater than zero")
        (check "(d :lead :src :click :gate (euclid-rot 4 16 -1))\n(start!)"
               "euclid-rot rotation must be a non-negative integer"))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing invalid euclid loop inference smoke");
    assert!(
        output.status.success(),
        "swing invalid euclid loop inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false" || *line == "missing-error")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true", "true"],
        "loop inference should reject invalid euclid gates before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_pattern_wrapper_arity_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (letfn [(check [source expected]
                (try
                  (glitchlisp.swing.render/inferred-loop-steps source)
                  (println "missing-error")
                  (catch Exception ex
                    (println (= expected (.getMessage ex))))))]
        (check "(d :lead :src :click :note (p [c3] [d3]) :gate 1)\n(start!)"
               "p expects one vector")
        (check "(d :lead :src :click :note (gate-seq [c3] [d3]) :gate 1)\n(start!)"
               "gate-seq expects one vector")
        (check "(d :lead :src :click :gate (p [1 0] then [1 1]))\n(start!)"
               "p wraps exactly one pattern; use (p (then A B)) instead of (p A then B)")
        (check "(d :lead :src :click :gate (reverse (p [1 0]) (p [0 1])))\n(start!)"
               "reverse expects one pattern")
        (check "(d :lead :src :click :gate (times 2 [1 0] [0 1]))\n(start!)"
               "times expects count and one pattern")
        (check "(d :lead :src :click :gate (then [1 0]))\n(start!)"
               "then expects at least two patterns"))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing pattern wrapper arity loop inference smoke");
    assert!(
        output.status.success(),
        "swing pattern wrapper arity loop inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false" || *line == "missing-error")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true", "true", "true", "true"],
        "loop inference should reject malformed pattern wrappers before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_malformed_track_params_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (doseq [[source expected]
              [["(d :lead :src :click :gate 1 :ammp 0.8)\n(start!)"
                "unknown track parameter ':ammp'"]
               ["(d :lead :src :click :gate 1 :pulse-width 0.2 :pw 0.4)\n(start!)"
                "duplicate track parameter ':pw'"]
               ["(d :lead :src :click :gate)\n(start!)"
                "track parameter ':gate' requires a value"]
               ["(d :lead :src :click :gate 1 123 456)\n(start!)"
                "track parameters must be keyword/value pairs"]]]
        (try
          (glitchlisp.swing.render/inferred-loop-steps source)
          (println "missing-error")
          (catch Exception ex
            (println (= expected (.getMessage ex))))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing malformed track params loop inference smoke");
    assert!(
        output.status.success(),
        "swing malformed track params loop inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false" || *line == "missing-error")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true", "true"],
        "loop inference should reject malformed track params before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_invalid_offset_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (letfn [(check [source expected]
                (try
                  (glitchlisp.swing.render/inferred-loop-steps source)
                  (println "missing-error")
                  (catch Exception ex
                    (println (= expected (.getMessage ex))))))]
        (check "(d :lead :src :click :gate 1 :offset -1)\n(start!)"
               "offset must be a non-negative integer")
        (check "(d :lead :src :click :gate 1 :offset 1.5)\n(start!)"
               "offset must be a non-negative integer")
        (try
          (println (= 1 (glitchlisp.swing.render/inferred-loop-steps
                         "(d :lead :src :click :gate 1 :offset 1)\n(start!)")))
          (catch Exception _
            (println "missing-error")))
        (try
          (println (= 1 (glitchlisp.swing.render/inferred-loop-steps
                         "(d :lead :src :click :gate 1 :offset null)\n(start!)")))
          (catch Exception _
            (println "missing-error"))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing offset validation loop inference smoke");
    assert!(
        output.status.success(),
        "swing offset validation loop inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false" || *line == "missing-error")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true", "true"],
        "loop inference should reject invalid offset before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_invalid_detune_and_phase_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (letfn [(check [source expected]
                (try
                  (glitchlisp.swing.render/inferred-loop-steps source)
                  (println "missing-error")
                  (catch Exception ex
                    (println (= expected (.getMessage ex))))))]
        (check "(d :lead :src :sine-synth :note c3 :gate 1 :detune-cents :wide)\n(start!)"
               ":detune-cents expected number or note")
        (check "(d :lead :src :sine-synth :note c3 :gate 1 :detune :wide)\n(start!)"
               ":detune expected number or note")
        (check "(d :lead :src :sine-synth :note c3 :gate 1 :phase :late)\n(start!)"
               ":phase expected number or note")
        (check "(d :lead :src :sine-synth :note c3 :gate 1 :phase (p [0.1 :late]))\n(start!)"
               ":phase expected numeric pattern value")
        (try
          (println (= 1 (glitchlisp.swing.render/inferred-loop-steps
                         "(d :lead :src :sine-synth :note c3 :gate 1 :detune-cents (p [-5 7]) :phase (p [0.25 1.25]))\n(start!)")))
          (catch Exception _
            (println "missing-error")))
        (try
          (println (= 1 (glitchlisp.swing.render/inferred-loop-steps
                         "(d :lead :src :sine-synth :note c3 :gate 1 :detune-cents null :phase null)\n(start!)")))
          (catch Exception _
            (println "missing-error"))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing detune/phase validation loop inference smoke");
    assert!(
        output.status.success(),
        "swing detune/phase validation loop inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false" || *line == "missing-error")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true", "true", "true", "true"],
        "loop inference should reject invalid detune/phase before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_invalid_sources_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (letfn [(check [source expected]
                (try
                  (glitchlisp.swing.render/inferred-loop-steps source)
                  (println "missing-error")
                  (catch Exception ex
                    (println (= expected (.getMessage ex))))))]
        (check "(d :lead :src :bogus-synth :gate 1)\n(start!)"
               "unsupported source ':bogus-synth'")
        (check "(d :lead :src \"click\" :gate 1)\n(start!)"
               "source must be a keyword")
        (try
          (println (= 1 (glitchlisp.swing.render/inferred-loop-steps
                         "(d :lead :src null :gate 1)\n(start!)")))
          (catch Exception _
            (println "missing-error"))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing invalid sources loop inference smoke");
    assert!(
        output.status.success(),
        "swing invalid sources loop inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false" || *line == "missing-error")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true"],
        "loop inference should reject invalid sources before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_invalid_harmonics_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (letfn [(check [source expected]
                (try
                  (glitchlisp.swing.render/inferred-loop-steps source)
                  (println "missing-error")
                  (catch Exception ex
                    (println (= expected (.getMessage ex))))))]
        (check "(d :lead :src :additive :gate 1 :harmonics 1)\n(start!)"
               "harmonics must be a vector")
        (check "(d :lead :src :additive :gate 1 :harmonics [1 -0.1])\n(start!)"
               "harmonics must be between 0 and 2, got -0.1")
        (check "(d :lead :src :additive :gate 1 :harmonics [1 2.5])\n(start!)"
               "harmonics must be between 0 and 2, got 2.5")
        (check "(d :lead :src :additive :gate 1 :harmonics [1 1 1 1 1 1 1 1 1])\n(start!)"
               "harmonics accepts at most 8 values, got 9")
        (check "(d :lead :src :additive :gate 1 :harmonics [1 :bad])\n(start!)"
               "expected numeric pattern value")
        (try
          (println (= 1 (glitchlisp.swing.render/inferred-loop-steps
                         "(d :lead :src :additive :gate 1 :harmonics [1 0.5 0])\n(start!)")))
          (catch Exception _
            (println "missing-error"))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing invalid harmonics loop inference smoke");
    assert!(
        output.status.success(),
        "swing invalid harmonics loop inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false" || *line == "missing-error")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true", "true", "true", "true"],
        "loop inference should reject invalid harmonics before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_malformed_fx_values_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (letfn [(check [source expected]
                (try
                  (glitchlisp.swing.render/inferred-loop-steps source)
                  (println "missing-error")
                  (catch Exception ex
                    (println (= expected (.getMessage ex))))))]
        (check "(d :lead :src :sine-synth :note c3 :gate 1 :fx 123)\n(start!)"
               "fx must be a vector of effect forms")
        (check "(d :lead :src :sine-synth :note c3 :gate 1 :fx [123])\n(start!)"
               "effect must be a form")
        (try
          (println (= 1 (glitchlisp.swing.render/inferred-loop-steps
                         "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(delay :mix 0.2)])\n(start!)")))
          (catch Exception _
            (println "missing-error")))
        (try
          (println (= 1 (glitchlisp.swing.render/inferred-loop-steps
                         "(d :lead :src :sine-synth :note c3 :gate 1 :fx null)\n(start!)")))
          (catch Exception _
            (println "missing-error"))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing malformed fx loop inference smoke");
    assert!(
        output.status.success(),
        "swing malformed fx loop inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false" || *line == "missing-error")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true", "true"],
        "loop inference should reject malformed fx values before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_out_of_range_amp_and_dur_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (letfn [(check [source expected]
                (try
                  (glitchlisp.swing.render/inferred-loop-steps source)
                  (println "missing-error")
                  (catch Exception ex
                    (println (= expected (.getMessage ex))))))]
        (check "(d :lead :src :sine-synth :note c3 :gate 1 :amp 3)\n(start!)"
               ":amp amp must be between 0 and 1, got 3")
        (check "(d :lead :src :sine-synth :note c3 :gate 1 :amp (p [0.5 -0.1]))\n(start!)"
               ":amp amp must be between 0 and 1, got -0.1")
        (check "(d :lead :src :sine-synth :note c3 :gate 1 :dur 0)\n(start!)"
               ":dur dur must be between 0.005 and 4, got 0")
        (check "(d :lead :src :sine-synth :note c3 :gate 1 :dur (p [0.1 5]))\n(start!)"
               ":dur dur must be between 0.005 and 4, got 5")
        (check "(d :lead :src :sine-synth :note c3 :gate 1 :dur (p [0.1 :short]))\n(start!)"
               ":dur expected numeric pattern value")
        (try
          (println (= 1 (glitchlisp.swing.render/inferred-loop-steps
                         "(d :lead :src :sine-synth :note c3 :gate 1 :amp (p [0 1]) :dur (p [0.005 4]))\n(start!)")))
          (catch Exception _
            (println "missing-error")))
        (try
          (println (= 1 (glitchlisp.swing.render/inferred-loop-steps
                         "(d :lead :src :sine-synth :note c3 :gate 1 :amp null :dur null)\n(start!)")))
          (catch Exception _
            (println "missing-error"))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing amp/dur range loop inference smoke");
    assert!(
        output.status.success(),
        "swing amp/dur range loop inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false" || *line == "missing-error")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true", "true", "true", "true", "true"],
        "loop inference should reject out-of-range amp/dur before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_out_of_range_oscillator_params_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (letfn [(check [source expected]
                (try
                  (glitchlisp.swing.render/inferred-loop-steps source)
                  (println "missing-error")
                  (catch Exception ex
                    (println (= expected (.getMessage ex))))))]
        (check "(d :lead :src :pulse :note c3 :gate 1 :pulse-width 1)\n(start!)"
               ":pulse-width pulse-width must be between 0.01 and 0.99, got 1")
        (check "(d :lead :src :morph :note c3 :gate 1 :morph -0.1)\n(start!)"
               ":morph morph must be between 0 and 1, got -0.1")
        (check "(d :lead :src :sine-synth :note c3 :gate 1 :gain 3)\n(start!)"
               ":gain gain must be between 0 and 2, got 3")
        (check "(d :lead :src :sine-synth :note c3 :gate 1 :unison-detune 120)\n(start!)"
               ":unison-detune unison-detune must be between 0 and 100, got 120")
        (check "(d :lead :src :sine-synth :note c3 :gate 1 :unison-spread -0.1)\n(start!)"
               ":unison-spread unison-spread must be between 0 and 1, got -0.1")
        (check "(d :lead :src :fm-op :note c3 :gate 1 :fm-ratio 0)\n(start!)"
               ":fm-ratio fm-ratio must be at least 0.01, got 0")
        (check "(d :lead :src :fm-op :note c3 :gate 1 :fm-depth 33)\n(start!)"
               ":fm-depth fm-depth must be between 0 and 32, got 33")
        (check "(d :lead :src :pulse :note c3 :gate 1 :pulse-width (p [0.5 1]))\n(start!)"
               ":pulse-width pulse-width must be between 0.01 and 0.99, got 1")
        (try
          (println (= 1 (glitchlisp.swing.render/inferred-loop-steps
                         "(d :lead :src :fm-op :note c3 :gate 1 :pulse-width null :morph null :gain null :unison-detune null :unison-spread null :fm-ratio null :fm-depth null)\n(start!)")))
          (catch Exception _
            (println "missing-error")))
        (try
          (println (= 1 (glitchlisp.swing.render/inferred-loop-steps
                         "(d :lead :src :fm-op :note c3 :gate 1 :pulse-width (p [0.01 0.99]) :morph (p [0 1]) :gain (p [0 2]) :unison-detune (p [0 100]) :unison-spread (p [0 1]) :fm-ratio (p [0.01 2]) :fm-depth (p [0 32]))\n(start!)")))
          (catch Exception _
            (println "missing-error"))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing oscillator range loop inference smoke");
    assert!(
        output.status.success(),
        "swing oscillator range loop inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false" || *line == "missing-error")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec![
            "true", "true", "true", "true", "true", "true", "true", "true", "true", "true"
        ],
        "loop inference should reject out-of-range oscillator params before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_invalid_unison_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (letfn [(check [source expected]
                (try
                  (glitchlisp.swing.render/inferred-loop-steps source)
                  (println "missing-error")
                  (catch Exception ex
                    (println (= expected (.getMessage ex))))))]
        (check "(d :lead :src :sine-synth :note c3 :gate 1 :unison 0)\n(start!)"
               ":unison unison must be between 1 and 10, got 0")
        (check "(d :lead :src :sine-synth :note c3 :gate 1 :unison 11)\n(start!)"
               ":unison unison must be between 1 and 10, got 11")
        (check "(d :lead :src :sine-synth :note c3 :gate 1 :unison (p [1.5 3]))\n(start!)"
               ":unison unison must be a non-negative integer")
        (check "(d :lead :src :sine-synth :note c3 :gate 1 :unison (p [-1 3]))\n(start!)"
               ":unison unison must be a non-negative integer")
        (check "(d :lead :src :sine-synth :note c3 :gate 1 :unison (p [1 11]))\n(start!)"
               ":unison unison must be between 1 and 10, got 11")
        (try
          (println (= 1 (glitchlisp.swing.render/inferred-loop-steps
                         "(d :lead :src :sine-synth :note c3 :gate 1 :unison (p [1 10]))\n(start!)")))
          (catch Exception _
            (println "missing-error")))
        (try
          (println (= 1 (glitchlisp.swing.render/inferred-loop-steps
                         "(d :lead :src :sine-synth :note c3 :gate 1 :unison null)\n(start!)")))
          (catch Exception _
            (println "missing-error"))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing unison validation loop inference smoke");
    assert!(
        output.status.success(),
        "swing unison validation loop inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false" || *line == "missing-error")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true", "true", "true", "true", "true"],
        "loop inference should reject invalid unison before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_malformed_sample_headers_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (letfn [(check [source expected]
                (try
                  (glitchlisp.swing.render/inferred-loop-steps source)
                  (println "missing-error")
                  (catch Exception ex
                    (println (= expected (.getMessage ex))))))]
        (check "(sample hit \"kick.wav\" :gate 1)\n(start!)"
               "sample track id must be a keyword")
        (check "(sample :hit)\n(start!)"
               "sample requires a wav path or :sample-data")
        (check "(sample :hit \"kick.wav\" 1)\n(start!)"
               "sample options must be keyword/value pairs")
        (check "(sample :hit \"kick.wav\" :amp)\n(start!)"
               "sample :amp requires a value")
        (check "(sample :hit 123 :gate 1)\n(start!)"
               "expected string")
        (check "(sample :hit :amp 0.5)\n(start!)"
               "sample requires a wav path or :sample-data")
        (check "(sample :hit :sample-data [])\n(start!)"
               "sample-data requires at least one value")
        (check "(sample :hit :sample-data 1)\n(start!)"
               "sample-data must be a vector")
        (check "(sample :hit :sample-data [1 bad])\n(start!)"
               "unknown symbol 'bad'")
        (check "(sample :hit :sample-data [1 :bad])\n(start!)"
               "expected number or note")
        (check "(d :hit :src :sample :sample-data [1 :bad] :gate 1)\n(start!)"
               "expected number or note")
        (check "(d :hit :src :sample :sample-path 123 :gate 1)\n(start!)"
               "expected string")
        (check "(d :hit :src :sample :sample 123 :gate 1)\n(start!)"
               "expected string")
        (try
          (println (= 1 (glitchlisp.swing.render/inferred-loop-steps
                         "(d :hit :src :sample :sample-path null :gate 1)\n(start!)")))
          (catch Exception _
            (println "missing-error")))
        (println (= 2 (glitchlisp.swing.render/inferred-loop-steps
                       "(sample :hit :sample-data [1 0 -1] :gate (p [1 0]))\n(start!)")))
        (println (= 1 (glitchlisp.swing.render/inferred-loop-steps
                       "(sample :hit :sample-data [c3] :gate 1)\n(start!)")))
      )
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing malformed sample headers loop inference smoke");
    assert!(
        output.status.success(),
        "swing malformed sample headers loop inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false" || *line == "missing-error")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec![
            "true", "true", "true", "true", "true", "true", "true", "true", "true", "true", "true",
            "true", "true", "true", "true", "true",
        ],
        "loop inference should reject malformed sample headers and keep inline sample data valid: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_zero_scene_steps_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (try
        (glitchlisp.swing.render/inferred-loop-steps
          "(scene :intro :steps 0 :repeat 1\n  (d :kick :src :click :gate 1))\n(play-scene :intro)")
        (println "missing-error")
        (catch Exception ex
          (println (.getMessage ex))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing zero-step loop inference smoke");
    assert!(
        output.status.success(),
        "swing zero-step loop inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout
            .lines()
            .any(|line| line == "steps must be greater than zero"),
        "loop inference should reject zero scene steps before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_empty_scene_without_steps_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (try
        (glitchlisp.swing.render/inferred-loop-steps
          "(scene :intro :repeat 1)\n(play-scene :intro)")
        (println "missing-error")
        (catch Exception ex
          (println (.getMessage ex))))
      (println (= 8 (glitchlisp.swing.render/inferred-loop-steps
                     "(scene :intro :steps 8 :repeat 1)\n(play-scene :intro)")))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing empty scene inference smoke");
    assert!(
        output.status.success(),
        "swing empty scene inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout
            .lines()
            .any(|line| line == "scene has nothing to play; add a track or set :steps explicitly"),
        "loop inference should reject empty inferred scenes before estimating render length: {}",
        stdout
    );
    assert!(
        stdout.lines().any(|line| line == "true"),
        "explicit scene steps should keep an empty timed scene previewable: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_loop_false_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (try
        (glitchlisp.swing.render/inferred-loop-steps
          "(scene :intro :loop false\n  (d :kick :src :click :gate 1))\n(play-scene :intro)")
        (println "missing-error")
        (catch Exception ex
          (println (.getMessage ex))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing loop false inference smoke");
    assert!(
        output.status.success(),
        "swing loop false inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout
            .lines()
            .any(|line| line == "scene :loop only accepts true; use :repeat N for finite scenes"),
        "loop inference should reject :loop false before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_unknown_steps_of_track_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (try
        (glitchlisp.swing.render/inferred-loop-steps
          "(scene :intro :steps-of :missing\n  (d :kick :src :click :gate 1))\n(play-scene :intro)")
        (println "missing-error")
        (catch Exception ex
          (println (.getMessage ex))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing steps-of inference smoke");
    assert!(
        output.status.success(),
        "swing steps-of inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout
            .lines()
            .any(|line| line == "scene :steps-of references unknown track ':missing'"),
        "loop inference should reject unknown :steps-of target before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_duplicate_scene_options_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (try
        (glitchlisp.swing.render/inferred-loop-steps
          "(scene :intro :repeat 2 :times 4\n  (d :kick :src :click :gate 1))\n(play-scene :intro)")
        (println "missing-error")
        (catch Exception ex
          (println (.getMessage ex))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing duplicate scene option smoke");
    assert!(
        output.status.success(),
        "swing duplicate scene option smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout
            .lines()
            .any(|line| line == "duplicate scene option ':times'"),
        "loop inference should reject duplicate scene option aliases: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_unknown_scene_options_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (try
        (glitchlisp.swing.render/inferred-loop-steps
          "(scene :intro :repeet 2\n  (d :kick :src :click :gate 1))\n(play-scene :intro)")
        (println "missing-error")
        (catch Exception ex
          (println (.getMessage ex))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing unknown scene option smoke");
    assert!(
        output.status.success(),
        "swing unknown scene option smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout
            .lines()
            .any(|line| line == "unknown scene option ':repeet'"),
        "loop inference should reject unknown scene options: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_bad_scene_option_values_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (letfn [(check [source expected]
                (try
                  (glitchlisp.swing.render/inferred-loop-steps source)
                  (println "missing-error")
                  (catch Exception ex
                    (println (= expected (.getMessage ex))))))]
        (check "(scene :intro :repeat)\n(play-scene :intro)"
               "scene :repeat requires a value")
        (check "(scene :intro :steps)\n(play-scene :intro)"
               "scene :steps requires a value")
        (check "(scene :intro :next 123\n  (d :kick :src :click :gate 1))\n(play-scene :intro)"
               "scene :next requires a keyword argument")
        (check "(scene :intro :steps-of 123\n  (d :kick :src :click :gate 1))\n(play-scene :intro)"
               "scene :steps-of requires a keyword argument"))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing bad scene option values smoke");
    assert!(
        output.status.success(),
        "swing bad scene option values smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false" || *line == "missing-error")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true", "true"],
        "loop inference should reject bad scene option values before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_supports_custom_scene_bar_steps() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (println
        (glitchlisp.swing.render/scene-steps-from-form
          (read-string "(scene :intro :bars 2 :bar-steps 8 (d :kick :src :click :gate (p [1 0 0 0])))")))
      (println
        (glitchlisp.swing.render/scene-steps-from-form
          (read-string "(scene :intro :bars 3 :bar-steps-of :kick (d :kick :src :click :gate (p [1 0 0])))")))
      (try
        (glitchlisp.swing.render/scene-steps-from-form
          (read-string "(scene :intro :bar-steps 8 (d :kick :src :click :gate 1))"))
        (println "missing-error")
        (catch Exception ex
          (println (.getMessage ex))))
      (try
        (glitchlisp.swing.render/scene-steps-from-form
          (read-string "(scene :intro :bars 2 :bar-steps-of :missing (d :kick :src :click :gate 1))"))
        (println "missing-error")
        (catch Exception ex
          (println (.getMessage ex))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing custom scene bar steps smoke");
    assert!(
        output.status.success(),
        "swing custom scene bar steps smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.lines().any(|line| line == "16")
            && stdout.lines().any(|line| line == "9")
            && stdout
                .lines()
                .any(|line| line == "scene :bar-steps requires :bars")
            && stdout
                .lines()
                .any(|line| { line == "scene :bar-steps-of references unknown track ':missing'" }),
        "swing loop inference should support custom bar step options: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_supports_scene_loop_by() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (println
        (glitchlisp.swing.render/scene-steps-from-form
          (read-string "(scene :intro :loop-by :click 4 (d :click :src :click :gate (p [1 0 0 0])))")))
      (println
        (glitchlisp.swing.render/scene-repeat-from-form
          (read-string "(scene :intro :loop-by :click 4 (d :click :src :click :gate (p [1 0 0 0])))")))
      (println
        (glitchlisp.swing.render/scene-repeat-from-form
          (read-string "(scene :intro :loop-by :click 4 :next :hams (d :click :src :click :gate (p [1 0 0 0])))")))
      (println
        (glitchlisp.swing.render/scene-repeat-from-form
          (read-string "(scene :intro :loop-by :click 4 :next :hams :loop true (d :click :src :click :gate (p [1 0 0 0])))")))
      (try
        (glitchlisp.swing.render/scene-steps-from-form
          (read-string "(scene :intro :loop-by :missing 4 (d :click :src :click :gate (p [1 0 0 0])))"))
        (println "missing-error")
        (catch Exception ex
          (println (.getMessage ex))))
      (try
        (glitchlisp.swing.render/scene-steps-from-form
          (read-string "(scene :intro :loop-by :click 0 (d :click :src :click :gate (p [1 0 0 0])))"))
        (println "missing-error")
        (catch Exception ex
          (println (.getMessage ex))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing scene loop-by smoke");
    assert!(
        output.status.success(),
        "swing scene loop-by smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.lines().any(|line| line == "16")
            && stdout.lines().any(|line| line == "0")
            && stdout.lines().any(|line| line == "1")
            && stdout
                .lines()
                .any(|line| line == "scene :loop-by references unknown track ':missing'")
            && stdout
                .lines()
                .any(|line| line == "loop-by must be greater than zero"),
        "swing loop inference should support :loop-by and reject bad values: {}",
        stdout
    );
}

#[test]
fn swing_loop_inference_rejects_invalid_repeat_counts_like_runtime() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (letfn [(check [source expected]
                (try
                  (glitchlisp.swing.render/inferred-loop-steps source)
                  (println "missing-error")
                  (catch Exception ex
                    (println (= expected (.getMessage ex))))))]
        (check "(scene :intro :repeat -1\n  (d :kick :src :click :gate 1))\n(play-scene :intro)"
               "repeat must be a non-negative integer")
        (check "(scene :intro :repeat 1.5\n  (d :kick :src :click :gate 1))\n(play-scene :intro)"
               "repeat must be a non-negative integer")
        (check "(scene :intro :repeat :twice\n  (d :kick :src :click :gate 1))\n(play-scene :intro)"
               "repeat must be numeric"))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing invalid repeat count smoke");
    assert!(
        output.status.success(),
        "swing invalid repeat count smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false" || *line == "missing-error")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true"],
        "loop inference should reject invalid repeat counts before estimating render length: {}",
        stdout
    );
}

#[test]
fn swing_loop_seconds_uses_top_level_runtime_bpm() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (let [commented "; (bpm 60)\n\"(bpm 80)\"\n(bpm 120)\n(d :a :src :click :gate 1)"
            repeated "(bpm 60)\n(bpm 120)\n(d :a :src :click :gate 1)"]
        (println (= 120.0 (glitchlisp.swing.render/bpm-from-source commented)))
        (println (= 120.0 (glitchlisp.swing.render/bpm-from-source repeated)))
        (println (= 1.0 (glitchlisp.swing.render/seconds-for-steps repeated 8))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing bpm inference smoke");
    assert!(
        output.status.success(),
        "swing bpm inference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true"],
        "unexpected swing bpm inference results: {}",
        stdout
    );
}

#[test]
fn native_capabilities_cover_current_gui_language_features() {
    let capabilities = cli::capabilities()
        .split_whitespace()
        .collect::<std::collections::HashSet<_>>();
    for required in [
        "null-params",
        "empty-gate-silent",
        "gui-live",
        "live-audio-info",
        "check-live-source",
        "gate-then-times",
        "scene-loop-true",
        "scene-loop-by",
        "sample-form",
        "gui-render-preview",
        "drum-note-pitch",
        "native-compiler-source",
        "native-compile-command",
    ] {
        assert!(capabilities.contains(required), "missing {}", required);
    }
}

#[test]
fn cli_playback_hint_explains_valid_but_stopped_files() {
    let mut scene_runtime = Runtime::new();
    eval_program(
        &mut scene_runtime,
        "(scene :intro :loop true
           (d :click :src :click :gate 1))",
    )
    .unwrap();
    assert_eq!(
        cli::playback_hint(&scene_runtime),
        Some(
            "no scene is playing; add (play-scene :scene-name) or use the editor preview/render path"
        )
    );

    let mut track_runtime = Runtime::new();
    eval_program(&mut track_runtime, "(d :click :src :click :gate 1)").unwrap();
    assert_eq!(
        cli::playback_hint(&track_runtime),
        Some(
            "tracks are defined but playback is stopped; add (start!) or wrap tracks in a scene and call (play-scene :scene-name)"
        )
    );

    eval_program(&mut track_runtime, "(start!)").unwrap();
    assert_eq!(cli::playback_hint(&track_runtime), None);

    let mut post_fx_runtime = Runtime::new();
    eval_program(&mut post_fx_runtime, "(post-fx [(reverb :mix 0.2)])").unwrap();
    assert_eq!(
        cli::playback_hint(&post_fx_runtime),
        Some("post-fx is defined but there is no audio source; add a track or scene to render")
    );
}

#[test]
fn live_file_startup_rejects_valid_but_stopped_source_before_audio() {
    let input = PathBuf::from(format!(
        "/tmp/glitchlisp-live-stopped-source-{}.gl",
        std::process::id()
    ));
    std::fs::write(
        &input,
        "(scene :intro :loop true
           (d :click :src :click :gate 1))",
    )
    .unwrap();

    let args = vec![
        "glitchlisp-native".to_string(),
        "live".to_string(),
        input.to_string_lossy().into_owned(),
    ];
    let err = cli::run_with_args(&args).unwrap_err();
    assert!(
        err.contains("no scene is playing; add (play-scene :scene-name)"),
        "{}",
        err
    );

    let _ = std::fs::remove_file(input);
}

#[test]
fn native_interactive_render_rejects_valid_but_stopped_runtime_before_writing() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(scene :intro :loop true
           (d :click :src :click :gate 1))",
    )
    .unwrap();
    let output = PathBuf::from(format!(
        "/tmp/glitchlisp-interactive-render-stopped-{}.wav",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&output);

    let err = match cli::render_interactive_runtime(runtime, output.clone()) {
        Ok(_) => panic!("interactive render should reject stopped runtime"),
        Err(err) => err,
    };
    assert!(
        err.contains("no scene is playing; add (play-scene :scene-name)"),
        "{}",
        err
    );
    assert!(
        !output.exists(),
        "interactive render should fail before writing silent audio"
    );
}

#[test]
fn native_interactive_render_requires_output_path() {
    assert_eq!(
        cli::interactive_render_path("").unwrap_err(),
        "render requires an output path"
    );
    assert_eq!(
        cli::interactive_render_path("   ").unwrap_err(),
        "render requires an output path"
    );
    assert_eq!(
        cli::interactive_render_path(" out.wav ").unwrap(),
        PathBuf::from("out.wav")
    );
}

#[test]
fn cli_seconds_option_rejects_ambiguous_values() {
    let args = vec!["glitchlisp-native".to_string(), "tone".to_string()];
    assert_eq!(cli::parse_seconds(&args, 2.0).unwrap(), 2.0);

    let args = vec![
        "glitchlisp-native".to_string(),
        "tone".to_string(),
        "--seconds".to_string(),
        "0.25".to_string(),
    ];
    assert_eq!(cli::parse_seconds(&args, 2.0).unwrap(), 0.25);

    let args = vec![
        "glitchlisp-native".to_string(),
        "tone".to_string(),
        "--seconds".to_string(),
    ];
    assert_eq!(
        cli::parse_seconds(&args, 2.0).unwrap_err(),
        "--seconds requires a numeric value"
    );

    let args = vec![
        "glitchlisp-native".to_string(),
        "tone".to_string(),
        "--seconds".to_string(),
        "soon".to_string(),
    ];
    assert_eq!(
        cli::parse_seconds(&args, 2.0).unwrap_err(),
        "--seconds must be numeric, got 'soon'"
    );

    let args = vec![
        "glitchlisp-native".to_string(),
        "tone".to_string(),
        "--seconds".to_string(),
        "0".to_string(),
    ];
    assert_eq!(
        cli::parse_seconds(&args, 2.0).unwrap_err(),
        "--seconds must be greater than 0, got '0'"
    );

    let args = vec![
        "glitchlisp-native".to_string(),
        "tone".to_string(),
        "--seconds".to_string(),
        "1".to_string(),
        "--seconds".to_string(),
        "2".to_string(),
    ];
    assert_eq!(
        cli::parse_seconds(&args, 2.0).unwrap_err(),
        "duplicate option '--seconds'"
    );
}

#[test]
fn cli_positional_args_ignore_known_options() {
    let args = vec![
        "glitchlisp-native".to_string(),
        "render".to_string(),
        "--seconds".to_string(),
        "0.5".to_string(),
        "input.gl".to_string(),
        "out.wav".to_string(),
    ];
    assert_eq!(
        cli::positional_args(&args).unwrap(),
        vec!["input.gl".to_string(), "out.wav".to_string()]
    );

    let args = vec![
        "glitchlisp-native".to_string(),
        "render".to_string(),
        "input.gl".to_string(),
        "out.wav".to_string(),
        "--seconds".to_string(),
        "0.5".to_string(),
    ];
    assert_eq!(
        cli::positional_args(&args).unwrap(),
        vec!["input.gl".to_string(), "out.wav".to_string()]
    );

    let args = vec![
        "glitchlisp-native".to_string(),
        "render".to_string(),
        "--loud".to_string(),
        "input.gl".to_string(),
    ];
    assert_eq!(
        cli::positional_args(&args).unwrap_err(),
        "unknown option '--loud'"
    );

    let args = vec![
        "glitchlisp-native".to_string(),
        "render".to_string(),
        "--seconds".to_string(),
        "0.5".to_string(),
        "input.gl".to_string(),
        "out.wav".to_string(),
        "--seconds".to_string(),
        "1".to_string(),
    ];
    assert_eq!(
        cli::positional_args(&args).unwrap_err(),
        "duplicate option '--seconds'"
    );
}

#[test]
fn cli_commands_reject_extra_args_and_unknown_options_before_startup() {
    let args = vec![
        "glitchlisp-native".to_string(),
        "play".to_string(),
        "a.gl".to_string(),
        "b.gl".to_string(),
    ];
    assert_eq!(
        cli::run_with_args(&args).unwrap_err(),
        "play expects exactly 1 positional argument"
    );

    let args = vec![
        "glitchlisp-native".to_string(),
        "compile".to_string(),
        "in.gl".to_string(),
        "out.gl".to_string(),
        "extra.gl".to_string(),
    ];
    assert_eq!(
        cli::run_with_args(&args).unwrap_err(),
        "compile expects exactly 2 positional arguments"
    );

    let args = vec![
        "glitchlisp-native".to_string(),
        "compile".to_string(),
        "in.gl".to_string(),
        "out.gl".to_string(),
        "--seconds".to_string(),
        "1".to_string(),
    ];
    assert_eq!(
        cli::run_with_args(&args).unwrap_err(),
        "unknown option '--seconds'"
    );

    let args = vec![
        "glitchlisp-native".to_string(),
        "live".to_string(),
        "session.gl".to_string(),
        "extra.gl".to_string(),
    ];
    assert_eq!(
        cli::run_with_args(&args).unwrap_err(),
        "live expects at most 1 positional argument"
    );

    let args = vec![
        "glitchlisp-native".to_string(),
        "edit".to_string(),
        "--seconds".to_string(),
        "1".to_string(),
    ];
    assert_eq!(
        cli::run_with_args(&args).unwrap_err(),
        "unknown option '--seconds'"
    );

    let args = vec![
        "glitchlisp-native".to_string(),
        "gui-live".to_string(),
        "--device".to_string(),
    ];
    assert_eq!(
        cli::run_with_args(&args).unwrap_err(),
        "--device requires a value"
    );

    let args = vec![
        "glitchlisp-native".to_string(),
        "gui-live".to_string(),
        "--device".to_string(),
        "A".to_string(),
        "--device".to_string(),
        "B".to_string(),
    ];
    assert_eq!(
        cli::run_with_args(&args).unwrap_err(),
        "duplicate option '--device'"
    );

    let args = vec![
        "glitchlisp-native".to_string(),
        "gui-render".to_string(),
        "--seconds".to_string(),
        "1".to_string(),
        "--seconds".to_string(),
        "2".to_string(),
    ];
    assert_eq!(
        cli::run_with_args(&args).unwrap_err(),
        "duplicate option '--seconds'"
    );

    let args = vec![
        "glitchlisp-native".to_string(),
        "check-live-source".to_string(),
        "a.gl".to_string(),
        "b.gl".to_string(),
    ];
    assert_eq!(
        cli::run_with_args(&args).unwrap_err(),
        "check-live-source expects exactly 1 positional argument"
    );

    let args = vec![
        "glitchlisp-native".to_string(),
        "check-live-source".to_string(),
        "/tmp/glitchlisp-missing-live-source.gl".to_string(),
    ];
    let err = cli::run_with_args(&args).unwrap_err();
    assert!(
        err.contains("/tmp/glitchlisp-missing-live-source.gl:"),
        "{}",
        err
    );

    let args = vec![
        "glitchlisp-native".to_string(),
        "capabilities".to_string(),
        "extra".to_string(),
    ];
    assert_eq!(
        cli::run_with_args(&args).unwrap_err(),
        "capabilities expects exactly 0 positional arguments"
    );
}

#[test]
fn native_live_status_summary_names_active_scene() {
    let mut runtime = Runtime::new();
    let source = "(scene :intro :loop true
                    (d :lead :src :sine-synth :note c3 :gate 1))
                  (scene :help :loop true
                    (d :lead :src :sine-synth :note e3 :gate 1))
                  (play-scene :intro)";
    let compiled = compile_source_for_runtime(source).unwrap();
    eval_program(&mut runtime, &compiled).unwrap();

    let status = cli::live_status_summary(&runtime);
    assert!(
        status.contains("running=true"),
        "live status should show running state: {}",
        status
    );
    assert!(
        status.contains("tracks=1"),
        "live status should show active track count: {}",
        status
    );
    assert!(
        status.contains("scenes=2"),
        "live status should show defined scene count: {}",
        status
    );
    assert!(
        status.contains("scene=:intro"),
        "live status should show active scene name: {}",
        status
    );
    assert!(
        status.contains("cycle=1/loop"),
        "live status should show current scene cycle: {}",
        status
    );

    runtime.running = false;
    let stopped_status = cli::live_status_summary(&runtime);
    assert!(
        stopped_status.contains("scene=-"),
        "stopped live status should hide stale scene state: {}",
        stopped_status
    );
    assert!(
        !stopped_status.contains("scene=:intro"),
        "stopped live status should not show an old active scene: {}",
        stopped_status
    );
    assert!(
        stopped_status.contains("cycle=-"),
        "stopped live status should hide stale scene cycle: {}",
        stopped_status
    );
}

#[test]
fn native_interactive_success_summary_names_active_scene() {
    let mut runtime = Runtime::new();
    let source = "(scene :intro :loop true
                    (d :lead :src :sine-synth :note c3 :gate 1))
                  (play-scene :intro)";
    cli::eval_interactive_source(&mut runtime, source).unwrap();

    let success = format!("ok {}", cli::live_status_summary(&runtime));
    assert!(
        success.contains("running=true"),
        "interactive success should show running state: {}",
        success
    );
    assert!(
        success.contains("scenes=1"),
        "interactive success should show scene count: {}",
        success
    );
    assert!(
        success.contains("scene=:intro"),
        "interactive success should show active scene: {}",
        success
    );
    assert!(
        success.contains("cycle=1/loop"),
        "interactive success should show active scene cycle: {}",
        success
    );
}

#[test]
fn native_editor_run_message_names_active_scene() {
    let runtime = Arc::new(Mutex::new(Runtime::new()));
    let source = "(scene :intro :repeat 4
                    (d :lead :src :sine-synth :note c3 :gate 1))
                  (play-scene :intro)";
    let snapshot = apply_runtime_source(&runtime, &editor::editor_preview_source(source)).unwrap();
    let message = editor::editor_run_message(&snapshot);
    assert!(
        message.contains("running bpm="),
        "editor run message should stay recognizable: {}",
        message
    );
    assert!(
        message.contains("scenes=1"),
        "editor run message should show scene count: {}",
        message
    );
    assert!(
        message.contains("scene=:intro"),
        "editor run message should show active scene: {}",
        message
    );
    assert!(
        message.contains("cycle=1/4"),
        "editor run message should show counted scene cycle: {}",
        message
    );

    let mut stopped_snapshot = snapshot.clone();
    stopped_snapshot.running = false;
    stopped_snapshot.scene_state = None;
    let stopped = editor::editor_stop_message(&stopped_snapshot);
    assert!(
        stopped.contains("stopped bpm=")
            && stopped.contains("running=false")
            && stopped.contains("tracks=1")
            && stopped.contains("scenes=1"),
        "editor stop message should preserve state counts: {}",
        stopped
    );
    assert!(
        stopped.contains("scene=-") && stopped.contains("cycle=-"),
        "editor stop message should hide stale scene state: {}",
        stopped
    );

    let cued = editor::editor_scene_message("cued", "intro", &snapshot);
    assert!(
        cued.contains("cued scene :intro")
            && cued.contains("running=true")
            && cued.contains("tracks=1")
            && cued.contains("scene=:intro")
            && cued.contains("cycle=1/4"),
        "editor cue message should include scene runtime status: {}",
        cued
    );
}

#[test]
fn render_rejects_invalid_seconds_before_loading_input_file() {
    let args = vec![
        "glitchlisp-native".to_string(),
        "render".to_string(),
        "/tmp/glitchlisp-missing-input.gl".to_string(),
        "/tmp/glitchlisp-output.wav".to_string(),
        "--seconds".to_string(),
        "soon".to_string(),
    ];
    assert_eq!(
        cli::run_with_args_quiet(&args).unwrap_err(),
        "--seconds must be numeric, got 'soon'"
    );
}

#[test]
fn gui_render_seconds_rejects_ambiguous_values() {
    assert_eq!(gui_render::parse_render_seconds("0.5").unwrap(), 0.5);
    assert_eq!(
        gui_render::parse_render_seconds("soon").unwrap_err(),
        "render seconds must be numeric, got 'soon'"
    );
    assert_eq!(
        gui_render::parse_render_seconds("-1").unwrap_err(),
        "render seconds must be greater than 0, got '-1'"
    );
}

#[test]
fn cli_usage_paths_do_not_hide_typos() {
    let args = vec!["glitchlisp-native".to_string()];
    assert_eq!(
        cli::run_with_args_quiet(&args).unwrap_err(),
        "missing command"
    );

    let args = vec!["glitchlisp-native".to_string(), "frobnicate".to_string()];
    assert_eq!(
        cli::run_with_args_quiet(&args).unwrap_err(),
        "unknown command 'frobnicate'"
    );

    let args = vec!["glitchlisp-native".to_string(), "--help".to_string()];
    assert!(cli::run_with_args_quiet(&args).is_ok());
    for alias in ["help", "--help", "-h"] {
        assert!(
            cli::is_help_command(alias),
            "help alias '{}' should be recognized by the CLI help matcher",
            alias
        );
    }
    assert!(!cli::is_help_command("helps"));
}

#[test]
fn native_cli_usage_and_readme_list_supported_commands() {
    let usage = cli::usage_text();
    let readme = std::fs::read_to_string("README.md").expect("read README.md");
    let expected_readme_block = usage
        .lines()
        .skip(1)
        .map(|line| line.replacen("  glitchlisp-native", "./glitchlisp-native", 1))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        readme.contains(&expected_readme_block),
        "README native command block should mirror CLI usage order and syntax:\n{}",
        expected_readme_block
    );
    for command in [
        "help",
        "tone",
        "play",
        "live",
        "gui-live",
        "edit",
        "gui-render",
        "compile-gui",
        "render",
        "compile",
        "check-live-source",
        "capabilities",
        "devices",
        "devices-plain",
        "repl",
    ] {
        let usage_line = format!("glitchlisp-native {}", command);
        assert!(
            usage.contains(&usage_line),
            "usage missing command '{}': {}",
            command,
            usage
        );
        assert!(
            readme.contains(&usage_line),
            "README missing command '{}'",
            command
        );
    }
    assert!(
        !readme.contains("CLI `play` and `render` load files literally"),
        "README should not imply native file commands skip helper compilation"
    );
    assert!(
        readme.contains("CLI `play` and `render` do not infer playback commands"),
        "README should explain the actual CLI distinction: playback inference"
    );
    assert!(
        readme.contains("Native file commands and typed native `live` / `repl` input still"),
        "README should state saved .gl files and native interactive input use the helper compiler"
    );
    assert!(
        readme.contains("Native `live` status")
            && readme.contains("terminal editor run messages")
            && readme.contains("scene=:name")
            && readme.contains("cycle=1/4"),
        "README should explain native scene-aware status feedback"
    );
}

#[test]
fn generated_gui_artifacts_are_ignored_and_documented() {
    let readme = std::fs::read_to_string("README.md").expect("read README.md");
    let gitignore = std::fs::read_to_string(".gitignore").expect("read .gitignore");
    let ignore = std::fs::read_to_string(".ignore").expect("read .ignore");
    for artifact in [
        "target/",
        "mescript.jar",
        "glitchlisp-native",
        "*.wav",
        "renders/",
        "mescript-swing-session.gl",
    ] {
        assert!(
            readme.contains(artifact),
            "README missing generated artifact '{}'",
            artifact
        );
        assert!(
            gitignore.lines().any(|line| line == artifact),
            ".gitignore missing generated artifact '{}'",
            artifact
        );
        assert!(
            ignore.lines().any(|line| line == artifact),
            ".ignore missing generated artifact '{}'",
            artifact
        );
    }
    assert!(
        readme.contains("renders/swing-compiled.gl"),
        "README should name the GUI compiled preview artifact"
    );
}

#[test]
fn readme_clojure_examples_compile_and_evaluate() {
    let readme = std::fs::read_to_string("README.md").expect("read README.md");
    let mut in_clojure = false;
    let mut current = Vec::new();
    let mut blocks = Vec::new();

    for line in readme.lines() {
        if line.trim() == "```clojure" {
            in_clojure = true;
            current.clear();
            continue;
        }
        if in_clojure && line.trim() == "```" {
            blocks.push(current.join("\n"));
            in_clojure = false;
            current.clear();
            continue;
        }
        if in_clojure {
            current.push(line.to_string());
        }
    }

    assert!(blocks.len() >= 4, "expected README clojure examples");
    for (index, block) in blocks.iter().enumerate() {
        let compiled = compile_source_for_runtime(block).unwrap_or_else(|err| {
            panic!(
                "README clojure block {} did not compile: {}\n{}",
                index + 1,
                err,
                block
            )
        });
        let mut runtime = Runtime::new();
        eval_program(&mut runtime, &compiled).unwrap_or_else(|err| {
            panic!(
                "README clojure block {} did not evaluate: {}\n{}",
                index + 1,
                err,
                block
            )
        });
        assert!(
            runtime.running,
            "README clojure block {} should start playback:\n{}",
            index + 1,
            block
        );
    }
}

#[test]
fn numeric_effect_parameter_errors_name_the_parameter() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(filter :cutoff :low)])
         (start!)",
    )
    .unwrap_err();
    assert!(err.contains(":cutoff"), "{}", err);

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate 1)
         (post-fx [(reverse :mix :wet)])
         (start!)",
    )
    .unwrap_err();
    assert!(err.contains(":mix"), "{}", err);
}

#[test]
fn numeric_track_parameter_errors_name_the_parameter() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :amp :loud)",
    )
    .unwrap_err();
    assert!(err.contains(":amp"), "{}", err);

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :dur (p [0.1 :short]))",
    )
    .unwrap_err();
    assert!(err.contains(":dur"), "{}", err);
}

#[test]
fn unknown_track_parameters_error_instead_of_being_ignored() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :ammp 0.8)",
    )
    .unwrap_err();

    assert!(err.contains("unknown track parameter ':ammp'"), "{}", err);
}

#[test]
fn track_parameters_require_values_with_parameter_name() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate)",
    )
    .unwrap_err();
    assert!(
        err.contains("track parameter ':gate' requires a value"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(sample :hit
            :sample-data [0 1 0]
            :amp)",
    )
    .unwrap_err();
    assert!(err.contains("sample :amp requires a value"), "{}", err);
}

#[test]
fn unknown_scene_options_error_instead_of_falling_through_to_body() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(scene :intro :repeet 2
           (d :lead :src :sine-synth :note c3 :gate 1))",
    )
    .unwrap_err();

    assert!(err.contains("unknown scene option ':repeet'"), "{}", err);
}

#[test]
fn playback_alias_errors_name_the_typed_form() {
    let mut runtime = Runtime::new();
    let err = eval_program(&mut runtime, "(cue)").unwrap_err();
    assert!(err.contains("cue requires a keyword argument"), "{}", err);

    let mut runtime = Runtime::new();
    let err = eval_program(&mut runtime, "(play-block)").unwrap_err();
    assert!(
        err.contains("play-block requires a keyword argument"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(scene :intro :steps 1)
         (play-scene :intro :extra)",
    )
    .unwrap_err();
    assert!(
        err.contains("play-scene expects exactly one scene keyword"),
        "{}",
        err
    );
}

fn read_edn_string_at(text: &str, quote: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    if bytes.get(quote) != Some(&b'"') {
        return None;
    }
    let mut out = String::new();
    let mut idx = quote + 1;
    while idx < bytes.len() {
        match bytes[idx] {
            b'\\' => {
                idx += 1;
                let escaped = match *bytes.get(idx)? {
                    b'n' => '\n',
                    b't' => '\t',
                    b'"' => '"',
                    b'\\' => '\\',
                    other => other as char,
                };
                out.push(escaped);
                idx += 1;
            }
            b'"' => return Some((out, idx + 1)),
            other => {
                out.push(other as char);
                idx += 1;
            }
        }
    }
    None
}

fn catalog_effect_forms() -> Vec<(String, String)> {
    let source = fs::read_to_string("data/effects.edn").expect("read data/effects.edn");
    let mut forms = Vec::new();
    let mut idx = 0;
    while let Some(label_rel) = source[idx..].find(":label") {
        let label_start = idx + label_rel;
        let Some(label_quote_rel) = source[label_start..].find('"') else {
            break;
        };
        let label_quote = label_start + label_quote_rel;
        let Some((label, label_end)) = read_edn_string_at(&source, label_quote) else {
            break;
        };
        let Some(form_rel) = source[label_end..].find(":form") else {
            break;
        };
        let form_start = label_end + form_rel;
        let Some(form_quote_rel) = source[form_start..].find('"') else {
            break;
        };
        let form_quote = form_start + form_quote_rel;
        let Some((form, form_end)) = read_edn_string_at(&source, form_quote) else {
            break;
        };
        forms.push((label, form));
        idx = form_end;
    }
    forms
}

fn offline_catalog_effect(label: &str) -> bool {
    matches!(
        label,
        "reverse"
            | "tape-stop"
            | "granular"
            | "granular-stretch"
            | "spectral-freeze"
            | "haas"
            | "stereo-widen"
            | "stereo-imager"
            | "width-enhance"
            | "freq-shift"
            | "autopan"
            | "ping-pong-delay"
    )
}

#[test]
fn oscillator_catalog_track_ids_do_not_collapse_distinct_sources() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/catalog.clj")
      (doseq [source glitchlisp.swing.catalog/oscillator-sources]
        (println (str source "\t" (glitchlisp.swing.catalog/track-id-for-source source))))
    "#;
    let output = Command::new("clojure")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run clojure catalog smoke");
    assert!(
        output.status.success(),
        "catalog smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let pairs = stdout
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .collect::<Vec<_>>();
    assert!(pairs.len() > 50, "expected oscillator catalog ids");

    let mut ids = std::collections::HashMap::new();
    for (source, id) in pairs {
        if let Some(previous) = ids.insert(id.to_string(), source.to_string()) {
            panic!(
                "oscillator sources '{}' and '{}' both insert track id :{}",
                previous, source, id
            );
        }
    }
}

#[test]
fn oscillator_catalog_lists_detune_for_detuned_synth_sources() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/catalog.clj")
      (doseq [source ["square-synth" "pulse" "morph" "wavetable" "fm-op" "sync" "pwm-sweep"]]
        (println (str source "\t" (clojure.string/join "," (glitchlisp.swing.catalog/oscillator-parameter-examples source)))))
    "#;
    let output = Command::new("clojure")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run clojure catalog parameter smoke");
    assert!(
        output.status.success(),
        "catalog parameter smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let Some((source, params)) = line.split_once('\t') else {
            continue;
        };
        assert!(
            params.split(',').any(|param| param == ":detune-cents"),
            "{} catalog examples did not include :detune-cents: {}",
            source,
            params
        );
    }
}

#[test]
fn sample_oscillator_insert_uses_sample_form() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/catalog.clj")
      (println (pr-str (glitchlisp.swing.catalog/oscillator-structure-snippet "sample" false)))
    "#;
    let output = Command::new("clojure")
        .arg("-e")
        .arg(script)
        .output()
        .expect("extract sample oscillator snippet");
    assert!(
        output.status.success(),
        "sample oscillator snippet extraction failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let snippet_line = stdout
        .lines()
        .rev()
        .find(|line| line.starts_with('"'))
        .unwrap_or_else(|| panic!("missing sample oscillator snippet in output: {}", stdout));
    let snippet = unescape_clojure_pr_str(snippet_line);
    assert!(
        snippet.starts_with("(sample :sample\n"),
        "sample oscillator insert should teach the sample form: {}",
        snippet
    );
    assert!(
        snippet.contains(":sample-data null")
            && snippet.contains(":gate null")
            && snippet.contains(":note null")
            && snippet.contains(":dur null")
            && snippet.contains(":amp null"),
        "sample oscillator insert should use blank null placeholders: {}",
        snippet
    );
    assert!(
        snippet.rfind(":gate null") > snippet.rfind(":amp null"),
        "sample oscillator insert should place :gate at the bottom: {}",
        snippet
    );
    assert!(
        !snippet.contains(":src :sample"),
        "sample oscillator insert should not teach the low-level :src :sample form: {}",
        snippet
    );
}

#[test]
fn swing_auto_indent_handles_enter_before_first_form() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (defn run-case [label text caret]
        (let [pane (javax.swing.JTextPane.)]
          (.setText pane text)
          (.setCaretPosition pane caret)
          (println label "helper" caret (pr-str (glitchlisp.swing.editor/smart-next-line-indent text caret)))
          (glitchlisp.swing.editor/install-auto-indent! pane)
          (.actionPerformed (.get (.getActionMap pane) "glitchlisp-auto-indent") nil)
          (println label "action" caret (pr-str (.getText pane)) (.getCaretPosition pane))))
      (doseq [[label text carets] [["empty" "\n(def a)" [0 1]]
                                   ["space" " \n(def a)" [0 1]]
                                   ["spaces" "  \n(def a)" [0 1 2]]]]
        (doseq [caret carets]
          (run-case label text caret)))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run swing auto-indent smoke");
    assert!(
        output.status.success(),
        "swing auto-indent smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout
            .lines()
            .any(|line| line == "empty action 0 \"\\n\\n(def a)\" 1"),
        "auto-indent did not insert a newline at buffer start: {}",
        stdout
    );
    assert!(
        stdout
            .lines()
            .any(|line| line == "empty action 1 \"\\n\\n(def a)\" 2"),
        "auto-indent did not insert a newline before the first form: {}",
        stdout
    );
    assert!(
        stdout
            .lines()
            .any(|line| line == "space action 1 \" \\n\\n(def a)\" 2"),
        "auto-indent did not handle Enter after first-line whitespace: {}",
        stdout
    );
    assert!(
        stdout
            .lines()
            .any(|line| line == "spaces action 2 \"  \\n\\n(def a)\" 3"),
        "auto-indent did not handle Enter at the end of a whitespace-only first line: {}",
        stdout
    );
}

#[test]
fn swing_editor_treats_sample_as_track_for_indent_helpers() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [sample-text "(sample :hit"
            mixed "(d :kick :src :click :gate 1)\n    (sample :hit :sample-data [1 0 -1] :gate 1)"
            sample-start (.indexOf mixed "(sample")
            sample-caret (+ sample-start 10)
            align (glitchlisp.swing.editor/align-current-track-to-previous mixed sample-caret)]
        (println (= "   " (glitchlisp.swing.editor/smart-next-line-indent sample-text (count sample-text))))
        (println (some? align))
        (println (clojure.string/starts-with? (:text align) "(sample :hit")))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run sample editor helper smoke");
    assert!(
        output.status.success(),
        "sample editor helper smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true"],
        "unexpected sample editor helper results: {}",
        stdout
    );
}

#[test]
fn live_highlight_ignores_def_names_inside_scene_comments_and_strings() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [text (str "(def click\n"
                      "  (d :click :src :click :gate (p [1 0])))\n"
                      "(def unused\n"
                      "  (d :unused :src :click :gate (p [0 1])))\n"
                      "(scene :intro\n"
                      "  click\n"
                      "  ; unused\n"
                      "  \"unused\"\n"
                      ")\n")
            ranges (glitchlisp.swing.editor/gate-pattern-vector-ranges text)
            active (filter #(glitchlisp.swing.editor/active-gate-entry? text "intro" %) ranges)]
        (println (count ranges))
        (println (count active)))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live highlight scene membership smoke");
    assert!(
        output.status.success(),
        "live highlight scene membership smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.lines().any(|line| line == "2"),
        "expected two gate ranges in smoke source: {}",
        stdout
    );
    assert!(
        stdout.lines().any(|line| line == "1"),
        "expected only the real scene member to be active: {}",
        stdout
    );
}

#[test]
fn live_scene_membership_context_precomputes_symbol_set() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [text (str "(def click\n"
                      "  (d :click :src :click :gate (p [1 0])))\n"
                      "(scene :intro\n"
                      "  click\n"
                      "  ; unused\n"
                      "  \"fake\"\n"
                      ")\n")
            scene-range (glitchlisp.swing.editor/scene-form-range-by-id text "intro")
            context (glitchlisp.swing.editor/scene-membership-context text scene-range)]
        (println (contains? (:symbols context) "click"))
        (println (contains? (:symbols context) "unused"))
        (println (contains? (:symbols context) "fake")))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live scene membership context smoke");
    assert!(
        output.status.success(),
        "live scene membership context smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "false", "false"],
        "scene membership context should precompute visible symbols only: {}",
        stdout
    );
}

#[test]
fn swing_scene_range_helpers_ignore_comments_and_strings() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [text (str "; (scene :intro\n"
                      ";   fake)\n"
                      "\"(scene :intro fake)\"\n"
                      "(def click\n"
                      "  (d :click :src :click :gate (p [1 0])))\n"
                      "(scene :intro\n"
                      "  click\n"
                      ")\n")
            real-start (.indexOf text "(scene :intro\n  click")
            named (glitchlisp.swing.editor/named-scene-range text "intro")
            header (glitchlisp.swing.editor/scene-header-range-by-id text "intro")
            scene-ranges (glitchlisp.swing.editor/form-ranges text "scene")
            gate-ranges (glitchlisp.swing.editor/gate-pattern-vector-ranges text)
            active (filter #(glitchlisp.swing.editor/active-gate-entry? text "intro" %) gate-ranges)]
        (println real-start)
        (println (first named))
        (println (first header))
        (println (count scene-ranges))
        (println (count active)))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run scene range helper smoke");
    assert!(
        output.status.success(),
        "scene range helper smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let values = stdout
        .lines()
        .filter_map(|line| line.parse::<usize>().ok())
        .collect::<Vec<_>>();
    assert_eq!(
        values.len(),
        5,
        "unexpected scene range helper output: {}",
        stdout
    );
    assert_eq!(
        values[0], values[1],
        "named scene matched fake text: {}",
        stdout
    );
    assert_eq!(
        values[0], values[2],
        "header scene matched fake text: {}",
        stdout
    );
    assert_eq!(
        values[3], 1,
        "form-ranges included fake scene text: {}",
        stdout
    );
    assert_eq!(
        values[4], 1,
        "active scene membership did not use the real scene only: {}",
        stdout
    );
}

#[test]
fn live_scene_highlight_uses_full_multiline_scene_range() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [text (str "(def click\n"
                      "  (d :click :src :click :gate (p [1 0])))\n\n"
                      "(scene :intro :loop true\n"
                      "  click)\n\n"
                      "(play-scene :intro)\n")
            pane (javax.swing.JTextPane.)
            scene-start (.indexOf text "(scene :intro")
            scene-end (inc (.indexOf text ")\n\n(play-scene"))]
        (.setText pane text)
        (glitchlisp.swing.editor/highlight-live-step! pane 0 "intro")
        (println scene-start)
        (println scene-end)
        (println (pr-str (.getClientProperty pane glitchlisp.swing.editor/live-scene-highlight-key))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live scene full-range highlight smoke");
    assert!(
        output.status.success(),
        "live scene full-range highlight smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let values = stdout
        .lines()
        .filter_map(|line| line.parse::<usize>().ok())
        .collect::<Vec<_>>();
    assert_eq!(
        values.len(),
        2,
        "expected scene start/end output: {}",
        stdout
    );
    let expected = format!("[{} {}]", values[0], values[1]);
    assert!(
        stdout.lines().any(|line| line == expected),
        "live scene highlight should span the full scene form, expected {}: {}",
        expected,
        stdout
    );
}

#[test]
fn live_scene_highlight_caches_line_segments_until_document_changes() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [text (str "(def click\n"
                      "  (d :click :src :click :gate (p [1 0])))\n\n"
                      "(scene :intro :loop true\n"
                      "  click)\n\n"
                      "(play-scene :intro)\n")
            pane (javax.swing.JTextPane.)]
        (.setText pane text)
        (glitchlisp.swing.editor/install-live-gate-range-cache! pane)
        (glitchlisp.swing.editor/highlight-live-step! pane 0 "intro")
        (let [scene-range (.getClientProperty pane glitchlisp.swing.editor/live-scene-highlight-key)
              segments (.getClientProperty pane glitchlisp.swing.editor/live-scene-highlight-segments-key)
              cached-segments (get (.getClientProperty pane glitchlisp.swing.editor/live-scene-segments-cache-key)
                                   "intro")]
          (println (some? scene-range))
          (println (= segments
                      (glitchlisp.swing.editor/live-scene-range-segments text scene-range)))
          (println (identical? segments cached-segments))
          (println (> (count segments) 1))
          (glitchlisp.swing.editor/highlight-live-step! pane 1 "intro")
          (println (identical? cached-segments
                               (get (.getClientProperty pane glitchlisp.swing.editor/live-scene-segments-cache-key)
                                    "intro"))))
        (.setText pane text)
        (println (nil? (.getClientProperty pane glitchlisp.swing.editor/live-scene-highlight-segments-key)))
        (println (nil? (.getClientProperty pane glitchlisp.swing.editor/live-scene-segments-cache-key))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live scene segment cache smoke");
    assert!(
        output.status.success(),
        "live scene segment cache smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true", "true", "true", "true", "true"],
        "live scene segments should be cached until document changes: {}",
        stdout
    );
}

#[test]
fn live_scene_segment_bounds_include_last_character() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [text "(scene :intro :loop true\n  click)\n"
            pane (javax.swing.JTextPane.)]
        (.setFont pane (java.awt.Font. "Monospaced" java.awt.Font/PLAIN 12))
        (.setText pane text)
        (.setSize pane 500 200)
        (let [line-end (.indexOf text "\n")
              form-end (.indexOf text ")")
              metrics (.getFontMetrics pane (.getFont pane))
              scene-bounds (glitchlisp.swing.editor/live-scene-segment-bounds pane 0 line-end)
              body-bounds (glitchlisp.swing.editor/live-scene-segment-bounds pane (inc line-end) (inc form-end))
              final-header-rect (.modelToView pane (dec line-end))
              final-body-rect (.modelToView pane form-end)
              header-right (+ (.x final-header-rect)
                              (.charWidth metrics (.charAt text (dec line-end))))
              body-right (+ (.x final-body-rect)
                            (.charWidth metrics (.charAt text form-end)))]
          (println (some? scene-bounds))
          (println (some? body-bounds))
          (println (>= (+ (.x scene-bounds) (.width scene-bounds)) header-right))
          (println (>= (+ (.x body-bounds) (.width body-bounds)) body-right))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live scene segment bounds smoke");
    assert!(
        output.status.success(),
        "live scene segment bounds smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let values = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        values,
        vec!["true", "true", "true", "true"],
        "scene segment bounds should include the final character: {}",
        stdout
    );
}

#[test]
fn live_overlay_clip_helper_skips_non_intersecting_regions() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [image (java.awt.image.BufferedImage. 32 32 java.awt.image.BufferedImage/TYPE_INT_ARGB)
            graphics (.getGraphics image)]
        (.setClip graphics (java.awt.Rectangle. 0 0 10 10))
        (println (glitchlisp.swing.editor/rect-intersects-clip?
                   graphics
                   (java.awt.Rectangle. 5 5 4 4)))
        (println (glitchlisp.swing.editor/rect-intersects-clip?
                   graphics
                   (java.awt.Rectangle. 20 20 4 4)))
        (.dispose graphics))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live overlay clip helper smoke");
    assert!(
        output.status.success(),
        "live overlay clip helper smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "false"],
        "clip helper should reject non-intersecting repaint regions: {}",
        stdout
    );
}

#[test]
fn swing_gate_highlight_ignores_commented_and_string_gates() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [text (str "; :gate (p [0 0 0 0])\n"
                      "\":gate (p [1 1 1 1])\"\n"
                      "(d :click :src :click :gate (p [1 0]))\n")
            real-gate (.indexOf text ":gate (p [1 0])")
            ranges (glitchlisp.swing.editor/gate-pattern-vector-ranges text)
            entry (first ranges)]
        (println real-gate)
        (println (count ranges))
        (println (:gate-idx entry))
        (println (count (:cells entry))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run gate highlight comment/string smoke");
    assert!(
        output.status.success(),
        "gate highlight comment/string smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let values = stdout
        .lines()
        .filter_map(|line| line.parse::<usize>().ok())
        .collect::<Vec<_>>();
    assert_eq!(
        values.len(),
        4,
        "unexpected gate highlight smoke output: {}",
        stdout
    );
    assert_eq!(values[1], 1, "scanner included fake gates: {}", stdout);
    assert_eq!(
        values[0], values[2],
        "scanner matched a fake gate first: {}",
        stdout
    );
    assert_eq!(values[3], 2, "real gate cells were not parsed: {}", stdout);
}

#[test]
fn live_highlight_clear_removes_scene_only_overlay() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [pane (javax.swing.JTextPane.)]
        (.putClientProperty pane glitchlisp.swing.editor/live-scene-highlight-key [0 5])
        (glitchlisp.swing.editor/clear-live-step-highlight! pane)
        (println (nil? (.getClientProperty pane glitchlisp.swing.editor/live-scene-highlight-key)))
        (println (nil? (.getClientProperty pane glitchlisp.swing.editor/live-step-highlight-key))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live scene-only clear smoke");
    assert!(
        output.status.success(),
        "live scene-only clear smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true"],
        "clear-live-step-highlight should clear scene-only overlays: {}",
        stdout
    );
}

#[test]
fn source_error_reporting_updates_status_even_without_offset() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [pane (javax.swing.JTextPane.)
            status (javax.swing.JLabel. "stale")]
        (.setText pane "a\nb\n")
        (glitchlisp.swing.editor/report-source-error! pane status (ex-info "plain failure" {}))
        (println (.getText status))
        (println (nil? (.getClientProperty pane glitchlisp.swing.editor/error-highlight-key)))
        (glitchlisp.swing.editor/report-source-error! pane status (ex-info "bad thing at line 2, col 1" {}))
        (println (.getText status))
        (println (some? (.getClientProperty pane glitchlisp.swing.editor/error-highlight-key))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run source error reporting smoke");
    assert!(
        output.status.success(),
        "source error reporting smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| {
            *line == "true"
                || *line == "false"
                || line.contains("failure")
                || line.contains("line 2")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["plain failure", "true", "line 2, col 1: bad thing", "true",],
        "unexpected source error reporting output: {}",
        stdout
    );
}

#[test]
fn source_error_reporting_resolves_nested_note_vector_runtime_errors() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [source "(d :hat\n   :src :hat-909\n   :note c6\n   :gate (euclid 5 16))\n\n(d :pad\n   :src :pad-wash\n   :note (p (then\n             (times 12 [[c3 eb3 g3 bb3]])\n             (times 4 [[c3 e4]])))\n   :gate (p [1 0 0 0 1 0 0 0]))\n\n(d :drone-dark\n   :src :drone-dark\n   :note (s [c2 eb2 bb2 [[e3 f3]]])\n   :gate (p [1 0 0 0 0 0 0 0]))\n(start!)"
            pane (javax.swing.JTextPane.)
            status (javax.swing.JLabel. "stale")
            ex (glitchlisp.swing.editor/source-error-exception source "expected number or note")
            data (ex-data ex)]
        (.setText pane source)
        (glitchlisp.swing.editor/report-source-error! pane status ex)
        (println (.getText status))
        (println (subs source (:offset data) (:end-offset data)))
        (println (some? (.getClientProperty pane glitchlisp.swing.editor/error-highlight-key))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run nested note vector source error smoke");
    assert!(
        output.status.success(),
        "nested note vector source error smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("expected number or note in :note pattern"),
        "status should include resolved line and better message: {}",
        stdout
    );
    assert!(
        stdout.contains("[e3 f3]\ntrue"),
        "source error should highlight the offending nested vector: {}",
        stdout
    );
}

#[test]
fn live_err_lines_highlight_resolved_source_error() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/live.clj")
      (let [source "(d :drone-dark\n   :src :drone-dark\n   :note (s [c2 eb2 bb2 [[e3 f3]]])\n   :gate (p [1 0 0 0 0 0 0 0]))\n(start!)"
            pane (javax.swing.JTextPane.)
            status (javax.swing.JLabel. "stale")]
        (.setText pane source)
        (glitchlisp.swing.live/handle-live-line! pane status "ERR expected number or note")
        (javax.swing.SwingUtilities/invokeAndWait (fn [] nil))
        (println (.getText status))
        (println (some? (.getClientProperty pane glitchlisp.swing.editor/error-highlight-key))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live ERR source highlight smoke");
    assert!(
        output.status.success(),
        "live ERR source highlight smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("live error: line 3, col 26: expected number or note in :note pattern"),
        "live ERR should include resolved line and clearer message: {}",
        stdout
    );
    assert!(
        stdout.contains("\ntrue"),
        "live ERR should install an error highlight: {}",
        stdout
    );
}

#[test]
fn render_session_file_does_not_become_current_save_target() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (reset! glitchlisp.swing.shared/state {})
      (let [session (glitchlisp.swing.render/current-file-or-session!)]
        (println (.getPath session))
        (println (nil? (:file @glitchlisp.swing.shared/state))))
      (let [real-file (java.io.File. "song.gl")]
        (swap! glitchlisp.swing.shared/state assoc :file real-file)
        (println (= real-file (glitchlisp.swing.render/current-file-or-session!)))
        (println (= real-file (:file @glitchlisp.swing.shared/state))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run render session file smoke");
    assert!(
        output.status.success(),
        "render session file smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || line.contains("mescript-swing-session.gl"))
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["mescript-swing-session.gl", "true", "true", "true"],
        "unexpected render session file behavior: {}",
        stdout
    );
}

#[test]
fn live_status_lines_show_active_scene_name() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (doseq [line (glitchlisp.swing.shared/live-status-lines
                     {:live-ready true
                      :live-tracks 2
                      :live-scenes 3
                      :live-highlight-scene "help"
                      :live-cycle "2/4"
                      :live-highlight-step 17})]
        (println line))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live status active scene smoke");
    assert!(
        output.status.success(),
        "live status active scene smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.lines().any(|line| line == "Scene: help"),
        "live status should show the active scene name: {}",
        stdout
    );
    assert!(
        stdout.lines().any(|line| line == "Cycle: 2/4"),
        "live status should show the active scene cycle: {}",
        stdout
    );
    assert!(
        stdout.lines().any(|line| line == "Step: 17"),
        "live status should keep showing the active step: {}",
        stdout
    );
}

#[test]
fn live_status_lines_hide_stale_scene_when_stopped() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (doseq [line (glitchlisp.swing.shared/live-status-lines
                     {:live-ready false
                      :live-process nil
                      :live-awaiting-update false
                      :live-highlight-scene "old"
                      :live-cycle "9/16"
                      :live-highlight-step 99})]
        (println line))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live status stale scene smoke");
    assert!(
        output.status.success(),
        "live status stale scene smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.lines().any(|line| line == "State: stopped"),
        "live status should report stopped state: {}",
        stdout
    );
    assert!(
        stdout.lines().any(|line| line == "Scene: -"),
        "stopped live status should not show a stale scene name: {}",
        stdout
    );
    assert!(
        stdout.lines().any(|line| line == "Cycle: -"),
        "stopped live status should not show a stale cycle: {}",
        stdout
    );
    assert!(
        stdout.lines().any(|line| line == "Step: -"),
        "stopped live status should not show a stale step: {}",
        stdout
    );
}

#[test]
fn swing_live_update_reports_unclosed_forms_before_playback_inference() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (load-file "src/glitchlisp/swing/live.clj")
      (let [pane (javax.swing.JTextPane.)
            status (javax.swing.JLabel.)]
        (.setText pane "(def click\n  (d :click\n     :src :hat-808\n     :amp (p [0.6 0.2 0.4])\n\n(def bass\n  (d :morph :src :morph :gate 1))\n(scene :intro :loop true click bass)\n(play-scene :intro)")
        (glitchlisp.swing.live/live-update!
          nil pane status nil
          (fn [_] "unused-renderer")
          identity
          glitchlisp.swing.render/require-playback-form!
          identity)
        (Thread/sleep 400)
        (println (.getText status))
        (shutdown-agents))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live delimiter-before-playback smoke");
    assert!(
        output.status.success(),
        "live delimiter-before-playback smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("expected ) to close (") && !stdout.contains("add an explicit playback"),
        "live update should report delimiter errors before playback inference: {}",
        stdout
    );
}

#[test]
fn swing_live_ok_lines_parse_scene_cycle() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/live.clj")
      (let [pane (javax.swing.JTextPane.)
            status (javax.swing.JLabel.)]
        (glitchlisp.swing.live/handle-live-line!
          pane
          status
          "OK bpm=100 running=true tracks=2 scenes=1 scene=:intro cycle=1/4")
        (println (:live-tracks @glitchlisp.swing.shared/state))
        (println (:live-scenes @glitchlisp.swing.shared/state))
        (println (:live-highlight-scene @glitchlisp.swing.shared/state))
        (println (:live-cycle @glitchlisp.swing.shared/state))
        (println (.getText status)))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live OK scene cycle parser smoke");
    assert!(
        output.status.success(),
        "live OK scene cycle parser smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.lines().any(|line| line == "2")
            && stdout.lines().any(|line| line == "1")
            && stdout.lines().any(|line| line == "intro")
            && stdout.lines().any(|line| line == "1/4"),
        "live OK parser should store counts, scene, and cycle: {}",
        stdout
    );
    assert!(
        stdout
            .lines()
            .any(|line| line.contains("live bpm=100 running=true tracks=2 scenes=1")),
        "live OK parser should keep status text useful: {}",
        stdout
    );
}

#[test]
fn swing_live_step_lines_parse_without_regex_path() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/live.clj")
      (doseq [line ["STEP 32"
                    "STEP 32 :drop"
                    "STEP   7   :intro"
                    "STEP "
                    "STEP x"
                    "STEP 1 :"
                    "STEP 1 :drop extra"]]
        (println (pr-str (glitchlisp.swing.live/parse-step-line line))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live STEP parser smoke");
    assert!(
        output.status.success(),
        "live STEP parser smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed = stdout
        .lines()
        .filter(|line| *line == "nil" || line.chars().nth(1).is_some_and(|ch| ch.is_ascii_digit()))
        .collect::<Vec<_>>();
    assert_eq!(
        parsed,
        vec![
            "[32 nil]",
            "[32 \"drop\"]",
            "[7 \"intro\"]",
            "nil",
            "nil",
            "nil",
            "nil",
        ],
        "unexpected live STEP parser output: {}",
        stdout
    );
}

#[test]
fn swing_live_step_scene_change_clears_stale_cycle() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/live.clj")
      (let [pane (javax.swing.JTextPane.)
            status (javax.swing.JLabel.)]
        (.setText pane "(scene :drop :loop true\n  (d :kick :src :click :gate (p [1 0])))\n(play-scene :drop)")
        (swap! glitchlisp.swing.shared/state assoc
               :live-ready true
               :live-highlight-scene "intro"
               :live-cycle "1/4")
        (glitchlisp.swing.live/handle-live-line! pane status "STEP 32 :drop")
        (javax.swing.SwingUtilities/invokeAndWait (fn [] nil))
        (println (:live-highlight-scene @glitchlisp.swing.shared/state))
        (println (nil? (:live-cycle @glitchlisp.swing.shared/state)))
        (println (:live-highlight-scheduled @glitchlisp.swing.shared/state))
        (println (some? (.getClientProperty pane glitchlisp.swing.editor/live-step-highlight-key)))
        (println (some? (.getClientProperty pane glitchlisp.swing.editor/live-scene-highlight-key))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live step scene change cycle smoke");
    assert!(
        output.status.success(),
        "live step scene change cycle smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.lines().any(|line| line == "drop")
            && stdout.lines().filter(|line| *line == "true").count() == 3
            && stdout.lines().any(|line| line == "false"),
        "STEP scene change should clear stale cycle: {}",
        stdout
    );
}

#[test]
fn swing_live_reader_step_state_matches_handler() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/live.clj")
      (swap! glitchlisp.swing.shared/state assoc
             :live-ready true
             :live-highlight-scene "intro"
             :live-cycle "1/4")
      (glitchlisp.swing.live/update-live-step-state! 48 "drop")
      (println (:live-highlight-step @glitchlisp.swing.shared/state))
      (println (:live-highlight-scene @glitchlisp.swing.shared/state))
      (println (nil? (:live-cycle @glitchlisp.swing.shared/state)))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live reader step state smoke");
    assert!(
        output.status.success(),
        "live reader step state smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.lines().any(|line| line == "48")
            && stdout.lines().any(|line| line == "drop")
            && stdout.lines().any(|line| line == "true"),
        "reader STEP state update should match handler behavior: {}",
        stdout
    );
}

#[test]
fn remove_playback_highlighting_preference_disables_live_step_overlay() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/live.clj")
      (let [pane (javax.swing.JTextPane.)
            status (javax.swing.JLabel. "stale")
            source "(d :kick :src :click :gate (p [1 0]))\n(start!)"]
        (.setText pane source)
        (swap! glitchlisp.swing.shared/state assoc :remove-playback-highlighting true)
        (glitchlisp.swing.live/handle-live-line! pane status "STEP 0")
        (javax.swing.SwingUtilities/invokeAndWait (fn [] nil))
        (println (:live-highlight-step @glitchlisp.swing.shared/state))
        (println (nil? (.getClientProperty pane glitchlisp.swing.editor/live-step-highlight-key)))
        (swap! glitchlisp.swing.shared/state assoc :remove-playback-highlighting false)
        (glitchlisp.swing.live/handle-live-line! pane status "STEP 0")
        (javax.swing.SwingUtilities/invokeAndWait (fn [] nil))
        (println (some? (.getClientProperty pane glitchlisp.swing.editor/live-step-highlight-key))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run remove playback highlighting preference smoke");
    assert!(
        output.status.success(),
        "remove playback highlighting preference smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "0" || *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["0", "true", "true"],
        "checked preference should keep step state but remove editor overlay: {}",
        stdout
    );
}

#[test]
fn close_live_process_clears_cycle_state() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/live.clj")
      (swap! glitchlisp.swing.shared/state assoc
             :live-ready true
             :live-highlight-step 12
             :live-highlight-scene "intro"
             :live-cycle "3/4")
      (glitchlisp.swing.live/close-live-process!)
      (println (nil? (:live-highlight-step @glitchlisp.swing.shared/state)))
      (println (nil? (:live-highlight-scene @glitchlisp.swing.shared/state)))
      (println (nil? (:live-cycle @glitchlisp.swing.shared/state))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run close live process cycle smoke");
    assert!(
        output.status.success(),
        "close live process cycle smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true"],
        "close-live-process should clear scene, step, and cycle state: {}",
        stdout
    );
}

#[test]
fn ended_live_process_cleanup_clears_editor_overlay() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/live.clj")
      (let [pane (javax.swing.JTextPane.)
            process (.start (ProcessBuilder. (into-array String ["true"])))]
        (.waitFor process)
        (.putClientProperty pane glitchlisp.swing.editor/live-step-highlight-key [[0 1]])
        (.putClientProperty pane glitchlisp.swing.editor/live-scene-highlight-key [0 5])
        (swap! glitchlisp.swing.shared/state assoc
               :live-process process
               :live-ready true
               :live-highlight-step 9
               :live-highlight-scene "old"
               :live-cycle "9/16"
               :live-highlight-scheduled true)
        (glitchlisp.swing.live/clear-ended-live-process! pane process)
        (javax.swing.SwingUtilities/invokeAndWait (fn [] nil))
        (println (nil? (:live-process @glitchlisp.swing.shared/state)))
        (println (nil? (:live-highlight-step @glitchlisp.swing.shared/state)))
        (println (nil? (:live-highlight-scene @glitchlisp.swing.shared/state)))
        (println (nil? (:live-cycle @glitchlisp.swing.shared/state)))
        (println (nil? (.getClientProperty pane glitchlisp.swing.editor/live-step-highlight-key)))
        (println (nil? (.getClientProperty pane glitchlisp.swing.editor/live-scene-highlight-key))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run ended live process cleanup smoke");
    assert!(
        output.status.success(),
        "ended live process cleanup smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true", "true", "true", "true"],
        "ended live process cleanup should clear state and editor overlay: {}",
        stdout
    );
}

#[test]
fn live_step_highlight_queue_paints_on_next_edt_pass() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [pane (javax.swing.JTextPane.)]
        (.setText pane "(scene :intro :loop true\n  (d :kick :src :click :gate (p [1 0])))\n(play-scene :intro)")
        (glitchlisp.swing.editor/queue-live-step-highlight! pane 0 "intro")
        (javax.swing.SwingUtilities/invokeAndWait (fn [] nil))
        (println glitchlisp.swing.editor/live-highlight-delay-ms)
        (println (:live-highlight-scheduled @glitchlisp.swing.shared/state))
        (println (some? (.getClientProperty pane glitchlisp.swing.editor/live-step-highlight-key)))
        (println (some? (.getClientProperty pane glitchlisp.swing.editor/live-scene-highlight-key))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live step immediate queue smoke");
    assert!(
        output.status.success(),
        "live step immediate queue smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "0" || *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["0", "false", "true", "true"],
        "live cursor queue should paint on the next EDT pass without timer delay: {}",
        stdout
    );
}

#[test]
fn live_cursor_profile_records_queue_and_highlight_timings_when_enabled() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (System/setProperty glitchlisp.swing.editor/live-cursor-profile-property "true")
      (reset! glitchlisp.swing.editor/live-cursor-profile-state {:samples 0 :stats {}})
      (let [pane (javax.swing.JTextPane.)]
        (.setText pane "(d :kick :src :click :gate (p [1 0]))")
        (swap! glitchlisp.swing.shared/state assoc
               :live-highlight-step 0
               :live-highlight-scene nil
               :live-highlight-scheduled false)
        (glitchlisp.swing.editor/queue-current-live-step-highlight! pane (System/nanoTime))
        (javax.swing.SwingUtilities/invokeAndWait (fn [] nil))
        (let [stats (:stats @glitchlisp.swing.editor/live-cursor-profile-state)]
          (println (:samples @glitchlisp.swing.editor/live-cursor-profile-state))
          (println (pos? (get-in stats [:receive-to-edt :n] 0)))
          (println (pos? (get-in stats [:queue :n] 0)))
          (println (pos? (get-in stats [:highlight :n] 0)))))
      (System/clearProperty glitchlisp.swing.editor/live-cursor-profile-property)
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live cursor profile smoke");
    assert!(
        output.status.success(),
        "live cursor profile smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "1" || *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["1", "true", "true", "true"],
        "live cursor profile should record queue and highlight timing: {}",
        stdout
    );
}

#[test]
fn live_step_highlight_reuses_cached_text_between_document_edits() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [pane (javax.swing.JTextPane.)
            source "(d :kick :src :click :gate (p [1 0]))\n(start!)"]
        (.setText pane source)
        (glitchlisp.swing.editor/install-live-gate-range-cache! pane)
        (glitchlisp.swing.editor/highlight-live-step! pane 0 nil)
        (.setText pane "(d :kick :src :click :gate (p [0 1]))\n(start!)")
        (println (nil? (.getClientProperty pane glitchlisp.swing.editor/live-gate-ranges-key)))
        (glitchlisp.swing.editor/highlight-live-step! pane 0 nil)
        (let [cached (.getClientProperty pane glitchlisp.swing.editor/live-gate-ranges-text-key)]
          (println (= cached (.getText pane)))
          (println (some? (.getClientProperty pane glitchlisp.swing.editor/live-gate-ranges-key))))
        (glitchlisp.swing.editor/highlight-live-step! pane 1 nil)
        (println (= (.getClientProperty pane glitchlisp.swing.editor/live-gate-ranges-text-key)
                    (.getText pane))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live highlight cached text smoke");
    assert!(
        output.status.success(),
        "live highlight cached text smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true", "true"],
        "live highlight should cache text between edits and clear on document changes: {}",
        stdout
    );
}

#[test]
fn live_step_highlight_marks_active_note_times_stage() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [source "(d :pad\n   :src :pad-wash\n   :note (p (then\n             (times 3 [[c3 eb3 g3 bb3]])\n             (times 4 [[c3 e4]])\n             (times 5 [[c4 e3 f5]])))\n   :gate (p [1 0 0 0 1 0 0 0])\n   :dur 1.5\n   :amp 0.16)\n(start!)"
            pane (javax.swing.JTextPane.)
            highlighted-forms (fn []
                                (->> (.getClientProperty pane glitchlisp.swing.editor/live-step-highlight-key)
                                     (map (fn [[start end]] (subs source start end)))
                                     (filter #(clojure.string/starts-with? % "(times"))
                                     vec))]
        (.setText pane source)
        (glitchlisp.swing.editor/install-live-gate-range-cache! pane)
        (glitchlisp.swing.editor/highlight-live-step! pane 0 nil)
        (println (some #(clojure.string/starts-with? % "(times 3") (highlighted-forms)))
        (glitchlisp.swing.editor/highlight-live-step! pane 3 nil)
        (println (some #(clojure.string/starts-with? % "(times 4") (highlighted-forms)))
        (glitchlisp.swing.editor/highlight-live-step! pane 7 nil)
        (println (some #(clojure.string/starts-with? % "(times 5") (highlighted-forms))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live note times highlight smoke");
    assert!(
        output.status.success(),
        "live note times highlight smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false" || *line == "nil")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true"],
        "live highlight should mark active note times stage: {}",
        stdout
    );
}

#[test]
fn live_step_highlight_skips_repaint_when_visible_range_is_unchanged() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [calls (atom 0)
            pane (proxy [javax.swing.JTextPane] []
                   (repaint
                     ([x y w h]
                      (swap! calls inc)
                      (proxy-super repaint x y w h))
                     ([]
                      (proxy-super repaint))))]
        (.setText pane "(d :kick :src :click :gate (p [1]))")
        (.setSize pane 500 200)
        (reset! calls 0)
        (glitchlisp.swing.editor/highlight-live-step! pane 0 nil)
        (println @calls)
        (reset! calls 0)
        (glitchlisp.swing.editor/highlight-live-step! pane 1 nil)
        (println @calls)
        (println (pr-str (.getClientProperty pane glitchlisp.swing.editor/live-step-highlight-key))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run unchanged live highlight repaint smoke");
    assert!(
        output.status.success(),
        "unchanged live highlight repaint smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let counts = stdout
        .lines()
        .filter_map(|line| line.parse::<usize>().ok())
        .collect::<Vec<_>>();
    assert_eq!(
        counts,
        vec![1, 0],
        "unchanged visible live cursor range should not repaint again: {}",
        stdout
    );
    assert!(
        stdout.lines().any(|line| line.starts_with("[[")),
        "live cursor range should remain stored after skipped repaint: {}",
        stdout
    );
}

#[test]
fn live_step_highlight_paints_dirty_region_immediately() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [repaints (atom 0)
            paints (atom 0)
            pane (proxy [javax.swing.JTextPane] []
                   (repaint
                     ([x y w h]
                      (swap! repaints inc)
                      (proxy-super repaint x y w h))
                     ([]
                      (proxy-super repaint)))
                   (paintImmediately
                     ([x y w h]
                      (swap! paints inc)
                      (proxy-super paintImmediately x y w h))))]
        (.setText pane "(d :kick :src :click :gate (p [1 0]))")
        (.setSize pane 500 200)
        (glitchlisp.swing.editor/highlight-live-step! pane 0 nil)
        (println (pos? @repaints))
        (println (pos? @paints)))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run immediate live highlight paint smoke");
    assert!(
        output.status.success(),
        "immediate live highlight paint smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true"],
        "live highlight should flush dirty range paint immediately: {}",
        stdout
    );
}

#[test]
fn live_repaint_pump_keeps_active_overlay_repainting() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [repaints (atom 0)
            pane (proxy [javax.swing.JTextPane] []
                   (repaint
                     ([x y w h]
                      (swap! repaints inc)
                      (proxy-super repaint x y w h))
                     ([]
                      (swap! repaints inc)
                      (proxy-super repaint))))]
        (.setText pane "(d :kick :src :click :gate (p [1 0]))")
        (.setSize pane 500 200)
        (glitchlisp.swing.editor/install-live-repaint-pump! pane)
        (println (.isRunning (.getClientProperty pane glitchlisp.swing.editor/live-repaint-pump-key)))
        (glitchlisp.swing.editor/highlight-live-step! pane 0 nil)
        (reset! repaints 0)
        (Thread/sleep 60)
        (println (pos? @repaints))
        (.stop (.getClientProperty pane glitchlisp.swing.editor/live-repaint-pump-key)))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live repaint pump smoke");
    assert!(
        output.status.success(),
        "live repaint pump smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true"],
        "live repaint pump should repaint active overlay without mouse events: {}",
        stdout
    );
}

#[test]
fn syntax_refresh_forces_editor_repaint() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [repaints (atom 0)
            revalidates (atom 0)
            pane (proxy [javax.swing.JTextPane] []
                   (repaint
                     ([]
                      (swap! repaints inc)
                      (proxy-super repaint))
                     ([x y w h]
                      (proxy-super repaint x y w h)))
                   (revalidate []
                     (swap! revalidates inc)
                     (proxy-super revalidate)))]
        (.setText pane "(d :lead :src :sine-synth :note c3 :gate 1)")
        (reset! repaints 0)
        (reset! revalidates 0)
        (glitchlisp.swing.editor/refresh-syntax-colors! pane)
        (println (pos? @repaints))
        (println (pos? @revalidates)))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run syntax refresh repaint smoke");
    assert!(
        output.status.success(),
        "syntax refresh repaint smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true"],
        "syntax refresh should force visible editor repaint: {}",
        stdout
    );
}

#[test]
fn live_step_highlight_caches_step_rects_until_range_or_document_changes() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [pane (javax.swing.JTextPane.)
            source "(d :kick :src :click :gate (p [1]))"]
        (.setText pane source)
        (.setSize pane 500 200)
        (glitchlisp.swing.editor/install-live-gate-range-cache! pane)
        (glitchlisp.swing.editor/highlight-live-step! pane 0 nil)
        (let [first-rects (.getClientProperty pane glitchlisp.swing.editor/live-step-highlight-rects-key)]
          (println (some? first-rects))
          (println (= (count first-rects)
                      (count (.getClientProperty pane glitchlisp.swing.editor/live-step-highlight-key))))
          (glitchlisp.swing.editor/highlight-live-step! pane 1 nil)
          (println (identical? first-rects
                               (.getClientProperty pane glitchlisp.swing.editor/live-step-highlight-rects-key))))
        (.setText pane source)
        (println (nil? (.getClientProperty pane glitchlisp.swing.editor/live-step-highlight-rects-key))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live step rect cache smoke");
    assert!(
        output.status.success(),
        "live step rect cache smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true", "true"],
        "live step rects should be cached until range or document changes: {}",
        stdout
    );
}

#[test]
fn live_step_highlight_ignores_caret_focus_and_marks_all_active_tracks() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [pane (javax.swing.JTextPane.)
            source "(d :kick :src :click :gate (p [1 0]))\n(d :hat :src :hat-909 :gate (p [1 0]))\n(start!)"
            highlighted (fn []
                          (->> (.getClientProperty pane glitchlisp.swing.editor/live-step-highlight-key)
                               (map (fn [[start end]] (subs source start end)))
                               vec))]
        (.setText pane source)
        (glitchlisp.swing.editor/install-live-gate-range-cache! pane)
        (.setCaretPosition pane (.indexOf source ":kick"))
        (glitchlisp.swing.editor/highlight-live-step! pane 0 nil)
        (let [ranges (highlighted)]
          (println (every? #(= "1" %) ranges))
          (println (= 2 (count ranges))))
        (.setCaretPosition pane (.indexOf source ":hat"))
        (glitchlisp.swing.editor/highlight-live-step! pane 0 nil)
        (println (= 2 (count (highlighted)))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run all active live highlight smoke");
    assert!(
        output.status.success(),
        "all active live highlight smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true"],
        "live highlight should mark all active tracks regardless of caret: {}",
        stdout
    );
}

#[test]
fn live_step_highlight_uses_active_by_scene_branch_for_def_tracks() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [pane (javax.swing.JTextPane.)
            source "(def hat\n  (d :hat\n     :src :hat-909\n     :note c6\n     :gate (by-scene\n             :intro (p [1 0 1 0])\n             :drop (p (then\n                        (times 4 [1 0 1 0])\n                        [1 1 0 1])))\n     :dur 0.025\n     :amp 0.08))\n\n(scene :intro :repeat 1 :next :drop\n  hat)\n\n(scene :drop :loop true\n  hat)\n\n(play-scene :intro)"
            highlighted (fn []
                          (->> (.getClientProperty pane glitchlisp.swing.editor/live-step-highlight-key)
                               (map (fn [[start end]] (subs source start end)))
                               vec))]
        (.setText pane source)
        (glitchlisp.swing.editor/install-live-gate-range-cache! pane)
        (glitchlisp.swing.editor/highlight-live-step! pane 0 "intro")
        (println (some #(= "1" %) (highlighted)))
        (glitchlisp.swing.editor/highlight-live-step! pane 0 "drop")
        (println (some #(clojure.string/starts-with? % "(times 4") (highlighted)))
        (glitchlisp.swing.editor/highlight-live-step! pane 4 "drop")
        (println (some #(= "1" %) (highlighted))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run by-scene live highlight smoke");
    assert!(
        output.status.success(),
        "by-scene live highlight smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false" || *line == "nil")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true"],
        "live highlight should use active by-scene branch for def tracks: {}",
        stdout
    );
}

#[test]
fn live_step_highlight_reuses_scene_range_until_document_changes() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [pane (javax.swing.JTextPane.)
            source "(scene :intro :loop true\n  (d :kick :src :click :gate (p [1 0])))\n(play-scene :intro)"]
        (.setText pane source)
        (glitchlisp.swing.editor/install-live-gate-range-cache! pane)
        (glitchlisp.swing.editor/highlight-live-step! pane 0 "intro")
        (let [cache (.getClientProperty pane glitchlisp.swing.editor/live-scene-ranges-key)
              first-range (get cache "intro")]
          (println (some? first-range))
          (glitchlisp.swing.editor/highlight-live-step! pane 1 "intro")
          (println (= first-range
                      (get (.getClientProperty pane glitchlisp.swing.editor/live-scene-ranges-key)
                           "intro"))))
        (.setText pane source)
        (println (nil? (.getClientProperty pane glitchlisp.swing.editor/live-scene-ranges-key))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live scene range cache smoke");
    assert!(
        output.status.success(),
        "live scene range cache smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true"],
        "live highlight should cache scene ranges until document changes: {}",
        stdout
    );
}

#[test]
fn live_step_highlight_reuses_scene_membership_context_until_document_changes() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [pane (javax.swing.JTextPane.)
            source (str "(def click\n"
                        "  (d :click :src :click :gate (p [1 0])))\n"
                        "(scene :intro :loop true\n"
                        "  click)\n"
                        "(play-scene :intro)")]
        (.setText pane source)
        (glitchlisp.swing.editor/install-live-gate-range-cache! pane)
        (glitchlisp.swing.editor/highlight-live-step! pane 0 "intro")
        (let [cache (.getClientProperty pane glitchlisp.swing.editor/live-scene-contexts-key)
              first-context (get cache "intro")]
          (println (contains? (:symbols first-context) "click"))
          (glitchlisp.swing.editor/highlight-live-step! pane 1 "intro")
          (println (identical? first-context
                               (get (.getClientProperty pane glitchlisp.swing.editor/live-scene-contexts-key)
                                    "intro"))))
        (.setText pane source)
        (println (nil? (.getClientProperty pane glitchlisp.swing.editor/live-scene-contexts-key))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live scene membership context cache smoke");
    assert!(
        output.status.success(),
        "live scene membership context cache smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true"],
        "live highlight should reuse scene membership context until document changes: {}",
        stdout
    );
}

#[test]
fn live_step_highlight_reuses_active_gate_entries_until_document_changes() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [pane (javax.swing.JTextPane.)
            source (str "(def click\n"
                        "  (d :click :src :click :gate (p [1 0])))\n"
                        "(def unused\n"
                        "  (d :unused :src :click :gate (p [0 1])))\n"
                        "(scene :intro :loop true\n"
                        "  click\n"
                        "  ; unused\n"
                        "  \"unused\")\n"
                        "(play-scene :intro)")]
        (.setText pane source)
        (glitchlisp.swing.editor/install-live-gate-range-cache! pane)
        (glitchlisp.swing.editor/highlight-live-step! pane 0 "intro")
        (let [cache (.getClientProperty pane glitchlisp.swing.editor/live-active-gate-entries-key)
              first-entries (get cache "intro")]
          (println (count first-entries))
          (glitchlisp.swing.editor/highlight-live-step! pane 1 "intro")
          (println (identical? first-entries
                               (get (.getClientProperty pane glitchlisp.swing.editor/live-active-gate-entries-key)
                                    "intro"))))
        (.setText pane source)
        (println (nil? (.getClientProperty pane glitchlisp.swing.editor/live-active-gate-entries-key))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live active gate entries cache smoke");
    assert!(
        output.status.success(),
        "live active gate entries cache smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "1" || *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["1", "true", "true"],
        "live highlight should reuse active gate entries until document changes: {}",
        stdout
    );
}

#[test]
fn live_step_highlight_reuses_resolved_ranges_for_repeating_steps() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [pane (javax.swing.JTextPane.)
            source "(d :kick :src :click :gate (p [1 0 1 0]))"]
        (.setText pane source)
        (glitchlisp.swing.editor/install-live-gate-range-cache! pane)
        (glitchlisp.swing.editor/highlight-live-step! pane 0 nil)
        (let [cache (.getClientProperty pane glitchlisp.swing.editor/live-resolved-step-ranges-key)
              values (:values cache)
              first-key (first (keys values))
              first-ranges (get values first-key)]
          (println (count values))
          (glitchlisp.swing.editor/highlight-live-step! pane 4 nil)
          (let [next-cache (.getClientProperty pane glitchlisp.swing.editor/live-resolved-step-ranges-key)]
            (println (count (:values next-cache)))
            (println (identical? first-ranges (get (:values next-cache) first-key)))))
        (glitchlisp.swing.editor/highlight-live-step! pane 1 nil)
        (println (count (:values (.getClientProperty pane glitchlisp.swing.editor/live-resolved-step-ranges-key))))
        (.setText pane source)
        (println (nil? (.getClientProperty pane glitchlisp.swing.editor/live-resolved-step-ranges-key)))
        (let [bounded (reduce (fn [cache n]
                                (glitchlisp.swing.editor/bounded-cache-assoc cache n n 256))
                              {}
                              (range 300))]
          (println (count (:values bounded)))
          (println (contains? (:values bounded) 0))
          (println (contains? (:values bounded) 299))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run live resolved range cache smoke");
    assert!(
        output.status.success(),
        "live resolved range cache smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| {
            *line == "1" || *line == "2" || *line == "256" || *line == "true" || *line == "false"
        })
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["1", "1", "true", "2", "true", "256", "false", "true"],
        "live highlight should reuse bounded resolved step ranges: {}",
        stdout
    );
}

#[test]
fn language_reference_next_scene_example_is_copy_safe() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/catalog.clj")
      (load-file "src/glitchlisp/swing/docs.clj")
      (let [[_ _ example] (some (fn [row]
                                  (when (= (first row) ":next :scene")
                                    row))
                                glitchlisp.swing.docs/scene-options)]
        (println example))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("extract language reference next-scene example");
    assert!(
        output.status.success(),
        "language reference extraction failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mut source = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.starts_with("(scene"))
        .collect::<Vec<_>>()
        .join("\n");
    source.push_str("\n(play-scene :a)\n");

    let mut runtime = Runtime::new();
    eval_program(&mut runtime, &source)
        .unwrap_or_else(|err| panic!("language reference :next example failed: {}", err));
    assert!(
        runtime.scenes.contains_key("a") && runtime.scenes.contains_key("b"),
        "expected :next example to define both scenes: {}",
        source
    );
    assert_eq!(
        runtime
            .scene_state
            .as_ref()
            .map(|state| state.current.as_str()),
        Some("a")
    );
}

#[test]
fn language_reference_scene_option_examples_start_scenes() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/catalog.clj")
      (load-file "src/glitchlisp/swing/docs.clj")
      (doseq [[name _ example] glitchlisp.swing.docs/scene-options]
        (println (str name "\t" (pr-str example))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("extract language reference scene option examples");
    assert!(
        output.status.success(),
        "language reference scene option extraction failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut checked = 0;
    for line in stdout.lines() {
        let parts = line.splitn(2, '\t').collect::<Vec<_>>();
        if parts.len() != 2 {
            continue;
        }
        let name = parts[0];
        let example = unescape_clojure_pr_str(parts[1]);
        let compiled = compile_source_for_runtime(&example).unwrap_or_else(|err| {
            panic!("scene option example '{}' did not compile: {}", name, err)
        });
        let mut runtime = Runtime::new();
        eval_program(&mut runtime, &compiled).unwrap_or_else(|err| {
            panic!("scene option example '{}' did not evaluate: {}", name, err)
        });
        assert_eq!(
            runtime
                .scene_state
                .as_ref()
                .map(|state| state.current.as_str()),
            Some("a"),
            "scene option example '{}' should start scene :a: {}",
            name,
            example
        );
        checked += 1;
    }
    assert_eq!(checked, 9, "expected every scene option row to be checked");
}

#[test]
fn language_reference_top_level_examples_are_copy_safe() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/catalog.clj")
      (load-file "src/glitchlisp/swing/docs.clj")
      (doseq [[name _ example] glitchlisp.swing.docs/top-level-forms]
        (println (str name "\t" (pr-str example))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("extract language reference top-level examples");
    assert!(
        output.status.success(),
        "language reference top-level extraction failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut example_count = 0;
    for line in stdout.lines() {
        let parts = line.splitn(2, '\t').collect::<Vec<_>>();
        if parts.len() != 2 {
            continue;
        }
        let name = parts[0];
        let example = unescape_clojure_pr_str(parts[1]);
        example_count += 1;
        let compiled = compile_source_for_runtime(&example)
            .unwrap_or_else(|err| panic!("top-level example '{}' did not compile: {}", name, err));
        let mut runtime = Runtime::new();
        eval_program(&mut runtime, &compiled)
            .unwrap_or_else(|err| panic!("top-level example '{}' did not evaluate: {}", name, err));
        if [
            "(d :id ...)",
            "(sample :id SOURCE ...)",
            "(post-fx [...])",
            "(mute :id)",
            "(unmute :id)",
            "(solo :id)",
            "(unsolo :id)",
            "(clear :id)",
        ]
        .contains(&name)
        {
            assert!(
                runtime.running && !runtime.tracks.is_empty(),
                "stateful top-level example '{}' should include an audio source and start playback: {}",
                name,
                example
            );
        }
        if name == "(scene :name ...)" {
            assert_eq!(
                runtime
                    .scene_state
                    .as_ref()
                    .map(|state| state.current.as_str()),
                Some("intro"),
                "scene definition example should start scene :intro: {}",
                example
            );
        }
        if name == "(stop!)" {
            assert!(
                !runtime.running && !runtime.tracks.is_empty(),
                "stop! example should demonstrate stopping active playback without deleting tracks: {}",
                example
            );
        }
        if name == "(clear-all)" {
            assert!(
                !runtime.running && runtime.tracks.is_empty() && runtime.scenes.is_empty(),
                "clear-all example should demonstrate clearing active runtime state: {}",
                example
            );
        }
    }

    assert!(
        example_count >= 15,
        "expected top-level examples from language reference, got {}: {}",
        example_count,
        stdout
    );
}

#[test]
fn language_reference_playback_alias_examples_are_copy_safe() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/catalog.clj")
      (load-file "src/glitchlisp/swing/docs.clj")
      (doseq [[name _ example] glitchlisp.swing.docs/compatibility-aliases]
        (when (#{"(play-block :name)" "(cue :name)"} name)
          (println (str name "\t" (pr-str example)))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("extract language reference playback alias examples");
    assert!(
        output.status.success(),
        "language reference playback alias extraction failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut example_count = 0;
    for line in stdout.lines() {
        let parts = line.splitn(2, '\t').collect::<Vec<_>>();
        if parts.len() != 2 {
            continue;
        }
        let name = parts[0];
        let example = unescape_clojure_pr_str(parts[1]);
        example_count += 1;
        let compiled = compile_source_for_runtime(&example).unwrap_or_else(|err| {
            panic!("playback alias example '{}' did not compile: {}", name, err)
        });
        let mut runtime = Runtime::new();
        eval_program(&mut runtime, &compiled).unwrap_or_else(|err| {
            panic!(
                "playback alias example '{}' did not evaluate: {}",
                name, err
            )
        });
        assert!(
            runtime.scene_state.is_some(),
            "playback alias example '{}' should start a scene: {}",
            name,
            example
        );
    }

    assert_eq!(
        example_count, 2,
        "expected play-block and cue alias examples, got {}: {}",
        example_count, stdout
    );
}

#[test]
fn language_reference_compatibility_alias_examples_show_context() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/catalog.clj")
      (load-file "src/glitchlisp/swing/docs.clj")
      (doseq [[name description example] glitchlisp.swing.docs/compatibility-aliases]
        (println (str name "\t" (pr-str description) "\t" (pr-str example))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("extract language reference compatibility aliases");
    assert!(
        output.status.success(),
        "language reference compatibility alias extraction failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut example_count = 0;
    let contextual_aliases = [
        ":repeats N",
        ":times N",
        ":length N",
        ":length-of :id",
        ":detune",
        ":pw",
        ":sample",
        "nil",
        ":resonance",
        ":gain_db",
        ":bit-depth",
        ":sample-rate-reduction",
        ":duration-pct",
        ":delay",
        "(g [...])",
        "(gs [...])",
        "(gate_seq [...])",
        "(rev X)",
        "(arp root kind)",
    ];
    for line in stdout.lines() {
        let parts = line.splitn(3, '\t').collect::<Vec<_>>();
        if parts.len() != 3 {
            continue;
        }
        let name = parts[0];
        let description = unescape_clojure_pr_str(parts[1]);
        let example = unescape_clojure_pr_str(parts[2]);
        example_count += 1;
        if contextual_aliases.contains(&name) {
            assert!(
                example.starts_with("(d ")
                    || example.starts_with("(scene ")
                    || example.starts_with(":fx ")
                    || example.contains("(post-fx")
                    || example.contains("(master-fx")
                    || example.starts_with(":note ")
                    || example.starts_with(":gate "),
                "compatibility alias '{}' should show its required context: {}",
                name,
                example
            );
        }

        let source = if example.starts_with(":fx ") {
            Some(format!(
                "(d :lead :src :sine-synth :note c3 :gate 1 {} :dur 0.1 :amp 0.2)\n(start!)",
                example
            ))
        } else if example.starts_with(":note ") {
            Some(format!(
                "(d :lead :src :sine-synth {} :gate 1 :dur 0.1 :amp 0.2)\n(start!)",
                example
            ))
        } else if example.starts_with(":gate ") {
            Some(format!(
                "(d :lead :src :sine-synth :note c3 {} :dur 0.1 :amp 0.2)\n(start!)",
                example
            ))
        } else if example.starts_with("(post-fx") || example.starts_with("(master-fx") {
            panic!(
                "compatibility alias '{}' should include its audio source: {}",
                name, example
            );
        } else if example.contains("(post-fx") || example.contains("(master-fx") {
            Some(example)
        } else if name == ":sample" {
            assert!(
                description.contains("requires an existing wav file"),
                ":sample compatibility alias should warn that the visible example is path-backed: {}",
                description
            );
            compile_source_for_runtime(&example).unwrap_or_else(|err| {
                panic!("compatibility alias '{}' did not compile: {}", name, err)
            });
            None
        } else {
            Some(example)
        };

        if let Some(source) = source {
            let compiled = compile_source_for_runtime(&source).unwrap_or_else(|err| {
                panic!("compatibility alias '{}' did not compile: {}", name, err)
            });
            let mut runtime = Runtime::new();
            eval_program(&mut runtime, &compiled).unwrap_or_else(|err| {
                panic!("compatibility alias '{}' did not evaluate: {}", name, err)
            });
            if source.contains("(post-fx") || source.contains("(master-fx") {
                assert!(
                    runtime.running && !runtime.tracks.is_empty(),
                    "compatibility alias '{}' should include a running source: {}",
                    name,
                    source
                );
            }
            if [":repeats N", ":times N", ":length N", ":length-of :id"].contains(&name) {
                assert_eq!(
                    runtime
                        .scene_state
                        .as_ref()
                        .map(|state| state.current.as_str()),
                    Some("a"),
                    "scene compatibility alias '{}' should start scene :a: {}",
                    name,
                    source
                );
            }
            if name == "(block :name ...)" {
                assert_eq!(
                    runtime
                        .scene_state
                        .as_ref()
                        .map(|state| state.current.as_str()),
                    Some("intro"),
                    "block compatibility alias should start scene :intro: {}",
                    source
                );
            }
            if [":detune", ":pw", "nil"].contains(&name) {
                assert!(
                    runtime.running && !runtime.tracks.is_empty(),
                    "track compatibility alias '{}' should start playback: {}",
                    name,
                    source
                );
            }
        }
    }

    assert!(
        example_count >= 20,
        "expected compatibility alias examples, got {}: {}",
        example_count,
        stdout
    );
}

#[test]
fn language_reference_times_example_is_gate_safe() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/catalog.clj")
      (load-file "src/glitchlisp/swing/docs.clj")
      (let [[_ _ example] (some (fn [row]
                                  (when (= (first row) "(times N PATTERN)")
                                    row))
                                glitchlisp.swing.docs/pattern-forms)]
        (println example))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("extract language reference times example");
    assert!(
        output.status.success(),
        "language reference pattern extraction failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let example = stdout
        .lines()
        .find(|line| line.starts_with("(p "))
        .unwrap_or_else(|| panic!("missing times example in output: {}", stdout))
        .to_string();
    assert!(
        example.starts_with("(p (then "),
        "times example should show the required p/then context: {}",
        example
    );

    let source = format!(
        "(d :kick :src :kick-synth :note c2 :gate {} :dur 0.1 :amp 0.2)\n(start!)",
        example
    );
    let compiled = compile_source_for_runtime(&source)
        .unwrap_or_else(|err| panic!("language reference times example did not compile: {}", err));
    let mut runtime = Runtime::new();
    eval_program(&mut runtime, &compiled)
        .unwrap_or_else(|err| panic!("language reference times example failed: {}", err));
    assert!(runtime.running);
}

#[test]
fn language_reference_compiler_examples_compile() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/catalog.clj")
      (load-file "src/glitchlisp/swing/docs.clj")
      (load-file "src/compiler.clj")
      (doseq [[name _ example] glitchlisp.swing.docs/compiler-forms]
        (try
          (let [source (if (.startsWith example ":note ")
                         (str "(d :lead :src :sine-synth " example " :gate 1 :dur 0.1 :amp 0.2)\n(start!)")
                         example)]
            (glitchlisp-compiler/compile-source source))
          (println (str "OK\t" name "\t" (pr-str example)))
          (catch Exception ex
            (println (str "ERR\t" name "\t" (.getMessage ex))))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("extract language reference compiler examples");
    assert!(
        output.status.success(),
        "language reference compiler example extraction failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut example_count = 0;
    let note_context_examples = [
        "(scale root kind count)",
        "(chord root kind)",
        "(chord root [intervals])",
        "(shape values [pos])",
        "(arpeggio root kind)",
        "(arpeggio root [intervals])",
    ];
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("OK\t") {
            let parts = rest.splitn(2, '\t').collect::<Vec<_>>();
            assert_eq!(
                parts.len(),
                2,
                "compiler example output should include name and example: {}",
                line
            );
            let name = parts[0];
            let example = unescape_clojure_pr_str(parts[1]);
            example_count += 1;
            assert!(!name.is_empty(), "empty compiler example name: {}", stdout);
            if note_context_examples.contains(&name) {
                assert!(
                    example.starts_with(":note "),
                    "compiler note helper '{}' should show :note context: {}",
                    name,
                    example
                );
            }
        } else if let Some(error) = line.strip_prefix("ERR\t") {
            panic!(
                "language reference compiler example did not compile: {}",
                error
            )
        }
    }

    assert!(
        example_count >= 25,
        "expected compiler-form examples from language reference, got {}: {}",
        example_count,
        stdout
    );
}

#[test]
fn language_reference_effect_examples_show_required_context() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/catalog.clj")
      (load-file "src/glitchlisp/swing/docs.clj")
      (doseq [[name _ example] (concat glitchlisp.swing.docs/effect-forms
                                        (glitchlisp.swing.docs/effect-rows))]
        (println (str name "\t" (pr-str example))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("extract language reference effect examples");
    assert!(
        output.status.success(),
        "language reference effect example extraction failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut contextual_count = 0;
    let mut checked_on_gate = false;
    for line in stdout.lines() {
        let parts = line.splitn(2, '\t').collect::<Vec<_>>();
        if parts.len() != 2 {
            continue;
        }
        let name = parts[0];
        let example = unescape_clojure_pr_str(parts[1]);
        let trimmed = example.trim();
        if name == "(on :gate PATTERN EFFECT)" {
            assert!(
                trimmed.starts_with("(d :lead"),
                "on-gate reference example should be directly copyable track source: {}",
                example
            );
            let compiled = compile_source_for_runtime(trimmed).unwrap_or_else(|err| {
                panic!(
                    "on-gate reference example did not compile directly: {}",
                    err
                )
            });
            let mut runtime = Runtime::new();
            eval_program(&mut runtime, &compiled).unwrap_or_else(|err| {
                panic!(
                    "on-gate reference example did not evaluate directly: {}",
                    err
                )
            });
            assert!(
                runtime.running && !runtime.tracks.is_empty(),
                "on-gate reference example should start playback: {}",
                example
            );
            checked_on_gate = true;
            contextual_count += 1;
            continue;
        }
        let source = if trimmed.starts_with(":fx") {
            contextual_count += 1;
            format!(
                "(d :lead :src :sine-synth :note c3 :gate 1 {})\n(start!)",
                trimmed
            )
        } else if trimmed.starts_with("(post-fx") {
            panic!(
                "effect reference post-fx example '{}' should include its audio source: {}",
                name, example
            );
        } else if trimmed.contains("(post-fx") {
            contextual_count += 1;
            trimmed.to_string()
        } else if trimmed.starts_with(':') {
            continue;
        } else {
            panic!(
                "effect reference example '{}' lacks :fx or post-fx context: {}",
                name, example
            );
        };

        let compiled = compile_source_for_runtime(&source)
            .unwrap_or_else(|err| panic!("effect example '{}' did not compile: {}", name, err));
        let mut runtime = Runtime::new();
        eval_program(&mut runtime, &compiled)
            .unwrap_or_else(|err| panic!("effect example '{}' did not evaluate: {}", name, err));
        if trimmed.contains("(post-fx") {
            assert!(
                runtime.running && !runtime.tracks.is_empty(),
                "post-fx effect example '{}' should include a running source: {}",
                name,
                example
            );
        }
    }

    assert!(
        contextual_count > 80,
        "expected contextual effect examples, got {}: {}",
        contextual_count,
        stdout
    );
    assert!(
        checked_on_gate,
        "missing on-gate reference example: {}",
        stdout
    );
}

#[test]
fn language_reference_effect_rows_show_keyword_choices() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/catalog.clj")
      (load-file "src/glitchlisp/swing/docs.clj")
      (doseq [[name desc _] (glitchlisp.swing.docs/effect-rows)]
        (when (#{"filter" "formant" "distort" "haas" "ams-reverb" "la2a" "sem-filter" "obxa-filter"} name)
          (println (str name "\t" desc))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("extract language reference effect keyword docs");
    assert!(
        output.status.success(),
        "language reference effect keyword extraction failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    for required in [
        "filter\tApply filter. Keywords: :type :lowpass, :lp",
        "formant\tApply formant. Keywords: :vowel :a, :e, :i, :o, :u",
        "distort\tApply distort. Keywords: :type :tanh, :hard-clip, :hard_clip",
        "haas\tApply haas. Keywords: :side :left, :right",
        "ams-reverb\tApply ams-reverb. Keywords: :program :nonlin, :non-linear, :nonlinear, :ambience, :ambient, :plate",
        "la2a\tApply la2a. Keywords: :mode :compress, :limit",
        "sem-filter\tApply sem-filter. Keywords: :type :lowpass, :lp",
        "obxa-filter\tApply obxa-filter. Keywords: :type :lowpass, :lp",
    ] {
        assert!(
            stdout.contains(required),
            "missing keyword choices '{}': {}",
            required,
            stdout
        );
    }
}

#[test]
fn language_reference_scale_and_chord_examples_show_note_context() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/catalog.clj")
      (load-file "src/glitchlisp/swing/docs.clj")
      (doseq [[name _ example] (concat (glitchlisp.swing.docs/scale-rows)
                                        (glitchlisp.swing.docs/chord-rows))]
        (println (str name "\t" (pr-str example))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("extract language reference scale/chord examples");
    assert!(
        output.status.success(),
        "language reference scale/chord extraction failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut example_count = 0;
    for line in stdout.lines() {
        let parts = line.splitn(2, '\t').collect::<Vec<_>>();
        if parts.len() != 2 {
            continue;
        }
        let name = parts[0];
        let example = unescape_clojure_pr_str(parts[1]);
        assert!(
            example.trim_start().starts_with(":note "),
            "scale/chord example '{}' should show :note context: {}",
            name,
            example
        );
        example_count += 1;
        let source = format!(
            "(d :lead :src :sine-synth {} :gate 1 :dur 0.1 :amp 0.2)\n(start!)",
            example
        );
        let compiled = compile_source_for_runtime(&source).unwrap_or_else(|err| {
            panic!("scale/chord example '{}' did not compile: {}", name, err)
        });
        let mut runtime = Runtime::new();
        eval_program(&mut runtime, &compiled).unwrap_or_else(|err| {
            panic!("scale/chord example '{}' did not evaluate: {}", name, err)
        });
    }

    assert!(
        example_count >= 35,
        "expected generated scale/chord examples, got {}: {}",
        example_count,
        stdout
    );
}

#[test]
fn language_reference_keeps_aliases_out_of_primary_sections() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/catalog.clj")
      (load-file "src/glitchlisp/swing/docs.clj")
      (let [top-names (set (map first glitchlisp.swing.docs/top-level-forms))
            scene-option-names (set (map first glitchlisp.swing.docs/scene-options))
            track-param-names (set (map first glitchlisp.swing.docs/track-params))
            pattern-names (set (map first glitchlisp.swing.docs/pattern-forms))
            effect-names (set (map first glitchlisp.swing.docs/effect-forms))
            compiler-names (set (map first glitchlisp.swing.docs/compiler-forms))
            syntax-names (set (map first glitchlisp.swing.docs/syntax-basics))
            aliases (set (map first glitchlisp.swing.docs/compatibility-aliases))]
        (println (contains? top-names "(post-fx [...])"))
        (println (not (contains? top-names "(master-fx [...])")))
        (println (contains? aliases "(master-fx [...])"))
        (println (contains? scene-option-names ":repeat N"))
        (println (not (contains? scene-option-names ":repeats N")))
        (println (not (contains? scene-option-names ":times N")))
        (println (contains? aliases ":repeats N"))
        (println (contains? aliases ":times N"))
        (println (contains? scene-option-names ":steps N"))
        (println (not (contains? scene-option-names ":length N")))
        (println (contains? aliases ":length N"))
        (println (contains? scene-option-names ":steps-of :id"))
        (println (not (contains? scene-option-names ":length-of :id")))
        (println (contains? aliases ":length-of :id"))
        (println (contains? scene-option-names ":loop-by :id N"))
        (println (contains? track-param-names ":detune-cents"))
        (println (not (contains? track-param-names ":detune")))
        (println (contains? aliases ":detune"))
        (println (contains? track-param-names ":pulse-width"))
        (println (not (contains? track-param-names ":pw")))
        (println (contains? aliases ":pw"))
        (println (contains? track-param-names ":sample-path"))
        (println (not (contains? track-param-names ":sample")))
        (println (contains? aliases ":sample"))
        (println (contains? pattern-names "(gate-seq [...])"))
        (println (contains? pattern-names "(g [...])"))
        (println (contains? pattern-names "(s [...])"))
        (println (not (contains? pattern-names "(gs [...])")))
        (println (not (contains? pattern-names "(gate_seq [...])")))
        (println (contains? aliases "(g [...])"))
        (println (contains? aliases "(gs [...])"))
        (println (contains? aliases "(gate_seq [...])"))
        (println (contains? compiler-names "(reverse X)"))
        (println (not (contains? compiler-names "(rev X)")))
        (println (contains? aliases "(rev X)"))
        (println (contains? compiler-names "(arpeggio root kind)"))
        (println (not (contains? compiler-names "(arp root kind)")))
        (println (contains? aliases "(arp root kind)"))
        (println (contains? syntax-names "null"))
        (println (not (contains? syntax-names "nil")))
        (println (contains? aliases "nil"))
        (println (contains? effect-names ":res"))
        (println (not (contains? effect-names ":res / :resonance")))
        (println (contains? aliases ":resonance"))
        (println (contains? effect-names "tape-stop duration"))
        (println (contains? aliases ":duration-pct"))
        (println (contains? aliases ":bit-depth"))
        (println (contains? aliases ":sample-rate-reduction"))
        (println (contains? aliases ":gain_db"))
        (println (contains? aliases ":delay")))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("inspect language reference aliases");
    assert!(
        output.status.success(),
        "language reference alias inspection failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    assert!(
        !results.is_empty() && results.iter().all(|line| *line == "true"),
        "unexpected language reference alias placement: {}",
        stdout
    );
}

#[test]
fn language_reference_quick_start_examples_are_runnable() {
    let script = r#"
      (do
        (load-file "src/glitchlisp/swing/shared.clj")
        (load-file "src/glitchlisp/swing/catalog.clj")
        (load-file "src/glitchlisp/swing/docs.clj")
        (doseq [[name _ example] glitchlisp.swing.docs/quick-start]
          (println (str name "\t" (pr-str example)))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("extract language reference quick start");
    assert!(
        output.status.success(),
        "language reference quick start extraction failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut checked = 0;
    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        let Some((name, example)) = line.split_once('\t') else {
            panic!("expected quick start row in output: {}", stdout);
        };
        let example = unescape_clojure_pr_str(example);
        let compiled = compile_source_for_runtime(&example)
            .unwrap_or_else(|err| panic!("quick start '{}' did not compile: {}", name, err));
        let mut runtime = Runtime::new();
        eval_program(&mut runtime, &compiled)
            .unwrap_or_else(|err| panic!("quick start '{}' did not evaluate: {}", name, err));
        assert!(
            runtime.running,
            "quick start '{}' should start playback",
            name
        );
        checked += 1;
    }
    assert_eq!(checked, 3, "expected every quick start row to be checked");
}

#[test]
fn editor_defaults_are_blank_and_aligned() {
    let script = r#"
      (do
        (load-file "src/glitchlisp/swing/shared.clj")
        (load-file "src/glitchlisp/swing/catalog.clj")
        (println (pr-str glitchlisp.swing.catalog/default-source)))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("extract swing default source");
    assert!(
        output.status.success(),
        "swing default source extraction failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let swing_default = unescape_clojure_pr_str(&String::from_utf8_lossy(&output.stdout));
    assert_eq!(
        editor::DEFAULT_SOURCE,
        swing_default,
        "native terminal editor default source should stay aligned with Swing first-run source"
    );
    assert!(
        editor::DEFAULT_SOURCE.is_empty(),
        "editors should open with a blank buffer"
    );
}

#[test]
fn scene_insert_template_honors_remove_comments_preference() {
    let script = r#"
      (load-file "src/main.clj")
      (println (pr-str (glitchlisp-swing/insert-scene-template "intro" true)))
      (println "---")
      (println (pr-str (glitchlisp-swing/insert-scene-template "intro" false)))
      (println "---")
      (println (pr-str (glitchlisp-swing/scene-wrapper "intro" "(d :lead :src :click :gate 1)" false)))
    "#;
    let output = Command::new("clojure")
        .env("GLITCHLISP_NO_GUI", "1")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("extract scene insert templates");
    assert!(
        output.status.success(),
        "scene template extraction failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parts = stdout.split("---").collect::<Vec<_>>();
    let [with_comments, without_comments, wrapped_without_comments] = parts.as_slice() else {
        panic!("expected scene template separator in output: {}", stdout);
    };
    assert!(
        with_comments.contains("; :loop true"),
        "default scene template should include helpful comments: {}",
        stdout
    );
    assert!(
        !without_comments.contains("; :loop true")
            && !without_comments.contains("; :steps 16")
            && !without_comments.contains("; :bars 1"),
        "remove-comments scene template should omit comments: {}",
        stdout
    );
    assert!(
        !wrapped_without_comments.contains("; :loop true")
            && !wrapped_without_comments.contains("; :steps 16")
            && !wrapped_without_comments.contains("; :bars 1"),
        "remove-comments scene wrapper should omit comments: {}",
        stdout
    );
}

#[test]
fn scene_insert_can_wrap_top_level_sample_track() {
    let script = r#"
      (load-file "src/main.clj")
      (let [pane (javax.swing.JTextPane.)
            category (javax.swing.JComboBox.)
            form (javax.swing.JComboBox.)]
        (.setText pane "(sample :hit :sample-data [1 0 -1] :gate (p [1 0]))")
        (.setCaretPosition pane (.indexOf (.getText pane) ":hit"))
        (.addItem category "Scene")
        (.setSelectedItem category "Scene")
        (.addItem form "scene")
        (.setSelectedItem form "scene")
        (glitchlisp-swing/insert-selected-form! pane category form "intro")
        (println (.getText pane)))
    "#;
    let output = Command::new("clojure")
        .env("GLITCHLISP_NO_GUI", "1")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run sample scene insertion smoke");
    assert!(
        output.status.success(),
        "sample scene insertion smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("(scene :intro")
            && stdout.contains("  (sample :hit")
            && !stdout.contains("(d :lead"),
        "sample scene insertion should wrap the sample track, not add a starter track: {}",
        stdout
    );
}

#[test]
fn arithmetic_insert_wrappers_keep_existing_token_parseable() {
    let script = r#"
      (load-file "src/main.clj")
      (doseq [option ["+" "-" "*" "/"]]
        (let [pane (javax.swing.JTextPane.)]
          (.setText pane "c3")
          (.setCaretPosition pane 1)
          (glitchlisp-swing/insert-math-logic-form! pane option "")
          (println (str option "\t" (.getText pane)))))
      (doseq [[label text needle] [["string" "\"c3\"" "c3"]
                                   ["comment" "; c3" "c3"]]]
        (let [pane (javax.swing.JTextPane.)]
          (.setText pane text)
          (.setCaretPosition pane (.indexOf text needle))
          (glitchlisp-swing/insert-math-logic-form! pane "+" "(+ 1 1)")
          (println (str label "\t" (.getText pane)))))
    "#;
    let output = Command::new("clojure")
        .env("GLITCHLISP_NO_GUI", "1")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run arithmetic insert wrapper smoke");
    assert!(
        output.status.success(),
        "arithmetic insert wrapper smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    for expected in ["+\t(+ c3 1)", "-\t(- c3 1)", "*\t(* c3 2)", "/\t(/ c3 2)"] {
        assert!(
            stdout.lines().any(|line| line == expected),
            "missing expected wrapped arithmetic form '{}': {}",
            expected,
            stdout
        );
    }
    assert!(
        stdout.lines().any(|line| line == "string\t\"(+ 1 1)c3\""),
        "string token should not be replaced as code: {}",
        stdout
    );
    assert!(
        stdout.lines().any(|line| line == "comment\t; (+ 1 1)c3"),
        "comment token should not be replaced as code: {}",
        stdout
    );
}

#[test]
fn playback_track_snippets_infer_current_track_id() {
    let script = r#"
      (load-file "src/main.clj")
      (let [inside (javax.swing.JTextPane.)
            outside (javax.swing.JTextPane.)
            single (javax.swing.JTextPane.)
            ambiguous (javax.swing.JTextPane.)
            inside-sample (javax.swing.JTextPane.)
            commented-track (javax.swing.JTextPane.)
            no-scene (javax.swing.JTextPane.)
            missing-scene-nonblank (javax.swing.JTextPane.)
            existing-scene (javax.swing.JTextPane.)
            top-level-track (javax.swing.JTextPane.)
            def-track-only (javax.swing.JTextPane.)
            scene-track-only (javax.swing.JTextPane.)]
        (.setText inside "(d :kick :src :kick-synth :gate 1)")
        (.setCaretPosition inside 5)
        (.setText outside "")
        (.setText single "(d :hat :src :hat-808 :gate 1)\n\n")
        (.setCaretPosition single (count (.getText single)))
        (.setText ambiguous "(d :kick :src :kick-synth :gate 1)\n(d :hat :src :hat-808 :gate 1)\n")
        (.setCaretPosition ambiguous (count (.getText ambiguous)))
        (.setText inside-sample "(d :kick :src :kick-synth :gate 1)\n(sample :hit :sample-data [1 0 -1] :gate 1)\n")
        (.setCaretPosition inside-sample (.indexOf (.getText inside-sample) ":hit"))
        (.setText commented-track "; (d :fake :src :click :gate 1)\n")
        (.setCaretPosition commented-track (.indexOf (.getText commented-track) ":fake"))
        (.setText no-scene "")
        (.setText missing-scene-nonblank "(d :lead :src :click :gate 1)\n")
        (.setText existing-scene "(scene :intro :loop true\n  (d :lead :src :click :gate 1))\n")
        (.setText top-level-track "(d :lead :src :click :gate 1)\n")
        (.setText def-track-only "(def lead\n  (d :lead :src :click :gate 1))\n")
        (.setText scene-track-only "(scene :intro :loop true\n  (d :lead :src :click :gate 1))\n")
        (doseq [[label pane] [["inside" inside] ["outside" outside] ["single" single] ["ambiguous" ambiguous] ["inside-sample" inside-sample] ["commented-track" commented-track]]
                option ["mute" "unmute" "solo" "unsolo" "clear"]]
          (println (str label "\t" option "\t"
                        (pr-str (glitchlisp-swing/insert-form-snippet pane "Playback" option "intro" false)))))
        (doseq [[label pane] [["no-scene" no-scene] ["missing-scene-nonblank" missing-scene-nonblank] ["existing-scene" existing-scene]]]
          (println (str label "\tplay-scene\t"
                        (pr-str (glitchlisp-swing/insert-form-snippet pane "Playback" "play-scene" "intro" false)))))
        (doseq [[label pane] [["outside" outside] ["top-level-track" top-level-track] ["def-track-only" def-track-only] ["scene-track-only" scene-track-only]]]
          (println (str label "\tstart!\t"
                        (pr-str (glitchlisp-swing/insert-form-snippet pane "Playback" "start!" "intro" false)))))
        (println (str "options\tPlayback\t"
                      (clojure.string/join "," (glitchlisp-swing/insert-form-options "Playback"))))
        (println (str "outside\tstop!\t"
                      (pr-str (glitchlisp-swing/insert-form-snippet outside "Playback" "stop!" "intro" false))))
        (println (str "top-level-track\tstop!\t"
                      (pr-str (glitchlisp-swing/insert-form-snippet top-level-track "Playback" "stop!" "intro" false))))
        (println (str "outside\tplay-note\t"
                      (pr-str (glitchlisp-swing/insert-form-snippet outside "Playback" "play-note" "intro" false))))
        (println (str "outside\tbpm\t"
                      (pr-str (glitchlisp-swing/insert-form-snippet outside "Playback" "bpm" "intro" false))))
        (println (str "outside\tclear-all\t"
                      (pr-str (glitchlisp-swing/insert-form-snippet outside "Playback" "clear-all" "intro" false))))
        (println (str "top-level-track\tclear-all\t"
                      (pr-str (glitchlisp-swing/insert-form-snippet top-level-track "Playback" "clear-all" "intro" false)))))
    "#;
    let output = Command::new("clojure")
        .env("GLITCHLISP_NO_GUI", "1")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run playback track snippet smoke");
    assert!(
        output.status.success(),
        "playback track snippet smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    for expected in [
        "inside\tmute\t\"(mute :kick)\\n\"",
        "inside\tunmute\t\"(unmute :kick)\\n\"",
        "inside\tsolo\t\"(solo :kick)\\n\"",
        "inside\tunsolo\t\"(unsolo :kick)\\n\"",
        "inside\tclear\t\"(clear :kick)\\n\"",
        "outside\tmute\t\"(d :lead :src :sine-synth :note c3 :gate 1)\\n(start!)\\n(mute :lead)\\n\"",
        "outside\tunmute\t\"(d :lead :src :sine-synth :note c3 :gate 1)\\n(start!)\\n(mute :lead)\\n(unmute :lead)\\n\"",
        "outside\tsolo\t\"(d :lead :src :sine-synth :note c3 :gate 1)\\n(start!)\\n(solo :lead)\\n\"",
        "outside\tunsolo\t\"(d :lead :src :sine-synth :note c3 :gate 1)\\n(start!)\\n(solo :lead)\\n(unsolo :lead)\\n\"",
        "outside\tclear\t\"(d :lead :src :sine-synth :note c3 :gate 1)\\n(d :keep :src :sine-synth :note e3 :gate 1)\\n(start!)\\n(clear :lead)\\n\"",
        "single\tmute\t\"(mute :hat)\\n\"",
        "single\tunmute\t\"(unmute :hat)\\n\"",
        "single\tsolo\t\"(solo :hat)\\n\"",
        "single\tunsolo\t\"(unsolo :hat)\\n\"",
        "single\tclear\t\"(clear :hat)\\n\"",
        "ambiguous\tmute\t\"; (mute :track)\\n\"",
        "ambiguous\tunmute\t\"; (unmute :track)\\n\"",
        "ambiguous\tsolo\t\"; (solo :track)\\n\"",
        "ambiguous\tunsolo\t\"; (unsolo :track)\\n\"",
        "ambiguous\tclear\t\"; (clear :track)\\n\"",
        "inside-sample\tmute\t\"(mute :hit)\\n\"",
        "inside-sample\tunmute\t\"(unmute :hit)\\n\"",
        "inside-sample\tsolo\t\"(solo :hit)\\n\"",
        "inside-sample\tunsolo\t\"(unsolo :hit)\\n\"",
        "inside-sample\tclear\t\"(clear :hit)\\n\"",
        "commented-track\tmute\t\"; (mute :track)\\n\"",
        "commented-track\tunmute\t\"; (unmute :track)\\n\"",
        "commented-track\tsolo\t\"; (solo :track)\\n\"",
        "commented-track\tunsolo\t\"; (unsolo :track)\\n\"",
        "commented-track\tclear\t\"; (clear :track)\\n\"",
        "no-scene\tplay-scene\t\"(scene :intro :loop true\\n  (d :lead :src :sine-synth :note c3 :gate 1))\\n(play-scene :intro)\\n\"",
        "missing-scene-nonblank\tplay-scene\t\"; (play-scene :intro)\\n\"",
        "existing-scene\tplay-scene\t\"(play-scene :intro)\\n\"",
        "outside\tstart!\t\"(d :lead :src :sine-synth :note c3 :gate 1)\\n(start!)\\n\"",
        "top-level-track\tstart!\t\"(start!)\\n\"",
        "def-track-only\tstart!\t\"; (start!)\\n\"",
        "scene-track-only\tstart!\t\"; (start!)\\n\"",
        "options\tPlayback\tstart!,stop!,play-scene,play-note,bpm,mute,unmute,solo,unsolo,clear,clear-all",
        "outside\tstop!\t\"(d :lead :src :sine-synth :note c3 :gate 1)\\n(start!)\\n(stop!)\\n\"",
        "top-level-track\tstop!\t\"(stop!)\\n\"",
        "outside\tplay-note\t\"(play-note c3)\\n\"",
        "outside\tbpm\t\"(bpm 100)\\n\"",
        "outside\tclear-all\t\"(d :lead :src :sine-synth :note c3 :gate 1)\\n(start!)\\n(clear-all)\\n\"",
        "top-level-track\tclear-all\t\"(clear-all)\\n\"",
    ] {
        assert!(
            stdout.lines().any(|line| line == expected),
            "missing expected playback snippet '{}': {}",
            expected,
            stdout
        );
    }
}

#[test]
fn blank_playback_state_and_scene_snippets_evaluate() {
    let script = r#"
      (load-file "src/main.clj")
      (let [pane (javax.swing.JTextPane.)]
        (doseq [option ["start!" "play-scene" "mute" "unmute" "solo" "unsolo" "clear" "stop!" "clear-all"]]
          (.setText pane "")
          (println (str option "\t"
                        (pr-str (glitchlisp-swing/insert-form-snippet pane "Playback" option "intro" false))))))
    "#;
    let output = Command::new("clojure")
        .env("GLITCHLISP_NO_GUI", "1")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("extract blank playback state snippets");
    assert!(
        output.status.success(),
        "blank playback snippet extraction failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut checked = 0;
    for line in stdout.lines() {
        let Some((option, snippet)) = line.split_once('\t') else {
            continue;
        };
        let snippet = unescape_clojure_pr_str(snippet);
        let mut runtime = Runtime::new();
        eval_program(&mut runtime, &snippet)
            .unwrap_or_else(|err| panic!("Playback {} snippet failed: {}", option, err));
        match option {
            "start!" => {
                assert!(runtime.running, "{}", snippet);
                assert!(runtime.tracks.contains_key("lead"), "{}", snippet);
            }
            "play-scene" => assert_eq!(
                runtime
                    .scene_state
                    .as_ref()
                    .map(|state| state.current.as_str()),
                Some("intro"),
                "{}",
                snippet
            ),
            "mute" => assert!(runtime.tracks["lead"].muted, "{}", snippet),
            "unmute" => assert!(!runtime.tracks["lead"].muted, "{}", snippet),
            "solo" => assert!(runtime.tracks["lead"].solo, "{}", snippet),
            "unsolo" => assert!(!runtime.tracks["lead"].solo, "{}", snippet),
            "clear" => {
                assert!(!runtime.tracks.contains_key("lead"), "{}", snippet);
                assert!(runtime.tracks.contains_key("keep"), "{}", snippet);
                assert!(runtime.running, "{}", snippet);
            }
            "stop!" => {
                assert!(!runtime.running, "{}", snippet);
                assert!(runtime.tracks.contains_key("lead"), "{}", snippet);
            }
            "clear-all" => {
                assert!(!runtime.running, "{}", snippet);
                assert!(runtime.tracks.is_empty(), "{}", snippet);
            }
            other => panic!("unexpected blank Playback option: {}", other),
        }
        checked += 1;
    }
    assert_eq!(
        checked, 9,
        "expected every blank Playback state/scene snippet"
    );
}

#[test]
fn fx_insert_outside_track_creates_runnable_track_context() {
    let script = r#"
      (load-file "src/main.clj")
      (let [outside (javax.swing.JTextPane.)
            outside-on-gate (javax.swing.JTextPane.)
            inside (javax.swing.JTextPane.)
            fake-fx (javax.swing.JTextPane.)
            gate-wrap-string (javax.swing.JTextPane.)
            fake-gate (javax.swing.JTextPane.)
            category (javax.swing.JComboBox.)
            form (javax.swing.JComboBox.)]
        (.addItem category "FX")
        (.setSelectedItem category "FX")
        (.addItem form "delay")
        (.setSelectedItem form "delay")
        (glitchlisp-swing/insert-selected-form! outside category form "intro")
        (println "outside")
        (println (.getText outside))
        (.removeAllItems form)
        (.addItem form "on :gate")
        (.setSelectedItem form "on :gate")
        (glitchlisp-swing/insert-selected-form! outside-on-gate category form "intro")
        (println "outside-on-gate")
        (println (.getText outside-on-gate))
        (.removeAllItems form)
        (.addItem form "delay")
        (.setSelectedItem form "delay")
        (.setText inside "(d :lead :src :sine-synth :note c3 :gate 1)")
        (.setCaretPosition inside 5)
        (glitchlisp-swing/insert-selected-form! inside category form "intro")
        (println "inside")
        (println (.getText inside))
        (.setText fake-fx "(d :lead :src :sine-synth :note c3 :gate 1 :label \":fx [(fake)]\")")
        (.setCaretPosition fake-fx 5)
        (glitchlisp-swing/insert-selected-form! fake-fx category form "intro")
        (println "fake-fx")
        (println (.getText fake-fx))
        (.setText gate-wrap-string "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(delay :label \"(fake)\" :mix 0.2)])")
        (.setCaretPosition gate-wrap-string (.indexOf (.getText gate-wrap-string) "fake"))
        (.removeAllItems form)
        (.addItem form "on :gate")
        (.setSelectedItem form "on :gate")
        (glitchlisp-swing/insert-selected-form! gate-wrap-string category form "intro")
        (println "gate-wrap-string")
        (println (.getText gate-wrap-string))
        (.setText fake-gate "(d :lead :src :sine-synth :label \":gate (p [0 0])\" :note c3 :gate (p [1 0]) :fx [(delay :mix 0.2)])")
        (.setCaretPosition fake-gate (.indexOf (.getText fake-gate) "delay"))
        (glitchlisp-swing/insert-selected-form! fake-gate category form "intro")
        (println "fake-gate")
        (println (.getText fake-gate)))
    "#;
    let output = Command::new("clojure")
        .env("GLITCHLISP_NO_GUI", "1")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run fx insert context smoke");
    assert!(
        output.status.success(),
        "fx insert context smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some((outside, rest)) = stdout.split_once("outside-on-gate\n") else {
        panic!("expected outside-on-gate marker in output: {}", stdout);
    };
    let Some((outside_on_gate, rest)) = rest.split_once("inside\n") else {
        panic!("expected inside marker in output: {}", stdout);
    };
    let Some((inside, fake_fx)) = rest.split_once("fake-fx\n") else {
        panic!("expected fake-fx marker in output: {}", stdout);
    };
    let Some((fake_fx, gate_wrap_string)) = fake_fx.split_once("gate-wrap-string\n") else {
        panic!("expected gate-wrap-string marker in output: {}", stdout);
    };
    let Some((gate_wrap_string, fake_gate)) = gate_wrap_string.split_once("fake-gate\n") else {
        panic!("expected fake-gate marker in output: {}", stdout);
    };
    assert!(
        outside.contains("(d :fx-demo")
            && outside.contains(":fx [")
            && outside.contains("(delay")
            && outside.contains("(start!)"),
        "outside-track FX insertion should create a track context: {}",
        stdout
    );
    assert!(
        outside_on_gate.contains("(d :fx-demo")
            && outside_on_gate.contains(":fx [")
            && outside_on_gate.contains("(on :gate (p [1 0])")
            && outside_on_gate.contains("(delay")
            && outside_on_gate.contains("(start!)"),
        "outside-track on-gate insertion should create a runnable track context: {}",
        stdout
    );
    assert!(
        inside.contains("(d :lead")
            && inside.contains(":fx [")
            && inside.contains("(delay")
            && !inside.contains(":fx-demo"),
        "inside-track FX insertion should modify the existing track: {}",
        stdout
    );
    assert!(
        fake_fx.contains(":label \":fx [(fake)]\"")
            && fake_fx.contains("\n   :fx [")
            && fake_fx.contains("(delay")
            && !fake_fx.contains(":fx [(fake)\n"),
        "FX insertion should ignore :fx text inside strings: {}",
        stdout
    );
    assert!(
        gate_wrap_string.contains(":label \"(fake)\"")
            && gate_wrap_string.contains("(on :gate 1")
            && gate_wrap_string.contains("(delay")
            && !gate_wrap_string.contains(":label \"(on :gate"),
        "on-gate wrapping should ignore parentheses inside strings: {}",
        stdout
    );
    assert!(
        fake_gate.contains(":label \":gate (p [0 0])\"")
            && fake_gate.contains("(on :gate (p [1 0])")
            && fake_gate.contains("(delay")
            && !fake_gate.contains("(on :gate (p [0 0])"),
        "on-gate wrapping should ignore :gate text inside strings: {}",
        stdout
    );

    let source = outside
        .lines()
        .skip_while(|line| *line != "outside")
        .skip(1)
        .collect::<Vec<_>>()
        .join("\n");
    let mut runtime = Runtime::new();
    eval_program(&mut runtime, &source)
        .unwrap_or_else(|err| panic!("outside-track FX insertion did not evaluate: {}", err));
    assert!(
        runtime.running,
        "outside-track FX insertion should start playback"
    );
    assert!(runtime.tracks.contains_key("fx-demo"));

    let source = outside_on_gate.lines().collect::<Vec<_>>().join("\n");
    let mut runtime = Runtime::new();
    eval_program(&mut runtime, &source)
        .unwrap_or_else(|err| panic!("outside-track on-gate insertion did not evaluate: {}", err));
    assert!(
        runtime.running,
        "outside-track on-gate insertion should start playback"
    );
    assert!(runtime.tracks.contains_key("fx-demo"));
}

#[test]
fn post_fx_insert_in_blank_editor_creates_runnable_source() {
    let script = r#"
      (load-file "src/main.clj")
      (let [blank (javax.swing.JTextPane.)
            nonblank (javax.swing.JTextPane.)
            category (javax.swing.JComboBox.)
            form (javax.swing.JComboBox.)]
        (.addItem category "Post FX")
        (.setSelectedItem category "Post FX")
        (.addItem form "reverse")
        (.setSelectedItem form "reverse")
        (glitchlisp-swing/insert-selected-form! blank category form "intro")
        (println "blank")
        (println (.getText blank))
        (.setText nonblank "(d :lead :src :sine-synth :note c3 :gate 1)\n(start!)\n")
        (.setCaretPosition nonblank (.getLength (.getDocument nonblank)))
        (glitchlisp-swing/insert-selected-form! nonblank category form "intro")
        (println "nonblank")
        (println (.getText nonblank)))
    "#;
    let output = Command::new("clojure")
        .env("GLITCHLISP_NO_GUI", "1")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run post-fx insert context smoke");
    assert!(
        output.status.success(),
        "post-fx insert context smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some((blank, nonblank)) = stdout.split_once("nonblank\n") else {
        panic!("expected nonblank marker in output: {}", stdout);
    };
    assert!(
        blank.contains("(d :post-fx-demo")
            && blank.contains("(post-fx [")
            && blank.contains("(start!)"),
        "blank Post FX insertion should create runnable source: {}",
        stdout
    );
    assert!(
        nonblank.contains("(d :lead")
            && nonblank.contains("(post-fx [")
            && !nonblank.contains(":post-fx-demo"),
        "nonblank Post FX insertion should keep pure top-level post-fx insertion: {}",
        stdout
    );

    let source = blank
        .lines()
        .skip_while(|line| *line != "blank")
        .skip(1)
        .take_while(|line| *line != "nonblank")
        .collect::<Vec<_>>()
        .join("\n");
    let mut runtime = Runtime::new();
    eval_program(&mut runtime, &source)
        .unwrap_or_else(|err| panic!("blank Post FX insertion did not evaluate: {}", err));
    assert!(
        runtime.running,
        "blank Post FX insertion should start playback"
    );
    assert!(runtime.tracks.contains_key("post-fx-demo"));
    assert!(!runtime.post_effects.is_empty());
}

#[test]
fn pattern_insert_in_blank_editor_creates_runnable_gate_track() {
    let script = r#"
      (load-file "src/main.clj")
      (let [blank (javax.swing.JTextPane.)
            inline (javax.swing.JTextPane.)
            category (javax.swing.JComboBox.)
            form (javax.swing.JComboBox.)]
        (.addItem category "Pattern")
        (.setSelectedItem category "Pattern")
        (.addItem form "then / times")
        (.setSelectedItem form "then / times")
        (glitchlisp-swing/insert-selected-form! blank category form "intro")
        (println "blank")
        (println (.getText blank))
        (.setText inline ":gate ")
        (.setCaretPosition inline (.getLength (.getDocument inline)))
        (glitchlisp-swing/insert-selected-form! inline category form "intro")
        (println "inline")
        (println (.getText inline)))
    "#;
    let output = Command::new("clojure")
        .env("GLITCHLISP_NO_GUI", "1")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run pattern insert context smoke");
    assert!(
        output.status.success(),
        "pattern insert context smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some((blank, inline)) = stdout.split_once("inline\n") else {
        panic!("expected inline marker in output: {}", stdout);
    };
    assert!(
        blank.contains("(d :pattern-demo")
            && blank.contains(":gate (p (then")
            && blank.contains("(start!)"),
        "blank Pattern insertion should create runnable source: {}",
        stdout
    );
    assert!(
        inline.contains(":gate (p (then")
            && !inline.contains(":pattern-demo")
            && !inline.contains("(start!)"),
        "nonblank Pattern insertion should keep inline fragment behavior: {}",
        stdout
    );

    let source = blank
        .lines()
        .skip_while(|line| *line != "blank")
        .skip(1)
        .take_while(|line| *line != "inline")
        .collect::<Vec<_>>()
        .join("\n");
    let compiled = compile_source_for_runtime(&source)
        .unwrap_or_else(|err| panic!("blank Pattern insertion did not compile: {}", err));
    let mut runtime = Runtime::new();
    eval_program(&mut runtime, &compiled)
        .unwrap_or_else(|err| panic!("blank Pattern insertion did not evaluate: {}", err));
    assert!(
        runtime.running,
        "blank Pattern insertion should start playback"
    );
    assert!(runtime.tracks.contains_key("pattern-demo"));
}

#[test]
fn oscillator_insert_uses_blank_parameter_form() {
    let script = r#"
      (load-file "src/main.clj")
      (let [blank (javax.swing.JTextPane.)
            nonblank (javax.swing.JTextPane.)
            category (javax.swing.JComboBox.)
            form (javax.swing.JComboBox.)]
        (.addItem category "Oscillator")
        (.setSelectedItem category "Oscillator")
        (.addItem form "Synth / sine-synth")
        (.setSelectedItem form "Synth / sine-synth")
        (glitchlisp-swing/insert-selected-form! blank category form "intro")
        (println "blank")
        (println (.getText blank))
        (.setText nonblank "(bpm 100)\n")
        (.setCaretPosition nonblank (.getLength (.getDocument nonblank)))
        (glitchlisp-swing/insert-selected-form! nonblank category form "intro")
        (println "nonblank")
        (println (.getText nonblank)))
    "#;
    let output = Command::new("clojure")
        .env("GLITCHLISP_NO_GUI", "1")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run oscillator insert context smoke");
    assert!(
        output.status.success(),
        "oscillator insert context smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some((blank, nonblank)) = stdout.split_once("nonblank\n") else {
        panic!("expected nonblank marker in output: {}", stdout);
    };
    assert!(
        blank.contains("(d :sine")
            && blank.contains(":src :sine-synth")
            && blank.contains(":note null")
            && blank.contains(":gate null")
            && blank.contains(":dur null")
            && blank.contains(":amp null")
            && blank.contains("(start!)"),
        "blank Oscillator insertion should create a blank parameter form: {}",
        stdout
    );
    assert!(
        blank.rfind(":gate null") > blank.rfind(":amp null"),
        "blank Oscillator insertion should place :gate at the bottom: {}",
        stdout
    );
    assert!(
        nonblank.contains("(bpm 100)")
            && nonblank.contains("(d :sine")
            && nonblank.contains(":note null")
            && nonblank.contains(":gate null")
            && !nonblank.contains("(start!)"),
        "nonblank Oscillator insertion should keep blank track-only behavior: {}",
        stdout
    );
}

#[test]
fn track_insert_ignores_open_scene_text_inside_comments_and_strings() {
    let script = r#"
      (load-file "src/main.clj")
      (doseq [[label text] [["comment" "; (scene :fake\n"]
                            ["string" "\"(scene :fake\""]]]
        (let [pane (javax.swing.JTextPane.)]
          (.setText pane text)
          (.setCaretPosition pane (.getLength (.getDocument pane)))
          (glitchlisp-swing/insert-track-form! pane "(d :lead :src :click :gate 1)")
          (println label)
          (println (.getText pane))))
    "#;
    let output = Command::new("clojure")
        .env("GLITCHLISP_NO_GUI", "1")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run track insert comment/string smoke");
    assert!(
        output.status.success(),
        "track insert comment/string smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some((comment, string)) = stdout.split_once("string\n") else {
        panic!("expected string marker in output: {}", stdout);
    };
    assert!(
        comment.contains("; (scene :fake\n(d :lead :src :click :gate 1)")
            && !comment.contains("  (d :lead")
            && !comment.trim_end().ends_with("))"),
        "commented open scene should not receive scene-indented insertion or an added close paren: {}",
        stdout
    );
    assert!(
        string.contains("\"(scene :fake\"\n(d :lead :src :click :gate 1)")
            && !string.contains("  (d :lead")
            && !string.trim_end().ends_with("))"),
        "string open scene text should not receive scene-indented insertion or an added close paren: {}",
        stdout
    );
}

fn unescape_clojure_pr_str(value: &str) -> String {
    let mut chars = value.trim().trim_matches('"').chars();
    let mut output = String::new();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => output.push('\n'),
                Some('t') => output.push('\t'),
                Some('"') => output.push('"'),
                Some('\\') => output.push('\\'),
                Some(other) => output.push(other),
                None => output.push('\\'),
            }
        } else {
            output.push(ch);
        }
    }
    output
}

#[test]
fn swing_main_load_does_not_launch_ui_in_headless_mode() {
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg("(load-file \"src/main.clj\") (println :loaded)")
        .output()
        .expect("load Swing main in headless mode");
    assert!(
        output.status.success(),
        "headless Swing main load failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.lines().any(|line| line == ":loaded"),
        "expected load marker in stdout: {}",
        stdout
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("HeadlessException"),
        "headless load should not launch Swing UI: {}",
        stderr
    );
}

#[test]
fn file_menu_source_lists_new_save_as_and_exit() {
    let source = std::fs::read_to_string("src/main.clj").expect("read Swing main source");
    for label in [
        "(menu-item \"New\"",
        "(menu-item \"Open...\"",
        "(menu-item \"Save\"",
        "(menu-item \"Save As...\"",
        "(menu-item \"Save Audio...\"",
        "(menu-item \"Exit\"",
    ] {
        assert!(source.contains(label), "missing File menu item {}", label);
    }
}

#[test]
fn insert_form_categories_hide_math_logic_and_pattern() {
    let source = std::fs::read_to_string("src/main.clj").expect("read Swing main source");
    assert!(
        source.contains(
            "(def insert-form-categories\n  [\"Oscillator\" \"FX\" \"Post FX\" \"Scene\" \"Playback\"]"
        ),
        "insert form categories should hide Math / Logic and Pattern"
    );
}

#[test]
fn new_file_adds_blank_tab_and_clears_active_file_state() {
    let script = r#"
      (load-file "src/main.clj")
      (let [tabs (javax.swing.JTabbedPane.)
            status (javax.swing.JLabel. "stale")]
        (glitchlisp-swing/add-editor-tab! tabs status "(d :lead :src :click :gate 1)" (java.io.File. "old.gl"))
        (swap! glitchlisp-swing/state assoc :file (java.io.File. "old.gl"))
        (glitchlisp-swing/new-file! tabs status)
        (let [editor (glitchlisp-swing/active-editor tabs)]
          (println (= 2 (.getTabCount tabs)))
          (println (= "" (.getText editor)))
          (println (nil? (glitchlisp-swing/editor-file editor)))
          (println (not (glitchlisp-swing/editor-dirty? editor))))
        (println (nil? (:file @glitchlisp-swing/state)))
        (println (.getText status))
        (flush)
        (System/exit 0))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run new file helper smoke");
    assert!(
        output.status.success(),
        "new file helper smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "new file")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true", "true", "true", "new file"],
        "new-file should add a blank active tab and clear active file state: {}",
        stdout
    );
}

#[test]
fn editor_tabs_mark_dirty_and_install_close_header() {
    let script = r#"
      (load-file "src/main.clj")
      (let [tabs (javax.swing.JTabbedPane.)
            status (javax.swing.JLabel. "stale")
            editor (glitchlisp-swing/add-editor-tab! tabs status "(bpm 118)" nil)]
        (.insertString (.getDocument editor) (.getLength (.getDocument editor)) "\n" nil)
        (println (glitchlisp-swing/editor-dirty? editor))
        (println (clojure.string/starts-with? (.getTitleAt tabs 0) "*"))
        (println (some? (.getTabComponentAt tabs 0)))
        (flush)
        (System/exit 0))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run editor tab dirty smoke");
    assert!(
        output.status.success(),
        "editor tab dirty smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true"],
        "editor tab should mark dirty and install close header: {}",
        stdout
    );
}

#[test]
fn editor_tab_rename_sets_save_suggestion() {
    let script = r#"
      (load-file "src/main.clj")
      (let [tabs (javax.swing.JTabbedPane.)
            status (javax.swing.JLabel. "stale")
            editor (glitchlisp-swing/add-editor-tab! tabs status "(bpm 118)" nil)]
        (glitchlisp-swing/set-editor-tab-name! editor "drums")
        (glitchlisp-swing/refresh-tab-title! tabs editor)
        (println (= "drums" (glitchlisp-swing/editor-tab-name editor 1)))
        (println (= "drums" (.getText (.getClientProperty editor "mescript.tab-label"))))
        (println (= "drums.gl" (.getName (glitchlisp-swing/suggested-save-file editor))))
        (flush)
        (System/exit 0))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run editor tab rename smoke");
    assert!(
        output.status.success(),
        "editor tab rename smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true"],
        "renamed tab should update title and save suggestion: {}",
        stdout
    );
}

#[test]
fn custom_tab_header_selection_selects_clicked_tab() {
    let script = r#"
      (load-file "src/main.clj")
      (let [tabs (javax.swing.JTabbedPane.)
            status (javax.swing.JLabel. "stale")
            first (glitchlisp-swing/add-editor-tab! tabs status "(bpm 118)" (java.io.File. "first.gl"))
            second (glitchlisp-swing/add-editor-tab! tabs status "(bpm 120)" (java.io.File. "second.gl"))]
        (glitchlisp-swing/select-editor-tab! tabs first)
        (println (= first (glitchlisp-swing/active-editor tabs)))
        (println (= (java.io.File. "first.gl") (:file @glitchlisp-swing/state)))
        (glitchlisp-swing/select-editor-tab! tabs second)
        (println (= second (glitchlisp-swing/active-editor tabs)))
        (println (= (java.io.File. "second.gl") (:file @glitchlisp-swing/state)))
        (flush)
        (System/exit 0))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run tab header selection smoke");
    assert!(
        output.status.success(),
        "tab header selection smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true")
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true", "true", "true"],
        "custom tab header selection should select clicked tab: {}",
        stdout
    );
}

#[test]
fn line_numbers_use_four_digit_minimum_width() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (let [pane (javax.swing.JTextPane.)]
        (.setText pane "a\nb\nc")
        (println (pr-str (glitchlisp.swing.editor/line-number-text pane))))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run line number width smoke");
    assert!(
        output.status.success(),
        "line number width smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .rev()
        .find(|line| line.starts_with('"'))
        .unwrap_or_else(|| panic!("missing line number output: {}", stdout));
    let gutter = unescape_clojure_pr_str(line);
    assert!(
        gutter.starts_with("1    "),
        "line numbers should reserve four left-aligned digits by default: {:?}",
        gutter
    );
}

#[test]
fn swing_compile_expands_includes_relative_to_current_file() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/editor.clj")
      (load-file "src/glitchlisp/swing/render.clj")
      (let [dir (java.io.File. (str "/tmp/glitchlisp-swing-include-" (.getName (java.io.File. "."))))
            _ (.mkdirs (java.io.File. dir "parts"))
            song (java.io.File. dir "song.gl")
            inst (java.io.File. dir "parts/instruments.gl")]
        (spit (.getPath inst) "(def click\n  (d :click :src :click :gate (p [1 0]) :dur 0.02 :amp 0.2))\n")
        (spit (.getPath song) "(include \"parts/instruments.gl\")\n(scene :intro :loop true click)\n(play-scene :intro)\n")
        (swap! glitchlisp.swing.shared/state assoc :file song)
        (let [compiled (glitchlisp.swing.render/compile-glitchlisp-source (slurp (.getPath song)))]
          (println (clojure.string/includes? compiled "(d :click"))
          (println (not (clojure.string/includes? compiled "(include"))))
        (doseq [file [inst song]] (.delete file))
        (.delete (java.io.File. dir "parts"))
        (.delete dir))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("run Swing include compile smoke");
    assert!(
        output.status.success(),
        "Swing include compile smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = stdout
        .lines()
        .filter(|line| *line == "true" || *line == "false")
        .collect::<Vec<_>>();
    let results = results.into_iter().rev().take(2).collect::<Vec<_>>();
    assert_eq!(
        results,
        vec!["true", "true"],
        "Swing compile should expand includes: {}",
        stdout
    );
}

#[test]
fn run_sh_raw_clojure_fallback_reports_headless_display_error() {
    let dir = std::env::temp_dir().join(format!("mescript-run-sh-headless-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create run.sh headless temp dir");
    let script = dir.join("run.sh");
    fs::copy("run.sh", &script).expect("copy run.sh into temp dir");

    let output = Command::new("bash")
        .arg(&script)
        .env("JAVA_TOOL_OPTIONS", "-Djava.awt.headless=true")
        .output()
        .expect("run copied run.sh in headless mode");
    let _ = fs::remove_dir_all(&dir);

    assert!(
        !output.status.success(),
        "headless run.sh fallback should fail instead of silently exiting"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error: unable to open the Swing display"),
        "expected concise display error, got: {}",
        stderr
    );
    assert!(
        stderr.contains("glitchlisp-native edit"),
        "expected terminal editor hint, got: {}",
        stderr
    );
    assert!(
        !stderr.contains("HeadlessException"),
        "run.sh should not dump a Swing stack trace: {}",
        stderr
    );
}

#[test]
fn compiler_helper_insert_snippets_stay_compatible() {
    let script = r#"
      (load-file "src/main.clj")
      (let [pane (javax.swing.JTextPane.)]
        (doseq [category ["Math / Logic" "Pattern"]
                option (glitchlisp-swing/insert-form-options category)]
          (println (str category "\t"
                        option "\t"
                        (pr-str (glitchlisp-swing/insert-form-snippet pane category option "intro" false))))))
    "#;
    let output = Command::new("clojure")
        .env("GLITCHLISP_NO_GUI", "1")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("extract compiler helper insert snippets");
    assert!(
        output.status.success(),
        "compiler helper snippet extraction failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut math_count = 0;
    let mut pattern_count = 0;
    for line in stdout.lines() {
        let parts = line.splitn(3, '\t').collect::<Vec<_>>();
        if parts.len() != 3 {
            continue;
        }
        let category = parts[0];
        let option = parts[1];
        let snippet = unescape_clojure_pr_str(parts[2]);
        if category == "Math / Logic" {
            math_count += 1;
            compile_source_for_runtime(&snippet).unwrap_or_else(|err| {
                panic!("Math / Logic -> {} did not compile: {}", option, err)
            });
        } else if category == "Pattern" {
            pattern_count += 1;
            let source = format!(
                "(d :snippet :src :sine-synth :note c3 :gate {})\n(start!)",
                snippet
            );
            let compiled = compile_source_for_runtime(&source)
                .unwrap_or_else(|err| panic!("Pattern -> {} did not compile: {}", option, err));
            let mut runtime = Runtime::new();
            eval_program(&mut runtime, &compiled)
                .unwrap_or_else(|err| panic!("Pattern -> {} did not evaluate: {}", option, err));
            assert!(
                runtime.running,
                "Pattern -> {} did not start runtime",
                option
            );
        }
    }

    assert!(
        math_count >= 20,
        "expected Math / Logic snippets, got {}",
        math_count
    );
    assert!(
        pattern_count >= 6,
        "expected Pattern snippets, got {}",
        pattern_count
    );
}

#[test]
fn effect_catalog_snippets_parse_as_inserted_source() {
    let forms = catalog_effect_forms();
    assert!(forms.len() > 80, "expected effect catalog forms");

    for (label, form) in forms {
        let source = if form.trim_start().starts_with(":fx") {
            format!(
                "(d :catalog :src :sine-synth :note c3 :gate 1 {})\n(start!)",
                form
            )
        } else if offline_catalog_effect(&label) {
            format!(
                "(d :catalog :src :sine-synth :note c3 :gate 1)\n(post-fx [{}])\n(start!)",
                form
            )
        } else {
            format!(
                "(d :catalog :src :sine-synth :note c3 :gate 1 :fx [{}])\n(start!)",
                form
            )
        };
        let mut runtime = Runtime::new();
        eval_program(&mut runtime, &source)
            .unwrap_or_else(|err| panic!("catalog effect '{}' failed: {}", label, err));
    }
}

#[test]
fn control_forms_reject_extra_arguments() {
    let mut runtime = Runtime::new();
    let err = eval_program(&mut runtime, "(start! :unexpected)").unwrap_err();
    assert!(err.contains("start! expects no arguments"), "{}", err);

    let mut runtime = Runtime::new();
    let err = eval_program(&mut runtime, "(start!)").unwrap_err();
    assert!(
        err.contains("start! requires at least one top-level track"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(scene :intro :loop true
           (d :lead :src :sine-synth :note c3 :gate 1))
         (start!)",
    )
    .unwrap_err();
    assert!(
        err.contains("start! only starts top-level tracks; use (play-scene :scene-name)"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(&mut runtime, "(play-note c3 :extra)").unwrap_err();
    assert!(
        err.contains("play-note expects exactly 1 argument"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    eval_program(&mut runtime, "(d :a :src :click :gate 1)").unwrap();
    let err = eval_program(&mut runtime, "(clear :a :b)").unwrap_err();
    assert!(err.contains("clear expects exactly 1 argument"), "{}", err);

    let err = eval_program(&mut runtime, "(mute :a :b)").unwrap_err();
    assert!(err.contains("mute expects exactly 1 argument"), "{}", err);

    eval_program(&mut runtime, "(mute :a)").unwrap();
    assert!(runtime.tracks["a"].muted);

    let err = eval_program(&mut runtime, "(clear :missing)").unwrap_err();
    assert!(err.contains("unknown track ':missing'"), "{}", err);

    eval_program(&mut runtime, "(start!) (clear-all)").unwrap();
    assert!(
        !runtime.running,
        "clear-all should stop playback after removing every playable target"
    );
}

#[test]
fn bpm_rejects_out_of_range_values_instead_of_clamping() {
    let mut runtime = Runtime::new();
    let err = eval_program(&mut runtime, "(bpm 0)").unwrap_err();
    assert!(
        err.contains("bpm must be between 20 and 320, got 0"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(&mut runtime, "(bpm 1000)").unwrap_err();
    assert!(
        err.contains("bpm must be between 20 and 320, got 1000"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    eval_program(&mut runtime, "(bpm 320)").unwrap();
    assert_eq!(runtime.bpm, 320.0);
}

#[test]
fn effect_insert_parameter_comments_match_runtime_ranges() {
    let script = r#"
      (load-file "src/glitchlisp/swing/shared.clj")
      (load-file "src/glitchlisp/swing/catalog.clj")
      (doseq [[effect param] [["bitcrush" ":bits"]
                              ["bitcrush" ":bit-depth"]
                              ["bitcrush" ":rate"]
                              ["bitcrush" ":sample-rate-reduction"]
                              ["delay" ":feedback"]
                              ["tape-stop" ":duration"]
                              ["tape-stop" ":duration-pct"]
                              ["crystal" ":decay"]
                              ["wavefolder" ":folds"]
                              ["wavefolder" ":gain"]
                              ["wavefolder" ":symmetry"]
                              ["fold" ":gain"]
                              ["resonator" ":freq"]
                              ["resonator" ":decay"]
                              ["resonator" ":harmonics"]
                              ["chorus" ":depth"]
                              ["chorus" ":voices"]
                              ["ensemble" ":depth"]
                              ["ensemble" ":voices"]
                              ["ce1-chorus" ":rate"]
                              ["re301-chorus" ":rate"]
                              ["phaser" ":stages"]
                              ["dimension" ":mode"]
                              ["flanger" ":depth"]
                              ["vibrato" ":depth"]
                              ["tremolo" ":rate"]
                              ["ring-mod" ":freq"]
                              ["arp-ring-mod" ":diode-curve"]
                              ["fairchild" ":time-constant"]
                              ["1176" ":attack"]
                              ["transient" ":attack-gain"]
                              ["spring-reverb" ":decay"]
                              ["emt-plate" ":decay"]
                              ["lexicon-224" ":size"]
                              ["ams-reverb" ":decay"]
                              ["studer-tape" ":speed"]
                              ["moog" ":drive"]
                              ["tb-303" ":env-mod"]
                              ["neve-preamp" ":gain"]
                              ["pultec-eq" ":low-boost"]
                              ["space-echo" ":time"]
                              ["tc2290" ":time-ms"]
                              ["tc2290" ":mod-depth"]
                              ["stutter" ":grain-ms"]
                              ["stutter" ":repeats"]
                              ["glitch" ":slice-ms"]
                              ["fade" ":duration"]
                              ["adsr" ":sustain"]
                              ["asdr" ":duration"]
                              ["doppler" ":speed"]]]
        (println effect param (glitchlisp.swing.catalog/effect-param-contract effect param)))
    "#;
    let output = Command::new("clojure")
        .arg("-J-Djava.awt.headless=true")
        .arg("-e")
        .arg(script)
        .output()
        .expect("inspect effect insert parameter comments");
    assert!(
        output.status.success(),
        "effect parameter comment inspection failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    for expected in [
        "bitcrush :bits type: integer; range: 2..16",
        "bitcrush :bit-depth type: integer; range: 2..16",
        "bitcrush :rate type: integer; range: 1..128",
        "bitcrush :sample-rate-reduction type: integer; range: 1..128",
        "delay :feedback type: number; range: 0..0.95",
        "tape-stop :duration type: number; range: 0.1..1",
        "tape-stop :duration-pct type: number; range: 0.1..1",
        "crystal :decay type: number; range: 0..0.95",
        "wavefolder :folds type: number; range: 1..8",
        "wavefolder :gain type: number; range: 0.1..12",
        "wavefolder :symmetry type: number; range: 0.1..2",
        "fold :gain type: number; range: 0.1..12",
        "resonator :freq type: number Hz; range: >=20",
        "resonator :decay type: number; range: 0..1",
        "resonator :harmonics type: number; range: 1..16",
        "chorus :depth type: number seconds; range: 0.0001..0.05",
        "chorus :voices type: integer; range: 1..8",
        "ensemble :depth type: number seconds; range: 0.0005..0.05",
        "ensemble :voices type: integer; range: 2..12",
        "ce1-chorus :rate type: number Hz; range: 0.01..10",
        "re301-chorus :rate type: number Hz; range: 0.01..10",
        "phaser :stages type: integer; range: 1..12",
        "dimension :mode type: integer; range: 1..4",
        "flanger :depth type: number seconds; range: 0.0001..0.02",
        "vibrato :depth type: number seconds; range: 0.0001..0.03",
        "tremolo :rate type: number Hz; range: 0.01..40",
        "ring-mod :freq type: number Hz; range: 0.01..20000",
        "arp-ring-mod :diode-curve type: number; range: 0..1",
        "fairchild :time-constant type: number; range: 1..6",
        "1176 :attack type: number; range: 0..1",
        "transient :attack-gain type: number; range: 0..8",
        "spring-reverb :decay type: number; range: 0..4",
        "emt-plate :decay type: number; range: 0.1..5",
        "lexicon-224 :size type: number; range: 0.2..2",
        "ams-reverb :decay type: number; range: 0.1..5",
        "studer-tape :speed type: number; range: 0..2",
        "moog :drive type: number; range: 0..1",
        "tb-303 :env-mod type: number; range: 0..1",
        "neve-preamp :gain type: number; range: 0..1",
        "pultec-eq :low-boost type: number; range: 0..1",
        "space-echo :time type: number seconds; range: 0.02..2",
        "tc2290 :time-ms type: number ms; range: 1..2000",
        "tc2290 :mod-depth type: number seconds; range: 0..0.05",
        "stutter :grain-ms type: number ms; range: 1..500",
        "stutter :repeats type: integer; range: 1..16",
        "glitch :slice-ms type: number ms; range: 1..500",
        "fade :duration type: number seconds; range: >=0.001",
        "adsr :sustain type: number; range: 0..1",
        "asdr :duration type: number seconds; range: >=0.001",
        "doppler :speed type: number; range: 0.01..8",
    ] {
        assert!(
            stdout.lines().any(|line| line == expected),
            "missing expected parameter comment '{}': {}",
            expected,
            stdout
        );
    }
}

#[test]
fn unknown_effect_parameters_error_instead_of_being_ignored() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(delay :mixx 1)])
         (start!)",
    )
    .unwrap_err();
    assert!(err.contains("unknown delay parameter ':mixx'"), "{}", err);

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate 1)
         (post-fx [(reverse :mixx 1)])
         (start!)",
    )
    .unwrap_err();
    assert!(err.contains("unknown reverse parameter ':mixx'"), "{}", err);
}

#[test]
fn effect_parameters_are_validated_per_effect_not_globally() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(delay :cutoff 100)])
         (start!)",
    )
    .unwrap_err();
    assert!(err.contains("unknown delay parameter ':cutoff'"), "{}", err);

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate 1)
         (post-fx [(reverse :cutoff 100)])
         (start!)",
    )
    .unwrap_err();
    assert!(
        err.contains("unknown reverse parameter ':cutoff'"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(delay :time 0.25 :feedback 0.2 :mix 0.3)])
         (post-fx [(reverse :mix 0.5)])
         (start!)",
    )
    .unwrap();
}

#[test]
fn h3000_rejects_unimplemented_delay_and_feedback_parameters_instead_of_ignoring_them() {
    for (source, message) in [
        (
            "(d :lead
                :src :sine-synth
                :note c3
                :gate 1
                :fx [(h3000 :delay-ms 18)])
             (start!)",
            "h3000 :delay-ms is not implemented by this port yet; remove it",
        ),
        (
            "(d :lead
                :src :sine-synth
                :note c3
                :gate 1
                :fx [(h3000 :feedback 0.2)])
             (start!)",
            "h3000 :feedback is not implemented by this port yet; remove it",
        ),
    ] {
        let mut runtime = Runtime::new();
        let err = eval_program(&mut runtime, source).unwrap_err();
        assert!(err.contains(message), "{}", err);
    }

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(h3000 :detune-cents 9 :mix 0.3)
                 (h3000 :delay-ms null :feedback null :mix null)])
         (start!)",
    )
    .unwrap();
}

#[test]
fn effect_mix_rejects_out_of_range_values_instead_of_clamping() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(delay :time 0.25 :mix 3)])
         (start!)",
    )
    .unwrap_err();
    assert!(
        err.contains("delay :mix mix must be between 0 and 1, got 3"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate 1)
         (post-fx [(reverse :mix -1)])
         (start!)",
    )
    .unwrap_err();
    assert!(
        err.contains("reverse :mix mix must be between 0 and 1, got -1"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(delay :mix 0) (reverb :mix 1)])
         (post-fx [(reverse :mix null)])
         (start!)",
    )
    .unwrap();
}

#[test]
fn effect_feedback_rejects_out_of_range_values_instead_of_clamping() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(delay :time 0.25 :feedback 1)])
         (start!)",
    )
    .unwrap_err();
    assert!(
        err.contains("delay :feedback feedback must be between 0 and 0.95, got 1"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate 1)
         (post-fx [(ping-pong-delay :time 0.3 :feedback -0.1)])
         (start!)",
    )
    .unwrap_err();
    assert!(
        err.contains("ping-pong-delay :feedback feedback must be between 0 and 0.95, got -0.1"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(delay :feedback 0) (flanger :feedback 0.95)])
         (post-fx [(ping-pong-delay :time 0.3 :feedback null)])
         (start!)",
    )
    .unwrap();
}

#[test]
fn effect_resonance_rejects_out_of_range_values_instead_of_clamping() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(filter :type :lowpass :cutoff 1200 :res 2)])
         (start!)",
    )
    .unwrap_err();
    assert!(
        err.contains("filter :res resonance must be between 0 and 1, got 2"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(moog :cutoff 1200 :resonance -0.1)])
         (start!)",
    )
    .unwrap_err();
    assert!(
        err.contains("moog :resonance resonance must be between 0 and 1, got -0.1"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(filter :res 0)
                 (tb-303 :resonance 1)
                 (buchla-lpg :res null)])
         (start!)",
    )
    .unwrap();
}

#[test]
fn bitcrush_parameters_reject_out_of_range_values_instead_of_clamping() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(bitcrush :bits 1)])
         (start!)",
    )
    .unwrap_err();
    assert!(
        err.contains("bitcrush :bits bits must be between 2 and 16, got 1"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(bitcrush :sample-rate-reduction 129)])
         (start!)",
    )
    .unwrap_err();
    assert!(
        err.contains("bitcrush :sample-rate-reduction rate must be between 1 and 128, got 129"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(bitcrush :bits 2 :rate 1)
                 (bitcrush :bit-depth 16 :sample-rate-reduction 128)
                 (bitcrush :bits null :rate null)])
         (start!)",
    )
    .unwrap();
}

#[test]
fn post_fx_normalized_parameters_reject_out_of_range_values_instead_of_clamping() {
    for (source, message) in [
        (
            "(d :lead :src :sine-synth :note c3 :gate 1)
             (post-fx [(granular :density -0.1)])
             (start!)",
            "granular :density density must be between 0 and 1, got -0.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1)
             (post-fx [(granular :spray 1.2)])
             (start!)",
            "granular :spray spray must be between 0 and 1, got 1.2",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1)
             (post-fx [(granular :pitch-spread 2)])
             (start!)",
            "granular :pitch-spread pitch-spread must be between 0 and 1, got 2",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1)
             (post-fx [(spectral-freeze :freeze-pos -0.1)])
             (start!)",
            "spectral-freeze :freeze-pos freeze-pos must be between 0 and 1, got -0.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1)
             (post-fx [(spectral-freeze :sustain 1.1)])
             (start!)",
            "spectral-freeze :sustain sustain must be between 0 and 1, got 1.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1)
             (post-fx [(autopan :depth -0.1)])
             (start!)",
            "autopan :depth depth must be between 0 and 1, got -0.1",
        ),
    ] {
        let mut runtime = Runtime::new();
        let err = eval_program(&mut runtime, source).unwrap_err();
        assert!(err.contains(message), "{}", err);
    }

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate 1)
         (post-fx [(granular :density 0 :spray 1 :pitch-spread null)
                   (spectral-freeze :freeze-pos 1 :sustain 0)
                   (auto-pan :depth null)])
         (start!)",
    )
    .unwrap();
}

#[test]
fn tape_stop_duration_rejects_out_of_range_values_instead_of_clamping() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate 1)
         (post-fx [(tape-stop :duration-pct 0.05)])
         (start!)",
    )
    .unwrap_err();
    assert!(
        err.contains("tape-stop :duration-pct duration must be between 0.1 and 1, got 0.05"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate 1)
         (post-fx [(tape-stop :duration 1.5)])
         (start!)",
    )
    .unwrap_err();
    assert!(
        err.contains("tape-stop :duration duration must be between 0.1 and 1, got 1.5"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate 1)
         (post-fx [(tape-stop :duration-pct 0.1)
                   (tape-stop :duration 1)
                   (tape-stop :duration null)])
         (start!)",
    )
    .unwrap();
}

#[test]
fn creative_normalized_parameters_reject_out_of_range_values_instead_of_clamping() {
    for (source, message) in [
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(lofi :amount 1.1)])
             (start!)",
            "lofi :amount amount must be between 0 and 1, got 1.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(vinyl :wow -0.1)])
             (start!)",
            "vinyl :wow wow must be between 0 and 1, got -0.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(sidechain :shape 2)])
             (start!)",
            "sidechain :shape shape must be between 0 and 1, got 2",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(radio :intensity -0.1)])
             (start!)",
            "radio :intensity intensity must be between 0 and 1, got -0.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(telephone :quality 1.5)])
             (start!)",
            "telephone :quality quality must be between 0 and 1, got 1.5",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(underwater :depth-amount -0.1)])
             (start!)",
            "underwater :depth-amount depth must be between 0 and 1, got -0.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(crystal :brightness 1.1)])
             (start!)",
            "crystal :brightness brightness must be between 0 and 1, got 1.1",
        ),
    ] {
        let mut runtime = Runtime::new();
        let err = eval_program(&mut runtime, source).unwrap_err();
        assert!(err.contains(message), "{}", err);
    }

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(lofi :intensity 0)
                 (vinyl :crackle 0 :hiss 1 :wow null)
                 (sidechain :depth 1 :shape 0)
                 (radio :intensity null)
                 (telephone :quality 1)
                 (underwater :depth 0)
                 (crystal :brightness 1)])
         (start!)",
    )
    .unwrap();
}

#[test]
fn modulation_count_and_mode_parameters_reject_out_of_range_values_instead_of_clamping() {
    for (source, message) in [
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(chorus :voices 0)])
             (start!)",
            "chorus :voices voices must be between 1 and 8, got 0",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(ensemble :voices 13)])
             (start!)",
            "ensemble :voices voices must be between 2 and 12, got 13",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(phaser :stages 13)])
             (start!)",
            "phaser :stages stages must be between 1 and 12, got 13",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(dimension :mode 0)])
             (start!)",
            "dimension :mode mode must be between 1 and 4, got 0",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(dimension-d :mode 5)])
             (start!)",
            "dimension-d :mode mode must be between 1 and 4, got 5",
        ),
    ] {
        let mut runtime = Runtime::new();
        let err = eval_program(&mut runtime, source).unwrap_err();
        assert!(err.contains(message), "{}", err);
    }

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(chorus :voices 1)
                 (chorus :voices 8)
                 (ensemble :voices 2)
                 (ensemble :voices 12)
                 (phaser :stages 1)
                 (phaser :stages 12)
                 (dimension :mode 1)
                 (dimension-d :mode 4)
                 (chorus :voices null)])
         (start!)",
    )
    .unwrap();
}

#[test]
fn modulation_rate_and_depth_parameters_reject_out_of_range_values_instead_of_clamping() {
    for (source, message) in [
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(chorus :depth 0)])
             (start!)",
            "chorus :depth depth must be between 0.0001 and 0.05, got 0",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(ensemble :rate 0)])
             (start!)",
            "ensemble :rate rate must be at least 0.01, got 0",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(ce1-chorus :intensity 1.1)])
             (start!)",
            "ce1-chorus :intensity intensity must be between 0 and 1, got 1.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(re301-chorus :tone -0.1)])
             (start!)",
            "re301-chorus :tone tone must be between 0 and 1, got -0.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(flanger :rate 0)])
             (start!)",
            "flanger :rate rate must be between 0.01 and 20, got 0",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(phaser :depth 1.1)])
             (start!)",
            "phaser :depth depth must be between 0 and 1, got 1.1",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(small-stone :rate 21)])
             (start!)",
            "small-stone :rate rate must be between 0.01 and 20, got 21",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(vibrato :depth 0)])
             (start!)",
            "vibrato :depth depth must be between 0.0001 and 0.03, got 0",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(tremolo :rate 41)])
             (start!)",
            "tremolo :rate rate must be between 0.01 and 40, got 41",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(ring-mod :freq 0)])
             (start!)",
            "ring-mod :freq freq must be between 0.01 and 20000, got 0",
        ),
        (
            "(d :lead :src :sine-synth :note c3 :gate 1 :fx [(arp-ring-mod :mix 1.1)])
             (start!)",
            "arp-ring-mod :mix mix must be between 0 and 1, got 1.1",
        ),
    ] {
        let mut runtime = Runtime::new();
        let err = eval_program(&mut runtime, source).unwrap_err();
        assert!(err.contains(message), "{}", err);
    }

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(chorus :rate 0.01 :depth 0.0001 :voices null :mix null)
                 (chorus :rate null :depth 0.05)
                 (ensemble :rate 0.01 :depth 0.0005 :voices null)
                 (ensemble :rate null :depth 0.05)
                 (ce1-chorus :rate 0.01 :intensity 0)
                 (ce-1 :rate 10 :intensity 1)
                 (re301-chorus :rate 0.01 :depth 0 :tone 1)
                 (re-301-chorus :rate 10 :depth 1 :tone null)
                 (flanger :rate 0.01 :depth 0.0001)
                 (flanger :rate 20 :depth 0.02)
                 (phaser :rate 0.01 :depth 0 :stages null)
                 (phaser :rate 20 :depth 1)
                 (small-stone :rate 0.01 :depth 0 :color null)
                 (small-stone :rate 20 :depth 1)
                 (vibrato :rate 0.01 :depth 0.0001)
                 (vibrato :rate null :depth 0.03)
                 (tremolo :rate 0.01 :depth 0)
                 (tremolo :rate 40 :depth 1)
                 (ring-mod :freq 0.01)
                 (ringmod :freq 20000 :mix null)
                 (arp-ring-mod :freq 0.01 :depth 0 :diode-curve 1)
                 (arp-ring-mod :freq 20000 :mix 1 :diode-curve null)])
         (start!)",
    )
    .unwrap();
}

#[test]
fn duplicate_keyword_parameters_error_instead_of_choosing_one() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate (p [1 0])
            :gate (p [0 1]))
         (start!)",
    )
    .unwrap_err();
    assert!(err.contains("duplicate track parameter ':gate'"), "{}", err);

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(scene :intro :repeat 2 :repeat 4
           (d :lead :src :sine-synth :note c3 :gate 1))",
    )
    .unwrap_err();
    assert!(err.contains("duplicate scene option ':repeat'"), "{}", err);

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(delay :mix 0.1 :mix 0.9)])
         (start!)",
    )
    .unwrap_err();
    assert!(err.contains("duplicate delay parameter ':mix'"), "{}", err);

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate 1)
         (post-fx [(reverse :mix 0.1 :mix 0.9)])
         (start!)",
    )
    .unwrap_err();
    assert!(
        err.contains("duplicate reverse parameter ':mix'"),
        "{}",
        err
    );
}

#[test]
fn duplicate_alias_parameters_error_instead_of_choosing_one() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(scene :intro :repeat 2 :times 4
           (d :lead :src :sine-synth :note c3 :gate 1))",
    )
    .unwrap_err();
    assert!(err.contains("duplicate scene option ':times'"), "{}", err);

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :pulse
            :note c3
            :gate 1
            :pulse-width 0.25
            :pw 0.75)
         (start!)",
    )
    .unwrap_err();
    assert!(err.contains("duplicate track parameter ':pw'"), "{}", err);

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(filter :res 0.2 :resonance 0.9)])
         (start!)",
    )
    .unwrap_err();
    assert!(
        err.contains("duplicate filter parameter ':resonance'"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate 1)
         (post-fx [(tape-stop :duration 0.4 :duration-pct 0.8)])
         (start!)",
    )
    .unwrap_err();
    assert!(
        err.contains("duplicate tape-stop parameter ':duration-pct'"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(scene :intro :times 1
           (d :lead
              :src :pulse
              :note c3
              :gate 1
              :pw 0.5
              :fx [(filter :res 0.2)]))
         (play-scene :intro)",
    )
    .unwrap();
}

#[test]
fn gated_effect_wrapper_rejects_duplicate_gate_or_effect_forms() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(on :gate (p [1])
                     (delay :mix 0.1)
                     (reverb :mix 0.2))])
         (start!)",
    )
    .unwrap_err();
    assert!(
        err.contains("on expects exactly one effect form"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(on :gate (p [1])
                     :gate (p [0])
                     (delay :mix 0.1))])
         (start!)",
    )
    .unwrap_err();
    assert!(err.contains("on expects only one :gate pattern"), "{}", err);

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(on :gate (p [1])
                     (delay :mix 0.1))])
         (start!)",
    )
    .unwrap();
}

#[test]
fn pattern_wrappers_reject_extra_arguments() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note (p [c3] [d3]) :gate 1)",
    )
    .unwrap_err();
    assert!(err.contains("p expects one vector"), "{}", err);

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate 1 :amp (g [0.1] [0.2]))",
    )
    .unwrap_err();
    assert!(err.contains("g expects one vector"), "{}", err);

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate (euclid 4 16 99))",
    )
    .unwrap_err();
    assert!(err.contains("euclid expects pulses and steps"), "{}", err);

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate (euclid 4.5 16))",
    )
    .unwrap_err();
    assert!(
        err.contains("euclid pulses must be a non-negative integer"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate (euclid 4 0))",
    )
    .unwrap_err();
    assert!(
        err.contains("euclid steps must be greater than zero"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate (euclid-rot 4 16 -1))",
    )
    .unwrap_err();
    assert!(
        err.contains("euclid-rot rotation must be a non-negative integer"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate (reverse (p [1 0]) (p [0 1])))",
    )
    .unwrap_err();
    assert!(err.contains("reverse expects one pattern"), "{}", err);

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate (times 2 (p [1 0 0 0] then [1 1 1 1])))",
    )
    .unwrap_err();
    assert!(
        err.contains("use (p (then A B)) instead of (p A then B)"),
        "{}",
        err
    );

    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate (p (then
                      (times 0 [1 0])
                      [1 1])))",
    )
    .unwrap_err();
    assert!(err.contains("times must be greater than zero"), "{}", err);

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note (p [c3 d3])
            :gate (euclid 4 16)
            :amp (g [0.1 0.2]))",
    )
    .unwrap();
}

#[test]
fn effect_parameters_require_values() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(delay :mix)])
         (start!)",
    )
    .unwrap_err();
    assert!(err.contains("delay :mix requires a value"), "{}", err);
}

#[test]
fn keyword_effect_parameters_reject_non_keyword_values() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(filter :type 123)])
         (start!)",
    )
    .unwrap_err();

    assert!(err.contains(":type expected keyword or symbol"), "{}", err);
}

#[test]
fn boolean_effect_parameters_reject_unknown_values() {
    let mut runtime = Runtime::new();
    let err = eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(small-stone :color :maybe)])
         (start!)",
    )
    .unwrap_err();
    assert!(err.contains(":color must be true or false"), "{}", err);

    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead
            :src :sine-synth
            :note c3
            :gate 1
            :fx [(small-stone :color :off)])
         (start!)",
    )
    .unwrap();
}

#[test]
fn tape_stop_accepts_insert_menu_duration_alias() {
    let mut runtime = Runtime::new();
    eval_program(
        &mut runtime,
        "(d :lead :src :sine-synth :note c3 :gate 1)
         (post-fx [(tape-stop :duration 0.75)])
         (start!)",
    )
    .unwrap();

    assert!(runtime.running);
}
