//! Track outline by dead reckoning. iRacing has no live world coordinates (`Lat`/`Lon` only
//! exist in .ibt files), so one clean lap of the player is integrated from `Speed` and `Yaw`
//! and the closing error is spread over the lap. Afterwards every car is placed on this line
//! by its lap distance (`CarIdxLapDistPct`).

/// Resolution along the lap.
const BINS: usize = 1000;
/// Longest frame gap (s) that still counts as continuous driving.
const MAX_DT: f64 = 0.25;
/// Points needed for a usable lap (~60 Hz → a lap has thousands).
const MIN_POINTS: usize = 300;
/// Largest accepted closing error relative to the driven distance.
const MAX_CLOSURE: f32 = 0.08;
/// File format version of saved maps; bump when the geometry convention changes.
const FILE_VERSION: u32 = 1;

#[derive(Default)]
pub struct TrackMap {
    bins: Vec<[f32; 2]>,
    rec: Vec<(f32, [f32; 2])>,
    recording: bool,
    pos: [f32; 2],
    dist: f32,
    last_pct: f32,
    last_t: f64,
}

/// Forward and right unit vectors in the (x, z) plane for a yaw angle (radians,
/// counter-clockwise, x right / z up as on the overlay map).
pub fn axes(yaw: f32) -> ([f32; 2], [f32; 2]) {
    let (s, c) = yaw.sin_cos();
    ([c, s], [s, -c])
}

impl TrackMap {
    pub fn is_built(&self) -> bool {
        !self.bins.is_empty()
    }

