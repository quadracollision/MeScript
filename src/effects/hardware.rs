use super::advance_lfo;
use super::filters::{Biquad, FilterKind};

#[derive(Clone, Debug)]
pub struct MoogLadder {
    cutoff: f32,
    resonance: f32,
    drive: f32,
    sample_rate: f32,
    s: [f32; 4],
}

impl MoogLadder {
    pub fn new(cutoff: f32, resonance: f32, drive: f32, sample_rate: f32) -> Self {
        Self {
            cutoff,
            resonance: resonance.clamp(0.0, 1.0),
            drive: drive.clamp(0.0, 1.0),
            sample_rate,
            s: [0.0; 4],
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let fc = (self.cutoff / self.sample_rate).clamp(0.0, 0.45);
        let k = 4.0 * self.resonance;
        let mut x = if self.drive > 0.0 {
            (sample * (1.0 + self.drive * 5.0)).tanh()
        } else {
            sample
        };
        x -= k * self.s[3];
        for idx in 0..4 {
            let input = if idx == 0 { x } else { self.s[idx - 1] };
            self.s[idx] += 2.0 * fc * ((input / 1.22).tanh() - (self.s[idx] / 1.22).tanh());
        }
        self.s[3]
    }
}

#[derive(Clone, Debug)]
pub struct ProphetFilter {
    cutoff: f32,
    resonance: f32,
    sample_rate: f32,
    s: [f32; 4],
}

impl ProphetFilter {
    pub fn new(cutoff: f32, resonance: f32, sample_rate: f32) -> Self {
        Self {
            cutoff,
            resonance: resonance.clamp(0.0, 1.0),
            sample_rate,
            s: [0.0; 4],
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let fc = self.cutoff.min(self.sample_rate * 0.45);
        let g = 2.0 * (std::f32::consts::PI * fc / self.sample_rate).sin();
        let k = self.resonance * 3.7;
        let x = sample - k * self.s[3].tanh();
        for idx in 0..4 {
            let input = if idx == 0 { x } else { self.s[idx - 1] };
            self.s[idx] += g * (input.tanh() - self.s[idx].tanh());
        }
        self.s[3]
    }
}

#[derive(Clone, Debug)]
pub struct ObxaFilter {
    cutoff: f32,
    resonance: f32,
    kind: FilterKind,
    sample_rate: f32,
    s1: f32,
    s2: f32,
}

impl ObxaFilter {
    pub fn new(cutoff: f32, resonance: f32, kind: FilterKind, sample_rate: f32) -> Self {
        Self {
            cutoff,
            resonance: resonance.clamp(0.0, 1.0),
            kind,
            sample_rate,
            s1: 0.0,
            s2: 0.0,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let fc = self.cutoff.clamp(20.0, self.sample_rate * 0.45);
        let g = 2.0 * (std::f32::consts::PI * fc / self.sample_rate).sin();
        let k = self.resonance * 3.5;
        let hp = sample - self.s1 * k - self.s2;
        let bp = hp * g + self.s1;
        let lp = bp * g + self.s2;
        self.s1 = bp;
        self.s2 = lp;
        let out = match self.kind {
            FilterKind::Lowpass => lp,
            FilterKind::Highpass => hp,
            FilterKind::Bandpass => bp,
            FilterKind::Notch => hp + lp,
            FilterKind::Allpass => sample,
            FilterKind::Peaking | FilterKind::LowShelf | FilterKind::HighShelf => sample,
        };
        (out * 1.1).tanh() * 0.95
    }
}

#[derive(Clone, Debug)]
pub struct WaspFilter {
    cutoff: f32,
    resonance: f32,
    sample_rate: f32,
    s1: f32,
    s2: f32,
}

impl WaspFilter {
    pub fn new(cutoff: f32, resonance: f32, sample_rate: f32) -> Self {
        Self {
            cutoff,
            resonance: resonance.clamp(0.0, 1.0),
            sample_rate,
            s1: 0.0,
            s2: 0.0,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let fc = self.cutoff.clamp(20.0, self.sample_rate * 0.45);
        let g = 2.0 * (std::f32::consts::PI * fc / self.sample_rate).sin();
        let k = self.resonance * 3.8;
        let x = sample - k * self.s2;
        let quantized = (x * 8.0).round() / 8.0;
        let mixed = 0.7 * x + 0.3 * quantized;
        self.s1 += g * (mixed - self.s1);
        self.s2 += g * (self.s1 - self.s2);
        (self.s2 * 1.5).tanh() * 0.9
    }
}

#[derive(Clone, Debug)]
pub struct SemFilter {
    cutoff: f32,
    resonance: f32,
    kind: FilterKind,
    sample_rate: f32,
    lp: f32,
    bp: f32,
}

impl SemFilter {
    pub fn new(cutoff: f32, resonance: f32, kind: FilterKind, sample_rate: f32) -> Self {
        Self {
            cutoff,
            resonance: resonance.clamp(0.0, 1.0),
            kind,
            sample_rate,
            lp: 0.0,
            bp: 0.0,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let fc = 2.0
            * (std::f32::consts::PI * self.cutoff.min(self.sample_rate * 0.45) / self.sample_rate)
                .sin();
        let q = 1.0 - self.resonance * 0.95;
        let hp = sample - self.lp - q * self.bp;
        self.bp += fc * hp;
        self.lp += fc * self.bp;
        match self.kind {
            FilterKind::Lowpass => self.lp,
            FilterKind::Highpass => hp,
            FilterKind::Bandpass => self.bp,
            FilterKind::Notch => hp + self.lp,
            FilterKind::Allpass => sample,
            FilterKind::Peaking | FilterKind::LowShelf | FilterKind::HighShelf => sample,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Ms20Filter {
    cutoff: f32,
    resonance: f32,
    sample_rate: f32,
    s1: f32,
    s2: f32,
}

impl Ms20Filter {
    pub fn new(cutoff: f32, resonance: f32, sample_rate: f32) -> Self {
        Self {
            cutoff,
            resonance: resonance.clamp(0.0, 1.0),
            sample_rate,
            s1: 0.0,
            s2: 0.0,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let fc = 2.0
            * (std::f32::consts::PI * self.cutoff.min(self.sample_rate * 0.45) / self.sample_rate)
                .sin();
        let feedback = (self.s2 * self.resonance * 3.8).tanh() * 0.8;
        let x = sample - feedback;
        let hp = x - self.s1;
        self.s1 += fc * hp;
        let hp2 = self.s1 - self.s2;
        self.s2 += fc * hp2;
        self.s2
    }
}

#[derive(Clone, Debug)]
pub struct Diode303 {
    cutoff: f32,
    resonance: f32,
    env_mod: f32,
    accent: f32,
    decay: f32,
    sample_rate: f32,
    age: f32,
    s: [f32; 4],
}

impl Diode303 {
    pub fn new(
        cutoff: f32,
        resonance: f32,
        env_mod: f32,
        accent: f32,
        decay: f32,
        sample_rate: f32,
    ) -> Self {
        Self {
            cutoff,
            resonance: resonance.clamp(0.0, 1.0),
            env_mod: env_mod.clamp(0.0, 1.0),
            accent: accent.clamp(0.0, 1.0),
            decay: decay.max(0.001),
            sample_rate,
            age: 0.0,
            s: [0.0; 4],
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let env = (1.0 - self.age / self.decay)
            .clamp(0.0, 1.0)
            .powf(1.0 + self.accent * 2.0);
        self.age += 1.0 / self.sample_rate;
        let max_cutoff = (self.cutoff + self.env_mod * 8_000.0).min(self.sample_rate * 0.45);
        let cutoff = self.cutoff + env * (max_cutoff - self.cutoff);
        let fc = (cutoff / self.sample_rate).clamp(0.0, 0.45);
        let k = 4.0 * self.resonance * 0.9;
        let mut x = sample - k * self.s[3];
        x = (x * 1.2).tanh() * 0.9;
        for idx in 0..4 {
            let input = if idx == 0 { x } else { self.s[idx - 1] };
            self.s[idx] += 2.0 * fc * (input - self.s[idx]);
        }
        self.s[3] * (1.0 + self.accent * 1.5)
    }
}

#[derive(Clone, Debug)]
pub struct SpaceEcho {
    buffer: Vec<f32>,
    pos: usize,
    feedback: f32,
    mix: f32,
    base_delay: f32,
    wow: f32,
    flutter: f32,
    wow_phase: f32,
    flutter_phase: f32,
    tone: Biquad,
    spring: Vec<SpringTap>,
    spring_mix: f32,
}

impl SpaceEcho {
    pub fn new(
        time: f32,
        feedback: f32,
        wow: f32,
        flutter: f32,
        tone: f32,
        spring_mix: f32,
        mix: f32,
        sample_rate: f32,
    ) -> Self {
        let delay = (time.clamp(0.02, 2.0) * sample_rate).floor() as usize;
        let extra = (0.01 * sample_rate).floor() as usize;
        let tone_filter = if tone < 0.5 {
            Biquad::new(
                FilterKind::Lowpass,
                800.0 + tone * 4_000.0,
                0.25,
                sample_rate,
            )
        } else {
            Biquad::new(FilterKind::Highpass, 200.0, 0.2, sample_rate)
        };
        Self {
            buffer: vec![0.0; delay + extra],
            pos: 0,
            feedback: feedback.clamp(0.0, 0.95),
            mix: mix.clamp(0.0, 1.0),
            base_delay: delay as f32,
            wow: wow.clamp(0.0, 1.0),
            flutter: flutter.clamp(0.0, 1.0),
            wow_phase: 0.0,
            flutter_phase: 0.0,
            tone: tone_filter,
            spring: [29.0, 37.0, 43.0, 53.0]
                .into_iter()
                .map(|ms| SpringTap::new(ms, sample_rate))
                .collect(),
            spring_mix: spring_mix.clamp(0.0, 1.0),
        }
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let wow =
            advance_lfo(&mut self.wow_phase, 0.5, sample_rate) * self.wow * 0.002 * sample_rate;
        let flutter = advance_lfo(&mut self.flutter_phase, 6.0, sample_rate)
            * self.flutter
            * 0.0005
            * sample_rate;
        let tape = (self.read(self.base_delay + wow + flutter) * 1.1).tanh();
        self.buffer[self.pos] = sample + tape * self.feedback;
        self.pos = (self.pos + 1) % self.buffer.len();

        let mut wet = self.tone.process(tape);
        if self.spring_mix > 0.0 {
            let spring = self
                .spring
                .iter_mut()
                .map(|tap| tap.process(wet))
                .sum::<f32>();
            wet += spring * self.spring_mix;
        }
        (1.0 - self.mix) * sample + self.mix * wet
    }

    fn read(&self, delay_samples: f32) -> f32 {
        let len = self.buffer.len();
        let delay = delay_samples.clamp(1.0, (len - 1) as f32);
        let int = delay.floor() as usize;
        self.buffer[(self.pos + len - int) % len]
    }

    #[cfg(test)]
    pub(crate) fn spring_probe(&mut self, sample: f32) -> f32 {
        self.spring.iter_mut().map(|tap| tap.process(sample)).sum()
    }

    #[cfg(test)]
    pub(crate) fn tape_settings_for_test(&self) -> (usize, usize) {
        (self.base_delay as usize, self.buffer.len())
    }

    #[cfg(test)]
    pub(crate) fn tape_read_probe_for_test(&self, delay_samples: f32) -> f32 {
        self.read(delay_samples)
    }

    #[cfg(test)]
    pub(crate) fn set_tape_probe_for_test(&mut self, pos: usize, values: &[f32]) {
        self.pos = pos % self.buffer.len();
        for (idx, value) in values.iter().enumerate().take(self.buffer.len()) {
            self.buffer[idx] = *value;
        }
    }
}

#[derive(Clone, Debug)]
struct SpringTap {
    buffer: Vec<f32>,
    pos: usize,
}

impl SpringTap {
    fn new(delay_ms: f32, sample_rate: f32) -> Self {
        let samples = (delay_ms * sample_rate / 1_000.0) as usize;
        Self {
            buffer: vec![0.0; samples.max(1)],
            pos: 0,
        }
    }

    fn process(&mut self, sample: f32) -> f32 {
        let delayed = self.buffer[self.pos] * 0.15;
        self.buffer[self.pos] = sample;
        self.pos = (self.pos + 1) % self.buffer.len();
        delayed
    }
}

#[derive(Clone, Debug)]
pub struct JunoHpf {
    filter: Biquad,
}

impl JunoHpf {
    pub fn new(cutoff: f32, resonance: f32, sample_rate: f32) -> Self {
        Self {
            filter: Biquad::new(FilterKind::Highpass, cutoff, resonance, sample_rate),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        self.filter.process(sample)
    }
}

#[derive(Clone, Debug)]
pub struct BuchlaLpg {
    strike: f32,
    decay: f32,
    resonance: f32,
    sample_rate: f32,
    age: f32,
    s0: f32,
    s1: f32,
}

impl BuchlaLpg {
    pub fn new(strike: f32, decay: f32, resonance: f32, sample_rate: f32) -> Self {
        Self {
            strike: strike.clamp(0.0, 1.0),
            decay: decay.max(0.001),
            resonance: resonance.clamp(0.0, 1.0),
            sample_rate,
            age: 0.0,
            s0: 0.0,
            s1: 0.0,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let fast = (-self.age / (self.decay * 0.15).max(0.001)).exp();
        let slow = (-self.age / (self.decay * 0.8).max(0.001)).exp();
        let env = self.strike * (0.7 * fast + 0.3 * slow);
        self.age += 1.0 / self.sample_rate;
        let max_fc = (self.sample_rate * 0.45).min(12_000.0);
        let cutoff = 50.0 + env * (max_fc - 50.0);
        let g = 2.0 * (std::f32::consts::PI * cutoff / self.sample_rate).sin();
        let k = self.resonance * 2.5;
        let input = sample * env;
        let x = input - k * self.s1;
        self.s0 += g * (x - self.s0);
        self.s1 += g * (self.s0 - self.s1);
        self.s1
    }
}

#[derive(Clone, Debug)]
pub struct SpringReverb {
    lines: Vec<SpringLine>,
    tone: Biquad,
    drip: Biquad,
    feedback: f32,
    drip_amount: f32,
    mix: f32,
    wet_peak: f32,
}

impl SpringReverb {
    pub fn new(decay: f32, tone: f32, mix: f32, drip: f32, sample_rate: f32) -> Self {
        let feedback = (0.4 + decay.clamp(0.0, 4.0) * 0.12).min(0.85);
        let tone = tone.clamp(0.0, 1.0);
        let tone_filter = if tone < 0.5 {
            Biquad::new(
                FilterKind::Lowpass,
                (1_000.0 + tone * 6_000.0).max(800.0),
                0.25,
                sample_rate,
            )
        } else {
            Biquad::new(FilterKind::Highpass, 300.0, 0.2, sample_rate)
        };
        Self {
            lines: [30.0, 42.0, 67.0]
                .into_iter()
                .map(|delay_ms| SpringLine::new(delay_ms, sample_rate))
                .collect(),
            tone: tone_filter,
            drip: Biquad::new(FilterKind::Bandpass, 3_000.0, 0.7, sample_rate),
            feedback,
            drip_amount: drip.clamp(0.0, 1.0),
            mix: mix.clamp(0.0, 1.0),
            wet_peak: 1.0,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let wet = self
            .lines
            .iter_mut()
            .map(|line| line.process(sample, self.feedback))
            .sum::<f32>()
            / self.lines.len().max(1) as f32;
        let drip = self.drip.process(wet).tanh() * self.drip_amount;
        let wet = self.tone.process((wet + drip).tanh());
        self.wet_peak = (self.wet_peak * 0.9999).max(wet.abs()).max(1e-6);
        let wet = wet / self.wet_peak;
        (1.0 - self.mix) * sample + self.mix * wet
    }
}

#[derive(Clone, Debug)]
struct SpringLine {
    buffer: Vec<f32>,
    pos: usize,
}

impl SpringLine {
    fn new(delay_ms: f32, sample_rate: f32) -> Self {
        let samples = (delay_ms * sample_rate / 1_000.0).round() as usize;
        Self {
            buffer: vec![0.0; samples.max(1)],
            pos: 0,
        }
    }

    fn process(&mut self, sample: f32, feedback: f32) -> f32 {
        let delayed = self.buffer[self.pos];
        let boing = (delayed * 1.1).tanh();
        self.buffer[self.pos] = sample + boing * feedback;
        self.pos = (self.pos + 1) % self.buffer.len();
        boing
    }
}

#[derive(Clone, Debug)]
pub struct NevePreamp {
    gain: f32,
    warmth: f32,
    low: Biquad,
    high: Biquad,
}

impl NevePreamp {
    pub fn new(gain: f32, warmth: f32, sample_rate: f32) -> Self {
        Self {
            gain: gain.clamp(0.0, 1.0),
            warmth: warmth.clamp(0.0, 1.0),
            low: Biquad::new(FilterKind::Lowpass, 80.0, 0.2, sample_rate),
            high: Biquad::new(FilterKind::Highpass, 8_000.0, 0.2, sample_rate),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let driven = sample * (1.0 + self.gain * 3.0);
        let mut out = (driven * 1.3).tanh() * 0.85 + 0.03 * driven * driven;
        out += self.low.process(sample) * self.warmth * 0.15;
        out += self.high.process(out) * 0.05;
        out
    }
}

#[derive(Clone, Debug)]
pub struct MarshallAmp {
    gain: f32,
    presence: f32,
    low_cut: Biquad,
    mid_scoop: Biquad,
    presence_filter: Biquad,
    cab_rolloff: Vec<Biquad>,
    cab_resonance: Biquad,
    peak: f32,
}

impl MarshallAmp {
    pub fn new(gain: f32, tone: f32, presence: f32, sample_rate: f32) -> Self {
        let tone = tone.clamp(0.0, 1.0);
        let presence = presence.clamp(0.0, 1.0);
        let mid_q = 1.5_f32;
        let mid_resonance = ((mid_q - 0.5) / 11.5).clamp(0.0, 1.0);
        let scoop_db = -3.0 * (1.0 - tone);
        Self {
            gain: gain.clamp(0.0, 1.0),
            presence,
            low_cut: Biquad::new(
                FilterKind::Highpass,
                80.0 + (1.0 - tone) * 300.0,
                0.25,
                sample_rate,
            ),
            mid_scoop: Biquad::new_with_gain(
                FilterKind::Peaking,
                600.0,
                mid_resonance,
                scoop_db,
                sample_rate,
            ),
            presence_filter: Biquad::new(
                FilterKind::Highpass,
                3_000.0 + presence * 3_000.0,
                0.2,
                sample_rate,
            ),
            cab_rolloff: (0..2)
                .map(|_| Biquad::new(FilterKind::Lowpass, 5_000.0, 0.2, sample_rate))
                .collect(),
            cab_resonance: Biquad::new(FilterKind::Bandpass, 120.0, 0.75, sample_rate),
            peak: 1.0,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let mut out = sample * (1.0 + self.gain * 8.0);
        out = (out * 1.5).tanh();
        if self.gain > 0.3 {
            out = (out * (1.0 + self.gain * 2.0)).tanh();
        }
        out = (out + 0.1 * out * out).tanh();
        out = self.low_cut.process(out);
        out = self.mid_scoop.process(out);
        out += self.presence_filter.process(out) * self.presence * 0.2;
        let rolled = self
            .cab_rolloff
            .iter_mut()
            .fold(out, |sample, filter| filter.process(sample));
        out = rolled + self.cab_resonance.process(out) * 0.18;
        self.peak = (self.peak * 0.9999).max(out.abs()).max(1e-6);
        (out / self.peak).clamp(-1.0, 1.0)
    }

    #[cfg(test)]
    pub(crate) fn mid_scoop_probe(&mut self, sample: f32) -> f32 {
        self.mid_scoop.process(sample)
    }
}

#[derive(Clone, Debug)]
pub struct VoxAc30 {
    gain: f32,
    top_boost: Biquad,
    cut: Biquad,
    cab: Vec<Biquad>,
}

impl VoxAc30 {
    pub fn new(gain: f32, treble: f32, cut: f32, sample_rate: f32) -> Self {
        let treble = treble.clamp(0.0, 1.0);
        let top_boost_q = 1.5_f32;
        let top_boost_resonance = ((top_boost_q - 0.5) / 11.5).clamp(0.0, 1.0);
        Self {
            gain: gain.clamp(0.0, 1.0),
            top_boost: Biquad::new_with_gain(
                FilterKind::Peaking,
                2_500.0,
                top_boost_resonance,
                treble * 6.0,
                sample_rate,
            ),
            cut: Biquad::new(
                FilterKind::Lowpass,
                (12_000.0 - cut.clamp(0.0, 1.0) * 8_000.0).max(2_000.0),
                0.2,
                sample_rate,
            ),
            cab: (0..2)
                .map(|_| Biquad::new(FilterKind::Lowpass, 7_000.0, 0.2, sample_rate))
                .collect(),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let x = sample * (1.0 + self.gain * 5.0);
        let pos = x.max(0.0);
        let neg = x.min(0.0);
        let mut out = (pos * 1.2).tanh() + (neg * 1.8).tanh() * 0.8;
        out = self.top_boost.process(out);
        out = self.cut.process(out);
        self.cab
            .iter_mut()
            .fold(out, |sample, filter| filter.process(sample))
            .clamp(-1.0, 1.0)
    }

    #[cfg(test)]
    pub(crate) fn top_boost_probe(&mut self, sample: f32) -> f32 {
        self.top_boost.process(sample)
    }
}

#[derive(Clone, Debug)]
pub struct FenderTwin {
    volume: f32,
    bass: f32,
    treble: f32,
    bass_filter: Biquad,
    treble_filter: Biquad,
    spring: SpringReverb,
}

impl FenderTwin {
    pub fn new(volume: f32, treble: f32, bass: f32, reverb_mix: f32, sample_rate: f32) -> Self {
        let bass = bass.clamp(0.0, 1.0);
        let treble = treble.clamp(0.0, 1.0);
        Self {
            volume: volume.clamp(0.0, 1.0),
            bass,
            treble,
            bass_filter: Biquad::new(FilterKind::Lowpass, 100.0 + bass * 200.0, 0.2, sample_rate),
            treble_filter: Biquad::new(
                FilterKind::Highpass,
                3_000.0 + treble * 3_000.0,
                0.2,
                sample_rate,
            ),
            spring: SpringReverb::new(1.0, 0.55, reverb_mix, 0.35, sample_rate),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let mut out = sample * (1.0 + self.volume * 3.0);
        out = (out * 0.8).tanh() / 0.8;
        out += self.bass_filter.process(out) * self.bass * 0.3;
        out += self.treble_filter.process(out) * self.treble * 0.15;
        self.spring.process(out).clamp(-1.0, 1.0)
    }
}

#[derive(Clone, Debug)]
pub struct PultecEq {
    low_boost: f32,
    low_atten: f32,
    high_boost: f32,
    high_atten: f32,
    low: Vec<Biquad>,
    low_bump: Biquad,
    high: Biquad,
}

impl PultecEq {
    pub fn new(
        low_boost: f32,
        low_atten: f32,
        low_freq: f32,
        high_boost: f32,
        high_atten: f32,
        high_freq: f32,
        sample_rate: f32,
    ) -> Self {
        let low_freq = low_freq.max(30.0);
        let bump_q = 2.0_f32;
        let bump_resonance = ((bump_q - 0.5) / 11.5).clamp(0.0, 1.0);
        let boost_db = (low_boost.clamp(0.0, 1.0) * 3.0).min(4.0);
        Self {
            low_boost: low_boost.clamp(0.0, 1.0),
            low_atten: low_atten.clamp(0.0, 1.0),
            high_boost: high_boost.clamp(0.0, 1.0),
            high_atten: high_atten.clamp(0.0, 1.0),
            low: (0..2)
                .map(|_| Biquad::new(FilterKind::Lowpass, low_freq, 0.018, sample_rate))
                .collect(),
            low_bump: Biquad::new_with_gain(
                FilterKind::Peaking,
                low_freq * 1.5,
                bump_resonance,
                boost_db,
                sample_rate,
            ),
            high: Biquad::new(FilterKind::Highpass, high_freq, 0.2, sample_rate),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let lows = self
            .low
            .iter_mut()
            .fold(sample, |sample, filter| filter.process(sample));
        let highs = self.high.process(sample);
        let mut out = sample + lows * self.low_boost * 0.4 - lows * self.low_atten * 0.3;
        if self.low_boost > 0.0 && self.low_atten > 0.0 {
            out = self.low_bump.process(out);
        }
        out += highs * self.high_boost * 0.3 - highs * self.high_atten * 0.25;
        (out + 0.01 * out * out).clamp(-1.0, 1.0)
    }

    #[cfg(test)]
    pub(crate) fn low_bump_probe(&mut self, sample: f32) -> f32 {
        self.low_bump.process(sample)
    }
}

#[derive(Clone, Debug)]
pub struct Tc2290 {
    buffer: Vec<f32>,
    pos: usize,
    delay_samples: f32,
    feedback: f32,
    mod_rate: f32,
    mod_depth_samples: f32,
    mix: f32,
    phase: f32,
}

impl Tc2290 {
    pub fn new(
        time_ms: f32,
        feedback: f32,
        mod_rate: f32,
        mod_depth: f32,
        mix: f32,
        sample_rate: f32,
    ) -> Self {
        let delay_samples = (time_ms.clamp(1.0, 2_000.0) * sample_rate / 1_000.0).floor();
        let mod_depth_samples = mod_depth.clamp(0.0, 0.05) * sample_rate;
        let len = delay_samples as usize + mod_depth_samples.floor() as usize + 100;
        Self {
            buffer: vec![0.0; len.max(4)],
            pos: 0,
            delay_samples,
            feedback: feedback.clamp(0.0, 0.95),
            mod_rate: mod_rate.clamp(0.0, 20.0),
            mod_depth_samples,
            mix: mix.clamp(0.0, 1.0),
            phase: 0.0,
        }
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let lfo = advance_lfo(&mut self.phase, self.mod_rate, sample_rate);
        let read_offset = self.delay_samples + lfo * self.mod_depth_samples;
        let delayed = self.read(read_offset);
        self.buffer[self.pos] = sample + delayed * self.feedback;
        self.pos = (self.pos + 1) % self.buffer.len();
        (1.0 - self.mix) * sample + self.mix * delayed
    }

    fn read(&self, delay_samples: f32) -> f32 {
        let len = self.buffer.len();
        let delay = delay_samples.clamp(1.0, (len - 2) as f32);
        let int = delay.floor() as usize;
        let frac = delay - int as f32;
        let a = self.buffer[(self.pos + len - int) % len];
        let b = self.buffer[(self.pos + len - int + 1) % len];
        a * (1.0 - frac) + b * frac
    }

    #[cfg(test)]
    pub(crate) fn settings_for_test(&self) -> (usize, usize) {
        (self.delay_samples as usize, self.buffer.len())
    }

    #[cfg(test)]
    pub(crate) fn set_probe_for_test(&mut self, pos: usize, values: &[f32]) {
        self.pos = pos % self.buffer.len();
        for (idx, value) in values.iter().enumerate().take(self.buffer.len()) {
            self.buffer[idx] = *value;
        }
    }

    #[cfg(test)]
    pub(crate) fn read_probe_for_test(&self, delay_samples: f32) -> f32 {
        self.read(delay_samples)
    }
}

#[derive(Clone, Debug)]
pub struct EmtPlate {
    pre_delay: SimpleDelay,
    diffusers: Vec<PlateAllpass>,
    lines: Vec<PlateLine>,
    damping: Biquad,
    mix: f32,
    wet_peak: f32,
}

impl EmtPlate {
    pub fn new(decay: f32, damping: f32, mix: f32, pre_delay_ms: f32, sample_rate: f32) -> Self {
        let feedback = (0.3 + decay.clamp(0.1, 5.0) * 0.15).min(0.95);
        let damping = damping.clamp(0.0, 1.0);
        Self {
            pre_delay: SimpleDelay::new(pre_delay_ms, sample_rate),
            diffusers: [113.0, 199.0, 307.0, 421.0]
                .into_iter()
                .map(|samples| PlateAllpass::new(samples as usize, 0.6))
                .collect(),
            lines: [1427.0, 1781.0, 2099.0, 2467.0]
                .into_iter()
                .map(|samples| PlateLine::new(samples as usize, feedback))
                .collect(),
            damping: Biquad::new(
                FilterKind::Lowpass,
                (12_000.0 - damping * 10_000.0).max(1_000.0),
                0.2,
                sample_rate,
            ),
            mix: mix.clamp(0.0, 1.0),
            wet_peak: 1.0,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let mut wet = self.pre_delay.process(sample);
        for diffuser in &mut self.diffusers {
            wet = diffuser.process(wet);
        }
        wet = self
            .lines
            .iter_mut()
            .map(|line| line.process(wet))
            .sum::<f32>()
            * 0.25;
        wet = self.damping.process(wet);
        self.wet_peak = (self.wet_peak * 0.9999).max(wet.abs()).max(1e-6);
        wet /= self.wet_peak;
        (1.0 - self.mix) * sample + self.mix * wet
    }
}

#[derive(Clone, Debug)]
pub struct Lexicon224 {
    pre_delay: SimpleDelay,
    diffusers: Vec<PlateAllpass>,
    lines: Vec<LexiconLine>,
    damping: Biquad,
    mix: f32,
    phase: f32,
    wet_peak: f32,
}

impl Lexicon224 {
    pub fn new(
        size: f32,
        decay: f32,
        damping: f32,
        pre_delay_ms: f32,
        mix: f32,
        sample_rate: f32,
    ) -> Self {
        let size = size.clamp(0.2, 2.0);
        let feedback = (0.25 + decay.clamp(0.1, 8.0) * 0.12).min(0.92);
        Self {
            pre_delay: SimpleDelay::new(pre_delay_ms, sample_rate),
            diffusers: [142.0, 236.0, 379.0, 503.0]
                .into_iter()
                .map(|samples| PlateAllpass::new((samples * size).floor() as usize, 0.55))
                .collect(),
            lines: [1557.0, 1933.0, 2311.0, 2731.0]
                .into_iter()
                .enumerate()
                .map(|(idx, samples)| {
                    LexiconLine::new(
                        (samples * size).floor() as usize,
                        feedback,
                        idx as f32 * 0.17,
                    )
                })
                .collect(),
            damping: Biquad::new(
                FilterKind::Lowpass,
                (14_000.0 - damping.clamp(0.0, 1.0) * 12_000.0).max(2_000.0),
                0.2,
                sample_rate,
            ),
            mix: mix.clamp(0.0, 1.0),
            phase: 0.0,
            wet_peak: 1.0,
        }
    }

    #[cfg(test)]
    pub(crate) fn delay_lengths_for_test(&self) -> (usize, Vec<usize>, Vec<usize>) {
        (
            self.pre_delay.len_for_test(),
            self.diffusers
                .iter()
                .map(PlateAllpass::len_for_test)
                .collect(),
            self.lines.iter().map(LexiconLine::len_for_test).collect(),
        )
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let mut wet = self.pre_delay.process(sample);
        for diffuser in &mut self.diffusers {
            wet = diffuser.process(wet);
        }
        let mod_lfo = advance_lfo(&mut self.phase, 0.7, sample_rate) * 3.0;
        wet = self
            .lines
            .iter_mut()
            .map(|line| line.process(wet, mod_lfo))
            .sum::<f32>()
            * 0.25;
        wet = self.damping.process(wet);
        self.wet_peak = (self.wet_peak * 0.9999).max(wet.abs()).max(1e-6);
        wet /= self.wet_peak;
        (1.0 - self.mix) * sample + self.mix * wet
    }
}

#[derive(Clone, Copy, Debug)]
pub enum AmsProgram {
    Nonlin,
    Ambience,
    Plate,
}

#[derive(Clone, Debug)]
pub struct AmsReverb {
    lines: Vec<AmsLine>,
    damping: f32,
    mix: f32,
    program: AmsProgram,
    age: usize,
    sample_rate: f32,
    decay: f32,
}

impl AmsReverb {
    pub fn new(decay: f32, damping: f32, program: AmsProgram, mix: f32, sample_rate: f32) -> Self {
        let feedback = (0.7 + decay.clamp(0.1, 5.0) * 0.05).min(0.95);
        Self {
            lines: [29.0, 37.0, 43.0, 53.0, 67.0, 79.0]
                .into_iter()
                .map(|delay_ms| AmsLine::new(delay_ms, feedback, sample_rate))
                .collect(),
            damping: damping.clamp(0.0, 1.0),
            mix: mix.clamp(0.0, 1.0),
            program,
            age: 0,
            sample_rate,
            decay: decay.max(0.001),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let mut wet = self
            .lines
            .iter_mut()
            .map(|line| line.process(sample, self.damping))
            .sum::<f32>()
            / self.lines.len().max(1) as f32;
        let t = self.age as f32 / self.sample_rate;
        let env = match self.program {
            AmsProgram::Nonlin => self.nonlin_envelope(),
            AmsProgram::Ambience => (-t / (self.decay * 0.3).max(0.001)).exp(),
            AmsProgram::Plate => (-t / self.decay.max(0.001)).exp().max(0.15),
        };
        self.age = self.age.saturating_add(1);
        wet = (wet * env).tanh();
        (1.0 - self.mix) * sample + self.mix * wet
    }

    fn nonlin_envelope(&self) -> f32 {
        let gate_samples = (self.decay.min(0.8) * self.sample_rate).floor() as usize;
        if gate_samples == 0 {
            return 0.0;
        }
        if self.age >= gate_samples {
            return 0.0;
        }
        if gate_samples == 1 {
            return 1.0;
        }
        1.0 - 0.8 * (self.age as f32 / (gate_samples - 1) as f32)
    }

    #[cfg(test)]
    pub(crate) fn line_lengths_for_test(&self) -> Vec<usize> {
        self.lines.iter().map(AmsLine::len_for_test).collect()
    }

    #[cfg(test)]
    pub(crate) fn nonlin_envelope_for_test(&self) -> f32 {
        self.nonlin_envelope()
    }

    #[cfg(test)]
    pub(crate) fn set_age_for_test(&mut self, age: usize) {
        self.age = age;
    }
}

#[derive(Clone, Debug)]
struct SimpleDelay {
    buffer: Vec<f32>,
    pos: usize,
}

impl SimpleDelay {
    fn new(delay_ms: f32, sample_rate: f32) -> Self {
        let samples = (delay_ms.max(0.0) * sample_rate / 1_000.0).floor() as usize;
        Self {
            buffer: vec![0.0; samples],
            pos: 0,
        }
    }

    fn process(&mut self, sample: f32) -> f32 {
        if self.buffer.is_empty() {
            return sample;
        }
        let delayed = self.buffer[self.pos];
        self.buffer[self.pos] = sample;
        self.pos = (self.pos + 1) % self.buffer.len();
        delayed
    }

    #[cfg(test)]
    fn len_for_test(&self) -> usize {
        self.buffer.len()
    }
}

#[derive(Clone, Debug)]
struct PlateAllpass {
    buffer: Vec<f32>,
    pos: usize,
    gain: f32,
}

impl PlateAllpass {
    fn new(samples: usize, gain: f32) -> Self {
        Self {
            buffer: vec![0.0; samples.max(1)],
            pos: 0,
            gain,
        }
    }

    fn process(&mut self, sample: f32) -> f32 {
        let delayed = self.buffer[self.pos];
        let out = -self.gain * sample + delayed;
        self.buffer[self.pos] = sample + delayed * self.gain;
        self.pos = (self.pos + 1) % self.buffer.len();
        out
    }

    #[cfg(test)]
    fn len_for_test(&self) -> usize {
        self.buffer.len()
    }
}

#[derive(Clone, Debug)]
struct PlateLine {
    buffer: Vec<f32>,
    pos: usize,
    feedback: f32,
}

impl PlateLine {
    fn new(samples: usize, feedback: f32) -> Self {
        Self {
            buffer: vec![0.0; samples.max(1)],
            pos: 0,
            feedback,
        }
    }

    fn process(&mut self, sample: f32) -> f32 {
        let delayed = self.buffer[self.pos];
        self.buffer[self.pos] = sample + delayed * self.feedback;
        self.pos = (self.pos + 1) % self.buffer.len();
        delayed
    }
}

#[derive(Clone, Debug)]
struct LexiconLine {
    buffer: Vec<f32>,
    pos: usize,
    feedback: f32,
    phase_offset: f32,
}

impl LexiconLine {
    fn new(samples: usize, feedback: f32, phase_offset: f32) -> Self {
        Self {
            buffer: vec![0.0; samples.max(4)],
            pos: 0,
            feedback,
            phase_offset,
        }
    }

    fn process(&mut self, sample: f32, modulation: f32) -> f32 {
        let delay = (self.buffer.len() as f32 - 2.0 + modulation * (1.0 + self.phase_offset))
            .clamp(1.0, (self.buffer.len() - 2) as f32);
        let delayed = self.read(delay);
        self.buffer[self.pos] = sample + delayed * self.feedback;
        self.pos = (self.pos + 1) % self.buffer.len();
        delayed
    }

    fn read(&self, delay_samples: f32) -> f32 {
        let len = self.buffer.len();
        let int = delay_samples.floor() as usize;
        let frac = delay_samples - int as f32;
        let a = self.buffer[(self.pos + len - int) % len];
        let b = self.buffer[(self.pos + len - int - 1) % len];
        a * (1.0 - frac) + b * frac
    }

    #[cfg(test)]
    fn len_for_test(&self) -> usize {
        self.buffer.len()
    }
}

#[derive(Clone, Debug)]
struct AmsLine {
    buffer: Vec<f32>,
    pos: usize,
    feedback: f32,
}

impl AmsLine {
    fn new(delay_ms: f32, feedback: f32, sample_rate: f32) -> Self {
        let samples = (delay_ms * sample_rate / 1_000.0).floor() as usize;
        Self {
            buffer: vec![0.0; samples.max(1)],
            pos: 0,
            feedback,
        }
    }

    fn process(&mut self, sample: f32, damping: f32) -> f32 {
        let delayed = self.buffer[self.pos];
        let damped = delayed * (1.0 - damping * 0.5);
        self.buffer[self.pos] = sample + damped * self.feedback;
        self.pos = (self.pos + 1) % self.buffer.len();
        delayed
    }

    #[cfg(test)]
    fn len_for_test(&self) -> usize {
        self.buffer.len()
    }
}
