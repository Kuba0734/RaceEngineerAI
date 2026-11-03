//! Pit stop prediction: where the player rejoins and undercut risk.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::state::RaceState;
use crate::storage::settings::Settings;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PitPrediction {
    pub pit_loss_s: f32,
    pub pit_loss_measured: bool,
    pub current_position: i32,
    pub exit_position: i32,
    /// Car directly ahead after the stop.
    pub exit_behind_car: Option<i32>,
    /// Car directly behind after the stop.
    pub exit_ahead_of_car: Option<i32>,
    /// Gap to the car ahead after rejoining (s).
    pub exit_gap_ahead_s: Option<f32>,
    pub exit_gap_behind_s: Option<f32>,
    pub in_laps: u32,
    pub refuel_l: f32,
}

pub fn pit_loss(state: &RaceState, settings: &Settings) -> (f32, bool) {
    match state.pit_loss_s {
        Some(l) => (l, true),
        None => (settings.strategy.default_pit_loss_s, false),
    }
}

/// Predicts the rejoin position if the player pits `in_laps` laps from now.
/// `extra_loss_s` lets callers model longer stops (tyres + fuel).
pub fn predict(state: &RaceState, settings: &Settings, in_laps: u32, extra_loss_s: f32) -> PitPrediction {
    let (loss, measured) = pit_loss(state, settings);
    let loss = loss + extra_loss_s;
    let me = state.frame.player_car_id;
    let pos = state.player_position();
    // Gaps of all other cars at the player's position now; trends project them `in_laps` ahead.
    let mut rel: Vec<(i32, f32)> = state
        .cars
        .iter()
        .filter(|c| state.is_rival(c) && !c.location.in_pits())
        .filter_map(|c| {
            let g = state.gaps.gap(me, c.id)? as f32;
            let trend = state.gaps.trend(c.id).unwrap_or(0.0) as f32;
            // trend is change of (other − me) per lap; sign handled by gap convention.
            Some((c.id, g + trend * in_laps as f32))
        })
        .collect();
    // After the stop the player is `loss` seconds further back: every car with gap in (-loss, 0) passes.
    let passed = rel.iter().filter(|(_, g)| *g < 0.0 && *g > -loss).count() as i32;
    let exit_position = (pos + passed).max(1);
    // Relative to the rejoin point: shift by loss.
    for r in &mut rel {
        r.1 += loss;
    }
    let ahead = rel.iter().filter(|(_, g)| *g >= 0.0).min_by(|a, b| a.1.total_cmp(&b.1)).copied();
    let behind = rel.iter().filter(|(_, g)| *g < 0.0).max_by(|a, b| a.1.total_cmp(&b.1)).copied();
    PitPrediction {
        pit_loss_s: loss,
        pit_loss_measured: measured,
        current_position: pos,
        exit_position,
        exit_behind_car: ahead.map(|a| a.0),
        exit_ahead_of_car: behind.map(|b| b.0),
        exit_gap_ahead_s: ahead.map(|a| a.1),
        exit_gap_behind_s: behind.map(|b| -b.1),
        in_laps,
        refuel_l: state.fuel_strategy(settings).refuel_l,
    }
}

/// Undercut threat: a car within `threshold_s` (ahead or behind) just entered the pits.
pub fn is_undercut_threat(state: &RaceState, settings: &Settings, car_id: i32) -> bool {
    let me = state.frame.player_car_id;
    state.gaps.gap(me, car_id).is_some_and(|g| g.abs() < f64::from(settings.strategy.undercut_threshold_s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::provider::{CarLocation, CarState};

    #[test]
    fn exit_position_counts_cars_within_loss() {
        let mut s = RaceState::new();
        let settings = Settings::default();
        // Player id 0 at P2; cars behind at 5 s, 12 s and 30 s.
        let mk = |id, pos| CarState { id, position: pos, location: CarLocation::Track, is_player: id == 0, ..Default::default() };
        s.cars = vec![mk(0, 2), mk(1, 1), mk(2, 3), mk(3, 4), mk(4, 5)];
        s.frame.player_car_id = 0;
        let lap = 0.01; // 100 s lap
        let mut t = 0.0;
        while t < 60.0 {
            for (id, off) in [(0, 0.5), (1, 0.53), (2, 0.45), (3, 0.38), (4, 0.2)] {
                let p: f64 = off + lap * t;
                s.gaps.update(id, p.floor() as i32, p.fract() as f32, t);
            }
            t += 0.1;
        }
        let p = predict(&s, &settings, 0, 0.0);
        assert_eq!(p.exit_position, 4, "{p:?}");
        assert_eq!(p.exit_behind_car, Some(3));
        assert_eq!(p.exit_ahead_of_car, Some(4));
    }
}
