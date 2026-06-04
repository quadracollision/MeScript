use crate::effects::offline::{OfflineEffectSpec, StereoSide};
use crate::effects::{self, DistortionKind, EffectSpec, FilterKind};
use crate::model::{
    NoteMode, OscillatorParams, ParamPatterns, Runtime, Scene, SceneState, Track, TrackEffect,
    Waveform,
};
use crate::sequencer;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
enum Expr {
    List(Vec<Expr>),
    Vector(Vec<Expr>),
    Symbol(String),
    Keyword(String),
    Number(f32),
    String(String),
}

fn tokenize(source: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = source.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            ';' => {
                for next in chars.by_ref() {
                    if next == '\n' {
                        break;
                    }
                }
            }
            '(' | ')' | '[' | ']' => tokens.push(ch.to_string()),
            '"' => {
                let mut value = String::new();
                for next in chars.by_ref() {
                    if next == '"' {
                        break;
                    }
                    value.push(next);
                }
                tokens.push(format!("\"{}\"", value));
            }
            ch if ch.is_whitespace() => {}
            _ => {
                let mut atom = ch.to_string();
                while let Some(next) = chars.peek() {
                    if next.is_whitespace() || matches!(next, '(' | ')' | '[' | ']') {
                        break;
                    }
                    atom.push(*next);
                    chars.next();
                }
                tokens.push(atom);
            }
        }
    }
    tokens
}

fn parse_program(source: &str) -> Result<Vec<Expr>, String> {
    let tokens = tokenize(source);
    let mut index = 0;
    let mut forms = Vec::new();
    while index < tokens.len() {
        forms.push(parse_expr(&tokens, &mut index)?);
    }
    Ok(forms)
}

fn parse_expr(tokens: &[String], index: &mut usize) -> Result<Expr, String> {
    let Some(token) = tokens.get(*index) else {
        return Err("unexpected end of input".to_string());
    };
    *index += 1;
    match token.as_str() {
        "(" => {
            let mut values = Vec::new();
            while tokens.get(*index).map(String::as_str) != Some(")") {
                if *index >= tokens.len() {
                    return Err("missing ')'".to_string());
                }
                values.push(parse_expr(tokens, index)?);
            }
            *index += 1;
            Ok(Expr::List(values))
        }
        "[" => {
            let mut values = Vec::new();
            while tokens.get(*index).map(String::as_str) != Some("]") {
                if *index >= tokens.len() {
                    return Err("missing ']'".to_string());
                }
                values.push(parse_expr(tokens, index)?);
            }
            *index += 1;
            Ok(Expr::Vector(values))
        }
        ")" | "]" => Err(format!("unexpected '{}'", token)),
        _ if token.starts_with('"') && token.ends_with('"') => {
            Ok(Expr::String(token.trim_matches('"').to_string()))
        }
        _ if token.starts_with(':') => Ok(Expr::Keyword(token.trim_start_matches(':').to_string())),
        _ => {
            if let Ok(number) = token.parse::<f32>() {
                Ok(Expr::Number(number))
            } else {
                Ok(Expr::Symbol(token.to_string()))
            }
        }
    }
}

pub(crate) fn eval_program(runtime: &mut Runtime, source: &str) -> Result<(), String> {
    for form in parse_program(source)? {
        eval_form(runtime, &form)?;
    }
    Ok(())
}

pub(crate) fn load_runtime(path: &str) -> Result<Runtime, String> {
    let source = fs::read_to_string(path).map_err(|error| format!("{}: {}", path, error))?;
    let mut runtime = Runtime::new();
    eval_program(&mut runtime, &source)?;
    Ok(runtime)
}

fn eval_form(runtime: &mut Runtime, expr: &Expr) -> Result<(), String> {
    let Expr::List(items) = expr else {
        return Err("top-level value must be a form".to_string());
    };
    let Some(Expr::Symbol(name)) = items.first() else {
        return Err("form must start with a symbol".to_string());
    };

    match name.as_str() {
        "bpm" => {
            runtime.bpm = number_arg(items, 1, "bpm")?.clamp(20.0, 320.0);
            Ok(())
        }
        "start!" => {
            runtime.running = true;
            Ok(())
        }
        "stop!" => {
            runtime.running = false;
            runtime.scene_state = None;
            Ok(())
        }
        "block" | "scene" => define_scene(runtime, items),
        "play-block" | "play-scene" | "cue" => play_scene(runtime, items),
        "play-note" => {
            let freq = number_arg(items, 1, "play-note")?;
            runtime.tracks.insert(
                "tone".to_string(),
                Track {
                    id: "tone".to_string(),
                    waveform: Waveform::Sine,
                    oscillator: OscillatorParams::default(),
                    notes: vec![freq],
                    note_mode: NoteMode::Step,
                    gates: vec![true, false, false, false],
                    gate_subdivisions: vec![vec![true], vec![false], vec![false], vec![false]],
                    gate_holds: vec![vec![0], vec![0], vec![0], vec![0]],
                    step_every: 1,
                    step_offset: 0,
                    amp: 0.35,
                    dur_seconds: 0.2,
                    param_patterns: ParamPatterns::default(),
                    effects: Vec::new(),
                    sample_data: Vec::new(),
                    muted: false,
                    solo: false,
                },
            );
            runtime.running = true;
            Ok(())
        }
        "post-fx" | "master-fx" => {
            runtime.post_effects =
                offline_effect_chain(items.get(1).ok_or("post-fx requires an effect vector")?)?;
            Ok(())
        }
        "d" => define_track(runtime, items),
        "clear" => {
            let id = keyword_arg(items, 1, "clear")?;
            runtime.tracks.remove(&id);
            Ok(())
        }
        "clear-all" => {
            runtime.tracks.clear();
            runtime.post_effects.clear();
            runtime.scenes.clear();
            runtime.scene_state = None;
            Ok(())
        }
        "mute" => set_track_flag(runtime, items, "mute", true, false),
        "unmute" => set_track_flag(runtime, items, "unmute", false, false),
        "solo" => set_track_flag(runtime, items, "solo", true, true),
        "unsolo" => set_track_flag(runtime, items, "unsolo", false, true),
        other => Err(format!("unsupported form '{}'", other)),
    }
}

pub(crate) fn apply_scene(runtime: &mut Runtime, id: &str) -> Result<(), String> {
    let scene = runtime
        .scenes
        .get(id)
        .cloned()
        .ok_or_else(|| format!("unknown block ':{}'", id))?;
    runtime.tracks = scene.tracks;
    runtime.post_effects = scene.post_effects;
    runtime.scene_state = Some(SceneState {
        current: scene.id,
        cycle: 0,
    });
    runtime.running = true;
    Ok(())
}

fn define_scene(runtime: &mut Runtime, items: &[Expr]) -> Result<(), String> {
    let Expr::Keyword(id) = items.get(1).ok_or("block requires a block id")? else {
        return Err("block id must be a keyword".to_string());
    };

    let mut repeats = 0;
    let mut steps = 0;
    let mut explicit_steps = false;
    let mut steps_of = None;
    let mut next = None;
    let mut index = 2;
    while index < items.len() {
        let Expr::Keyword(key) = &items[index] else {
            break;
        };
        match key.as_str() {
            "repeat" | "repeats" | "times" => {
                let value = items
                    .get(index + 1)
                    .ok_or("block :repeat requires a value")?;
                repeats = usize_value(value, "repeat")?;
                index += 2;
            }
            "steps" | "length" | "bars" => {
                let value = items
                    .get(index + 1)
                    .ok_or("block :steps requires a value")?;
                steps = usize_value(value, "steps")?.max(1);
                explicit_steps = true;
                index += 2;
            }
            "steps-of" | "length-of" => {
                steps_of = Some(keyword_arg(items, index + 1, "block :steps-of")?);
                index += 2;
            }
            "next" => {
                next = Some(keyword_arg(items, index + 1, "block :next")?);
                index += 2;
            }
            _ => break,
        }
    }

    let mut block_runtime = Runtime::new();
    block_runtime.bpm = runtime.bpm;
    while index < items.len() {
        eval_form(&mut block_runtime, &items[index])?;
        index += 1;
    }

    if let Some(track_id) = steps_of {
        let track = block_runtime
            .tracks
            .get(&track_id)
            .ok_or_else(|| format!("block :steps-of references unknown track ':{}'", track_id))?;
        steps = inferred_track_steps(track);
    } else if !explicit_steps {
        steps = inferred_scene_steps(&block_runtime)?;
    }

    runtime.scenes.insert(
        id.clone(),
        Scene {
            id: id.clone(),
            steps,
            repeats,
            next,
            tracks: block_runtime.tracks,
            post_effects: block_runtime.post_effects,
        },
    );
    Ok(())
}

