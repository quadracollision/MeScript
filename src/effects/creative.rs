use super::advance_lfo;
use super::filters::{Biquad, FilterKind};

#[derive(Clone, Debug)]
pub struct Wavefolder {
    folds: usize,
    gain: f32,
    symmetry: f32,
    peak: f32,
}

#[derive(Clone, Debug)]
pub struct Resonator {
    bands: Vec<ResonatorBand>,
    mix: f32,
    peak: f32,
}

impl Resonator {
    pub fn new(freq: f32, decay: f32, mix: f32, harmonics: f32, sample_rate: f32) -> Self {
        let freq = freq.clamp(20.0, sample_rate * 0.45);
        let decay = decay.clamp(0.0, 1.0);
        let q = 10.0 + decay * 40.0;
        let harmonics = harmonics.clamp(1.0, 16.0).floor() as usize;
        let bands = (1..=harmonics)
            .filter_map(|harmonic| {
                let partial = freq * harmonic as f32;
                (partial < sample_rate * 0.45)
                    .then(|| ResonatorBand::new(partial, q, sample_rate, 1.0 / harmonic as f32))
            })
            .collect();
        Self {
            bands,
            mix: mix.clamp(0.0, 1.0),
            peak: 1e-9,
        }
    }

    #[cfg(test)]
    pub(crate) fn band_count_for_test(&self) -> usize {
        self.bands.len()
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let wet = self
            .bands
            .iter_mut()
            .map(|band| band.process(sample))
            .sum::<f32>();
        self.peak = self.peak.max(wet.abs());
        let wet = (wet / self.peak).clamp(-1.0, 1.0);
        (1.0 - self.mix) * sample + self.mix * wet
    }
}

#[derive(Clone, Debug)]
struct ResonatorBand {
    b0: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
    gain: f32,
}

impl ResonatorBand {
    fn new(freq: f32, q: f32, sample_rate: f32, gain: f32) -> Self {
        let w0 = std::f32::consts::TAU * freq / sample_rate;
        let alpha = w0.sin() / (2.0 * q.max(0.001));
        let cos_w0 = w0.cos();
        let a0 = 1.0 + alpha;
        Self {
            b0: alpha / a0,
            b2: -alpha / a0,
            a1: -2.0 * cos_w0 / a0,
            a2: (1.0 - alpha) / a0,
            z1: 0.0,
            z2: 0.0,
            gain,
        }
    }

    fn process(&mut self, sample: f32) -> f32 {
        let out = self.b0 * sample + self.z1;
        self.z1 = self.z2 - self.a1 * out;
        self.z2 = self.b2 * sample - self.a2 * out;
        out * self.gain
    }
}

impl Wavefolder {
    pub fn new(folds: f32, gain: f32, symmetry: f32) -> Self {
        Self {
            folds: folds.clamp(1.0, 8.0).floor() as usize,
            gain: gain.clamp(0.1, 12.0),
            symmetry: symmetry.clamp(0.1, 2.0),
            peak: 1e-9,
        }
    }

    #[cfg(test)]
    pub(crate) fn fold_count_for_test(&self) -> usize {
        self.folds
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let mut x = sample * self.gain;
        for _ in 0..self.folds {
            x = (x * std::f32::consts::FRAC_PI_2)
                .sin()
                .clamp(-1.0, 1.0)
                .asin()
                * 2.0
                / std::f32::consts::PI;
            x *= self.symmetry;
        }
        self.peak = self.peak.max(x.abs());
        (x / self.peak).clamp(-1.0, 1.0)
    }
}

#[derive(Clone, Debug)]
pub struct Lofi {
    amount: f32,
    levels: f32,
    hold: usize,
    counter: usize,
    held: f32,
    noise: u32,
    lowpasses: Vec<Biquad>,
}

impl Lofi {
    pub fn new(amount: f32, sample_rate: f32) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        let bits = (16.0 - amount * 12.0).floor().max(3.0);
        let cutoff = (15_000.0 - amount * 12_000.0).max(500.0);
        Self {
            amount,
            levels: 2.0_f32.powf(bits),
            hold: (1.0 + amount * 8.0).floor().max(1.0) as usize,
            counter: 0,
            held: 0.0,
            noise: 0x1234_abcd,
            lowpasses: vec![
                Biquad::lowpass_with_q(cutoff, 0.541_196_1, sample_rate),
                Biquad::lowpass_with_q(cutoff, 1.306_563, sample_rate),
            ],
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        if self.counter == 0 {
            self.held = sample;
        }
        self.counter = (self.counter + 1) % self.hold.max(1);
        let crushed = (self.held * self.levels).round() / self.levels;
        let degraded = crushed + signed_noise(&mut self.noise) * self.amount * 0.03;
        self.lowpasses
            .iter_mut()
            .fold(degraded, |sample, filter| filter.process(sample))
    }

    #[cfg(test)]
    pub(crate) fn python_integer_settings(&self) -> (usize, f32) {
        (self.hold, self.levels)
    }
}

#[derive(Clone, Debug)]
pub struct Vinyl {
    crackle: f32,
    hiss: f32,
    wow: f32,
    wow_phase: f32,
    delay: VinylDelay,
    hiss_first: FirstOrderLowpass,
    hiss_filter: Biquad,
    noise: u32,
}

