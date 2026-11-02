//! Rule-based spotter. Runs inline on every telemetry frame; never touches network or AI.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::games::provider::{NativeSide, TelemetryFrame};
use crate::storage::settings::SpotterSettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Side {
    Clear,
    Left,
    Right,
    ThreeWide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpotterCall {
    Left,
    Right,
    ThreeWide,
    Clear,
    StillThere,
    ClosingFast,
}

impl SpotterCall {
    pub fn call_id(self) -> &'static str {
        match self {
            SpotterCall::Left => "spotter.left",
            SpotterCall::Right => "spotter.right",
            SpotterCall::ThreeWide => "spotter.three_wide",
            SpotterCall::Clear => "spotter.clear",
            SpotterCall::StillThere => "spotter.still_there",
            SpotterCall::ClosingFast => "spotter.closing_fast",
        }
    }
}

/// Car relative to the player, for the overlay radar. lon: + ahead, lat: + right (metres).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RadarCar {
    pub id: i32,
    pub lon: f32,
    pub lat: f32,
    pub overlap: bool,
}

pub struct Spotter {
    /// Announced state.
    state: Side,
    /// Raw state candidate and since when it is observed (ms).
    candidate: Side,
    candidate_since: u64,
    overlap_since: Option<u64>,
    last_still_there: u64,
    announced_since_clear: bool,
    last_lon: HashMap<i32, (f32, u64)>,
    closing_cooldown: HashMap<i32, u64>,
    pub radar: Vec<RadarCar>,
}

impl Default for Spotter {
    fn default() -> Self {
        Self::new()
    }
}

/// Lateral exit margin (m) and longitudinal exit margin on top of the car length: spatial
/// hysteresis so cars on the edge do not flicker.
const EXIT_MARGIN_LON: f32 = 0.6;
const EXIT_MARGIN_LAT: f32 = 0.5;
const RADAR_RANGE: f32 = 25.0;
/// Lateral offset (m) for radar dots when the game only tells the side, not the distance.
const NATIVE_LAT: f32 = 2.5;

impl Spotter {
    pub fn new() -> Self {
        Self {
            state: Side::Clear,
            candidate: Side::Clear,
            candidate_since: 0,
            overlap_since: None,
            last_still_there: 0,
            announced_since_clear: false,
            last_lon: HashMap::new(),
            closing_cooldown: HashMap::new(),
            radar: Vec::new(),
        }
    }

