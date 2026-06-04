use crate::effects::EffectChain;
use crate::effects::filters::{Biquad, FilterKind};
use crate::effects::offline;
use crate::model::{NoteMode, OscillatorParams, Runtime, TrackEffect, Waveform};
use crate::sequencer;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::HashMap;
use std::f32::consts::TAU;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub(crate) const TRANSPORT_STOPPED_STEP: usize = usize::MAX;

#[derive(Clone, Debug)]
struct Voice {
    waveform: Waveform,
    params: OscillatorParams,
    freq: f32,
    amp: f32,
    pan: f32,
    phase: f32,
    aux_phase: [f32; 8],
    age: f32,
    dur: f32,
    noise: u32,
    sample_rate: f32,
    filters: Vec<Biquad>,
    delay_line: Vec<f32>,
    delay_pos: usize,
    sample_data: Vec<f32>,
    sample_pos: usize,
    color_state: [f32; 8],
    effects: EffectChain,
}

#[derive(Clone, Debug)]
struct PendingTrigger {
    samples_until: f32,
    track: crate::model::Track,
    freq: f32,
    seed_step: usize,
    track_step: usize,
    sub_index: usize,
    param_index: usize,
    hold_seconds: f32,
}

impl Voice {
    fn new(
        track: &crate::model::Track,
        freq: f32,
        step: usize,
        track_step: usize,
        sub_index: usize,
        param_index: usize,
        hold_seconds: f32,
        sample_rate: f32,
    ) -> Self {
        let params = effective_oscillator_params(track, param_index);
        let amp = pattern_f32_or(&track.param_patterns.amp, track.amp, param_index).clamp(0.0, 1.0);
        let dur_seconds = pattern_f32_or(
            &track.param_patterns.dur_seconds,
            track.dur_seconds,
            param_index,
        )
        .clamp(0.005, 4.0);
        Self::with_params(
            track,
            freq,
            step,
            track_step,
            sub_index,
            hold_seconds,
            sample_rate,
            params,
            0.5,
            amp,
            dur_seconds,
        )
    }

    fn with_params(
        track: &crate::model::Track,
        freq: f32,
        step: usize,
        track_step: usize,
        sub_index: usize,
        hold_seconds: f32,
        sample_rate: f32,
        params: OscillatorParams,
        pan: f32,
        amp: f32,
        dur_seconds: f32,
    ) -> Self {
        let mut noise = seed_from_step(&track.id, step);
        let delay_line = if matches!(track.waveform, Waveform::Pluck) {
            let delay_samples = pluck_delay_samples_for_test(sample_rate, freq);
            (0..delay_samples)
                .map(|_| signed_noise(&mut noise))
                .collect()
        } else {
            Vec::new()
        };
        let phase = params.phase;
        let effects = active_effect_specs(&track.effects, track_step, sub_index);
        let dur = (dur_seconds + hold_seconds.max(0.0)).max(0.005);
        Self {
            waveform: track.waveform,
            params,
            freq,
            amp,
            pan: pan.clamp(0.0, 1.0),
            phase,
            aux_phase: [0.0; 8],
            age: 0.0,
            dur,
            noise,
            sample_rate,
            filters: voice_filters(track.waveform, freq, sample_rate),
            delay_line,
            delay_pos: 0,
            sample_data: track.sample_data.clone(),
            sample_pos: 0,
            color_state: [0.0; 8],
            effects: EffectChain::new_with_duration(&effects, sample_rate, Some(dur)),
        }
    }
}

pub(crate) fn active_effect_specs(
    effects: &[TrackEffect],
    track_step: usize,
    sub_index: usize,
) -> Vec<crate::effects::EffectSpec> {
    effects
        .iter()
        .filter(|effect| {
            let Some(gates) = &effect.gate_subdivisions else {
                return true;
            };
            let step_gates = sequencer::pattern_step_gates(gates, track_step);
            if step_gates.len() <= 1 {
                return step_gates.first().copied().unwrap_or(false);
            }
            step_gates[sub_index % step_gates.len()]
        })
        .map(|effect| effect.spec.clone())
        .collect()
}

fn note_at(values: &[f32], index: usize) -> f32 {
    if values.is_empty() {
        440.0
    } else {
        values[index % values.len()]
    }
}

fn pattern_f32_or(pattern: &Option<Vec<f32>>, fallback: f32, index: usize) -> f32 {
    pattern
        .as_ref()
        .and_then(|values| values.get(index % values.len().max(1)).copied())
        .unwrap_or(fallback)
}

fn pattern_usize_or(pattern: &Option<Vec<usize>>, fallback: usize, index: usize) -> usize {
    pattern
        .as_ref()
        .and_then(|values| values.get(index % values.len().max(1)).copied())
        .unwrap_or(fallback)
}

fn effective_oscillator_params(track: &crate::model::Track, index: usize) -> OscillatorParams {
    let mut params = track.oscillator.clone();
    params.detune_cents = pattern_f32_or(
        &track.param_patterns.detune_cents,
        params.detune_cents,
        index,
    );
    params.phase = pattern_f32_or(&track.param_patterns.phase, params.phase, index).rem_euclid(1.0);
    params.pulse_width =
        pattern_f32_or(&track.param_patterns.pulse_width, params.pulse_width, index)
            .clamp(0.01, 0.99);
    params.morph_pos =
        pattern_f32_or(&track.param_patterns.morph_pos, params.morph_pos, index).clamp(0.0, 1.0);
    params.gain = pattern_f32_or(&track.param_patterns.gain, params.gain, index).clamp(0.0, 2.0);
    params.unison =
        pattern_usize_or(&track.param_patterns.unison, params.unison, index).clamp(1, 10);
    params.unison_detune = pattern_f32_or(
        &track.param_patterns.unison_detune,
        params.unison_detune,
        index,
    )
    .clamp(0.0, 100.0);
    params.unison_spread = pattern_f32_or(
        &track.param_patterns.unison_spread,
        params.unison_spread,
        index,
    )
    .clamp(0.0, 1.0);
    params.fm_ratio =
        pattern_f32_or(&track.param_patterns.fm_ratio, params.fm_ratio, index).max(0.01);
    params.fm_depth =
        pattern_f32_or(&track.param_patterns.fm_depth, params.fm_depth, index).clamp(0.0, 32.0);
    params
}

pub(crate) struct AudioEngine {
    runtime: Arc<Mutex<Runtime>>,
    sample_rate: f32,
    step_samples: f32,
    samples_until_step: f32,
    step: usize,
    voices: Vec<Voice>,
    pending_triggers: Vec<PendingTrigger>,
    note_cursors: HashMap<String, usize>,
    param_cursors: HashMap<String, usize>,
    step_sender: Option<Sender<usize>>,
    seen_transport_revision: u64,
}

impl AudioEngine {
    pub(crate) fn new(runtime: Runtime, sample_rate: f32) -> Self {
        let step_samples = samples_per_step(runtime.bpm, sample_rate);
        let revision = runtime.transport_revision;
        Self::with_shared_runtime(
            Arc::new(Mutex::new(runtime)),
            sample_rate,
            step_samples,
            revision,
        )
    }

    pub(crate) fn new_shared(runtime: Arc<Mutex<Runtime>>, sample_rate: f32) -> Self {
        let locked = runtime.lock().expect("runtime lock poisoned");
        let bpm = locked.bpm;
        let revision = locked.transport_revision;
        drop(locked);
        let step_samples = samples_per_step(bpm, sample_rate);
        Self::with_shared_runtime(runtime, sample_rate, step_samples, revision)
    }

