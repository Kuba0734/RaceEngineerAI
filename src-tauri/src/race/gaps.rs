//! Position-based timing: the lap is split into mini-sectors, every car's crossing time is
//! stored per mini-sector, the gap is the time difference at the same point on track.

use std::collections::{BTreeMap, VecDeque};

pub const MINI_SECTORS: usize = 100;

#[derive(Debug, Clone)]
struct CarTiming {
    /// Last two crossing times (s) per mini-sector: [latest, previous].
    crossings: Vec<[Option<f64>; 2]>,
    last_progress: Option<f64>,
    last_t: f64,
}

impl CarTiming {
    fn new() -> Self {
        Self { crossings: vec![[None, None]; MINI_SECTORS], last_progress: None, last_t: 0.0 }
    }

    /// `progress` = laps + spline (monotonic).
    fn update(&mut self, raw: f64, t: f64) {
        let mut progress = raw;
        if let Some(prev) = self.last_progress {
            // At the line the lap counter and the spline don't flip in the same packet:
            // that looks like a jump of exactly one lap and is no teleport.
            let wrap = (progress - prev).round();
            if wrap.abs() == 1.0 && (progress - prev - wrap).abs() < 0.05 {
                progress -= wrap;
            }
            let d = progress - prev;
            // Teleports (back to pits, session restart) reset the timing.
            if !(0.0..0.5).contains(&d) {
                *self = Self::new();
            } else if d > 0.0 {
                let from = (prev * MINI_SECTORS as f64).floor() as i64 + 1;
                let to = (progress * MINI_SECTORS as f64).floor() as i64;
                for b in from..=to {
                    let at = b as f64 / MINI_SECTORS as f64;
                    let frac = (at - prev) / d;
                    let tc = self.last_t + (t - self.last_t) * frac;
                    let slot = &mut self.crossings[b.rem_euclid(MINI_SECTORS as i64) as usize];
                    slot[1] = slot[0];
                    slot[0] = Some(tc);
                }
            }
        }
        self.last_progress = Some(progress);
        self.last_t = t;
    }

    fn latest_index(&self) -> Option<usize> {
        self.last_progress.map(|p| ((p * MINI_SECTORS as f64).floor() as i64).rem_euclid(MINI_SECTORS as i64) as usize)
    }
}

#[derive(Debug, Default)]
pub struct GapTracker {
    cars: BTreeMap<i32, CarTiming>,
    /// Per car: gap history (s, + = car is ahead of player) sampled at each player lap.
    history: BTreeMap<i32, VecDeque<f64>>,
}

impl GapTracker {
    pub fn reset(&mut self) {
        self.cars.clear();
        self.history.clear();
    }

    pub fn update(&mut self, car_id: i32, laps: i32, spline: f32, t_s: f64) {
        let progress = f64::from(laps) + f64::from(spline.clamp(0.0, 0.99999));
        self.cars.entry(car_id).or_insert_with(CarTiming::new).update(progress, t_s);
    }

    /// Time gap (s) of `other` relative to `me` at the most recent common point.
    /// Positive: `other` is ahead (passed the point earlier).
    pub fn gap(&self, me: i32, other: i32) -> Option<f64> {
        let a = self.cars.get(&me)?;
        let b = self.cars.get(&other)?;
        let (leader, trailer, sign) = if b.last_progress? > a.last_progress? { (b, a, 1.0) } else { (a, b, -1.0) };
        // Measure at the point the trailing car crossed last.
        let idx = trailer.latest_index()?;
        let t_trail = trailer.crossings[idx][0]?;
        let [latest, prev] = leader.crossings[idx];
        let t_lead = latest.filter(|t| *t <= t_trail).or(prev.filter(|t| *t <= t_trail))?;
        Some(sign * (t_trail - t_lead))
    }

    /// Progress (laps + spline) of a car, if known.
    pub fn progress(&self, id: i32) -> Option<f64> {
        self.cars.get(&id)?.last_progress
    }

