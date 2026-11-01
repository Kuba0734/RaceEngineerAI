//! Corner-by-corner lap comparison against the reference (best) lap.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Distance resolution of a trace (m).
pub const BIN_M: f32 = 5.0;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Sample {
    pub speed_kmh: f32,
    pub throttle: f32,
    pub brake: f32,
    pub tc: bool,
    pub abs: bool,
}

/// One lap as samples per distance bin.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Trace {
    pub track_m: f32,
    pub lap_ms: i32,
    pub bins: Vec<Option<Sample>>,
}

impl Trace {
    pub fn new(track_m: f32) -> Self {
        let n = (track_m / BIN_M).ceil().max(1.0) as usize;
        Self { track_m, lap_ms: 0, bins: vec![None; n] }
    }

    pub fn record(&mut self, spline: f32, s: Sample) {
        let n = self.bins.len();
        let i = ((spline.clamp(0.0, 0.99999) * n as f32) as usize).min(n - 1);
        self.bins[i] = Some(s);
    }

    /// Share of bins filled; incomplete laps (pit, reset) are not used.
    pub fn coverage(&self) -> f32 {
        self.bins.iter().filter(|b| b.is_some()).count() as f32 / self.bins.len().max(1) as f32
    }

    /// Fills gaps by carrying the previous sample forward.
    pub fn filled(&self) -> Vec<Sample> {
        let mut out = Vec::with_capacity(self.bins.len());
        let mut last = self.bins.iter().flatten().next().copied().unwrap_or_default();
        for b in &self.bins {
            if let Some(s) = b {
                last = *s;
            }
            out.push(last);
        }
        out
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        serde_json::from_slice(b).ok()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Corner {
    pub number: u32,
    pub apex_m: f32,
    pub start_m: f32,
    pub end_m: f32,
}

/// Detects corners from local speed minima of the reference lap.
pub fn detect_corners(reference: &Trace) -> Vec<Corner> {
    let s = reference.filled();
    let n = s.len();
    if n < 20 {
        return Vec::new();
    }
    // Smooth speeds (moving average ±3 bins, wrapping).
    let v: Vec<f32> = (0..n)
        .map(|i| {
            let mut sum = 0.0;
            for k in -3i64..=3 {
                sum += s[((i as i64 + k).rem_euclid(n as i64)) as usize].speed_kmh;
            }
            sum / 7.0
        })
        .collect();
    let window = (120.0 / BIN_M) as i64;
    let mut apexes = Vec::new();
    for i in 0..n {
        let vi = v[i];
        let is_min = (-window..=window).all(|k| k == 0 || v[((i as i64 + k).rem_euclid(n as i64)) as usize] >= vi);
        if !is_min {
            continue;
        }
        // Prominence: the speed before the corner must be clearly higher.
        let back = (300.0 / BIN_M) as i64;
        let max_before = (1..=back).map(|k| v[((i as i64 - k).rem_euclid(n as i64)) as usize]).fold(0.0f32, f32::max);
        if max_before - vi > 15.0 && apexes.last().is_none_or(|last: &usize| i - last > window as usize) {
            apexes.push(i);
        }
    }
    apexes
        .iter()
        .enumerate()
        .map(|(k, &i)| {
            let apex = i as f32 * BIN_M;
            Corner { number: k as u32 + 1, apex_m: apex, start_m: (apex - 250.0).max(0.0), end_m: (apex + 150.0).min(reference.track_m) }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CornerIssue {
    BrakedEarly,
    BrakedLate,
    SlowApex,
    LateThrottle,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CornerDelta {
    pub corner: u32,
    /// Seconds lost (+) or gained (−) in this corner.
    pub time_loss_s: f32,
    pub brake_point_delta_m: Option<f32>,
    pub apex_speed_delta_kmh: f32,
    pub throttle_delta_m: Option<f32>,
    pub issue: CornerIssue,
}

fn idx(m: f32, n: usize) -> usize {
    ((m / BIN_M) as usize).min(n.saturating_sub(1))
}

fn brake_point(s: &[Sample], from: usize, apex: usize) -> Option<usize> {
    (from..=apex).find(|&i| s[i].brake > 0.2)
}

fn throttle_point(s: &[Sample], apex: usize, to: usize) -> Option<usize> {
    (apex..=to).find(|&i| s[i].throttle > 0.9)
}

fn segment_time(s: &[Sample], from: usize, to: usize) -> f32 {
    (from..=to).map(|i| BIN_M / (s[i].speed_kmh.max(5.0) / 3.6)).sum()
}

pub fn compare(reference: &Trace, lap: &Trace, corners: &[Corner]) -> Vec<CornerDelta> {
    let r = reference.filled();
    let l = lap.filled();
    let n = r.len().min(l.len());
    if n == 0 {
        return Vec::new();
    }
    corners
        .iter()
        .map(|c| {
            let (a, b, apex) = (idx(c.start_m, n), idx(c.end_m, n), idx(c.apex_m, n));
            let loss = segment_time(&l, a, b) - segment_time(&r, a, b);
            let rmin = (a..=b).map(|i| r[i].speed_kmh).fold(f32::MAX, f32::min);
            let lmin = (a..=b).map(|i| l[i].speed_kmh).fold(f32::MAX, f32::min);
            let bp = match (brake_point(&r, a, apex), brake_point(&l, a, apex)) {
                (Some(x), Some(y)) => Some((x as f32 - y as f32) * BIN_M),
                _ => None,
            };
            let tp = match (throttle_point(&r, apex, b), throttle_point(&l, apex, b)) {
                (Some(x), Some(y)) => Some((y as f32 - x as f32) * BIN_M),
                _ => None,
            };
            let apex_delta = lmin - rmin;
            // bp > 0: braked earlier than reference by bp metres.
            let issue = if bp.is_some_and(|d| d > 6.0) {
                CornerIssue::BrakedEarly
            } else if bp.is_some_and(|d| d < -6.0) && apex_delta < -2.0 {
                CornerIssue::BrakedLate
            } else if apex_delta < -3.0 {
                CornerIssue::SlowApex
            } else if tp.is_some_and(|d| d > 10.0) {
                CornerIssue::LateThrottle
            } else {
                CornerIssue::Unknown
            };
            CornerDelta { corner: c.number, time_loss_s: loss, brake_point_delta_m: bp, apex_speed_delta_kmh: apex_delta, throttle_delta_m: tp, issue }
        })
        .collect()
}

/// The `max` biggest losses above `min_loss_s`, biggest first.
pub fn biggest_losses(d: &[CornerDelta], max: usize, min_loss_s: f32) -> Vec<CornerDelta> {
    let mut v: Vec<CornerDelta> = d.iter().filter(|c| c.time_loss_s >= min_loss_s).cloned().collect();
    v.sort_by(|a, b| b.time_loss_s.total_cmp(&a.time_loss_s));
    v.truncate(max);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Track of 2000 m with two corners at 600 m and 1500 m.
    fn lap(brake_shift_m: f32, apex_drop: f32) -> Trace {
        let mut t = Trace::new(2000.0);
        let n = t.bins.len();
        for i in 0..n {
            let m = i as f32 * BIN_M;
            let mut speed: f32 = 250.0;
            let mut brake: f32 = 0.0;
            let mut throttle: f32 = 1.0;
            for (apex, vmin) in [(600.0f32, 90.0f32), (1500.0, 120.0)] {
                let bp = apex - 120.0 - if apex == 600.0 { brake_shift_m } else { 0.0 };
                let vmin = if apex == 600.0 { vmin - apex_drop } else { vmin };
                if m >= bp && m <= apex {
                    let k = (m - bp) / (apex - bp);
                    speed = speed.min(250.0 - (250.0 - vmin) * k);
                    brake = 1.0;
                    throttle = 0.0;
                } else if m > apex && m < apex + 150.0 {
                    let k = (m - apex) / 150.0;
                    speed = speed.min(vmin + (250.0 - vmin) * k);
                    throttle = if k > 0.3 { 1.0 } else { 0.5 };
                }
            }
            t.bins[i] = Some(Sample { speed_kmh: speed, throttle, brake, tc: false, abs: false });
        }
        t
    }

    #[test]
    fn finds_two_corners() {
        let c = detect_corners(&lap(0.0, 0.0));
        assert_eq!(c.len(), 2, "{c:?}");
        assert!((c[0].apex_m - 600.0).abs() < 20.0);
        assert!((c[1].apex_m - 1500.0).abs() < 20.0);
    }

    #[test]
    fn early_braking_is_reported() {
        let r = lap(0.0, 0.0);
        let corners = detect_corners(&r);
        let d = compare(&r, &lap(10.0, 6.0), &corners);
        let worst = biggest_losses(&d, 1, 0.05);
        assert_eq!(worst.len(), 1);
        assert_eq!(worst[0].corner, 1);
        assert_eq!(worst[0].issue, CornerIssue::BrakedEarly);
        assert!((worst[0].brake_point_delta_m.unwrap() - 10.0).abs() < 6.0);
    }

    #[test]
    fn identical_lap_no_losses() {
        let r = lap(0.0, 0.0);
        let corners = detect_corners(&r);
        assert!(biggest_losses(&compare(&r, &r, &corners), 2, 0.01).is_empty());
    }
}
