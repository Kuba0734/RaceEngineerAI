//! Team-radio effect, applied offline when a clip is rendered (never in the audio callback).
//! `strength` 0..=1 scales band limiting, saturation and noise together.

use crate::storage::settings::{PttSoundStyle, RadioFx};

#[derive(Clone, Copy)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    fn new(kind: char, f0: f32, sr: f32, q: f32) -> Self {
        let w0 = std::f32::consts::TAU * f0 / sr;
        let (s, c) = w0.sin_cos();
        let alpha = s / (2.0 * q);
        let (b0, b1, b2) = if kind == 'h' { ((1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0) } else { ((1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0) };
        let a0 = 1.0 + alpha;
        Self { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: -2.0 * c / a0, a2: (1.0 - alpha) / a0, z1: 0.0, z2: 0.0 }
    }

    fn run(&mut self, x: f32) -> f32 {
        // Transposed direct form II.
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

/// Small deterministic noise source (no allocation, no RNG dependency in the hot path).
struct Lcg(u32);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
    }
}

fn click(out: &mut Vec<f32>, sr: f32, level: f32, rng: &mut Lcg) {
    let n = (sr * 0.045) as usize;
    for i in 0..n {
        let env = (1.0 - i as f32 / n as f32).powi(3);
        out.push(rng.next() * env * level);
    }
}

pub fn apply(input: &[f32], sr: u32, fx: &RadioFx) -> Vec<f32> {
    let srf = sr as f32;
    let k = fx.strength.clamp(0.0, 1.0);
    let mut rng = Lcg(0x5EED_1234);
    let mut out = Vec::with_capacity(input.len() + (srf * 0.2) as usize);
    if !fx.enabled || k <= 0.001 {
        out.extend_from_slice(input);
        return out;
    }
    // Band: full range at k=0 → 300..3400 Hz at k=1.
    let hp_f = 80.0 + 220.0 * k;
    let lp_f = (9000.0 - 5600.0 * k).min(srf * 0.45);
    let mut hp1 = Biquad::new('h', hp_f, srf, 0.707);
    let mut hp2 = Biquad::new('h', hp_f, srf, 0.707);
    let mut lp1 = Biquad::new('l', lp_f, srf, 0.707);
    let mut lp2 = Biquad::new('l', lp_f, srf, 0.707);
    let drive = 1.0 + 5.0 * k;
    let noise = 0.02 * fx.noise_level.clamp(0.0, 1.0) * k;
    let norm = drive.tanh();
    if fx.clicks {
        click(&mut out, srf, 0.25 * k, &mut rng);
    }
    for &x in input {
        let mut y = hp2.run(hp1.run(x));
        y = lp2.run(lp1.run(y));
        y = (y * drive).tanh() / norm;
        y += rng.next() * noise;
        out.push(y * 0.9);
    }
    if fx.clicks {
        // Squelch tail.
        let tail = (srf * 0.08) as usize;
        for i in 0..tail {
            let env = 1.0 - i as f32 / tail as f32;
            out.push(rng.next() * noise * 4.0 * env);
        }
        click(&mut out, srf, 0.2 * k, &mut rng);
    }
    out
}

/// Band-limited radio static fading from `level` to silence.
fn static_burst(out: &mut Vec<f32>, srf: f32, ms: f32, low: f32, high: f32, level: f32, rng: &mut Lcg) {
    let n = (srf * ms / 1000.0) as usize;
    let mut lp = Biquad::new('l', high, srf, 0.7);
    let mut hp = Biquad::new('h', low, srf, 0.7);
    for i in 0..n {
        let env = (1.0 - i as f32 / n as f32).powi(2);
        out.push(hp.run(lp.run(rng.next())) * level * env);
    }
}

/// Short thump of the transmit key.
fn key_thump(out: &mut Vec<f32>, srf: f32, freq: f32, ms: f32, level: f32) {
    let n = (srf * ms / 1000.0) as usize;
    for i in 0..n {
        let env = (1.0 - i as f32 / n as f32).powi(3);
        out.push((i as f32 * freq * std::f32::consts::TAU / srf).sin() * level * env);
    }
}

