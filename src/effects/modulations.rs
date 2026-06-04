use super::advance_lfo;
use super::filters::{Biquad, FilterKind};

#[derive(Clone, Debug)]
pub struct Tremolo {
    phase: f32,
    rate: f32,
    depth: f32,
}

#[derive(Clone, Debug)]
struct VariableDelay {
    buffer: Vec<f32>,
    pos: usize,
}

impl VariableDelay {
    fn new(max_delay_seconds: f32, sample_rate: f32) -> Self {
        let samples = (max_delay_seconds.max(0.002) * sample_rate).ceil() as usize + 2;
        Self {
            buffer: vec![0.0; samples.max(4)],
            pos: 0,
        }
    }

    fn read(&self, delay_samples: f32) -> f32 {
        let len = self.buffer.len();
        let delay = delay_samples.clamp(1.0, (len - 2) as f32);
        let int = delay.floor() as usize;
        let frac = delay - int as f32;
        let a = self.buffer[(self.pos + len - int) % len];
        let b = self.buffer[(self.pos + len - int - 1) % len];
        a * (1.0 - frac) + b * frac
    }

    fn read_with_current(&self, delay_samples: f32, current: f32) -> f32 {
        if delay_samples <= 0.0 {
            return current;
        }
        if delay_samples < 1.0 {
            let previous = self.buffer[(self.pos + self.buffer.len() - 1) % self.buffer.len()];
            return current * (1.0 - delay_samples) + previous * delay_samples;
        }
        self.read(delay_samples)
    }

    fn write(&mut self, sample: f32) {
        self.buffer[self.pos] = sample;
        self.pos = (self.pos + 1) % self.buffer.len();
    }
}

#[derive(Clone, Debug)]
pub struct Chorus {
    delays: Vec<VariableDelay>,
    phases: Vec<f32>,
    rate: f32,
    depth: f32,
    mix: f32,
}

#[derive(Clone, Debug)]
pub struct Ensemble {
    delays: Vec<VariableDelay>,
    phases: Vec<f32>,
    rate: f32,
    depth: f32,
}

#[derive(Clone, Debug)]
pub struct Ce1Chorus {
    delay: VariableDelay,
    lowpass: Biquad,
    phase: f32,
    rate: f32,
    intensity: f32,
    noise: u32,
}

impl Ce1Chorus {
    pub fn new(rate: f32, intensity: f32, sample_rate: f32) -> Self {
        Self {
            delay: VariableDelay::new(0.012, sample_rate),
            lowpass: Biquad::new(FilterKind::Lowpass, 10_000.0, 0.2, sample_rate),
            phase: 0.0,
            rate: rate.clamp(0.01, 10.0),
            intensity: intensity.clamp(0.0, 1.0),
            noise: 0x0ce1_ce1u32,
        }
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        self.phase = (self.phase + self.rate / sample_rate) % 1.0;
        let triangle = 2.0 * (2.0 * self.phase - 1.0).abs() - 1.0;
        let delay_samples = (0.005 + triangle * self.intensity * 0.003) * sample_rate;
        let wet = self.lowpass.process(
            self.delay.read(delay_samples) + signed_noise(&mut self.noise) * 0.003 * self.intensity,
        );
        self.delay.write(sample);
        sample * 0.5 + wet * 0.5
    }
}

#[derive(Clone, Debug)]
pub struct Re301Chorus {
    delays: Vec<VariableDelay>,
    phases: Vec<f32>,
    lowpass: Biquad,
    rate: f32,
    depth: f32,
    noise: u32,
}