fn inferred_scene_steps(runtime: &Runtime) -> Result<usize, String> {
    let mut steps = runtime.tracks.values().map(inferred_track_steps);
    let Some(first) = steps.next() else {
        return Err("scene has nothing to play; add a track or set :steps explicitly".to_string());
    };
    Ok(steps.fold(first, lcm).max(1))
}

fn inferred_track_steps(track: &Track) -> usize {
    let gate_length = track.gate_subdivisions.len().max(1);
    let gate_hits = track
        .gate_subdivisions
        .iter()
        .filter(|step| step.iter().any(|gate| *gate))
        .count();
    let gate_slots = track
        .gate_subdivisions
        .iter()
        .map(|step| step.len().max(1))
        .sum::<usize>()
        .max(1);
    let note_length = track.notes.len().max(1);
    let note_period = match track.note_mode {
        NoteMode::Step => note_length,
        NoteMode::Hit => {
            if gate_hits == 0 {
                gate_length
            } else {
                gate_length * (note_length / gcd(note_length, gate_hits))
            }
        }
        NoteMode::Tick => gate_length * (note_length / gcd(note_length, gate_slots)),
    };
    track.step_every.max(1) * lcm(gate_length, note_period)
}

fn play_scene(runtime: &mut Runtime, items: &[Expr]) -> Result<(), String> {
    let id = keyword_arg(items, 1, "play-block")?;
    apply_scene(runtime, &id)
}

fn keyword_arg(items: &[Expr], index: usize, form: &str) -> Result<String, String> {
    match items.get(index) {
        Some(Expr::Keyword(value)) => Ok(value.clone()),
        _ => Err(format!("{} requires a keyword argument", form)),
    }
}

fn set_track_flag(
    runtime: &mut Runtime,
    items: &[Expr],
    form: &str,
    value: bool,
    solo: bool,
) -> Result<(), String> {
    let id = keyword_arg(items, 1, form)?;
    let track = runtime
        .tracks
        .get_mut(&id)
        .ok_or_else(|| format!("unknown track ':{}'", id))?;
    if solo {
        track.solo = value;
    } else {
        track.muted = value;
    }
    Ok(())
}

fn define_track(runtime: &mut Runtime, items: &[Expr]) -> Result<(), String> {
    let Expr::Keyword(id) = items.get(1).ok_or("d requires a track id")? else {
        return Err("d track id must be a keyword".to_string());
    };

    let mut track = Track {
        id: id.clone(),
        waveform: Waveform::Sine,
        oscillator: OscillatorParams::default(),
        notes: vec![note_freq("c3").unwrap()],
        note_mode: NoteMode::Step,
        gates: vec![true],
        gate_subdivisions: vec![vec![true]],
        gate_holds: vec![vec![0]],
        step_every: 1,
        step_offset: 0,
        amp: 0.2,
        dur_seconds: 0.12,
        param_patterns: ParamPatterns::default(),
        effects: Vec::new(),
        sample_data: Vec::new(),
        muted: false,
        solo: false,
    };
    if let Some(previous) = runtime.tracks.get(id) {
        track.muted = previous.muted;
        track.solo = previous.solo;
    }

    let mut index = 2;
    while index < items.len() {
        let Expr::Keyword(key) = &items[index] else {
            return Err("track parameters must be keyword/value pairs".to_string());
        };
        let value = items
            .get(index + 1)
            .ok_or("missing track parameter value")?;
        if null_value(value) {
            index += 2;
            continue;
        }
        match key.as_str() {
            "src" => track.waveform = waveform(value)?,
            "note" => {
                let (notes, mode) = note_pattern(value)?;
                track.notes = notes;
                track.note_mode = mode;
            }
            "gate" => {
                let (gate_subdivisions, gate_holds) = gate_pattern(value)?;
                track.gate_subdivisions = gate_subdivisions;
                track.gate_holds = gate_holds;
                track.gates = track
                    .gate_subdivisions
                    .iter()
                    .map(|step| step.iter().any(|gate| *gate))
                    .collect();
            }
            "detune" | "detune-cents" => {
                set_f32_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.detune_cents,
                    &mut track.oscillator.detune_cents,
                    |value| value,
                )?;
            }
            "phase" => {
                set_f32_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.phase,
                    &mut track.oscillator.phase,
                    |value| value.rem_euclid(1.0),
                )?;
            }
            "pulse-width" | "pulse_width" | "pw" => {
                set_f32_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.pulse_width,
                    &mut track.oscillator.pulse_width,
                    |value| value.clamp(0.01, 0.99),
                )?;
            }
            "morph" | "morph-pos" | "morph_pos" => {
                set_f32_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.morph_pos,
                    &mut track.oscillator.morph_pos,
                    |value| value.clamp(0.0, 1.0),
                )?;
            }
            "gain" => {
                set_f32_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.gain,
                    &mut track.oscillator.gain,
                    |value| value.clamp(0.0, 2.0),
                )?;
            }
            "unison" => {
                set_usize_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.unison,
                    &mut track.oscillator.unison,
                    |value| value.clamp(1, 10),
                    "unison",
                )?;
            }
            "unison-detune" | "unison_detune" => {
                set_f32_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.unison_detune,
                    &mut track.oscillator.unison_detune,
                    |value| value.clamp(0.0, 100.0),
                )?;
            }
            "unison-spread" | "unison_spread" | "spread" => {
                set_f32_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.unison_spread,
                    &mut track.oscillator.unison_spread,
                    |value| value.clamp(0.0, 1.0),
                )?;
            }
            "fm-ratio" | "fm_ratio" => {
                set_f32_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.fm_ratio,
                    &mut track.oscillator.fm_ratio,
                    |value| value.max(0.01),
                )?;
            }
            "fm-depth" | "fm_depth" => {
                set_f32_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.fm_depth,
                    &mut track.oscillator.fm_depth,
                    |value| value.clamp(0.0, 32.0),
                )?;
            }
            "harmonics" => track.oscillator.harmonics = harmonic_values(value)?,
            "sample" | "sample-path" | "sample_path" => {
                track.sample_data = load_sample_path(value)?;
                track.waveform = Waveform::Sample;
            }
            "sample-data" | "sample_data" => {
                track.sample_data = sample_values(value)?;
                track.waveform = Waveform::Sample;
            }
            "every" => track.step_every = usize_value(value, "every")?.max(1),
            "offset" => track.step_offset = usize_value(value, "offset")?,
            "amp" => {
                set_f32_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.amp,
                    &mut track.amp,
                    |value| value.clamp(0.0, 1.0),
                )?;
            }
            "dur" => {
                set_f32_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.dur_seconds,
                    &mut track.dur_seconds,
                    |value| value.clamp(0.005, 4.0),
                )?;
            }
            "fx" => track.effects = effect_chain(value)?,
            _ => {}
        }
        index += 2;
    }

    runtime.tracks.insert(id.clone(), track);
    Ok(())
}

fn number_arg(items: &[Expr], index: usize, form: &str) -> Result<f32, String> {
    let value = items
        .get(index)
        .ok_or_else(|| format!("{} requires argument {}", form, index))?;
    number_value(value)
}

fn number_value(expr: &Expr) -> Result<f32, String> {
    match expr {
        Expr::Number(value) => Ok(*value),
        Expr::Symbol(name) => note_freq(name).ok_or_else(|| format!("unknown symbol '{}'", name)),
        _ => Err("expected number or note".to_string()),
    }
}

fn null_value(expr: &Expr) -> bool {
    matches!(expr, Expr::Symbol(name) if matches!(name.as_str(), "nil" | "null"))
}

