//! Views of the race for the windows, the crew and the AI: standings, class summaries, the
//! relative box and the race report. Read-only, built from `RaceState`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::irating;
use super::state::RaceState;
use crate::games::provider::*;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StandingRow {
    pub id: i32,
    pub position: i32,
    pub race_number: i32,
    pub driver: String,
    pub car: String,
    pub brand: String,
    pub laps: i32,
    /// Gap to player in seconds (+ = ahead), position-based timing.
    pub gap_to_player_s: Option<f32>,
    pub trend_s_per_lap: Option<f32>,
    pub last_lap_ms: Option<i32>,
    pub best_lap_ms: Option<i32>,
    pub last_splits: [Option<i32>; 3],
    pub best_splits: [Option<i32>; 3],
    pub in_pits: bool,
    pub pit_stops: u32,
    pub tyre_age_laps: Option<i32>,
    pub is_player: bool,
    pub is_session_best_lap: bool,
    pub class_id: i32,
    pub class_name: String,
    #[ts(optional)]
    pub class_color: Option<String>,
    pub class_position: i32,
    pub number_text: String,
    #[ts(optional)]
    pub irating: Option<i32>,
    /// Estimated iRating change if the race ended now (races, all cars rated).
    #[ts(optional)]
    pub irating_change: Option<f32>,
    #[ts(optional)]
    pub license: Option<String>,
    #[ts(optional)]
    pub license_color: Option<String>,
    #[ts(optional)]
    pub start_position: Option<i32>,
    /// Race: gap to the class leader; otherwise difference of best laps (s).
    #[ts(optional)]
    pub gap_to_class_leader_s: Option<f32>,
    /// Same, to the car ahead in the class.
    #[ts(optional)]
    pub interval_s: Option<f32>,
    /// Last sector times compared with the player's (s, + = slower than you).
    pub sector_delta_to_player_s: [Option<f32>; 3],
    pub is_class_best_lap: bool,
    pub last_is_personal_best: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ClassSummary {
    pub id: i32,
    pub name: String,
    #[ts(optional)]
    pub color: Option<String>,
    pub cars: i32,
    #[ts(optional)]
    pub sof: Option<i32>,
}

/// End-of-race numbers for the report and the summary call.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RaceReport {
    #[ts(optional)]
    pub irating: Option<i32>,
    #[ts(optional)]
    pub irating_change: Option<f32>,
    #[ts(optional)]
    pub license: Option<String>,
    #[ts(optional)]
    pub incidents: Option<i32>,
    #[ts(optional)]
    pub incident_limit: Option<i32>,
    #[ts(optional)]
    pub corners: Option<i32>,
    /// Corners driven per incident point (higher = cleaner).
    #[ts(optional)]
    pub corners_per_incident: Option<f32>,
}

/// One line of the relative: cars around the player on track.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RelativeRow {
    pub id: i32,
    pub position: i32,
    pub class_position: i32,
    pub number_text: String,
    pub driver: String,
    #[ts(optional)]
    pub class_color: Option<String>,
    #[ts(optional)]
    pub license: Option<String>,
    #[ts(optional)]
    pub license_color: Option<String>,
    #[ts(optional)]
    pub irating: Option<i32>,
    /// Time gap on track (s, + = ahead).
    pub gap_s: f32,
    /// Laps ahead (+) or behind (−) in the race: colours lapping / lapped cars.
    pub laps_ahead: i32,
    pub in_pits: bool,
    pub is_player: bool,
}

