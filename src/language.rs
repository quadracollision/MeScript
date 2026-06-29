use crate::effects::offline::{OfflineEffectSpec, StereoSide};
use crate::effects::{self, DistortionKind, EffectSpec, FilterKind};
use crate::model::{
    EffectParamPattern, EffectParamPatternMode, GateCell, NoteMode, OscillatorParams,
    ParamPatterns, Runtime, Scene, SceneState, Track, TrackEffect, Waveform,
};
use crate::sequencer;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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

pub(crate) fn source_needs_compiler(source: &str) -> bool {
    if tokenize(source)
        .into_iter()
        .any(|token| token.starts_with("1_"))
    {
        return true;
    }
    parse_program(source)
        .map(|forms| forms.iter().any(expr_needs_compiler))
        .unwrap_or(false)
}

fn expr_needs_compiler(expr: &Expr) -> bool {
    match expr {
        Expr::List(items) => {
            let head = match items.first() {
                Some(Expr::Symbol(head)) => head.as_str(),
                _ => return items.iter().any(expr_needs_compiler),
            };
            if head == "p"
                && (items.len() != 2
                    || matches!(items.get(1), Some(Expr::Keyword(key)) if key == "repeat"))
            {
                return true;
            }
            if head == "times"
                && (items.len() != 3
                    || !matches!(items.get(1), Some(Expr::Number(value)) if *value > 0.0 && value.fract() == 0.0))
            {
                return true;
            }
            if head == "then" && items.len() < 3 {
                return true;
            }
            matches!(
                head,
                "def"
                    | "tracks"
                    | "with"
                    | "section"
                    | "by-scene"
                    | "+"
                    | "-"
                    | "*"
                    | "/"
                    | "and"
                    | "or"
                    | "not"
                    | "map"
                    | "range"
                    | "repeat"
                    | "take"
                    | "rotate"
                    | "interleave"
                    | "every-n"
                    | "choose"
                    | "rand-range"
                    | "scale"
                    | "chord"
                    | "shape"
                    | "arpeggio"
                    | "arp"
                    | "transpose"
            ) || items.iter().skip(1).any(expr_needs_compiler)
        }
        Expr::Vector(items) => items.iter().any(expr_needs_compiler),
        _ => false,
    }
}

fn include_path_from_line(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("(include")?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    let after = rest[end + 1..].trim();
    (after == ")").then_some(&rest[..end])
}

fn include_display_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn expand_source_includes_inner(
    source: &str,
    base_dir: &Path,
    stack: &mut Vec<PathBuf>,
) -> Result<String, String> {
    let mut expanded = String::new();
    for line in source.lines() {
        if let Some(include_path) = include_path_from_line(line) {
            let path = Path::new(include_path);
            let resolved = if path.is_absolute() {
                path.to_path_buf()
            } else {
                base_dir.join(path)
            };
            let key = resolved.canonicalize().unwrap_or_else(|_| resolved.clone());
            if stack.iter().any(|existing| existing == &key) {
                return Err(format!(
                    "include cycle detected at {}",
                    include_display_path(&resolved)
                ));
            }
            let included = fs::read_to_string(&resolved).map_err(|error| {
                format!("include {}: {}", include_display_path(&resolved), error)
            })?;
            stack.push(key);
            let child_base = resolved.parent().unwrap_or(base_dir);
            expanded.push_str(&expand_source_includes_inner(&included, child_base, stack)?);
            if !expanded.ends_with('\n') {
                expanded.push('\n');
            }
            stack.pop();
        } else {
            expanded.push_str(line);
            expanded.push('\n');
        }
    }
    Ok(expanded)
}

pub(crate) fn expand_source_includes(
    source: &str,
    source_path: Option<&Path>,
) -> Result<String, String> {
    if !source
        .lines()
        .any(|line| include_path_from_line(line).is_some())
    {
        return Ok(source.to_string());
    }
    let base_dir = source_path
        .and_then(Path::parent)
        .unwrap_or_else(|| Path::new("."));
    let mut stack = Vec::new();
    if let Some(path) = source_path {
        stack.push(path.canonicalize().unwrap_or_else(|_| path.to_path_buf()));
    }
    expand_source_includes_inner(source, base_dir, &mut stack)
}

pub(crate) fn compile_source_for_runtime_with_base(
    source: &str,
    source_path: Option<&Path>,
) -> Result<String, String> {
    let source = expand_source_includes(source, source_path)?;
    if !source_needs_compiler(&source) {
        return Ok(source);
    }
    let compiler_path = Path::new("src/compiler.clj");
    if !compiler_path.exists() {
        return Err(
            "source uses compiler helper forms, but src/compiler.clj was not found".to_string(),
        );
    }
    let mut child = Command::new("clojure")
        .args([
            "-e",
            "(do (load-file \"src/compiler.clj\") (print (glitchlisp-compiler/compile-source (slurp *in*))))",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to run Clojure compiler: {}", error))?;
    child
        .stdin
        .as_mut()
        .ok_or("failed to open compiler stdin")?
        .write_all(source.as_bytes())
        .map_err(|error| format!("failed to send source to compiler: {}", error))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("failed to wait for compiler: {}", error))?;
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|error| error.to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            "compiler failed without an error message".to_string()
        } else {
            clean_compiler_error(&stderr)
        })
    }
}

pub(crate) fn compile_source_for_runtime(source: &str) -> Result<String, String> {
    compile_source_for_runtime_with_base(source, None)
}

fn clean_compiler_error(stderr: &str) -> String {
    let mut lines = stderr
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    while let Some(line) = lines.next() {
        if line.starts_with("Execution error") {
            return lines.next().unwrap_or(line).to_string();
        }
        if line.starts_with("Full report at:") {
            break;
        }
    }
    stderr.to_string()
}

pub(crate) fn load_runtime(path: &str) -> Result<Runtime, String> {
    let source = fs::read_to_string(path).map_err(|error| format!("{}: {}", path, error))?;
    let source = compile_source_for_runtime_with_base(&source, Some(Path::new(path)))?;
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
            expect_arity(items, "bpm", 2)?;
            runtime.bpm = bpm_value(number_arg(items, 1, "bpm")?)?;
            Ok(())
        }
        "start!" => {
            expect_arity(items, "start!", 1)?;
            if runtime.tracks.is_empty() {
                return if runtime.scenes.is_empty() {
                    Err("start! requires at least one top-level track".to_string())
                } else {
                    Err("start! only starts top-level tracks; use (play-scene :scene-name) for scenes".to_string())
                };
            }
            runtime.running = true;
            Ok(())
        }
        "stop!" => {
            expect_arity(items, "stop!", 1)?;
            runtime.running = false;
            runtime.scene_state = None;
            Ok(())
        }
        "block" | "scene" => define_scene(runtime, items),
        "sample" => define_sample_track(runtime, items),
        "play-block" | "play-scene" | "cue" => play_scene(runtime, items, name),
        "play-note" => {
            expect_arity(items, "play-note", 2)?;
            let freq = number_arg(items, 1, "play-note")?;
            runtime.tracks.insert(
                "tone".to_string(),
                Track {
                    id: "tone".to_string(),
                    waveform: Waveform::Sine,
                    oscillator: OscillatorParams::default(),
                    notes: vec![freq],
                    note_chords: vec![vec![freq]],
                    note_mode: NoteMode::Step,
                    gates: vec![true, false, false, false],
                    gate_subdivisions: vec![vec![true], vec![false], vec![false], vec![false]],
                    gate_cells: vec![
                        vec![GateCell::Static(true)],
                        vec![GateCell::Static(false)],
                        vec![GateCell::Static(false)],
                        vec![GateCell::Static(false)],
                    ],
                    gate_holds: vec![vec![0], vec![0], vec![0], vec![0]],
                    gate_loop_start: 0,
                    step_every: 1,
                    step_offset: 0,
                    drunk: 0.0,
                    amp: 0.35,
                    dur_seconds: 0.2,
                    param_patterns: ParamPatterns::default(),
                    effects: Vec::new(),
                    sample_data: Vec::new(),
                    choke: false,
                    muted: false,
                    solo: false,
                },
            );
            runtime.running = true;
            Ok(())
        }
        "post-fx" | "master-fx" => {
            expect_arity(items, name, 2)?;
            runtime.post_effects =
                offline_effect_chain(items.get(1).ok_or("post-fx requires an effect vector")?)?;
            Ok(())
        }
        "d" => define_track(runtime, items),
        "clear" => {
            expect_arity(items, "clear", 2)?;
            let id = keyword_arg(items, 1, "clear")?;
            runtime
                .tracks
                .remove(&id)
                .map(|_| ())
                .ok_or_else(|| format!("unknown track ':{}'", id))
        }
        "clear-all" => {
            expect_arity(items, "clear-all", 1)?;
            runtime.tracks.clear();
            runtime.post_effects.clear();
            runtime.scenes.clear();
            runtime.scene_state = None;
            runtime.running = false;
            Ok(())
        }
        "mute" => set_track_flag(runtime, items, "mute", true, false),
        "unmute" => set_track_flag(runtime, items, "unmute", false, false),
        "solo" => set_track_flag(runtime, items, "solo", true, true),
        "unsolo" => set_track_flag(runtime, items, "unsolo", false, true),
        other => Err(format!("unsupported form '{}'", other)),
    }
}

fn expect_arity(items: &[Expr], form: &str, expected: usize) -> Result<(), String> {
    if items.len() == expected {
        Ok(())
    } else if expected == 1 {
        Err(format!("{} expects no arguments", form))
    } else {
        Err(format!(
            "{} expects exactly {} argument",
            form,
            expected - 1
        ))
    }
}

pub(crate) fn apply_scene(runtime: &mut Runtime, id: &str) -> Result<(), String> {
    let scene = runtime
        .scenes
        .get(id)
        .cloned()
        .ok_or_else(|| format!("unknown scene ':{}'", id))?;
    validate_scene_next_targets(runtime, id)?;
    runtime.tracks = scene.tracks;
    runtime.post_effects = scene.post_effects;
    if let Some(bpm) = scene.bpm {
        runtime.bpm = bpm;
    }
    runtime.scene_state = Some(SceneState {
        current: scene.id,
        cycle: 0,
        start_step: 0,
    });
    runtime.running = true;
    Ok(())
}

fn validate_scene_next_targets(runtime: &Runtime, start: &str) -> Result<(), String> {
    let mut current = start.to_string();
    let mut visited = HashSet::new();
    while visited.insert(current.clone()) {
        let Some(scene) = runtime.scenes.get(&current) else {
            return Err(format!("unknown scene ':{}'", current));
        };
        if scene.repeats == 0 {
            return Ok(());
        }
        let Some(next) = &scene.next else {
            return Ok(());
        };
        if !runtime.scenes.contains_key(next) {
            return Err(format!(
                "scene ':{}' :next references unknown scene ':{}'",
                scene.id, next
            ));
        }
        current = next.clone();
    }
    Ok(())
}

