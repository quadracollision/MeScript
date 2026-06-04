use super::{EffectChain, EffectSpec};

#[derive(Clone, Debug)]
pub enum OfflineEffectSpec {
    Reverse {
        mix: f32,
    },
    TapeStop {
        duration_pct: f32,
    },
    Granular {
        grain_ms: f32,
        density: f32,
        spray: f32,
        pitch_spread: f32,
    },
    GranularStretch {
        rate: f32,
        grain_ms: f32,
    },
    SpectralFreeze {
        freeze_pos: f32,
        sustain: f32,
        mix: f32,
    },
    Haas {
        delay_ms: f32,
        side: StereoSide,
    },
    StereoWiden {
        width: f32,
    },
    StereoImager {
        width: f32,
        bass_mono_freq: f32,
    },
    WidthEnhance {
        low_width: f32,
        high_width: f32,
        crossover: f32,
    },
    FreqShift {
        shift_hz: f32,
        mix: f32,
    },
    AutoPan {
        rate: f32,
        depth: f32,
    },
    PingPongDelay {
        time: f32,
        feedback: f32,
        mix: f32,
    },
    Live(EffectSpec),
}

#[derive(Clone, Copy, Debug)]
pub enum StereoSide {
    Left,
    Right,
}

#[allow(dead_code)]
pub fn apply_chain(
    audio: Vec<f32>,
    effects: &[OfflineEffectSpec],
    sample_rate: f32,
) -> Vec<[f32; 2]> {
    let audio = audio
        .into_iter()
        .map(|sample| [sample, sample])
        .collect::<Vec<_>>();
    apply_chain_stereo(audio, effects, sample_rate)
}

pub fn apply_chain_stereo(
    mut audio: Vec<[f32; 2]>,
    effects: &[OfflineEffectSpec],
    sample_rate: f32,
) -> Vec<[f32; 2]> {
    for effect in effects {
        audio = match *effect {
            OfflineEffectSpec::Reverse { mix } => {
                map_channels(&audio, |channel| reverse(&channel, mix))
            }
            OfflineEffectSpec::TapeStop { duration_pct } => {
                map_channels(&audio, |channel| tape_stop(&channel, duration_pct))
            }
            OfflineEffectSpec::Granular {
                grain_ms,
                density,
                spray,
                pitch_spread,
            } => map_channels(&audio, |channel| {
                granular(
                    &channel,
                    grain_ms,
                    density,
                    spray,
                    pitch_spread,
                    sample_rate,
                )
            }),
            OfflineEffectSpec::GranularStretch { rate, grain_ms } => {
                map_channels(&audio, |channel| {
                    granular_stretch(&channel, rate, grain_ms, sample_rate)
                })
            }
            OfflineEffectSpec::SpectralFreeze {
                freeze_pos,
                sustain,
                mix,
            } => map_channels(&audio, |channel| {
                spectral_freeze(&channel, freeze_pos, sustain, mix)
            }),
            OfflineEffectSpec::Haas { delay_ms, side } => haas(&audio, delay_ms, side, sample_rate),
            OfflineEffectSpec::StereoWiden { width } => stereo_widen(&audio, width),
            OfflineEffectSpec::StereoImager {
                width,
                bass_mono_freq,
            } => stereo_imager(&audio, width, bass_mono_freq, sample_rate),
            OfflineEffectSpec::WidthEnhance {
                low_width,
                high_width,
                crossover,
            } => width_enhance(&audio, low_width, high_width, crossover, sample_rate),
            OfflineEffectSpec::FreqShift { shift_hz, mix } => map_channels(&audio, |channel| {
                freq_shift(&channel, shift_hz, mix, sample_rate)
            }),
            OfflineEffectSpec::AutoPan { rate, depth } => {
                auto_pan(&audio, rate, depth, sample_rate)
            }
            OfflineEffectSpec::PingPongDelay {
                time,
                feedback,
                mix,
            } => ping_pong_delay(&audio, time, feedback, mix, sample_rate),
            OfflineEffectSpec::Live(ref spec) => live_effect(&audio, spec, sample_rate),
        };
    }
    audio
}