impl RaceState {
    pub fn standings(&self, trend_window: usize) -> Vec<StandingRow> {
        let _ = trend_window;
        let me = self.frame.player_car_id;
        let race = self.session.session_type == SessionType::Race;
        let player = self.player_car();
        let mut cars: Vec<&CarState> = self.cars.iter().collect();
        cars.sort_by_key(|c| if c.position > 0 { c.position } else { i32::MAX });
        // Per class: order, best lap, estimated iRating changes.
        let mut classes: BTreeMap<i32, Vec<&CarState>> = BTreeMap::new();
        for c in &cars {
            classes.entry(c.class_id).or_default().push(c);
        }
        let mut class_pos: BTreeMap<i32, i32> = BTreeMap::new();
        let mut ir_change: BTreeMap<i32, f32> = BTreeMap::new();
        let mut gap_leader: BTreeMap<i32, f32> = BTreeMap::new();
        let mut interval: BTreeMap<i32, f32> = BTreeMap::new();
        let mut class_best: BTreeMap<i32, i32> = BTreeMap::new();
        for (cid, list) in &classes {
            for (k, c) in list.iter().enumerate() {
                class_pos.insert(c.id, if c.class_position > 0 { c.class_position } else { k as i32 + 1 });
            }
            if let Some(b) = list.iter().filter_map(|c| c.best_lap_ms).min() {
                class_best.insert(*cid, b);
            }
            // iRating estimate only when every car of the class has a rating.
            let ratings: Option<Vec<f64>> = list.iter().map(|c| c.irating.map(f64::from)).collect();
            if let Some(r) = ratings.filter(|r| r.len() > 1 && race) {
                for (c, d) in list.iter().zip(irating::changes(&r)) {
                    ir_change.insert(c.id, d as f32);
                }
            }
            // Gap to the class leader and to the car ahead: official race gaps, else best laps.
            let key = |c: &CarState| -> Option<f32> { if race { c.gap_to_leader_s } else { c.best_lap_ms.map(|ms| ms as f32 / 1000.0) } };
            let lead = list.first().and_then(|c| key(c));
            let mut prev: Option<f32> = None;
            for c in list {
                if let (Some(v), Some(l)) = (key(c), lead) {
                    gap_leader.insert(c.id, v - l);
                    if let Some(p) = prev {
                        interval.insert(c.id, v - p);
                    }
                }
                prev = key(c);
            }
        }
        let session_best = self.cars.iter().filter_map(|c| c.best_lap_ms).min();
        cars.iter()
            .map(|c| {
                let r = self.rivals.get(&c.id);
                let sector_delta = std::array::from_fn(|k| {
                    let mine = player.and_then(|p| p.last_splits[k])?;
                    Some((c.last_splits[k]? - mine) as f32 / 1000.0)
                });
                StandingRow {
                    id: c.id,
                    position: c.position,
                    race_number: c.race_number,
                    driver: c.driver.clone(),
                    car: c.car_model.clone(),
                    brand: c.car_brand.clone(),
                    laps: c.laps,
                    gap_to_player_s: if c.is_player { Some(0.0) } else { self.gaps.gap(me, c.id).map(|g| g as f32) },
                    trend_s_per_lap: if c.is_player { None } else { self.gaps.trend(c.id).map(|t| t as f32) },
                    last_lap_ms: c.last_lap_ms,
                    best_lap_ms: c.best_lap_ms,
                    last_splits: c.last_splits,
                    best_splits: c.best_splits,
                    in_pits: c.location.in_pits(),
                    pit_stops: r.map(|r| r.pit_stops).unwrap_or(0),
                    tyre_age_laps: r.and_then(|r| r.last_pit_lap).map(|l| c.laps - l).or(Some(c.laps)),
                    is_player: c.is_player,
                    is_session_best_lap: c.best_lap_ms.is_some() && c.best_lap_ms == session_best,
                    class_id: c.class_id,
                    class_name: c.class_name.clone(),
                    class_color: c.class_color.clone(),
                    class_position: class_pos.get(&c.id).copied().unwrap_or(0),
                    number_text: if c.number_text.is_empty() { c.race_number.to_string() } else { c.number_text.clone() },
                    irating: c.irating,
                    irating_change: ir_change.get(&c.id).map(|d| d.round()),
                    license: c.license.clone(),
                    license_color: c.license_color.clone(),
                    start_position: r.and_then(|r| r.start_position),
                    gap_to_class_leader_s: gap_leader.get(&c.id).copied(),
                    interval_s: interval.get(&c.id).copied(),
                    sector_delta_to_player_s: if c.is_player { [None; 3] } else { sector_delta },
                    is_class_best_lap: c.best_lap_ms.is_some() && c.best_lap_ms == class_best.get(&c.class_id).copied(),
                    last_is_personal_best: c.last_lap_ms.is_some() && c.last_lap_ms == c.best_lap_ms,
                }
            })
            .collect()
    }

