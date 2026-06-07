use crate::audio::open_output_stream;
use crate::language::{apply_scene, compile_source_for_runtime, eval_program};
use crate::model::Runtime;
use cpal::traits::StreamTrait;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

pub(crate) const DEFAULT_SOURCE: &str = "";

#[derive(Clone, Debug, Default)]
pub(crate) struct EditorBuffer {
    pub(crate) lines: Vec<String>,
    path: Option<PathBuf>,
}

impl EditorBuffer {
    pub(crate) fn new(path: Option<PathBuf>, source: String) -> Self {
        let mut lines = source.lines().map(str::to_string).collect::<Vec<_>>();
        if source.is_empty() {
            lines.clear();
        }
        Self { lines, path }
    }

    #[cfg(test)]
    pub(crate) fn empty(path: Option<PathBuf>) -> Self {
        Self {
            lines: Vec::new(),
            path,
        }
    }

    pub(crate) fn source(&self) -> String {
        self.lines.join("\n")
    }

    pub(crate) fn range_source(&self, range: LineRange) -> Result<String, String> {
        if self.lines.is_empty() {
            return Err("buffer is empty".to_string());
        }
        let start = checked_editor_index(range.start, self.lines.len(), "run")?;
        let end = checked_editor_index(range.end, self.lines.len(), "run")?;
        if start > end {
            return Err("run start line must be before end line".to_string());
        }
        Ok(self.lines[start..=end].join("\n"))
    }

    #[cfg(test)]
    pub(crate) fn append(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }

    #[cfg(test)]
    pub(crate) fn insert(&mut self, line_number: usize, line: &str) -> Result<(), String> {
        let index = checked_editor_index(line_number, self.lines.len() + 1, "insert")?;
        self.lines.insert(index, line.to_string());
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn replace(&mut self, line_number: usize, line: &str) -> Result<(), String> {
        let index = checked_editor_index(line_number, self.lines.len(), "replace")?;
        self.lines[index] = line.to_string();
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn delete(&mut self, line_number: usize) -> Result<(), String> {
        let index = checked_editor_index(line_number, self.lines.len(), "delete")?;
        self.lines.remove(index);
        Ok(())
    }

    fn write(&mut self, path: Option<PathBuf>) -> Result<PathBuf, String> {
        if let Some(path) = path {
            self.path = Some(path);
        }
        let path = self
            .path
            .clone()
            .ok_or("write requires a path the first time")?;
        fs::write(&path, self.source())
            .map_err(|error| format!("{}: {}", path.display(), error))?;
        Ok(path)
    }

    fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

fn checked_editor_index(
    line_number: usize,
    max_line_number: usize,
    command: &str,
) -> Result<usize, String> {
    if line_number == 0 || line_number > max_line_number {
        return Err(format!(
            "{} line must be between 1 and {}",
            command, max_line_number
        ));
    }
    Ok(line_number - 1)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LineRange {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

pub(crate) fn apply_runtime_source(
    runtime: &Arc<Mutex<Runtime>>,
    source: &str,
) -> Result<Runtime, String> {
    let mut next = Runtime::new();
    let source = compile_source_for_runtime(source)?;
    eval_program(&mut next, &source)?;
    *runtime.lock().expect("runtime lock poisoned") = next.clone();
    Ok(next)
}

pub(crate) fn editor_run_message(snapshot: &Runtime) -> String {
    format!("running {}", snapshot.status_summary())
}

pub(crate) fn editor_stop_message(snapshot: &Runtime) -> String {
    format!("stopped {}", snapshot.status_summary())
}

pub(crate) fn editor_scene_message(action: &str, scene: &str, snapshot: &Runtime) -> String {
    format!("{} scene :{} {}", action, scene, snapshot.status_summary())
}

pub(crate) fn editor_preview_source(source: &str) -> String {
    if has_playback_command(source) {
        source.to_string()
    } else if let Some(scene) = first_scene_name(source) {
        format!("{}\n\n(play-scene :{})", source.trim(), scene)
    } else if has_top_level_playable(source) {
        format!("{}\n\n(start!)", source.trim())
    } else {
        source.to_string()
    }
}

#[derive(Debug)]
struct TopLevelForm {
    head: String,
    first_arg: Option<String>,
    start_line: usize,
    end_line: usize,
}

fn read_form_token(source: &str, start: usize) -> Option<(String, usize)> {
    let mut token_start = None;
    for (offset, ch) in source[start..].char_indices() {
        if token_start.is_none() && ch.is_whitespace() {
            continue;
        }
        let absolute = start + offset;
        if token_start.is_none() {
            token_start = Some(absolute);
        }
        if ch.is_whitespace() || matches!(ch, '(' | ')' | '[' | ']') {
            let begin = token_start?;
            return (absolute > begin).then(|| (source[begin..absolute].to_string(), absolute));
        }
    }
    let begin = token_start?;
    Some((source[begin..].to_string(), source.len()))
}

fn top_level_forms(source: &str) -> Vec<TopLevelForm> {
    let mut forms = Vec::new();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escape = false;
    let mut in_comment = false;
    let mut line = 0usize;
    let mut pending: Option<TopLevelForm> = None;

    for (idx, ch) in source.char_indices() {
        if in_comment {
            if ch == '\n' {
                line += 1;
            }
            in_comment = ch != '\n';
            continue;
        }
        if escape {
            escape = false;
            if ch == '\n' {
                line += 1;
            }
            continue;
        }
        if in_string {
            if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            } else if ch == '\n' {
                line += 1;
            }
            continue;
        }

        match ch {
            ';' => in_comment = true,
            '"' => in_string = true,
            '(' | '[' | '{' => {
                if depth == 0 {
                    if let Some((head, next)) = read_form_token(source, idx + ch.len_utf8()) {
                        let first_arg = read_form_token(source, next).map(|(token, _)| token);
                        pending = Some(TopLevelForm {
                            head,
                            first_arg,
                            start_line: line,
                            end_line: line,
                        });
                    }
                }
                depth += 1;
            }
            ')' | ']' | '}' => {
                if depth == 1 {
                    if let Some(mut form) = pending.take() {
                        form.end_line = line;
                        forms.push(form);
                    }
                }
                depth = depth.saturating_sub(1);
            }
            '\n' => line += 1,
            _ => {}
        }
    }
    if let Some(mut form) = pending {
        form.end_line = line;
        forms.push(form);
    }

    forms
}

fn has_playback_command(source: &str) -> bool {
    top_level_forms(source).iter().any(|form| {
        matches!(
            form.head.as_str(),
            "start!" | "play-scene" | "play-block" | "cue"
        )
    })
}

fn first_scene_name(source: &str) -> Option<String> {
    top_level_forms(source)
        .into_iter()
        .find(|form| matches!(form.head.as_str(), "scene" | "block"))
        .and_then(|form| form.first_arg)
        .and_then(|token| token.strip_prefix(':').map(str::to_string))
}

pub(crate) fn scene_name_for_cursor(lines: &[String], cursor_line: usize) -> Option<String> {
    let source = lines.join("\n");
    let forms = top_level_forms(&source);
    for offset in 0..=3 {
        let index = cursor_line.saturating_sub(offset);
        if let Some(scene) = forms
            .iter()
            .find(|form| {
                form.start_line == index
                    && matches!(
                        form.head.as_str(),
                        "scene" | "block" | "play-scene" | "play-block" | "cue"
                    )
            })
            .and_then(scene_name_from_form)
        {
            return Some(scene);
        }
    }

    if let Some(scene) = forms
        .iter()
        .rev()
        .find(|form| {
            matches!(form.head.as_str(), "scene" | "block")
                && form.start_line <= cursor_line
                && cursor_line <= form.end_line
        })
        .and_then(scene_name_from_form)
    {
        return Some(scene);
    }

    unique_scene_definition_name(&forms)
}

fn scene_name_from_form(form: &TopLevelForm) -> Option<String> {
    form.first_arg
        .as_deref()
        .and_then(|token| token.trim_end_matches(')').strip_prefix(':'))
        .map(str::to_string)
}

fn unique_scene_definition_name(forms: &[TopLevelForm]) -> Option<String> {
    let mut names = forms
        .iter()
        .filter(|form| matches!(form.head.as_str(), "scene" | "block"))
        .filter_map(scene_name_from_form);
    let first = names.next()?;
    if names.next().is_none() {
        Some(first)
    } else {
        None
    }
}

fn has_top_level_playable(source: &str) -> bool {
    top_level_forms(source)
        .iter()
        .any(|form| matches!(form.head.as_str(), "d" | "sample"))
}

fn eval_live_form(runtime: &Arc<Mutex<Runtime>>, source: &str) -> Result<Runtime, String> {
    let source = compile_source_for_runtime(source)?;
    let mut runtime_guard = runtime.lock().expect("runtime lock poisoned");
    eval_program(&mut runtime_guard, &source)?;
    Ok(runtime_guard.clone())
}

pub(crate) fn run(path: Option<&str>) -> Result<(), String> {
    let mut buffer = if let Some(path) = path {
        let path = PathBuf::from(path);
        let source =
            fs::read_to_string(&path).map_err(|error| format!("{}: {}", path.display(), error))?;
        EditorBuffer::new(Some(path), source)
    } else {
        EditorBuffer::new(None, DEFAULT_SOURCE.to_string())
    };
    if buffer.lines.is_empty() {
        buffer.lines.push(String::new());
    }

    let runtime = Arc::new(Mutex::new(Runtime::new()));
    if !buffer.source().is_empty() {
        apply_runtime_source(&runtime, &editor_preview_source(&buffer.source()))?;
    }
    let stream = open_output_stream(runtime.clone())?;
    stream.play().map_err(|error| error.to_string())?;

    let _terminal = TerminalSession::enter()?;
    let mut app = EditorApp::new(buffer, runtime);
    app.run()
}

struct TerminalSession {
    saved_state: String,
}

impl TerminalSession {
    fn enter() -> Result<Self, String> {
        let saved = Command::new("stty")
            .args(["-F", "/dev/tty", "-g"])
            .output()
            .map_err(|error| error.to_string())?;
        if !saved.status.success() {
            let stderr = String::from_utf8_lossy(&saved.stderr);
            return Err(format!("failed to read terminal state: {}", stderr.trim()));
        }
        let saved_state = String::from_utf8(saved.stdout)
            .map_err(|error| error.to_string())?
            .trim()
            .to_string();
        let status = Command::new("stty")
            .args(["-F", "/dev/tty", "raw", "-echo", "min", "0", "time", "1"])
            .status()
            .map_err(|error| error.to_string())?;
        if !status.success() {
            return Err("failed to enable raw terminal mode".to_string());
        }
        print!("\x1b[?1049h\x1b[?25l");
        io::stdout().flush().map_err(|error| error.to_string())?;
        Ok(Self { saved_state })
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = Command::new("stty")
            .args(["-F", "/dev/tty", &self.saved_state])
            .status();
        print!("\x1b[?25h\x1b[0m\x1b[?1049l");
        let _ = io::stdout().flush();
    }
}

#[derive(Debug, Clone, Copy)]
enum Key {
    Char(char),
    Ctrl(char),
    Enter,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Esc,
    Unknown,
}

struct EditorApp {
    buffer: EditorBuffer,
    runtime: Arc<Mutex<Runtime>>,
    cursor_line: usize,
    cursor_col: usize,
    scroll_top: usize,
    scroll_left: usize,
    active_block: Option<LineRange>,
    selection_anchor: Option<usize>,
    dirty: bool,
    message: String,
    menu: Option<MenuState>,
}

struct MenuState {
    title: &'static str,
    items: &'static [Snippet],
    selected: usize,
}

#[derive(Clone, Copy)]
struct Snippet {
    label: &'static str,
    text: &'static str,
}

const OSCILLATOR_SNIPPETS: &[Snippet] = &[
    Snippet {
        label: "kick-808",
        text: "(d :kick\n   :src :kick-808\n   :note c1\n   :gate (p [1 0 0 0 1 0 0 0])\n   :dur 0.36\n   :amp 0.42)\n",
    },
    Snippet {
        label: "snare-808",
        text: "(d :snare\n   :src :snare-808\n   :note c3\n   :gate (p [0 0 0 0 1 0 0 0])\n   :dur 0.16\n   :amp 0.22)\n",
    },
    Snippet {
        label: "hat-909",
        text: "(d :hat\n   :src :hat-909\n   :note c6\n   :gate (p [0 1 0 1 0 1 0 1])\n   :dur 0.03\n   :amp 0.06)\n",
    },
    Snippet {
        label: "supersaw lead",
        text: "(d :lead\n   :src :supersaw\n   :note (p [c4 eb4 g4 bb4])\n   :gate (p [1 0 1 1 0 1 0 1])\n   :dur 0.08\n   :amp 0.12\n   :unison 5\n   :unison-detune 9\n   :unison-spread 0.8)\n",
    },
    Snippet {
        label: "fm bass",
        text: "(d :bass\n   :src :fm-op\n   :note (p [c2 c2 eb2 g1 bb1 c2])\n   :gate (p [1 0 1 1 0 1 0 1])\n   :dur 0.16\n   :amp 0.22\n   :fm-ratio 1.5\n   :fm-depth 2.2)\n",
    },
    Snippet {
        label: "pad wash",
        text: "(d :pad\n   :src :pad-wash\n   :note (p [c2 eb2 g2 bb2])\n   :gate (p [1 0 0 0 0 0 0 0])\n   :dur 1.8\n   :amp 0.12\n   :unison 5\n   :unison-detune 10\n   :unison-spread 0.8)\n",
    },
    Snippet {
        label: "scene",
        text: "(scene :intro :loop true\n  \n)\n\n(play-scene :intro)\n",
    },
];

const EFFECT_SNIPPETS: &[Snippet] = &[
    Snippet {
        label: "filter lowpass",
        text: "(filter :type :lowpass :cutoff 1200 :res 0.35)",
    },
    Snippet {
        label: "moog",
        text: "(moog :cutoff 760 :res 0.55 :drive 0.10)",
    },
    Snippet {
        label: "tb-303",
        text: "(tb-303 :cutoff 1050 :res 0.85 :env-mod 0.45 :accent 0.25 :decay 0.24)",
    },
    Snippet {
        label: "delay",
        text: "(delay :time 0.125 :feedback 0.32 :mix 0.22)",
    },
    Snippet {
        label: "ams reverb",
        text: "(ams-reverb :decay 0.58 :damping 0.42 :program :nonlin :mix 0.42)",
    },
    Snippet {
        label: "h3000",
        text: "(h3000 :detune-cents 9 :mix 0.32)",
    },
    Snippet {
        label: "sub bass",
        text: "(sub-bass :mix 0.24)",
    },
    Snippet {
        label: "sidechain",
        text: "(sidechain :rate 2 :depth 0.45 :shape 0.45)",
    },
    Snippet {
        label: "tube",
        text: "(tube :drive 0.10 :asymmetry 0.03)",
    },
    Snippet {
        label: "fx vector",
        text: ":fx [(filter :type :lowpass :cutoff 1200 :res 0.35)\n     (delay :time 0.125 :feedback 0.32 :mix 0.22)]",
    },
];

impl EditorApp {
    fn new(buffer: EditorBuffer, runtime: Arc<Mutex<Runtime>>) -> Self {
        Self {
            buffer,
            runtime,
            cursor_line: 0,
            cursor_col: 0,
            scroll_top: 0,
            scroll_left: 0,
            active_block: None,
            selection_anchor: None,
            dirty: false,
            message: "Ctrl-S save  Ctrl-R run  Ctrl-O osc  Ctrl-E fx  Ctrl-P cue  Ctrl-B loop  Ctrl-X stop  Ctrl-Q quit".to_string(),
            menu: None,
        }
    }

    fn run(&mut self) -> Result<(), String> {
        loop {
            self.ensure_cursor_visible();
            self.render()?;
            let key = read_key().map_err(|error| error.to_string())?;
            if self.handle_menu_key(key) {
                continue;
            }
            match key {
                Key::Char(c) => self.insert_char(c),
                Key::Enter => self.insert_newline(),
                Key::Backspace => self.backspace(),
                Key::Delete => self.delete_forward(),
                Key::Left => self.move_left(),
                Key::Right => self.move_right(),
                Key::Up => self.move_up(),
                Key::Down => self.move_down(),
                Key::Home => self.move_home(),
                Key::End => self.move_end(),
                Key::Ctrl('q') => return Ok(()),
                Key::Ctrl('s') => self.save()?,
                Key::Ctrl('r') => self.run_buffer()?,
                Key::Ctrl('o') => self.open_menu("Oscillators", OSCILLATOR_SNIPPETS),
                Key::Ctrl('e') => self.open_menu("Effects", EFFECT_SNIPPETS),
                Key::Ctrl('p') => self.play_scene_at_cursor()?,
                Key::Ctrl('b') => self.loop_scene_at_cursor()?,
                Key::Ctrl('x') => self.stop_playback(),
                Key::Ctrl('k') => self.toggle_block(),
                Key::Ctrl('u') => self.clear_block(),
                Key::Ctrl('l') => self.message = "refreshed".to_string(),
                Key::Esc => self.message = "escape".to_string(),
                Key::Ctrl(_) | Key::Unknown => {}
            }
        }
    }

    fn current_line(&self) -> &str {
        self.buffer
            .lines
            .get(self.cursor_line)
            .map_or("", String::as_str)
    }

    fn current_line_mut(&mut self) -> &mut String {
        if self.buffer.lines.is_empty() {
            self.buffer.lines.push(String::new());
        }
        self.buffer
            .lines
            .get_mut(self.cursor_line)
            .expect("cursor line in bounds")
    }

    fn line_len(&self, line: usize) -> usize {
        self.buffer
            .lines
            .get(line)
            .map(|line| line.chars().count())
            .unwrap_or(0)
    }

    fn insert_char(&mut self, ch: char) {
        let cursor_col = self.cursor_col;
        let line = self.current_line_mut();
        let byte = char_to_byte_index(line, cursor_col);
        line.insert(byte, ch);
        self.cursor_col += 1;
        self.dirty = true;
    }

    fn insert_newline(&mut self) {
        if self.buffer.lines.is_empty() {
            self.buffer.lines.push(String::new());
        }
        let byte = {
            let line = self.current_line();
            char_to_byte_index(line, self.cursor_col)
        };
        let tail = self.current_line()[byte..].to_string();
        {
            let line = self.current_line_mut();
            line.truncate(byte);
        }
        self.cursor_line += 1;
        self.cursor_col = 0;
        self.buffer.lines.insert(self.cursor_line, tail);
        self.dirty = true;
    }

    fn insert_text(&mut self, text: &str) {
        for ch in text.chars() {
            if ch == '\n' {
                self.insert_newline();
            } else {
                self.insert_char(ch);
            }
        }
    }

    fn open_menu(&mut self, title: &'static str, items: &'static [Snippet]) {
        self.menu = Some(MenuState {
            title,
            items,
            selected: 0,
        });
        self.message = format!("{}: arrows select, Enter insert, Esc cancel", title);
    }

    fn handle_menu_key(&mut self, key: Key) -> bool {
        let Some(menu) = self.menu.as_mut() else {
            return false;
        };
        match key {
            Key::Up => {
                menu.selected = menu.selected.saturating_sub(1);
                true
            }
            Key::Down => {
                menu.selected = (menu.selected + 1).min(menu.items.len().saturating_sub(1));
                true
            }
            Key::Enter => {
                let snippet = menu.items[menu.selected];
                self.menu = None;
                self.insert_text(snippet.text);
                self.message = format!("inserted {}", snippet.label);
                true
            }
            Key::Esc | Key::Ctrl('o') | Key::Ctrl('e') => {
                self.menu = None;
                self.message = "menu closed".to_string();
                true
            }
            _ => true,
        }
    }

    fn backspace(&mut self) {
        if self.cursor_col > 0 {
            let cursor_col = self.cursor_col;
            let line = self.current_line_mut();
            let byte = char_prev_byte_index(line, cursor_col);
            line.remove(byte);
            self.cursor_col -= 1;
            self.dirty = true;
            return;
        }
        if self.cursor_line == 0 {
            return;
        }
        let current = self.buffer.lines.remove(self.cursor_line);
        self.cursor_line -= 1;
        let prev_len = self.line_len(self.cursor_line);
        self.cursor_col = prev_len;
        self.buffer.lines[self.cursor_line].push_str(&current);
        self.dirty = true;
    }

    fn delete_forward(&mut self) {
        let len = self.line_len(self.cursor_line);
        if self.cursor_col < len {
            let cursor_col = self.cursor_col;
            let line = self.current_line_mut();
            let byte = char_to_byte_index(line, cursor_col);
            line.remove(byte);
            self.dirty = true;
            return;
        }
        if self.cursor_line + 1 >= self.buffer.lines.len() {
            return;
        }
        let next = self.buffer.lines.remove(self.cursor_line + 1);
        self.buffer.lines[self.cursor_line].push_str(&next);
        self.dirty = true;
    }

    fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.line_len(self.cursor_line);
        }
    }

    fn move_right(&mut self) {
        if self.cursor_col < self.line_len(self.cursor_line) {
            self.cursor_col += 1;
        } else if self.cursor_line + 1 < self.buffer.lines.len() {
            self.cursor_line += 1;
            self.cursor_col = 0;
        }
    }

    fn move_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.cursor_col.min(self.line_len(self.cursor_line));
        }
    }

