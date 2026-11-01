//! Team-radio effect, applied offline when a clip is rendered (never in the audio callback).
//! `strength` 0..=1 scales band limiting, saturation and noise together.

use crate::storage::settings::RadioFx;

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
    fn resample_length() {
        let x = vec![0.0; 22_050];
        assert_eq!(resample(&x, 22_050, 48_000).len(), 48_000);
    }
}