impl Re301Chorus {
    pub fn new(rate: f32, depth: f32, tone: f32, sample_rate: f32) -> Self {
        Self {
            delays: (0..3)
                .map(|_| VariableDelay::new(0.015, sample_rate))
                .collect(),
            phases: (0..3).map(|idx| idx as f32 / 3.0).collect(),
            lowpass: Biquad::new(
                FilterKind::Lowpass,
                (5_000.0 + tone.clamp(0.0, 1.0) * 7_000.0).max(3_000.0),
                0.2,
                sample_rate,
            ),
            rate: rate.clamp(0.01, 10.0),
            depth: depth.clamp(0.0, 1.0),
            noise: 0x3010_301u32,
        }
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let mut wet = 0.0;
        for (idx, delay) in self.delays.iter_mut().enumerate() {
            let lfo = advance_lfo(&mut self.phases[idx], self.rate, sample_rate);
            let delay_samples = (0.006 + lfo * self.depth * 0.003) * sample_rate;
            wet += delay.read(delay_samples);
            delay.write(sample);
        }
        wet /= self.delays.len().max(1) as f32;
        wet = self
            .lowpass
            .process(wet + signed_noise(&mut self.noise) * 0.002);
        sample * 0.5 + wet * 0.5
    }
}

#[derive(Clone, Debug)]
pub struct DimensionD {
    delay_a: VariableDelay,
    delay_b: VariableDelay,
    phase: f32,
    rate: f32,
    depth: f32,
    spread: f32,
}

#[derive(Clone, Debug)]
pub struct Dimension {
    delay: VariableDelay,
    phase: f32,
    rate: f32,
    depth: f32,
}

#[derive(Clone, Debug)]
pub struct H3000 {
    shifter: ResampleTap,
    mix: f32,
}

impl H3000 {
    pub fn new(
        detune_cents: f32,
        _delay_ms: f32,
        _feedback: f32,
        mix: f32,
        sample_rate: f32,
    ) -> Self {
        Self {
            shifter: ResampleTap::new(
                2.0_f32.powf(detune_cents / 1_200.0).clamp(0.5, 2.0),
                sample_rate,
            ),
            mix: mix.clamp(0.0, 1.0),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let wet = self.shifter.process(sample);
        (1.0 - self.mix) * sample + self.mix * wet
    }

    #[cfg(test)]
    pub(crate) fn pitch_read_position(&self) -> f32 {
        self.shifter.read_pos
    }
}

#[derive(Clone, Debug)]
struct ResampleTap {
    buffer: Vec<f32>,
    write_pos: usize,
    read_pos: f32,
    ratio: f32,
}

impl ResampleTap {
    fn new(ratio: f32, sample_rate: f32) -> Self {
        let len = (0.08 * sample_rate).round() as usize;
        Self {
            buffer: vec![0.0; len.max(8)],
            write_pos: 0,
            read_pos: 0.0,
            ratio,
        }
    }

    fn process(&mut self, sample: f32) -> f32 {
        self.buffer[self.write_pos] = sample;
        let out = self.read_interp(self.read_pos);
        self.write_pos = (self.write_pos + 1) % self.buffer.len();
        self.read_pos = (self.read_pos + self.ratio) % self.buffer.len() as f32;
        out
    }

    fn read_interp(&self, pos: f32) -> f32 {
        let len = self.buffer.len();
        let idx = pos.floor() as usize % len;
        let next = (idx + 1) % len;
        let frac = pos - idx as f32;
        self.buffer[idx] * (1.0 - frac) + self.buffer[next] * frac
    }
}

impl Dimension {
    pub fn new(mode: f32, sample_rate: f32) -> Self {
        let (rate, depth) = match mode.clamp(1.0, 4.0).floor() as usize {
            1 => (0.5, 0.001),
            2 => (0.8, 0.002),
            3 => (0.6, 0.004),
            _ => (1.2, 0.006),
        };
        Self {
            delay: VariableDelay::new(0.012, sample_rate),
            phase: 0.0,
            rate,
            depth,
        }
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let lfo = advance_lfo(&mut self.phase, self.rate, sample_rate);
        let delay_samples = (lfo * self.depth * sample_rate).max(0.0);
        let wet = self.delay.read_with_current(delay_samples, sample);
        self.delay.write(sample);
        sample * 0.5 + wet * 0.5
    }

