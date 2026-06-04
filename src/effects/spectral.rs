use std::f32::consts::TAU;

#[derive(Clone, Debug)]
pub struct RingMod {
    phase: f32,
    freq: f32,
    mix: f32,
}

impl RingMod {
    pub fn new(freq: f32, mix: f32) -> Self {
        Self {
            phase: 0.0,
            freq: freq.clamp(0.01, 20_000.0),
            mix: mix.clamp(0.0, 1.0),
        }
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let carrier = (self.phase * TAU).sin();
        self.phase = (self.phase + self.freq / sample_rate) % 1.0;
        (1.0 - self.mix) * sample + self.mix * sample * carrier
    }
}

#[derive(Clone, Debug)]
pub struct ArpRingMod {
    phase: f32,
    freq: f32,
    depth: f32,
    diode_curve: f32,
}

impl ArpRingMod {
    pub fn new(freq: f32, depth: f32, diode_curve: f32) -> Self {
        Self {
            phase: 0.0,
            freq: freq.clamp(0.01, 20_000.0),
            depth: depth.clamp(0.0, 1.0),
            diode_curve: diode_curve.clamp(0.0, 1.0),
        }
    }

    pub fn process(&mut self, sample: f32, sample_rate: f32) -> f32 {
        let carrier = (self.phase * TAU).sin();
        self.phase = (self.phase + self.freq / sample_rate) % 1.0;
        let carrier = if self.diode_curve > 0.0 {
            carrier.signum() * carrier.abs().powf(1.0 - self.diode_curve * 0.5)
        } else {
            carrier
        };
        sample * carrier * self.depth + sample * (1.0 - self.depth)
    }
}