    fn with_shared_runtime(
        runtime: Arc<Mutex<Runtime>>,
        sample_rate: f32,
        step_samples: f32,
        seen_transport_revision: u64,
    ) -> Self {
        Self {
            runtime,
            sample_rate,
            step_samples,
            samples_until_step: 0.0,
            step: 0,
            voices: Vec::new(),
            pending_triggers: Vec::new(),
            note_cursors: HashMap::new(),
            param_cursors: HashMap::new(),
            step_sender: None,
            seen_transport_revision,
        }
    }

    pub(crate) fn set_step_sender(&mut self, sender: Sender<usize>) {
        self.step_sender = Some(sender);
    }

    fn reset_transport(&mut self, revision: u64, bpm: f32) {
        self.seen_transport_revision = revision;
        self.step_samples = samples_per_step(bpm, self.sample_rate);
        self.samples_until_step = 0.0;
        self.step = 0;
        self.voices.clear();
        self.pending_triggers.clear();
        self.note_cursors.clear();
        self.param_cursors.clear();
    }

    fn trigger_step(&mut self) {
        let runtime_header = self.runtime.lock().expect("runtime lock poisoned").clone();
        if runtime_header.transport_revision != self.seen_transport_revision {
            self.reset_transport(runtime_header.transport_revision, runtime_header.bpm);
        }

        let advanced_scene = self.advance_scene_if_needed();
        if advanced_scene {
            self.voices.clear();
            self.pending_triggers.clear();
            self.note_cursors.clear();
            self.param_cursors.clear();
        }

        let runtime = self.runtime.lock().expect("runtime lock poisoned").clone();
        self.step_samples = samples_per_step(runtime.bpm, self.sample_rate);
        if !runtime.running {
            if advanced_scene {
                if let Some(sender) = &self.step_sender {
                    let _ = sender.send(TRANSPORT_STOPPED_STEP);
                }
            }
            return;
        }

        if let Some(sender) = &self.step_sender {
            let _ = sender.send(self.step);
        }

        let solo_active = runtime.tracks.values().any(|track| track.solo);
        for track in runtime.tracks.values() {
            if track.muted || (solo_active && !track.solo) {
                continue;
            }
            let step_every = track.step_every.max(1);
            if self.step % step_every != 0 {
                continue;
            }
            let track_step = (self.step / step_every).wrapping_add(track.step_offset);
            let gates = sequencer::pattern_step_gates(&track.gate_subdivisions, track_step);
            let holds = sequencer::pattern_step_holds(&track.gate_holds, track_step);
            if gates.len() <= 1 {
                let gate = gates
                    .first()
                    .copied()
                    .unwrap_or_else(|| sequencer::pattern_bool(&track.gates, track_step));
                let freq = self.note_for_slot(track, track_step, gate);
                if gate {
                    let hold = holds.first().copied().unwrap_or(0);
                    let hold_seconds = hold as f32 * self.step_samples / self.sample_rate;
                    let param_index = self.next_param_index_for_track(track);
                    self.push_track_voices(
                        track,
                        freq,
                        self.step,
                        track_step,
                        0,
                        param_index,
                        hold_seconds,
                    );
                }
                continue;
            }

            let sub_step_samples = self.step_samples / gates.len() as f32;
            for (idx, gate) in gates.iter().enumerate() {
                let freq = self.note_for_slot(track, track_step, *gate);
                if !gate {
                    continue;
                }
                let seed_step = self.step.wrapping_mul(1024).wrapping_add(idx);
                let hold = holds.get(idx).copied().unwrap_or(0);
                let hold_seconds = hold as f32 * sub_step_samples / self.sample_rate;
                let param_index = self.next_param_index_for_track(track);
                if idx == 0 {
                    self.push_track_voices(
                        track,
                        freq,
                        seed_step,
                        track_step,
                        idx,
                        param_index,
                        hold_seconds,
                    );
                } else {
                    self.pending_triggers.push(PendingTrigger {
                        samples_until: sub_step_samples * idx as f32,
                        track: track.clone(),
                        freq,
                        seed_step,
                        track_step,
                        sub_index: idx,
                        param_index,
                        hold_seconds,
                    });
                }
            }
        }

        self.step = self.step.wrapping_add(1);
    }

    fn note_for_slot(&mut self, track: &crate::model::Track, track_step: usize, gate: bool) -> f32 {
        match track.note_mode {
            NoteMode::Step => sequencer::pattern_f32(&track.notes, track_step),
            NoteMode::Hit if gate => self.next_note_for_track(track),
            NoteMode::Hit => note_at(
                &track.notes,
                *self.note_cursors.get(&track.id).unwrap_or(&0),
            ),
            NoteMode::Tick => self.next_note_for_track(track),
        }
    }

    fn next_note_for_track(&mut self, track: &crate::model::Track) -> f32 {
        let cursor = self.note_cursors.entry(track.id.clone()).or_insert(0);
        let freq = note_at(&track.notes, *cursor);
        *cursor = cursor.wrapping_add(1);
        freq
    }

    fn next_param_index_for_track(&mut self, track: &crate::model::Track) -> usize {
        let cursor = self.param_cursors.entry(track.id.clone()).or_insert(0);
        let index = *cursor;
        *cursor = cursor.wrapping_add(1);
        index
    }

    fn push_track_voices(
        &mut self,
        track: &crate::model::Track,
        freq: f32,
        seed_step: usize,
        track_step: usize,
        sub_index: usize,
        param_index: usize,
        hold_seconds: f32,
    ) {
        let effective_params = effective_oscillator_params(track, param_index);
        let amp = pattern_f32_or(&track.param_patterns.amp, track.amp, param_index).clamp(0.0, 1.0);
        let dur_seconds = pattern_f32_or(
            &track.param_patterns.dur_seconds,
            track.dur_seconds,
            param_index,
        )
        .clamp(0.005, 4.0);
        let unison = effective_params.unison.max(1);
        if unison == 1 {
            self.voices.push(Voice::new(
                track,
                freq,
                seed_step,
                track_step,
                sub_index,
                param_index,
                hold_seconds,
                self.sample_rate,
            ));
            return;
        }

        let center = (unison - 1) as f32 * 0.5;
        for idx in 0..unison {
            let position = if center > 0.0 {
                (idx as f32 - center) / center
            } else {
                0.0
            };
            let mut params = effective_params.clone();
            params.unison = 1;
            params.detune_cents += position * effective_params.unison_detune;
            params.phase = (params.phase + idx as f32 / 1.618_034).fract();
            let pan = (0.5 + position * effective_params.unison_spread * 0.5).clamp(0.0, 1.0);
            self.voices.push(Voice::with_params(
                track,
                freq,
                seed_step,
                track_step,
                sub_index,
                hold_seconds,
                self.sample_rate,
                params,
                pan,
                amp,
                dur_seconds,
            ));
        }
    }

    fn trigger_pending(&mut self) {
        let mut due = Vec::new();
        let mut waiting = Vec::new();
        for mut trigger in self.pending_triggers.drain(..) {
            trigger.samples_until -= 1.0;
            if trigger.samples_until <= 0.0 {
                due.push(trigger);
            } else {
                waiting.push(trigger);
            }
        }
        self.pending_triggers = waiting;
        for trigger in due {
            self.push_track_voices(
                &trigger.track,
                trigger.freq,
                trigger.seed_step,
                trigger.track_step,
                trigger.sub_index,
                trigger.param_index,
                trigger.hold_seconds,
            );
        }
    }