fn numeric_param_pattern(
    expr: &Expr,
    normalize: fn(f32) -> f32,
) -> Result<Option<Vec<f32>>, String> {
    match expr {
        Expr::List(items) if matches!(items.first(), Some(Expr::Symbol(name)) if name == "p") => {
            Ok(Some(
                number_pattern(expr, false)?
                    .into_iter()
                    .map(normalize)
                    .collect(),
            ))
        }
        Expr::Vector(_) => Ok(Some(
            number_pattern(expr, false)?
                .into_iter()
                .map(normalize)
                .collect(),
        )),
        _ => Ok(None),
    }
}

fn set_f32_param_pattern_or_scalar(
    expr: &Expr,
    pattern: &mut Option<Vec<f32>>,
    scalar: &mut f32,
    normalize: fn(f32) -> f32,
) -> Result<(), String> {
    if let Some(values) = numeric_param_pattern(expr, normalize)? {
        *pattern = Some(values);
    } else {
        *scalar = normalize(number_value(expr)?);
        *pattern = None;
    }
    Ok(())
}

fn set_usize_param_pattern_or_scalar(
    expr: &Expr,
    pattern: &mut Option<Vec<usize>>,
    scalar: &mut usize,
    normalize: fn(usize) -> usize,
    name: &str,
) -> Result<(), String> {
    if let Some(values) = numeric_param_pattern(expr, |value| value)? {
        *pattern = Some(
            values
                .into_iter()
                .map(|value| normalize(value.floor().max(0.0) as usize))
                .collect(),
        );
    } else {
        *scalar = normalize(usize_value(expr, name)?);
        *pattern = None;
    }
    Ok(())
}

fn usize_value(expr: &Expr, name: &str) -> Result<usize, String> {
    let value = numeric_only(expr)?;
    if value < 0.0 || value.fract() != 0.0 {
        return Err(format!("{} must be a non-negative integer", name));
    }
    Ok(value as usize)
}

fn harmonic_values(expr: &Expr) -> Result<[f32; 8], String> {
    let Expr::Vector(items) = expr else {
        return Err("harmonics must be a vector".to_string());
    };
    let mut harmonics = OscillatorParams::default().harmonics;
    for (idx, item) in items.iter().take(8).enumerate() {
        harmonics[idx] = number_value(item)?.clamp(0.0, 2.0);
    }
    Ok(harmonics)
}

fn sample_values(expr: &Expr) -> Result<Vec<f32>, String> {
    let Expr::Vector(items) = expr else {
        return Err("sample-data must be a vector".to_string());
    };
    items.iter().map(number_value).collect()
}

fn string_value(expr: &Expr) -> Result<&str, String> {
    match expr {
        Expr::String(value) => Ok(value.as_str()),
        _ => Err("expected string".to_string()),
    }
}

fn load_sample_path(expr: &Expr) -> Result<Vec<f32>, String> {
    let path = string_value(expr)?;
    let mut reader =
        hound::WavReader::open(Path::new(path)).map_err(|error| format!("{}: {}", path, error))?;
    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;
    let samples = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("{}: {}", path, error))?,
        hound::SampleFormat::Int => {
            let scale = if spec.bits_per_sample == 0 {
                1.0
            } else {
                (1_i64 << (spec.bits_per_sample.saturating_sub(1) as u32)) as f32
            };
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|value| value as f32 / scale))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("{}: {}", path, error))?
        }
    };
    if channels == 1 {
        return Ok(samples);
    }
    Ok(samples
        .chunks(channels)
        .map(|frame| frame.iter().copied().sum::<f32>() / frame.len() as f32)
        .collect())
}

fn waveform(expr: &Expr) -> Result<Waveform, String> {
    let name = match expr {
        Expr::Keyword(value) | Expr::Symbol(value) => value.as_str(),
        _ => return Err("source must be a keyword".to_string()),
    };
    Ok(match name {
        "sine-synth" => Waveform::Sine,
        "saw-synth" => Waveform::Saw,
        "square-synth" => Waveform::Square,
        "tri-synth" => Waveform::Tri,
        "pulse" | "pulse-synth" => Waveform::Pulse,
        "morph" | "morph-synth" => Waveform::Morph,
        "supersaw" | "supersaw-synth" => Waveform::SuperSaw,
        "wavetable" | "wavetable-synth" => Waveform::Wavetable,
        "fm-op" | "fm_op" | "fm-op-synth" => Waveform::FmOp,
        "additive" | "additive-synth" => Waveform::Additive,
        "sync" | "sync-synth" => Waveform::Sync,
        "pwm-sweep" | "pwm_sweep" => Waveform::PwmSweep,
        "harsh" => Waveform::Harsh,
        "chip" => Waveform::Chip,
        "pluck" => Waveform::Pluck,
        "strings" => Waveform::Strings,
        "brass" => Waveform::Brass,
        "organ" => Waveform::Organ,
        "bell" => Waveform::Bell,
        "glass" => Waveform::Glass,
        "vocal" => Waveform::Vocal,
        "breath" => Waveform::Breath,
        "pad-wash" | "pad_wash" => Waveform::PadWash,
        "click" => Waveform::Click,
        "noise-synth" => Waveform::Noise,
        "kick-synth" => Waveform::Kick,
        "snare" | "snare-synth" => Waveform::Snare,
        "hat" | "hat-synth" => Waveform::Hat,
        "kick-808" | "808-kick" => Waveform::Kick808,
        "snare-808" | "808-snare" => Waveform::Snare808,
        "hat-808" | "808-hat" => Waveform::Hat808,
        "cowbell-808" | "808-cowbell" => Waveform::Cowbell808,
        "kick-909" | "909-kick" => Waveform::Kick909,
        "snare-909" | "909-snare" => Waveform::Snare909,
        "hat-909" | "909-hat" => Waveform::Hat909,
        "kick-78" | "78-kick" => Waveform::Kick78,
        "snare-78" | "78-snare" => Waveform::Snare78,
        "hat-78" | "78-hat" => Waveform::Hat78,
        "kick-707" | "707-kick" => Waveform::Kick707,
        "snare-707" | "707-snare" => Waveform::Snare707,
        "clap" => Waveform::Clap,
        "cymbal-crash" | "cymbal_crash" => Waveform::CymbalCrash,
        "cymbal-ride" | "cymbal_ride" => Waveform::CymbalRide,
        "tom" => Waveform::Tom,
        "rimshot" => Waveform::Rimshot,
        "shaker" => Waveform::Shaker,
        "woodblock" => Waveform::Woodblock,
        "cowbell" => Waveform::Cowbell,
        "zap" => Waveform::Zap,
        "scratch" => Waveform::Scratch,
        "impact" => Waveform::Impact,
        "bass-slap" | "bass_slap" => Waveform::BassSlap,
        "piano-electric" | "piano_electric" => Waveform::PianoElectric,
        "drone-dark" | "drone_dark" => Waveform::DroneDark,
        "noise-white" | "noise_white" => Waveform::NoiseWhite,
        "noise-pink" | "noise_pink" => Waveform::NoisePink,
        "noise-brown" | "noise_brown" => Waveform::NoiseBrown,
        "noise-blue" | "noise_blue" => Waveform::NoiseBlue,
        "noise-purple" | "noise_purple" => Waveform::NoisePurple,
        "sample" => Waveform::Sample,
        other => return Err(format!("unsupported source ':{}'", other)),
    })
}

fn effect_chain(expr: &Expr) -> Result<Vec<TrackEffect>, String> {
    match expr {
        Expr::Vector(items) => items.iter().map(track_effect).collect(),
        Expr::List(_) => Ok(vec![track_effect(expr)?]),
        _ => Err("fx must be a vector of effect forms".to_string()),
    }
}

