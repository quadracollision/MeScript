use super::advance_lfo;
use super::filters::{Biquad, FilterKind};

#[derive(Clone, Debug)]
pub struct TubeSaturation {
    drive: f32,
    asymmetry: f32,
    dc: f32,
}

impl TubeSaturation {
    pub fn new(drive: f32, asymmetry: f32) -> Self {
        Self {
            drive: drive.clamp(0.0, 1.0),
            asymmetry: asymmetry.clamp(0.0, 1.0),
            dc: 0.0,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let gain = 1.0 + self.drive * 10.0;
        let shaped = (sample * gain + self.asymmetry).tanh();
        self.dc = self.dc * 0.999 + shaped * 0.001;
        (shaped - self.dc) / (1.0 + self.drive)
    }
}

#[derive(Clone, Debug)]
pub struct Exciter {
    amount: f32,
    highpasses: Vec<Biquad>,
    dc_blocks: Vec<Biquad>,
}

impl Exciter {
    pub fn new(amount: f32, cutoff: f32, sample_rate: f32) -> Self {
        let butterworth_q = 0.707_f32;
        let resonance = ((butterworth_q - 0.5) / 11.5).clamp(0.0, 1.0);
        Self {
            amount: amount.clamp(0.0, 1.0),
            highpasses: (0..5)
                .map(|_| Biquad::new(FilterKind::Highpass, cutoff, resonance, sample_rate))
                .collect(),
            dc_blocks: (0..5)
                .map(|_| Biquad::new(FilterKind::Highpass, cutoff, resonance, sample_rate))
                .collect(),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let highs = self
            .highpasses
            .iter_mut()
            .fold(sample, |sample, filter| filter.process(sample));
        let harmonics = self
            .dc_blocks
            .iter_mut()
            .fold((highs * 5.0).tanh(), |sample, filter| {
                filter.process(sample)
            });
        sample + harmonics * self.amount * 0.5
    }
}

#[derive(Clone, Debug)]
pub struct Tape {
    saturation: f32,
    wow: f32,
    flutter: f32,
    wow_phase: f32,
    flutter_phase: f32,
    buffer: Vec<f32>,
    pos: usize,
}

#[derive(Clone, Debug)]
pub struct StuderTape {
    input_level: f32,
    head_bump: Biquad,
    hf_rolloff: Biquad,
    wow_phase: f32,
    flutter_phase: f32,
    buffer: Vec<f32>,
    pos: usize,
    noise: u32,
}

impl StuderTape {
    pub fn new(input_level: f32, speed: f32, bias: f32, sample_rate: f32) -> Self {
        let speed = speed.clamp(0.0, 2.0).round() as usize;
        let bump_freq = [40.0, 60.0, 100.0][speed];
        let hf_cutoff = ([10_000.0, 14_000.0, 18_000.0][speed]
            - (1.0 - bias.clamp(0.0, 1.0)) * 3_000.0)
            .max(4_000.0);
        Self {
            input_level: input_level.clamp(0.0, 1.0),
            head_bump: Biquad::new(FilterKind::Bandpass, bump_freq, 0.75, sample_rate),
            hf_rolloff: Biquad::new(FilterKind::Lowpass, hf_cutoff, 0.2, sample_rate),
            wow_phase: 0.0,
            flutter_phase: 0.0,
            buffer: vec![0.0; (0.006 * sample_rate).round() as usize + 4],
            pos: 0,
            noise: 0x57d3_a800,
        }
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let drive = 1.0 + self.input_level * 3.0;
        let mut x = (sample * drive).tanh() / drive.tanh().max(1e-6);
        x += self.head_bump.process(x) * 0.2;
        x = self.hf_rolloff.process(x);
        x += signed_noise(&mut self.noise) * 0.001;
        self.buffer[self.pos] = x;
        let wow = advance_lfo(&mut self.wow_phase, 0.4, sample_rate) * 0.001 * sample_rate;
        let flutter = advance_lfo(&mut self.flutter_phase, 7.5, sample_rate) * 0.0003 * sample_rate;
        let out = self.read((wow + flutter).abs().max(1.0));
        self.pos = (self.pos + 1) % self.buffer.len();
        out
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
}

impl Tape {
    pub fn new(saturation: f32, wow: f32, flutter: f32, sample_rate: f32) -> Self {
        let max_delay = (0.008 * sample_rate).ceil() as usize + 4;
        Self {
            saturation: saturation.clamp(0.0, 1.0),
            wow: wow.clamp(0.0, 1.0),
            flutter: flutter.clamp(0.0, 1.0),
            wow_phase: 0.0,
            flutter_phase: 0.0,
            buffer: vec![0.0; max_delay.max(8)],
            pos: 0,
        }
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let saturated =
            (sample * (1.0 + self.saturation * 5.0)).tanh() * (0.8 / (1.0 + self.saturation));
        self.buffer[self.pos] = saturated;

        let wow =
            advance_lfo(&mut self.wow_phase, 1.0, sample_rate) * self.wow * 0.005 * sample_rate;
        let flutter = advance_lfo(&mut self.flutter_phase, 12.0, sample_rate)
            * self.flutter
            * 0.001
            * sample_rate;
        let delayed = self.read((wow + flutter).max(0.0));
        self.pos = (self.pos + 1) % self.buffer.len();
        delayed
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

fn signed_noise(state: &mut u32) -> f32 {
    *state = state.wrapping_mul(1664525).wrapping_add(1013904223);
    ((*state >> 8) as f32 / 16_777_216.0) * 2.0 - 1.0
}
