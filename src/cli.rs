use crate::audio;
use crate::editor;
use crate::gui_render;
use crate::language::{
    compile_source_for_runtime, compile_source_for_runtime_with_base, eval_program, load_runtime,
};
use crate::model::Runtime;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::HashSet;
use std::env;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

fn has_seconds(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--seconds")
}

pub(crate) fn parse_seconds(args: &[String], default: f32) -> Result<f32, String> {
    let mut seconds_options = args
        .iter()
        .enumerate()
        .filter_map(|(index, arg)| (arg == "--seconds").then_some(index));
    let Some(index) = seconds_options.next() else {
        return Ok(default);
    };
    if seconds_options.next().is_some() {
        return Err("duplicate option '--seconds'".to_string());
    }
    let value = args
        .get(index + 1)
        .ok_or("--seconds requires a numeric value")?;
    if value.starts_with("--") {
        return Err("--seconds requires a numeric value".to_string());
    }
    let seconds = value
        .parse::<f32>()
        .map_err(|_| format!("--seconds must be numeric, got '{}'", value))?;
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err(format!("--seconds must be greater than 0, got '{}'", value));
    }
    Ok(seconds)
}

pub(crate) fn positional_args_with_options(
    args: &[String],
    value_options: &[&str],
) -> Result<Vec<String>, String> {
    let mut positional = Vec::new();
    let mut seen_options = HashSet::new();
    let mut index = 2;
    while index < args.len() {
        let arg = args[index].as_str();
        if value_options.contains(&arg) {
            if !seen_options.insert(arg) {
                return Err(format!("duplicate option '{}'", arg));
            }
            let value = args
                .get(index + 1)
                .ok_or_else(|| format!("{} requires a value", arg))?;
            if value.starts_with("--") {
                return Err(format!("{} requires a value", arg));
            }
            index += 2;
        } else if arg.starts_with("--") {
            return Err(format!("unknown option '{}'", arg));
        } else {
            positional.push(arg.to_string());
            index += 1;
        }
    }
    Ok(positional)
}

pub(crate) fn positional_args(args: &[String]) -> Result<Vec<String>, String> {
    positional_args_with_options(args, &["--seconds"])
}

fn expect_positional_count(
    positional: &[String],
    command: &str,
    expected: usize,
) -> Result<(), String> {
    if positional.len() == expected {
        Ok(())
    } else {
        Err(format!(
            "{} expects exactly {} positional argument{}",
            command,
            expected,
            if expected == 1 { "" } else { "s" }
        ))
    }
}

fn expect_positional_max(positional: &[String], command: &str, max: usize) -> Result<(), String> {
    if positional.len() <= max {
        Ok(())
    } else {
        Err(format!(
            "{} expects at most {} positional argument{}",
            command,
            max,
            if max == 1 { "" } else { "s" }
        ))
    }
}

fn option_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

pub(crate) fn is_help_command(command: &str) -> bool {
    matches!(command, "--help" | "-h" | "help")
}

pub(crate) fn eval_interactive_source(runtime: &mut Runtime, source: &str) -> Result<(), String> {
    let source = compile_source_for_runtime(source)?;
    eval_program(runtime, &source)
}

pub(crate) fn auto_render_seconds(runtime: &Runtime) -> Option<f32> {
    let mut current = runtime.scene_state.as_ref()?.current.clone();
    let mut visited = HashSet::new();
    let mut steps = 0usize;

    loop {
        if !visited.insert(current.clone()) {
            return None;
        }
        let scene = runtime.scenes.get(&current)?;
        if scene.repeats == 0 {
            return None;
        }
        steps += scene.repeats * scene.steps.max(1);
        let Some(next) = &scene.next else {
            break;
        };
        current = next.clone();
    }

    let steps_per_second = runtime.bpm.max(1.0) / 60.0 * 4.0;
    Some(steps as f32 / steps_per_second + 2.0)
}

