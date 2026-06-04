use super::soft_clip;

#[derive(Clone, Copy, Debug)]
pub enum DistortionKind {
    Tanh,
    HardClip,
    SoftClip,
    SineFold,
    Rectify,
    HalfRectify,
    Waveshape,
}

#[derive(Clone, Debug)]
pub struct Distortion {
    kind: DistortionKind,
    drive: f32,
    peak: f32,
}

impl Distortion {
    pub fn new(kind: DistortionKind, drive: f32) -> Self {
        Self {
            kind,
            drive: drive.clamp(0.0, 10.0),
            peak: 1e-9,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let x = sample * (1.0 + self.drive * 20.0);
        let out = match self.kind {
            DistortionKind::Tanh => x.tanh(),
            DistortionKind::HardClip => x.clamp(-1.0, 1.0),
            DistortionKind::SoftClip => soft_clip(x),
            DistortionKind::SineFold => x.sin(),
            DistortionKind::Rectify => self.normalize_rectified(x.abs()),
            DistortionKind::HalfRectify => self.normalize_rectified(x.max(0.0)),
            DistortionKind::Waveshape => (1.5 * x - 0.5 * x.powi(3)).clamp(-1.0, 1.0),
        };
        out / (1.0 + self.drive * 2.0)
    }

    fn normalize_rectified(&mut self, sample: f32) -> f32 {
        self.peak = self.peak.max(sample.abs());
        sample / self.peak
    }
}

#[derive(Clone, Debug)]
pub struct Bitcrush {
    levels: f32,
    hold: usize,
    counter: usize,
    held: f32,
}

impl Bitcrush {
    pub fn new(bit_depth: f32, sample_rate_reduction: f32) -> Self {
        Self {
            levels: 2.0_f32.powi(bit_depth.clamp(2.0, 16.0) as i32),
            hold: sample_rate_reduction.clamp(1.0, 128.0) as usize,
            counter: 0,
            held: 0.0,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        if self.counter == 0 {
            self.held = (sample * self.levels).round() / self.levels;
        }
        self.counter = (self.counter + 1) % self.hold.max(1);
        self.held
    }
}