impl Vinyl {
    pub fn new(crackle: f32, hiss: f32, wow: f32, sample_rate: f32) -> Self {
        Self {
            crackle: crackle.clamp(0.0, 1.0),
            hiss: hiss.clamp(0.0, 1.0),
            wow: wow.clamp(0.0, 1.0),
            wow_phase: 0.0,
            delay: VinylDelay::new(0.006, sample_rate),
            hiss_first: FirstOrderLowpass::new(8_000.0, sample_rate),
            hiss_filter: Biquad::lowpass_with_q(8_000.0, 1.0, sample_rate),
            noise: 0x51f1_d15c,
        }
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let mut out = sample;
        if self.crackle > 0.0 && random01(&mut self.noise) < self.crackle * 0.001 {
            out += signed_noise(&mut self.noise) * 0.15;
        }
        if self.hiss > 0.0 {
            let noise = signed_noise(&mut self.noise) * self.hiss * 0.05;
            out += self.process_hiss(noise);
        }

        let wow_delay = (self.wow_phase * std::f32::consts::TAU).sin() * self.wow * 0.001;
        self.wow_phase = (self.wow_phase + 0.5 / sample_rate) % 1.0;
        self.delay.process(out, wow_delay.max(0.0) * sample_rate)
    }

    #[cfg(test)]
    pub(crate) fn set_wow_phase_for_test(&mut self, phase: f32) {
        self.wow_phase = phase.rem_euclid(1.0);
    }

    fn process_hiss(&mut self, sample: f32) -> f32 {
        let first = self.hiss_first.process(sample);
        self.hiss_filter.process(first)
    }

    #[cfg(test)]
    pub(crate) fn process_hiss_probe(&mut self, sample: f32) -> f32 {
        self.process_hiss(sample)
    }
}

#[derive(Clone, Debug)]
pub struct SubBass {
    mix: f32,
    lowpasses: Vec<Biquad>,
    dc: f32,
}