fn define_scene(runtime: &mut Runtime, items: &[Expr]) -> Result<(), String> {
    let Expr::Keyword(id) = items.get(1).ok_or("scene requires a scene id")? else {
        return Err("scene id must be a keyword".to_string());
    };

    let mut repeats = 0;
    let mut explicit_repeats = false;
    let mut steps = 0;
    let mut explicit_steps = false;
    let mut steps_of = None;
    let mut loop_by = None;
    let mut bars = None;
    let mut bar_steps = None;
    let mut bar_steps_of = None;
    let mut next = None;
    let mut seen_options = HashSet::new();
    let mut index = 2;
    while index < items.len() {
        let Expr::Keyword(key) = &items[index] else {
            break;
        };
        let canonical_key = scene_option_canonical_key(key);
        if !seen_options.insert(canonical_key) {
            return Err(format!("duplicate scene option ':{}'", key));
        }
        match key.as_str() {
            "repeat" | "repeats" | "times" => {
                let value = items
                    .get(index + 1)
                    .ok_or("scene :repeat requires a value")?;
                repeats = usize_value(value, "repeat")?;
                explicit_repeats = true;
                index += 2;
            }
            "loop" => {
                let value = items.get(index + 1).ok_or("scene :loop requires a value")?;
                loop_true_value(value)?;
                repeats = 0;
                explicit_repeats = true;
                index += 2;
            }
            "steps" | "length" => {
                let value = items
                    .get(index + 1)
                    .ok_or("scene :steps requires a value")?;
                steps = positive_usize_value(value, "steps")?;
                explicit_steps = true;
                index += 2;
            }
            "bars" => {
                let value = items.get(index + 1).ok_or("scene :bars requires a value")?;
                bars = Some(positive_usize_value(value, "bars")?);
                explicit_steps = true;
                index += 2;
            }
            "bar-steps" | "bar-length" => {
                let value = items
                    .get(index + 1)
                    .ok_or("scene :bar-steps requires a value")?;
                bar_steps = Some(positive_usize_value(value, "bar-steps")?);
                index += 2;
            }
            "bar-steps-of" | "bar-length-of" => {
                bar_steps_of = Some(keyword_arg(items, index + 1, "scene :bar-steps-of")?);
                index += 2;
            }
            "steps-of" | "length-of" => {
                steps_of = Some(keyword_arg(items, index + 1, "scene :steps-of")?);
                index += 2;
            }
            "loop-by" => {
                let track_id = keyword_arg(items, index + 1, "scene :loop-by")?;
                let count = items
                    .get(index + 2)
                    .ok_or("scene :loop-by requires a count")?;
                loop_by = Some((track_id, positive_usize_value(count, "loop-by")?));
                index += 3;
            }
            "next" => {
                next = Some(keyword_arg(items, index + 1, "scene :next")?);
                index += 2;
            }
            _ => return Err(format!("unknown scene option ':{}'", key)),
        }
    }

    let mut block_runtime = Runtime::new();
    block_runtime.bpm = runtime.bpm;
    let mut scene_bpm = None;
    while index < items.len() {
        if let Expr::List(form_items) = &items[index] {
            if matches!(form_items.first(), Some(Expr::Symbol(name)) if name == "bpm") {
                expect_arity(form_items, "bpm", 2)?;
                scene_bpm = Some(bpm_value(number_arg(form_items, 1, "bpm")?)?);
                index += 1;
                continue;
            }
        }
        eval_form(&mut block_runtime, &items[index])?;
        index += 1;
    }

    let has_loop_by = loop_by.is_some();
    if let Some(track_id) = steps_of {
        let track = block_runtime
            .tracks
            .get(&track_id)
            .ok_or_else(|| format!("scene :steps-of references unknown track ':{}'", track_id))?;
        steps = inferred_track_steps(track);
    } else if let Some((track_id, count)) = loop_by {
        let track = block_runtime
            .tracks
            .get(&track_id)
            .ok_or_else(|| format!("scene :loop-by references unknown track ':{}'", track_id))?;
        steps = count.saturating_mul(inferred_track_steps(track));
    } else if let Some(bars) = bars {
        let per_bar = if let Some(track_id) = bar_steps_of {
            let track = block_runtime.tracks.get(&track_id).ok_or_else(|| {
                format!(
                    "scene :bar-steps-of references unknown track ':{}'",
                    track_id
                )
            })?;
            inferred_track_steps(track)
        } else {
            bar_steps.unwrap_or(16)
        };
        steps = bars.saturating_mul(per_bar);
    } else if bar_steps.is_some() || bar_steps_of.is_some() {
        return Err("scene :bar-steps requires :bars".to_string());
    } else if !explicit_steps {
        steps = inferred_scene_steps(&block_runtime)?;
    }

    if has_loop_by && next.is_some() && !explicit_repeats {
        repeats = 1;
    }

    runtime.scenes.insert(
        id.clone(),
        Scene {
            id: id.clone(),
            bpm: scene_bpm,
            steps,
            repeats,
            next,
            tracks: block_runtime.tracks,
            post_effects: block_runtime.post_effects,
        },
    );
    Ok(())
}

fn define_sample_track(runtime: &mut Runtime, items: &[Expr]) -> Result<(), String> {
    let id = items.get(1).ok_or("sample requires a track id")?.clone();
    if !matches!(id, Expr::Keyword(_)) {
        return Err("sample track id must be a keyword".to_string());
    }
    let sample_arg = items
        .get(2)
        .ok_or("sample requires a wav path or :sample-data")?
        .clone();
    let inline_options = matches!(sample_arg, Expr::Keyword(_));
    let options_start = if inline_options { 2 } else { 3 };
    validate_sample_options(items, options_start)?;
    let options = normalized_track_options(items, options_start)?;
    let mut track_items = vec![
        Expr::Symbol("d".to_string()),
        id,
        Expr::Keyword("src".to_string()),
        Expr::Keyword("sample".to_string()),
    ];
    if !inline_options {
        track_items.push(Expr::Keyword("sample-path".to_string()));
        track_items.push(sample_arg);
    }
    for (key, value) in [
        ("note", Expr::Symbol("c3".to_string())),
        ("gate", Expr::Number(1.0)),
        ("dur", Expr::Number(1.0)),
        ("amp", Expr::Number(1.0)),
    ] {
        if !track_options_contain(&options, key) {
            track_items.push(Expr::Keyword(key.to_string()));
            track_items.push(value);
        }
    }
    track_items.extend(options.iter().cloned());
    if inline_options
        && !track_options_contain_any(
            &options,
            &[
                "sample-data",
                "sample_data",
                "sample",
                "sample-path",
                "sample_path",
            ],
        )
    {
        return Err("sample requires a wav path or :sample-data".to_string());
    }
    define_track(runtime, &track_items)
}

fn validate_sample_options(items: &[Expr], start: usize) -> Result<(), String> {
    let mut index = start;
    while index < items.len() {
        let Expr::Keyword(key) = &items[index] else {
            return Err("sample options must be keyword/value pairs".to_string());
        };
        if unary_track_flag(key) {
            if index + 1 >= items.len() || matches!(items.get(index + 1), Some(Expr::Keyword(_))) {
                index += 1;
                continue;
            }
        }
        if index + 1 >= items.len() {
            return Err(format!("sample :{} requires a value", key));
        }
        index += 2;
    }
    Ok(())
}

fn normalized_track_options(items: &[Expr], start: usize) -> Result<Vec<Expr>, String> {
    let mut options = Vec::new();
    let mut index = start;
    while index < items.len() {
        let Expr::Keyword(key) = &items[index] else {
            return Err("track parameters must be keyword/value pairs".to_string());
        };
        if unary_track_flag(key) {
            if index + 1 >= items.len() || matches!(items.get(index + 1), Some(Expr::Keyword(_))) {
                options.push(Expr::Keyword(key.clone()));
                options.push(Expr::Symbol("true".to_string()));
                index += 1;
                continue;
            }
        }
        let value = items
            .get(index + 1)
            .ok_or_else(|| format!("track parameter ':{}' requires a value", key))?;
        options.push(Expr::Keyword(key.clone()));
        options.push(value.clone());
        index += 2;
    }
    Ok(options)
}

fn track_options_contain(options: &[Expr], key: &str) -> bool {
    options
        .iter()
        .step_by(2)
        .any(|expr| matches!(expr, Expr::Keyword(name) if name == key))
}

fn track_options_contain_any(options: &[Expr], keys: &[&str]) -> bool {
    options
        .iter()
        .step_by(2)
        .any(|expr| matches!(expr, Expr::Keyword(name) if keys.contains(&name.as_str())))
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
    track.step_every.max(1) * gate_length
}

fn play_scene(runtime: &mut Runtime, items: &[Expr], form: &str) -> Result<(), String> {
    if items.len() > 2 {
        return Err(format!("{} expects exactly one scene keyword", form));
    }
    let id = keyword_arg(items, 1, form)?;
    apply_scene(runtime, &id)
}

fn keyword_arg(items: &[Expr], index: usize, form: &str) -> Result<String, String> {
    match items.get(index) {
        Some(Expr::Keyword(value)) => Ok(value.clone()),
        _ => Err(format!("{} requires a keyword argument", form)),
    }
}

fn scene_option_canonical_key(key: &str) -> &str {
    match key {
        "repeat" | "repeats" | "times" | "loop" => "repeat",
        "steps" | "length" | "bars" | "steps-of" | "length-of" | "loop-by" => "steps",
        "bar-steps" | "bar-length" | "bar-steps-of" | "bar-length-of" => "bar-steps",
        other => other,
    }
}

fn track_param_canonical_key(key: &str) -> &str {
    match key {
        "detune" | "detune-cents" => "detune",
        "pulse-width" | "pulse_width" | "pw" => "pulse-width",
        "morph" | "morph-pos" | "morph_pos" => "morph",
        "unison-detune" | "unison_detune" => "unison-detune",
        "unison-spread" | "unison_spread" | "spread" => "unison-spread",
        "fm-ratio" | "fm_ratio" => "fm-ratio",
        "fm-depth" | "fm_depth" => "fm-depth",
        "sample" | "sample-path" | "sample_path" | "sample-data" | "sample_data" => "sample",
        "choke" | "cut" => "choke",
        other => other,
    }
}

fn unary_track_flag(key: &str) -> bool {
    matches!(key, "off" | "choke" | "cut")
}

