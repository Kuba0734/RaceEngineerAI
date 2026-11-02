//! Fuel model: rolling average over clean laps and derived strategy numbers.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy)]
pub struct LapFuel {
    pub used_l: f32,
    pub lap_ms: i32,
    pub clean: bool,
}

#[derive(Debug, Default)]
pub struct FuelModel {
    laps: VecDeque<LapFuel>,
    lap_start_fuel: Option<f32>,
    pub refuelled_this_lap: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FuelStrategy {
    pub fuel_l: f32,
    /// Average per clean lap (or game estimate as fallback).
    pub per_lap_l: f32,
    pub from_clean_laps: u32,
    pub laps_possible: f32,
    pub laps_to_go: f32,
    pub fuel_needed_l: f32,
    pub refuel_l: f32,
    pub enough: bool,
    /// Litres per lap to save to finish without stopping (if within reach), else null.
    pub save_per_lap_l: Option<f32>,
    pub avg_lap_ms: i32,
}

impl FuelModel {
    pub fn reset(&mut self) {
        self.laps.clear();
        self.lap_start_fuel = None;
        self.refuelled_this_lap = false;
    }

    /// Watches the fuel level; refuelling (fuel goes up) taints the current lap.
    pub fn on_frame(&mut self, fuel_l: f32) {
        match self.lap_start_fuel {
            None => self.lap_start_fuel = Some(fuel_l),
            Some(start) if fuel_l > start + 0.5 => {
                self.refuelled_this_lap = true;
                self.lap_start_fuel = Some(fuel_l);
            }
            _ => {}
        }
    }

    pub fn on_lap_completed(&mut self, fuel_l: f32, lap_ms: i32, clean: bool, window: usize) {
        if let Some(start) = self.lap_start_fuel {
            let used = start - fuel_l;
            let ok = clean && !self.refuelled_this_lap && used > 0.05 && lap_ms > 0;
            self.laps.push_back(LapFuel { used_l: used.max(0.0), lap_ms, clean: ok });
            while self.laps.len() > 30 {
                self.laps.pop_front();
            }
        }
        let _ = window;
        self.lap_start_fuel = Some(fuel_l);
        self.refuelled_this_lap = false;
    }

    pub fn clean_laps(&self, window: usize) -> impl Iterator<Item = &LapFuel> {
        self.laps.iter().rev().filter(|l| l.clean).take(window.max(1))
    }

    pub fn average(&self, window: usize) -> Option<(f32, i32, u32)> {
        let v: Vec<&LapFuel> = self.clean_laps(window).collect();
        if v.is_empty() {
            return None;
        }
        let n = v.len() as f32;
        let fuel = v.iter().map(|l| l.used_l).sum::<f32>() / n;
        let lap = (v.iter().map(|l| l.lap_ms as f32).sum::<f32>() / n) as i32;
        Some((fuel, lap, v.len() as u32))
    }

