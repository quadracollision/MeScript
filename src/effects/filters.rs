use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug)]
pub enum FilterKind {
    Lowpass,
    Highpass,
    Bandpass,
    Notch,
    Allpass,
    Peaking,
    LowShelf,
    HighShelf,
}

#[derive(Clone, Copy, Debug)]
pub enum Vowel {
    A,
    E,
    I,
    O,
    U,
}

#[derive(Clone, Debug)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

#[derive(Clone, Debug)]
pub struct Comb {
    buffer: Vec<f32>,
    pos: usize,
    feedback: f32,
    mix: f32,
}

impl Comb {
    pub fn new(delay_ms: f32, feedback: f32, mix: f32, sample_rate: f32) -> Self {
        let samples = (delay_ms.clamp(0.1, 250.0) * sample_rate / 1_000.0).floor() as usize;
        Self {
            buffer: vec![0.0; samples.max(1)],
            pos: 0,
            feedback: feedback.clamp(0.0, 0.95),
            mix: mix.clamp(0.0, 1.0),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let delayed = self.buffer[self.pos];
        self.buffer[self.pos] = sample;
        self.pos = (self.pos + 1) % self.buffer.len();
        let wet = sample + delayed * self.feedback;
        sample * (1.0 - self.mix) + wet * self.mix
    }

    #[cfg(test)]
    pub(crate) fn delay_samples_for_test(&self) -> usize {
        self.buffer.len()
    }
}

#[derive(Clone, Debug)]
pub struct Formant {
    bands: Vec<Biquad>,
    mix: f32,
}

impl Formant {
    pub fn new(vowel: Vowel, mix: f32, sample_rate: f32) -> Self {
        let freqs = match vowel {
            Vowel::A => [800.0, 1150.0, 2900.0, 3900.0, 4950.0],
            Vowel::E => [350.0, 2000.0, 2800.0, 3600.0, 4950.0],
            Vowel::I => [270.0, 2140.0, 2950.0, 3900.0, 4950.0],
            Vowel::O => [450.0, 800.0, 2830.0, 3800.0, 4950.0],
            Vowel::U => [325.0, 700.0, 2530.0, 3500.0, 4950.0],
        };
        Self {
            bands: freqs
                .into_iter()
                .filter(|freq| *freq < sample_rate * 0.5)
                .map(|freq| Biquad::new(FilterKind::Bandpass, freq, 0.3, sample_rate))
                .collect(),
            mix: mix.clamp(0.0, 1.0),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let wet = self
            .bands
            .iter_mut()
            .map(|band| band.process(sample))
            .sum::<f32>()
            * 0.3;
        sample * (1.0 - self.mix) + wet * self.mix
    }
}

impl Biquad {
    pub(crate) fn lowpass_with_q(cutoff: f32, q: f32, sample_rate: f32) -> Self {
        let cutoff = cutoff.clamp(20.0, sample_rate * 0.45);
        let q = q.max(0.001);
        let w0 = TAU * cutoff / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();
        let b0 = (1.0 - cos_w0) / 2.0;
        let b1 = 1.0 - cos_w0;
        let b2 = (1.0 - cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    pub(crate) fn highpass_with_q(cutoff: f32, q: f32, sample_rate: f32) -> Self {
        let cutoff = cutoff.clamp(1.0, sample_rate * 0.45);
        let q = q.max(0.001);
        let w0 = TAU * cutoff / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();
        let b0 = (1.0 + cos_w0) / 2.0;
        let b1 = -(1.0 + cos_w0);
        let b2 = (1.0 + cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    pub fn new(kind: FilterKind, cutoff: f32, resonance: f32, sample_rate: f32) -> Self {
        Self::new_with_gain(kind, cutoff, resonance, 0.0, sample_rate)
    }

    pub fn new_with_gain(
        kind: FilterKind,
        cutoff: f32,
        resonance: f32,
        gain_db: f32,
        sample_rate: f32,
    ) -> Self {
        let cutoff = cutoff.clamp(20.0, sample_rate * 0.45);
        let q = 0.5 + resonance.clamp(0.0, 1.0) * 11.5;
        let w0 = TAU * cutoff / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();
        let a_gain = 10.0_f32.powf(gain_db / 40.0);
        let (b0, b1, b2, a0, a1, a2) = match kind {
            FilterKind::Lowpass => (
                (1.0 - cos_w0) / 2.0,
                1.0 - cos_w0,
                (1.0 - cos_w0) / 2.0,
                1.0 + alpha,
                -2.0 * cos_w0,
                1.0 - alpha,
            ),
            FilterKind::Highpass => (
                (1.0 + cos_w0) / 2.0,
                -(1.0 + cos_w0),
                (1.0 + cos_w0) / 2.0,
                1.0 + alpha,
                -2.0 * cos_w0,
                1.0 - alpha,
            ),
            FilterKind::Bandpass => (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha),
            FilterKind::Notch => (
                1.0,
                -2.0 * cos_w0,
                1.0,
                1.0 + alpha,
                -2.0 * cos_w0,
                1.0 - alpha,
            ),
            FilterKind::Allpass => (
                1.0 - alpha,
                -2.0 * cos_w0,
                1.0 + alpha,
                1.0 + alpha,
                -2.0 * cos_w0,
                1.0 - alpha,
            ),
            FilterKind::Peaking => (
                1.0 + alpha * a_gain,
                -2.0 * cos_w0,
                1.0 - alpha * a_gain,
                1.0 + alpha / a_gain,
                -2.0 * cos_w0,
                1.0 - alpha / a_gain,
            ),
            FilterKind::LowShelf => {
                let sq = 2.0 * a_gain.sqrt() * alpha;
                (
                    a_gain * ((a_gain + 1.0) - (a_gain - 1.0) * cos_w0 + sq),
                    2.0 * a_gain * ((a_gain - 1.0) - (a_gain + 1.0) * cos_w0),
                    a_gain * ((a_gain + 1.0) - (a_gain - 1.0) * cos_w0 - sq),
                    (a_gain + 1.0) + (a_gain - 1.0) * cos_w0 + sq,
                    -2.0 * ((a_gain - 1.0) + (a_gain + 1.0) * cos_w0),
                    (a_gain + 1.0) + (a_gain - 1.0) * cos_w0 - sq,
                )
            }
            FilterKind::HighShelf => {
                let sq = 2.0 * a_gain.sqrt() * alpha;
                (
                    a_gain * ((a_gain + 1.0) + (a_gain - 1.0) * cos_w0 + sq),
                    -2.0 * a_gain * ((a_gain - 1.0) + (a_gain + 1.0) * cos_w0),
                    a_gain * ((a_gain + 1.0) + (a_gain - 1.0) * cos_w0 - sq),
                    (a_gain + 1.0) - (a_gain - 1.0) * cos_w0 + sq,
                    2.0 * ((a_gain - 1.0) - (a_gain + 1.0) * cos_w0),
                    (a_gain + 1.0) - (a_gain - 1.0) * cos_w0 - sq,
                )
            }
        };
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let out = self.b0 * sample + self.z1;
        self.z1 = self.b1 * sample - self.a1 * out + self.z2;
        self.z2 = self.b2 * sample - self.a2 * out;
        out
    }
}