fn map_channels<F>(audio: &[[f32; 2]], mut process: F) -> Vec<[f32; 2]>
where
    F: FnMut(Vec<f32>) -> Vec<f32>,
{
    let left = process(audio.iter().map(|frame| frame[0]).collect());
    let right = process(audio.iter().map(|frame| frame[1]).collect());
    left.into_iter()
        .zip(right)
        .map(|(left, right)| [left, right])
        .collect()
}

fn live_effect(audio: &[[f32; 2]], spec: &EffectSpec, sample_rate: f32) -> Vec<[f32; 2]> {
    let mut left = EffectChain::new(std::slice::from_ref(spec), sample_rate);
    let mut right = EffectChain::new(std::slice::from_ref(spec), sample_rate);
    audio
        .iter()
        .map(|frame| {
            [
                left.process(frame[0], sample_rate),
                right.process(frame[1], sample_rate),
            ]
        })
        .collect()
}

fn reverse(audio: &[f32], mix: f32) -> Vec<f32> {
    let mix = mix.clamp(0.0, 1.0);
    audio
        .iter()
        .zip(audio.iter().rev())
        .map(|(dry, wet)| (1.0 - mix) * dry + mix * wet)
        .collect()
}

fn tape_stop(audio: &[f32], duration_pct: f32) -> Vec<f32> {
    let n = audio.len();
    let duration_pct = duration_pct.clamp(0.1, 1.0);
    let stop_start = (n as f32 * (1.0 - duration_pct)) as usize;
    let stop_len = n.saturating_sub(stop_start);
    if stop_len < 2 {
        return audio.to_vec();
    }

    let mut out = audio.to_vec();
    let speed_sum = (0..stop_len)
        .map(|i| 1.0 - i as f32 / (stop_len - 1) as f32)
        .sum::<f32>()
        .max(1.0e-9);
    let mut read_offset = 0.0;
    for idx in stop_start..n {
        let t = (idx - stop_start) as f32 / (stop_len - 1) as f32;
        let speed = 1.0 - t;
        read_offset += speed;
        let read = stop_start as f32 + read_offset / speed_sum * (stop_len - 1) as f32;
        out[idx] = read_interp(audio, read) * (1.0 - t);
    }
    out
}

fn granular(
    audio: &[f32],
    grain_ms: f32,
    density: f32,
    spray: f32,
    pitch_spread: f32,
    sample_rate: f32,
) -> Vec<f32> {
    let n = audio.len();
    let grain = (grain_ms.clamp(5.0, 500.0) * sample_rate / 1_000.0)
        .floor()
        .max(64.0) as usize;
    if n < grain + 1 {
        return audio.to_vec();
    }

    let mut out = vec![0.0; n];
    let mut rng = 0x6d2b_79f5_u32;
    let count = ((n / grain).max(1) as f32 * density.clamp(0.0, 1.0) * 3.0).floor() as usize;
    for _ in 0..count {
        let src = (random01(&mut rng) * (n - grain) as f32) as usize;
        let spread = ((random01(&mut rng) - 0.5) * spray.clamp(0.0, 1.0) * n as f32) as isize;
        let dest = (src as isize + spread).clamp(0, (n - grain) as isize) as usize;
        let pitch = 1.0 + (random01(&mut rng) - 0.5) * pitch_spread.clamp(0.0, 1.0) * 2.0;
        let grain_len = if (pitch - 1.0).abs() > 0.01 {
            ((grain as f32 / pitch) as usize).clamp(2, grain * 4 - 1)
        } else {
            grain
        };
        for i in 0..grain_len {
            if dest + i >= n {
                break;
            }
            let src_pos = src as f32
                + if grain_len <= 1 {
                    0.0
                } else {
                    i as f32 * (grain - 1) as f32 / (grain_len - 1) as f32
                };
            out[dest + i] += read_interp(audio, src_pos) * hann(i, grain_len) * 0.3;
        }
    }

    audio
        .iter()
        .zip(out)
        .map(|(dry, wet)| dry * 0.6 + wet * 0.4)
        .collect()
}

