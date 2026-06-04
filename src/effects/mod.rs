pub mod analog;
pub mod creative;
pub mod delays;
pub mod distortions;
pub mod dynamics;
pub mod filters;
pub mod hardware;
pub mod modulations;
pub mod offline;
pub mod reverb;
pub mod spectral;

use std::f32::consts::TAU;

pub use distortions::DistortionKind;
pub use filters::FilterKind;

#[derive(Clone, Debug)]
pub enum EffectSpec {
    Filter {
        kind: FilterKind,
        cutoff: f32,
        resonance: f32,
        gain_db: f32,
    },
    Comb {
        delay_ms: f32,
        feedback: f32,
        mix: f32,
    },
    Formant {
        vowel: filters::Vowel,
        mix: f32,
    },
    Distortion {
        kind: DistortionKind,
        drive: f32,
    },
    Bitcrush {
        bit_depth: f32,
        sample_rate_reduction: f32,
    },
    Delay {
        time: f32,
        feedback: f32,
        mix: f32,
    },
    Wavefolder {
        folds: f32,
        gain: f32,
        symmetry: f32,
    },
    Resonator {
        freq: f32,
        decay: f32,
        mix: f32,
        harmonics: f32,
    },
    Lofi {
        amount: f32,
    },
    Vinyl {
        crackle: f32,
        hiss: f32,
        wow: f32,
    },
    SubBass {
        mix: f32,
    },
    Sidechain {
        rate: f32,
        depth: f32,
        shape: f32,
    },
    Radio {
        intensity: f32,
    },
    Telephone {
        quality: f32,
    },
    Underwater {
        depth: f32,
    },
    Crystal {
        brightness: f32,
        decay: f32,
    },
    DcRemove,
    PitchShift {
        semitones: f32,
        mix: f32,
    },
    Harmonizer {
        interval: f32,
        mix: f32,
    },
    Octaver {
        octave_up: f32,
        octave_down: f32,
    },
    Shimmer {
        shift_semitones: f32,
        feedback: f32,
        mix: f32,
    },
    Stutter {
        grain_size_ms: f32,
        repeats: f32,
        mix: f32,
    },
    Glitch {
        density: f32,
        slice_ms: f32,
    },
    Fade {
        fade_in_ms: f32,
        fade_out_ms: f32,
        duration_seconds: Option<f32>,
    },
    Adsr {
        attack: f32,
        decay: f32,
        sustain: f32,
        release: f32,
        duration_seconds: Option<f32>,
    },
    Doppler {
        speed: f32,
        depth: f32,
    },
    Maximizer {
        ceiling: f32,
        warmth: f32,
        release_ms: f32,
    },
    MultibandComp {
        low_thresh: f32,
        mid_thresh: f32,
        high_thresh: f32,
        crossover_low: f32,
        crossover_high: f32,
    },
    HarmonicEnhance {
        low_harmonics: f32,
        high_harmonics: f32,
        air: f32,
    },
    Body {
        size: f32,
        tone: f32,
        mix: f32,
    },
    Warmth {
        amount: f32,
    },
    Spatial {
        room_size: f32,
        position: f32,
        height: f32,
    },
    ParallelComp {
        threshold: f32,
        ratio: f32,
        mix: f32,
    },
    Tremolo {
        rate: f32,
        depth: f32,
    },
    Chorus {
        rate: f32,
        depth: f32,
        voices: f32,
        mix: f32,
    },
    Ensemble {
        voices: f32,
        depth: f32,
        rate: f32,
    },
    Dimension {
        mode: f32,
    },
    Ce1Chorus {
        rate: f32,
        intensity: f32,
    },
    Re301Chorus {
        rate: f32,
        depth: f32,
        tone: f32,
    },
    DimensionD {
        mode: f32,
    },
    H3000 {
        detune_cents: f32,
        delay_ms: f32,
        feedback: f32,
        mix: f32,
    },
    Flanger {
        rate: f32,
        depth: f32,
        feedback: f32,
        mix: f32,
    },
    Phaser {
        rate: f32,
        depth: f32,
        stages: f32,
        mix: f32,
    },
    SmallStone {
        rate: f32,
        depth: f32,
        feedback: f32,
        color: bool,
    },
    Vibrato {
        rate: f32,
        depth: f32,
    },
    RingMod {
        freq: f32,
        mix: f32,
    },
    ArpRingMod {
        freq: f32,
        depth: f32,
        diode_curve: f32,
    },
    Compressor {
        threshold: f32,
        ratio: f32,
        attack: f32,
        release: f32,
        makeup_gain: f32,
    },
    Fairchild {
        input_gain: f32,
        threshold: f32,
        time_constant: f32,
        mix: f32,
    },
    SslComp {
        threshold: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        makeup_db: f32,
    },
    Dbx160 {
        threshold: f32,
        ratio: f32,
    },
    La2a {
        peak_reduction: f32,
        limit: bool,
    },
    Urei1176 {
        input_gain: f32,
        ratio: f32,
        attack: f32,
        release: f32,
    },
    Limiter {
        ceiling: f32,
        release: f32,
    },
    Gate {
        threshold: f32,
        attack: f32,
        release: f32,
    },
    Transient {
        attack_gain: f32,
        sustain_gain: f32,
        sensitivity: f32,
    },
    Reverb {
        decay: f32,
        mix: f32,
    },
    SpringReverb {
        decay: f32,
        tone: f32,
        mix: f32,
        drip: f32,
    },
    EmtPlate {
        decay: f32,
        damping: f32,
        mix: f32,
        pre_delay_ms: f32,
    },
    Lexicon224 {
        size: f32,
        decay: f32,
        damping: f32,
        pre_delay_ms: f32,
        mix: f32,
    },
    AmsReverb {
        decay: f32,
        damping: f32,
        program: hardware::AmsProgram,
        mix: f32,
    },
    Tube {
        drive: f32,
        asymmetry: f32,
    },
    NevePreamp {
        gain: f32,
        warmth: f32,
    },
    MarshallAmp {
        gain: f32,
        tone: f32,
        presence: f32,
    },
    VoxAc30 {
        gain: f32,
        treble: f32,
        cut: f32,
    },
    FenderTwin {
        volume: f32,
        treble: f32,
        bass: f32,
        reverb_mix: f32,
    },
    PultecEq {
        low_boost: f32,
        low_atten: f32,
        low_freq: f32,
        high_boost: f32,
        high_atten: f32,
        high_freq: f32,
    },
    Tc2290 {
        time_ms: f32,
        feedback: f32,
        mod_rate: f32,
        mod_depth: f32,
        mix: f32,
    },
    Exciter {
        amount: f32,
        cutoff: f32,
    },
    Tape {
        saturation: f32,
        wow: f32,
        flutter: f32,
    },
    StuderTape {
        input_level: f32,
        speed: f32,
        bias: f32,
    },
    Moog {
        cutoff: f32,
        resonance: f32,
        drive: f32,
    },
    ProphetFilter {
        cutoff: f32,
        resonance: f32,
    },
    ObxaFilter {
        cutoff: f32,
        resonance: f32,
        kind: FilterKind,
    },
    WaspFilter {
        cutoff: f32,
        resonance: f32,
    },
    Diode303 {
        cutoff: f32,
        resonance: f32,
        env_mod: f32,
        accent: f32,
        decay: f32,
    },
    SpaceEcho {
        time: f32,
        feedback: f32,
        wow: f32,
        flutter: f32,
        tone: f32,
        spring_mix: f32,
        mix: f32,
    },
    Sem {
        cutoff: f32,
        resonance: f32,
        kind: FilterKind,
    },
    Ms20 {
        cutoff: f32,
        resonance: f32,
    },
    JunoHpf {
        cutoff: f32,
        resonance: f32,
    },
    BuchlaLpg {
        strike: f32,
        decay: f32,
        resonance: f32,
    },
}