    fn advance_scene_if_needed(&mut self) -> bool {
        let mut runtime = self.runtime.lock().expect("runtime lock poisoned");
        let Some(state) = runtime.scene_state.clone() else {
            return false;
        };
        let Some(scene) = runtime.scenes.get(&state.current).cloned() else {
            runtime.scene_state = None;
            return false;
        };
        let scene_steps = scene.steps.max(1);
        if self.step == 0 || self.step % scene_steps != 0 {
            return false;
        }

        let next_cycle = state.cycle + 1;
        if scene.repeats == 0 || next_cycle < scene.repeats {
            if let Some(active) = runtime.scene_state.as_mut() {
                active.cycle = next_cycle;
            }
            return false;
        }

        let Some(next_id) = scene.next else {
            runtime.running = false;
            runtime.scene_state = None;
            return true;
        };
        let Some(next_scene) = runtime.scenes.get(&next_id).cloned() else {
            runtime.running = false;
            runtime.scene_state = None;
            return true;
        };
        let same_scene = next_id == state.current;

        runtime.tracks = next_scene.tracks;
        runtime.post_effects = next_scene.post_effects;
        runtime.scene_state = Some(crate::model::SceneState {
            current: next_scene.id,
            cycle: 0,
        });
        !same_scene
    }

    #[allow(dead_code)]
    pub(crate) fn next_sample(&mut self) -> f32 {
        let frame = self.next_frame();
        (frame[0] + frame[1]) * 0.5
    }

    pub(crate) fn next_frame(&mut self) -> [f32; 2] {
        self.samples_until_step -= 1.0;
        if self.samples_until_step <= 0.0 {
            self.trigger_step();
            self.samples_until_step += self.step_samples;
        }
        self.trigger_pending();

        let dt = 1.0 / self.sample_rate;
        let mut out = [0.0, 0.0];
        for voice in &mut self.voices {
            let env = amplitude_envelope(voice);
            let mut sample = oscillator(voice) * voice.params.gain * voice.amp * env;
            sample = voice.effects.process(sample, self.sample_rate);
            let (left_gain, right_gain) = pan_gains(voice.pan);
            out[0] += sample * left_gain;
            out[1] += sample * right_gain;
            voice.age += dt;
            voice.phase = (voice.phase + voice.freq * dt) % 1.0;
        }
        self.voices
            .retain(|voice| voice.age < voice.dur + voice.effects.tail_seconds());
        [soft_clip(out[0] * 0.8), soft_clip(out[1] * 0.8)]
    }

    #[cfg(test)]
    pub(crate) fn note_cursor_for_test(&self, track_id: &str) -> usize {
        *self.note_cursors.get(track_id).unwrap_or(&0)
    }
}

fn pan_gains(pan: f32) -> (f32, f32) {
    let pan = pan.clamp(0.0, 1.0);
    ((1.0 - pan).sqrt(), pan.sqrt())
}

fn voice_filters(waveform: Waveform, freq: f32, sample_rate: f32) -> Vec<Biquad> {
    match waveform {
        Waveform::Snare => vec![
            Biquad::new(FilterKind::Highpass, 1_000.0, 0.2, sample_rate),
            Biquad::new(FilterKind::Lowpass, 8_000.0, 0.2, sample_rate),
        ],
        Waveform::Snare808 => vec![
            Biquad::new(FilterKind::Highpass, 1_000.0, 0.2, sample_rate),
            Biquad::new(FilterKind::Lowpass, 6_000.0, 0.2, sample_rate),
        ],
        Waveform::Clap => vec![
            Biquad::new(FilterKind::Highpass, 800.0, 0.2, sample_rate),
            Biquad::new(FilterKind::Lowpass, 4_000.0, 0.2, sample_rate),
        ],
        Waveform::Hat => vec![Biquad::new(FilterKind::Highpass, 5_000.0, 0.2, sample_rate)],
        Waveform::Hat808 => vec![
            Biquad::new(FilterKind::Highpass, 7_000.0, 0.2, sample_rate),
            Biquad::new(FilterKind::Highpass, 7_000.0, 0.2, sample_rate),
        ],
        Waveform::Cowbell808 => {
            let ratio = if freq > 0.0 { freq / 440.0 } else { 1.0 };
            let f1 = 540.0 * ratio;
            let f2 = 800.0 * ratio;
            vec![
                Biquad::new(FilterKind::Highpass, f1 * 0.8, 0.2, sample_rate),
                Biquad::new(FilterKind::Lowpass, f2 * 1.5, 0.2, sample_rate),
            ]
        }
        Waveform::Snare909 => vec![
            Biquad::new(FilterKind::Highpass, 1_000.0, 0.2, sample_rate),
            Biquad::new(FilterKind::Lowpass, 8_000.0, 0.2, sample_rate),
        ],
        Waveform::Hat909 => vec![Biquad::new(FilterKind::Highpass, 6_000.0, 0.2, sample_rate)],
        Waveform::Snare78 => vec![Biquad::new(FilterKind::Highpass, 2_000.0, 0.2, sample_rate)],
        Waveform::Hat78 => vec![
            Biquad::new(FilterKind::Highpass, 5_000.0, 0.2, sample_rate),
            Biquad::new(FilterKind::Lowpass, 9_000.0, 0.2, sample_rate),
        ],
        Waveform::Kick707 => vec![Biquad::new(FilterKind::Lowpass, 400.0, 0.2, sample_rate)],
        Waveform::CymbalCrash => vec![
            Biquad::new(FilterKind::Highpass, 2_000.0, 0.2, sample_rate),
            Biquad::new(FilterKind::Highpass, 4_000.0, 0.2, sample_rate),
        ],
        Waveform::CymbalRide => vec![Biquad::new(FilterKind::Highpass, 6_000.0, 0.2, sample_rate)],
        Waveform::Rimshot => vec![Biquad::new(FilterKind::Bandpass, 1_800.0, 1.0, sample_rate)],
        Waveform::Shaker => vec![Biquad::new(FilterKind::Highpass, 6_000.0, 0.2, sample_rate)],
        Waveform::Cowbell => {
            let f = if freq > 300.0 { freq } else { 580.0 };
            vec![
                Biquad::new(FilterKind::Highpass, f, 0.2, sample_rate),
                Biquad::new(FilterKind::Lowpass, f * 2.5, 0.2, sample_rate),
            ]
        }
        Waveform::Scratch => vec![
            Biquad::new(FilterKind::Highpass, 800.0, 0.2, sample_rate),
            Biquad::new(FilterKind::Lowpass, 1_600.0, 0.2, sample_rate),
        ],
        Waveform::Impact => vec![Biquad::new(FilterKind::Lowpass, 200.0, 0.2, sample_rate)],
        Waveform::DroneDark => vec![Biquad::new(
            FilterKind::Lowpass,
            (freq * 4.0).max(20.0),
            0.2,
            sample_rate,
        )],
        Waveform::Strings => vec![Biquad::new(
            FilterKind::Lowpass,
            (freq * 4.0).max(20.0),
            0.2,
            sample_rate,
        )],
        Waveform::Breath => vec![
            Biquad::new(
                FilterKind::Highpass,
                (freq * 0.5).max(20.0),
                0.2,
                sample_rate,
            ),
            Biquad::new(
                FilterKind::Lowpass,
                (freq * 3.0).min(sample_rate * 0.45),
                0.2,
                sample_rate,
            ),
        ],
        _ => Vec::new(),
    }
}

fn samples_per_step(bpm: f32, sample_rate: f32) -> f32 {
    let beats_per_second = bpm.max(1.0) / 60.0;
    let steps_per_second = beats_per_second * 4.0;
    sample_rate / steps_per_second
}