fn set_track_flag(
    runtime: &mut Runtime,
    items: &[Expr],
    form: &str,
    value: bool,
    solo: bool,
) -> Result<(), String> {
    expect_arity(items, form, 2)?;
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
        note_chords: vec![vec![note_freq("c3").unwrap()]],
        note_mode: NoteMode::Step,
        gates: vec![true],
        gate_subdivisions: vec![vec![true]],
        gate_cells: vec![vec![GateCell::Static(true)]],
        gate_holds: vec![vec![0]],
        gate_loop_start: 0,
        step_every: 1,
        step_offset: 0,
        drunk: 0.0,
        amp: 0.2,
        dur_seconds: 0.12,
        param_patterns: ParamPatterns::default(),
        effects: Vec::new(),
        sample_data: Vec::new(),
        choke: false,
        muted: false,
        solo: false,
    };
    if let Some(previous) = runtime.tracks.get(id) {
        track.muted = previous.muted;
        track.solo = previous.solo;
    }

    let mut index = 2;
    let mut seen_parameters = HashSet::new();
    while index < items.len() {
        let Expr::Keyword(key) = &items[index] else {
            return Err("track parameters must be keyword/value pairs".to_string());
        };
        let canonical_key = track_param_canonical_key(key);
        if !seen_parameters.insert(canonical_key) {
            return Err(format!("duplicate track parameter ':{}'", key));
        }
        if unary_track_flag(key) {
            if index + 1 >= items.len() || matches!(items.get(index + 1), Some(Expr::Keyword(_))) {
                match key.as_str() {
                    "off" => track.muted = true,
                    "choke" | "cut" => track.choke = true,
                    _ => {}
                }
                index += 1;
                continue;
            }
        }
        let value = items
            .get(index + 1)
            .ok_or_else(|| format!("track parameter ':{}' requires a value", key))?;
        if null_value(value) {
            index += 2;
            continue;
        }
        match key.as_str() {
            "src" => track.waveform = waveform(value)?,
            "note" => {
                let (notes, note_chords, mode) = note_pattern(value)?;
                track.notes = notes;
                track.note_chords = note_chords;
                track.note_mode = mode;
            }
            "gate" => {
                let gate_pattern = gate_pattern(value)?;
                track.gate_subdivisions = gate_pattern.gates;
                track.gate_cells = gate_pattern.cells;
                track.gate_holds = gate_pattern.holds;
                track.gate_loop_start = gate_pattern.loop_start;
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
                    key,
                )?;
            }
            "phase" => {
                set_f32_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.phase,
                    &mut track.oscillator.phase,
                    |value| value.rem_euclid(1.0),
                    key,
                )?;
            }
            "pulse-width" | "pulse_width" | "pw" => {
                set_bounded_f32_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.pulse_width,
                    &mut track.oscillator.pulse_width,
                    0.01,
                    0.99,
                    key,
                )?;
            }
            "morph" | "morph-pos" | "morph_pos" => {
                set_bounded_f32_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.morph_pos,
                    &mut track.oscillator.morph_pos,
                    0.0,
                    1.0,
                    key,
                )?;
            }
            "gain" => {
                set_bounded_f32_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.gain,
                    &mut track.oscillator.gain,
                    0.0,
                    2.0,
                    key,
                )?;
            }
            "unison" => {
                set_bounded_usize_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.unison,
                    &mut track.oscillator.unison,
                    1,
                    10,
                    "unison",
                )?;
            }
            "unison-detune" | "unison_detune" => {
                set_bounded_f32_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.unison_detune,
                    &mut track.oscillator.unison_detune,
                    0.0,
                    100.0,
                    key,
                )?;
            }
            "unison-spread" | "unison_spread" | "spread" => {
                set_bounded_f32_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.unison_spread,
                    &mut track.oscillator.unison_spread,
                    0.0,
                    1.0,
                    key,
                )?;
            }
            "fm-ratio" | "fm_ratio" => {
                set_min_f32_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.fm_ratio,
                    &mut track.oscillator.fm_ratio,
                    0.01,
                    key,
                )?;
            }
            "fm-depth" | "fm_depth" => {
                set_bounded_f32_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.fm_depth,
                    &mut track.oscillator.fm_depth,
                    0.0,
                    32.0,
                    key,
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
            "every" => track.step_every = positive_usize_value(value, "every")?,
            "offset" => track.step_offset = usize_value(value, "offset")?,
            "drunk" => track.drunk = drunk_value(value)?,
            "off" => track.muted = bool_value(value, key)?,
            "choke" | "cut" => track.choke = bool_value(value, key)?,
            "amp" => {
                set_bounded_f32_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.amp,
                    &mut track.amp,
                    0.0,
                    1.0,
                    key,
                )?;
            }
            "dur" => {
                set_bounded_f32_param_pattern_or_scalar(
                    value,
                    &mut track.param_patterns.dur_seconds,
                    &mut track.dur_seconds,
                    0.005,
                    4.0,
                    key,
                )?;
            }
            "fx" => track.effects = effect_chain(value)?,
            _ => return Err(format!("unknown track parameter ':{}'", key)),
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

fn bpm_value(value: f32) -> Result<f32, String> {
    if (20.0..=320.0).contains(&value) {
        Ok(value)
    } else {
        Err(format!(
            "bpm must be between 20 and 320, got {}",
            trim_float(value)
        ))
    }
}

fn trim_float(value: f32) -> String {
    let text = value.to_string();
    text.strip_suffix(".0").unwrap_or(&text).to_string()
}

fn number_value(expr: &Expr) -> Result<f32, String> {
    match expr {
        Expr::Number(value) => Ok(*value),
        Expr::Symbol(name) => note_freq(name).ok_or_else(|| format!("unknown symbol '{}'", name)),
        Expr::Vector(values) if values.len() == 1 => number_value(&values[0]),
        Expr::Vector(values) => Err(format!(
            "expected one number or note, got vector with {} values",
            values.len()
        )),
        _ => Err("expected number or note".to_string()),
    }
}

fn null_value(expr: &Expr) -> bool {
    matches!(expr, Expr::Symbol(name) if matches!(name.as_str(), "nil" | "null"))
}

fn bool_value(expr: &Expr, name: &str) -> Result<bool, String> {
    match expr {
        Expr::Symbol(value) if value == "true" => Ok(true),
        Expr::Symbol(value) if value == "false" => Ok(false),
        Expr::Number(value) => Ok(*value != 0.0),
        _ => Err(format!("{} must be true or false", name)),
    }
}

fn loop_true_value(expr: &Expr) -> Result<(), String> {
    if bool_value(expr, "loop")? {
        Ok(())
    } else {
        Err("scene :loop only accepts true; use :repeat N for finite scenes".to_string())
    }
}

fn drunk_value(expr: &Expr) -> Result<f32, String> {
    let value = number_value(expr)?;
    if !(0.0..=100.0).contains(&value) {
        return Err(format!(
            "drunk must be between 0 and 1, or 0 and 100 percent, got {}",
            trim_float(value)
        ));
    }
    Ok(if value > 1.0 { value / 100.0 } else { value })
}