    /// Driving direction of the built line (shoelace area; x right, z up). A known clockwise
    /// track that comes out counter-clockwise means the `Yaw` sign is flipped.
    pub fn clockwise(&self) -> Option<bool> {
        if self.bins.is_empty() {
            return None;
        }
        let n = self.bins.len();
        let area: f32 = (0..n).map(|i| {
            let (a, b) = (self.bins[i], self.bins[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        }).sum();
        Some(area < 0.0)
    }

    /// Saved form: version, bin count, then x/z pairs (f32 little endian).
    pub fn to_bytes(&self) -> Option<Vec<u8>> {
        if self.bins.is_empty() {
            return None;
        }
        let mut b = Vec::with_capacity(8 + self.bins.len() * 8);
        b.extend_from_slice(&FILE_VERSION.to_le_bytes());
        b.extend_from_slice(&(self.bins.len() as u32).to_le_bytes());
        for p in &self.bins {
            b.extend_from_slice(&p[0].to_le_bytes());
            b.extend_from_slice(&p[1].to_le_bytes());
        }
        Some(b)
    }

    /// Takes over a saved map (ignored if a map was built meanwhile or the file is invalid).
    pub fn load_bytes(&mut self, b: &[u8]) -> bool {
        let word = |o: usize| b.get(o..o + 4).map(|s| [s[0], s[1], s[2], s[3]]);
        let (Some(v), Some(n)) = (word(0), word(4)) else { return false };
        let n = u32::from_le_bytes(n) as usize;
        if u32::from_le_bytes(v) != FILE_VERSION || n != BINS || b.len() != 8 + n * 8 || self.is_built() {
            return false;
        }
        self.bins = (0..n)
            .filter_map(|i| {
                let o = 8 + i * 8;
                Some([f32::from_le_bytes(word(o)?), f32::from_le_bytes(word(o + 4)?)])
            })
            .collect();
        self.recording = false;
        self.rec.clear();
        true
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Position on the built line for a lap fraction.
    pub fn at(&self, pct: f32) -> Option<[f32; 2]> {
        if self.bins.is_empty() || !(0.0..=1.0).contains(&pct) {
            return None;
        }
        let f = pct * BINS as f32;
        let i = (f as usize).min(BINS - 1);
        let (a, b) = (self.bins[i], self.bins[(i + 1) % BINS]);
        let t = f - i as f32;
        Some([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t])
    }

    /// Player position: on the line once built, dead-reckoned before.
    pub fn player_pos(&self, pct: f32) -> [f32; 2] {
        self.at(pct).unwrap_or(self.pos)
    }

    /// Feeds one frame. `clean` = driving on track (not on pit road, not towed, no replay).
    pub fn update(&mut self, pct: f32, speed_ms: f32, yaw: f32, t_s: f64, clean: bool) {
        let dt = t_s - self.last_t;
        let prev_pct = self.last_pct;
        self.last_t = t_s;
        self.last_pct = pct;
        if !(0.0..=1.0).contains(&pct) {
            self.abort();
            return;
        }
        if dt <= 0.0 || dt > MAX_DT || !clean {
            self.abort();
            return;
        }
        let (fw, _) = axes(yaw);
        let step = speed_ms * dt as f32;
        self.pos = [self.pos[0] + fw[0] * step, self.pos[1] + fw[1] * step];
        self.dist += step;
        let crossed = prev_pct > 0.9 && pct < 0.1;
        if crossed {
            if self.recording && !self.is_built() {
                self.finish();
            }
            // Start a fresh lap at the line.
            self.rec.clear();
            self.recording = !self.is_built();
            self.pos = [0.0, 0.0];
            self.dist = 0.0;
        } else if self.recording && pct + 0.01 < prev_pct {
            // Went backwards (reset, spin onto the grass): this lap is unusable.
            self.abort();
            return;
        }
        if self.recording {
            self.rec.push((pct, self.pos));
        }
    }

    fn abort(&mut self) {
        self.recording = false;
        self.rec.clear();
    }

    fn finish(&mut self) {
        let n = self.rec.len();
        if n < MIN_POINTS || self.dist < 300.0 {
            return;
        }
        // The lap ends where it started: spread the closing error linearly over the lap.
        let err = self.pos;
        if (err[0].hypot(err[1])) > self.dist * MAX_CLOSURE {
            tracing::debug!("iracing map: closing error too large, lap dropped");
            return;
        }
        let pts: Vec<(f32, [f32; 2])> = self
            .rec
            .iter()
            .enumerate()
            .map(|(i, (p, xy))| {
                let k = i as f32 / n as f32;
                (*p, [xy[0] - err[0] * k, xy[1] - err[1] * k])
            })
            .collect();
        let mut bins = Vec::with_capacity(BINS);
        let mut j = 0;
        for b in 0..BINS {
            let target = b as f32 / BINS as f32;
            while j + 1 < pts.len() && pts[j + 1].0 < target {
                j += 1;
            }
            let (p0, a) = pts[j];
            let (p1, c) = pts.get(j + 1).copied().unwrap_or((1.0, [0.0, 0.0]));
            let t = if p1 > p0 { ((target - p0) / (p1 - p0)).clamp(0.0, 1.0) } else { 0.0 };
            bins.push([a[0] + (c[0] - a[0]) * t, a[1] + (c[1] - a[1]) * t]);
        }
        self.bins = bins;
        self.recording = false;
        self.rec.clear();
        tracing::info!("iracing map built ({n} samples, {:.0} m, clockwise={:?})", self.dist, self.clockwise());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    /// Drives `laps` laps on a circle of radius r at 50 m/s, 60 Hz.
    fn drive(m: &mut TrackMap, r: f32, laps: f32, drift: f32) {
        let len = TAU * r;
        let v = 50.0;
        let steps = (laps * len / v * 60.0) as usize;
        for k in 0..steps {
            let s = k as f32 * v / 60.0;
            let pct = (s / len).fract();
            // Heading of a counter-clockwise circle, with a small sensor drift.
            let yaw = pct * TAU + std::f32::consts::FRAC_PI_2 + drift;
            m.update(pct, v, yaw, k as f64 / 60.0, true);
        }
    }

    #[test]
    fn builds_a_closed_circle() {
        let mut m = TrackMap::default();
        drive(&mut m, 400.0, 2.5, 0.0);
        assert!(m.is_built());
        let a = m.at(0.0).unwrap();
        let b = m.at(0.5).unwrap();
        // Opposite points of the circle are one diameter apart.
        let d = (a[0] - b[0]).hypot(a[1] - b[1]);
        assert!((d - 800.0).abs() < 15.0, "{d}");
        // Start heading up (+z), turning left: a quarter lap later the car is up and left.
        let q = m.at(0.25).unwrap();
        assert!(q[1] > 300.0 && q[0] < -300.0, "{q:?}");
        assert_eq!(m.clockwise(), Some(false));
        // Saved and loaded again: same line.
        let mut m2 = TrackMap::default();
        assert!(m2.load_bytes(&m.to_bytes().unwrap()));
        assert_eq!(m2.at(0.37), m.at(0.37));
        assert!(!TrackMap::default().load_bytes(&[1, 2, 3]));
    }

    #[test]
    fn small_drift_is_closed_and_pit_or_gap_aborts() {
        let mut m = TrackMap::default();
        drive(&mut m, 400.0, 2.5, 0.0);
        let mut m2 = TrackMap::default();
        drive(&mut m2, 400.0, 2.5, 0.01);
        assert!(m2.is_built());
        // Not clean: never built.
        let mut m3 = TrackMap::default();
        for k in 0..20_000 {
            m3.update((k as f32 / 3000.0).fract(), 50.0, 0.0, k as f64 / 60.0, false);
        }
        assert!(!m3.is_built());
    }

    #[test]
    fn axes_are_orthogonal() {
        let (f, r) = axes(0.0);
        assert_eq!(f, [1.0, 0.0]);
        assert!((r[0]).abs() < 1e-6 && (r[1] + 1.0).abs() < 1e-6);
        let (f, r) = axes(std::f32::consts::FRAC_PI_2);
        assert!((f[1] - 1.0).abs() < 1e-6 && (r[0] - 1.0).abs() < 1e-6);
    }
}
