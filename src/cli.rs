use crate::audio;
use crate::editor;
use crate::gui_render;
use crate::language::{eval_program, load_runtime};
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

fn parse_seconds(args: &[String], default: f32) -> f32 {
    args.windows(2)
        .find(|pair| pair[0] == "--seconds")
        .and_then(|pair| pair[1].parse::<f32>().ok())
        .unwrap_or(default)
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
            let stats = audio::render(runtime.clone(), 4.0, PathBuf::from(path.trim()))?;
            println!("rendered peak={:.3} rms={:.3}", stats.peak, stats.rms);
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        match eval_program(&mut runtime, trimmed) {
            Ok(()) => println!("ok bpm={} tracks={}", runtime.bpm, runtime.tracks.len()),
            Err(error) => eprintln!("error: {}", error),
        }
    }
}

fn live(path: Option<&str>) -> Result<(), String> {
    let runtime = Arc::new(Mutex::new(if let Some(path) = path {
        load_runtime(path)?
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
            println!(
                "bpm={} running={} tracks={}",
                runtime.bpm,
                runtime.running,
                runtime.tracks.len()
            );
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
            let stats = audio::render(snapshot, 4.0, PathBuf::from(path.trim()))?;
            println!("rendered peak={:.3} rms={:.3}", stats.peak, stats.rms);
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        let mut runtime = runtime.lock().expect("runtime lock poisoned");
        match eval_program(&mut runtime, trimmed) {
            Ok(()) => println!("ok bpm={} tracks={}", runtime.bpm, runtime.tracks.len()),
            Err(error) => eprintln!("error: {}", error),
        }
    }
}

fn option_after(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

pub(crate) fn apply_gui_live_source(
    runtime: &mut Runtime,
    source: &str,
) -> Result<(bool, usize, usize), String> {
    let mut next_runtime = Runtime::new();
    eval_program(&mut next_runtime, source)?;
    let tracks = next_runtime.tracks.len();
    let scenes = next_runtime.scenes.len();
    let running = next_runtime.running;
    next_runtime.transport_revision = runtime.transport_revision.wrapping_add(1);
    *runtime = next_runtime;
    Ok((running, tracks, scenes))
}

fn gui_live(device_name: Option<&str>) -> Result<(), String> {
    let runtime = Arc::new(Mutex::new(Runtime::new()));
    let (step_tx, step_rx) = mpsc::channel();
    let (stream, audio_info) =
        audio::open_output_stream_named_with_info(runtime.clone(), Some(step_tx), device_name)?;
    stream.play().map_err(|error| error.to_string())?;

    thread::spawn(move || {
        for step in step_rx {
            if step == audio::TRANSPORT_STOPPED_STEP {
                println!("STOPPED");
            } else {
                println!("STEP {}", step);
            }
            let _ = io::stdout().flush();
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

                let mut runtime = runtime.lock().expect("runtime lock poisoned");
                match apply_gui_live_source(&mut runtime, &source) {
                    Ok((running, tracks, scenes)) => {
                        println!("OK running={} tracks={} scenes={}", running, tracks, scenes);
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
                println!("OK stopped");
                io::stdout().flush().map_err(|error| error.to_string())?;
            }
            "STATUS" => {
                let runtime = runtime.lock().expect("runtime lock poisoned");
                println!(
                    "OK bpm={} running={} tracks={} scenes={}",
                    runtime.bpm,
                    runtime.running,
                    runtime.tracks.len(),
                    runtime.scenes.len()
                );
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

fn usage() {
    eprintln!(
        "usage:
  glitchlisp-native tone [--seconds N]
  glitchlisp-native play <file.gl> [--seconds N]
  glitchlisp-native live [file.gl]
  glitchlisp-native gui-live [--device NAME]
  glitchlisp-native edit [file.gl]
  glitchlisp-native gui-render [--seconds N]
  glitchlisp-native render <file.gl> <out.wav> [--seconds N]
  glitchlisp-native check-live-source <file.gl>
  glitchlisp-native capabilities
  glitchlisp-native devices
  glitchlisp-native devices-plain
  glitchlisp-native repl"
    );
}

pub(crate) fn run() -> Result<(), String> {
    let args = env::args().collect::<Vec<_>>();
    let Some(command) = args.get(1).map(String::as_str) else {
        usage();
        return Ok(());
    };

    match command {
        "tone" => {
            let seconds = parse_seconds(&args, 2.0);
            let mut runtime = Runtime::new();
            eval_program(&mut runtime, "(play-note 880)")?;
            audio::play(runtime, seconds)
        }
        "play" => {
            let path = args.get(2).ok_or("play requires a .gl file")?;
            let seconds = parse_seconds(&args, 12.0);
            audio::play(load_runtime(path)?, seconds)
        }
        "render" => {
            let input = args.get(2).ok_or("render requires a .gl file")?;
            let output = args.get(3).ok_or("render requires an output .wav")?;
            let runtime = load_runtime(input)?;
            let seconds = if has_seconds(&args) {
                parse_seconds(&args, 8.0)
            } else {
                auto_render_seconds(&runtime).unwrap_or(8.0)
            };
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
        "live" => live(args.get(2).map(String::as_str)),
        "gui-live" => gui_live(option_after(&args, "--device").as_deref()),
        "edit" => editor::run(args.get(2).map(String::as_str)),
        "gui-render" | "compile-gui" => {
            let seconds = parse_seconds(&args, 8.0);
            gui_render::run(seconds)
        }
        "check-live-source" => {
            let input = args.get(2).ok_or("check-live-source requires a .gl file")?;
            let source = std::fs::read_to_string(input).map_err(|error| error.to_string())?;
            let mut runtime = Runtime::new();
            let (running, tracks, scenes) = apply_gui_live_source(&mut runtime, &source)?;
            println!("OK running={} tracks={} scenes={}", running, tracks, scenes);
            Ok(())
        }
        "capabilities" => {
            println!(
                "glitchlisp-native capabilities null-params empty-gate-silent gui-live live-audio-info check-live-source"
            );
            Ok(())
        }
        "devices" => list_devices(),
        "devices-plain" => list_devices_plain(),
        "repl" => repl(),
        _ => {
            usage();
            Ok(())
        }
    }
}