fn numeric_param_pattern(
    expr: &Expr,
    normalize: fn(f32) -> f32,
) -> Result<Option<Vec<f32>>, String> {
    match expr {
        Expr::List(items) if matches!(items.first(), Some(Expr::Symbol(name)) if numeric_pattern_form(name)) => {
            Ok(Some(
                number_pattern(expr, false)?
                    .into_iter()
                    .map(normalize)
                    .collect(),
            ))
        }
        Expr::List(items) if matches!(items.first(), Some(Expr::Symbol(name)) if name == "rev" || name == "reverse") => {
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

fn numeric_pattern_form(name: &str) -> bool {
    matches!(name, "p" | "s" | "g" | "gs" | "gate-seq" | "gate_seq")
}

fn numeric_pattern_mode(name: &str) -> Option<EffectParamPatternMode> {
    match name {
        "p" => Some(EffectParamPatternMode::Step),
        "s" => Some(EffectParamPatternMode::Hit),
        "g" | "gs" | "gate-seq" | "gate_seq" => Some(EffectParamPatternMode::Gate),
        _ => None,
    }
}

fn numeric_effect_param_pattern(
    expr: &Expr,
) -> Result<Option<(EffectParamPatternMode, Vec<f32>)>, String> {
    match expr {
        Expr::List(items) => {
            let Some(Expr::Symbol(name)) = items.first() else {
                return Ok(None);
            };
            if let Some(mode) = numeric_pattern_mode(name) {
                return Ok(Some((mode, number_pattern(expr, false)?)));
            }
            if name == "rev" || name == "reverse" {
                return Ok(Some((
                    EffectParamPatternMode::Step,
                    number_pattern(expr, false)?,
                )));
            }
            Ok(None)
        }
        Expr::Vector(_) => Ok(Some((
            EffectParamPatternMode::Step,
            number_pattern(expr, false)?,
        ))),
        _ => Ok(None),
    }
}

fn effect_param_patterns(
    items: &[Expr],
    form_name: &str,
) -> Result<Vec<EffectParamPattern>, String> {
    let mut patterns = Vec::new();
    let mut index = 1;
    while index + 1 < items.len() {
        let Expr::Keyword(key) = &items[index] else {
            index += 1;
            continue;
        };
        let canonical_key = effect_param_canonical_key(form_name, key).to_string();
        if let Some((mode, values)) = numeric_effect_param_pattern(&items[index + 1])
            .map_err(|error| format!(":{} {}", key, error))?
        {
            if values.is_empty() {
                return Err(format!(":{} pattern cannot be empty", key));
            }
            for value in &values {
                let scalar = Expr::Number(*value);
                validate_common_effect_param_value(form_name, key, &scalar)?;
                validate_effect_specific_param_value(form_name, key, &scalar)?;
            }
            patterns.push(EffectParamPattern {
                key: canonical_key,
                mode,
                values,
            });
        }
        index += 2;
    }
    Ok(patterns)
}

fn set_f32_param_pattern_or_scalar(
    expr: &Expr,
    pattern: &mut Option<Vec<f32>>,
    scalar: &mut f32,
    normalize: fn(f32) -> f32,
    name: &str,
) -> Result<(), String> {
    if let Some(values) =
        numeric_param_pattern(expr, normalize).map_err(|error| format!(":{} {}", name, error))?
    {
        *pattern = Some(values);
    } else {
        *scalar = normalize(number_value(expr).map_err(|error| format!(":{} {}", name, error))?);
        *pattern = None;
    }
    Ok(())
}

fn set_bounded_f32_param_pattern_or_scalar(
    expr: &Expr,
    pattern: &mut Option<Vec<f32>>,
    scalar: &mut f32,
    min: f32,
    max: f32,
    name: &str,
) -> Result<(), String> {
    if let Some(values) = numeric_param_pattern(expr, |value| value)
        .map_err(|error| format!(":{} {}", name, error))?
    {
        *pattern = Some(
            values
                .into_iter()
                .map(|value| {
                    bounded_f32_value(value, min, max, name)
                        .map_err(|error| format!(":{} {}", name, error))
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
    } else {
        *scalar = bounded_f32_value(
            number_value(expr).map_err(|error| format!(":{} {}", name, error))?,
            min,
            max,
            name,
        )
        .map_err(|error| format!(":{} {}", name, error))?;
        *pattern = None;
    }
    Ok(())
}

fn set_min_f32_param_pattern_or_scalar(
    expr: &Expr,
    pattern: &mut Option<Vec<f32>>,
    scalar: &mut f32,
    min: f32,
    name: &str,
) -> Result<(), String> {
    if let Some(values) = numeric_param_pattern(expr, |value| value)
        .map_err(|error| format!(":{} {}", name, error))?
    {
        *pattern = Some(
            values
                .into_iter()
                .map(|value| {
                    min_f32_value(value, min, name).map_err(|error| format!(":{} {}", name, error))
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
    } else {
        *scalar = min_f32_value(
            number_value(expr).map_err(|error| format!(":{} {}", name, error))?,
            min,
            name,
        )
        .map_err(|error| format!(":{} {}", name, error))?;
        *pattern = None;
    }
    Ok(())
}

fn bounded_f32_value(value: f32, min: f32, max: f32, name: &str) -> Result<f32, String> {
    if (min..=max).contains(&value) {
        Ok(value)
    } else {
        Err(format!(
            "{} must be between {} and {}, got {}",
            name,
            trim_float(min),
            trim_float(max),
            trim_float(value)
        ))
    }
}

fn min_f32_value(value: f32, min: f32, name: &str) -> Result<f32, String> {
    if value >= min {
        Ok(value)
    } else {
        Err(format!(
            "{} must be at least {}, got {}",
            name,
            trim_float(min),
            trim_float(value)
        ))
    }
}

fn set_bounded_usize_param_pattern_or_scalar(
    expr: &Expr,
    pattern: &mut Option<Vec<usize>>,
    scalar: &mut usize,
    min: usize,
    max: usize,
    name: &str,
) -> Result<(), String> {
    if let Some(values) = numeric_param_pattern(expr, |value| value)
        .map_err(|error| format!(":{} {}", name, error))?
    {
        *pattern = Some(
            values
                .into_iter()
                .map(|value| {
                    usize_from_f32(value, name)
                        .and_then(|value| bounded_usize_value(value, min, max, name))
                        .map_err(|error| format!(":{} {}", name, error))
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
    } else {
        *scalar = bounded_usize_value(usize_value(expr, name)?, min, max, name)?;
        *pattern = None;
    }
    Ok(())
}

fn usize_value(expr: &Expr, name: &str) -> Result<usize, String> {
    let value = numeric_only(expr)?;
    usize_from_f32(value, name)
}

fn usize_from_f32(value: f32, name: &str) -> Result<usize, String> {
    if value < 0.0 || value.fract() != 0.0 {
        return Err(format!("{} must be a non-negative integer", name));
    }
    Ok(value as usize)
}

fn bounded_usize_value(value: usize, min: usize, max: usize, name: &str) -> Result<usize, String> {
    if (min..=max).contains(&value) {
        Ok(value)
    } else {
        Err(format!(
            "{} must be between {} and {}, got {}",
            name, min, max, value
        ))
    }
}

fn positive_usize_value(expr: &Expr, name: &str) -> Result<usize, String> {
    let value = usize_value(expr, name)?;
    if value == 0 {
        return Err(format!("{} must be greater than zero", name));
    }
    Ok(value)
}

fn harmonic_values(expr: &Expr) -> Result<[f32; 8], String> {
    let Expr::Vector(items) = expr else {
        return Err("harmonics must be a vector".to_string());
    };
    if items.len() > 8 {
        return Err(format!(
            "harmonics accepts at most 8 values, got {}",
            items.len()
        ));
    }
    let mut harmonics = OscillatorParams::default().harmonics;
    for (idx, item) in items.iter().enumerate() {
        harmonics[idx] = bounded_f32_value(number_value(item)?, 0.0, 2.0, "harmonics")?;
    }
    Ok(harmonics)
}

fn sample_values(expr: &Expr) -> Result<Vec<f32>, String> {
    let Expr::Vector(items) = expr else {
        return Err("sample-data must be a vector".to_string());
    };
    non_empty_sample_data(
        items
            .iter()
            .map(number_value)
            .collect::<Result<Vec<_>, _>>()?,
    )
}

fn non_empty_sample_data(samples: Vec<f32>) -> Result<Vec<f32>, String> {
    if samples.is_empty() {
        Err("sample-data requires at least one value".to_string())
    } else {
        Ok(samples)
    }
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
        return non_empty_sample_data(samples);
    }
    non_empty_sample_data(
        samples
            .chunks(channels)
            .map(|frame| frame.iter().copied().sum::<f32>() / frame.len() as f32)
            .collect(),
    )
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
        Expr::Vector(items) => {
            let mut effects = Vec::new();
            for item in items {
                effects.extend(track_effects(item)?);
            }
            Ok(effects)
        }
        Expr::List(_) => match track_effect(expr) {
            Some(effect) => Ok(vec![effect?]),
            None => Ok(Vec::new()),
        },
        _ => Err("fx must be a vector of effect forms".to_string()),
    }
}

fn track_effects(expr: &Expr) -> Result<Vec<TrackEffect>, String> {
    if effect_pattern_form(expr) {
        return effect_pattern_chain(expr);
    }
    match track_effect(expr) {
        Some(effect) => effect.map(|effect| vec![effect]),
        None => Ok(Vec::new()),
    }
}

fn effect_pattern_form(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::List(items)
            if matches!(
                items.first(),
                Some(Expr::Symbol(name))
                    if numeric_pattern_form(name) || matches!(name.as_str(), "then" | "times" | "rev" | "reverse")
            )
    )
}

fn effect_pattern_chain(expr: &Expr) -> Result<Vec<TrackEffect>, String> {
    let slots = effect_pattern_slots(expr, "fx")?;
    let pattern_len = slots.len();
    let mut effects = Vec::new();
    for (index, slot) in slots.into_iter().enumerate() {
        let mut effect = match track_effect(&slot) {
            Some(effect) => effect?,
            None => continue,
        };
        effect.gate_subdivisions = Some(effect_pattern_slot_gates(
            pattern_len,
            index,
            effect.gate_subdivisions.take(),
        ));
        effects.push(effect);
    }
    Ok(effects)
}

fn effect_pattern_slot_gates(
    pattern_len: usize,
    active_index: usize,
    inner: Option<Vec<Vec<bool>>>,
) -> Vec<Vec<bool>> {
    let mut gates = vec![vec![false]; pattern_len];
    gates[active_index] = inner
        .and_then(|inner_gates| inner_gates.first().cloned())
        .unwrap_or_else(|| vec![true]);
    gates
}

fn effect_pattern_slots(expr: &Expr, name: &str) -> Result<Vec<Expr>, String> {
    match expr {
        Expr::Vector(values) => Ok(values.clone()),
        Expr::List(items) => {
            let Some(Expr::Symbol(form_name)) = items.first() else {
                if items.len() == 1 {
                    return effect_pattern_slots(&items[0], name);
                }
                return Err(format!("{} requires effect forms", name));
            };
            match form_name.as_str() {
                "times" => {
                    let count_expr = items.get(1).ok_or("times requires a count")?;
                    let source = items.get(2).ok_or("times requires a pattern")?;
                    if items.len() > 3 {
                        return Err("times expects count and one pattern".to_string());
                    }
                    let count = positive_usize_value(count_expr, "times")?;
                    let pattern = effect_pattern_slots(source, "times")?;
                    let mut slots = Vec::with_capacity(pattern.len() * count);
                    for _ in 0..count {
                        slots.extend(pattern.iter().cloned());
                    }
                    Ok(slots)
                }
                "then" => {
                    if items.len() < 3 {
                        return Err("then expects at least two patterns".to_string());
                    }
                    let mut slots = Vec::new();
                    for stage in items.iter().skip(1) {
                        slots.extend(effect_pattern_slots(stage, "then")?);
                    }
                    Ok(slots)
                }
                "rev" | "reverse" => {
                    if items.len() > 2 {
                        return Err("reverse expects one pattern".to_string());
                    }
                    let source = items.get(1).ok_or("reverse requires a pattern")?;
                    let mut slots = effect_pattern_slots(source, "reverse")?;
                    slots.reverse();
                    Ok(slots)
                }
                wrapper if numeric_pattern_form(wrapper) => {
                    let Some(source) = items.get(1) else {
                        return Err(format!("{} requires a pattern", wrapper));
                    };
                    if items.len() > 2 {
                        if items
                            .iter()
                            .skip(2)
                            .any(|item| matches!(item, Expr::Symbol(name) if name == "then"))
                        {
                            return Err(
                                format!(
                                    "{} wraps exactly one pattern; use ({} (then A B)) instead of ({} A then B)",
                                    wrapper, wrapper, wrapper
                                )
                                    .to_string(),
                            );
                        }
                        return Err(format!("{} expects one pattern", wrapper));
                    }
                    effect_pattern_slots(source, wrapper)
                }
                _ => Ok(vec![expr.clone()]),
            }
        }
        _ => Err(format!("{} requires effect forms", name)),
    }
}

fn track_effect(expr: &Expr) -> Option<Result<TrackEffect, String>> {
    let Expr::List(items) = expr else {
        return Some(Err("effect must be a form".to_string()));
    };
    let Some(Expr::Symbol(name)) = items.first() else {
        return Some(effect_spec(expr).map(|spec| TrackEffect {
            spec,
            gate_subdivisions: None,
            param_patterns: Vec::new(),
        }));
    };

    if name != "on" {
        if all_effect_params_null(items) {
            if let Some(keys) = live_effect_param_keys(name) {
                if let Err(error) = validate_effect_args(items, name, keys) {
                    return Some(Err(error));
                }
            }
            return None;
        }
        return Some(effect_spec(expr).and_then(|spec| {
            Ok(TrackEffect {
                spec,
                gate_subdivisions: None,
                param_patterns: effect_param_patterns(items, name)?,
            })
        }));
    }

    let mut gate_subdivisions = None;
    let mut effect: Option<Option<(EffectSpec, Vec<EffectParamPattern>)>> = None;
    let mut index = 1;
    while index < items.len() {
        match &items[index] {
            Expr::Keyword(key) if key == "gate" => {
                if gate_subdivisions.is_some() {
                    return Some(Err("on expects only one :gate pattern".to_string()));
                }
                let value = items
                    .get(index + 1)
                    .ok_or("on :gate requires a gate pattern")
                    .map_err(String::from);
                let Ok(value) = value else {
                    return Some(Err(value.unwrap_err()));
                };
                match gate_subdivision_pattern(value) {
                    Ok(pattern) => gate_subdivisions = Some(pattern),
                    Err(error) => return Some(Err(error)),
                }
                index += 2;
            }
            form @ Expr::List(_) => {
                if effect.is_some() {
                    return Some(Err("on expects exactly one effect form".to_string()));
                }
                let Expr::List(effect_items) = form else {
                    unreachable!()
                };
                if all_effect_params_null(effect_items) {
                    if let Some(Expr::Symbol(effect_name)) = effect_items.first() {
                        if let Some(keys) = live_effect_param_keys(effect_name) {
                            if let Err(error) =
                                validate_effect_args(effect_items, effect_name, keys)
                            {
                                return Some(Err(error));
                            }
                        }
                    }
                    effect = Some(None);
                } else {
                    match effect_spec(form) {
                        Ok(spec) => {
                            let param_patterns = match effect_items.first() {
                                Some(Expr::Symbol(effect_name)) => {
                                    match effect_param_patterns(effect_items, effect_name) {
                                        Ok(patterns) => patterns,
                                        Err(error) => return Some(Err(error)),
                                    }
                                }
                                _ => Vec::new(),
                            };
                            effect = Some(Some((spec, param_patterns)));
                        }
                        Err(error) => return Some(Err(error)),
                    }
                }
                index += 1;
            }
            _ => {
                return Some(Err(
                    "on expects :gate PATTERN followed by one effect form".to_string()
                ));
            }
        }
    }

    match effect {
        Some(Some((spec, param_patterns))) => Some(Ok(TrackEffect {
            spec,
            gate_subdivisions,
            param_patterns,
        })),
        Some(None) => None,
        None => Some(Err("on requires an effect form".to_string())),
    }
}

fn offline_effect_chain(expr: &Expr) -> Result<Vec<OfflineEffectSpec>, String> {
    match expr {
        Expr::Vector(items) => items.iter().filter_map(offline_effect).collect(),
        Expr::List(_) => match offline_effect(expr) {
            Some(effect) => Ok(vec![effect?]),
            None => Ok(Vec::new()),
        },
        _ => Err("post-fx must be a vector of effect forms".to_string()),
    }
}

fn offline_effect(expr: &Expr) -> Option<Result<OfflineEffectSpec, String>> {
    match expr {
        Expr::List(items) if all_effect_params_null(items) => {
            if let Some(Expr::Symbol(name)) = items.first() {
                if let Some(keys) = offline_effect_param_keys(name) {
                    if let Err(error) = validate_effect_args(items, name, keys) {
                        return Some(Err(error));
                    }
                } else if let Some(keys) = live_effect_param_keys(name) {
                    if let Err(error) = validate_effect_args(items, name, keys) {
                        return Some(Err(error));
                    }
                }
            }
            None
        }
        _ => Some(offline_effect_spec(expr)),
    }
}

fn all_effect_params_null(items: &[Expr]) -> bool {
    let mut saw_param = false;
    let mut index = 1;
    while index + 1 < items.len() {
        if matches!(items[index], Expr::Keyword(_)) {
            saw_param = true;
            if !null_value(&items[index + 1]) {
                return false;
            }
        } else {
            return false;
        }
        index += 2;
    }
    saw_param && index == items.len()
}

fn offline_effect_spec(expr: &Expr) -> Result<OfflineEffectSpec, String> {
    let Expr::List(items) = expr else {
        return Err("offline effect must be a form".to_string());
    };
    let Some(Expr::Symbol(name)) = items.first() else {
        return Err("offline effect form must start with a symbol".to_string());
    };
    let name = name.as_str();
    if let Some(keys) = offline_effect_param_keys(name) {
        validate_effect_args(items, name, keys)?;
    }
    match name {
        "reverse" => Ok(OfflineEffectSpec::Reverse {
            mix: number_param(items, "mix", 1.0)?,
        }),
        "tape-stop" => Ok(OfflineEffectSpec::TapeStop {
            duration_pct: number_param(
                items,
                "duration-pct",
                number_param(items, "duration", 0.5)?,
            )?,
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
            side: stereo_side(keyword_param(items, "side")?.as_deref().unwrap_or("right"))?,
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
    if let Some(keys) = live_effect_param_keys(name) {
        validate_effect_args(items, name, keys)?;
    }
    match name {
        "filter" => Ok(EffectSpec::Filter {
            kind: filter_kind(
                keyword_param(items, "type")?
                    .as_deref()
                    .unwrap_or("lowpass"),
            )?,
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
            vowel: vowel(keyword_param(items, "vowel")?.as_deref().unwrap_or("a"))?,
            mix: number_param(items, "mix", 1.0)?,
        }),
        "distort" | "distortion" => Ok(EffectSpec::Distortion {
            kind: distortion_kind(keyword_param(items, "type")?.as_deref().unwrap_or("tanh"))?,
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
            color: bool_param(items, "color", false)?,
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
            limit: matches!(keyword_param(items, "mode")?.as_deref(), Some("limit")),
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
                keyword_param(items, "program")?
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
            kind: filter_kind(
                keyword_param(items, "type")?
                    .as_deref()
                    .unwrap_or("lowpass"),
            )?,
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
            kind: filter_kind(
                keyword_param(items, "type")?
                    .as_deref()
                    .unwrap_or("lowpass"),
            )?,
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

fn offline_effect_param_keys(name: &str) -> Option<&'static [&'static str]> {
    Some(match name {
        "reverse" => &["mix"],
        "tape-stop" => &["duration-pct", "duration"],
        "granular" => &["grain-ms", "density", "spray", "pitch-spread"],
        "granular-stretch" => &["rate", "grain-ms"],
        "spectral-freeze" => &["freeze-pos", "sustain", "mix"],
        "haas" => &["delay-ms", "side"],
        "stereo-widen" | "stereo_widen" => &["width"],
        "stereo-imager" | "stereo_imager" => &["width", "bass-mono-freq"],
        "width-enhance" | "width_enhance" => &["low-width", "high-width", "crossover"],
        "freq-shift" | "freq_shift" => &["shift-hz", "mix"],
        "autopan" | "auto-pan" => &["rate", "depth"],
        "ping-pong-delay" | "ping_pong_delay" | "ping-pong" => &["time", "feedback", "mix"],
        _ => return None,
    })
}

fn live_effect_param_keys(name: &str) -> Option<&'static [&'static str]> {
    Some(match name {
        "filter" => &["type", "cutoff", "res", "resonance", "gain-db", "gain_db"],
        "comb" => &["delay-ms", "delay", "feedback", "mix"],
        "formant" => &["vowel", "mix"],
        "distort" | "distortion" => &["type", "drive"],
        "bitcrush" => &["bits", "bit-depth", "rate", "sample-rate-reduction"],
        "delay" => &["time", "feedback", "mix"],
        "wavefolder" | "fold" => &["folds", "gain", "symmetry"],
        "resonator" => &["freq", "decay", "mix", "harmonics"],
        "lofi" | "lo-fi" => &["amount", "intensity"],
        "vinyl" => &["crackle", "hiss", "wow"],
        "sub-bass" | "subbass" => &["mix"],
        "sidechain" => &["rate", "depth", "shape"],
        "radio" => &["intensity"],
        "telephone" => &["quality"],
        "underwater" => &["depth", "depth-amount"],
        "crystal" => &["brightness", "decay"],
        "dc-remove" | "dc-block" => &[],
        "pitch-shift" => &["semitones", "mix"],
        "harmonizer" => &["interval", "mix"],
        "octaver" => &["octave-up", "octave-down"],
        "shimmer" => &["shift-semitones", "feedback", "mix"],
        "stutter" | "granular-stutter" => &["grain-size-ms", "grain-ms", "repeats", "mix"],
        "glitch" => &["density", "slice-ms"],
        "fade" => &["fade-in-ms", "fade-out-ms", "duration"],
        "adsr" | "asdr" => &[
            "attack", "a", "decay", "d", "sustain", "s", "release", "r", "duration",
        ],
        "doppler" => &["speed", "depth"],
        "maximizer" => &["ceiling", "warmth", "release-ms"],
        "multiband-comp" => &[
            "low-thresh",
            "mid-thresh",
            "high-thresh",
            "crossover-low",
            "crossover-high",
        ],
        "harmonic-enhance" => &["low-harmonics", "high-harmonics", "air"],
        "body" => &["size", "tone", "mix"],
        "warmth" => &["amount"],
        "spatial" => &["room-size", "position", "height"],
        "parallel-comp" | "parallel-compressor" | "ny-comp" => &["threshold", "ratio", "mix"],
        "tremolo" => &["rate", "depth"],
        "chorus" => &["rate", "depth", "voices", "mix"],
        "dimension" => &["mode"],
        "ensemble" => &["voices", "depth", "rate"],
        "ce1-chorus" | "ce-1" => &["rate", "intensity"],
        "re301-chorus" | "re-301-chorus" => &["rate", "depth", "tone"],
        "dimension-d" => &["mode"],
        "h3000" => &["detune-cents", "delay-ms", "feedback", "mix"],
        "flanger" => &["rate", "depth", "feedback", "mix"],
        "phaser" => &["rate", "depth", "stages", "mix"],
        "small-stone" => &["rate", "depth", "feedback", "color"],
        "vibrato" => &["rate", "depth"],
        "ring-mod" | "ringmod" => &["freq", "mix"],
        "arp-ring-mod" => &["freq", "depth", "mix", "diode-curve"],
        "compressor" => &[
            "threshold",
            "ratio",
            "attack",
            "release",
            "makeup",
            "makeup-gain",
        ],
        "fairchild" => &["input-gain", "threshold", "time-constant", "mix"],
        "ssl-comp" => &["threshold", "ratio", "attack-ms", "release-ms", "makeup-db"],
        "dbx160" => &["threshold", "ratio"],
        "la2a" => &["peak-reduction", "mode"],
        "1176" | "urei-1176" => &["input-gain", "ratio", "attack", "release"],
        "limiter" => &["ceiling", "release"],
        "gate" => &["threshold", "attack", "release"],
        "transient" | "transient-shaper" => &["attack-gain", "sustain-gain", "sensitivity"],
        "reverb" => &["decay", "mix"],
        "spring-reverb" => &["decay", "tone", "mix", "drip"],
        "emt-plate" => &["decay", "damping", "mix", "pre-delay-ms"],
        "lexicon-224" => &["size", "decay", "damping", "pre-delay-ms", "mix"],
        "ams-reverb" => &["decay", "damping", "program", "mix"],
        "tube" | "tube-saturation" => &["drive", "gain", "asymmetry"],
        "neve-preamp" => &["gain", "warmth"],
        "marshall-amp" => &["gain", "tone", "presence"],
        "vox-ac30" => &["gain", "treble", "cut"],
        "fender-twin" => &["volume", "gain", "treble", "bass", "reverb-mix"],
        "pultec-eq" | "pultec" => &[
            "low-boost",
            "low-atten",
            "low-freq",
            "high-boost",
            "high-atten",
            "high-freq",
        ],
        "tape" => &["saturation", "input-level", "wow", "flutter"],
        "studer-tape" => &["input-level", "speed", "bias"],
        "exciter" => &["amount", "cutoff"],
        "moog" | "moog-ladder" => &["cutoff", "res", "resonance", "drive"],
        "prophet-filter" => &["cutoff", "res", "resonance"],
        "obxa-filter" => &["cutoff", "res", "resonance", "type"],
        "303" | "303-filter" | "tb303" | "tb-303" => {
            &["cutoff", "res", "resonance", "env-mod", "accent", "decay"]
        }
        "space-echo" | "re201" | "re-201" => &[
            "time",
            "feedback",
            "wow",
            "flutter",
            "tone",
            "spring-mix",
            "mix",
        ],
        "tc2290" | "tc-2290" => &["time-ms", "feedback", "mod-rate", "mod-depth", "mix"],
        "sem-filter" | "sem" => &["cutoff", "res", "resonance", "type"],
        "ms20" | "ms20-filter" => &["cutoff", "res", "resonance"],
        "wasp-filter" => &["cutoff", "res", "resonance"],
        "juno-hpf" => &["cutoff", "res", "resonance"],
        "buchla-lpg" | "lpg" => &["strike", "decay", "res", "resonance"],
        _ => return None,
    })
}

fn validate_effect_args(
    items: &[Expr],
    form_name: &str,
    allowed_keys: &[&str],
) -> Result<(), String> {
    let mut seen_parameters = HashSet::new();
    let mut index = 1;
    while index < items.len() {
        let Expr::Keyword(key) = &items[index] else {
            return Err(format!(
                "{} parameters must be keyword/value pairs",
                form_name
            ));
        };
        if index + 1 >= items.len() {
            return Err(format!("{} :{} requires a value", form_name, key));
        }
        if !allowed_keys.contains(&key.as_str()) {
            return Err(format!("unknown {} parameter ':{}'", form_name, key));
        }
        let canonical_key = effect_param_canonical_key(form_name, key);
        if !seen_parameters.insert(canonical_key) {
            return Err(format!("duplicate {} parameter ':{}'", form_name, key));
        }
        validate_common_effect_param_value(form_name, key, &items[index + 1])?;
        validate_effect_specific_param_value(form_name, key, &items[index + 1])?;
        index += 2;
    }
    Ok(())
}

fn validate_common_effect_param_value(
    form_name: &str,
    key: &str,
    value: &Expr,
) -> Result<(), String> {
    if null_value(value) {
        return Ok(());
    }
    if let Some((_, values)) =
        numeric_effect_param_pattern(value).map_err(|error| format!(":{} {}", key, error))?
    {
        for value in values {
            validate_common_effect_param_value(form_name, key, &Expr::Number(value))?;
        }
        return Ok(());
    }
    if form_name == "h3000" && matches!(key, "delay-ms" | "feedback") {
        return Err(format!(
            "h3000 :{} is not implemented by this port yet; remove it",
            key
        ));
    }
    match key {
        "mix" => {
            let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
            bounded_f32_value(value, 0.0, 1.0, "mix")
                .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
        }
        "feedback" => {
            let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
            bounded_f32_value(value, 0.0, 0.95, "feedback")
                .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
        }
        "res" | "resonance" => {
            let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
            bounded_f32_value(value, 0.0, 1.0, "resonance")
                .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
        }
        _ => {}
    }
    Ok(())
}

fn validate_effect_specific_param_value(
    form_name: &str,
    key: &str,
    value: &Expr,
) -> Result<(), String> {
    if null_value(value) {
        return Ok(());
    }
    if let Some((_, values)) =
        numeric_effect_param_pattern(value).map_err(|error| format!(":{} {}", key, error))?
    {
        for value in values {
            validate_effect_specific_param_value(form_name, key, &Expr::Number(value))?;
        }
        return Ok(());
    }
    match form_name {
        "distort" | "distortion" => match key {
            "drive" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 10.0, "drive")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "bitcrush" => match key {
            "bits" | "bit-depth" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 2.0, 16.0, "bits")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "rate" | "sample-rate-reduction" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 1.0, 128.0, "rate")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "granular" => match key {
            "density" | "spray" | "pitch-spread" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "spectral-freeze" => match key {
            "freeze-pos" | "sustain" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "autopan" | "auto-pan" => match key {
            "depth" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "depth")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "tape-stop" => match key {
            "duration-pct" | "duration" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.1, 1.0, "duration")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "lofi" | "lo-fi" => match key {
            "amount" | "intensity" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "amount")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "vinyl" => match key {
            "crackle" | "hiss" | "wow" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "sidechain" => match key {
            "depth" | "shape" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "radio" => match key {
            "intensity" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "intensity")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "telephone" => match key {
            "quality" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "quality")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "underwater" => match key {
            "depth" | "depth-amount" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "depth")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "crystal" => match key {
            "brightness" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "brightness")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "decay" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 0.95, "decay")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "wavefolder" | "fold" => match key {
            "folds" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 1.0, 8.0, "folds")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "gain" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.1, 12.0, "gain")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "symmetry" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.1, 2.0, "symmetry")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "resonator" => match key {
            "freq" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                min_f32_value(value, 20.0, "freq")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "decay" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "decay")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "harmonics" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 1.0, 16.0, "harmonics")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "maximizer" => match key {
            "warmth" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "warmth")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "release-ms" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                min_f32_value(value, 1.0, "release-ms")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "harmonic-enhance" => match key {
            "low-harmonics" | "high-harmonics" | "air" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "body" => match key {
            "size" | "tone" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "warmth" => match key {
            "amount" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "amount")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "spatial" => match key {
            "room-size" | "position" | "height" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "chorus" => match key {
            "rate" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                min_f32_value(value, 0.01, "rate")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "depth" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0001, 0.05, "depth")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "voices" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 1.0, 8.0, "voices")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "ensemble" => match key {
            "rate" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                min_f32_value(value, 0.01, "rate")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "depth" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0005, 0.05, "depth")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "voices" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 2.0, 12.0, "voices")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "ce1-chorus" | "ce-1" => match key {
            "rate" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.01, 10.0, "rate")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "intensity" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "intensity")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "re301-chorus" | "re-301-chorus" => match key {
            "rate" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.01, 10.0, "rate")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "depth" | "tone" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "phaser" => match key {
            "rate" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.01, 20.0, "rate")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "depth" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "depth")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "stages" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 1.0, 12.0, "stages")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "dimension" | "dimension-d" => match key {
            "mode" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 1.0, 4.0, "mode")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "flanger" => match key {
            "rate" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.01, 20.0, "rate")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "depth" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0001, 0.02, "depth")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "small-stone" => match key {
            "rate" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.01, 20.0, "rate")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "depth" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "depth")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "vibrato" => match key {
            "rate" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                min_f32_value(value, 0.01, "rate")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "depth" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0001, 0.03, "depth")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "tremolo" => match key {
            "rate" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.01, 40.0, "rate")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "depth" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "depth")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "ring-mod" | "ringmod" => match key {
            "freq" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.01, 20_000.0, "freq")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "arp-ring-mod" => match key {
            "freq" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.01, 20_000.0, "freq")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "depth" | "mix" | "diode-curve" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "tube" | "tube-saturation" => match key {
            "drive" | "gain" | "asymmetry" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "tape" => match key {
            "saturation" | "input-level" | "wow" | "flutter" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "studer-tape" => match key {
            "input-level" | "bias" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "speed" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 2.0, "speed")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "exciter" => match key {
            "amount" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "amount")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "fairchild" => match key {
            "input-gain" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "input-gain")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "time-constant" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 1.0, 6.0, "time-constant")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "la2a" => match key {
            "peak-reduction" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "peak-reduction")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "1176" | "urei-1176" => match key {
            "input-gain" | "attack" | "release" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "transient" | "transient-shaper" => match key {
            "attack-gain" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 8.0, "attack-gain")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "sustain-gain" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 4.0, "sustain-gain")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "reverb" => match key {
            "decay" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "decay")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "spring-reverb" => match key {
            "decay" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 4.0, "decay")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "tone" | "drip" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "emt-plate" => match key {
            "decay" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.1, 5.0, "decay")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "damping" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "damping")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "lexicon-224" => match key {
            "size" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.2, 2.0, "size")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "decay" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.1, 8.0, "decay")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "damping" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "damping")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "ams-reverb" => match key {
            "decay" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.1, 5.0, "decay")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "damping" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "damping")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "moog" | "moog-ladder" => match key {
            "drive" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "drive")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "303" | "303-filter" | "tb303" | "tb-303" => match key {
            "env-mod" | "accent" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "buchla-lpg" | "lpg" => match key {
            "strike" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "strike")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "neve-preamp" => match key {
            "gain" | "warmth" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "marshall-amp" => match key {
            "gain" | "tone" | "presence" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "vox-ac30" => match key {
            "gain" | "treble" | "cut" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "fender-twin" => match key {
            "volume" | "gain" | "treble" | "bass" | "reverb-mix" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "pultec-eq" | "pultec" => match key {
            "low-boost" | "low-atten" | "high-boost" | "high-atten" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "space-echo" | "re201" | "re-201" => match key {
            "time" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.02, 2.0, "time")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "wow" | "flutter" | "tone" | "spring-mix" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "tc2290" | "tc-2290" => match key {
            "time-ms" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 1.0, 2_000.0, "time-ms")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "mod-rate" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 20.0, "mod-rate")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "mod-depth" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 0.05, "mod-depth")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "octaver" => match key {
            "octave-up" | "octave-down" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "stutter" | "granular-stutter" => match key {
            "grain-size-ms" | "grain-ms" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 1.0, 500.0, "grain-size-ms")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "repeats" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 1.0, 16.0, "repeats")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "glitch" => match key {
            "density" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "density")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "slice-ms" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 1.0, 500.0, "slice-ms")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "fade" => match key {
            "fade-in-ms" | "fade-out-ms" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                min_f32_value(value, 0.0, key)
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "duration" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                min_f32_value(value, 0.001, "duration")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "adsr" | "asdr" => match key {
            "attack" | "a" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                min_f32_value(value, 0.0, "attack")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "decay" | "d" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                min_f32_value(value, 0.0, "decay")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "sustain" | "s" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "sustain")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "release" | "r" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                min_f32_value(value, 0.0, "release")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "duration" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                min_f32_value(value, 0.001, "duration")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        "doppler" => match key {
            "speed" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.01, 8.0, "speed")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            "depth" => {
                let value = number_value(value).map_err(|error| format!(":{} {}", key, error))?;
                bounded_f32_value(value, 0.0, 1.0, "depth")
                    .map_err(|error| format!("{} :{} {}", form_name, key, error))?;
            }
            _ => {}
        },
        _ => {}
    }
    Ok(())
}

fn effect_param_canonical_key<'a>(form_name: &str, key: &'a str) -> &'a str {
    match form_name {
        "filter" => match key {
            "res" | "resonance" => "resonance",
            "gain-db" | "gain_db" => "gain-db",
            other => other,
        },
        "comb" => match key {
            "delay-ms" | "delay" => "delay-ms",
            other => other,
        },
        "bitcrush" => match key {
            "bits" | "bit-depth" => "bits",
            "rate" | "sample-rate-reduction" => "rate",
            other => other,
        },
        "lofi" | "lo-fi" => match key {
            "amount" | "intensity" => "amount",
            other => other,
        },
        "underwater" => match key {
            "depth" | "depth-amount" => "depth",
            other => other,
        },
        "stutter" | "granular-stutter" => match key {
            "grain-size-ms" | "grain-ms" => "grain-size-ms",
            other => other,
        },
        "adsr" | "asdr" => match key {
            "attack" | "a" => "attack",
            "decay" | "d" => "decay",
            "sustain" | "s" => "sustain",
            "release" | "r" => "release",
            other => other,
        },
        "arp-ring-mod" => match key {
            "depth" | "mix" => "depth",
            other => other,
        },
        "compressor" => match key {
            "makeup" | "makeup-gain" => "makeup",
            other => other,
        },
        "tube" | "tube-saturation" => match key {
            "drive" | "gain" => "drive",
            other => other,
        },
        "fender-twin" => match key {
            "volume" | "gain" => "volume",
            other => other,
        },
        "tape" => match key {
            "saturation" | "input-level" => "saturation",
            other => other,
        },
        "moog" | "moog-ladder" | "prophet-filter" | "obxa-filter" | "303" | "303-filter"
        | "tb303" | "tb-303" | "sem-filter" | "sem" | "ms20" | "ms20-filter" | "wasp-filter"
        | "juno-hpf" | "buchla-lpg" | "lpg" => match key {
            "res" | "resonance" => "resonance",
            other => other,
        },
        "tape-stop" => match key {
            "duration-pct" | "duration" => "duration-pct",
            other => other,
        },
        other => {
            let _ = other;
            key
        }
    }
}

fn keyword_param(items: &[Expr], key: &str) -> Result<Option<String>, String> {
    let mut index = 1;
    while index + 1 < items.len() {
        if matches!(&items[index], Expr::Keyword(value) if value == key) {
            if null_value(&items[index + 1]) {
                return Ok(None);
            }
            return match &items[index + 1] {
                Expr::Keyword(value) | Expr::Symbol(value) => Ok(Some(value.clone())),
                _ => Err(format!(":{} expected keyword or symbol", key)),
            };
        }
        index += 2;
    }
    Ok(None)
}

fn number_param(items: &[Expr], key: &str, default: f32) -> Result<f32, String> {
    let mut index = 1;
    while index + 1 < items.len() {
        if matches!(&items[index], Expr::Keyword(value) if value == key) {
            if null_value(&items[index + 1]) {
                return Ok(default);
            }
            if let Some((_, values)) = numeric_effect_param_pattern(&items[index + 1])
                .map_err(|error| format!(":{} {}", key, error))?
            {
                return values
                    .first()
                    .copied()
                    .ok_or_else(|| format!(":{} pattern cannot be empty", key));
            }
            return number_value(&items[index + 1]).map_err(|error| format!(":{} {}", key, error));
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
            if let Some((_, values)) = numeric_effect_param_pattern(&items[index + 1])
                .map_err(|error| format!(":{} {}", key, error))?
            {
                return values
                    .first()
                    .copied()
                    .map(Some)
                    .ok_or_else(|| format!(":{} pattern cannot be empty", key));
            }
            return number_value(&items[index + 1])
                .map(Some)
                .map_err(|error| format!(":{} {}", key, error));
        }
        index += 2;
    }
    Ok(None)
}

fn bool_param(items: &[Expr], key: &str, default: bool) -> Result<bool, String> {
    let mut index = 1;
    while index + 1 < items.len() {
        if matches!(&items[index], Expr::Keyword(value) if value == key) {
            if null_value(&items[index + 1]) {
                return Ok(default);
            }
            return match &items[index + 1] {
                Expr::Keyword(value) | Expr::Symbol(value) => match value.as_str() {
                    "true" | "on" | "yes" | "1" => Ok(true),
                    "false" | "off" | "no" | "0" => Ok(false),
                    _ => Err(format!(":{} must be true or false", key)),
                },
                Expr::Number(value) if *value == 0.0 => Ok(false),
                Expr::Number(value) if *value == 1.0 => Ok(true),
                Expr::Number(_) => Err(format!(":{} must be true or false", key)),
                _ => Err(format!(":{} must be true or false", key)),
            };
        }
        index += 2;
    }
    Ok(default)
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

fn note_pattern(expr: &Expr) -> Result<(Vec<f32>, Vec<Vec<f32>>, NoteMode), String> {
    match expr {
        Expr::List(items) if matches!(items.first(), Some(Expr::Symbol(name)) if name == "p") => {
            let chords = note_chord_wrapped_pattern_values(items, "p")?;
            Ok((flatten_note_chords(&chords), chords, NoteMode::Step))
        }
        Expr::List(items) if matches!(items.first(), Some(Expr::Symbol(name)) if name == "s") => {
            let chords = note_chord_wrapped_pattern_values(items, "s")?;
            Ok((flatten_note_chords(&chords), chords, NoteMode::Hit))
        }
        Expr::List(items) if matches!(items.first(), Some(Expr::Symbol(name)) if name == "g" || name == "gs" || name == "gate-seq" || name == "gate_seq") =>
        {
            let name = match items.first() {
                Some(Expr::Symbol(name)) => name.as_str(),
                _ => "gs",
            };
            let chords = note_chord_wrapped_pattern_values(items, name)?;
            Ok((flatten_note_chords(&chords), chords, NoteMode::Tick))
        }
        Expr::List(items) if matches!(items.first(), Some(Expr::Symbol(name)) if name == "rev" || name == "reverse") =>
        {
            if items.len() > 2 {
                return Err("reverse expects one pattern".to_string());
            }
            let source = items.get(1).ok_or("reverse requires a pattern")?;
            let (_, mut chords, mode) = note_pattern(source)?;
            chords.reverse();
            Ok((flatten_note_chords(&chords), chords, mode))
        }
        Expr::Vector(values) => {
            let chords = note_chords_from_values(values)?;
            Ok((flatten_note_chords(&chords), chords, NoteMode::Hit))
        }
        _ => {
            let values = number_pattern(expr, true)?;
            let chords = values.iter().copied().map(|value| vec![value]).collect();
            Ok((values, chords, NoteMode::Step))
        }
    }
}

fn flatten_note_chords(chords: &[Vec<f32>]) -> Vec<f32> {
    chords.iter().flatten().copied().collect()
}

fn note_chord_wrapped_pattern_values(items: &[Expr], name: &str) -> Result<Vec<Vec<f32>>, String> {
    if items.len() > 2 {
        return Err(format!("{} expects one vector", name));
    }
    let Some(source) = items.get(1) else {
        return Err(format!("{} requires a vector", name));
    };
    note_chord_pattern_values(source, name)
}

fn note_chord_pattern_values(expr: &Expr, name: &str) -> Result<Vec<Vec<f32>>, String> {
    match expr {
        Expr::Vector(values) => note_chords_from_values(values),
        Expr::List(items) => {
            let Some(Expr::Symbol(form_name)) = items.first() else {
                return Err(format!("{} requires a vector", name));
            };
            match form_name.as_str() {
                "times" => {
                    let count_expr = items.get(1).ok_or("times requires a count")?;
                    let source = items.get(2).ok_or("times requires a pattern")?;
                    if items.len() > 3 {
                        return Err("times expects count and one pattern".to_string());
                    }
                    let count = positive_usize_value(count_expr, "times")?;
                    let pattern = note_chord_pattern_values(source, "times")?;
                    let mut chords = Vec::with_capacity(pattern.len() * count);
                    for _ in 0..count {
                        chords.extend(pattern.iter().cloned());
                    }
                    Ok(chords)
                }
                "then" => {
                    if items.len() < 3 {
                        return Err("then expects at least two patterns".to_string());
                    }
                    let mut chords = Vec::new();
                    for stage in items.iter().skip(1) {
                        chords.extend(note_chord_pattern_values(stage, "then")?);
                    }
                    Ok(chords)
                }
                "rev" | "reverse" => {
                    if items.len() > 2 {
                        return Err("reverse expects one pattern".to_string());
                    }
                    let source = items.get(1).ok_or("reverse requires a pattern")?;
                    let mut chords = note_chord_pattern_values(source, "reverse")?;
                    chords.reverse();
                    Ok(chords)
                }
                wrapper if numeric_pattern_form(wrapper) => {
                    note_chord_wrapped_pattern_values(items, wrapper)
                }
                _ => Err(format!("{} requires a vector", name)),
            }
        }
        _ => Err(format!("{} requires a vector", name)),
    }
}

fn note_chords_from_values(values: &[Expr]) -> Result<Vec<Vec<f32>>, String> {
    values
        .iter()
        .map(|value| match value {
            Expr::Vector(notes) => notes.iter().map(number_value).collect(),
            _ => Ok(vec![number_value(value)?]),
        })
        .collect()
}

fn number_pattern(expr: &Expr, notes: bool) -> Result<Vec<f32>, String> {
    match expr {
        Expr::List(items) if matches!(items.first(), Some(Expr::Symbol(name)) if numeric_pattern_form(name)) =>
        {
            let name = match items.first() {
                Some(Expr::Symbol(name)) => name.as_str(),
                _ => "p",
            };
            wrapped_pattern_values(items, notes, name)
        }
        Expr::List(items) if matches!(items.first(), Some(Expr::Symbol(name)) if name == "rev" || name == "reverse") =>
        {
            if items.len() > 2 {
                return Err("reverse expects one pattern".to_string());
            }
            let source = items.get(1).ok_or("reverse requires a pattern")?;
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

fn wrapped_pattern_values(items: &[Expr], notes: bool, name: &str) -> Result<Vec<f32>, String> {
    if items.len() > 2 {
        return Err(format!("{} expects one vector", name));
    }
    let Some(source) = items.get(1) else {
        return Err(format!("{} requires a vector", name));
    };
    expanded_pattern_values(source, notes, name)
}

fn expanded_pattern_values(expr: &Expr, notes: bool, name: &str) -> Result<Vec<f32>, String> {
    match expr {
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
        Expr::List(items) => {
            let Some(Expr::Symbol(form_name)) = items.first() else {
                return Err(format!("{} requires a vector", name));
            };
            match form_name.as_str() {
                "times" => {
                    let count_expr = items.get(1).ok_or("times requires a count")?;
                    let source = items.get(2).ok_or("times requires a pattern")?;
                    if items.len() > 3 {
                        return Err("times expects count and one pattern".to_string());
                    }
                    let count = positive_usize_value(count_expr, "times")?;
                    let pattern = expanded_pattern_values(source, notes, "times")?;
                    let mut values = Vec::with_capacity(pattern.len() * count);
                    for _ in 0..count {
                        values.extend(pattern.iter().copied());
                    }
                    Ok(values)
                }
                "then" => {
                    if items.len() < 3 {
                        return Err("then expects at least two patterns".to_string());
                    }
                    let mut values = Vec::new();
                    for stage in items.iter().skip(1) {
                        values.extend(expanded_pattern_values(stage, notes, "then")?);
                    }
                    Ok(values)
                }
                "rev" | "reverse" => {
                    if items.len() > 2 {
                        return Err("reverse expects one pattern".to_string());
                    }
                    let source = items.get(1).ok_or("reverse requires a pattern")?;
                    let mut values = expanded_pattern_values(source, notes, "reverse")?;
                    values.reverse();
                    Ok(values)
                }
                wrapper if numeric_pattern_form(wrapper) => {
                    wrapped_pattern_values(items, notes, wrapper)
                }
                _ => Err(format!("{} requires a vector", name)),
            }
        }
        _ => Err(format!("{} requires a vector", name)),
    }
}

#[derive(Clone, Debug)]
struct ParsedGatePattern {
    gates: Vec<Vec<bool>>,
    cells: Vec<Vec<GateCell>>,
    holds: Vec<Vec<usize>>,
    loop_start: usize,
}

fn parsed_gate_pattern(
    gates: Vec<Vec<bool>>,
    cells: Vec<Vec<GateCell>>,
    holds: Vec<Vec<usize>>,
    loop_start: usize,
) -> Result<ParsedGatePattern, String> {
    validate_gate_holds(&gates, &holds)?;
    Ok(ParsedGatePattern {
        gates,
        cells,
        holds,
        loop_start,
    })
}

fn static_gate_cells_like(gates: &[Vec<bool>]) -> Vec<Vec<GateCell>> {
    gates
        .iter()
        .map(|step| step.iter().copied().map(GateCell::Static).collect())
        .collect()
}

fn gate_pattern(expr: &Expr) -> Result<ParsedGatePattern, String> {
    if let Expr::List(items) = expr {
        if let Some(Expr::Symbol(name)) = items.first() {
            match name.as_str() {
                "euclid" => {
                    if items.len() != 3 {
                        return Err("euclid expects pulses and steps".to_string());
                    }
                    let pulses = usize_value(&items[1], "euclid pulses")?;
                    let steps = positive_usize_value(&items[2], "euclid steps")?;
                    let gates: Vec<Vec<bool>> = sequencer::euclid(pulses, steps, 0)
                        .into_iter()
                        .map(|gate| vec![gate])
                        .collect();
                    let cells = static_gate_cells_like(&gates);
                    let holds = empty_holds_like(&gates);
                    return parsed_gate_pattern(gates, cells, holds, 0);
                }
                "euclid-rot" => {
                    if items.len() != 4 {
                        return Err("euclid-rot expects pulses, steps, and rotation".to_string());
                    }
                    let pulses = usize_value(&items[1], "euclid-rot pulses")?;
                    let steps = positive_usize_value(&items[2], "euclid-rot steps")?;
                    let rotation = usize_value(&items[3], "euclid-rot rotation")?;
                    let gates: Vec<Vec<bool>> = sequencer::euclid(pulses, steps, rotation)
                        .into_iter()
                        .map(|gate| vec![gate])
                        .collect();
                    let cells = static_gate_cells_like(&gates);
                    let holds = empty_holds_like(&gates);
                    return parsed_gate_pattern(gates, cells, holds, 0);
                }
                "rev" | "reverse" => {
                    if items.len() > 2 {
                        return Err("reverse expects one pattern".to_string());
                    }
                    let source = items.get(1).ok_or("reverse requires a pattern")?;
                    let mut pattern = gate_pattern(source)?;
                    pattern.gates.reverse();
                    pattern.cells.reverse();
                    pattern.holds.reverse();
                    pattern.loop_start = 0;
                    validate_gate_holds(&pattern.gates, &pattern.holds)?;
                    return Ok(pattern);
                }
                "times" => {
                    let count_expr = items.get(1).ok_or("times requires a count")?;
                    let source = items.get(2).ok_or("times requires a pattern")?;
                    if items.len() > 3 {
                        return Err("times expects count and one pattern".to_string());
                    }
                    let count = positive_usize_value(count_expr, "times")?;
                    let pattern = gate_pattern(source)?;
                    let mut gates = Vec::with_capacity(pattern.gates.len() * count);
                    let mut cells = Vec::with_capacity(pattern.cells.len() * count);
                    let mut holds = Vec::with_capacity(pattern.holds.len() * count);
                    for _ in 0..count {
                        gates.extend(pattern.gates.iter().cloned());
                        cells.extend(pattern.cells.iter().cloned());
                        holds.extend(pattern.holds.iter().cloned());
                    }
                    return parsed_gate_pattern(gates, cells, holds, 0);
                }
                "then" => {
                    if items.len() < 3 {
                        return Err("then expects at least two patterns".to_string());
                    }
                    let mut gates = Vec::new();
                    let mut cells = Vec::new();
                    let mut holds = Vec::new();
                    let mut loop_start = 0;
                    let last_stage_is_times = items.last().is_some_and(|stage| {
                        matches!(
                            stage,
                            Expr::List(stage_items)
                                if matches!(stage_items.first(), Some(Expr::Symbol(name)) if name == "times")
                        )
                    });
                    for (idx, stage_expr) in items.iter().skip(1).enumerate() {
                        if !last_stage_is_times && idx == items.len() - 2 {
                            loop_start = gates.len();
                        }
                        let stage = gate_pattern(stage_expr)?;
                        gates.extend(stage.gates);
                        cells.extend(stage.cells);
                        holds.extend(stage.holds);
                    }
                    return parsed_gate_pattern(gates, cells, holds, loop_start);
                }
                name if numeric_pattern_form(name) => {
                    let Some(source) = items.get(1) else {
                        return Err(format!("{} requires a pattern", name));
                    };
                    if items.len() > 2 {
                        if items
                            .iter()
                            .skip(2)
                            .any(|item| matches!(item, Expr::Symbol(name) if name == "then"))
                        {
                            return Err(
                                format!(
                                    "{} wraps exactly one pattern; use ({} (then A B)) instead of ({} A then B)",
                                    name, name, name
                                )
                                    .to_string(),
                            );
                        }
                        return Err(format!("{} expects one pattern", name));
                    }
                    return match source {
                        Expr::Vector(values) => {
                            let pattern = gate_steps_from_values(values)?;
                            gate_pattern_from_steps(pattern)
                        }
                        _ => gate_pattern(source),
                    };
                }
                _ => {}
            }
        }
    }
    match expr {
        Expr::Vector(values) => {
            let pattern = gate_steps_from_values(values)?;
            gate_pattern_from_steps(pattern)
        }
        _ => {
            let step = gate_step_pattern(expr)?;
            gate_pattern_from_steps(vec![step])
        }
    }
}

fn gate_steps_from_values(values: &[Expr]) -> Result<Vec<Vec<GateSlot>>, String> {
    let mut steps = Vec::new();
    let mut index = 0;
    let mut has_previous_hit = false;
    while index < values.len() {
        if let Some(amount) = gate_sustain_value(&values[index])? {
            if !has_previous_hit {
                return Err("gate sustain must follow a hit".to_string());
            }
            for _ in 0..amount {
                steps.push(vec![GateSlot {
                    gate: false,
                    cell: GateCell::Static(false),
                    hold: 0,
                }]);
            }
            index += 1;
            continue;
        }
        if let Some(chance) = chance_gate_value(&values[index])? {
            if let Some(Expr::Vector(_)) = values.get(index + 1) {
                let step = chance_prefixed_gate_step(chance, &values[index + 1])?;
                has_previous_hit |= step.iter().any(|slot| slot.gate);
                steps.push(step);
                index += 2;
                continue;
            }
        }
        let step = gate_step_pattern(&values[index])?;
        has_previous_hit |= step.iter().any(|slot| slot.gate);
        steps.push(step);
        index += 1;
    }
    Ok(steps)
}

fn gate_subdivision_pattern(expr: &Expr) -> Result<Vec<Vec<bool>>, String> {
    Ok(gate_pattern(expr)?.gates)
}

#[derive(Clone, Debug)]
struct GateSlot {
    gate: bool,
    cell: GateCell,
    hold: usize,
}

fn gate_pattern_from_steps(steps: Vec<Vec<GateSlot>>) -> Result<ParsedGatePattern, String> {
    let gates: Vec<Vec<bool>> = steps
        .iter()
        .map(|step| step.iter().map(|slot| slot.gate).collect())
        .collect();
    let cells: Vec<Vec<GateCell>> = steps
        .iter()
        .map(|step| step.iter().map(|slot| slot.cell.clone()).collect())
        .collect();
    let holds: Vec<Vec<usize>> = steps
        .iter()
        .map(|step| step.iter().map(|slot| slot.hold).collect())
        .collect();
    parsed_gate_pattern(gates, cells, holds, 0)
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
    for (idx, hold) in flat_holds.iter().copied().enumerate() {
        if hold == 0 {
            continue;
        }
        if !flat_gates.get(idx).copied().unwrap_or(false) {
            return Err("gate hold can only be attached to a hit".to_string());
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
            if hold == 0 {
                return Err("gate-hold must be greater than zero".to_string());
            }
            if items.len() > 2 {
                return Err("gate-hold expects zero or one amount".to_string());
            }
            Ok(vec![GateSlot {
                gate: true,
                cell: GateCell::Static(true),
                hold,
            }])
        }
        Expr::List(items) if matches!(items.first(), Some(Expr::Symbol(name)) if name == "gate-repeat") =>
        {
            if items.len() != 2 {
                return Err("gate-repeat expects one vector".to_string());
            }
            let values = gate_repeat_values(&items[1])?;
            Ok(vec![GateSlot {
                gate: values.first().copied().unwrap_or(false),
                cell: GateCell::Repeat(values),
                hold: 0,
            }])
        }
        Expr::Vector(values) => {
            if values.is_empty() {
                return Ok(vec![GateSlot {
                    gate: false,
                    cell: GateCell::Static(false),
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
        _ => {
            let cell = gate_cell(expr)?;
            Ok(vec![GateSlot {
                gate: gate_cell_preview(&cell),
                cell,
                hold: 0,
            }])
        }
    }
}

fn expand_gate_cell(pattern: &[GateSlot], width: usize) -> Vec<GateSlot> {
    let mut expanded = vec![
        GateSlot {
            gate: false,
            cell: GateCell::Static(false),
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

fn gate_cell(expr: &Expr) -> Result<GateCell, String> {
    match expr {
        Expr::Symbol(name) if name.starts_with('~') => {
            Err("gate sustain must follow a hit inside a vector pattern".to_string())
        }
        Expr::Symbol(name) if name.starts_with('?') => {
            Ok(GateCell::Chance(chance_gate_percent(name)?))
        }
        Expr::Symbol(name) if name.contains('%') => {
            Ok(GateCell::Repeat(parse_gate_repeat_token(name)?))
        }
        Expr::Number(_) => Ok(GateCell::Static(numeric_only(expr)? > 0.0)),
        _ => Err("expected numeric pattern value".to_string()),
    }
}

fn gate_sustain_value(expr: &Expr) -> Result<Option<usize>, String> {
    match expr {
        Expr::Symbol(name) if name.starts_with('~') => Ok(Some(gate_sustain_amount(
            name.strip_prefix('~').unwrap_or(""),
        )?)),
        Expr::List(items) if matches!(items.first(), Some(Expr::Symbol(name)) if name == "gate-sustain") =>
        {
            if items.len() != 2 {
                return Err("gate-sustain expects one amount".to_string());
            }
            Ok(Some(positive_usize_value(&items[1], "gate-sustain")?))
        }
        _ => Ok(None),
    }
}

fn gate_sustain_amount(text: &str) -> Result<usize, String> {
    let amount = text
        .parse::<usize>()
        .map_err(|_| "gate sustain must be written like ~12".to_string())?;
    if amount == 0 {
        return Err("gate sustain must be greater than zero".to_string());
    }
    Ok(amount)
}

fn chance_gate_value(expr: &Expr) -> Result<Option<f32>, String> {
    match expr {
        Expr::Symbol(name) if name.starts_with('?') => Ok(Some(chance_gate_percent(name)?)),
        _ => Ok(None),
    }
}

fn chance_gate_percent(name: &str) -> Result<f32, String> {
    if name == "?" {
        return Ok(0.5);
    }
    let percent = name
        .strip_prefix('?')
        .and_then(|value| value.parse::<f32>().ok())
        .ok_or_else(|| "chance gate must be ? or ?0 through ?100".to_string())?;
    if !(0.0..=100.0).contains(&percent) {
        return Err("chance gate must be between ?0 and ?100".to_string());
    }
    Ok(percent / 100.0)
}

fn chance_prefixed_gate_step(chance: f32, expr: &Expr) -> Result<Vec<GateSlot>, String> {
    let mut slots = gate_step_pattern(expr)?;
    for slot in &mut slots {
        if slot.gate {
            slot.gate = chance >= 0.5;
            slot.cell = GateCell::Chance(chance);
        }
    }
    Ok(slots)
}

fn gate_cell_preview(cell: &GateCell) -> bool {
    match cell {
        GateCell::Static(gate) => *gate,
        GateCell::Repeat(values) => values.first().copied().unwrap_or(false),
        GateCell::Chance(chance) => *chance >= 0.5,
    }
}

fn parse_gate_repeat_token(token: &str) -> Result<Vec<bool>, String> {
    if token.starts_with('%') || token.ends_with('%') {
        return Err("repeat gate cells use values separated by %, like 1%0%1%0".to_string());
    }
    let mut values = Vec::new();
    for part in token.split('%') {
        match part {
            "0" => values.push(false),
            "1" => values.push(true),
            _ => {
                return Err("repeat gate cells only accept 0 and 1 values, like 1%0%1%0".to_string());
            }
        }
    }
    if values.is_empty() {
        return Err("repeat gate cells need at least one value".to_string());
    }
    Ok(values)
}

fn gate_repeat_values(expr: &Expr) -> Result<Vec<bool>, String> {
    let Expr::Vector(items) = expr else {
        return Err("gate-repeat expects a vector".to_string());
    };
    if items.is_empty() {
        return Err("gate-repeat vector cannot be empty".to_string());
    }
    items
        .iter()
        .map(|item| match item {
            Expr::Number(value) if *value == 0.0 => Ok(false),
            Expr::Number(value) if *value == 1.0 => Ok(true),
            _ => Err("gate-repeat only accepts 0 and 1 values".to_string()),
        })
        .collect()
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