    /// Call on player lap completion: stores gaps for trend analysis.
    pub fn sample_lap(&mut self, me: i32, window: usize) {
        let ids: Vec<i32> = self.cars.keys().copied().filter(|id| *id != me).collect();
        for id in ids {
            if let Some(g) = self.gap(me, id) {
                let h = self.history.entry(id).or_default();
                h.push_back(g);
                while h.len() > window.max(2) {
                    h.pop_front();
                }
            }
        }
    }

    /// Change of the gap per lap (s/lap) over the stored window, least squares.
    /// Positive: the other car pulls away (if ahead) / falls back (if behind) – i.e. gap magnitude grows.
    pub fn trend(&self, id: i32) -> Option<f64> {
        let h = self.history.get(&id)?;
        if h.len() < 3 {
            return None;
        }
        let n = h.len() as f64;
        let mean_x = (n - 1.0) / 2.0;
        let mean_y = h.iter().sum::<f64>() / n;
        let (mut num, mut den) = (0.0, 0.0);
        for (i, y) in h.iter().enumerate() {
            let dx = i as f64 - mean_x;
            num += dx * (y - mean_y);
            den += dx * dx;
        }
        (den > 0.0).then(|| num / den)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_crossing_with_late_lap_counter_keeps_timing() {
        let mut gt = GapTracker::default();
        gt.update(1, 3, 0.99, 0.0);
        gt.update(1, 3, 0.005, 0.1); // spline wrapped, lap counter still old
        assert!((gt.progress(1).unwrap() - 4.005).abs() < 1e-3);
        assert!(gt.cars[&1].crossings[0][0].is_some(), "timing must survive the line");
        gt.update(1, 4, 0.02, 0.2); // counter caught up
        assert!((gt.progress(1).unwrap() - 4.02).abs() < 1e-3);
        gt.update(2, 5, 0.99, 0.0);
        gt.update(2, 6, 0.995, 0.1); // counter first, spline still old
        assert!((gt.progress(2).unwrap() - 5.995).abs() < 1e-3);
        gt.update(2, 6, 0.01, 0.2);
        assert!((gt.progress(2).unwrap() - 6.01).abs() < 1e-3);
    }

    /// Simulates two cars at constant speeds (laps per second).
    fn sim(gt: &mut GapTracker, speeds: &[(i32, f64, f64)], t_end: f64) {
        let mut t = 0.0;
        while t <= t_end {
            for (id, start, v) in speeds {
                let p = start + v * t;
                gt.update(*id, p.floor() as i32, p.fract() as f32, t);
            }
            t += 0.1;
        }
    }

    #[test]
    fn gap_to_car_ahead() {
        let mut gt = GapTracker::default();
        // Lap = 100 s. Car 2 is 2 s ahead (0.02 lap).
        sim(&mut gt, &[(1, 0.0, 0.01), (2, 0.02, 0.01)], 50.0);
        let g = gt.gap(1, 2).unwrap();
        assert!((g - 2.0).abs() < 0.15, "gap {g}");
        let g = gt.gap(2, 1).unwrap();
        assert!((g + 2.0).abs() < 0.15, "gap {g}");
    }

    #[test]
    fn trend_detects_closing() {
        let mut gt = GapTracker::default();
        let mut t: f64 = 0.0;
        // Car 2 starts 3 s ahead, player faster by 0.3 s per lap (lap ≈ 100 s).
        let v1 = 1.0 / 99.7;
        let v2 = 1.0 / 100.0;
        let mut last_lap = 0;
        while t < 600.0 {
            let p1 = v1 * t;
            let p2 = 0.03 + v2 * t;
            gt.update(1, p1.floor() as i32, p1.fract() as f32, t);
            gt.update(2, p2.floor() as i32, p2.fract() as f32, t);
            if p1.floor() as i32 > last_lap {
                last_lap = p1.floor() as i32;
                gt.sample_lap(1, 5);
            }
            t += 0.1;
        }
        let tr = gt.trend(2).unwrap();
        assert!((tr + 0.3).abs() < 0.05, "trend {tr}");
    }
}