pub(crate) fn playback_hint(runtime: &Runtime) -> Option<&'static str> {
    if runtime.running {
        None
    } else if !runtime.scenes.is_empty() {
        Some(
            "no scene is playing; add (play-scene :scene-name) or use the editor preview/render path",
        )
    } else if !runtime.tracks.is_empty() {
        Some(
            "tracks are defined but playback is stopped; add (start!) or wrap tracks in a scene and call (play-scene :scene-name)",
        )
    } else if !runtime.post_effects.is_empty() {
        Some("post-fx is defined but there is no audio source; add a track or scene to render")
    } else {
        None
    }
}

pub(crate) fn render_interactive_runtime(
    runtime: Runtime,
    path: PathBuf,
) -> Result<audio::RenderStats, String> {
    if let Some(hint) = playback_hint(&runtime) {
        return Err(hint.to_string());
    }
    audio::render(runtime, 4.0, path)
}

pub(crate) fn interactive_render_path(path: &str) -> Result<PathBuf, String> {
    let path = path.trim();
    if path.is_empty() {
        Err("render requires an output path".to_string())
    } else {
        Ok(PathBuf::from(path))
    }
}

fn repl() -> Result<(), String> {
    let mut runtime = Runtime::new();
    println!("MeScript Native REPL. Type forms, 'render <path>', or 'quit'.");
    loop {
        print!("gl> ");
        io::stdout().flush().map_err(|error| error.to_string())?;
        let mut line = String::new();
        io::stdin()
            .read_line(&mut line)
            .map_err(|error| error.to_string())?;
        let trimmed = line.trim();
        if trimmed == "quit" || trimmed == "exit" {
            return Ok(());
        }
        if let Some(path) = trimmed.strip_prefix("render ") {
            let stats =
                render_interactive_runtime(runtime.clone(), interactive_render_path(path)?)?;
            println!("rendered peak={:.3} rms={:.3}", stats.peak, stats.rms);
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        match eval_interactive_source(&mut runtime, trimmed) {
            Ok(()) => println!("ok {}", live_status_summary(&runtime)),
            Err(error) => eprintln!("error: {}", error),
        }
    }
}

fn live(path: Option<&str>) -> Result<(), String> {
    let runtime = Arc::new(Mutex::new(if let Some(path) = path {
        let runtime = load_runtime(path)?;
        if let Some(hint) = playback_hint(&runtime) {
            return Err(hint.to_string());
        }
        runtime
    } else {
        Runtime::new()
    }));
    let stream = audio::open_output_stream(runtime.clone())?;
    stream.play().map_err(|error| error.to_string())?;
    println!(
        "MeScript Native Live. Audio is running. Type forms, 'status', 'render <path>', or 'quit'."
    );
    loop {
        print!("live> ");
        io::stdout().flush().map_err(|error| error.to_string())?;
        let mut line = String::new();
        io::stdin()
            .read_line(&mut line)
            .map_err(|error| error.to_string())?;
        let trimmed = line.trim();
        if trimmed == "quit" || trimmed == "exit" {
            return Ok(());
        }
        if trimmed == "status" {
            let runtime = runtime.lock().expect("runtime lock poisoned");
            println!("{}", live_status_summary(&runtime));
            for track in runtime.tracks.values() {
                println!(
                    "  :{} {:?} notes={} gates={} every={} offset={} amp={:.2} muted={} solo={}",
                    track.id,
                    track.waveform,
                    track.notes.len(),
                    track.gates.len(),
                    track.step_every,
                    track.step_offset,
                    track.amp,
                    track.muted,
                    track.solo
                );
            }
            continue;
        }
        if let Some(path) = trimmed.strip_prefix("render ") {
            let snapshot = runtime.lock().expect("runtime lock poisoned").clone();
            let stats = render_interactive_runtime(snapshot, interactive_render_path(path)?)?;
            println!("rendered peak={:.3} rms={:.3}", stats.peak, stats.rms);
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        let mut runtime = runtime.lock().expect("runtime lock poisoned");
        match eval_interactive_source(&mut runtime, trimmed) {
            Ok(()) => println!("ok {}", live_status_summary(&runtime)),
            Err(error) => eprintln!("error: {}", error),
        }
    }
}

pub(crate) fn live_status_summary(runtime: &Runtime) -> String {
    runtime.status_summary()
}

pub(crate) fn apply_gui_live_source(
    runtime: &mut Runtime,
    source: &str,
) -> Result<(bool, usize, usize), String> {
    let next_runtime = build_gui_live_runtime(source, runtime.transport_revision.wrapping_add(1))?;
    let tracks = next_runtime.tracks.len();
    let scenes = next_runtime.scenes.len();
    let running = next_runtime.running;
    *runtime = next_runtime;
    Ok((running, tracks, scenes))
}

fn build_gui_live_runtime(source: &str, transport_revision: u64) -> Result<Runtime, String> {
    let source = compile_source_for_runtime(source)?;
    let mut next_runtime = Runtime::new();
    eval_program(&mut next_runtime, &source)?;
    next_runtime.transport_revision = transport_revision;
    Ok(next_runtime)
}

pub(crate) fn gui_live_ok_summary(runtime: &Runtime) -> String {
    format!("OK {}", runtime.status_summary())
}

pub(crate) fn coalesced_step_event(
    first: audio::StepEvent,
    receiver: &mpsc::Receiver<audio::StepEvent>,
) -> audio::StepEvent {
    if first.step == audio::TRANSPORT_STOPPED_STEP {
        return first;
    }
    let mut latest = first;
    while let Ok(event) = receiver.try_recv() {
        let stopped = event.step == audio::TRANSPORT_STOPPED_STEP;
        latest = event;
        if stopped {
            break;
        }
    }
    latest
}

fn gui_live(device_name: Option<&str>) -> Result<(), String> {
    let runtime = Arc::new(Mutex::new(Runtime::new()));
    let (step_tx, step_rx) = mpsc::channel();
    let (stream, audio_info) =
        audio::open_output_stream_named_with_info(runtime.clone(), Some(step_tx), device_name)?;
    stream.play().map_err(|error| error.to_string())?;

    thread::spawn(move || {
        while let Ok(event) = step_rx.recv() {
            let event = coalesced_step_event(event, &step_rx);
            let stdout = io::stdout();
            let mut stdout = stdout.lock();
            if event.step == audio::TRANSPORT_STOPPED_STEP {
                let _ = writeln!(stdout, "STOPPED");
            } else if let Some(scene) = event.scene {
                let _ = writeln!(stdout, "STEP {} :{}", event.step, scene);
            } else {
                let _ = writeln!(stdout, "STEP {}", event.step);
            }
            let _ = stdout.flush();
        }
    });

    println!("AUDIO {}", audio_info.replace('\n', " "));
    println!("READY");
    io::stdout().flush().map_err(|error| error.to_string())?;

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    while let Some(line) = lines.next() {
        let line = line.map_err(|error| error.to_string())?;
        match line.trim() {
            "EVAL" => {
                let mut source = String::new();
                loop {
                    let Some(next) = lines.next() else {
                        return Ok(());
                    };
                    let next = next.map_err(|error| error.to_string())?;
                    if next == "__GLITCHLISP_END__" {
                        break;
                    }
                    source.push_str(&next);
                    source.push('\n');
                }

                let revision = {
                    let runtime = runtime.lock().expect("runtime lock poisoned");
                    runtime.transport_revision.wrapping_add(1)
                };
                match build_gui_live_runtime(&source, revision) {
                    Ok(next_runtime) => {
                        let summary = gui_live_ok_summary(&next_runtime);
                        let mut runtime = runtime.lock().expect("runtime lock poisoned");
                        *runtime = next_runtime;
                        println!("{}", summary);
                    }
                    Err(error) => println!("ERR {}", error.replace('\n', " ")),
                }
                io::stdout().flush().map_err(|error| error.to_string())?;
            }
            "STOP" => {
                let mut runtime = runtime.lock().expect("runtime lock poisoned");
                runtime.running = false;
                runtime.scene_state = None;
                runtime.transport_revision = runtime.transport_revision.wrapping_add(1);
                println!("{}", gui_live_ok_summary(&runtime));
                io::stdout().flush().map_err(|error| error.to_string())?;
            }
            "STATUS" => {
                let runtime = runtime.lock().expect("runtime lock poisoned");
                println!("{}", gui_live_ok_summary(&runtime));
                io::stdout().flush().map_err(|error| error.to_string())?;
            }
            "QUIT" | "EXIT" => return Ok(()),
            "" => {}
            other => {
                println!("ERR unsupported gui-live command {}", other);
                io::stdout().flush().map_err(|error| error.to_string())?;
            }
        }
    }
    Ok(())
}

fn list_devices() -> Result<(), String> {
    let host = cpal::default_host();
    println!("Host: {:?}", host.id());
    for device in host.output_devices().map_err(|error| error.to_string())? {
        let name = device.name().unwrap_or_else(|_| "unknown".to_string());
        let config = device
            .default_output_config()
            .map(|config| {
                format!(
                    "{}ch {}Hz {:?}",
                    config.channels(),
                    config.sample_rate().0,
                    config.sample_format()
                )
            })
            .unwrap_or_else(|_| "no default config".to_string());
        println!("{} - {}", name, config);
    }
    Ok(())
}

fn list_devices_plain() -> Result<(), String> {
    for name in audio::output_device_names()? {
        println!("{}", name);
    }
    Ok(())
}

pub(crate) fn usage_text() -> &'static str {
    "usage:
  glitchlisp-native help
  glitchlisp-native tone [--seconds N]
  glitchlisp-native play <file.gl> [--seconds N]
  glitchlisp-native live [file.gl]
  glitchlisp-native gui-live [--device NAME]
  glitchlisp-native edit [file.gl]
  glitchlisp-native gui-render [--seconds N]
  glitchlisp-native compile-gui [--seconds N]
  glitchlisp-native render <file.gl> <out.wav> [--seconds N]
  glitchlisp-native compile <file.gl> <out.gl>
  glitchlisp-native check-live-source <file.gl>
  glitchlisp-native capabilities
  glitchlisp-native devices
  glitchlisp-native devices-plain
  glitchlisp-native repl"
}

fn usage() {
    eprintln!("{}", usage_text());
}

pub(crate) fn capabilities() -> &'static str {
    "glitchlisp-native capabilities null-params empty-gate-silent gui-live live-audio-info check-live-source gate-then-times scene-loop-true scene-loop-by sample-form gui-render-preview drum-note-pitch native-compiler-source native-compile-command"
}