    pub fn state(&self) -> Side {
        self.state
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Processes one frame. `active` = spotter allowed now (session type, settings).
    pub fn update(&mut self, f: &TelemetryFrame, s: &SpotterSettings, active: bool, out: &mut Vec<SpotterCall>) {
        self.radar.clear();
        let stopped = f.speed_kmh < 15.0;
        let in_pits = (f.in_pit_lane || f.in_pit_box) && s.off_in_pits;
        let (px, pz) = (f.pos[0], f.pos[2]);
        let (fw, rt) = (f.forward, f.right);
        let mut left = false;
        let mut right = false;
        let currently_overlapping = self.state != Side::Clear;
        let lon_limit = s.car_length_m + if currently_overlapping { EXIT_MARGIN_LON } else { 0.0 };
        let lat_max = s.lateral_max_m + if currently_overlapping { EXIT_MARGIN_LAT } else { 0.0 };

        // (id, lon, lat, overlap) of every car around the player.
        let mut around: Vec<(i32, f32, f32, bool)> = Vec::with_capacity(8);
        if let Some(native) = f.native_side {
            // The game decides the side; positions only come along the track (no lateral offset).
            (left, right) = match native {
                NativeSide::Left => (true, false),
                NativeSide::Right => (false, true),
                NativeSide::ThreeWide => (true, true),
                NativeSide::Off | NativeSide::Clear => (false, false),
            };
            let mut next_left = true;
            for c in &f.near_cars {
                let overlap = c.lon_m.abs() < lon_limit && (left || right);
                // Show overlapping cars on the side the game reports.
                let lat = match (overlap, left, right) {
                    (false, _, _) => 0.0,
                    (true, true, true) => {
                        next_left = !next_left;
                        if next_left { NATIVE_LAT } else { -NATIVE_LAT }
                    }
                    (true, true, false) => -NATIVE_LAT,
                    (true, _, _) => NATIVE_LAT,
                };
                around.push((c.id, c.lon_m, lat, overlap));
            }
        } else {
            for c in &f.cars {
                if c.id == f.player_car_id {
                    continue;
                }
                let (dx, dz) = (c.x - px, c.z - pz);
                if dx.abs() > 60.0 || dz.abs() > 60.0 {
                    continue;
                }
                let lon = dx * fw[0] + dz * fw[1];
                let lat = dx * rt[0] + dz * rt[1];
                let overlap = lon.abs() < lon_limit && lat.abs() >= s.lateral_min_m && lat.abs() <= lat_max;
                if overlap {
                    if lat > 0.0 {
                        right = true;
                    } else {
                        left = true;
                    }
                }
                around.push((c.id, lon, lat, overlap));
            }
        }

        for (id, lon, lat, overlap) in around {
            if lon.abs() < RADAR_RANGE && lat.abs() < RADAR_RANGE {
                self.radar.push(RadarCar { id, lon, lat, overlap });
            }
            // Closing fast from behind: relative speed from successive longitudinal offsets.
            if active && !stopped && (-40.0..-6.0).contains(&lon) && lat.abs() < 4.0 {
                if let Some((prev, t)) = self.last_lon.get(&id) {
                    let dt = (f.t_ms.saturating_sub(*t)) as f32 / 1000.0;
                    if dt > 0.05 {
                        let closing_kmh = (lon - prev) / dt * 3.6;
                        let cool = self.closing_cooldown.get(&id).copied().unwrap_or(0);
                        if closing_kmh > s.closing_fast_kmh && f.t_ms > cool {
                            out.push(SpotterCall::ClosingFast);
                            self.closing_cooldown.insert(id, f.t_ms + 15_000);
                        }
                    }
                }
            }
            self.last_lon.insert(id, (lon, f.t_ms));
        }

        let native_off = f.native_side == Some(NativeSide::Off);
        if !active || stopped || in_pits || !s.enabled || native_off {
            if self.state != Side::Clear {
                self.state = Side::Clear;
                self.announced_since_clear = false;
            }
            self.candidate = Side::Clear;
            self.overlap_since = None;
            return;
        }

        let raw = match (left, right) {
            (true, true) => Side::ThreeWide,
            (true, false) => Side::Left,
            (false, true) => Side::Right,
            (false, false) => Side::Clear,
        };
        if raw != self.candidate {
            self.candidate = raw;
            self.candidate_since = f.t_ms;
        }
        // Into an overlap: immediate (latency budget). Back to clear: hold for the hysteresis.
        let hold = if raw == Side::Clear { u64::from(s.hysteresis_ms) } else { 0 };
        if self.candidate != self.state && f.t_ms.saturating_sub(self.candidate_since) >= hold {
            let prev = self.state;
            self.state = self.candidate;
            match self.state {
                Side::Left => out.push(SpotterCall::Left),
                Side::Right => out.push(SpotterCall::Right),
                Side::ThreeWide => out.push(SpotterCall::ThreeWide),
                Side::Clear => {
                    if self.announced_since_clear && s.say_clear {
                        out.push(SpotterCall::Clear);
                    }
                }
            }
            if self.state == Side::Clear {
                self.announced_since_clear = false;
                self.overlap_since = None;
            } else {
                self.announced_since_clear = true;
                if prev == Side::Clear {
                    self.overlap_since = Some(f.t_ms);
                    self.last_still_there = f.t_ms;
                }
            }
        }
        // Long side-by-side: "still there".
        if let Some(since) = self.overlap_since {
            let iv = (s.still_there_interval_s * 1000.0) as u64;
            if iv > 0 && f.t_ms.saturating_sub(since) > iv && f.t_ms.saturating_sub(self.last_still_there) > iv {
                self.last_still_there = f.t_ms;
                out.push(SpotterCall::StillThere);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::provider::CarPos;

    fn frame(t: u64, others: &[(f32, f32)]) -> TelemetryFrame {
        // Player at origin heading +z, right = +x.
        let mut cars = vec![CarPos { id: 0, x: 0.0, z: 0.0 }];
        for (i, (lat, lon)) in others.iter().enumerate() {
            cars.push(CarPos { id: i as i32 + 1, x: *lat, z: *lon });
        }
        TelemetryFrame { t_ms: t, speed_kmh: 150.0, forward: [0.0, 1.0], right: [1.0, 0.0], player_car_id: 0, cars, ..Default::default() }
    }

    fn run(sp: &mut Spotter, f: &TelemetryFrame) -> Vec<SpotterCall> {
        let mut out = Vec::new();
        sp.update(f, &SpotterSettings::default(), true, &mut out);
        out
    }

    #[test]
    fn left_right_three_wide_clear() {
        let mut sp = Spotter::new();
        assert!(run(&mut sp, &frame(0, &[(10.0, 0.0)])).is_empty());
        assert_eq!(run(&mut sp, &frame(16, &[(-2.5, 1.0)])), vec![SpotterCall::Left]);
        assert_eq!(run(&mut sp, &frame(32, &[(-2.5, 1.0), (2.6, -2.0)])), vec![SpotterCall::ThreeWide]);
        assert_eq!(run(&mut sp, &frame(48, &[(2.6, -2.0)])), vec![SpotterCall::Right]);
        // Clear needs the hysteresis (300 ms) of stable "no overlap".
        assert!(run(&mut sp, &frame(64, &[(2.6, -9.0)])).is_empty());
        assert!(run(&mut sp, &frame(200, &[(2.6, -9.0)])).is_empty());
        assert_eq!(run(&mut sp, &frame(400, &[(2.6, -9.0)])), vec![SpotterCall::Clear]);
    }

    #[test]
    fn no_flicker_at_edge() {
        let mut sp = Spotter::new();
        assert_eq!(run(&mut sp, &frame(0, &[(-2.5, 4.7)])), vec![SpotterCall::Left]);
        // Slightly beyond the car length but within the exit margin: stays "left".
        for t in 1..40 {
            assert!(run(&mut sp, &frame(t * 16, &[(-2.5, 5.1)])).is_empty());
        }
    }

    #[test]
    fn clear_only_after_announcement_and_off_when_slow() {
        let mut sp = Spotter::new();
        let mut f = frame(0, &[(-2.5, 0.0)]);
        f.speed_kmh = 5.0;
        assert!(run(&mut sp, &f).is_empty());
        assert_eq!(sp.state(), Side::Clear);
    }

    #[test]
    fn same_lane_car_is_not_overlap() {
        let mut sp = Spotter::new();
        assert!(run(&mut sp, &frame(0, &[(0.3, 3.0)])).is_empty());
    }

    #[test]
    fn closing_fast() {
        let mut sp = Spotter::new();
        run(&mut sp, &frame(0, &[(0.5, -35.0)]));
        let out = run(&mut sp, &frame(500, &[(0.5, -27.0)])); // 16 m/s = 57 km/h closing
        assert_eq!(out, vec![SpotterCall::ClosingFast]);
    }

    fn native(t: u64, side: NativeSide, near: &[(i32, f32)]) -> TelemetryFrame {
        let near_cars = near.iter().map(|(id, lon)| crate::games::provider::NearCar { id: *id, lon_m: *lon }).collect();
        TelemetryFrame { t_ms: t, speed_kmh: 150.0, player_car_id: 0, native_side: Some(side), near_cars, ..Default::default() }
    }

    #[test]
    fn native_side_drives_calls_and_radar() {
        let mut sp = Spotter::new();
        assert_eq!(run(&mut sp, &native(0, NativeSide::Left, &[(1, 1.0)])), vec![SpotterCall::Left]);
        assert!(sp.radar[0].overlap && sp.radar[0].lat < 0.0);
        assert_eq!(run(&mut sp, &native(16, NativeSide::ThreeWide, &[(1, 1.0), (2, -2.0)])), vec![SpotterCall::ThreeWide]);
        assert!(run(&mut sp, &native(32, NativeSide::Clear, &[(1, 12.0)])).is_empty());
        assert_eq!(run(&mut sp, &native(400, NativeSide::Clear, &[(1, 12.0)])), vec![SpotterCall::Clear]);
        // Game spotter inactive: no calls even with a car alongside.
        let mut sp = Spotter::new();
        assert!(run(&mut sp, &native(0, NativeSide::Off, &[(1, 0.0)])).is_empty());
    }

    #[test]
    fn native_closing_fast_from_track_distance() {
        let mut sp = Spotter::new();
        run(&mut sp, &native(0, NativeSide::Clear, &[(3, -35.0)]));
        assert_eq!(run(&mut sp, &native(500, NativeSide::Clear, &[(3, -27.0)])), vec![SpotterCall::ClosingFast]);
    }

    #[test]
    fn still_there() {
        let mut sp = Spotter::new();
        run(&mut sp, &frame(0, &[(-2.5, 0.0)]));
        let mut calls = Vec::new();
        for t in 1..=600 {
            calls.extend(run(&mut sp, &frame(t * 16, &[(-2.5, 0.0)])));
        }
        assert_eq!(calls.iter().filter(|c| **c == SpotterCall::StillThere).count(), 2);
    }
}
