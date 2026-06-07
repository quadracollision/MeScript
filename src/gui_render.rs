use crate::audio;
use crate::editor::editor_preview_source;
use crate::language::{compile_source_for_runtime, eval_program};
use crate::model::Runtime;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

enum DialogTool {
    Zenity,
    KDialog,
}

pub(crate) fn run(default_seconds: f32) -> Result<(), String> {
    let tool = find_dialog_tool().ok_or(
        "no GUI dialog tool found; install 'zenity' or 'kdialog', then run gui-render again",
    )?;
    let Some(input) = select_gl_file(&tool)? else {
        return Ok(());
    };
    let Some(output_dir) = select_output_dir(&tool)? else {
        return Ok(());
    };
    let seconds = ask_seconds(&tool, default_seconds)?.unwrap_or(default_seconds);
    let output = wav_output_path(&input, &output_dir)?;

    match render_selected_file(&input, seconds, output.clone()) {
        Ok(stats) => {
            show_info(
                &tool,
                &format!(
                    "Rendered {} frames\npeak={:.3} rms={:.3}\n{}",
                    stats.frames,
                    stats.peak,
                    stats.rms,
                    output.display()
                ),
            );
            Ok(())
        }
        Err(error) => {
            show_error(&tool, &error);
            Err(error)
        }
    }
}

pub(crate) fn render_selected_file(
    input: &Path,
    seconds: f32,
    output: PathBuf,
) -> Result<audio::RenderStats, String> {
    let source = std::fs::read_to_string(path_to_str(input)?).map_err(|error| error.to_string())?;
    let preview = editor_preview_source(&source);
    let preview = compile_source_for_runtime(&preview)?;
    let mut runtime = Runtime::new();
    eval_program(&mut runtime, &preview)?;
    audio::render(runtime, seconds, output)
}

pub(crate) fn wav_output_path(input: &Path, output_dir: &Path) -> Result<PathBuf, String> {
    let stem = input
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or("selected .gl file has no usable file name")?;
    Ok(output_dir.join(format!("{}.wav", stem)))
}

fn find_dialog_tool() -> Option<DialogTool> {
    if command_ok(Command::new("zenity").arg("--version").output()) {
        return Some(DialogTool::Zenity);
    }
    if command_ok(Command::new("kdialog").arg("--version").output()) {
        return Some(DialogTool::KDialog);
    }
    None
}

fn command_ok(output: std::io::Result<Output>) -> bool {
    output
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn select_gl_file(tool: &DialogTool) -> Result<Option<PathBuf>, String> {
    let output = match tool {
        DialogTool::Zenity => Command::new("zenity")
            .args([
                "--file-selection",
                "--title=Select a MeScript file",
                "--file-filter=MeScript files | *.gl",
                "--file-filter=All files | *",
            ])
            .output(),
        DialogTool::KDialog => Command::new("kdialog")
            .args([
                "--title",
                "Select a MeScript file",
                "--getopenfilename",
                ".",
                "*.gl",
            ])
            .output(),
    }
    .map_err(|error| error.to_string())?;
    selection_from_output(output)
}

fn select_output_dir(tool: &DialogTool) -> Result<Option<PathBuf>, String> {
    let output = match tool {
        DialogTool::Zenity => Command::new("zenity")
            .args([
                "--file-selection",
                "--directory",
                "--title=Select output folder",
            ])
            .output(),
        DialogTool::KDialog => Command::new("kdialog")
            .args([
                "--title",
                "Select output folder",
                "--getexistingdirectory",
                ".",
            ])
            .output(),
    }
    .map_err(|error| error.to_string())?;
    selection_from_output(output)
}

fn ask_seconds(tool: &DialogTool, default_seconds: f32) -> Result<Option<f32>, String> {
    let default = default_seconds.to_string();
    let output = match tool {
        DialogTool::Zenity => Command::new("zenity")
            .args([
                "--entry",
                "--title=Render length",
                "--text=Seconds to render",
                "--entry-text",
                &default,
            ])
            .output(),
        DialogTool::KDialog => Command::new("kdialog")
            .args([
                "--title",
                "Render length",
                "--inputbox",
                "Seconds to render",
                &default,
            ])
            .output(),
    }
    .map_err(|error| error.to_string())?;

    if !output.status.success() {
        return Ok(None);
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|error| error.to_string())?
        .trim()
        .to_string();
    if text.is_empty() {
        return Ok(Some(default_seconds));
    }
    parse_render_seconds(&text).map(Some)
}

pub(crate) fn parse_render_seconds(text: &str) -> Result<f32, String> {
    let seconds = text
        .parse::<f32>()
        .map_err(|_| format!("render seconds must be numeric, got '{}'", text))?;
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err(format!(
            "render seconds must be greater than 0, got '{}'",
            text
        ));
    }
    Ok(seconds)
}

fn selection_from_output(output: Output) -> Result<Option<PathBuf>, String> {
    if !output.status.success() {
        return Ok(None);
    }
    let selected = String::from_utf8(output.stdout)
        .map_err(|error| error.to_string())?
        .trim()
        .to_string();
    if selected.is_empty() {
        Ok(None)
    } else {
        Ok(Some(PathBuf::from(selected)))
    }
}

fn show_info(tool: &DialogTool, message: &str) {
    let _ = match tool {
        DialogTool::Zenity => Command::new("zenity")
            .args(["--info", "--title=Render complete", "--text", message])
            .status(),
        DialogTool::KDialog => Command::new("kdialog")
            .args(["--title", "Render complete", "--msgbox", message])
            .status(),
    };
}

fn show_error(tool: &DialogTool, message: &str) {
    let _ = match tool {
        DialogTool::Zenity => Command::new("zenity")
            .args(["--error", "--title=Render failed", "--text", message])
            .status(),
        DialogTool::KDialog => Command::new("kdialog")
            .args(["--title", "Render failed", "--error", message])
            .status(),
    };
}

fn path_to_str(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| format!("path is not valid UTF-8: {}", path.display()))
}