pub(crate) fn run() -> Result<(), String> {
    let args = env::args().collect::<Vec<_>>();
    run_with_args(&args)
}

pub(crate) fn run_with_args(args: &[String]) -> Result<(), String> {
    run_with_args_usage(args, true)
}

pub(crate) fn run_with_args_quiet(args: &[String]) -> Result<(), String> {
    run_with_args_usage(args, false)
}

fn maybe_usage(show_usage: bool) {
    if show_usage {
        usage();
    }
}

fn run_with_args_usage(args: &[String], show_usage: bool) -> Result<(), String> {
    let Some(command) = args.get(1).map(String::as_str) else {
        maybe_usage(show_usage);
        return Err("missing command".to_string());
    };

    match command {
        command if is_help_command(command) => {
            maybe_usage(show_usage);
            Ok(())
        }
        "tone" => {
            let positional = positional_args(&args)?;
            expect_positional_count(&positional, "tone", 0)?;
            let seconds = parse_seconds(&args, 2.0)?;
            let mut runtime = Runtime::new();
            eval_program(&mut runtime, "(play-note 880)")?;
            audio::play(runtime, seconds)
        }
        "play" => {
            let positional = positional_args(&args)?;
            expect_positional_count(&positional, "play", 1)?;
            let path = &positional[0];
            let seconds = parse_seconds(&args, 12.0)?;
            let runtime = load_runtime(path)?;
            if let Some(hint) = playback_hint(&runtime) {
                return Err(hint.to_string());
            }
            audio::play(runtime, seconds)
        }
        "render" => {
            let positional = positional_args(&args)?;
            expect_positional_count(&positional, "render", 2)?;
            let input = &positional[0];
            let output = &positional[1];
            let explicit_seconds = if has_seconds(&args) {
                Some(parse_seconds(&args, 8.0)?)
            } else {
                None
            };
            let runtime = load_runtime(input)?;
            if let Some(hint) = playback_hint(&runtime) {
                return Err(hint.to_string());
            }
            let seconds =
                explicit_seconds.unwrap_or_else(|| auto_render_seconds(&runtime).unwrap_or(8.0));
            let stats = audio::render(runtime, seconds, PathBuf::from(output))?;
            println!(
                "rendered {} frames {:.2}s peak={:.3} rms={:.3} -> {}",
                stats.frames, seconds, stats.peak, stats.rms, output
            );
            if stats.rms < 0.001 {
                return Err("rendered audio is effectively silent".to_string());
            }
            Ok(())
        }
        "compile" => {
            let positional = positional_args_with_options(&args, &[])?;
            expect_positional_count(&positional, "compile", 2)?;
            let input = &positional[0];
            let output = &positional[1];
            let source =
                std::fs::read_to_string(input).map_err(|error| format!("{}: {}", input, error))?;
            let mut compiled =
                compile_source_for_runtime_with_base(&source, Some(std::path::Path::new(input)))?;
            let mut validation_runtime = Runtime::new();
            eval_program(&mut validation_runtime, &compiled)
                .map_err(|error| format!("compiled source failed runtime validation: {}", error))?;
            if !compiled.ends_with('\n') {
                compiled.push('\n');
            }
            std::fs::write(output, compiled).map_err(|error| format!("{}: {}", output, error))?;
            println!("compiled {} -> {}", input, output);
            Ok(())
        }
        "live" => {
            let positional = positional_args_with_options(&args, &[])?;
            expect_positional_max(&positional, "live", 1)?;
            live(positional.first().map(String::as_str))
        }
        "gui-live" => {
            let positional = positional_args_with_options(&args, &["--device"])?;
            expect_positional_count(&positional, "gui-live", 0)?;
            gui_live(option_value(&args, "--device").as_deref())
        }
        "edit" => {
            let positional = positional_args_with_options(&args, &[])?;
            expect_positional_max(&positional, "edit", 1)?;
            editor::run(positional.first().map(String::as_str))
        }
        "gui-render" | "compile-gui" => {
            let positional = positional_args(&args)?;
            expect_positional_count(&positional, command, 0)?;
            let seconds = parse_seconds(&args, 8.0)?;
            gui_render::run(seconds)
        }
        "check-live-source" => {
            let positional = positional_args_with_options(&args, &[])?;
            expect_positional_count(&positional, "check-live-source", 1)?;
            let input = &positional[0];
            let source =
                std::fs::read_to_string(input).map_err(|error| format!("{}: {}", input, error))?;
            let mut runtime = Runtime::new();
            let source =
                compile_source_for_runtime_with_base(&source, Some(std::path::Path::new(input)))?;
            eval_program(&mut runtime, &source)?;
            println!("{}", gui_live_ok_summary(&runtime));
            Ok(())
        }
        "capabilities" => {
            let positional = positional_args_with_options(&args, &[])?;
            expect_positional_count(&positional, "capabilities", 0)?;
            println!("{}", capabilities());
            Ok(())
        }
        "devices" => {
            let positional = positional_args_with_options(&args, &[])?;
            expect_positional_count(&positional, "devices", 0)?;
            list_devices()
        }
        "devices-plain" => {
            let positional = positional_args_with_options(&args, &[])?;
            expect_positional_count(&positional, "devices-plain", 0)?;
            list_devices_plain()
        }
        "repl" => {
            let positional = positional_args_with_options(&args, &[])?;
            expect_positional_count(&positional, "repl", 0)?;
            repl()
        }
        other => {
            maybe_usage(show_usage);
            Err(format!("unknown command '{}'", other))
        }
    }
}