    /// Per class: name, colour, cars, strength of field.
    pub fn class_summaries(&self) -> Vec<ClassSummary> {
        let mut m: BTreeMap<i32, Vec<&CarState>> = BTreeMap::new();
        for c in &self.cars {
            m.entry(c.class_id).or_default().push(c);
        }
        m.into_values()
            .map(|list| {
                let ratings: Vec<f64> = list.iter().filter_map(|c| c.irating.map(f64::from)).collect();
                ClassSummary {
                    id: list[0].class_id,
                    name: list[0].class_name.clone(),
                    color: list[0].class_color.clone(),
                    cars: list.len() as i32,
                    sof: irating::sof(&ratings).map(|v| v.round() as i32),
                }
            })
            .collect()
    }

    /// Cars around the player on track, ordered from furthest ahead to furthest behind, with the
    /// time gap on track (from the game's estimated times).
    pub fn relative(&self, each_side: usize) -> Vec<RelativeRow> {
        let Some(me) = self.player_car() else { return Vec::new() };
        let Some(my_t) = me.est_time_s else { return Vec::new() };
        let lap = self.session.est_lap_s.filter(|l| *l > 10.0).unwrap_or(self.avg_lap_ms(5) as f32 / 1000.0);
        if lap <= 10.0 {
            return Vec::new();
        }
        let my_dist = me.laps as f32 + me.spline;
        let mut rows: Vec<RelativeRow> = self
            .cars
            .iter()
            .filter(|c| c.location != CarLocation::Unknown)
            .filter_map(|c| {
                let t = c.est_time_s?;
                // + = ahead on track; wrapped into half a lap either way.
                let mut d = t - my_t;
                d -= (d / lap).round() * lap;
                let laps_ahead = ((c.laps as f32 + c.spline) - my_dist).round() as i32;
                Some(RelativeRow {
                    id: c.id,
                    position: self.rank(c),
                    class_position: c.class_position,
                    number_text: if c.number_text.is_empty() { c.race_number.to_string() } else { c.number_text.clone() },
                    driver: c.driver.clone(),
                    class_color: c.class_color.clone(),
                    license: c.license.clone(),
                    license_color: c.license_color.clone(),
                    irating: c.irating,
                    gap_s: if c.is_player { 0.0 } else { d },
                    laps_ahead: if c.is_player { 0 } else { laps_ahead },
                    in_pits: c.location.in_pits(),
                    is_player: c.is_player,
                })
            })
            .collect();
        rows.sort_by(|a, b| b.gap_s.total_cmp(&a.gap_s));
        let Some(i) = rows.iter().position(|r| r.is_player) else { return rows };
        let from = i.saturating_sub(each_side);
        rows.into_iter().skip(from).take(i - from + 1 + each_side).collect()
    }

    /// Race report numbers of the player: estimated iRating change, incidents, incidents per
    /// corner (the basis of iRacing's safety rating; the rating change itself is not published).
    pub fn report(&self) -> RaceReport {
        let me = self.standings(5).into_iter().find(|r| r.is_player);
        let laps = self.laps.len() as i32;
        let corners = self.session.track_turns.map(|t| t * laps).filter(|c| *c > 0);
        let incidents = self.frame.incidents;
        RaceReport {
            irating: me.as_ref().and_then(|r| r.irating),
            irating_change: me.as_ref().and_then(|r| r.irating_change),
            license: me.as_ref().and_then(|r| r.license.clone()),
            incidents,
            incident_limit: self.session.incident_limit,
            corners,
            corners_per_incident: match (corners, incidents) {
                (Some(c), Some(i)) if i > 0 => Some(c as f32 / i as f32),
                _ => None,
            },
        }
    }
}