    fn move_down(&mut self) {
        if self.cursor_line + 1 < self.buffer.lines.len() {
            self.cursor_line += 1;
            self.cursor_col = self.cursor_col.min(self.line_len(self.cursor_line));
        }
    }

    fn move_home(&mut self) {
        self.cursor_col = 0;
    }

    fn move_end(&mut self) {
        self.cursor_col = self.line_len(self.cursor_line);
    }

    fn toggle_block(&mut self) {
        let line_number = self.cursor_line + 1;
        if let Some(anchor) = self.selection_anchor.take() {
            let start = anchor.min(line_number);
            let end = anchor.max(line_number);
            self.active_block = Some(LineRange { start, end });
            self.message = format!("block {}-{} active", start, end);
        } else {
            self.selection_anchor = Some(line_number);
            self.message = format!("block start set at line {}", line_number);
        }
    }

    fn clear_block(&mut self) {
        self.selection_anchor = None;
        self.active_block = None;
        self.message = "block cleared".to_string();
    }

    fn save(&mut self) -> Result<(), String> {
        let path = match self.buffer.path() {
            Some(path) => path.to_path_buf(),
            None => PathBuf::from("mescript-session.gl"),
        };
        let saved = self.buffer.write(Some(path.clone()))?;
        self.message = format!("saved {}", saved.display());
        self.dirty = false;
        Ok(())
    }