/// Like a radio speaker: 300–3400 Hz band, a little saturation.
fn radio_band(input: &mut [f32], srf: f32, drive: f32) {
    let mut hp1 = Biquad::new('h', 300.0, srf, 0.707);
    let mut hp2 = Biquad::new('h', 300.0, srf, 0.707);
    let mut lp1 = Biquad::new('l', 3400.0_f32.min(srf * 0.45), srf, 0.707);
    let mut lp2 = Biquad::new('l', 3400.0_f32.min(srf * 0.45), srf, 0.707);
    let norm = drive.tanh();
    for x in input.iter_mut() {
        let y = lp2.run(lp1.run(hp2.run(hp1.run(*x))));
        *x = (y * drive).tanh() / norm;
    }
}

/// Digital data burst ("brrt"): noise chopped into short frames, as digital radios send a
/// header when keying up and a tail when releasing.
fn data_burst(out: &mut Vec<f32>, srf: f32, ms: f32, frame_ms: f32, level: f32, rng: &mut Lcg) {
    let n = (srf * ms / 1000.0) as usize;
    let frame = ((srf * frame_ms / 1000.0) as usize).max(1);
    for i in 0..n {
        let k = i as f32 / n as f32;
        // Frames alternate loud / quiet; a soft attack and release keep it from clicking.
        let gate = if (i / frame) % 2 == 0 { 1.0 } else { 0.25 };
        let env = (k * 20.0).min(1.0) * (1.0 - k).powf(0.7);
        out.push(rng.next() * level * gate * env);
    }
}

/// Analog FM squelch tail: full-band hiss that stops abruptly when the squelch closes.
fn squelch_tail(out: &mut Vec<f32>, srf: f32, ms: f32, level: f32, rng: &mut Lcg) {
    let n = (srf * ms / 1000.0) as usize;
    let mut lp = Biquad::new('l', 5000.0_f32.min(srf * 0.45), srf, 0.7);
    for i in 0..n {
        let k = i as f32 / n as f32;
        // Holds, then a quick fall over the last 15 % – the "kssh" of a closing squelch.
        let env = if k < 0.85 { 1.0 - k * 0.3 } else { (1.0 - k) / 0.15 * 0.75 };
        out.push(lp.run(rng.next()) * level * env);
    }
}

/// Push-to-talk feedback for press and release (volume 0–1.5).
pub fn ptt_clip_vol(pressed: bool, style: PttSoundStyle, sr: u32, volume: f32) -> Vec<f32> {
    let v = volume.clamp(0.0, 1.5);
    ptt_clip(pressed, style, sr).into_iter().map(|x| (x * v).clamp(-1.0, 1.0)).collect()
}