    pub fn last_lap_used(&self) -> Option<f32> {
        self.laps.back().map(|l| l.used_l)
    }
}

/// Laps still to drive in a timed race, including the current one.
pub fn laps_to_go_timed(time_left_ms: f64, spline: f32, avg_lap_ms: i32) -> f32 {
    if avg_lap_ms <= 0 {
        return 0.0;
    }
    let avg = f64::from(avg_lap_ms);
    let rest_of_lap = f64::from(1.0 - spline.clamp(0.0, 1.0));
    let after = (time_left_ms - rest_of_lap * avg).max(0.0);
    // The race ends when the leader crosses the line after the timer: +1 full lap.
    (rest_of_lap + (after / avg).ceil()) as f32
}

#[allow(clippy::too_many_arguments)]
pub fn strategy(fuel_l: f32, per_lap_l: f32, clean: u32, avg_lap_ms: i32, laps_to_go: f32, reserve_laps: f32, reserve_l: f32, max_save_fraction: f32) -> FuelStrategy {
    let laps_possible = if per_lap_l > 0.0 { fuel_l / per_lap_l } else { 0.0 };
    let fuel_needed = laps_to_go * per_lap_l + reserve_laps * per_lap_l + reserve_l;
    let refuel = (fuel_needed - fuel_l).max(0.0);
    let enough = refuel <= 0.0;
    let save = if !enough && laps_to_go > 0.0 && per_lap_l > 0.0 {
        let allowed = (fuel_l - reserve_laps * per_lap_l - reserve_l) / laps_to_go;
        let save = per_lap_l - allowed;
        (save > 0.0 && save <= per_lap_l * max_save_fraction).then_some(save)
    } else {
        None
    };
    FuelStrategy {
        fuel_l,
        per_lap_l,
        from_clean_laps: clean,
        laps_possible,
        laps_to_go,
        fuel_needed_l: fuel_needed,
        refuel_l: refuel,
        enough,
        save_per_lap_l: save,
        avg_lap_ms,
    }
}

/// Start fuel for a race of `race_minutes`.
pub fn start_fuel(race_minutes: f32, per_lap_l: f32, avg_lap_ms: i32, formation_lap: bool, reserve_laps: f32, reserve_l: f32, max_fuel_l: f32) -> f32 {
    if avg_lap_ms <= 0 {
        return 0.0;
    }
    let laps = (race_minutes * 60_000.0 / avg_lap_ms as f32).ceil() + 1.0;
    start_fuel_laps(laps, per_lap_l, formation_lap, reserve_laps, reserve_l, max_fuel_l)
}

/// Start fuel for a race over a fixed number of laps.
pub fn start_fuel_laps(laps: f32, per_lap_l: f32, formation_lap: bool, reserve_laps: f32, reserve_l: f32, max_fuel_l: f32) -> f32 {
    let formation = if formation_lap { 1.0 } else { 0.0 };
    let f = (laps + formation + reserve_laps) * per_lap_l + reserve_l;
    if max_fuel_l > 0.0 {
        f.min(max_fuel_l)
    } else {
        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolling_average_skips_dirty_laps() {
        let mut m = FuelModel::default();
        let mut fuel = 60.0;
        m.on_frame(fuel);
        for (i, used) in [3.5, 2.9, 3.0, 2.8, 9.0].iter().enumerate() {
            fuel -= used;
            // lap 0 = out-lap, lap 4 = dirty (safety car)
            m.on_lap_completed(fuel, 108_000, i != 0 && i != 4, 5);
        }
        let (avg, lap, n) = m.average(5).unwrap();
        assert_eq!(n, 3);
        assert!((avg - 2.9).abs() < 0.01);
        assert_eq!(lap, 108_000);
    }

    #[test]
    fn refuel_taints_lap() {
        let mut m = FuelModel::default();
        m.on_frame(10.0);
        m.on_frame(50.0);
        m.on_lap_completed(47.0, 120_000, true, 5);
        assert!(m.average(5).is_none());
    }

    #[test]
    fn timed_race_laps() {
        // 10 min left, 100 s laps, at start of lap → 1 + ceil(500/100)=6
        assert_eq!(laps_to_go_timed(600_000.0, 0.0, 100_000), 6.0);
        assert!((laps_to_go_timed(600_000.0, 0.5, 100_000) - 6.5).abs() < 1e-4);
    }

    #[test]
    fn strategy_numbers() {
        let s = strategy(20.0, 3.0, 4, 100_000, 10.0, 1.0, 0.0, 0.1);
        assert!(!s.enough);
        assert!((s.refuel_l - 13.0).abs() < 1e-4);
        assert!(s.save_per_lap_l.is_none());
        // Close call: 31 L, 10 laps at 3.0 + 1 lap reserve = 33 L → save 0.2 L/lap.
        let s = strategy(31.0, 3.0, 4, 100_000, 10.0, 1.0, 0.0, 0.1);
        assert!((s.save_per_lap_l.unwrap() - 0.2).abs() < 1e-4);
    }

    #[test]
    fn start_fuel_calc() {
        // 60 min, 100 s laps → 36 + 1 = 37 laps + formation + 1 reserve = 39 × 3.0 = 117
        let f = start_fuel(60.0, 3.0, 100_000, true, 1.0, 0.0, 0.0);
        assert!((f - 117.0).abs() < 1e-3);
    }
}