    fn run_buffer(&mut self) -> Result<(), String> {
        let source = if let Some(range) = self.active_block {
            self.buffer.range_source(range)?
        } else {
            self.buffer.source()
        };
        let snapshot = if self.active_block.is_some() {
            eval_live_form(&self.runtime, &editor_preview_source(&source))?
        } else {
            apply_runtime_source(&self.runtime, &editor_preview_source(&source))?
        };
        self.message = editor_run_message(&snapshot);
        Ok(())
    }

    fn play_scene_at_cursor(&mut self) -> Result<(), String> {
        let scene = self.scene_name_near_cursor().ok_or(
            "put cursor on a (scene :name ...), (block :name ...), or (play-scene :name) line",
        )?;
        let mut runtime = Runtime::new();
        let source = compile_source_for_runtime(&self.buffer.source())?;
        eval_program(&mut runtime, &source)?;
        apply_scene(&mut runtime, &scene)?;
        let message = editor_scene_message("cued", &scene, &runtime);
        *self.runtime.lock().expect("runtime lock poisoned") = runtime;
        self.message = message;
        Ok(())
    }

    fn loop_scene_at_cursor(&mut self) -> Result<(), String> {
        let scene = self.scene_name_near_cursor().ok_or(
            "put cursor on a (scene :name ...), (block :name ...), or (play-scene :name) line",
        )?;
        let mut runtime = Runtime::new();
        let source = compile_source_for_runtime(&self.buffer.source())?;
        eval_program(&mut runtime, &source)?;
        let Some(scene_def) = runtime.scenes.get_mut(&scene) else {
            return Err(format!("unknown scene ':{}'", scene));
        };
        scene_def.next = Some(scene.clone());
        apply_scene(&mut runtime, &scene)?;
        let message = editor_scene_message("looping", &scene, &runtime);
        *self.runtime.lock().expect("runtime lock poisoned") = runtime;
        self.message = message;
        Ok(())
    }