/// Push-to-talk feedback for press and release.
pub fn ptt_clip(pressed: bool, style: PttSoundStyle, sr: u32) -> Vec<f32> {
    let srf = sr as f32;
    let mut rng = Lcg(if pressed { 0xBEEF } else { 0xF00D });
    let mut out = Vec::new();
    match (style, pressed) {
        // Key-up: mechanical click, a short silence, then the digital header burst.
        (PttSoundStyle::TeamRadio, true) => {
            key_thump(&mut out, srf, 180.0, 10.0, 0.5);
            click(&mut out, srf, 0.35, &mut rng);
            out.extend(std::iter::repeat_n(0.0, (srf * 0.018) as usize));
            data_burst(&mut out, srf, 70.0, 6.0, 0.45, &mut rng);
            radio_band(&mut out, srf, 2.5);
        }
        // Release: the tail burst, a touch of hiss, then the key letting go.
        (PttSoundStyle::TeamRadio, false) => {
            data_burst(&mut out, srf, 55.0, 5.0, 0.4, &mut rng);
            static_burst(&mut out, srf, 60.0, 600.0, 3200.0, 0.25, &mut rng);
            radio_band(&mut out, srf, 2.5);
            click(&mut out, srf, 0.25, &mut rng);
        }
        // Carrier opens: a thump and a short rush of hiss.
        (PttSoundStyle::Squelch, true) => {
            key_thump(&mut out, srf, 140.0, 14.0, 0.45);
            static_burst(&mut out, srf, 45.0, 300.0, 4500.0, 0.4, &mut rng);
        }
        // Carrier drops: the classic squelch tail.
        (PttSoundStyle::Squelch, false) => {
            squelch_tail(&mut out, srf, 220.0, 0.38, &mut rng);
            radio_band(&mut out, srf, 1.6);
        }
        // Key-up: click, then a bright, very short "tsch".
        (PttSoundStyle::Static, true) => {
            click(&mut out, srf, 0.45, &mut rng);
            static_burst(&mut out, srf, 80.0, 1400.0, 4200.0, 0.5, &mut rng);
        }
        // Release: darker, longer squelch tail that closes with a click.
        (PttSoundStyle::Static, false) => {
            static_burst(&mut out, srf, 190.0, 500.0, 2600.0, 0.5, &mut rng);
            click(&mut out, srf, 0.3, &mut rng);
        }
        (PttSoundStyle::Beep, true) => {
            for (f0, f1, ms) in [(900.0f32, 1300.0f32, 45.0f32), (1500.0, 1600.0, 55.0)] {
                let n = (srf * ms / 1000.0) as usize;
                let mut phase = 0.0f32;
                for i in 0..n {
                    let k = i as f32 / n as f32;
                    phase += (f0 + (f1 - f0) * k) * std::f32::consts::TAU / srf;
                    let env = (k * 12.0).min(1.0) * (1.0 - k).powf(0.6);
                    out.push(phase.sin() * 0.22 * env);
                }
                out.extend(std::iter::repeat_n(0.0, (srf * 0.012) as usize));
            }
        }
        (PttSoundStyle::Beep, false) => {
            static_burst(&mut out, srf, 160.0, 400.0, 2600.0, 0.35, &mut rng);
            click(&mut out, srf, 0.3, &mut rng);
        }
        (PttSoundStyle::Click, true) => key_thump(&mut out, srf, 210.0, 28.0, 0.6),
        (PttSoundStyle::Click, false) => key_thump(&mut out, srf, 320.0, 20.0, 0.45),
    }
    out
}

/// Linear resampling, good enough for speech.
pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let n = (input.len() as f64 / ratio) as usize;
    (0..n)
        .map(|i| {
            let p = i as f64 * ratio;
            let i0 = p.floor() as usize;
            let frac = (p - i0 as f64) as f32;
            let a = input[i0.min(input.len() - 1)];
            let b = input[(i0 + 1).min(input.len() - 1)];
            a + (b - a) * frac
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_limits_band_and_stays_bounded() {
        let sr = 22_050;
        let tone: Vec<f32> = (0..sr).map(|i| (i as f32 * 8000.0 * std::f32::consts::TAU / sr as f32).sin() * 0.8).collect();
        let fx = RadioFx { enabled: true, strength: 1.0, noise_level: 0.0, clicks: false };
        let out = apply(&tone, sr, &fx);
        let rms_in = (tone.iter().map(|x| x * x).sum::<f32>() / tone.len() as f32).sqrt();
        let rms_out = (out.iter().map(|x| x * x).sum::<f32>() / out.len() as f32).sqrt();
        assert!(rms_out < rms_in * 0.3, "8 kHz should be attenuated: {rms_out} vs {rms_in}");
        assert!(out.iter().all(|x| x.abs() <= 1.0));
    }

    #[test]
    fn disabled_is_passthrough() {
        let x = vec![0.1, -0.2, 0.3];
        let fx = RadioFx { enabled: false, ..RadioFx::default() };
        assert_eq!(apply(&x, 22_050, &fx), x);
    }

    #[test]
    fn ptt_clips_are_short_and_bounded() {
        for style in [PttSoundStyle::TeamRadio, PttSoundStyle::Squelch, PttSoundStyle::Static, PttSoundStyle::Beep, PttSoundStyle::Click] {
            for p in [true, false] {
                let c = ptt_clip(p, style, 48_000);
                assert!(!c.is_empty() && c.len() < 48_000 / 3);
                assert!(c.iter().all(|x| x.abs() <= 1.0));
            }
        }
    }

    #[test]
    fn resample_length() {
        let x = vec![0.0; 22_050];
        assert_eq!(resample(&x, 22_050, 48_000).len(), 48_000);
    }
}