    #[cfg(test)]
    pub(crate) fn mode_params(&self) -> (f32, f32) {
        (self.rate, self.depth)
    }
}

impl DimensionD {
    pub fn new(mode: f32, sample_rate: f32) -> Self {
        let (rate, depth, spread) = match mode.clamp(1.0, 4.0).floor() as usize {
            1 => (0.3, 0.0008, 0.3),
            2 => (0.5, 0.0015, 0.5),
            3 => (0.8, 0.0025, 0.7),
            _ => (1.2, 0.0035, 0.9),
        };
        Self {
            delay_a: VariableDelay::new(0.012, sample_rate),
            delay_b: VariableDelay::new(0.012, sample_rate),
            phase: 0.0,
            rate,
            depth,
            spread,
        }
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let lfo = advance_lfo(&mut self.phase, self.rate, sample_rate);
        let wet_a = self.delay_a.read(200.0 + lfo * self.depth * sample_rate);
        let wet_b = self.delay_b.read(250.0 - lfo * self.depth * sample_rate);
        self.delay_a.write(sample);
        self.delay_b.write(sample);
        let wet = (wet_a + wet_b) * 0.5;
        sample * (1.0 - self.spread * 0.4) + wet * self.spread * 0.4
    }

    #[cfg(test)]
    pub(crate) fn mode_params(&self) -> (f32, f32, f32) {
        (self.rate, self.depth, self.spread)
    }
}

impl Ensemble {
    pub fn new(voices: f32, depth: f32, rate: f32, sample_rate: f32) -> Self {
        let voices = voices.clamp(2.0, 12.0).round() as usize;
        let depth = depth.clamp(0.0005, 0.05);
        Self {
            delays: (0..voices)
                .map(|_| VariableDelay::new(depth * 2.5 + 0.02, sample_rate))
                .collect(),
            phases: (0..voices).map(|idx| idx as f32 / voices as f32).collect(),
            rate: rate.max(0.01),
            depth,
        }
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let mut wet = 0.0;
        for (idx, delay) in self.delays.iter_mut().enumerate() {
            let lfo = advance_lfo(&mut self.phases[idx], self.rate, sample_rate);
            let delay_samples = (self.depth + lfo * self.depth * 0.3) * sample_rate;
            wet += delay.read(delay_samples);
            delay.write(sample);
        }
        wet /= self.delays.len().max(1) as f32;
        sample * 0.5 + wet * 0.5
    }
}

fn signed_noise(state: &mut u32) -> f32 {
    *state = state.wrapping_mul(1664525).wrapping_add(1013904223);
    ((*state >> 8) as f32 / 16_777_216.0) * 2.0 - 1.0
}

impl Chorus {
    pub fn new(rate: f32, depth: f32, voices: f32, mix: f32, sample_rate: f32) -> Self {
        let voices = voices.clamp(1.0, 8.0).floor() as usize;
        Self {
            delays: (0..voices)
                .map(|_| VariableDelay::new(depth * 2.5 + 0.02, sample_rate))
                .collect(),
            phases: (0..voices).map(|idx| idx as f32 / voices as f32).collect(),
            rate: rate.max(0.01),
            depth: depth.clamp(0.0001, 0.05),
            mix: mix.clamp(0.0, 1.0),
        }
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let mut wet = 0.0;
        for (idx, delay) in self.delays.iter_mut().enumerate() {
            let lfo = advance_lfo(&mut self.phases[idx], self.rate, sample_rate);
            let delay_samples = (self.depth + lfo * self.depth * 0.5) * sample_rate;
            wet += delay.read_with_current(delay_samples, sample);
            delay.write(sample);
        }
        wet /= self.delays.len().max(1) as f32;
        (1.0 - self.mix) * sample + self.mix * wet
    }

