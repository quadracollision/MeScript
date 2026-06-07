use crate::effects::EffectSpec;
use crate::effects::offline::OfflineEffectSpec;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Waveform {
    Sine,
    Saw,
    Square,
    Tri,
    Pulse,
    Morph,
    SuperSaw,
    Wavetable,
    FmOp,
    Additive,
    Sync,
    PwmSweep,
    Harsh,
    Chip,
    Pluck,
    Strings,
    Brass,
    Organ,
    Bell,
    Glass,
    Vocal,
    Breath,
    PadWash,
    Click,
    Noise,
    Kick,
    Snare,
    Hat,
    Kick808,
    Snare808,
    Hat808,
    Cowbell808,
    Kick909,
    Snare909,
    Hat909,
    Kick78,
    Snare78,
    Hat78,
    Kick707,
    Snare707,
    Clap,
    CymbalCrash,
    CymbalRide,
    Tom,
    Rimshot,
    Shaker,
    Woodblock,
    Cowbell,
    Zap,
    Scratch,
    Impact,
    BassSlap,
    PianoElectric,
    DroneDark,
    NoiseWhite,
    NoisePink,
    NoiseBrown,
    NoiseBlue,
    NoisePurple,
    Sample,
}

#[derive(Clone, Debug)]
pub(crate) struct TrackEffect {
    pub(crate) spec: EffectSpec,
    pub(crate) gate_subdivisions: Option<Vec<Vec<bool>>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoteMode {
    Step,
    Hit,
    Tick,
}

#[derive(Clone, Debug)]
pub(crate) struct Track {
    pub(crate) id: String,
    pub(crate) waveform: Waveform,
    pub(crate) oscillator: OscillatorParams,
    pub(crate) notes: Vec<f32>,
    pub(crate) note_chords: Vec<Vec<f32>>,
    pub(crate) note_mode: NoteMode,
    pub(crate) gates: Vec<bool>,
    pub(crate) gate_subdivisions: Vec<Vec<bool>>,
    pub(crate) gate_holds: Vec<Vec<usize>>,
    pub(crate) gate_loop_start: usize,
    pub(crate) step_every: usize,
    pub(crate) step_offset: usize,
    pub(crate) amp: f32,
    pub(crate) dur_seconds: f32,
    pub(crate) param_patterns: ParamPatterns,
    pub(crate) effects: Vec<TrackEffect>,
    pub(crate) sample_data: Vec<f32>,
    pub(crate) muted: bool,
    pub(crate) solo: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ParamPatterns {
    pub(crate) amp: Option<Vec<f32>>,
    pub(crate) dur_seconds: Option<Vec<f32>>,
    pub(crate) detune_cents: Option<Vec<f32>>,
    pub(crate) phase: Option<Vec<f32>>,
    pub(crate) pulse_width: Option<Vec<f32>>,
    pub(crate) morph_pos: Option<Vec<f32>>,
    pub(crate) gain: Option<Vec<f32>>,
    pub(crate) unison: Option<Vec<usize>>,
    pub(crate) unison_detune: Option<Vec<f32>>,
    pub(crate) unison_spread: Option<Vec<f32>>,
    pub(crate) fm_ratio: Option<Vec<f32>>,
    pub(crate) fm_depth: Option<Vec<f32>>,
}

#[derive(Clone, Debug)]
pub(crate) struct OscillatorParams {
    pub(crate) detune_cents: f32,
    pub(crate) phase: f32,
    pub(crate) pulse_width: f32,
    pub(crate) morph_pos: f32,
    pub(crate) gain: f32,
    pub(crate) unison: usize,
    pub(crate) unison_detune: f32,
    pub(crate) unison_spread: f32,
    pub(crate) fm_ratio: f32,
    pub(crate) fm_depth: f32,
    pub(crate) harmonics: [f32; 8],
}

impl Default for OscillatorParams {
    fn default() -> Self {
        Self {
            detune_cents: 0.0,
            phase: 0.0,
            pulse_width: 0.5,
            morph_pos: 0.0,
            gain: 1.0,
            unison: 1,
            unison_detune: 0.0,
            unison_spread: 0.0,
            fm_ratio: 2.0,
            fm_depth: 1.0,
            harmonics: [1.0, 0.5, 0.3, 0.25, 0.2, 0.15, 0.1, 0.08],
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Scene {
    pub(crate) id: String,
    pub(crate) steps: usize,
    pub(crate) repeats: usize,
    pub(crate) next: Option<String>,
    pub(crate) tracks: HashMap<String, Track>,
    pub(crate) post_effects: Vec<OfflineEffectSpec>,
}

#[derive(Clone, Debug)]
pub(crate) struct SceneState {
    pub(crate) current: String,
    pub(crate) cycle: usize,
    pub(crate) start_step: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct Runtime {
    pub(crate) bpm: f32,
    pub(crate) running: bool,
    pub(crate) transport_revision: u64,
    pub(crate) tracks: HashMap<String, Track>,
    pub(crate) post_effects: Vec<OfflineEffectSpec>,
    pub(crate) scenes: HashMap<String, Scene>,
    pub(crate) scene_state: Option<SceneState>,
}

impl Runtime {
    pub(crate) fn new() -> Self {
        Self {
            bpm: 124.0,
            running: false,
            transport_revision: 0,
            tracks: HashMap::new(),
            post_effects: Vec::new(),
            scenes: HashMap::new(),
            scene_state: None,
        }
    }

    pub(crate) fn status_summary(&self) -> String {
        let (scene, cycle) = if self.running {
            self.scene_state
                .as_ref()
                .map(|state| {
                    let scene = format!(":{}", state.current);
                    let cycle = self
                        .scenes
                        .get(&state.current)
                        .map(|scene| {
                            if scene.repeats == 0 {
                                format!("{}/loop", state.cycle + 1)
                            } else {
                                format!("{}/{}", state.cycle + 1, scene.repeats)
                            }
                        })
                        .unwrap_or_else(|| (state.cycle + 1).to_string());
                    (scene, cycle)
                })
                .unwrap_or_else(|| ("-".to_string(), "-".to_string()))
        } else {
            ("-".to_string(), "-".to_string())
        };

        format!(
            "bpm={} running={} tracks={} scenes={} scene={} cycle={}",
            self.bpm,
            self.running,
            self.tracks.len(),
            self.scenes.len(),
            scene,
            cycle
        )
    }
}
