//! Fuel plan for a race before it starts (garage / setup screen): start fuel, stops and
//! refuel amounts from the consumption of earlier sessions with the same car and track.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Consumption measured in earlier sessions (same game, car and track).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FuelHistory {
    pub per_lap_l: f32,
    pub lap_ms: u32,
    pub laps: u32,
    pub sessions: u32,
}

/// Where the consumption per lap comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum FuelSource {
    /// Set by hand on the overlay page.
    Manual,
    /// Clean laps of the current session.
    Session,
    /// Earlier sessions with this car and track.
    History,
    /// The game's own estimate.
    Game,
    None,
}

/// Where the race length comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RaceLengthSource {
    /// Laps set by hand.
    ManualLaps,
    /// Race session of this event, by laps.
    SessionLaps,
    /// Race session of this event, by time.
    SessionTime,
    /// "Next race" minutes from the strategy settings.
    Settings,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FuelPlanView {
    pub per_lap_l: f32,
    pub source: FuelSource,
    #[ts(optional)]
    pub history: Option<FuelHistory>,
    /// Average lap time used for timed races (ms, 0 = unknown).
    pub lap_ms: u32,
    /// Laps of the race (incl. the extra lap of a timed race), 0 = unknown.
    pub race_laps: f32,
    pub race_source: RaceLengthSource,
    /// Race minutes for timed races.
    #[ts(optional)]
    pub race_minutes: Option<f32>,
    pub formation_lap: bool,
    pub reserve_l: f32,
    /// Fuel for the whole race incl. formation lap and reserve.
    pub total_l: f32,
    /// What to fill in at the start (capped by the tank).
    pub start_l: f32,
    pub tank_l: f32,
    pub stops: u32,
    /// Fuel to add at each stop.
    pub refuel_per_stop_l: f32,
    /// Fuel in the car right now.
    pub fuel_now_l: f32,
}

pub struct PlanInput {
    pub manual_per_lap_l: f32,
    pub session: Option<(f32, i32)>,
    pub history: Option<FuelHistory>,
    pub game_per_lap_l: f32,
    pub est_lap_s: Option<f32>,
    pub manual_laps: u32,
    pub session_race_laps: Option<i32>,
    pub session_race_time_s: Option<f64>,
    pub settings_minutes: f32,
    pub formation_lap: bool,
    pub reserve_laps: f32,
    pub reserve_liters: f32,
    pub tank_l: f32,
    pub fuel_now_l: f32,
}

pub fn plan(i: &PlanInput) -> FuelPlanView {
    let (per_lap_l, source) = if i.manual_per_lap_l > 0.0 {
        (i.manual_per_lap_l, FuelSource::Manual)
    } else if let Some((l, _)) = i.session.filter(|(l, _)| *l > 0.0) {
        (l, FuelSource::Session)
    } else if let Some(h) = i.history {
        (h.per_lap_l, FuelSource::History)
    } else if i.game_per_lap_l > 0.0 {
        (i.game_per_lap_l, FuelSource::Game)
    } else {
        (0.0, FuelSource::None)
    };
    let lap_ms = i
        .session
        .map(|(_, ms)| ms.max(0) as u32)
        .filter(|ms| *ms > 0)
        .or(i.history.map(|h| h.lap_ms).filter(|ms| *ms > 0))
        .or(i.est_lap_s.map(|s| (s * 1000.0) as u32))
        .unwrap_or(0);
    // Timed race: laps the leader drives in that time, plus the lap after the timer.
    let timed = |minutes: f32| if lap_ms > 0 { (minutes * 60_000.0 / lap_ms as f32).ceil() + 1.0 } else { 0.0 };
    let (race_laps, race_source, race_minutes) = if i.manual_laps > 0 {
        (i.manual_laps as f32, RaceLengthSource::ManualLaps, None)
    } else if let Some(l) = i.session_race_laps.filter(|l| *l > 0) {
        (l as f32, RaceLengthSource::SessionLaps, None)
    } else if let Some(t) = i.session_race_time_s.filter(|t| *t > 0.0) {
        let m = (t / 60.0) as f32;
        (timed(m), RaceLengthSource::SessionTime, Some(m))
    } else {
        (timed(i.settings_minutes), RaceLengthSource::Settings, Some(i.settings_minutes))
    };
    let formation = if i.formation_lap { 1.0 } else { 0.0 };
    let reserve_l = i.reserve_laps * per_lap_l + i.reserve_liters;
    let total_l = (race_laps + formation) * per_lap_l + reserve_l;
    let (start_l, stops, refuel) = if i.tank_l > 0.0 && total_l > i.tank_l {
        let missing = total_l - i.tank_l;
        let stops = (missing / i.tank_l).ceil().max(1.0);
        (i.tank_l, stops as u32, missing / stops)
    } else {
        (total_l, 0, 0.0)
    };
    FuelPlanView {
        per_lap_l,
        source,
        history: i.history,
        lap_ms,
        race_laps,
        race_source,
        race_minutes,
        formation_lap: i.formation_lap,
        reserve_l,
        total_l,
        start_l,
        tank_l: i.tank_l,
        stops,
        refuel_per_stop_l: refuel,
        fuel_now_l: i.fuel_now_l,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> PlanInput {
        PlanInput {
            manual_per_lap_l: 0.0,
            session: None,
            history: Some(FuelHistory { per_lap_l: 3.0, lap_ms: 100_000, laps: 30, sessions: 3 }),
            game_per_lap_l: 0.0,
            est_lap_s: None,
            manual_laps: 0,
            session_race_laps: None,
            session_race_time_s: Some(3600.0),
            settings_minutes: 60.0,
            formation_lap: true,
            reserve_laps: 1.0,
            reserve_liters: 0.0,
            tank_l: 120.0,
            fuel_now_l: 20.0,
        }
    }

    #[test]
    fn timed_race_from_history() {
        let p = plan(&input());
        assert_eq!(p.source, FuelSource::History);
        // 60 min / 100 s = 36 laps + the lap after the timer.
        assert_eq!(p.race_laps, 37.0);
        // (37 + formation 1) × 3 + reserve 3 = 117 L, fits into the tank.
        assert!((p.total_l - 117.0).abs() < 0.01 && p.stops == 0 && (p.start_l - 117.0).abs() < 0.01);
    }

    #[test]
    fn long_race_needs_stops_and_manual_values_win() {
        let mut i = input();
        i.session_race_laps = Some(80);
        i.manual_per_lap_l = 3.5;
        let p = plan(&i);
        assert_eq!((p.source, p.race_source), (FuelSource::Manual, RaceLengthSource::SessionLaps));
        // (80 + 1) × 3.5 + 3.5 = 287 L: tank 120 + two stops of 83.5 L.
        assert_eq!(p.stops, 2);
        assert!((p.start_l - 120.0).abs() < 0.01 && (p.refuel_per_stop_l - 83.5).abs() < 0.01);
    }

    #[test]
    fn nothing_known() {
        let mut i = input();
        i.history = None;
        let p = plan(&i);
        assert_eq!(p.source, FuelSource::None);
        assert_eq!(p.total_l, 0.0);
    }
}