#[cfg(test)]
pub(crate) fn granular_settings_for_test(
    len: usize,
    grain_ms: f32,
    density: f32,
    sample_rate: f32,
) -> (usize, usize) {
    let grain = (grain_ms.clamp(5.0, 500.0) * sample_rate / 1_000.0)
        .floor()
        .max(64.0) as usize;
    let count = ((len / grain).max(1) as f32 * density.clamp(0.0, 1.0) * 3.0).floor() as usize;
    (grain, count)
}

fn granular_stretch(audio: &[f32], rate: f32, grain_ms: f32, sample_rate: f32) -> Vec<f32> {
    let n = audio.len();
    let rate = rate.clamp(0.1, 4.0);
    let grain = ((grain_ms.clamp(10.0, 500.0) * sample_rate / 1_000.0) as usize).max(128);
    if n < grain + 1 {
        return audio.to_vec();
    }

    let hop_in = grain / 2;
    let hop_out = ((hop_in as f32) * rate) as usize;
    let hop_out = hop_out.max(1);
    let out_len = ((n as f32 / rate) as usize).max(n);
    let mut stretched = vec![0.0; out_len];
    let mut read = 0;
    let mut write = 0;
    while read + grain <= n && write + grain <= stretched.len() {
        for i in 0..grain {
            stretched[write + i] += audio[read + i] * hann(i, grain);
        }
        read += hop_in;
        write += hop_out;
    }
    stretched.truncate(n);
    stretched
}

fn spectral_freeze(audio: &[f32], freeze_pos: f32, sustain: f32, mix: f32) -> Vec<f32> {
    let n = audio.len();
    let frame = 2_048.min(n.max(1));
    if n < frame {
        return audio.to_vec();
    }
    let pos = ((freeze_pos.clamp(0.0, 1.0) * n as f32) as usize).min(n - frame);
    let sustain = sustain.clamp(0.0, 1.0);
    let mix = mix.clamp(0.0, 1.0);
    let hop = (frame / 4).max(1);

    let mut spectrum = vec![Complex::zero(); frame];
    for idx in 0..frame {
        spectrum[idx] = Complex::new(audio[pos + idx] * hann(idx, frame), 0.0);
    }
    fft(&mut spectrum, false);
    let magnitudes = spectrum
        .iter()
        .take(frame / 2 + 1)
        .map(|bin| bin.abs())
        .collect::<Vec<_>>();

    let mut frozen = vec![0.0; n];
    let mut rng = 0x9e37_79b9_u32;
    for start in (0..n.saturating_sub(frame)).step_by(hop) {
        let mut recon = vec![Complex::zero(); frame];
        for bin in 0..=frame / 2 {
            let phase = random01(&mut rng) * std::f32::consts::TAU;
            recon[bin] = Complex::from_polar(magnitudes[bin], phase);
            if bin > 0 && bin < frame / 2 {
                recon[frame - bin] = recon[bin].conj();
            }
        }
        fft(&mut recon, true);
        for idx in 0..frame {
            frozen[start + idx] += recon[idx].re * hann(idx, frame);
        }
    }

    normalize(&mut frozen);
    let denom = (sustain * n as f32).max(1.0);
    for (idx, sample) in frozen.iter_mut().enumerate() {
        let env = sustain + (1.0 - sustain) * (-(idx as f32) / denom).exp();
        *sample *= env;
    }

    audio
        .iter()
        .zip(frozen)
        .map(|(dry, wet)| (1.0 - mix) * dry + mix * wet)
        .collect()
}

#[derive(Clone, Copy, Debug)]
struct Complex {
    re: f32,
    im: f32,
}

impl Complex {
    fn new(re: f32, im: f32) -> Self {
        Self { re, im }
    }

    fn zero() -> Self {
        Self { re: 0.0, im: 0.0 }
    }

    fn from_polar(radius: f32, phase: f32) -> Self {
        Self {
            re: radius * phase.cos(),
            im: radius * phase.sin(),
        }
    }

    fn conj(self) -> Self {
        Self {
            re: self.re,
            im: -self.im,
        }
    }

    fn abs(self) -> f32 {
        (self.re * self.re + self.im * self.im).sqrt()
    }
}