    fn stop_playback(&mut self) {
        let mut runtime = self.runtime.lock().expect("runtime lock poisoned");
        runtime.running = false;
        runtime.scene_state = None;
        self.message = editor_stop_message(&runtime);
    }

    fn scene_name_near_cursor(&self) -> Option<String> {
        scene_name_for_cursor(&self.buffer.lines, self.cursor_line)
    }

    fn terminal_size(&self) -> (usize, usize) {
        terminal_size().unwrap_or((24, 80))
    }

    fn ensure_cursor_visible(&mut self) {
        let (rows, cols) = self.terminal_size();
        let content_rows = rows.saturating_sub(3).max(1);
        if self.cursor_line < self.scroll_top {
            self.scroll_top = self.cursor_line;
        } else if self.cursor_line >= self.scroll_top + content_rows {
            self.scroll_top = self.cursor_line + 1 - content_rows;
        }
        let gutter = self.gutter_width(cols);
        let visible_width = cols.saturating_sub(gutter + 1).max(1);
        if self.cursor_col < self.scroll_left {
            self.scroll_left = self.cursor_col;
        } else if self.cursor_col >= self.scroll_left + visible_width {
            self.scroll_left = self.cursor_col + 1 - visible_width;
        }
    }

    fn gutter_width(&self, cols: usize) -> usize {
        let digits = self.buffer.lines.len().max(1).to_string().len();
        digits.min(cols.saturating_sub(2)).max(2) + 1
    }