fn track_effect(expr: &Expr) -> Result<TrackEffect, String> {
    let Expr::List(items) = expr else {
        return Err("effect must be a form".to_string());
    };
    let Some(Expr::Symbol(name)) = items.first() else {
        return Ok(TrackEffect {
            spec: effect_spec(expr)?,
            gate_subdivisions: None,
        });
    };

    if name != "on" {
        return Ok(TrackEffect {
            spec: effect_spec(expr)?,
            gate_subdivisions: None,
        });
    }

    let mut gate_subdivisions = None;
    let mut effect = None;
    let mut index = 1;
    while index < items.len() {
        match &items[index] {
            Expr::Keyword(key) if key == "gate" => {
                let value = items
                    .get(index + 1)
                    .ok_or("on :gate requires a gate pattern")?;
                gate_subdivisions = Some(gate_subdivision_pattern(value)?);
                index += 2;
            }
            form @ Expr::List(_) => {
                effect = Some(effect_spec(form)?);
                index += 1;
            }
            _ => return Err("on expects :gate PATTERN followed by one effect form".to_string()),
        }
    }

    Ok(TrackEffect {
        spec: effect.ok_or("on requires an effect form")?,
        gate_subdivisions,
    })
}

fn offline_effect_chain(expr: &Expr) -> Result<Vec<OfflineEffectSpec>, String> {
    match expr {
        Expr::Vector(items) => items.iter().map(offline_effect_spec).collect(),
        Expr::List(_) => Ok(vec![offline_effect_spec(expr)?]),
        _ => Err("post-fx must be a vector of effect forms".to_string()),
    }
}

fn offline_effect_spec(expr: &Expr) -> Result<OfflineEffectSpec, String> {
    let Expr::List(items) = expr else {
        return Err("offline effect must be a form".to_string());
    };
    let Some(Expr::Symbol(name)) = items.first() else {
        return Err("offline effect form must start with a symbol".to_string());
    };
    match name.as_str() {
        "reverse" => Ok(OfflineEffectSpec::Reverse {
            mix: number_param(items, "mix", 1.0)?,
        }),
        "tape-stop" => Ok(OfflineEffectSpec::TapeStop {
            duration_pct: number_param(items, "duration-pct", 0.5)?,
        }),
        "granular" => Ok(OfflineEffectSpec::Granular {
            grain_ms: number_param(items, "grain-ms", 40.0)?,
            density: number_param(items, "density", 0.6)?,
            spray: number_param(items, "spray", 0.3)?,
            pitch_spread: number_param(items, "pitch-spread", 0.1)?,
        }),
        "granular-stretch" => Ok(OfflineEffectSpec::GranularStretch {
            rate: number_param(items, "rate", 0.5)?,
            grain_ms: number_param(items, "grain-ms", 60.0)?,
        }),
        "spectral-freeze" => Ok(OfflineEffectSpec::SpectralFreeze {
            freeze_pos: number_param(items, "freeze-pos", 0.3)?,
            sustain: number_param(items, "sustain", 0.6)?,
            mix: number_param(items, "mix", 0.4)?,
        }),
        "haas" => Ok(OfflineEffectSpec::Haas {
            delay_ms: number_param(items, "delay-ms", 14.0)?,
            side: stereo_side(keyword_param(items, "side").as_deref().unwrap_or("right"))?,
        }),
        "stereo-widen" | "stereo_widen" => Ok(OfflineEffectSpec::StereoWiden {
            width: number_param(items, "width", 0.8)?,
        }),
        "stereo-imager" | "stereo_imager" => Ok(OfflineEffectSpec::StereoImager {
            width: number_param(items, "width", 1.2)?,
            bass_mono_freq: number_param(items, "bass-mono-freq", 180.0)?,
        }),
        "width-enhance" | "width_enhance" => Ok(OfflineEffectSpec::WidthEnhance {
            low_width: number_param(items, "low-width", 0.7)?,
            high_width: number_param(items, "high-width", 1.4)?,
            crossover: number_param(items, "crossover", 800.0)?,
        }),
        "freq-shift" | "freq_shift" => Ok(OfflineEffectSpec::FreqShift {
            shift_hz: number_param(items, "shift-hz", 25.0)?,
            mix: number_param(items, "mix", 0.35)?,
        }),
        "autopan" | "auto-pan" => Ok(OfflineEffectSpec::AutoPan {
            rate: number_param(items, "rate", 2.0)?,
            depth: number_param(items, "depth", 0.8)?,
        }),
        "ping-pong-delay" | "ping_pong_delay" | "ping-pong" => {
            Ok(OfflineEffectSpec::PingPongDelay {
                time: number_param(items, "time", 0.3)?,
                feedback: number_param(items, "feedback", 0.5)?,
                mix: number_param(items, "mix", 0.5)?,
            })
        }
        _ => Ok(OfflineEffectSpec::Live(effect_spec(expr)?)),
    }
}