fn fft(buffer: &mut [Complex], inverse: bool) {
    let n = buffer.len();
    debug_assert!(n.is_power_of_two());

    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            buffer.swap(i, j);
        }
    }

    let mut len = 2;
    while len <= n {
        let sign = if inverse { 1.0 } else { -1.0 };
        let angle = sign * std::f32::consts::TAU / len as f32;
        let w_len = Complex::from_polar(1.0, angle);
        for start in (0..n).step_by(len) {
            let mut w = Complex::new(1.0, 0.0);
            for offset in 0..len / 2 {
                let even = buffer[start + offset];
                let odd = complex_mul(buffer[start + offset + len / 2], w);
                buffer[start + offset] = complex_add(even, odd);
                buffer[start + offset + len / 2] = complex_sub(even, odd);
                w = complex_mul(w, w_len);
            }
        }
        len <<= 1;
    }

    if inverse {
        let scale = 1.0 / n as f32;
        for sample in buffer {
            sample.re *= scale;
            sample.im *= scale;
        }
    }
}

fn complex_add(a: Complex, b: Complex) -> Complex {
    Complex::new(a.re + b.re, a.im + b.im)
}

fn complex_sub(a: Complex, b: Complex) -> Complex {
    Complex::new(a.re - b.re, a.im - b.im)
}

fn complex_mul(a: Complex, b: Complex) -> Complex {
    Complex::new(a.re * b.re - a.im * b.im, a.re * b.im + a.im * b.re)
}

fn haas(audio: &[[f32; 2]], delay_ms: f32, side: StereoSide, sample_rate: f32) -> Vec<[f32; 2]> {
    let delay = (delay_ms.clamp(0.0, 100.0) * sample_rate / 1_000.0).floor() as usize;
    if delay == 0 || delay >= audio.len() {
        return audio.to_vec();
    }
    let mut out = audio.to_vec();
    let ch = match side {
        StereoSide::Left => 0,
        StereoSide::Right => 1,
    };
    for idx in (0..audio.len()).rev() {
        out[idx][ch] = if idx >= delay {
            audio[idx - delay][ch]
        } else {
            0.0
        };
    }
    out
}

fn stereo_widen(audio: &[[f32; 2]], width: f32) -> Vec<[f32; 2]> {
    let side_gain = width.clamp(0.0, 1.0) * 2.0;
    mid_side(audio, |_, side, _| side * side_gain)
}

fn stereo_imager(
    audio: &[[f32; 2]],
    width: f32,
    bass_mono_freq: f32,
    sample_rate: f32,
) -> Vec<[f32; 2]> {
    let mut side_lp = OnePoleLowpass::new(bass_mono_freq.max(20.0), sample_rate);
    mid_side(audio, |_, side, _| {
        let bass_side = side_lp.process(side);
        side * width - bass_side * 0.8
    })
}

fn width_enhance(
    audio: &[[f32; 2]],
    low_width: f32,
    high_width: f32,
    crossover: f32,
    sample_rate: f32,
) -> Vec<[f32; 2]> {
    let mut side_lp = OnePoleLowpass::new(crossover.max(20.0), sample_rate);
    mid_side(audio, |_, side, _| {
        let low = side_lp.process(side);
        let high = side - low;
        low * low_width + high * high_width
    })
}

fn mid_side<F>(audio: &[[f32; 2]], mut side_fn: F) -> Vec<[f32; 2]>
where
    F: FnMut(f32, f32, usize) -> f32,
{
    audio
        .iter()
        .enumerate()
        .map(|(idx, frame)| {
            let mid = (frame[0] + frame[1]) * 0.5;
            let side = (frame[0] - frame[1]) * 0.5;
            let side = side_fn(mid, side, idx);
            [mid + side, mid - side]
        })
        .collect()
}

fn freq_shift(audio: &[f32], shift_hz: f32, mix: f32, sample_rate: f32) -> Vec<f32> {
    let mix = mix.clamp(0.0, 1.0);
    let quadrature = hilbert_transform(audio);
    audio
        .iter()
        .zip(quadrature)
        .enumerate()
        .map(|(idx, (sample, hilbert))| {
            let phase = std::f32::consts::TAU * shift_hz * idx as f32 / sample_rate;
            let wet = sample * phase.cos() - hilbert * phase.sin();
            (1.0 - mix) * sample + mix * wet
        })
        .collect()
}

