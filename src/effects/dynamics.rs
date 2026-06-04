use super::filters::{Biquad, FilterKind};

#[derive(Clone, Debug)]
pub struct Compressor {
    threshold_db: f32,
    ratio: f32,
    makeup: f32,
    attack_alpha: f32,
    release_alpha: f32,
    env: f32,
}

#[derive(Clone, Debug)]
pub struct Dbx160 {
    threshold: f32,
    ratio: f32,
    attack_alpha: f32,
    release_alpha: f32,
    env: f32,
    makeup: f32,
}

#[derive(Clone, Debug)]
pub struct Fairchild {
    input_gain: f32,
    threshold: f32,
    attack_alpha: f32,
    release_alpha: f32,
    env: f32,
    mix: f32,
}

impl Fairchild {
    pub fn new(
        input_gain: f32,
        threshold_db: f32,
        time_constant: f32,
        mix: f32,
        sample_rate: f32,
    ) -> Self {
        let (attack, release) = match time_constant.clamp(1.0, 6.0).floor() as usize {
            1 => (0.0002, 0.03),
            2 => (0.0002, 0.08),
            3 => (0.0004, 0.2),
            4 => (0.001, 0.5),
            5 => (0.002, 1.0),
            _ => (0.004, 2.0),
        };
        Self {
            input_gain: input_gain.clamp(0.0, 1.0),
            threshold: db_to_gain(threshold_db),
            attack_alpha: python_time_alpha(attack, sample_rate),
            release_alpha: python_time_alpha(release, sample_rate),
            env: 0.0,
            mix: mix.clamp(0.0, 1.0),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let mut driven = sample * (1.0 + self.input_gain * 4.0);
        driven = (driven * 1.2).tanh() / 1.2;
        self.track_env(driven.abs());
        let gain = if self.env > self.threshold {
            (self.threshold / (self.env + 1e-9)).sqrt()
        } else {
            1.0
        };
        let out = driven * gain;
        let out = out + 0.02 * out * out - 0.005 * out * out * out;
        (1.0 - self.mix) * sample + self.mix * out
    }

    fn track_env(&mut self, input: f32) {
        let alpha = if input > self.env {
            self.attack_alpha
        } else {
            self.release_alpha
        };
        self.env = alpha * self.env + (1.0 - alpha) * input;
    }
}

#[derive(Clone, Debug)]
pub struct SslComp {
    threshold: f32,
    ratio: f32,
    attack_alpha: f32,
    release_alpha: f32,
    env: f32,
    makeup: f32,
}

impl SslComp {
    pub fn new(
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        makeup_db: f32,
        sample_rate: f32,
    ) -> Self {
        Self {
            threshold: db_to_gain(threshold_db),
            ratio: ratio.max(1.0),
            attack_alpha: python_time_alpha(attack_ms.max(0.01) / 1_000.0, sample_rate),
            release_alpha: python_time_alpha(release_ms.max(1.0) / 1_000.0, sample_rate),
            env: 0.0,
            makeup: db_to_gain(makeup_db),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let input = sample.abs();
        let alpha = if input > self.env {
            self.attack_alpha
        } else {
            self.release_alpha
        };
        self.env = alpha * self.env + (1.0 - alpha) * input;
        if self.env <= self.threshold {
            return sample * self.makeup;
        }
        let over_db = 20.0 * (self.env / self.threshold + 1e-12).log10();
        let gain = db_to_gain(over_db / self.ratio - over_db);
        sample * gain * self.makeup
    }
}

impl Dbx160 {
    pub fn new(threshold_db: f32, ratio: f32, sample_rate: f32) -> Self {
        let ratio = ratio.max(1.0);
        Self {
            threshold: db_to_gain(threshold_db),
            ratio,
            attack_alpha: python_time_alpha(0.0001, sample_rate),
            release_alpha: python_time_alpha(0.05, sample_rate),
            env: 0.0,
            makeup: db_to_gain(threshold_db * (1.0 - 1.0 / ratio) / 2.0),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        self.track_env(sample.abs(), self.attack_alpha, self.release_alpha);
        if self.env <= self.threshold {
            return sample * self.makeup;
        }
        let over_db = 20.0 * (self.env / self.threshold + 1e-12).log10();
        let gain = db_to_gain(over_db / self.ratio - over_db);
        sample * gain * self.makeup
    }

    fn track_env(&mut self, input: f32, attack_alpha: f32, release_alpha: f32) {
        let alpha = if input > self.env {
            attack_alpha
        } else {
            release_alpha
        };
        self.env = alpha * self.env + (1.0 - alpha) * input;
    }
}

#[derive(Clone, Debug)]
pub struct La2a {
    threshold: f32,
    ratio: f32,
    attack_alpha: f32,
    release_fast_alpha: f32,
    release_slow_alpha: f32,
    env: f32,
}

impl La2a {
    pub fn new(peak_reduction: f32, limit: bool, sample_rate: f32) -> Self {
        let threshold_db = -20.0 + (1.0 - peak_reduction.clamp(0.0, 1.0)) * 15.0;
        Self {
            threshold: db_to_gain(threshold_db),
            ratio: if limit { 10.0 } else { 3.0 },
            attack_alpha: python_time_alpha(0.01, sample_rate),
            release_fast_alpha: python_time_alpha(0.06, sample_rate),
            release_slow_alpha: python_time_alpha(1.0, sample_rate),
            env: 0.0,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let input = sample.abs();
        if input > self.env {
            self.env = self.attack_alpha * self.env + (1.0 - self.attack_alpha) * input;
        } else {
            let alpha = if self.env > self.threshold * 1.5 {
                self.release_fast_alpha
            } else {
                self.release_slow_alpha
            };
            self.env = alpha * self.env + (1.0 - alpha) * input;
        }

        if self.env <= self.threshold {
            return sample;
        }
        let over = self.env / self.threshold;
        let gain = 1.0 / (1.0 + (over - 1.0) * (self.ratio - 1.0) / self.ratio);
        sample * gain
    }
}

#[derive(Clone, Debug)]
pub struct Urei1176 {
    ratio: f32,
    input_gain: f32,
    attack_alpha: f32,
    release_alpha: f32,
    env: f32,
    all_buttons: bool,
}

impl Urei1176 {
    pub fn new(input_gain: f32, ratio: f32, attack: f32, release: f32, sample_rate: f32) -> Self {
        let ratio = ratio.max(1.0);
        let attack_ms = 0.02 + attack.clamp(0.0, 1.0) * 0.8;
        let release_ms = 50.0 + release.clamp(0.0, 1.0) * 1_050.0;
        Self {
            ratio,
            input_gain: input_gain.clamp(0.0, 1.0),
            attack_alpha: python_time_alpha(attack_ms / 1_000.0, sample_rate),
            release_alpha: python_time_alpha(release_ms / 1_000.0, sample_rate),
            env: 0.0,
            all_buttons: ratio >= 20.0,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let driven = sample * (1.0 + self.input_gain * 3.0);
        let input = driven.abs();
        let alpha = if input > self.env {
            self.attack_alpha
        } else {
            self.release_alpha
        };
        self.env = alpha * self.env + (1.0 - alpha) * input;

        let threshold = 0.25;
        let gain = if self.env > threshold {
            let over_db = 20.0 * (self.env / threshold + 1e-12).log10();
            db_to_gain(over_db / self.ratio - over_db)
        } else {
            1.0
        };
        let mut out = driven * gain;
        out += if self.all_buttons { 0.1 } else { 0.03 } * out * out * out;
        if self.all_buttons {
            out = (out * 2.0).tanh() * 0.7;
        }
        out.clamp(-1.0, 1.0)
    }
}

impl Compressor {
    pub fn new(
        threshold_db: f32,
        ratio: f32,
        attack: f32,
        release: f32,
        makeup_gain: f32,
        sample_rate: f32,
    ) -> Self {
        Self {
            threshold_db,
            ratio: ratio.max(1.0),
            makeup: db_to_gain(makeup_gain),
            attack_alpha: time_alpha(attack, sample_rate),
            release_alpha: time_alpha(release, sample_rate),
            env: 0.0,
        }
    }