    fn render(&self) -> Result<(), String> {
        let (rows, cols) = self.terminal_size();
        let content_rows = rows.saturating_sub(3).max(1);
        let gutter = self.gutter_width(cols);
        let visible_width = cols.saturating_sub(gutter + 1).max(1);
        let current_line = self.cursor_line + 1;

        let mut out = String::new();
        out.push_str("\x1b[H\x1b[2J");
        out.push_str("\x1b[?25l");
        let path_label = self
            .buffer
            .path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "[unnamed]".to_string());
        let dirty = if self.dirty { "*" } else { " " };
        let _ = writeln!(
            out,
            "MeScript Native Editor {} {}  line {} col {}",
            dirty,
            path_label,
            current_line,
            self.cursor_col + 1
        );
        let _ = writeln!(
            out,
            "Ctrl-S save  Ctrl-R run  Ctrl-O osc  Ctrl-E fx  Ctrl-P cue  Ctrl-B loop  Ctrl-X stop  Ctrl-Q quit"
        );

        for row in 0..content_rows {
            let line_index = self.scroll_top + row;
            let line_no = line_index + 1;
            let prefix = format!("{:>width$} ", line_no, width = gutter - 1);
            let content = if let Some(line) = self.buffer.lines.get(line_index) {
                slice_by_chars(line, self.scroll_left, self.scroll_left + visible_width)
            } else {
                "~".to_string()
            };
            let highlight = self
                .active_block
                .is_some_and(|range| line_no >= range.start && line_no <= range.end)
                || line_index == self.cursor_line;
            if highlight {
                let _ = writeln!(out, "\x1b[7m{}{}\x1b[0m", prefix, content);
            } else {
                let _ = writeln!(out, "{}{}", prefix, content);
            }
        }