    #[cfg(test)]
    pub(crate) fn voice_count(&self) -> usize {
        self.delays.len()
    }
}

#[derive(Clone, Debug)]
pub struct Flanger {
    delay: VariableDelay,
    phase: f32,
    rate: f32,
    depth: f32,
    feedback: f32,
    mix: f32,
}

impl Flanger {
    pub fn new(rate: f32, depth: f32, feedback: f32, mix: f32, sample_rate: f32) -> Self {
        Self {
            delay: VariableDelay::new(depth + 0.004, sample_rate),
            phase: 0.0,
            rate: rate.clamp(0.01, 20.0),
            depth: depth.clamp(0.0001, 0.02),
            feedback: feedback.clamp(0.0, 0.95),
            mix: mix.clamp(0.0, 1.0),
        }
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let lfo = (advance_lfo(&mut self.phase, self.rate, sample_rate) + 1.0) * 0.5;
        let delayed = self.delay.read((0.001 + lfo * self.depth) * sample_rate);
        self.delay.write(sample + delayed * self.feedback);
        (1.0 - self.mix) * sample + self.mix * delayed
    }
}

#[derive(Clone, Debug)]
pub struct Vibrato {
    delay: VariableDelay,
    phase: f32,
    rate: f32,
    depth: f32,
}

impl Vibrato {
    pub fn new(rate: f32, depth: f32, sample_rate: f32) -> Self {
        Self {
            delay: VariableDelay::new(depth * 2.5 + 0.003, sample_rate),
            phase: 0.0,
            rate: rate.max(0.01),
            depth: depth.clamp(0.0001, 0.03),
        }
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let lfo = advance_lfo(&mut self.phase, self.rate, sample_rate);
        let wet = self
            .delay
            .read_with_current(self.depth * (1.0 + lfo) * sample_rate, sample);
        self.delay.write(sample);
        wet
    }
}

#[derive(Clone, Debug)]
pub struct Phaser {
    phase: f32,
    rate: f32,
    depth: f32,
    mix: f32,
    z: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct SmallStone {
    phase: f32,
    rate: f32,
    depth: f32,
    feedback: f32,
    z: Vec<f32>,
    last: f32,
}

impl SmallStone {
    pub fn new(rate: f32, depth: f32, feedback: f32, color: bool) -> Self {
        Self {
            phase: 0.0,
            rate: rate.clamp(0.01, 20.0),
            depth: depth.clamp(0.0, 1.0),
            feedback: feedback.clamp(0.0, 0.95),
            z: vec![0.0; if color { 6 } else { 4 }],
            last: 0.0,
        }
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let lfo = (advance_lfo(&mut self.phase, self.rate, sample_rate) + 1.0) * 0.5;
        let sweep = 200.0 + lfo * self.depth * 6_000.0;
        let w = (std::f32::consts::PI * sweep / sample_rate).tan();
        let coeff = ((1.0 - w) / (1.0 + w)).clamp(-0.98, 0.98);
        let mut wet = sample + self.last * self.feedback;
        for idx in 0..self.z.len() {
            let input = if idx == 0 { wet } else { self.z[idx - 1] };
            let out = coeff * input + self.z[idx] - coeff * self.z[idx];
            self.z[idx] = input;
            wet = out;
        }
        self.last = wet;
        sample * 0.5 + wet * 0.5
    }
}

impl Phaser {
    pub fn new(rate: f32, depth: f32, stages: f32, mix: f32) -> Self {
        Self {
            phase: 0.0,
            rate: rate.clamp(0.01, 20.0),
            depth: depth.clamp(0.0, 1.0),
            mix: mix.clamp(0.0, 1.0),
            z: vec![0.0; stages.clamp(1.0, 12.0).floor() as usize],
        }
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let lfo = advance_lfo(&mut self.phase, self.rate, sample_rate);
        let coeff = 0.5 + self.depth * 0.4 * lfo;
        let mut wet = sample;
        for state in &mut self.z {
            let allpass = -coeff * wet + *state;
            *state = coeff * allpass + wet;
            wet = allpass;
        }
        (1.0 - self.mix) * sample + self.mix * wet
    }

    #[cfg(test)]
    pub(crate) fn stage_count(&self) -> usize {
        self.z.len()
    }
}

impl Tremolo {
    pub fn new(rate: f32, depth: f32) -> Self {
        Self {
            phase: 0.0,
            rate: rate.clamp(0.01, 40.0),
            depth: depth.clamp(0.0, 1.0),
        }
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let lfo = advance_lfo(&mut self.phase, self.rate, sample_rate);
        sample * (1.0 - self.depth * 0.5 * (1.0 + lfo))
    }
}