fn envelope(age: f32, dur: f32) -> f32 {
    let attack = 0.006;
    let release = 0.035;
    if age < attack {
        age / attack
    } else if age > dur {
        (1.0 - ((age - dur) / release)).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

fn amplitude_envelope(voice: &Voice) -> f32 {
    if matches!(voice.waveform, Waveform::Sample) {
        1.0
    } else if is_percussive(voice.waveform) {
        if voice.age > voice.dur {
            (1.0 - ((voice.age - voice.dur) / 0.01)).clamp(0.0, 1.0)
        } else {
            1.0
        }
    } else {
        envelope(voice.age, voice.dur)
    }
}

fn is_percussive(waveform: Waveform) -> bool {
    matches!(
        waveform,
        Waveform::Click
            | Waveform::Kick
            | Waveform::Snare
            | Waveform::Hat
            | Waveform::Kick808
            | Waveform::Snare808
            | Waveform::Hat808
            | Waveform::Cowbell808
            | Waveform::Kick909
            | Waveform::Snare909
            | Waveform::Hat909
            | Waveform::Kick78
            | Waveform::Snare78
            | Waveform::Hat78
            | Waveform::Kick707
            | Waveform::Snare707
            | Waveform::Clap
            | Waveform::CymbalCrash
            | Waveform::CymbalRide
            | Waveform::Tom
            | Waveform::Rimshot
            | Waveform::Shaker
            | Waveform::Woodblock
            | Waveform::Cowbell
            | Waveform::Zap
            | Waveform::Scratch
            | Waveform::Impact
    )
}

fn oscillator(voice: &mut Voice) -> f32 {
    match voice.waveform {
        Waveform::Sine => unison(voice, sine_phase),
        Waveform::Saw => unison(voice, saw_phase),
        Waveform::Square => {
            if voice.phase < voice.params.pulse_width {
                1.0
            } else {
                -1.0
            }
        }
        Waveform::Tri => unison(voice, tri_phase),
        Waveform::Pulse => pulse(voice.phase, voice.params.pulse_width),
        Waveform::Morph => morph(voice.phase, voice.params.morph_pos),
        Waveform::SuperSaw => supersaw(voice),
        Waveform::Wavetable => wavetable(voice.phase, voice.params.morph_pos),
        Waveform::FmOp => fm_op(voice),
        Waveform::Additive => additive(voice),
        Waveform::Sync => sync_osc(voice.phase, voice.params.fm_ratio),
        Waveform::PwmSweep => pwm_sweep(voice),
        Waveform::Harsh => harsh(voice.phase),
        Waveform::Chip => quantize((voice.phase * 2.0) - 1.0, 16.0),
        Waveform::Pluck => pluck(voice),
        Waveform::Strings => strings(voice),
        Waveform::Brass => brass(voice.phase),
        Waveform::Organ => organ(voice.phase),
        Waveform::Bell => bell(voice),
        Waveform::Glass => glass(voice),
        Waveform::Vocal => vocal(voice),
        Waveform::Breath => breath(voice),
        Waveform::PadWash => pad_wash(voice),
        Waveform::Click => click(voice),
        Waveform::Kick => kick(voice),
        Waveform::Snare => snare(voice),
        Waveform::Hat => hat(voice),
        Waveform::Kick808 => kick_808(voice),
        Waveform::Snare808 => snare_808(voice),
        Waveform::Hat808 => hat_808(voice),
        Waveform::Cowbell808 => cowbell_808(voice),
        Waveform::Kick909 => kick_909(voice),
        Waveform::Snare909 => snare_909(voice),
        Waveform::Hat909 => hat_909(voice),
        Waveform::Kick78 => kick_78(voice),
        Waveform::Snare78 => snare_78(voice),
        Waveform::Hat78 => hat_78(voice),
        Waveform::Kick707 => kick_707(voice),
        Waveform::Snare707 => snare_707(voice),
        Waveform::Clap => clap(voice),
        Waveform::CymbalCrash => cymbal_crash(voice),
        Waveform::CymbalRide => cymbal_ride(voice),
        Waveform::Tom => tom(voice),
        Waveform::Rimshot => rimshot(voice),
        Waveform::Shaker => shaker(voice),
        Waveform::Woodblock => woodblock(voice),
        Waveform::Cowbell => cowbell(voice),
        Waveform::Zap => zap(voice),
        Waveform::Scratch => scratch(voice),
        Waveform::Impact => impact(voice),
        Waveform::BassSlap => bass_slap(voice),
        Waveform::PianoElectric => piano_electric(voice),
        Waveform::DroneDark => drone_dark(voice),
        Waveform::NoiseWhite => signed_noise(&mut voice.noise),
        Waveform::NoisePink => pink_noise(voice),
        Waveform::NoiseBrown => brown_noise(voice),
        Waveform::NoiseBlue => blue_noise(voice),
        Waveform::NoisePurple => purple_noise(voice),
        Waveform::Sample => sample_playback(voice),
        Waveform::Noise => {
            voice.noise = voice.noise.wrapping_mul(1664525).wrapping_add(1013904223);
            ((voice.noise >> 8) as f32 / 16_777_216.0) * 2.0 - 1.0
        }
    }
}

fn pulse(phase: f32, width: f32) -> f32 {
    if phase < width.clamp(0.01, 0.99) {
        1.0
    } else {
        -1.0
    }
}

fn sine_phase(phase: f32) -> f32 {
    (phase * TAU).sin()
}

fn saw_phase(phase: f32) -> f32 {
    phase * 2.0 - 1.0
}

fn tri_phase(phase: f32) -> f32 {
    1.0 - (4.0 * (phase - 0.5).abs())
}

fn unison(voice: &Voice, oscillator: fn(f32) -> f32) -> f32 {
    let voices = voice.params.unison.max(1);
    if voices == 1 {
        return oscillator(detuned_phase(voice, 0.0));
    }
    let spread = voice.params.unison_detune;
    let center = (voices - 1) as f32 * 0.5;
    let mut out = 0.0;
    for idx in 0..voices {
        let position = if center > 0.0 {
            (idx as f32 - center) / center
        } else {
            0.0
        };
        let phase_offset = if idx == 0 {
            0.0
        } else {
            idx as f32 / 1.618_034
        };
        out += oscillator((detuned_phase(voice, position * spread) + phase_offset).fract());
    }
    out / (voices as f32).sqrt()
}

fn detuned_phase(voice: &Voice, extra_cents: f32) -> f32 {
    let cents = voice.params.detune_cents + extra_cents;
    (voice.phase * 2.0_f32.powf(cents / 1_200.0)).fract()
}

fn morph(phase: f32, morph_pos: f32) -> f32 {
    let tri = 1.0 - (4.0 * (phase - 0.5).abs());
    let saw = phase * 2.0 - 1.0;
    let square = pulse(phase, 0.5);
    let morph_pos = morph_pos.clamp(0.0, 1.0);
    if morph_pos <= 0.5 {
        let amount = morph_pos * 2.0;
        tri * (1.0 - amount) + saw * amount
    } else {
        let amount = (morph_pos - 0.5) * 2.0;
        saw * (1.0 - amount) + square * amount
    }
}

fn supersaw(voice: &Voice) -> f32 {
    let phase = detuned_phase(voice, 0.0);
    let spread = voice.params.morph_pos.max(0.01);
    let detunes = [-0.011, -0.0063, -0.002, 0.0, 0.002, 0.0062, 0.0107];
    detunes
        .iter()
        .map(|detune| ((phase * (1.0 + detune * spread)).fract() * 2.0) - 1.0)
        .sum::<f32>()
        / 4.9
}

fn wavetable(phase: f32, morph_pos: f32) -> f32 {
    let frames = [
        (phase * TAU).sin(),
        (phase * TAU).sin() * 0.7 + (phase * TAU * 2.0).sin() * 0.3,
        (phase * TAU).sin() * 0.5
            + (phase * TAU * 2.0).sin() * 0.3
            + (phase * TAU * 3.0).sin() * 0.2,
        (phase * TAU).sin() * 0.6
            + (phase * TAU * 3.0).sin() * 0.25
            + (phase * TAU * 5.0).sin() * 0.15,
        tri_phase(phase) * 0.5 + (phase * TAU * 4.0).sin() * 0.3 + (phase * TAU * 6.0).sin() * 0.2,
        saw_phase(phase) * 0.6 + (phase * TAU).sin() * 0.4,
        saw_phase(phase) * 0.8
            + (phase * TAU * 2.0).sin() * 0.15
            + (phase * TAU * 4.0).sin() * 0.05,
        pulse(phase, 0.5) * 0.5 + saw_phase(phase) * 0.3 + (phase * TAU * 5.0).sin() * 0.2,
    ];
    let frame = morph_pos.clamp(0.0, 1.0) * 7.0;
    let lo = frame.floor() as usize;
    let hi = (lo + 1).min(7);
    let frac = frame - lo as f32;
    frames[lo] * (1.0 - frac) + frames[hi] * frac
}

fn fm_op(voice: &Voice) -> f32 {
    let phase = voice.phase * TAU;
    let modulator = (phase * voice.params.fm_ratio).sin() * voice.params.fm_depth;
    (phase + modulator).sin()
}

fn additive(voice: &mut Voice) -> f32 {
    let phase = detuned_phase(voice, 0.0);
    let sum = voice
        .params
        .harmonics
        .iter()
        .take(8)
        .enumerate()
        .filter(|(idx, amp)| {
            **amp > 0.001 && voice.freq * (*idx as f32 + 1.0) < voice.sample_rate * 0.45
        })
        .map(|(idx, amp)| (phase * TAU * (idx + 1) as f32).sin() * amp)
        .sum::<f32>();
    normalize_voice_peak(voice, 1, sum)
}

fn sync_osc(phase: f32, fm_ratio: f32) -> f32 {
    ((phase * fm_ratio.max(1.01)).fract() * TAU).sin()
}

fn pwm_sweep(voice: &Voice) -> f32 {
    let width = 0.5 + 0.35 * (voice.age * TAU * voice.params.fm_ratio * 0.5).sin();
    pulse(voice.phase, width)
}

fn harsh(phase: f32) -> f32 {
    let saw = phase * 2.0 - 1.0;
    let folded = (saw.abs() * std::f32::consts::PI * 3.0).sin();
    folded * 0.7 + saw * 0.3
}

fn quantize(sample: f32, levels: f32) -> f32 {
    (sample * levels).round() / levels
}

fn pluck(voice: &mut Voice) -> f32 {
    if voice.delay_line.is_empty() {
        return 0.0;
    }
    let current = voice.delay_line[voice.delay_pos];
    let next_pos = (voice.delay_pos + 1) % voice.delay_line.len();
    let next = voice.delay_line[next_pos];
    voice.delay_line[voice.delay_pos] = 0.996 * 0.5 * (current + next);
    voice.delay_pos = next_pos;
    current
}

pub(crate) fn pluck_delay_samples_for_test(sample_rate: f32, freq: f32) -> usize {
    (sample_rate / freq.max(20.0)).floor().max(2.0) as usize
}

fn strings(voice: &mut Voice) -> f32 {
    let saw = voice.phase * 2.0 - 1.0;
    let bow_noise = signed_noise(&mut voice.noise) * 0.05;
    let bow_noise = filter_sample(voice, 0, bow_noise);
    saw * 0.7
        + bow_noise
        + (voice.phase * TAU * 2.0).sin() * 0.15
        + (voice.phase * TAU * 3.0).sin() * 0.08
}

fn brass(phase: f32) -> f32 {
    (phase * 2.0 - 1.0) * 0.6
        + (phase * TAU * 2.0).sin() * 0.25
        + (phase * TAU * 3.0).sin() * 0.2
        + (phase * TAU * 5.0).sin() * 0.1
        + (phase * TAU * 7.0).sin() * 0.05
}

fn organ(phase: f32) -> f32 {
    let ratios = [0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0];
    let levels = [0.8, 1.0, 0.6, 0.8, 0.5, 0.7, 0.3, 0.4, 0.3];
    ratios
        .iter()
        .zip(levels)
        .map(|(ratio, level)| (phase * TAU * ratio).sin() * level)
        .sum::<f32>()
        / 5.4
}

fn bell(voice: &Voice) -> f32 {
    let phase = voice.phase * TAU;
    let mod_env = (-voice.age * 5.0).exp() * voice.params.fm_depth * 3.0;
    let amp = (-voice.age * 2.0).exp();
    (phase + (phase * 1.414).sin() * mod_env).sin() * amp
}

fn glass(voice: &mut Voice) -> f32 {
    let ratios = [1.0, 2.17, 3.31, 4.57, 5.84, 7.12];
    let amps = [1.0, 0.6, 0.4, 0.25, 0.15, 0.08];
    let out = ratios
        .iter()
        .zip(amps)
        .map(|(ratio, amp)| {
            (voice.phase * TAU * ratio).sin() * amp * (-voice.age * (1.5 + ratio * 0.5)).exp()
        })
        .sum::<f32>();
    normalize_voice_peak(voice, 2, out)
}

fn vocal(voice: &Voice) -> f32 {
    let glottal = (voice.phase * 2.0 - 1.0) * 0.5;
    let vowels = [
        (800.0, 1_200.0),
        (600.0, 1_800.0),
        (300.0, 2_500.0),
        (500.0, 900.0),
        (350.0, 700.0),
    ];
    let pos = voice.params.morph_pos.clamp(0.0, 1.0) * 4.0;
    let lo = pos.floor() as usize;
    let hi = (lo + 1).min(4);
    let frac = pos - lo as f32;
    let f1 = vowels[lo].0 * (1.0 - frac) + vowels[hi].0 * frac;
    let f2 = vowels[lo].1 * (1.0 - frac) + vowels[hi].1 * frac;
    glottal + (voice.age * TAU * f1).sin() * 0.3 + (voice.age * TAU * f2).sin() * 0.2
}

fn breath(voice: &mut Voice) -> f32 {
    let combined = signed_noise(&mut voice.noise) * 0.3 + (voice.phase * TAU).sin() * 0.4;
    let filtered = filter_sample(voice, 0, combined);
    let filtered = filter_sample(voice, 1, filtered);
    normalize_voice_peak(voice, 3, filtered)
}

fn pad_wash(voice: &Voice) -> f32 {
    [-7.0, -3.0, 0.0, 3.0, 7.0]
        .iter()
        .enumerate()
        .map(|(idx, detune)| {
            let ratio = 2.0_f32.powf((*detune + voice.params.detune_cents) / 1_200.0);
            let phase_offset = if idx == 0 {
                0.0
            } else {
                idx as f32 / 1.618_034
            };
            ((voice.phase * ratio + phase_offset).fract() * TAU).sin()
        })
        .sum::<f32>()
        / 5.0
}

fn click(voice: &Voice) -> f32 {
    if voice.age > 0.005 {
        0.0
    } else {
        (voice.age * TAU * voice.freq).sin() * (-voice.age * 650.0).exp()
    }
}

fn sample_playback(voice: &mut Voice) -> f32 {
    let Some(sample) = voice.sample_data.get(voice.sample_pos).copied() else {
        return 0.0;
    };
    voice.sample_pos += 1;
    sample
}

fn kick(voice: &mut Voice) -> f32 {
    let end_freq = voice.freq.clamp(30.0, 80.0);
    let start_freq = 150.0_f32.max(end_freq * 5.0) + voice.freq * 0.3;
    let f = end_freq + (start_freq - end_freq) * (-voice.age * 40.0).exp();
    (advance_aux_phase(voice, 0, f) * TAU).sin() * (-voice.age * 6.0).exp()
}

fn snare(voice: &mut Voice) -> f32 {
    let sweep = voice.freq + 80.0 * (-voice.age * 50.0).exp();
    let tone = (advance_aux_phase(voice, 0, sweep) * TAU).sin() * (-voice.age * 15.0).exp() * 0.6;
    let noise = filtered_noise(voice, 0) * (-voice.age * 10.0).exp() * 0.5;
    tone + noise
}

fn hat(voice: &mut Voice) -> f32 {
    let ratios = [1.0, 1.47, 1.84, 2.55, 3.17, 3.72];
    let metal = ratios
        .iter()
        .map(|ratio| (voice.phase * TAU * ratio).sin() * 0.15)
        .sum::<f32>();
    (metal + filtered_noise(voice, 0) * 0.5) * (-voice.age * 25.0).exp()
}

fn kick_808(voice: &mut Voice) -> f32 {
    let end_freq = voice.freq.clamp(30.0, 80.0);
    let start_freq = 150.0_f32.max(end_freq * 4.0) + voice.freq * 0.5;
    let pitch_env = (-voice.age * 25.0).exp();
    let instant_freq = end_freq + (start_freq - end_freq) * pitch_env;
    let tone = (advance_aux_phase(voice, 0, instant_freq) * TAU).sin();
    let click = (-voice.age * 1_000.0).exp() * 0.5;
    let amp_env = (-voice.age * 2.0).exp();
    ((tone * amp_env + click) * 1.5).tanh()
}

fn snare_808(voice: &mut Voice) -> f32 {
    let t = voice.age;
    let f1 = (voice.freq * 2.0).max(300.0);
    let f2 = f1 * 1.5;
    let tone_env = (-t * 35.0).exp();
    let tone = ((advance_aux_phase(voice, 0, f1) * TAU).sin() * 0.6
        + (advance_aux_phase(voice, 1, f2) * TAU).sin() * 0.4)
        * tone_env;

    let noise = filtered_noise(voice, 0);
    let noise = normalize_voice_peak(voice, 5, noise);
    let snap_env = (-t * 15.0).exp();
    let crack = noise * snap_env;
    tone * 0.7 + crack * 0.9
}

fn hat_808(voice: &mut Voice) -> f32 {
    let base = voice.freq.max(300.0);
    let ratios = [263.0, 400.0, 421.0, 474.0, 587.0, 845.0];
    let mut metal = 0.0;
    for (idx, ratio) in ratios.into_iter().enumerate() {
        let freq = base * (ratio / 300.0);
        let phase = advance_aux_phase(voice, idx, freq);
        metal += if phase < 0.5 { 1.0 } else { -1.0 };
    }
    let filtered = filter_sample(voice, 0, metal);
    let filtered = filter_sample(voice, 1, filtered);
    let env = (-voice.age * 20.0).exp();
    normalize_voice_peak(voice, 4, filtered * env)
}

fn cowbell_808(voice: &mut Voice) -> f32 {
    let ratio = if voice.freq > 0.0 {
        voice.freq / 440.0
    } else {
        1.0
    };
    let f1 = 540.0 * ratio;
    let f2 = 800.0 * ratio;
    let p1 = advance_aux_phase(voice, 0, f1);
    let p2 = advance_aux_phase(voice, 1, f2);
    let osc1 = if p1 < 0.5 { 1.0 } else { -1.0 };
    let osc2 = if p2 < 0.5 { 1.0 } else { -1.0 };
    let raw = osc1 + osc2;
    let filtered = filter_sample(voice, 0, raw);
    let filtered = filter_sample(voice, 1, filtered);
    filtered * (-voice.age * 5.0).exp()
}

fn kick_909(voice: &mut Voice) -> f32 {
    let end_freq = voice.freq.clamp(40.0, 90.0);
    let start_freq = 200.0_f32.max(end_freq * 5.0) + voice.freq * 0.3;
    let f = end_freq + (start_freq - end_freq) * (-voice.age * 60.0).exp();
    let tone = ((advance_aux_phase(voice, 0, f) * TAU).sin() * 2.0).tanh();
    let click = signed_noise(&mut voice.noise) * (-voice.age * 200.0).exp() * 0.5;
    tone * (-voice.age * 3.5).exp() + click
}

fn snare_909(voice: &mut Voice) -> f32 {
    let f = voice.freq.max(180.0) + 50.0 * (-voice.age * 30.0).exp();
    let tone = (advance_aux_phase(voice, 0, f) * TAU).sin() * (-voice.age * 20.0).exp();
    let noise = filtered_noise(voice, 0) * (-voice.age * 12.0).exp();
    tone * 0.6 + noise * 0.8
}

fn hat_909(voice: &mut Voice) -> f32 {
    let carriers = [300.0, 420.0, 680.0, 800.0];
    let mut out = 0.0;
    for carrier in carriers {
        let p = voice.age * TAU * carrier;
        out += (p + (p * 1.414).sin() * 5.0).sin();
    }
    let out = filter_sample(voice, 0, out) * (-voice.age * 18.0).exp();
    normalize_voice_peak(voice, 6, out)
}

fn kick_78(voice: &mut Voice) -> f32 {
    let end_freq = voice.freq.clamp(40.0, 100.0);
    let f = end_freq * (1.0 + 3.0 * (-voice.age * 40.0).exp());
    let click = if voice.age < 0.001 { 0.05 } else { 0.0 };
    (advance_aux_phase(voice, 0, f) * TAU).sin() * (-voice.age * 12.0).exp() + click
}

fn snare_78(voice: &mut Voice) -> f32 {
    let tone = (voice.age * TAU * 230.0).sin()
        * (voice.age * TAU * 340.0).sin()
        * (-voice.age * 20.0).exp()
        * 0.6;
    let noise = filtered_noise(voice, 0) * (-voice.age * 15.0).exp() * 0.4;
    tone + noise
}

fn hat_78(voice: &mut Voice) -> f32 {
    let raw = [300.0, 350.0, 420.0, 600.0]
        .iter()
        .enumerate()
        .map(|(idx, freq)| {
            if advance_aux_phase(voice, idx, *freq) < 0.5 {
                1.0
            } else {
                -1.0
            }
        })
        .sum::<f32>();
    let filtered = filter_sample(voice, 0, raw);
    let filtered = filter_sample(voice, 1, filtered);
    filtered * (-voice.age * 30.0).exp()
}

fn kick_707(voice: &mut Voice) -> f32 {
    let end_freq = voice.freq.clamp(40.0, 100.0);
    let f = end_freq + end_freq * 1.5 * (-voice.age * 20.0).exp();
    let raw = ((advance_aux_phase(voice, 0, f) * TAU).sin() * 5.0).tanh();
    let tone = filter_sample(voice, 0, raw);
    tone * (-voice.age * 5.0).exp()
        + signed_noise(&mut voice.noise) * (-voice.age * 500.0).exp() * 0.2
}

fn snare_707(voice: &mut Voice) -> f32 {
    let tone = (voice.age * TAU * 200.0).sin() * (-voice.age * 25.0).exp() * 0.3;
    let noise = signed_noise(&mut voice.noise) * (-voice.age * 12.0).exp() * 0.7;
    tone + noise
}

fn clap(voice: &mut Voice) -> f32 {
    let bursts = [0.0, 0.012, 0.025, 0.038];
    let mut env: f32 = 0.0;
    for onset in bursts {
        if voice.age >= onset {
            env += (-(voice.age - onset) * 300.0).exp() * 0.8;
        }
    }
    if voice.age >= 0.03 {
        env += (-(voice.age - 0.03) * 10.0).exp() * 0.6;
    }
    filtered_noise(voice, 0) * env.clamp(0.0, 1.0)
}

fn cymbal_crash(voice: &mut Voice) -> f32 {
    let metal = [300.0, 380.0, 520.0, 890.0, 1_100.0, 1_400.0]
        .iter()
        .enumerate()
        .map(|(idx, freq)| {
            if advance_aux_phase(voice, idx, *freq) < 0.5 {
                1.0
            } else {
                -1.0
            }
        })
        .sum::<f32>()
        * pulse((voice.age * 3_500.0).fract(), 0.5);
    let metal = filter_sample(voice, 0, metal);
    let noise = signed_noise(&mut voice.noise);
    let noise = filter_sample(voice, 1, noise);
    (metal * 0.4 + noise * 0.6) * (-voice.age * 2.5).exp()
}

fn cymbal_ride(voice: &mut Voice) -> f32 {
    let p = voice.age * TAU * 2_500.0;
    let tone = (p + (voice.age * TAU * 3_200.0).sin() * 1_000.0 * (-voice.age * 10.0).exp()).sin();
    let noise = signed_noise(&mut voice.noise);
    let noise = filter_sample(voice, 0, noise);
    (tone * 0.5 + noise * 0.5) * (-voice.age * 8.0).exp()
}

fn tom(voice: &mut Voice) -> f32 {
    let start = voice.freq * 1.5;
    let end = voice.freq * 0.8;
    let f = end + (start - end) * (-voice.age * 15.0).exp();
    ((advance_aux_phase(voice, 0, f) * TAU).sin() * 2.0).tanh() * (-voice.age * 5.0).exp()
}

fn rimshot(voice: &mut Voice) -> f32 {
    let impulse = if voice.age < 10.0 / voice.sample_rate {
        1.0
    } else {
        0.0
    };
    let ring = filter_sample(voice, 0, impulse) * 50.0;
    let body = (voice.age * TAU * 400.0).sin() * (-voice.age * 80.0).exp() * 0.5;
    ring + body
}

fn shaker(voice: &mut Voice) -> f32 {
    let gate = if random01(&mut voice.noise) > 0.3 {
        1.0
    } else {
        0.0
    };
    filtered_noise(voice, 0) * (-voice.age * 25.0).exp() * gate
}

fn woodblock(voice: &Voice) -> f32 {
    let freq = if voice.freq > 400.0 {
        voice.freq
    } else {
        800.0
    };
    ((voice.age * TAU * freq).sin() * 0.7 + pulse((voice.age * freq).fract(), 0.5) * 0.3)
        * (-voice.age * 40.0).exp()
}

fn cowbell(voice: &mut Voice) -> f32 {
    let freq = if voice.freq > 300.0 {
        voice.freq
    } else {
        580.0
    };
    let raw = pulse(advance_aux_phase(voice, 0, freq), 0.5)
        + pulse(advance_aux_phase(voice, 1, freq * 1.45), 0.5);
    let filtered = filter_sample(voice, 0, raw);
    let filtered = filter_sample(voice, 1, filtered);
    filtered * (-voice.age * 10.0).exp()
}

fn zap(voice: &mut Voice) -> f32 {
    let f = (50.0 * 40.0_f32.powf(1.0 - voice.age * 10.0)).clamp(50.0, 2_000.0);
    (advance_aux_phase(voice, 0, f) * TAU).sin()
}

fn scratch(voice: &mut Voice) -> f32 {
    let speed = 1.0 + (voice.age * TAU * 5.0).sin() * 0.5 + (voice.age * TAU * 1.3).sin() * 0.5;
    let noise = signed_noise(&mut voice.noise);
    let band = filter_sample(voice, 0, noise);
    let band = filter_sample(voice, 1, band);
    band * speed.clamp(0.0, 1.0)
}

fn impact(voice: &mut Voice) -> f32 {
    let sweep = 60.0 * (-voice.age * 2.0).exp();
    let sub = (advance_aux_phase(voice, 0, sweep.max(20.0)) * TAU).sin();
    let out = (sub + signed_noise(&mut voice.noise) * (-voice.age * 5.0).exp() * 0.5).tanh();
    filter_sample(voice, 0, out)
}

fn bass_slap(voice: &mut Voice) -> f32 {
    let mod_env = (-voice.age * 20.0).exp();
    let fm = (voice.phase * TAU * 4.0).sin() * mod_env * 5.0;
    (voice.phase * TAU + fm).sin()
        + signed_noise(&mut voice.noise) * (-voice.age * 100.0).exp() * 0.25
}

fn piano_electric(voice: &Voice) -> f32 {
    let phase = voice.phase * TAU;
    let body = phase.sin() * (-voice.age * 2.0).exp();
    let tine = (phase * 14.0).sin() * (-voice.age * 15.0).exp() * 0.1;
    (phase + body + tine).sin()
}

fn drone_dark(voice: &mut Voice) -> f32 {
    let raw = (((voice.phase * 1.00).fract() * 2.0 - 1.0)
        + ((voice.phase * 1.01).fract() * 2.0 - 1.0)
        + ((voice.phase * 0.99).fract() * 2.0 - 1.0))
        / 3.0;
    filter_sample(voice, 0, raw)
}

fn advance_aux_phase(voice: &mut Voice, index: usize, freq: f32) -> f32 {
    let index = index.min(voice.aux_phase.len() - 1);
    voice.aux_phase[index] = (voice.aux_phase[index] + freq.max(0.0) / voice.sample_rate) % 1.0;
    voice.aux_phase[index]
}

fn filter_sample(voice: &mut Voice, index: usize, sample: f32) -> f32 {
    if let Some(filter) = voice.filters.get_mut(index) {
        filter.process(sample)
    } else {
        sample
    }
}

fn filtered_noise(voice: &mut Voice, first_filter: usize) -> f32 {
    let mut sample = signed_noise(&mut voice.noise);
    for index in first_filter..voice.filters.len() {
        sample = filter_sample(voice, index, sample);
    }
    sample
}

fn normalize_voice_peak(voice: &mut Voice, state_index: usize, sample: f32) -> f32 {
    let state_index = state_index.min(voice.color_state.len() - 1);
    voice.color_state[state_index] = (voice.color_state[state_index] * 0.9999)
        .max(sample.abs())
        .max(1e-6);
    (sample / voice.color_state[state_index]).clamp(-1.0, 1.0)
}

fn pink_noise(voice: &mut Voice) -> f32 {
    let white = signed_noise(&mut voice.noise);
    let x1 = voice.color_state[0];
    let x2 = voice.color_state[1];
    let x3 = voice.color_state[2];
    let y1 = voice.color_state[3];
    let y2 = voice.color_state[4];
    let y3 = voice.color_state[5];
    let out = 0.049_922_035 * white - 0.095_993_54 * x1 + 0.050_612_7 * x2 - 0.004_408_786 * x3
        + 2.494_956 * y1
        - 2.017_265_8 * y2
        + 0.522_189_4 * y3;
    voice.color_state[2] = x2;
    voice.color_state[1] = x1;
    voice.color_state[0] = white;
    voice.color_state[5] = y2;
    voice.color_state[4] = y1;
    voice.color_state[3] = out;
    normalize_voice_peak(voice, 7, out)
}

fn brown_noise(voice: &mut Voice) -> f32 {
    let white = signed_noise(&mut voice.noise);
    voice.color_state[0] += white;
    normalize_voice_peak(voice, 7, voice.color_state[0])
}

fn blue_noise(voice: &mut Voice) -> f32 {
    let white = signed_noise(&mut voice.noise);
    let out = white - voice.color_state[0];
    voice.color_state[0] = white;
    normalize_voice_peak(voice, 7, out)
}

fn purple_noise(voice: &mut Voice) -> f32 {
    let white = signed_noise(&mut voice.noise);
    let first_diff = white - voice.color_state[0];
    let out = first_diff - voice.color_state[1];
    voice.color_state[0] = white;
    voice.color_state[1] = first_diff;
    normalize_voice_peak(voice, 7, out)
}

fn random01(state: &mut u32) -> f32 {
    *state = state.wrapping_mul(1664525).wrapping_add(1013904223);
    (*state >> 8) as f32 / 16_777_216.0
}

fn signed_noise(state: &mut u32) -> f32 {
    *state = state.wrapping_mul(1664525).wrapping_add(1013904223);
    ((*state >> 8) as f32 / 16_777_216.0) * 2.0 - 1.0
}

fn soft_clip(x: f32) -> f32 {
    x / (1.0 + x.abs())
}

fn seed_from_step(id: &str, step: usize) -> u32 {
    let mut hash = 2166136261u32 ^ step as u32;
    for byte in id.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    hash
}

pub(crate) fn render(runtime: Runtime, seconds: f32, path: PathBuf) -> Result<RenderStats, String> {
    let sample_rate = 48_000.0;
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: sample_rate as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let (stereo_samples, stats) = render_to_buffer(runtime, seconds, sample_rate);

    let mut writer = hound::WavWriter::create(&path, spec).map_err(|error| error.to_string())?;
    for frame in &stereo_samples {
        let left = (frame[0].clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        let right = (frame[1].clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer
            .write_sample(left)
            .map_err(|error| error.to_string())?;
        writer
            .write_sample(right)
            .map_err(|error| error.to_string())?;
    }
    writer.finalize().map_err(|error| error.to_string())?;
    Ok(stats)
}

pub(crate) fn render_to_buffer(
    runtime: Runtime,
    seconds: f32,
    sample_rate: f32,
) -> (Vec<[f32; 2]>, RenderStats) {
    let post_effects = runtime.post_effects.clone();
    let mut engine = AudioEngine::new(runtime, sample_rate);
    let frames = (seconds * sample_rate) as usize;
    let mut raw_samples = Vec::with_capacity(frames);
    for _ in 0..frames {
        raw_samples.push(engine.next_frame());
    }
    let stereo_samples: Vec<[f32; 2]> = if post_effects.is_empty() {
        raw_samples
    } else {
        offline::apply_chain_stereo(raw_samples, &post_effects, sample_rate)
    };
    let mut peak = 0.0_f32;
    let mut sum_sq = 0.0_f64;
    for frame in &stereo_samples {
        peak = peak.max(frame[0].abs()).max(frame[1].abs());
        sum_sq += (frame[0] as f64) * (frame[0] as f64);
        sum_sq += (frame[1] as f64) * (frame[1] as f64);
    }
    let stats = RenderStats {
        peak,
        rms: (sum_sq / (stereo_samples.len().max(1) * 2) as f64).sqrt() as f32,
        frames: stereo_samples.len(),
    };
    (stereo_samples, stats)
}

pub(crate) struct RenderStats {
    pub(crate) peak: f32,
    pub(crate) rms: f32,
    pub(crate) frames: usize,
}

pub(crate) fn play(runtime: Runtime, seconds: f32) -> Result<(), String> {
    let stream = open_output_stream(Arc::new(Mutex::new(runtime)))?;
    stream.play().map_err(|error| error.to_string())?;
    thread::sleep(Duration::from_secs_f32(seconds.max(0.1)));
    Ok(())
}

pub(crate) fn open_output_stream(runtime: Arc<Mutex<Runtime>>) -> Result<cpal::Stream, String> {
    open_output_stream_with_steps(runtime, None)
}

pub(crate) fn open_output_stream_with_steps(
    runtime: Arc<Mutex<Runtime>>,
    step_sender: Option<Sender<usize>>,
) -> Result<cpal::Stream, String> {
    open_output_stream_named(runtime, step_sender, None)
}

pub(crate) fn output_device_names() -> Result<Vec<String>, String> {
    let host = cpal::default_host();
    host.output_devices()
        .map_err(|error| error.to_string())?
        .map(|device| device.name().map_err(|error| error.to_string()))
        .collect()
}

pub(crate) fn open_output_stream_named(
    runtime: Arc<Mutex<Runtime>>,
    step_sender: Option<Sender<usize>>,
    device_name: Option<&str>,
) -> Result<cpal::Stream, String> {
    open_output_stream_named_with_info(runtime, step_sender, device_name)
        .map(|(stream, _info)| stream)
}

pub(crate) fn open_output_stream_named_with_info(
    runtime: Arc<Mutex<Runtime>>,
    step_sender: Option<Sender<usize>>,
    device_name: Option<&str>,
) -> Result<(cpal::Stream, String), String> {
    let host = cpal::default_host();
    let device = if let Some(name) = device_name.filter(|name| !name.trim().is_empty()) {
        let mut devices = host.output_devices().map_err(|error| error.to_string())?;
        devices
            .find(|device| {
                device
                    .name()
                    .map(|device_name| device_name == name)
                    .unwrap_or(false)
            })
            .ok_or_else(|| format!("output device '{}' was not found", name))?
    } else {
        host.default_output_device()
            .ok_or("no default output audio device found")?
    };
    let config = device
        .default_output_config()
        .map_err(|error| error.to_string())?;
    let device_label = device
        .name()
        .unwrap_or_else(|_| "unknown output".to_string());
    let description = format!(
        "{} - {}ch {}Hz {:?}",
        device_label,
        config.channels(),
        config.sample_rate().0,
        config.sample_format()
    );
    let sample_rate = config.sample_rate().0 as f32;
    let channels = config.channels() as usize;
    let mut audio_engine = AudioEngine::new_shared(runtime, sample_rate);
    if let Some(sender) = step_sender {
        audio_engine.set_step_sender(sender);
    }
    let engine = Arc::new(Mutex::new(audio_engine));
    let err_fn = |error| eprintln!("audio stream error: {}", error);

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            build_stream::<f32>(&device, &config.into(), channels, engine, err_fn)
        }
        cpal::SampleFormat::I16 => {
            build_stream::<i16>(&device, &config.into(), channels, engine, err_fn)
        }
        cpal::SampleFormat::U16 => {
            build_stream::<u16>(&device, &config.into(), channels, engine, err_fn)
        }
        other => return Err(format!("unsupported output sample format {:?}", other)),
    }?;
    Ok((stream, description))
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    engine: Arc<Mutex<AudioEngine>>,
    err_fn: fn(cpal::StreamError),
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                let mut engine = engine.lock().expect("audio engine lock poisoned");
                for frame in data.chunks_mut(channels) {
                    let stereo = engine.next_frame();
                    for (idx, channel) in frame.iter_mut().enumerate() {
                        let sample = match idx {
                            0 => stereo[0],
                            1 => stereo[1],
                            _ => (stereo[0] + stereo[1]) * 0.5,
                        };
                        *channel = T::from_sample(sample);
                    }
                }
            },
            err_fn,
            None,
        )
        .map_err(|error| error.to_string())
}