#[derive(Clone, Debug)]
pub struct EffectChain {
    effects: Vec<EffectNode>,
    tail_seconds: f32,
}

#[derive(Clone, Debug)]
enum EffectNode {
    Biquad(filters::Biquad),
    Comb(filters::Comb),
    Formant(filters::Formant),
    Distortion(distortions::Distortion),
    Bitcrush(distortions::Bitcrush),
    Delay(delays::Delay),
    Wavefolder(creative::Wavefolder),
    Resonator(creative::Resonator),
    Lofi(creative::Lofi),
    Vinyl(creative::Vinyl),
    SubBass(creative::SubBass),
    Sidechain(creative::Sidechain),
    Radio(creative::Radio),
    Telephone(creative::Telephone),
    Underwater(creative::Underwater),
    Crystal(creative::Crystal),
    DcRemove(creative::DcRemove),
    PitchShift(creative::PitchShift),
    Harmonizer(creative::Harmonizer),
    Octaver(creative::Octaver),
    Shimmer(creative::Shimmer),
    Stutter(creative::Stutter),
    Glitch(creative::Glitch),
    Fade(creative::Fade),
    Adsr(creative::Adsr),
    Doppler(creative::Doppler),
    Maximizer(creative::Maximizer),
    MultibandComp(creative::MultibandComp),
    HarmonicEnhance(creative::HarmonicEnhance),
    Body(creative::Body),
    Warmth(creative::Warmth),
    Spatial(creative::Spatial),
    ParallelComp(creative::ParallelComp),
    Tremolo(modulations::Tremolo),
    Chorus(modulations::Chorus),
    Ensemble(modulations::Ensemble),
    Dimension(modulations::Dimension),
    Ce1Chorus(modulations::Ce1Chorus),
    Re301Chorus(modulations::Re301Chorus),
    DimensionD(modulations::DimensionD),
    H3000(modulations::H3000),
    Flanger(modulations::Flanger),
    Phaser(modulations::Phaser),
    SmallStone(modulations::SmallStone),
    Vibrato(modulations::Vibrato),
    RingMod(spectral::RingMod),
    ArpRingMod(spectral::ArpRingMod),
    Compressor(dynamics::Compressor),
    Fairchild(dynamics::Fairchild),
    SslComp(dynamics::SslComp),
    Dbx160(dynamics::Dbx160),
    La2a(dynamics::La2a),
    Urei1176(dynamics::Urei1176),
    Gate(dynamics::NoiseGate),
    Transient(dynamics::TransientShaper),
    Reverb(reverb::SchroederReverb),
    SpringReverb(hardware::SpringReverb),
    EmtPlate(hardware::EmtPlate),
    Lexicon224(hardware::Lexicon224),
    AmsReverb(hardware::AmsReverb),
    Tube(analog::TubeSaturation),
    NevePreamp(hardware::NevePreamp),
    MarshallAmp(hardware::MarshallAmp),
    VoxAc30(hardware::VoxAc30),
    FenderTwin(hardware::FenderTwin),
    PultecEq(hardware::PultecEq),
    Tc2290(hardware::Tc2290),
    Exciter(analog::Exciter),
    Tape(analog::Tape),
    StuderTape(analog::StuderTape),
    Moog(hardware::MoogLadder),
    ProphetFilter(hardware::ProphetFilter),
    ObxaFilter(hardware::ObxaFilter),
    WaspFilter(hardware::WaspFilter),
    Diode303(hardware::Diode303),
    SpaceEcho(hardware::SpaceEcho),
    Sem(hardware::SemFilter),
    Ms20(hardware::Ms20Filter),
    JunoHpf(hardware::JunoHpf),
    BuchlaLpg(hardware::BuchlaLpg),
}