        let _ = writeln!(out, "\x1b[K{}", self.message);
        if let Some(menu) = &self.menu {
            self.render_menu(&mut out, rows, cols, menu);
        }
        let cursor_row = 3 + self.cursor_line.saturating_sub(self.scroll_top);
        let cursor_col = gutter + 1 + self.cursor_col.saturating_sub(self.scroll_left);
        let _ = write!(out, "\x1b[{};{}H\x1b[?25h", cursor_row, cursor_col);
        io::stdout()
            .write_all(out.as_bytes())
            .and_then(|_| io::stdout().flush())
            .map_err(|error| error.to_string())
    }

    fn render_menu(&self, out: &mut String, rows: usize, cols: usize, menu: &MenuState) {
        let width = cols.min(54).max(24);
        let height = (menu.items.len() + 4).min(rows.saturating_sub(2)).max(4);
        let top = 3;
        let left = cols.saturating_sub(width) / 2 + 1;
        let _ = write!(
            out,
            "\x1b[{};{}H\x1b[7m{:width$}\x1b[0m",
            top,
            left,
            menu.title,
            width = width
        );
        for row in 0..height.saturating_sub(2) {
            let item_index = row;
            let label = menu
                .items
                .get(item_index)
                .map(|item| item.label)
                .unwrap_or("");
            let marker = if item_index == menu.selected {
                "> "
            } else {
                "  "
            };
            let text = format!("{}{}", marker, label);
            if item_index == menu.selected {
                let _ = write!(
                    out,
                    "\x1b[{};{}H\x1b[7m{:width$}\x1b[0m",
                    top + row + 1,
                    left,
                    text,
                    width = width
                );
            } else {
                let _ = write!(
                    out,
                    "\x1b[{};{}H{:width$}",
                    top + row + 1,
                    left,
                    text,
                    width = width
                );
            }
        }
        let _ = write!(
            out,
            "\x1b[{};{}H{:width$}",
            top + height - 1,
            left,
            "Enter insert  Esc cancel",
            width = width
        );
    }
}