fn effect_spec(expr: &Expr) -> Result<EffectSpec, String> {
    let Expr::List(items) = expr else {
        return Err("effect must be a form".to_string());
    };
    let name = match items.first() {
        Some(Expr::Symbol(name)) => name.as_str(),
        Some(Expr::Number(value)) if (*value - 1176.0).abs() < f32::EPSILON => "1176",
        _ => return Err("effect form must start with a symbol".to_string()),
    };
    match name {
        "filter" => Ok(EffectSpec::Filter {
            kind: filter_kind(keyword_param(items, "type").as_deref().unwrap_or("lowpass"))?,
            cutoff: number_param(items, "cutoff", 1_000.0)?,
            resonance: number_param(items, "res", number_param(items, "resonance", 0.5)?)?,
            gain_db: number_param(items, "gain-db", number_param(items, "gain_db", 0.0)?)?,
        }),
        "comb" => Ok(EffectSpec::Comb {
            delay_ms: number_param(items, "delay-ms", number_param(items, "delay", 5.0)?)?,
            feedback: number_param(items, "feedback", 0.7)?,
            mix: number_param(items, "mix", 0.5)?,
        }),
        "formant" => Ok(EffectSpec::Formant {
            vowel: vowel(keyword_param(items, "vowel").as_deref().unwrap_or("a"))?,
            mix: number_param(items, "mix", 1.0)?,
        }),
        "distort" | "distortion" => Ok(EffectSpec::Distortion {
            kind: distortion_kind(keyword_param(items, "type").as_deref().unwrap_or("tanh"))?,
            drive: number_param(items, "drive", 0.5)?,
        }),
        "bitcrush" => Ok(EffectSpec::Bitcrush {
            bit_depth: number_param(items, "bits", number_param(items, "bit-depth", 8.0)?)?,
            sample_rate_reduction: number_param(
                items,
                "rate",
                number_param(items, "sample-rate-reduction", 1.0)?,
            )?,
        }),
        "delay" => Ok(EffectSpec::Delay {
            time: number_param(items, "time", 0.25)?,
            feedback: number_param(items, "feedback", 0.35)?,
            mix: number_param(items, "mix", 0.35)?,
        }),
        "wavefolder" | "fold" => Ok(EffectSpec::Wavefolder {
            folds: number_param(items, "folds", 3.0)?,
            gain: number_param(items, "gain", 2.0)?,
            symmetry: number_param(items, "symmetry", 1.0)?,
        }),
        "resonator" => Ok(EffectSpec::Resonator {
            freq: number_param(items, "freq", 200.0)?,
            decay: number_param(items, "decay", 0.98)?,
            mix: number_param(items, "mix", 0.5)?,
            harmonics: number_param(items, "harmonics", 4.0)?,
        }),
        "lofi" | "lo-fi" => Ok(EffectSpec::Lofi {
            amount: number_param(items, "amount", number_param(items, "intensity", 0.5)?)?,
        }),
        "vinyl" => Ok(EffectSpec::Vinyl {
            crackle: number_param(items, "crackle", 0.3)?,
            hiss: number_param(items, "hiss", 0.1)?,
            wow: number_param(items, "wow", 0.15)?,
        }),
        "sub-bass" | "subbass" => Ok(EffectSpec::SubBass {
            mix: number_param(items, "mix", 0.3)?,
        }),
        "sidechain" => Ok(EffectSpec::Sidechain {
            rate: number_param(items, "rate", 2.0)?,
            depth: number_param(items, "depth", 0.7)?,
            shape: number_param(items, "shape", 0.5)?,
        }),
        "radio" => Ok(EffectSpec::Radio {
            intensity: number_param(items, "intensity", 0.5)?,
        }),
        "telephone" => Ok(EffectSpec::Telephone {
            quality: number_param(items, "quality", 0.5)?,
        }),
        "underwater" => Ok(EffectSpec::Underwater {
            depth: number_param(items, "depth", number_param(items, "depth-amount", 0.5)?)?,
        }),
        "crystal" => Ok(EffectSpec::Crystal {
            brightness: number_param(items, "brightness", 0.5)?,
            decay: number_param(items, "decay", 0.3)?,
        }),
        "dc-remove" | "dc-block" => Ok(EffectSpec::DcRemove),
        "pitch-shift" => Ok(EffectSpec::PitchShift {
            semitones: number_param(items, "semitones", 7.0)?,
            mix: number_param(items, "mix", 0.5)?,
        }),
        "harmonizer" => Ok(EffectSpec::Harmonizer {
            interval: number_param(items, "interval", 7.0)?,
            mix: number_param(items, "mix", 0.3)?,
        }),
        "octaver" => Ok(EffectSpec::Octaver {
            octave_up: number_param(items, "octave-up", 0.0)?,
            octave_down: number_param(items, "octave-down", 0.3)?,
        }),
        "shimmer" => Ok(EffectSpec::Shimmer {
            shift_semitones: number_param(items, "shift-semitones", 12.0)?,
            feedback: number_param(items, "feedback", 0.3)?,
            mix: number_param(items, "mix", 0.4)?,
        }),
        "stutter" | "granular-stutter" => Ok(EffectSpec::Stutter {
            grain_size_ms: number_param(
                items,
                "grain-size-ms",
                number_param(items, "grain-ms", 50.0)?,
            )?,
            repeats: number_param(items, "repeats", 3.0)?,
            mix: number_param(items, "mix", 0.5)?,
        }),
        "glitch" => Ok(EffectSpec::Glitch {
            density: number_param(items, "density", 0.3)?,
            slice_ms: number_param(items, "slice-ms", 30.0)?,
        }),
        "fade" => Ok(EffectSpec::Fade {
            fade_in_ms: number_param(items, "fade-in-ms", 50.0)?,
            fade_out_ms: number_param(items, "fade-out-ms", 200.0)?,
            duration_seconds: optional_number_param(items, "duration")?,
        }),
        "adsr" | "asdr" => Ok(EffectSpec::Adsr {
            attack: number_param(items, "attack", number_param(items, "a", 0.01)?)?,
            decay: number_param(items, "decay", number_param(items, "d", 0.05)?)?,
            sustain: number_param(items, "sustain", number_param(items, "s", 0.7)?)?,
            release: number_param(items, "release", number_param(items, "r", 0.05)?)?,
            duration_seconds: optional_number_param(items, "duration")?,
        }),
        "doppler" => Ok(EffectSpec::Doppler {
            speed: number_param(items, "speed", 1.0)?,
            depth: number_param(items, "depth", 0.3)?,
        }),
        "maximizer" => Ok(EffectSpec::Maximizer {
            ceiling: number_param(items, "ceiling", -0.3)?,
            warmth: number_param(items, "warmth", 0.5)?,
            release_ms: number_param(items, "release-ms", 50.0)?,
        }),
        "multiband-comp" => Ok(EffectSpec::MultibandComp {
            low_thresh: number_param(items, "low-thresh", -20.0)?,
            mid_thresh: number_param(items, "mid-thresh", -18.0)?,
            high_thresh: number_param(items, "high-thresh", -15.0)?,
            crossover_low: number_param(items, "crossover-low", 200.0)?,
            crossover_high: number_param(items, "crossover-high", 4_000.0)?,
        }),
        "harmonic-enhance" => Ok(EffectSpec::HarmonicEnhance {
            low_harmonics: number_param(items, "low-harmonics", 0.3)?,
            high_harmonics: number_param(items, "high-harmonics", 0.2)?,
            air: number_param(items, "air", 0.15)?,
        }),
        "body" => Ok(EffectSpec::Body {
            size: number_param(items, "size", 0.5)?,
            tone: number_param(items, "tone", 0.5)?,
            mix: number_param(items, "mix", 0.3)?,
        }),
        "warmth" => Ok(EffectSpec::Warmth {
            amount: number_param(items, "amount", 0.5)?,
        }),
        "spatial" => Ok(EffectSpec::Spatial {
            room_size: number_param(items, "room-size", 0.5)?,
            position: number_param(items, "position", 0.5)?,
            height: number_param(items, "height", 0.3)?,
        }),
        "parallel-comp" | "parallel-compressor" | "ny-comp" => Ok(EffectSpec::ParallelComp {
            threshold: number_param(items, "threshold", -25.0)?,
            ratio: number_param(items, "ratio", 8.0)?,
            mix: number_param(items, "mix", 0.4)?,
        }),
        "tremolo" => Ok(EffectSpec::Tremolo {
            rate: number_param(items, "rate", 5.0)?,
            depth: number_param(items, "depth", 0.5)?,
        }),
        "chorus" => Ok(EffectSpec::Chorus {
            rate: number_param(items, "rate", 1.5)?,
            depth: number_param(items, "depth", 0.003)?,
            voices: number_param(items, "voices", 2.0)?,
            mix: number_param(items, "mix", 0.5)?,
        }),
        "dimension" => Ok(EffectSpec::Dimension {
            mode: number_param(items, "mode", 2.0)?,
        }),
        "ensemble" => Ok(EffectSpec::Ensemble {
            voices: number_param(items, "voices", 5.0)?,
            depth: number_param(items, "depth", 0.004)?,
            rate: number_param(items, "rate", 0.8)?,
        }),
        "ce1-chorus" | "ce-1" => Ok(EffectSpec::Ce1Chorus {
            rate: number_param(items, "rate", 0.5)?,
            intensity: number_param(items, "intensity", 0.5)?,
        }),
        "re301-chorus" | "re-301-chorus" => Ok(EffectSpec::Re301Chorus {
            rate: number_param(items, "rate", 0.6)?,
            depth: number_param(items, "depth", 0.5)?,
            tone: number_param(items, "tone", 0.5)?,
        }),
        "dimension-d" => Ok(EffectSpec::DimensionD {
            mode: number_param(items, "mode", 2.0)?,
        }),
        "h3000" => Ok(EffectSpec::H3000 {
            detune_cents: number_param(items, "detune-cents", 12.0)?,
            delay_ms: number_param(items, "delay-ms", 15.0)?,
            feedback: number_param(items, "feedback", 0.1)?,
            mix: number_param(items, "mix", 0.4)?,
        }),
        "flanger" => Ok(EffectSpec::Flanger {
            rate: number_param(items, "rate", 0.25)?,
            depth: number_param(items, "depth", 0.002)?,
            feedback: number_param(items, "feedback", 0.5)?,
            mix: number_param(items, "mix", 0.5)?,
        }),
        "phaser" => Ok(EffectSpec::Phaser {
            rate: number_param(items, "rate", 0.5)?,
            depth: number_param(items, "depth", 0.5)?,
            stages: number_param(items, "stages", 4.0)?,
            mix: number_param(items, "mix", 0.5)?,
        }),
        "small-stone" => Ok(EffectSpec::SmallStone {
            rate: number_param(items, "rate", 0.4)?,
            depth: number_param(items, "depth", 0.7)?,
            feedback: number_param(items, "feedback", 0.6)?,
            color: bool_param(items, "color", false),
        }),
        "vibrato" => Ok(EffectSpec::Vibrato {
            rate: number_param(items, "rate", 5.0)?,
            depth: number_param(items, "depth", 0.003)?,
        }),
        "ring-mod" | "ringmod" => Ok(EffectSpec::RingMod {
            freq: number_param(items, "freq", 200.0)?,
            mix: number_param(items, "mix", 0.5)?,
        }),
        "arp-ring-mod" => Ok(EffectSpec::ArpRingMod {
            freq: number_param(items, "freq", 300.0)?,
            depth: number_param(items, "depth", number_param(items, "mix", 0.8)?)?,
            diode_curve: number_param(items, "diode-curve", 0.3)?,
        }),
        "compressor" => Ok(EffectSpec::Compressor {
            threshold: number_param(items, "threshold", -20.0)?,
            ratio: number_param(items, "ratio", 4.0)?,
            attack: number_param(items, "attack", 0.01)?,
            release: number_param(items, "release", 0.1)?,
            makeup_gain: number_param(items, "makeup", number_param(items, "makeup-gain", 0.0)?)?,
        }),
        "fairchild" => Ok(EffectSpec::Fairchild {
            input_gain: number_param(items, "input-gain", 0.5)?,
            threshold: number_param(items, "threshold", -20.0)?,
            time_constant: number_param(items, "time-constant", 3.0)?,
            mix: number_param(items, "mix", 1.0)?,
        }),
        "ssl-comp" => Ok(EffectSpec::SslComp {
            threshold: number_param(items, "threshold", -15.0)?,
            ratio: number_param(items, "ratio", 4.0)?,
            attack_ms: number_param(items, "attack-ms", 10.0)?,
            release_ms: number_param(items, "release-ms", 100.0)?,
            makeup_db: number_param(items, "makeup-db", 0.0)?,
        }),
        "dbx160" => Ok(EffectSpec::Dbx160 {
            threshold: number_param(items, "threshold", -15.0)?,
            ratio: number_param(items, "ratio", 6.0)?,
        }),
        "la2a" => Ok(EffectSpec::La2a {
            peak_reduction: number_param(items, "peak-reduction", 0.5)?,
            limit: matches!(keyword_param(items, "mode").as_deref(), Some("limit")),
        }),
        "1176" | "urei-1176" => Ok(EffectSpec::Urei1176 {
            input_gain: number_param(items, "input-gain", 0.5)?,
            ratio: number_param(items, "ratio", 4.0)?,
            attack: number_param(items, "attack", 0.3)?,
            release: number_param(items, "release", 0.5)?,
        }),
        "limiter" => Ok(EffectSpec::Limiter {
            ceiling: number_param(items, "ceiling", -0.1)?,
            release: number_param(items, "release", 0.05)?,
        }),
        "gate" => Ok(EffectSpec::Gate {
            threshold: number_param(items, "threshold", -40.0)?,
            attack: number_param(items, "attack", 0.001)?,
            release: number_param(items, "release", 0.05)?,
        }),
        "transient" | "transient-shaper" => Ok(EffectSpec::Transient {
            attack_gain: number_param(items, "attack-gain", 1.5)?,
            sustain_gain: number_param(items, "sustain-gain", 1.0)?,
            sensitivity: number_param(items, "sensitivity", 0.01)?,
        }),
        "reverb" => Ok(EffectSpec::Reverb {
            decay: number_param(items, "decay", 0.5)?,
            mix: number_param(items, "mix", 0.3)?,
        }),
        "spring-reverb" => Ok(EffectSpec::SpringReverb {
            decay: number_param(items, "decay", 1.5)?,
            tone: number_param(items, "tone", 0.5)?,
            mix: number_param(items, "mix", 0.25)?,
            drip: number_param(items, "drip", 0.3)?,
        }),
        "emt-plate" => Ok(EffectSpec::EmtPlate {
            decay: number_param(items, "decay", 2.5)?,
            damping: number_param(items, "damping", 0.5)?,
            mix: number_param(items, "mix", 0.3)?,
            pre_delay_ms: number_param(items, "pre-delay-ms", 20.0)?,
        }),
        "lexicon-224" => Ok(EffectSpec::Lexicon224 {
            size: number_param(items, "size", 0.7)?,
            decay: number_param(items, "decay", 3.0)?,
            damping: number_param(items, "damping", 0.4)?,
            pre_delay_ms: number_param(items, "pre-delay-ms", 15.0)?,
            mix: number_param(items, "mix", 0.3)?,
        }),
        "ams-reverb" => Ok(EffectSpec::AmsReverb {
            decay: number_param(items, "decay", 2.0)?,
            damping: number_param(items, "damping", 0.5)?,
            program: ams_program(
                keyword_param(items, "program")
                    .as_deref()
                    .unwrap_or("nonlin"),
            )?,
            mix: number_param(items, "mix", 0.3)?,
        }),
        "tube" | "tube-saturation" => Ok(EffectSpec::Tube {
            drive: number_param(items, "drive", number_param(items, "gain", 0.5)?)?,
            asymmetry: number_param(items, "asymmetry", 0.2)?,
        }),
        "neve-preamp" => Ok(EffectSpec::NevePreamp {
            gain: number_param(items, "gain", 0.5)?,
            warmth: number_param(items, "warmth", 0.5)?,
        }),
        "marshall-amp" => Ok(EffectSpec::MarshallAmp {
            gain: number_param(items, "gain", 0.6)?,
            tone: number_param(items, "tone", 0.5)?,
            presence: number_param(items, "presence", 0.5)?,
        }),
        "vox-ac30" => Ok(EffectSpec::VoxAc30 {
            gain: number_param(items, "gain", 0.5)?,
            treble: number_param(items, "treble", 0.6)?,
            cut: number_param(items, "cut", 0.4)?,
        }),
        "fender-twin" => Ok(EffectSpec::FenderTwin {
            volume: number_param(items, "volume", number_param(items, "gain", 0.4)?)?,
            treble: number_param(items, "treble", 0.6)?,
            bass: number_param(items, "bass", 0.5)?,
            reverb_mix: number_param(items, "reverb-mix", 0.3)?,
        }),
        "pultec-eq" | "pultec" => Ok(EffectSpec::PultecEq {
            low_boost: number_param(items, "low-boost", 0.3)?,
            low_atten: number_param(items, "low-atten", 0.0)?,
            low_freq: number_param(items, "low-freq", 60.0)?,
            high_boost: number_param(items, "high-boost", 0.3)?,
            high_atten: number_param(items, "high-atten", 0.0)?,
            high_freq: number_param(items, "high-freq", 8_000.0)?,
        }),
        "tape" => Ok(EffectSpec::Tape {
            saturation: number_param(
                items,
                "saturation",
                number_param(items, "input-level", 0.5)?,
            )?,
            wow: number_param(items, "wow", 0.1)?,
            flutter: number_param(items, "flutter", 0.1)?,
        }),
        "studer-tape" => Ok(EffectSpec::StuderTape {
            input_level: number_param(items, "input-level", 0.5)?,
            speed: number_param(items, "speed", 1.0)?,
            bias: number_param(items, "bias", 0.5)?,
        }),
        "exciter" => Ok(EffectSpec::Exciter {
            amount: number_param(items, "amount", 0.5)?,
            cutoff: number_param(items, "cutoff", 3_000.0)?,
        }),
        "moog" | "moog-ladder" => Ok(EffectSpec::Moog {
            cutoff: number_param(items, "cutoff", 1_000.0)?,
            resonance: number_param(items, "res", number_param(items, "resonance", 0.7)?)?,
            drive: number_param(items, "drive", 0.1)?,
        }),
        "prophet-filter" => Ok(EffectSpec::ProphetFilter {
            cutoff: number_param(items, "cutoff", 2_000.0)?,
            resonance: number_param(items, "res", number_param(items, "resonance", 0.6)?)?,
        }),
        "obxa-filter" => Ok(EffectSpec::ObxaFilter {
            cutoff: number_param(items, "cutoff", 2_000.0)?,
            resonance: number_param(items, "res", number_param(items, "resonance", 0.6)?)?,
            kind: filter_kind(keyword_param(items, "type").as_deref().unwrap_or("lowpass"))?,
        }),
        "303" | "303-filter" | "tb303" | "tb-303" => Ok(EffectSpec::Diode303 {
            cutoff: number_param(items, "cutoff", 800.0)?,
            resonance: number_param(items, "res", number_param(items, "resonance", 0.8)?)?,
            env_mod: number_param(items, "env-mod", 0.5)?,
            accent: number_param(items, "accent", 0.3)?,
            decay: number_param(items, "decay", 0.3)?,
        }),
        "space-echo" | "re201" | "re-201" => Ok(EffectSpec::SpaceEcho {
            time: number_param(items, "time", 0.375)?,
            feedback: number_param(items, "feedback", 0.6)?,
            wow: number_param(items, "wow", 0.2)?,
            flutter: number_param(items, "flutter", 0.15)?,
            tone: number_param(items, "tone", 0.5)?,
            spring_mix: number_param(items, "spring-mix", 0.15)?,
            mix: number_param(items, "mix", 0.4)?,
        }),
        "tc2290" | "tc-2290" => Ok(EffectSpec::Tc2290 {
            time_ms: number_param(items, "time-ms", 350.0)?,
            feedback: number_param(items, "feedback", 0.4)?,
            mod_rate: number_param(items, "mod-rate", 0.3)?,
            mod_depth: number_param(items, "mod-depth", 0.002)?,
            mix: number_param(items, "mix", 0.35)?,
        }),
        "sem-filter" | "sem" => Ok(EffectSpec::Sem {
            cutoff: number_param(items, "cutoff", 2_000.0)?,
            resonance: number_param(items, "res", number_param(items, "resonance", 0.5)?)?,
            kind: filter_kind(keyword_param(items, "type").as_deref().unwrap_or("lowpass"))?,
        }),
        "ms20" | "ms20-filter" => Ok(EffectSpec::Ms20 {
            cutoff: number_param(items, "cutoff", 1_500.0)?,
            resonance: number_param(items, "res", number_param(items, "resonance", 0.7)?)?,
        }),
        "wasp-filter" => Ok(EffectSpec::WaspFilter {
            cutoff: number_param(items, "cutoff", 1_500.0)?,
            resonance: number_param(items, "res", number_param(items, "resonance", 0.7)?)?,
        }),
        "juno-hpf" => Ok(EffectSpec::JunoHpf {
            cutoff: number_param(items, "cutoff", 300.0)?,
            resonance: number_param(items, "res", number_param(items, "resonance", 0.3)?)?,
        }),
        "buchla-lpg" | "lpg" => Ok(EffectSpec::BuchlaLpg {
            strike: number_param(items, "strike", 0.7)?,
            decay: number_param(items, "decay", 0.3)?,
            resonance: number_param(items, "res", number_param(items, "resonance", 0.2)?)?,
        }),
        other => Err(format!("unsupported effect '{}'", other)),
    }
}