impl EffectChain {
    pub fn new(specs: &[EffectSpec], sample_rate: f32) -> Self {
        Self::new_with_duration(specs, sample_rate, None)
    }

    pub fn new_with_duration(
        specs: &[EffectSpec],
        sample_rate: f32,
        voice_duration_seconds: Option<f32>,
    ) -> Self {
        let effects = specs
            .iter()
            .map(|spec| match *spec {
                EffectSpec::Filter {
                    kind,
                    cutoff,
                    resonance,
                    gain_db,
                } => EffectNode::Biquad(filters::Biquad::new_with_gain(
                    kind,
                    cutoff,
                    resonance,
                    gain_db,
                    sample_rate,
                )),
                EffectSpec::Comb {
                    delay_ms,
                    feedback,
                    mix,
                } => EffectNode::Comb(filters::Comb::new(delay_ms, feedback, mix, sample_rate)),
                EffectSpec::Formant { vowel, mix } => {
                    EffectNode::Formant(filters::Formant::new(vowel, mix, sample_rate))
                }
                EffectSpec::Distortion { kind, drive } => {
                    EffectNode::Distortion(distortions::Distortion::new(kind, drive))
                }
                EffectSpec::Bitcrush {
                    bit_depth,
                    sample_rate_reduction,
                } => EffectNode::Bitcrush(distortions::Bitcrush::new(
                    bit_depth,
                    sample_rate_reduction,
                )),
                EffectSpec::Delay {
                    time,
                    feedback,
                    mix,
                } => EffectNode::Delay(delays::Delay::new(time, feedback, mix, sample_rate)),
                EffectSpec::Wavefolder {
                    folds,
                    gain,
                    symmetry,
                } => EffectNode::Wavefolder(creative::Wavefolder::new(folds, gain, symmetry)),
                EffectSpec::Resonator {
                    freq,
                    decay,
                    mix,
                    harmonics,
                } => EffectNode::Resonator(creative::Resonator::new(
                    freq,
                    decay,
                    mix,
                    harmonics,
                    sample_rate,
                )),
                EffectSpec::Lofi { amount } => {
                    EffectNode::Lofi(creative::Lofi::new(amount, sample_rate))
                }
                EffectSpec::Vinyl { crackle, hiss, wow } => {
                    EffectNode::Vinyl(creative::Vinyl::new(crackle, hiss, wow, sample_rate))
                }
                EffectSpec::SubBass { mix } => {
                    EffectNode::SubBass(creative::SubBass::new(mix, sample_rate))
                }
                EffectSpec::Sidechain { rate, depth, shape } => {
                    EffectNode::Sidechain(creative::Sidechain::new(rate, depth, shape))
                }
                EffectSpec::Radio { intensity } => {
                    EffectNode::Radio(creative::Radio::new(intensity, sample_rate))
                }
                EffectSpec::Telephone { quality } => {
                    EffectNode::Telephone(creative::Telephone::new(quality, sample_rate))
                }
                EffectSpec::Underwater { depth } => {
                    EffectNode::Underwater(creative::Underwater::new(depth, sample_rate))
                }
                EffectSpec::Crystal { brightness, decay } => {
                    EffectNode::Crystal(creative::Crystal::new(brightness, decay, sample_rate))
                }
                EffectSpec::DcRemove => EffectNode::DcRemove(creative::DcRemove::new(sample_rate)),
                EffectSpec::PitchShift { semitones, mix } => {
                    EffectNode::PitchShift(creative::PitchShift::new(semitones, mix, sample_rate))
                }
                EffectSpec::Harmonizer { interval, mix } => {
                    EffectNode::Harmonizer(creative::Harmonizer::new(interval, mix, sample_rate))
                }
                EffectSpec::Octaver {
                    octave_up,
                    octave_down,
                } => {
                    EffectNode::Octaver(creative::Octaver::new(octave_up, octave_down, sample_rate))
                }
                EffectSpec::Shimmer {
                    shift_semitones,
                    feedback,
                    mix,
                } => EffectNode::Shimmer(creative::Shimmer::new(
                    shift_semitones,
                    feedback,
                    mix,
                    sample_rate,
                )),
                EffectSpec::Stutter {
                    grain_size_ms,
                    repeats,
                    mix,
                } => EffectNode::Stutter(creative::Stutter::new(
                    grain_size_ms,
                    repeats,
                    mix,
                    sample_rate,
                )),
                EffectSpec::Glitch { density, slice_ms } => {
                    EffectNode::Glitch(creative::Glitch::new(density, slice_ms, sample_rate))
                }
                EffectSpec::Fade {
                    fade_in_ms,
                    fade_out_ms,
                    duration_seconds,
                } => EffectNode::Fade(creative::Fade::new(
                    fade_in_ms,
                    fade_out_ms,
                    duration_seconds.or(voice_duration_seconds).unwrap_or(1.0),
                    sample_rate,
                )),
                EffectSpec::Adsr {
                    attack,
                    decay,
                    sustain,
                    release,
                    duration_seconds,
                } => EffectNode::Adsr(creative::Adsr::new(
                    attack,
                    decay,
                    sustain,
                    release,
                    duration_seconds.or(voice_duration_seconds).unwrap_or(1.0),
                    sample_rate,
                )),
                EffectSpec::Doppler { speed, depth } => {
                    EffectNode::Doppler(creative::Doppler::new(speed, depth, sample_rate))
                }
                EffectSpec::Maximizer {
                    ceiling,
                    warmth,
                    release_ms,
                } => EffectNode::Maximizer(creative::Maximizer::new(
                    ceiling,
                    warmth,
                    release_ms,
                    sample_rate,
                )),
                EffectSpec::MultibandComp {
                    low_thresh,
                    mid_thresh,
                    high_thresh,
                    crossover_low,
                    crossover_high,
                } => EffectNode::MultibandComp(creative::MultibandComp::new(
                    low_thresh,
                    mid_thresh,
                    high_thresh,
                    crossover_low,
                    crossover_high,
                    sample_rate,
                )),
                EffectSpec::HarmonicEnhance {
                    low_harmonics,
                    high_harmonics,
                    air,
                } => EffectNode::HarmonicEnhance(creative::HarmonicEnhance::new(
                    low_harmonics,
                    high_harmonics,
                    air,
                    sample_rate,
                )),
                EffectSpec::Body { size, tone, mix } => {
                    EffectNode::Body(creative::Body::new(size, tone, mix, sample_rate))
                }
                EffectSpec::Warmth { amount } => {
                    EffectNode::Warmth(creative::Warmth::new(amount, sample_rate))
                }
                EffectSpec::Spatial {
                    room_size,
                    position,
                    height,
                } => EffectNode::Spatial(creative::Spatial::new(
                    room_size,
                    position,
                    height,
                    sample_rate,
                )),
                EffectSpec::ParallelComp {
                    threshold,
                    ratio,
                    mix,
                } => EffectNode::ParallelComp(creative::ParallelComp::new(
                    threshold,
                    ratio,
                    mix,
                    sample_rate,
                )),
                EffectSpec::Tremolo { rate, depth } => {
                    EffectNode::Tremolo(modulations::Tremolo::new(rate, depth))
                }
                EffectSpec::Chorus {
                    rate,
                    depth,
                    voices,
                    mix,
                } => EffectNode::Chorus(modulations::Chorus::new(
                    rate,
                    depth,
                    voices,
                    mix,
                    sample_rate,
                )),
                EffectSpec::Ensemble {
                    voices,
                    depth,
                    rate,
                } => EffectNode::Ensemble(modulations::Ensemble::new(
                    voices,
                    depth,
                    rate,
                    sample_rate,
                )),
                EffectSpec::Dimension { mode } => {
                    EffectNode::Dimension(modulations::Dimension::new(mode, sample_rate))
                }
                EffectSpec::Ce1Chorus { rate, intensity } => {
                    EffectNode::Ce1Chorus(modulations::Ce1Chorus::new(rate, intensity, sample_rate))
                }
                EffectSpec::Re301Chorus { rate, depth, tone } => EffectNode::Re301Chorus(
                    modulations::Re301Chorus::new(rate, depth, tone, sample_rate),
                ),
                EffectSpec::DimensionD { mode } => {
                    EffectNode::DimensionD(modulations::DimensionD::new(mode, sample_rate))
                }
                EffectSpec::H3000 {
                    detune_cents,
                    delay_ms,
                    feedback,
                    mix,
                } => EffectNode::H3000(modulations::H3000::new(
                    detune_cents,
                    delay_ms,
                    feedback,
                    mix,
                    sample_rate,
                )),
                EffectSpec::Flanger {
                    rate,
                    depth,
                    feedback,
                    mix,
                } => EffectNode::Flanger(modulations::Flanger::new(
                    rate,
                    depth,
                    feedback,
                    mix,
                    sample_rate,
                )),
                EffectSpec::Phaser {
                    rate,
                    depth,
                    stages,
                    mix,
                } => EffectNode::Phaser(modulations::Phaser::new(rate, depth, stages, mix)),
                EffectSpec::SmallStone {
                    rate,
                    depth,
                    feedback,
                    color,
                } => EffectNode::SmallStone(modulations::SmallStone::new(
                    rate, depth, feedback, color,
                )),
                EffectSpec::Vibrato { rate, depth } => {
                    EffectNode::Vibrato(modulations::Vibrato::new(rate, depth, sample_rate))
                }
                EffectSpec::RingMod { freq, mix } => {
                    EffectNode::RingMod(spectral::RingMod::new(freq, mix))
                }
                EffectSpec::ArpRingMod {
                    freq,
                    depth,
                    diode_curve,
                } => EffectNode::ArpRingMod(spectral::ArpRingMod::new(freq, depth, diode_curve)),
                EffectSpec::Compressor {
                    threshold,
                    ratio,
                    attack,
                    release,
                    makeup_gain,
                } => EffectNode::Compressor(dynamics::Compressor::new(
                    threshold,
                    ratio,
                    attack,
                    release,
                    makeup_gain,
                    sample_rate,
                )),
                EffectSpec::Fairchild {
                    input_gain,
                    threshold,
                    time_constant,
                    mix,
                } => EffectNode::Fairchild(dynamics::Fairchild::new(
                    input_gain,
                    threshold,
                    time_constant,
                    mix,
                    sample_rate,
                )),
                EffectSpec::SslComp {
                    threshold,
                    ratio,
                    attack_ms,
                    release_ms,
                    makeup_db,
                } => EffectNode::SslComp(dynamics::SslComp::new(
                    threshold,
                    ratio,
                    attack_ms,
                    release_ms,
                    makeup_db,
                    sample_rate,
                )),
                EffectSpec::Dbx160 { threshold, ratio } => {
                    EffectNode::Dbx160(dynamics::Dbx160::new(threshold, ratio, sample_rate))
                }
                EffectSpec::La2a {
                    peak_reduction,
                    limit,
                } => EffectNode::La2a(dynamics::La2a::new(peak_reduction, limit, sample_rate)),
                EffectSpec::Urei1176 {
                    input_gain,
                    ratio,
                    attack,
                    release,
                } => EffectNode::Urei1176(dynamics::Urei1176::new(
                    input_gain,
                    ratio,
                    attack,
                    release,
                    sample_rate,
                )),
                EffectSpec::Limiter { ceiling, release } => EffectNode::Compressor(
                    dynamics::Compressor::limiter(ceiling, release, sample_rate),
                ),
                EffectSpec::Gate {
                    threshold,
                    attack,
                    release,
                } => EffectNode::Gate(dynamics::NoiseGate::new(
                    threshold,
                    attack,
                    release,
                    sample_rate,
                )),
                EffectSpec::Transient {
                    attack_gain,
                    sustain_gain,
                    sensitivity,
                } => EffectNode::Transient(dynamics::TransientShaper::new(
                    attack_gain,
                    sustain_gain,
                    sensitivity,
                    sample_rate,
                )),
                EffectSpec::Reverb { decay, mix } => {
                    EffectNode::Reverb(reverb::SchroederReverb::new(decay, mix, sample_rate))
                }
                EffectSpec::SpringReverb {
                    decay,
                    tone,
                    mix,
                    drip,
                } => EffectNode::SpringReverb(hardware::SpringReverb::new(
                    decay,
                    tone,
                    mix,
                    drip,
                    sample_rate,
                )),
                EffectSpec::EmtPlate {
                    decay,
                    damping,
                    mix,
                    pre_delay_ms,
                } => EffectNode::EmtPlate(hardware::EmtPlate::new(
                    decay,
                    damping,
                    mix,
                    pre_delay_ms,
                    sample_rate,
                )),
                EffectSpec::Lexicon224 {
                    size,
                    decay,
                    damping,
                    pre_delay_ms,
                    mix,
                } => EffectNode::Lexicon224(hardware::Lexicon224::new(
                    size,
                    decay,
                    damping,
                    pre_delay_ms,
                    mix,
                    sample_rate,
                )),
                EffectSpec::AmsReverb {
                    decay,
                    damping,
                    program,
                    mix,
                } => EffectNode::AmsReverb(hardware::AmsReverb::new(
                    decay,
                    damping,
                    program,
                    mix,
                    sample_rate,
                )),
                EffectSpec::Tube { drive, asymmetry } => {
                    EffectNode::Tube(analog::TubeSaturation::new(drive, asymmetry))
                }
                EffectSpec::NevePreamp { gain, warmth } => {
                    EffectNode::NevePreamp(hardware::NevePreamp::new(gain, warmth, sample_rate))
                }
                EffectSpec::MarshallAmp {
                    gain,
                    tone,
                    presence,
                } => EffectNode::MarshallAmp(hardware::MarshallAmp::new(
                    gain,
                    tone,
                    presence,
                    sample_rate,
                )),
                EffectSpec::VoxAc30 { gain, treble, cut } => {
                    EffectNode::VoxAc30(hardware::VoxAc30::new(gain, treble, cut, sample_rate))
                }
                EffectSpec::FenderTwin {
                    volume,
                    treble,
                    bass,
                    reverb_mix,
                } => EffectNode::FenderTwin(hardware::FenderTwin::new(
                    volume,
                    treble,
                    bass,
                    reverb_mix,
                    sample_rate,
                )),
                EffectSpec::PultecEq {
                    low_boost,
                    low_atten,
                    low_freq,
                    high_boost,
                    high_atten,
                    high_freq,
                } => EffectNode::PultecEq(hardware::PultecEq::new(
                    low_boost,
                    low_atten,
                    low_freq,
                    high_boost,
                    high_atten,
                    high_freq,
                    sample_rate,
                )),
                EffectSpec::Tc2290 {
                    time_ms,
                    feedback,
                    mod_rate,
                    mod_depth,
                    mix,
                } => EffectNode::Tc2290(hardware::Tc2290::new(
                    time_ms,
                    feedback,
                    mod_rate,
                    mod_depth,
                    mix,
                    sample_rate,
                )),
                EffectSpec::Exciter { amount, cutoff } => {
                    EffectNode::Exciter(analog::Exciter::new(amount, cutoff, sample_rate))
                }
                EffectSpec::Tape {
                    saturation,
                    wow,
                    flutter,
                } => EffectNode::Tape(analog::Tape::new(saturation, wow, flutter, sample_rate)),
                EffectSpec::StuderTape {
                    input_level,
                    speed,
                    bias,
                } => EffectNode::StuderTape(analog::StuderTape::new(
                    input_level,
                    speed,
                    bias,
                    sample_rate,
                )),
                EffectSpec::Moog {
                    cutoff,
                    resonance,
                    drive,
                } => EffectNode::Moog(hardware::MoogLadder::new(
                    cutoff,
                    resonance,
                    drive,
                    sample_rate,
                )),
                EffectSpec::ProphetFilter { cutoff, resonance } => EffectNode::ProphetFilter(
                    hardware::ProphetFilter::new(cutoff, resonance, sample_rate),
                ),
                EffectSpec::ObxaFilter {
                    cutoff,
                    resonance,
                    kind,
                } => EffectNode::ObxaFilter(hardware::ObxaFilter::new(
                    cutoff,
                    resonance,
                    kind,
                    sample_rate,
                )),
                EffectSpec::WaspFilter { cutoff, resonance } => EffectNode::WaspFilter(
                    hardware::WaspFilter::new(cutoff, resonance, sample_rate),
                ),
                EffectSpec::Diode303 {
                    cutoff,
                    resonance,
                    env_mod,
                    accent,
                    decay,
                } => EffectNode::Diode303(hardware::Diode303::new(
                    cutoff,
                    resonance,
                    env_mod,
                    accent,
                    decay,
                    sample_rate,
                )),
                EffectSpec::SpaceEcho {
                    time,
                    feedback,
                    wow,
                    flutter,
                    tone,
                    spring_mix,
                    mix,
                } => EffectNode::SpaceEcho(hardware::SpaceEcho::new(
                    time,
                    feedback,
                    wow,
                    flutter,
                    tone,
                    spring_mix,
                    mix,
                    sample_rate,
                )),
                EffectSpec::Sem {
                    cutoff,
                    resonance,
                    kind,
                } => EffectNode::Sem(hardware::SemFilter::new(
                    cutoff,
                    resonance,
                    kind,
                    sample_rate,
                )),
                EffectSpec::Ms20 { cutoff, resonance } => {
                    EffectNode::Ms20(hardware::Ms20Filter::new(cutoff, resonance, sample_rate))
                }
                EffectSpec::JunoHpf { cutoff, resonance } => {
                    EffectNode::JunoHpf(hardware::JunoHpf::new(cutoff, resonance, sample_rate))
                }
                EffectSpec::BuchlaLpg {
                    strike,
                    decay,
                    resonance,
                } => EffectNode::BuchlaLpg(hardware::BuchlaLpg::new(
                    strike,
                    decay,
                    resonance,
                    sample_rate,
                )),
            })
            .collect();

        Self {
            effects,
            tail_seconds: tail_seconds(specs),
        }
    }