impl SubBass {
    pub fn new(mix: f32, sample_rate: f32) -> Self {
        Self {
            mix: mix.clamp(0.0, 1.0),
            lowpasses: butterworth_sixth_order_qs()
                .iter()
                .map(|q| Biquad::lowpass_with_q(150.0, *q, sample_rate))
                .collect(),
            dc: 0.0,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let rectified = sample.abs();
        let sub = self
            .lowpasses
            .iter_mut()
            .fold(rectified, |sample, filter| filter.process(sample));
        self.dc = self.dc * 0.999 + sub * 0.001;
        sample + (sub - self.dc) * self.mix
    }

    #[cfg(test)]
    pub(crate) fn process_sub_probe(&mut self, sample: f32) -> f32 {
        self.lowpasses
            .iter_mut()
            .fold(sample.abs(), |sample, filter| filter.process(sample))
    }
}

#[derive(Clone, Debug)]
pub struct Sidechain {
    phase: f32,
    rate: f32,
    depth: f32,
    shape: f32,
}

impl Sidechain {
    pub fn new(rate: f32, depth: f32, shape: f32) -> Self {
        Self {
            phase: 0.0,
            rate: rate.clamp(0.01, 40.0),
            depth: depth.clamp(0.0, 1.0),
            shape: shape.clamp(0.0, 1.0),
        }
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let release = if self.shape > 0.5 {
            self.phase.powf(1.0 + self.shape * 3.0)
        } else {
            self.phase.powf(0.5 + self.shape)
        };
        self.phase = (self.phase + self.rate / sample_rate) % 1.0;
        sample * (1.0 - self.depth * (1.0 - release))
    }
}

#[derive(Clone, Debug)]
pub struct Radio {
    intensity: f32,
    highpasses: Vec<Biquad>,
    lowpasses: Vec<Biquad>,
    hum_phase: f32,
    noise: u32,
}

impl Radio {
    pub fn new(intensity: f32, sample_rate: f32) -> Self {
        let intensity = intensity.clamp(0.0, 1.0);
        let low = 300.0 + (1.0 - intensity) * 200.0;
        let high = 3_000.0 + (1.0 - intensity) * 2_000.0;
        Self {
            intensity,
            highpasses: butterworth_fourth_order_qs()
                .iter()
                .map(|q| Biquad::highpass_with_q(low, *q, sample_rate))
                .collect(),
            lowpasses: butterworth_fourth_order_qs()
                .iter()
                .map(|q| Biquad::lowpass_with_q(high, *q, sample_rate))
                .collect(),
            hum_phase: 0.0,
            noise: 0x4567_cdef,
        }
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let band = self
            .highpasses
            .iter_mut()
            .fold(sample, |sample, filter| filter.process(sample));
        let band = self
            .lowpasses
            .iter_mut()
            .fold(band, |sample, filter| filter.process(sample));
        let mut out = (band * (1.0 + self.intensity * 5.0)).tanh();
        let hum = 1.0
            - self.intensity * 0.15 * (1.0 + advance_lfo(&mut self.hum_phase, 60.0, sample_rate));
        out *= hum;
        out + signed_noise(&mut self.noise) * self.intensity * 0.02
    }
}

#[derive(Clone, Debug)]
pub struct Telephone {
    highpasses: Vec<Biquad>,
    lowpasses: Vec<Biquad>,
    levels: f32,
}

impl Telephone {
    pub fn new(quality: f32, sample_rate: f32) -> Self {
        let bits = (8.0 - quality.clamp(0.0, 1.0) * 4.0).floor().max(4.0);
        Self {
            highpasses: butterworth_sixth_order_qs()
                .iter()
                .map(|q| Biquad::highpass_with_q(300.0, *q, sample_rate))
                .collect(),
            lowpasses: butterworth_sixth_order_qs()
                .iter()
                .map(|q| Biquad::lowpass_with_q(3_400.0, *q, sample_rate))
                .collect(),
            levels: 2.0_f32.powf(bits),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let band = self
            .highpasses
            .iter_mut()
            .fold(sample, |sample, filter| filter.process(sample));
        let band = self
            .lowpasses
            .iter_mut()
            .fold(band, |sample, filter| filter.process(sample));
        let saturated = (band * 2.0).tanh() * 0.7;
        (saturated * self.levels).round() / self.levels
    }

    #[cfg(test)]
    pub(crate) fn python_levels(&self) -> f32 {
        self.levels
    }
}

#[derive(Clone, Debug)]
pub struct Underwater {
    lowpasses: Vec<Biquad>,
    resonance_lowpass: Biquad,
    delay: VinylDelay,
    phase: f32,
    depth: f32,
}

impl Underwater {
    pub fn new(depth: f32, sample_rate: f32) -> Self {
        let depth = depth.clamp(0.0, 1.0);
        let cutoff = (2_000.0 - depth * 1_800.0).max(200.0);
        Self {
            lowpasses: butterworth_sixth_order_qs()
                .iter()
                .map(|q| Biquad::lowpass_with_q(cutoff, *q, sample_rate))
                .collect(),
            resonance_lowpass: Biquad::lowpass_with_q(
                (cutoff * 0.8).max(100.0),
                std::f32::consts::FRAC_1_SQRT_2,
                sample_rate,
            ),
            delay: VinylDelay::new(0.007, sample_rate),
            phase: 0.0,
            depth,
        }
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let filtered = self
            .lowpasses
            .iter_mut()
            .fold(sample, |sample, filter| filter.process(sample));
        let lfo = advance_lfo(&mut self.phase, 0.3, sample_rate);
        let delay_samples = (lfo * self.depth * 0.003 * sample_rate).max(0.0);
        let modulated = self.delay.process(filtered, delay_samples);
        self.resonance_lowpass.process(modulated)
    }
}

#[derive(Clone, Debug)]
pub struct Crystal {
    brightness: f32,
    highpass: Biquad,
    buffer: Vec<f32>,
    pos: usize,
    feedback: f32,
    peak: f32,
}

impl Crystal {
    pub fn new(brightness: f32, decay: f32, sample_rate: f32) -> Self {
        let brightness = brightness.clamp(0.0, 1.0);
        let freq = 3_000.0 + brightness * 5_000.0;
        Self {
            brightness,
            highpass: Biquad::new(
                FilterKind::Highpass,
                5_000.0 + brightness * 5_000.0,
                0.4,
                sample_rate,
            ),
            buffer: vec![0.0; (sample_rate / freq).floor().max(1.0) as usize],
            pos: 0,
            feedback: decay.clamp(0.0, 0.95),
            peak: 1e-9,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let highs = self.highpass.process(sample);
        let delayed = self.buffer[self.pos];
        let combed = highs + delayed * self.feedback;
        self.buffer[self.pos] = combed;
        self.pos = (self.pos + 1) % self.buffer.len();
        self.peak = self.peak.max(combed.abs());
        let sparkle = (combed / self.peak).clamp(-1.0, 1.0);
        sample + sparkle * self.brightness * 0.3
    }

    #[cfg(test)]
    pub(crate) fn comb_delay_samples_for_test(&self) -> usize {
        self.buffer.len()
    }
}

#[derive(Clone, Debug)]
pub struct DcRemove {
    highpass: Biquad,
}

impl DcRemove {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            highpass: Biquad::highpass_with_q(10.0, std::f32::consts::FRAC_1_SQRT_2, sample_rate),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        self.highpass.process(sample)
    }
}

#[derive(Clone, Debug)]
pub struct PitchShift {
    shifter: PitchTap,
    mix: f32,
}

impl PitchShift {
    pub fn new(semitones: f32, mix: f32, sample_rate: f32) -> Self {
        Self {
            shifter: PitchTap::new(2.0_f32.powf(semitones / 12.0), 0.08, sample_rate),
            mix: mix.clamp(0.0, 1.0),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let wet = self.shifter.process(sample);
        (1.0 - self.mix) * sample + self.mix * wet
    }
}

#[derive(Clone, Debug)]
pub struct Harmonizer {
    shifter: PitchTap,
    mix: f32,
}

impl Harmonizer {
    pub fn new(interval: f32, mix: f32, sample_rate: f32) -> Self {
        Self {
            shifter: PitchTap::new(2.0_f32.powf(interval / 12.0), 0.08, sample_rate),
            mix: mix.clamp(0.0, 1.0),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        sample + self.shifter.process(sample) * self.mix
    }
}

#[derive(Clone, Debug)]
pub struct Octaver {
    octave_up: f32,
    octave_down: f32,
    up: PitchTap,
    down: PitchTap,
}

impl Octaver {
    pub fn new(octave_up: f32, octave_down: f32, sample_rate: f32) -> Self {
        Self {
            octave_up: octave_up.clamp(0.0, 1.0),
            octave_down: octave_down.clamp(0.0, 1.0),
            up: PitchTap::new(2.0, 0.08, sample_rate),
            down: PitchTap::new(0.5, 0.08, sample_rate),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        sample
            + self.up.process(sample) * self.octave_up
            + self.down.process(sample) * self.octave_down
    }

    #[cfg(test)]
    pub(crate) fn pitch_read_positions(&self) -> (f32, f32) {
        (self.up.read_position(), self.down.read_position())
    }
}

#[derive(Clone, Debug)]
pub struct Shimmer {
    shifter: PitchTap,
    buffer: Vec<f32>,
    pos: usize,
    filled: usize,
    feedback: f32,
    mix: f32,
    peak: f32,
}

impl Shimmer {
    pub fn new(shift_semitones: f32, feedback: f32, mix: f32, sample_rate: f32) -> Self {
        Self {
            shifter: PitchTap::new(2.0_f32.powf(shift_semitones / 12.0), 0.09, sample_rate),
            buffer: vec![0.0; (0.08 * sample_rate).round() as usize],
            pos: 0,
            filled: 0,
            feedback: feedback.clamp(0.0, 0.95),
            mix: mix.clamp(0.0, 1.0),
            peak: 1.0e-6,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let shifted = self.shifter.process(sample);
        let wet = if self.filled < self.buffer.len() {
            self.filled += 1;
            0.0
        } else {
            shifted + self.buffer[self.pos] * self.feedback
        };
        self.buffer[self.pos] = wet;
        self.pos = (self.pos + 1) % self.buffer.len();
        self.peak = self.peak.max(wet.abs());
        (1.0 - self.mix) * sample + self.mix * (wet / self.peak)
    }
}

#[derive(Clone, Debug)]
pub struct Stutter {
    buffer: Vec<f32>,
    pos: usize,
    grain_samples: usize,
    repeats: usize,
    repeat_pos: usize,
    repeat_count: usize,
    mix: f32,
}

impl Stutter {
    pub fn new(grain_size_ms: f32, repeats: f32, mix: f32, sample_rate: f32) -> Self {
        let grain_samples =
            (grain_size_ms.clamp(1.0, 500.0) * sample_rate / 1_000.0).floor() as usize;
        Self {
            buffer: vec![0.0; grain_samples.max(1)],
            pos: 0,
            grain_samples: grain_samples.max(1),
            repeats: repeats.clamp(1.0, 16.0).floor() as usize,
            repeat_pos: 0,
            repeat_count: 0,
            mix: mix.clamp(0.0, 1.0),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let wet = if self.repeat_count > 0 {
            let out = self.buffer[self.repeat_pos] * 0.8_f32.powi(self.repeat_count as i32);
            self.repeat_pos += 1;
            if self.repeat_pos >= self.grain_samples {
                self.repeat_pos = 0;
                self.repeat_count -= 1;
            }
            out
        } else {
            self.buffer[self.pos] = sample;
            self.pos += 1;
            if self.pos >= self.grain_samples {
                self.pos = 0;
                self.repeat_count = self.repeats;
                self.repeat_pos = 0;
            }
            sample
        };
        (1.0 - self.mix) * sample + self.mix * wet
    }

    #[cfg(test)]
    pub(crate) fn python_integer_settings(&self) -> (usize, usize) {
        (self.grain_samples, self.repeats)
    }
}

#[derive(Clone, Debug)]
pub struct Glitch {
    density: f32,
    slice_samples: usize,
    pos: usize,
    input: Vec<f32>,
    output: Vec<f32>,
    history: Vec<Vec<f32>>,
    history_pos: usize,
    mode: u32,
    noise: u32,
    forced_mode: Option<u32>,
}

impl Glitch {
    pub fn new(density: f32, slice_ms: f32, sample_rate: f32) -> Self {
        let slice_samples = (slice_ms.clamp(1.0, 500.0) * sample_rate / 1_000.0)
            .floor()
            .max(1.0) as usize;
        Self {
            density: density.clamp(0.0, 1.0),
            slice_samples,
            pos: 0,
            input: vec![0.0; slice_samples],
            output: vec![0.0; slice_samples],
            history: vec![vec![0.0; slice_samples]; 8],
            history_pos: 0,
            mode: 0,
            noise: 0x9e37_79b9,
            forced_mode: None,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        if self.density <= 0.0 {
            return sample;
        }

        let out = if self.mode == 0 {
            sample
        } else {
            self.output[self.pos]
        };
        self.input[self.pos] = sample;
        self.pos += 1;
        if self.pos >= self.slice_samples {
            self.finish_slice();
            self.pos = 0;
        }
        out
    }

    fn finish_slice(&mut self) {
        self.history[self.history_pos].copy_from_slice(&self.input);
        self.history_pos = (self.history_pos + 1) % self.history.len();

        let active = random01(&mut self.noise) < self.density;
        self.mode = if active {
            self.forced_mode
                .unwrap_or_else(|| (random01(&mut self.noise) * 4.0) as u32 + 1)
        } else {
            0
        };

        match self.mode {
            1 => {
                for (dst, src) in self.output.iter_mut().zip(self.input.iter().rev()) {
                    *dst = *src;
                }
            }
            2 => {
                let half = self.slice_samples / 2;
                if half == 0 {
                    self.output.copy_from_slice(&self.input);
                } else {
                    self.output[..half].copy_from_slice(&self.input[..half]);
                    for i in half..self.slice_samples {
                        self.output[i] = self.input[i - half];
                    }
                }
            }
            3 => self.output.fill(0.0),
            4 => {
                let idx = (random01(&mut self.noise) * self.history.len() as f32) as usize
                    % self.history.len();
                self.output.copy_from_slice(&self.history[idx]);
            }
            _ => self.output.copy_from_slice(&self.input),
        }
    }

    #[cfg(test)]
    pub(crate) fn force_mode_for_test(&mut self, mode: u32) {
        self.forced_mode = Some(mode);
    }

    #[cfg(test)]
    pub(crate) fn slice_samples_for_test(&self) -> usize {
        self.slice_samples
    }
}

#[derive(Clone, Debug)]
pub struct Fade {
    age: usize,
    fade_in: usize,
    fade_out: usize,
    total: usize,
}

impl Fade {
    pub fn new(fade_in_ms: f32, fade_out_ms: f32, duration_seconds: f32, sample_rate: f32) -> Self {
        Self {
            age: 0,
            fade_in: (fade_in_ms.max(0.0) * sample_rate / 1_000.0).round() as usize,
            fade_out: (fade_out_ms.max(0.0) * sample_rate / 1_000.0).round() as usize,
            total: (duration_seconds.max(0.001) * sample_rate).round() as usize,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let mut gain: f32 = 1.0;
        if self.fade_in > 0 && self.age < self.fade_in {
            let fade_gain = if self.fade_in == 1 {
                0.0
            } else {
                self.age as f32 / (self.fade_in - 1) as f32
            };
            gain = gain.min(fade_gain);
        }
        if self.fade_out > 0 && self.age + self.fade_out >= self.total {
            let fade_pos = self
                .age
                .saturating_sub(self.total.saturating_sub(self.fade_out));
            let fade_gain = if self.fade_out == 1 {
                1.0
            } else {
                1.0 - fade_pos as f32 / (self.fade_out - 1) as f32
            };
            gain = gain.min(fade_gain);
        }
        self.age = self.age.saturating_add(1);
        sample * gain.clamp(0.0, 1.0)
    }
}

#[derive(Clone, Debug)]
pub struct Adsr {
    age: usize,
    attack: usize,
    decay: usize,
    sustain: f32,
    release: usize,
    note_off: usize,
}

impl Adsr {
    pub fn new(
        attack: f32,
        decay: f32,
        sustain: f32,
        release: f32,
        duration_seconds: f32,
        sample_rate: f32,
    ) -> Self {
        Self {
            age: 0,
            attack: seconds_to_samples(attack, sample_rate),
            decay: seconds_to_samples(decay, sample_rate),
            sustain: sustain.clamp(0.0, 1.0),
            release: seconds_to_samples(release, sample_rate),
            note_off: seconds_to_samples(duration_seconds.max(0.001), sample_rate),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let gain = self.gain();
        self.age = self.age.saturating_add(1);
        sample * gain
    }

    fn gain(&self) -> f32 {
        if self.attack > 0 && self.age < self.attack {
            return self.age as f32 / self.attack as f32;
        }

        let decay_start = self.attack;
        let decay_end = decay_start.saturating_add(self.decay);
        if self.decay > 0 && self.age < decay_end {
            let pos = (self.age - decay_start) as f32 / self.decay as f32;
            return 1.0 + (self.sustain - 1.0) * pos;
        }

        if self.age < self.note_off {
            return self.sustain;
        }

        if self.release == 0 {
            return 0.0;
        }

        let release_pos = self.age.saturating_sub(self.note_off);
        if release_pos >= self.release {
            0.0
        } else {
            self.sustain * (1.0 - release_pos as f32 / self.release as f32)
        }
    }
}

fn seconds_to_samples(seconds: f32, sample_rate: f32) -> usize {
    (seconds.max(0.0) * sample_rate).round() as usize
}

#[derive(Clone, Debug)]
pub struct Doppler {
    delay: PitchTap,
    phase: f32,
    speed: f32,
    depth: f32,
}

impl Doppler {
    pub fn new(speed: f32, depth: f32, sample_rate: f32) -> Self {
        Self {
            delay: PitchTap::new(1.0, 0.12, sample_rate),
            phase: 0.0,
            speed: speed.clamp(0.01, 8.0),
            depth: depth.clamp(0.0, 1.0),
        }
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let t = self.phase * 2.0 - 1.0;
        self.delay.ratio = (1.0 - self.depth * t * self.speed).clamp(0.5, 2.0);
        self.phase = (self.phase + self.speed / sample_rate) % 1.0;
        self.delay.process(sample)
    }