fn keyword_param(items: &[Expr], key: &str) -> Option<String> {
    let mut index = 1;
    while index + 1 < items.len() {
        if matches!(&items[index], Expr::Keyword(value) if value == key) {
            if null_value(&items[index + 1]) {
                return None;
            }
            return match &items[index + 1] {
                Expr::Keyword(value) | Expr::Symbol(value) => Some(value.clone()),
                _ => None,
            };
        }
        index += 2;
    }
    None
}

fn number_param(items: &[Expr], key: &str, default: f32) -> Result<f32, String> {
    let mut index = 1;
    while index + 1 < items.len() {
        if matches!(&items[index], Expr::Keyword(value) if value == key) {
            if null_value(&items[index + 1]) {
                return Ok(default);
            }
            return number_value(&items[index + 1]);
        }
        index += 2;
    }
    Ok(default)
}

fn optional_number_param(items: &[Expr], key: &str) -> Result<Option<f32>, String> {
    let mut index = 1;
    while index + 1 < items.len() {
        if matches!(&items[index], Expr::Keyword(value) if value == key) {
            if null_value(&items[index + 1]) {
                return Ok(None);
            }
            return number_value(&items[index + 1]).map(Some);
        }
        index += 2;
    }
    Ok(None)
}

fn bool_param(items: &[Expr], key: &str, default: bool) -> bool {
    let mut index = 1;
    while index + 1 < items.len() {
        if matches!(&items[index], Expr::Keyword(value) if value == key) {
            if null_value(&items[index + 1]) {
                return default;
            }
            return match &items[index + 1] {
                Expr::Keyword(value) | Expr::Symbol(value) => {
                    matches!(value.as_str(), "true" | "on" | "yes" | "1")
                }
                Expr::Number(value) => *value != 0.0,
                _ => default,
            };
        }
        index += 2;
    }
    default
}