    pub fn process(&mut self, mut sample: f32, sample_rate: f32) -> f32 {
        for effect in &mut self.effects {
            sample = match effect {
                EffectNode::Biquad(effect) => effect.process(sample),
                EffectNode::Comb(effect) => effect.process(sample),
                EffectNode::Formant(effect) => effect.process(sample),
                EffectNode::Distortion(effect) => effect.process(sample),
                EffectNode::Bitcrush(effect) => effect.process(sample),
                EffectNode::Delay(effect) => effect.process(sample),
                EffectNode::Wavefolder(effect) => effect.process(sample),
                EffectNode::Resonator(effect) => effect.process(sample),
                EffectNode::Lofi(effect) => effect.process(sample),
                EffectNode::Vinyl(effect) => effect.process(sample, sample_rate),
                EffectNode::SubBass(effect) => effect.process(sample),
                EffectNode::Sidechain(effect) => effect.process(sample, sample_rate),
                EffectNode::Radio(effect) => effect.process(sample, sample_rate),
                EffectNode::Telephone(effect) => effect.process(sample),
                EffectNode::Underwater(effect) => effect.process(sample, sample_rate),
                EffectNode::Crystal(effect) => effect.process(sample),
                EffectNode::DcRemove(effect) => effect.process(sample),
                EffectNode::PitchShift(effect) => effect.process(sample),
                EffectNode::Harmonizer(effect) => effect.process(sample),
                EffectNode::Octaver(effect) => effect.process(sample),
                EffectNode::Shimmer(effect) => effect.process(sample),
                EffectNode::Stutter(effect) => effect.process(sample),
                EffectNode::Glitch(effect) => effect.process(sample),
                EffectNode::Fade(effect) => effect.process(sample),
                EffectNode::Adsr(effect) => effect.process(sample),
                EffectNode::Doppler(effect) => effect.process(sample, sample_rate),
                EffectNode::Maximizer(effect) => effect.process(sample),
                EffectNode::MultibandComp(effect) => effect.process(sample),
                EffectNode::HarmonicEnhance(effect) => effect.process(sample),
                EffectNode::Body(effect) => effect.process(sample),
                EffectNode::Warmth(effect) => effect.process(sample),
                EffectNode::Spatial(effect) => effect.process(sample),
                EffectNode::ParallelComp(effect) => effect.process(sample),
                EffectNode::Tremolo(effect) => effect.process(sample, sample_rate),
                EffectNode::Chorus(effect) => effect.process(sample, sample_rate),
                EffectNode::Ensemble(effect) => effect.process(sample, sample_rate),
                EffectNode::Dimension(effect) => effect.process(sample, sample_rate),
                EffectNode::Ce1Chorus(effect) => effect.process(sample, sample_rate),
                EffectNode::Re301Chorus(effect) => effect.process(sample, sample_rate),
                EffectNode::DimensionD(effect) => effect.process(sample, sample_rate),
                EffectNode::H3000(effect) => effect.process(sample),
                EffectNode::Flanger(effect) => effect.process(sample, sample_rate),
                EffectNode::Phaser(effect) => effect.process(sample, sample_rate),
                EffectNode::SmallStone(effect) => effect.process(sample, sample_rate),
                EffectNode::Vibrato(effect) => effect.process(sample, sample_rate),
                EffectNode::RingMod(effect) => effect.process(sample, sample_rate),
                EffectNode::ArpRingMod(effect) => effect.process(sample, sample_rate),
                EffectNode::Compressor(effect) => effect.process(sample),
                EffectNode::Fairchild(effect) => effect.process(sample),
                EffectNode::SslComp(effect) => effect.process(sample),
                EffectNode::Dbx160(effect) => effect.process(sample),
                EffectNode::La2a(effect) => effect.process(sample),
                EffectNode::Urei1176(effect) => effect.process(sample),
                EffectNode::Gate(effect) => effect.process(sample),
                EffectNode::Transient(effect) => effect.process(sample),
                EffectNode::Reverb(effect) => effect.process(sample),
                EffectNode::SpringReverb(effect) => effect.process(sample),
                EffectNode::EmtPlate(effect) => effect.process(sample),
                EffectNode::Lexicon224(effect) => effect.process(sample, sample_rate),
                EffectNode::AmsReverb(effect) => effect.process(sample),
                EffectNode::Tube(effect) => effect.process(sample),
                EffectNode::NevePreamp(effect) => effect.process(sample),
                EffectNode::MarshallAmp(effect) => effect.process(sample),
                EffectNode::VoxAc30(effect) => effect.process(sample),
                EffectNode::FenderTwin(effect) => effect.process(sample),
                EffectNode::PultecEq(effect) => effect.process(sample),
                EffectNode::Tc2290(effect) => effect.process(sample, sample_rate),
                EffectNode::Exciter(effect) => effect.process(sample),
                EffectNode::Tape(effect) => effect.process(sample, sample_rate),
                EffectNode::StuderTape(effect) => effect.process(sample, sample_rate),
                EffectNode::Moog(effect) => effect.process(sample),
                EffectNode::ProphetFilter(effect) => effect.process(sample),
                EffectNode::ObxaFilter(effect) => effect.process(sample),
                EffectNode::WaspFilter(effect) => effect.process(sample),
                EffectNode::Diode303(effect) => effect.process(sample),
                EffectNode::SpaceEcho(effect) => effect.process(sample, sample_rate),
                EffectNode::Sem(effect) => effect.process(sample),
                EffectNode::Ms20(effect) => effect.process(sample),
                EffectNode::JunoHpf(effect) => effect.process(sample),
                EffectNode::BuchlaLpg(effect) => effect.process(sample),
            };
        }
        sample
    }