fn read_key() -> io::Result<Key> {
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    let mut buf = [0u8; 1];
    loop {
        match handle.read(&mut buf)? {
            0 => continue,
            _ => break,
        }
    }
    Ok(match buf[0] {
        b'\r' | b'\n' => Key::Enter,
        0x7f | 0x08 => Key::Backspace,
        0x1b => read_escape_sequence(&mut handle)?,
        0x01..=0x1a => Key::Ctrl((buf[0] + b'a' - 1) as char),
        byte if byte.is_ascii() => Key::Char(byte as char),
        _ => Key::Unknown,
    })
}

fn read_escape_sequence(handle: &mut impl Read) -> io::Result<Key> {
    let mut seq = [0u8; 2];
    match handle.read(&mut seq[..1])? {
        0 => return Ok(Key::Esc),
        _ => {}
    }
    if seq[0] != b'[' && seq[0] != b'O' {
        return Ok(Key::Esc);
    }
    match handle.read(&mut seq[1..2])? {
        0 => return Ok(Key::Esc),
        _ => {}
    }
    Ok(match (seq[0], seq[1]) {
        (b'[', b'A') => Key::Up,
        (b'[', b'B') => Key::Down,
        (b'[', b'C') => Key::Right,
        (b'[', b'D') => Key::Left,
        (b'[', b'H') | (b'O', b'H') => Key::Home,
        (b'[', b'F') | (b'O', b'F') => Key::End,
        (b'[', b'3') => {
            let mut tild = [0u8; 1];
            if handle.read(&mut tild)? == 0 || tild[0] != b'~' {
                Key::Unknown
            } else {
                Key::Delete
            }
        }
        _ => Key::Unknown,
    })
}

fn terminal_size() -> Option<(usize, usize)> {
    let output = Command::new("stty")
        .args(["-F", "/dev/tty", "size"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let mut parts = text.split_whitespace();
    let rows = parts.next()?.parse::<usize>().ok()?;
    let cols = parts.next()?.parse::<usize>().ok()?;
    Some((rows, cols))
}

fn char_to_byte_index(text: &str, char_index: usize) -> usize {
    if char_index == 0 {
        return 0;
    }
    text.char_indices()
        .nth(char_index)
        .map(|(idx, _)| idx)
        .unwrap_or_else(|| text.len())
}

fn char_prev_byte_index(text: &str, char_index: usize) -> usize {
    if char_index == 0 {
        return 0;
    }
    text.char_indices()
        .nth(char_index - 1)
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

fn slice_by_chars(text: &str, start: usize, end: usize) -> String {
    if start >= end {
        return String::new();
    }
    let start_byte = char_to_byte_index(text, start);
    let end_byte = char_to_byte_index(text, end);
    text[start_byte..end_byte].to_string()
}