    pub fn limiter(ceiling: f32, release: f32, sample_rate: f32) -> Self {
        Self::new(ceiling, 100.0, 0.001, release, 0.0, sample_rate)
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let input = sample.abs();
        let alpha = if input > self.env {
            self.attack_alpha
        } else {
            self.release_alpha
        };
        self.env = alpha * self.env + (1.0 - alpha) * input;
        let env_db = gain_to_db(self.env);
        let gain_db = if env_db > self.threshold_db {
            (self.threshold_db - env_db) * (1.0 - 1.0 / self.ratio)
        } else {
            0.0
        };
        sample * db_to_gain(gain_db) * self.makeup
    }
}

#[derive(Clone, Debug)]
pub struct NoiseGate {
    threshold: f32,
    attack_alpha: f32,
    release_alpha: f32,
    env: f32,
    gain_filter: Biquad,
}

impl NoiseGate {
    pub fn new(threshold_db: f32, attack: f32, release: f32, sample_rate: f32) -> Self {
        Self {
            threshold: db_to_gain(threshold_db),
            attack_alpha: time_alpha(attack, sample_rate),
            release_alpha: time_alpha(release, sample_rate),
            env: 0.0,
            gain_filter: Biquad::new(FilterKind::Lowpass, 200.0, 0.2, sample_rate),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let input = sample.abs();
        let alpha = if input > self.env {
            self.attack_alpha
        } else {
            self.release_alpha
        };
        self.env = alpha * self.env + (1.0 - alpha) * input;
        let target = if self.env > self.threshold { 1.0 } else { 0.0 };
        sample * self.gain_filter.process(target).clamp(0.0, 1.0)
    }
}

#[derive(Clone, Debug)]
pub struct TransientShaper {
    attack_gain: f32,
    sustain_gain: f32,
    fast_alpha: f32,
    slow_alpha: f32,
    fast: f32,
    slow: f32,
    transient_peak: f32,
    gain_peak: f32,
}

impl TransientShaper {
    pub fn new(attack_gain: f32, sustain_gain: f32, sensitivity: f32, sample_rate: f32) -> Self {
        Self {
            attack_gain: attack_gain.clamp(0.0, 8.0),
            sustain_gain: sustain_gain.clamp(0.0, 4.0),
            fast_alpha: time_alpha(sensitivity, sample_rate),
            slow_alpha: time_alpha(sensitivity * 10.0, sample_rate),
            fast: 0.0,
            slow: 0.0,
            transient_peak: 1.0e-9,
            gain_peak: 1.0e-9,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let input = sample.abs();
        self.fast = input.max(self.fast_alpha * self.fast);
        self.slow = input.max(self.slow_alpha * self.slow);
        let transient = (self.fast - self.slow).max(0.0);
        self.transient_peak = self.transient_peak.max(transient);
        let sustain_mask = 1.0 - transient / (self.transient_peak + 1e-9);
        let raw_gain =
            transient * self.attack_gain + sustain_mask.clamp(0.0, 1.0) * self.sustain_gain;
        self.gain_peak = self.gain_peak.max(raw_gain).max(1e-9);
        sample * (raw_gain / self.gain_peak).clamp(0.0, 2.0)
    }
}

fn time_alpha(seconds: f32, sample_rate: f32) -> f32 {
    (-1.0 / (seconds.max(0.000_1) * sample_rate)).exp()
}

fn python_time_alpha(seconds: f32, sample_rate: f32) -> f32 {
    (-1.0 / (seconds * sample_rate).max(1.0)).exp()
}

fn db_to_gain(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

fn gain_to_db(gain: f32) -> f32 {
    20.0 * (gain + 1e-9).log10()
}