    pub fn tail_seconds(&self) -> f32 {
        self.tail_seconds
    }
}

pub fn tail_seconds(specs: &[EffectSpec]) -> f32 {
    let delay_tail = specs.iter().fold(0.04_f32, |tail, spec| match *spec {
        EffectSpec::Delay { time, feedback, .. } => tail.max(time * (1.0 + feedback * 6.0)),
        EffectSpec::Comb {
            delay_ms, feedback, ..
        } => tail.max((delay_ms / 1_000.0) * (1.0 + feedback * 6.0)),
        EffectSpec::Reverb { decay, .. } => tail.max(0.5 + decay * 4.5),
        EffectSpec::SpringReverb { decay, .. } => tail.max(0.2 + decay * 1.5),
        EffectSpec::EmtPlate { decay, .. } => tail.max(0.5 + decay * 1.8),
        EffectSpec::Lexicon224 { decay, .. } => tail.max(0.5 + decay * 1.8),
        EffectSpec::AmsReverb { decay, .. } => tail.max(0.3 + decay * 1.2),
        EffectSpec::SpaceEcho { time, feedback, .. } => tail.max(time * (1.0 + feedback * 6.0)),
        EffectSpec::Tc2290 {
            time_ms, feedback, ..
        } => tail.max((time_ms / 1_000.0) * (1.0 + feedback * 6.0)),
        EffectSpec::Shimmer { feedback, .. } => tail.max(0.08 * (1.0 + feedback * 6.0)),
        EffectSpec::Adsr { release, .. } => tail.max(release),
        EffectSpec::Stutter {
            grain_size_ms,
            repeats,
            ..
        } => tail.max((grain_size_ms / 1_000.0) * (1.0 + repeats)),
        _ => tail,
    });
    delay_tail.clamp(0.04, 6.0)
}

pub(crate) fn soft_clip(x: f32) -> f32 {
    x / (1.0 + x.abs())
}

pub(crate) fn advance_lfo(phase: &mut f32, rate: f32, sample_rate: f32) -> f32 {
    let value = (*phase * TAU).sin();
    *phase = (*phase + rate / sample_rate) % 1.0;
    value
}