    #[cfg(test)]
    pub(crate) fn read_position(&self) -> f32 {
        self.delay.read_position()
    }
}

#[derive(Clone, Debug)]
pub struct Maximizer {
    ceiling: f32,
    warmth: f32,
    alpha: f32,
    env: f32,
}

#[derive(Clone, Debug)]
pub struct ParallelComp {
    compressor: SimpleBandComp,
    mix: f32,
    dry_peak: f32,
    crushed_peak: f32,
}

impl ParallelComp {
    pub fn new(threshold: f32, ratio: f32, mix: f32, sample_rate: f32) -> Self {
        Self {
            compressor: SimpleBandComp::new(threshold, ratio, 0.001, 0.05, sample_rate),
            mix: mix.clamp(0.0, 1.0),
            dry_peak: 1.0e-9,
            crushed_peak: 1.0e-9,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let crushed_raw = self.compressor.process(sample);
        self.dry_peak = self.dry_peak.max(sample.abs());
        self.crushed_peak = self.crushed_peak.max(crushed_raw.abs());
        let crushed = crushed_raw * (self.dry_peak / self.crushed_peak) * 0.9;
        (1.0 - self.mix) * sample + self.mix * crushed
    }
}

impl Maximizer {
    pub fn new(ceiling: f32, warmth: f32, release_ms: f32, sample_rate: f32) -> Self {
        Self {
            ceiling: db_to_gain(ceiling),
            warmth: warmth.clamp(0.0, 1.0),
            alpha: (-1.0 / ((release_ms.max(1.0) / 1_000.0) * sample_rate).max(1.0)).exp(),
            env: 0.0,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        self.env = sample.abs().max(self.alpha * self.env);
        let gain = if self.env > self.ceiling {
            self.ceiling / (self.env + 1e-9)
        } else {
            1.0
        };
        let out = sample * gain;
        if self.warmth > 0.0 {
            (out * (1.0 + self.warmth)).tanh() / (1.0 + self.warmth * 0.5)
        } else {
            out
        }
    }
}

#[derive(Clone, Debug)]
pub struct MultibandComp {
    low: Vec<Biquad>,
    mid_low: Vec<Biquad>,
    mid_high: Vec<Biquad>,
    high: Vec<Biquad>,
    low_comp: SimpleBandComp,
    mid_comp: SimpleBandComp,
    high_comp: SimpleBandComp,
}

impl MultibandComp {
    pub fn new(
        low_thresh: f32,
        mid_thresh: f32,
        high_thresh: f32,
        crossover_low: f32,
        crossover_high: f32,
        sample_rate: f32,
    ) -> Self {
        let butterworth_q = 0.707_f32;
        let resonance = ((butterworth_q - 0.5) / 11.5).clamp(0.0, 1.0);
        Self {
            low: (0..2)
                .map(|_| Biquad::new(FilterKind::Lowpass, crossover_low, resonance, sample_rate))
                .collect(),
            mid_low: (0..2)
                .map(|_| Biquad::new(FilterKind::Highpass, crossover_low, resonance, sample_rate))
                .collect(),
            mid_high: (0..2)
                .map(|_| Biquad::new(FilterKind::Lowpass, crossover_high, resonance, sample_rate))
                .collect(),
            high: (0..2)
                .map(|_| Biquad::new(FilterKind::Highpass, crossover_high, resonance, sample_rate))
                .collect(),
            low_comp: SimpleBandComp::new(low_thresh, 3.0, 0.01, 0.15, sample_rate),
            mid_comp: SimpleBandComp::new(mid_thresh, 4.0, 0.005, 0.1, sample_rate),
            high_comp: SimpleBandComp::new(high_thresh, 3.5, 0.002, 0.08, sample_rate),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let (low, mid, high) = self.split_bands(sample);
        self.low_comp.process(low) + self.mid_comp.process(mid) + self.high_comp.process(high)
    }

    fn split_bands(&mut self, sample: f32) -> (f32, f32, f32) {
        let low = process_filter_chain(&mut self.low, sample);
        let mid = process_filter_chain(
            &mut self.mid_high,
            process_filter_chain(&mut self.mid_low, sample),
        );
        let high = process_filter_chain(&mut self.high, sample);
        (low, mid, high)
    }

    #[cfg(test)]
    pub(crate) fn band_probe(&mut self, sample: f32) -> (f32, f32, f32) {
        self.split_bands(sample)
    }
}

#[derive(Clone, Debug)]
pub struct HarmonicEnhance {
    low_harmonics: f32,
    high_harmonics: f32,
    air: f32,
    low: Biquad,
    high: Biquad,
    air_filter: Biquad,
}

impl HarmonicEnhance {
    pub fn new(low_harmonics: f32, high_harmonics: f32, air: f32, sample_rate: f32) -> Self {
        Self {
            low_harmonics: low_harmonics.clamp(0.0, 1.0),
            high_harmonics: high_harmonics.clamp(0.0, 1.0),
            air: air.clamp(0.0, 1.0),
            low: Biquad::new(FilterKind::Lowpass, 500.0, 0.2, sample_rate),
            high: Biquad::new(FilterKind::Highpass, 3_000.0, 0.2, sample_rate),
            air_filter: Biquad::new(
                FilterKind::Highpass,
                sample_rate.min(10_000.0),
                0.2,
                sample_rate,
            ),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let lows = self.low.process(sample);
        let h2 = lows * lows * lows.signum() * 0.5;
        let highs = self.high.process(sample);
        let h_sat = (highs * 3.0).tanh() * 0.3;
        let air = self.air_filter.process(sample);
        sample + h2 * self.low_harmonics + h_sat * self.high_harmonics + air * self.air * 0.5
    }
}

#[derive(Clone, Debug)]
pub struct Body {
    modes: Vec<BodyMode>,
    mix: f32,
    peak: f32,
}

impl Body {
    pub fn new(size: f32, tone: f32, mix: f32, sample_rate: f32) -> Self {
        let size = size.clamp(0.0, 1.0);
        let tone = tone.clamp(0.0, 1.0);
        let freqs = [120.0, 250.0, 500.0, 1_200.0];
        Self {
            modes: freqs
                .into_iter()
                .enumerate()
                .map(|(idx, freq)| {
                    let amp = (1.0 - idx as f32 * 0.2)
                        * if freq > 300.0 {
                            0.5 + tone * 0.5
                        } else {
                            1.0 - tone * 0.3
                        };
                    BodyMode::new(freq, 0.02 + size * 0.03, amp, sample_rate)
                })
                .collect(),
            mix: mix.clamp(0.0, 1.0),
            peak: 1e-9,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let wet = self
            .modes
            .iter_mut()
            .map(|mode| mode.process(sample))
            .sum::<f32>();
        self.peak = self.peak.max(wet.abs());
        let wet = (wet / self.peak).clamp(-1.0, 1.0);
        (1.0 - self.mix) * sample + self.mix * wet
    }
}

#[derive(Clone, Debug)]
pub struct Warmth {
    amount: f32,
    low: FirstOrderLowpass,
    rolloff: FirstOrderLowpass,
}

impl Warmth {
    pub fn new(amount: f32, sample_rate: f32) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        Self {
            amount,
            low: FirstOrderLowpass::new(150.0, sample_rate),
            rolloff: FirstOrderLowpass::new(
                (16_000.0 - amount * 6_000.0).max(8_000.0),
                sample_rate,
            ),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let even = sample + self.amount * 0.03 * sample * sample;
        let saturated = (even * (1.0 + self.amount * 0.5)).tanh() / (1.0 + self.amount * 0.3);
        let low = self.low.process(sample);
        self.rolloff.process(saturated + low * self.amount * 0.1)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FirstOrderLowpass {
    b0: f32,
    b1: f32,
    a1: f32,
    x1: f32,
    y1: f32,
}

impl FirstOrderLowpass {
    pub(crate) fn new(cutoff: f32, sample_rate: f32) -> Self {
        let cutoff = cutoff.clamp(20.0, sample_rate * 0.45);
        let k = (std::f32::consts::PI * cutoff / sample_rate).tan();
        let norm = 1.0 / (1.0 + k);
        Self {
            b0: k * norm,
            b1: k * norm,
            a1: (k - 1.0) * norm,
            x1: 0.0,
            y1: 0.0,
        }
    }

    pub(crate) fn process(&mut self, sample: f32) -> f32 {
        let out = self.b0 * sample + self.b1 * self.x1 - self.a1 * self.y1;
        self.x1 = sample;
        self.y1 = out;
        out
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FirstOrderHighpass {
    b0: f32,
    b1: f32,
    a1: f32,
    x1: f32,
    y1: f32,
}

impl FirstOrderHighpass {
    pub(crate) fn new(cutoff: f32, sample_rate: f32) -> Self {
        let cutoff = cutoff.clamp(20.0, sample_rate * 0.45);
        let k = (std::f32::consts::PI * cutoff / sample_rate).tan();
        let norm = 1.0 / (1.0 + k);
        Self {
            b0: norm,
            b1: -norm,
            a1: (k - 1.0) * norm,
            x1: 0.0,
            y1: 0.0,
        }
    }

    pub(crate) fn process(&mut self, sample: f32) -> f32 {
        let out = self.b0 * sample + self.b1 * self.x1 - self.a1 * self.y1;
        self.x1 = sample;
        self.y1 = out;
        out
    }
}

#[derive(Clone, Debug)]
pub struct Spatial {
    position: f32,
    height: f32,
    taps: Vec<SpatialTap>,
    high: FirstOrderHighpass,
}

impl Spatial {
    pub fn new(room_size: f32, position: f32, height: f32, sample_rate: f32) -> Self {
        let room_size = room_size.clamp(0.0, 1.0);
        let delays = [5.0, 11.0, 17.0, 23.0, 31.0, 41.0];
        let gains = [0.4, 0.3, 0.25, 0.2, 0.15, 0.1];
        let ref_pans = [
            (0.3, 0.8),
            (0.7, 0.2),
            (0.4, 0.6),
            (0.6, 0.4),
            (0.2, 0.9),
            (0.9, 0.1),
        ];
        let height = height.clamp(0.0, 1.0);
        Self {
            position: position.clamp(0.0, 1.0),
            height,
            taps: delays
                .into_iter()
                .zip(gains)
                .zip(ref_pans)
                .map(|((delay, gain), (left_pan, right_pan))| {
                    let fold_down_gain = (left_pan + right_pan) * 0.5;
                    SpatialTap::new(
                        delay * room_size,
                        gain * fold_down_gain * room_size,
                        sample_rate,
                    )
                })
                .collect(),
            high: FirstOrderHighpass::new((4_000.0 + height * 8_000.0).max(2_000.0), sample_rate),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let pan = self.position * std::f32::consts::FRAC_PI_2;
        let pan_gain = (pan.cos() + pan.sin()) * 0.5;
        let reflections = self
            .taps
            .iter_mut()
            .map(|tap| tap.process(sample))
            .sum::<f32>();
        let height = self.high.process(sample) * self.height * 0.15;
        sample * pan_gain + reflections + height
    }
}

#[derive(Clone, Debug)]
struct PitchTap {
    buffer: Vec<f32>,
    write_pos: usize,
    read_pos: f32,
    ratio: f32,
}

impl PitchTap {
    fn new(ratio: f32, window_seconds: f32, sample_rate: f32) -> Self {
        let len = (window_seconds * sample_rate).round() as usize;
        Self {
            buffer: vec![0.0; len.max(8)],
            write_pos: 0,
            read_pos: 0.0,
            ratio: ratio.clamp(0.25, 4.0),
        }
    }

    fn process(&mut self, sample: f32) -> f32 {
        self.buffer[self.write_pos] = sample;
        let out = self.read_interp(self.read_pos);
        self.write_pos = (self.write_pos + 1) % self.buffer.len();
        self.read_pos = (self.read_pos + self.ratio) % self.buffer.len() as f32;
        out
    }

    #[cfg(test)]
    fn read_position(&self) -> f32 {
        self.read_pos
    }

    fn read_interp(&self, pos: f32) -> f32 {
        let len = self.buffer.len();
        let idx = pos.floor() as usize % len;
        let next = (idx + 1) % len;
        let frac = pos - idx as f32;
        self.buffer[idx] * (1.0 - frac) + self.buffer[next] * frac
    }
}

#[derive(Clone, Debug)]
struct VinylDelay {
    buffer: Vec<f32>,
    pos: usize,
}

impl VinylDelay {
    fn new(max_delay_seconds: f32, sample_rate: f32) -> Self {
        let samples = (max_delay_seconds * sample_rate).ceil() as usize + 2;
        Self {
            buffer: vec![0.0; samples.max(8)],
            pos: 0,
        }
    }

    fn process(&mut self, sample: f32, delay_samples: f32) -> f32 {
        self.buffer[self.pos] = sample;
        let out = self.read(delay_samples);
        self.pos = (self.pos + 1) % self.buffer.len();
        out
    }

    fn read(&self, delay_samples: f32) -> f32 {
        let len = self.buffer.len();
        let delay = delay_samples.clamp(0.0, (len - 2) as f32);
        let int = delay.floor() as usize;
        let frac = delay - int as f32;
        let a = self.buffer[(self.pos + len - int) % len];
        let b = self.buffer[(self.pos + len - int - 1) % len];
        a * (1.0 - frac) + b * frac
    }
}

#[derive(Clone, Debug)]
struct SimpleBandComp {
    threshold: f32,
    ratio: f32,
    attack_alpha: f32,
    release_alpha: f32,
    env: f32,
}

impl SimpleBandComp {
    fn new(threshold_db: f32, ratio: f32, attack: f32, release: f32, sample_rate: f32) -> Self {
        Self {
            threshold: db_to_gain(threshold_db),
            ratio,
            attack_alpha: (-1.0 / (attack * sample_rate).max(1.0)).exp(),
            release_alpha: (-1.0 / (release * sample_rate).max(1.0)).exp(),
            env: 0.0,
        }
    }

    fn process(&mut self, sample: f32) -> f32 {
        let input = sample.abs();
        let alpha = if input > self.env {
            self.attack_alpha
        } else {
            self.release_alpha
        };
        self.env = alpha * self.env + (1.0 - alpha) * input;
        if self.env <= self.threshold {
            return sample;
        }
        let over_db = 20.0 * (self.env / self.threshold + 1e-12).log10();
        let gain = 10.0_f32.powf(((over_db / self.ratio) - over_db) / 20.0);
        sample * gain
    }
}

#[derive(Clone, Debug)]
struct BodyMode {
    filter: Biquad,
    decay: f32,
    amp: f32,
    state: f32,
}

impl BodyMode {
    fn new(freq: f32, decay: f32, amp: f32, sample_rate: f32) -> Self {
        Self {
            filter: Biquad::new(FilterKind::Bandpass, freq, 0.8, sample_rate),
            decay: (-1.0 / (decay * sample_rate).max(1.0)).exp(),
            amp,
            state: 0.0,
        }
    }

    fn process(&mut self, sample: f32) -> f32 {
        self.state = self.state * self.decay + self.filter.process(sample);
        self.state * self.amp
    }
}

#[derive(Clone, Debug)]
struct SpatialTap {
    buffer: Vec<f32>,
    pos: usize,
    gain: f32,
}

impl SpatialTap {
    fn new(delay_ms: f32, gain: f32, sample_rate: f32) -> Self {
        let samples = (delay_ms.max(0.1) * sample_rate / 1_000.0).round() as usize;
        Self {
            buffer: vec![0.0; samples.max(1)],
            pos: 0,
            gain,
        }
    }

    fn process(&mut self, sample: f32) -> f32 {
        let delayed = self.buffer[self.pos];
        self.buffer[self.pos] = sample;
        self.pos = (self.pos + 1) % self.buffer.len();
        delayed * self.gain
    }
}

fn db_to_gain(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

fn butterworth_fourth_order_qs() -> [f32; 2] {
    [0.541_196_1, 1.306_563]
}

fn butterworth_sixth_order_qs() -> [f32; 3] {
    [0.517_638_1, std::f32::consts::FRAC_1_SQRT_2, 1.931_851_6]
}

fn process_filter_chain(filters: &mut [Biquad], sample: f32) -> f32 {
    filters
        .iter_mut()
        .fold(sample, |sample, filter| filter.process(sample))
}

fn random01(state: &mut u32) -> f32 {
    *state = state.wrapping_mul(1664525).wrapping_add(1013904223);
    (*state >> 8) as f32 / 16_777_216.0
}

fn signed_noise(state: &mut u32) -> f32 {
    *state = state.wrapping_mul(1664525).wrapping_add(1013904223);
    ((*state >> 8) as f32 / 16_777_216.0) * 2.0 - 1.0
}