fn hilbert_transform(audio: &[f32]) -> Vec<f32> {
    const RADIUS: isize = 127;
    let mut out = vec![0.0; audio.len()];
    for idx in 0..audio.len() {
        let mut acc = 0.0;
        for tap in -RADIUS..=RADIUS {
            if tap == 0 || tap % 2 == 0 {
                continue;
            }
            let src = idx as isize - tap;
            if !(0..audio.len() as isize).contains(&src) {
                continue;
            }
            let normalized = tap as f32 / RADIUS as f32;
            let window = 0.54 + 0.46 * (std::f32::consts::PI * normalized).cos();
            acc += audio[src as usize] * (2.0 / (std::f32::consts::PI * tap as f32)) * window;
        }
        out[idx] = acc;
    }
    out
}

fn auto_pan(audio: &[[f32; 2]], rate: f32, depth: f32, sample_rate: f32) -> Vec<[f32; 2]> {
    let rate = rate.clamp(0.01, 40.0);
    let depth = depth.clamp(0.0, 1.0);
    audio
        .iter()
        .enumerate()
        .map(|(idx, frame)| {
            let lfo = (std::f32::consts::TAU * rate * idx as f32 / sample_rate).sin();
            let pan = (0.5 + depth * 0.5 * lfo).clamp(0.0, 1.0);
            [frame[0] * (1.0 - pan).sqrt(), frame[1] * pan.sqrt()]
        })
        .collect()
}

fn ping_pong_delay(
    audio: &[[f32; 2]],
    time: f32,
    feedback: f32,
    mix: f32,
    sample_rate: f32,
) -> Vec<[f32; 2]> {
    let mut delay = (time.max(0.0) * sample_rate) as usize;
    if delay < 1 {
        return audio.to_vec();
    }
    if delay >= audio.len() {
        delay = audio.len().saturating_sub(1);
    }
    if delay < 1 {
        return audio.to_vec();
    }

    let feedback = feedback.clamp(0.0, 0.95);
    let mix = mix.clamp(0.0, 1.0);
    let mut out = audio.to_vec();
    let mut buf_l = vec![0.0; delay];
    let mut buf_r = vec![0.0; delay];
    let mut pos = 0;

    for (idx, frame) in audio.iter().enumerate() {
        let echo_l = buf_r[pos];
        let echo_r = buf_l[pos];
        let out_l = frame[0] + echo_l * feedback;
        let out_r = frame[1] + echo_r * feedback;
        buf_l[pos] = out_l;
        buf_r[pos] = out_r;
        out[idx] = [
            (1.0 - mix) * frame[0] + mix * echo_l,
            (1.0 - mix) * frame[1] + mix * echo_r,
        ];
        pos = (pos + 1) % delay;
    }

    out
}

struct OnePoleLowpass {
    alpha: f32,
    state: f32,
}

impl OnePoleLowpass {
    fn new(cutoff: f32, sample_rate: f32) -> Self {
        let rc = 1.0 / (std::f32::consts::TAU * cutoff);
        let dt = 1.0 / sample_rate;
        Self {
            alpha: dt / (rc + dt),
            state: 0.0,
        }
    }

    fn process(&mut self, sample: f32) -> f32 {
        self.state += self.alpha * (sample - self.state);
        self.state
    }
}

fn read_interp(audio: &[f32], pos: f32) -> f32 {
    if audio.is_empty() {
        return 0.0;
    }
    let pos = pos.clamp(0.0, (audio.len() - 1) as f32);
    let idx = pos.floor() as usize;
    let next = (idx + 1).min(audio.len() - 1);
    let frac = pos - idx as f32;
    audio[idx] * (1.0 - frac) + audio[next] * frac
}

fn hann(idx: usize, len: usize) -> f32 {
    if len <= 1 {
        return 1.0;
    }
    0.5 - 0.5 * (std::f32::consts::TAU * idx as f32 / (len - 1) as f32).cos()
}

fn normalize(audio: &mut [f32]) {
    let peak = audio
        .iter()
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
    if peak > 1e-6 {
        for sample in audio {
            *sample /= peak;
        }
    }
}

fn random01(state: &mut u32) -> f32 {
    *state = state.wrapping_mul(1664525).wrapping_add(1013904223);
    (*state >> 8) as f32 / 16_777_216.0
}
