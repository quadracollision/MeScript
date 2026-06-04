#[derive(Clone, Debug)]
pub struct SchroederReverb {
    combs: Vec<FeedbackDelay>,
    allpasses: Vec<Allpass>,
    mix: f32,
}

impl SchroederReverb {
    pub fn new(decay: f32, mix: f32, sample_rate: f32) -> Self {
        let t60 = 0.5 + decay.clamp(0.0, 1.0) * 4.5;
        let combs = [29.7, 37.1, 41.1, 43.7]
            .into_iter()
            .map(|ms| {
                let delay = (ms * sample_rate / 1_000.0) as usize;
                let d_sec = delay as f32 / sample_rate;
                let gain = 10.0_f32.powf(-3.0 * d_sec / t60);
                FeedbackDelay::new(delay, gain)
            })
            .collect();
        let allpasses = [5.0, 1.7]
            .into_iter()
            .map(|ms| Allpass::new((ms * sample_rate / 1_000.0) as usize, 0.7))
            .collect();
        Self {
            combs,
            allpasses,
            mix: mix.clamp(0.0, 1.0),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let mut wet = self
            .combs
            .iter_mut()
            .map(|comb| comb.process(sample))
            .sum::<f32>();
        for allpass in &mut self.allpasses {
            wet = allpass.process(wet);
        }
        (1.0 - self.mix) * sample + self.mix * wet * 0.2
    }
}

#[derive(Clone, Debug)]
struct FeedbackDelay {
    buffer: Vec<f32>,
    pos: usize,
    feedback: f32,
}

impl FeedbackDelay {
    fn new(samples: usize, feedback: f32) -> Self {
        Self {
            buffer: vec![0.0; samples.max(1)],
            pos: 0,
            feedback,
        }
    }

    fn process(&mut self, sample: f32) -> f32 {
        let delayed = self.buffer[self.pos];
        let out = sample + delayed * self.feedback;
        self.buffer[self.pos] = out;
        self.pos = (self.pos + 1) % self.buffer.len();
        out
    }
}

#[derive(Clone, Debug)]
struct Allpass {
    buffer: Vec<f32>,
    pos: usize,
    gain: f32,
}

impl Allpass {
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
}