fn stereo_side(name: &str) -> Result<StereoSide, String> {
    Ok(match name {
        "left" => StereoSide::Left,
        "right" => StereoSide::Right,
        other => return Err(format!("unsupported stereo side '{}'", other)),
    })
}

fn ams_program(name: &str) -> Result<effects::hardware::AmsProgram, String> {
    Ok(match name {
        "nonlin" | "non-linear" | "nonlinear" => effects::hardware::AmsProgram::Nonlin,
        "ambience" | "ambient" => effects::hardware::AmsProgram::Ambience,
        "plate" => effects::hardware::AmsProgram::Plate,
        other => return Err(format!("unsupported AMS reverb program '{}'", other)),
    })
}

fn filter_kind(name: &str) -> Result<FilterKind, String> {
    Ok(match name {
        "lowpass" | "lp" => FilterKind::Lowpass,
        "highpass" | "hp" => FilterKind::Highpass,
        "bandpass" | "bp" => FilterKind::Bandpass,
        "notch" => FilterKind::Notch,
        "allpass" | "ap" => FilterKind::Allpass,
        "peaking" | "peak" | "bell" => FilterKind::Peaking,
        "low-shelf" | "low_shelf" | "lowshelf" => FilterKind::LowShelf,
        "high-shelf" | "high_shelf" | "highshelf" => FilterKind::HighShelf,
        other => return Err(format!("unsupported filter type ':{}'", other)),
    })
}

fn vowel(name: &str) -> Result<effects::filters::Vowel, String> {
    Ok(match name {
        "a" => effects::filters::Vowel::A,
        "e" => effects::filters::Vowel::E,
        "i" => effects::filters::Vowel::I,
        "o" => effects::filters::Vowel::O,
        "u" => effects::filters::Vowel::U,
        other => return Err(format!("unsupported vowel ':{}'", other)),
    })
}

fn distortion_kind(name: &str) -> Result<DistortionKind, String> {
    Ok(match name {
        "tanh" => DistortionKind::Tanh,
        "hard-clip" | "hard_clip" => DistortionKind::HardClip,
        "soft-clip" | "soft_clip" => DistortionKind::SoftClip,
        "sine-fold" | "sine_fold" => DistortionKind::SineFold,
        "rectify" => DistortionKind::Rectify,
        "half-rectify" | "half_rectify" => DistortionKind::HalfRectify,
        "waveshape" => DistortionKind::Waveshape,
        other => return Err(format!("unsupported distortion type ':{}'", other)),
    })
}

fn note_pattern(expr: &Expr) -> Result<(Vec<f32>, NoteMode), String> {
    match expr {
        Expr::List(items) if matches!(items.first(), Some(Expr::Symbol(name)) if name == "p") => {
            Ok((pattern_values(items, true, "p")?, NoteMode::Step))
        }
        Expr::List(items) if matches!(items.first(), Some(Expr::Symbol(name)) if name == "s") => {
            Ok((pattern_values(items, true, "s")?, NoteMode::Hit))
        }
        Expr::List(items) if matches!(items.first(), Some(Expr::Symbol(name)) if name == "gs" || name == "gate-seq" || name == "gate_seq") =>
        {
            let name = match items.first() {
                Some(Expr::Symbol(name)) => name.as_str(),
                _ => "gs",
            };
            Ok((pattern_values(items, true, name)?, NoteMode::Tick))
        }
        Expr::List(items) if matches!(items.first(), Some(Expr::Symbol(name)) if name == "rev") => {
            let source = items.get(1).ok_or("rev requires a pattern")?;
            let (mut values, mode) = note_pattern(source)?;
            values.reverse();
            Ok((values, mode))
        }
        _ => Ok((number_pattern(expr, true)?, NoteMode::Step)),
    }
}

fn pattern_values(items: &[Expr], notes: bool, name: &str) -> Result<Vec<f32>, String> {
    let Some(Expr::Vector(values)) = items.get(1) else {
        return Err(format!("{} requires a vector", name));
    };
    values
        .iter()
        .map(|value| {
            if notes {
                number_value(value)
            } else {
                numeric_only(value)
            }
        })
        .collect()
}

