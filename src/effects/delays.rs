#[derive(Clone, Debug)]
pub struct Delay {
    buffer: Vec<f32>,
    pos: usize,
    feedback: f32,
    mix: f32,
}

impl Delay {
    pub fn new(time: f32, feedback: f32, mix: f32, sample_rate: f32) -> Self {
        let samples = (time.max(0.0) * sample_rate) as usize;
        Self {
            buffer: vec![0.0; samples],
            pos: 0,
            feedback: feedback.clamp(0.0, 0.95),
            mix: mix.clamp(0.0, 1.0),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        if self.buffer.is_empty() {
            return sample;
        }
        let delayed = self.buffer[self.pos];
        let wet = sample + delayed * self.feedback;
        self.buffer[self.pos] = wet;
        self.pos = (self.pos + 1) % self.buffer.len();
        (1.0 - self.mix) * sample + self.mix * wet
    }
}