fn number_pattern(expr: &Expr, notes: bool) -> Result<Vec<f32>, String> {
    match expr {
        Expr::List(items) if matches!(items.first(), Some(Expr::Symbol(name)) if name == "p") => {
            pattern_values(items, notes, "p")
        }
        Expr::List(items) if matches!(items.first(), Some(Expr::Symbol(name)) if name == "rev") => {
            let source = items.get(1).ok_or("rev requires a pattern")?;
            let mut values = number_pattern(source, notes)?;
            values.reverse();
            Ok(values)
        }
        Expr::Vector(values) => values
            .iter()
            .map(|value| {
                if notes {
                    number_value(value)
                } else {
                    numeric_only(value)
                }
            })
            .collect(),
        _ => Ok(vec![if notes {
            number_value(expr)?
        } else {
            numeric_only(expr)?
        }]),
    }
}

fn gate_pattern(expr: &Expr) -> Result<(Vec<Vec<bool>>, Vec<Vec<usize>>), String> {
    if let Expr::List(items) = expr {
        if let Some(Expr::Symbol(name)) = items.first() {
            match name.as_str() {
                "euclid" => {
                    let pulses = number_arg(items, 1, "euclid")? as usize;
                    let steps = number_arg(items, 2, "euclid")? as usize;
                    let gates: Vec<Vec<bool>> = sequencer::euclid(pulses, steps, 0)
                        .into_iter()
                        .map(|gate| vec![gate])
                        .collect();
                    let holds = empty_holds_like(&gates);
                    return Ok((gates, holds));
                }
                "euclid-rot" => {
                    let pulses = number_arg(items, 1, "euclid-rot")? as usize;
                    let steps = number_arg(items, 2, "euclid-rot")? as usize;
                    let rotation = number_arg(items, 3, "euclid-rot")? as usize;
                    let gates: Vec<Vec<bool>> = sequencer::euclid(pulses, steps, rotation)
                        .into_iter()
                        .map(|gate| vec![gate])
                        .collect();
                    let holds = empty_holds_like(&gates);
                    return Ok((gates, holds));
                }
                "rev" => {
                    let source = items.get(1).ok_or("rev requires a pattern")?;
                    let (mut gates, mut holds) = gate_pattern(source)?;
                    gates.reverse();
                    holds.reverse();
                    validate_gate_holds(&gates, &holds)?;
                    return Ok((gates, holds));
                }
                "p" => {
                    let Some(Expr::Vector(values)) = items.get(1) else {
                        return Err("p requires a vector".to_string());
                    };
                    let pattern = values
                        .iter()
                        .map(gate_step_pattern)
                        .collect::<Result<Vec<_>, _>>()?;
                    return gate_pattern_from_steps(pattern);
                }
                _ => {}
            }
        }
    }
    match expr {
        Expr::Vector(values) => {
            let pattern = values
                .iter()
                .map(gate_step_pattern)
                .collect::<Result<Vec<_>, _>>()?;
            gate_pattern_from_steps(pattern)
        }
        _ => {
            let step = gate_step_pattern(expr)?;
            gate_pattern_from_steps(vec![step])
        }
    }
}

fn gate_subdivision_pattern(expr: &Expr) -> Result<Vec<Vec<bool>>, String> {
    Ok(gate_pattern(expr)?.0)
}

#[derive(Clone, Debug)]
struct GateSlot {
    gate: bool,
    hold: usize,
}

fn gate_pattern_from_steps(
    steps: Vec<Vec<GateSlot>>,
) -> Result<(Vec<Vec<bool>>, Vec<Vec<usize>>), String> {
    let gates: Vec<Vec<bool>> = steps
        .iter()
        .map(|step| step.iter().map(|slot| slot.gate).collect())
        .collect();
    let holds: Vec<Vec<usize>> = steps
        .iter()
        .map(|step| step.iter().map(|slot| slot.hold).collect())
        .collect();
    validate_gate_holds(&gates, &holds)?;
    Ok((gates, holds))
}

fn empty_holds_like(gates: &[Vec<bool>]) -> Vec<Vec<usize>> {
    gates
        .iter()
        .map(|step| vec![0; step.len().max(1)])
        .collect()
}

fn flatten_gate_grid(gates: &[Vec<bool>]) -> Vec<bool> {
    gates.iter().flat_map(|step| step.iter().copied()).collect()
}

fn flatten_hold_grid(holds: &[Vec<usize>]) -> Vec<usize> {
    holds.iter().flat_map(|step| step.iter().copied()).collect()
}

fn validate_gate_holds(gates: &[Vec<bool>], holds: &[Vec<usize>]) -> Result<(), String> {
    let flat_gates = flatten_gate_grid(gates);
    let flat_holds = flatten_hold_grid(holds);
    let total = flat_gates.len().max(1);
    for (idx, hold) in flat_holds.iter().copied().enumerate() {
        if hold == 0 {
            continue;
        }
        if !flat_gates.get(idx).copied().unwrap_or(false) {
            return Err("gate hold can only be attached to a hit".to_string());
        }
        for extension in 1..=hold {
            let covered = (idx + extension) % total;
            if flat_gates.get(covered).copied().unwrap_or(false) {
                return Err(format!(
                    "gate hold at slot {} overlaps another hit at slot {}",
                    idx, covered
                ));
            }
        }
    }
    Ok(())
}

fn gate_step_pattern(expr: &Expr) -> Result<Vec<GateSlot>, String> {
    match expr {
        Expr::List(items) if matches!(items.first(), Some(Expr::Symbol(name)) if name == "gate-hold") =>
        {
            let hold = match items.get(1) {
                Some(value) => usize_value(value, "gate-hold")?,
                None => 1,
            };
            if items.len() > 2 {
                return Err("gate-hold expects zero or one amount".to_string());
            }
            Ok(vec![GateSlot { gate: true, hold }])
        }
        Expr::Vector(values) => {
            if values.is_empty() {
                return Ok(vec![GateSlot {
                    gate: false,
                    hold: 0,
                }]);
            }

            let child_patterns = values
                .iter()
                .map(gate_step_pattern)
                .collect::<Result<Vec<_>, _>>()?;
            let cell_width = child_patterns
                .iter()
                .map(|pattern| pattern.len().max(1))
                .fold(1, lcm);
            let mut flattened = Vec::with_capacity(values.len() * cell_width);
            for child in child_patterns {
                flattened.extend(expand_gate_cell(&child, cell_width));
            }
            Ok(flattened)
        }
        _ => Ok(vec![GateSlot {
            gate: numeric_only(expr)? > 0.0,
            hold: 0,
        }]),
    }
}

fn expand_gate_cell(pattern: &[GateSlot], width: usize) -> Vec<GateSlot> {
    let mut expanded = vec![
        GateSlot {
            gate: false,
            hold: 0,
        };
        width.max(1)
    ];
    if pattern.is_empty() {
        return expanded;
    }

    let scale = width.max(1) / pattern.len().max(1);
    for (idx, slot) in pattern.iter().enumerate() {
        if slot.gate {
            expanded[idx * scale] = slot.clone();
        }
    }
    expanded
}

fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        let next = a % b;
        a = b;
        b = next;
    }
    a.max(1)
}

fn lcm(a: usize, b: usize) -> usize {
    let a = a.max(1);
    let b = b.max(1);
    a / gcd(a, b) * b
}

fn numeric_only(expr: &Expr) -> Result<f32, String> {
    match expr {
        Expr::Number(value) => Ok(*value),
        _ => Err("expected numeric pattern value".to_string()),
    }
}

fn note_freq(name: &str) -> Option<f32> {
    let mut chars = name.chars().peekable();
    let root = chars.next()?.to_ascii_lowercase();
    let base = match root {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _ => return None,
    };
    let accidental = match chars.peek().copied() {
        Some('s') => {
            chars.next();
            1
        }
        Some('b') => {
            chars.next();
            -1
        }
        _ => 0,
    };
    let octave: i32 = chars.collect::<String>().parse().ok()?;
    let midi = 57 + ((octave - 4) * 12) + base + accidental;
    Some(440.0 * 2.0_f32.powf((midi as f32 - 69.0) / 12.0))
}
