//! RaceState: single source of truth fed by the provider. Owns all derived models.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::fuel::{self, FuelModel, FuelStrategy};
use super::gaps::GapTracker;
use super::lapcompare::{self, Corner, CornerDelta, Sample, Trace};
use super::spotter::{Spotter, SpotterCall};
use crate::games::provider::*;
use crate::storage::settings::Settings;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LapRecord {
    pub lap: i32,
    pub time_ms: i32,
    pub sectors: [Option<i32>; 3],
    pub valid: bool,
    pub fuel_used_l: Option<f32>,
    pub position: i32,
    pub clean: bool,
    pub in_lap: bool,
    pub out_lap: bool,
    pub avg_pressure: [f32; 4],
    pub avg_core_temp: [f32; 4],
}

#[derive(Debug, Clone)]
pub enum RaceEvent {
    NewSession,
    LapCompleted(LapRecord),
    SectorCompleted { sector: usize, time_ms: i32 },
    PitEntry,
    PitExit { loss_s: Option<f32> },
    RivalPitEntry { car_id: i32 },
    RivalPitExit { car_id: i32 },
    TrackLimit { count: u32 },
    PenaltyReceived(Penalty),
    PenaltyCleared,
    FlagChanged { from: Flag, to: Flag },
    YellowSectorsChanged([bool; 3]),
    Mistake,
    PersonalBest { time_ms: i32 },
    SessionPhaseChanged(SessionPhase),
    Game(GameEvent),
}

#[derive(Debug, Clone, Default)]
pub struct RivalInfo {
    pub pit_stops: u32,
    pub last_pit_lap: Option<i32>,
    pub in_pits: bool,
    pub last_location: CarLocation,
}

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
}

pub struct RaceState {
    pub session: SessionInfo,
    pub frame: TelemetryFrame,
    pub has_frame: bool,
    pub cars: Vec<CarState>,
    pub secondary_connected: bool,
    pub spotter: Spotter,
    pub gaps: GapTracker,
    pub fuel: FuelModel,
    pub laps: Vec<LapRecord>,
    pub rivals: BTreeMap<i32, RivalInfo>,
    pub track_limits: u32,
    pub track_outline: Vec<[f32; 2]>,
    pub outline_version: u32,
    pub trace: Trace,
    pub reference: Option<Trace>,
    pub corners: Vec<Corner>,
    pub last_corner_deltas: Vec<CornerDelta>,
    /// Trace of the last completed lap (TC share per corner etc.).
    pub last_trace: Option<Trace>,
    pub pit_loss_s: Option<f32>,
    pub road_temp_history: VecDeque<(f64, f32)>,
    pub stint_start_lap: i32,
    pub stint_start_t: f64,
    pub race_start_position: Option<i32>,
    pub weather_at_start: Option<Weather>,
    // Internal edge detection.
    last_completed_laps: i32,
    last_sector_index: i32,
    sector_times: [Option<i32>; 3],
    lap_was_valid: bool,
    lap_out: bool,
    lap_in: bool,
    lap_dirty: bool,
    was_in_pit_lane: bool,
    pit_entry_t: Option<f64>,
    last_flag: Flag,
    last_yellow: [bool; 3],
    last_penalty: Option<Penalty>,
    last_phase: SessionPhase,
    mistake_cooldown_t: f64,
    off_track_since: Option<f64>,
    pressure_acc: ([f32; 4], [f32; 4], u32),
    outline_lap: Option<i32>,
    outline_tmp: Vec<[f32; 2]>,
}

impl Default for RaceState {
    fn default() -> Self {
        Self::new()
    }
}

impl RaceState {
    pub fn new() -> Self {
        Self {
            session: SessionInfo::default(),
            frame: TelemetryFrame::default(),
            has_frame: false,
            cars: Vec::new(),
            secondary_connected: false,
            spotter: Spotter::new(),
            gaps: GapTracker::default(),
            fuel: FuelModel::default(),
            laps: Vec::new(),
            rivals: BTreeMap::new(),
            track_limits: 0,
            track_outline: Vec::new(),
            outline_version: 0,
            trace: Trace::new(1.0),
            reference: None,
            corners: Vec::new(),
            last_corner_deltas: Vec::new(),
            last_trace: None,
            pit_loss_s: None,
            road_temp_history: VecDeque::new(),
            stint_start_lap: 0,
            stint_start_t: 0.0,
            race_start_position: None,
            weather_at_start: None,
            last_completed_laps: -1,
            last_sector_index: -1,
            sector_times: [None; 3],
            lap_was_valid: true,
            lap_out: false,
            lap_in: false,
            lap_dirty: false,
            was_in_pit_lane: false,
            pit_entry_t: None,
            last_flag: Flag::None,
            last_yellow: [false; 3],
            last_penalty: None,
            last_phase: SessionPhase::Unknown,
            mistake_cooldown_t: 0.0,
            off_track_since: None,
            pressure_acc: ([0.0; 4], [0.0; 4], 0),
            outline_lap: None,
            outline_tmp: Vec::new(),
        }
    }

    pub fn t_s(&self) -> f64 {
        self.frame.t_ms as f64 / 1000.0
    }

    pub fn track_m(&self) -> f32 {
        if self.session.track_length_m > 100.0 {
            self.session.track_length_m
        } else {
            // Without broadcasting: estimate from the outline once available.
            let l: f32 = self.track_outline.windows(2).map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt()).sum();
            if l > 500.0 {
                l
            } else {
                5000.0
            }
        }
    }

    /// Clears per-session data (keeps settings-derived things).
    fn reset_session(&mut self) {
        let session = std::mem::take(&mut self.session);
        let reference = self.reference.take();
        let corners = std::mem::take(&mut self.corners);
        let pit_loss = self.pit_loss_s;
        *self = Self::new();
        self.session = session;
        self.reference = reference;
        self.corners = corners;
        self.pit_loss_s = pit_loss;
    }

    pub fn on_session(&mut self, info: SessionInfo, events: &mut Vec<RaceEvent>) {
        let new_session = info.track != self.session.track
            || info.session_type != self.session.session_type
            || info.session_index != self.session.session_index
            || info.car_model != self.session.car_model;
        if info.phase != self.last_phase {
            self.last_phase = info.phase;
            events.push(RaceEvent::SessionPhaseChanged(info.phase));
        }
        let track_or_car_changed = info.track != self.session.track || info.car_model != self.session.car_model;
        self.session = info;
        if new_session {
            if track_or_car_changed {
                self.reference = None;
                self.corners.clear();
                self.pit_loss_s = None;
            }
            self.reset_session();
            events.push(RaceEvent::NewSession);
        }
    }

    pub fn on_cars(&mut self, cars: Vec<CarState>, events: &mut Vec<RaceEvent>) {
        let t = self.t_s();
        for c in &cars {
            if !c.is_player {
                self.gaps.update(c.id, c.laps, c.spline, t);
            }
            let r = self.rivals.entry(c.id).or_default();
            let in_pits = c.location.in_pits();
            if in_pits && !r.in_pits && !c.is_player {
                r.pit_stops += 1;
                r.last_pit_lap = Some(c.laps);
                events.push(RaceEvent::RivalPitEntry { car_id: c.id });
            } else if !in_pits && r.in_pits && !c.is_player {
                events.push(RaceEvent::RivalPitExit { car_id: c.id });
            }
            r.in_pits = in_pits;
            r.last_location = c.location;
        }
        self.cars = cars;
    }

    pub fn player_car(&self) -> Option<&CarState> {
        self.cars.iter().find(|c| c.is_player).or_else(|| self.cars.iter().find(|c| c.id == self.frame.player_car_id))
    }

    pub fn car(&self, id: i32) -> Option<&CarState> {
        self.cars.iter().find(|c| c.id == id)
    }

    pub fn on_frame(&mut self, f: TelemetryFrame, settings: &Settings, spotter_out: &mut Vec<SpotterCall>, events: &mut Vec<RaceEvent>) {
        let t = f.t_ms as f64 / 1000.0;
        let kind = self.session.session_type.kind();
        use crate::storage::settings::SessionKind;
        let sp = &settings.spotter;
        let spotter_active = f.status == SimStatus::Live
            && match kind {
                SessionKind::Practice => sp.in_practice,
                SessionKind::Qualifying => sp.in_qualifying,
                SessionKind::Race => sp.in_race,
            };
        self.spotter.update(&f, sp, spotter_active, spotter_out);

        if f.status != SimStatus::Live {
            self.frame = f;
            self.has_frame = true;
            return;
        }

        // Player timing (high rate).
        self.gaps.update(f.player_car_id, f.completed_laps, f.spline, t);
        self.fuel.on_frame(f.fuel_l);

        // Trace for lap comparison.
        if self.trace.bins.len() <= 1 {
            self.trace = Trace::new(self.track_m());
        }
        self.trace.record(f.spline, Sample { speed_kmh: f.speed_kmh, throttle: f.throttle, brake: f.brake, tc: f.tc_active, abs: f.abs_active });

        // Track outline from the first full lap.
        self.record_outline(&f);

        // Pressures/temps while at speed.
        if f.speed_kmh > 80.0 {
            for i in 0..4 {
                self.pressure_acc.0[i] += f.wheels[i].pressure_psi;
                self.pressure_acc.1[i] += f.wheels[i].core_temp_c;
            }
            self.pressure_acc.2 += 1;
        }

        // Pit lane edges.
        let in_pit_lane = f.in_pit_lane || f.in_pit_box;
        if in_pit_lane && !self.was_in_pit_lane {
            self.lap_in = true;
            self.pit_entry_t = Some(t);
            events.push(RaceEvent::PitEntry);
        } else if !in_pit_lane && self.was_in_pit_lane {
            self.lap_out = true;
            let loss = self.pit_entry_t.take().map(|t0| (t - t0) as f32);
            self.stint_start_lap = f.completed_laps;
            self.stint_start_t = t;
            events.push(RaceEvent::PitExit { loss_s: loss });
        }
        self.was_in_pit_lane = in_pit_lane;

        // Flags.
        if f.flag != self.last_flag {
            events.push(RaceEvent::FlagChanged { from: self.last_flag, to: f.flag });
            self.last_flag = f.flag;
        }
        if f.yellow_sectors != self.last_yellow {
            self.last_yellow = f.yellow_sectors;
            events.push(RaceEvent::YellowSectorsChanged(f.yellow_sectors));
        }
        if f.flag == Flag::Yellow || f.global_yellow || f.yellow_sectors.iter().any(|y| *y) {
            self.lap_dirty = true;
        }

        // Penalties.
        if f.penalty != self.last_penalty {
            match f.penalty {
                Some(p) => events.push(RaceEvent::PenaltyReceived(p)),
                None => events.push(RaceEvent::PenaltyCleared),
            }
            self.last_penalty = f.penalty;
        }

        // Track limits: lap becomes invalid while on track.
        if self.lap_was_valid && !f.lap_valid && !in_pit_lane && f.current_lap_ms > 2000 {
            self.track_limits += 1;
            events.push(RaceEvent::TrackLimit { count: self.track_limits });
        }
        self.lap_was_valid = f.lap_valid;

        // Mistakes: spin or long off-track excursion.
        let spinning = f.yaw_rate.abs() > 1.6 && f.speed_kmh > 40.0;
        if f.tyres_out >= 4 {
            self.off_track_since.get_or_insert(t);
        } else {
            self.off_track_since = None;
        }
        let long_off = self.off_track_since.is_some_and(|s| t - s > 1.0);
        if (spinning || long_off) && t > self.mistake_cooldown_t {
            self.mistake_cooldown_t = t + 20.0;
            self.lap_dirty = true;
            events.push(RaceEvent::Mistake);
        }

        // Sectors.
        if self.last_sector_index >= 0 && f.sector_index != self.last_sector_index && f.last_sector_ms > 0 {
            let s = self.last_sector_index.clamp(0, 2) as usize;
            if f.sector_index > self.last_sector_index {
                self.sector_times[s] = Some(f.last_sector_ms);
                events.push(RaceEvent::SectorCompleted { sector: s, time_ms: f.last_sector_ms });
            }
        }
        self.last_sector_index = f.sector_index;

        // Road temp history (1 sample / 30 s).
        if self.road_temp_history.back().is_none_or(|(ts, _)| t - ts >= 30.0) {
            self.road_temp_history.push_back((t, f.weather.road_temp_c));
            while self.road_temp_history.len() > 120 {
                self.road_temp_history.pop_front();
            }
        }

        if self.race_start_position.is_none() && f.completed_laps == 0 && f.position > 0 {
            self.race_start_position = Some(f.position);
        }
        if self.weather_at_start.is_none() {
            self.weather_at_start = Some(f.weather);
        }

        // Lap completion.
        if self.last_completed_laps >= 0 && f.completed_laps > self.last_completed_laps {
            self.complete_lap(&f, settings, events);
        }
        if self.last_completed_laps < 0 {
            self.stint_start_lap = f.completed_laps;
            self.stint_start_t = t;
        }
        self.last_completed_laps = f.completed_laps;
        self.frame = f;
        self.has_frame = true;
    }

    fn complete_lap(&mut self, f: &TelemetryFrame, settings: &Settings, events: &mut Vec<RaceEvent>) {
        let lap_ms = f.last_lap_ms;
        let mut sectors = self.sector_times;
        if let (Some(s1), Some(s2)) = (sectors[0], sectors[1]) {
            if lap_ms > s1 + s2 {
                sectors[2] = Some(lap_ms - s1 - s2);
            }
        }
        let valid = self.lap_was_valid && lap_ms > 0;
        let clean = !self.lap_out && !self.lap_in && !self.lap_dirty;
        let window = settings.strategy.clean_laps_window as usize;
        self.fuel.on_lap_completed(f.fuel_l, lap_ms, clean, window);
        let n = self.pressure_acc.2.max(1) as f32;
        let rec = LapRecord {
            lap: f.completed_laps,
            time_ms: lap_ms,
            sectors,
            valid,
            fuel_used_l: self.fuel.last_lap_used(),
            position: f.position,
            clean,
            in_lap: self.lap_in,
            out_lap: self.lap_out,
            avg_pressure: self.pressure_acc.0.map(|p| p / n),
            avg_core_temp: self.pressure_acc.1.map(|p| p / n),
        };
        let prev_best = self.laps.iter().filter(|l| l.valid).map(|l| l.time_ms).min();

        // Lap comparison: complete traces only.
        let fresh = Trace::new(self.track_m());
        let mut trace = std::mem::replace(&mut self.trace, fresh);
        trace.lap_ms = lap_ms;
        self.last_trace = (trace.coverage() > 0.9).then(|| trace.clone());
        if trace.coverage() > 0.9 && valid && clean {
            if let Some(r) = &self.reference {
                self.last_corner_deltas = lapcompare::compare(r, &trace, &self.corners);
            }
            let better = self.reference.as_ref().is_none_or(|r| lap_ms < r.lap_ms);
            if better {
                self.corners = lapcompare::detect_corners(&trace);
                self.reference = Some(trace);
            }
        } else {
            self.last_corner_deltas.clear();
        }

        if valid && prev_best.is_none_or(|b| lap_ms < b) && !self.laps.is_empty() {
            events.push(RaceEvent::PersonalBest { time_ms: lap_ms });
        }
        self.gaps.sample_lap(f.player_car_id, window.max(3));
        // Pit loss: in-lap(s) + out-lap compared with the normal pace.
        if rec.out_lap {
            let avg = self.avg_lap_ms(window);
            let pit_laps: Vec<i32> = self.laps.iter().rev().take_while(|l| l.in_lap && !l.out_lap).map(|l| l.time_ms).collect();
            if avg > 0 && !pit_laps.is_empty() {
                let n = pit_laps.len() as i32 + 1;
                let loss = (pit_laps.iter().sum::<i32>() + lap_ms - n * avg) as f32 / 1000.0;
                if (5.0..120.0).contains(&loss) {
                    self.pit_loss_s = Some(loss);
                }
            }
        }
        self.laps.push(rec.clone());
        events.push(RaceEvent::LapCompleted(rec));

        self.sector_times = [None; 3];
        self.lap_out = false;
        self.lap_in = self.was_in_pit_lane;
        self.lap_dirty = false;
        self.lap_was_valid = true;
        self.pressure_acc = ([0.0; 4], [0.0; 4], 0);
    }

    fn record_outline(&mut self, f: &TelemetryFrame) {
        if !self.track_outline.is_empty() || f.in_pit_lane {
            return;
        }
        match self.outline_lap {
            None => {
                if f.spline < 0.02 {
                    self.outline_lap = Some(f.completed_laps);
                    self.outline_tmp.clear();
                }
            }
            Some(lap) => {
                if f.completed_laps > lap {
                    if self.outline_tmp.len() > 50 {
                        self.track_outline = std::mem::take(&mut self.outline_tmp);
                        self.outline_version += 1;
                    }
                    self.outline_lap = None;
                    return;
                }
                let p = [f.pos[0], f.pos[2]];
                if self.outline_tmp.last().is_none_or(|l| (l[0] - p[0]).powi(2) + (l[1] - p[1]).powi(2) > 100.0) {
                    self.outline_tmp.push(p);
                }
            }
        }
    }

    pub fn on_game_event(&mut self, e: GameEvent, events: &mut Vec<RaceEvent>) {
        events.push(RaceEvent::Game(e));
    }

    // ----- Derived views (used by triggers, tools and the frontend) -----

    pub fn avg_lap_ms(&self, window: usize) -> i32 {
        if let Some((_, lap, _)) = self.fuel.average(window) {
            return lap;
        }
        let v: Vec<i32> = self.laps.iter().rev().filter(|l| l.valid && !l.in_lap && !l.out_lap).take(window).map(|l| l.time_ms).collect();
        if v.is_empty() {
            if self.frame.best_lap_ms > 0 {
                self.frame.best_lap_ms
            } else {
                0
            }
        } else {
            (v.iter().map(|x| *x as i64).sum::<i64>() / v.len() as i64) as i32
        }
    }

    pub fn fuel_strategy(&self, settings: &Settings) -> FuelStrategy {
        let st = &settings.strategy;
        let window = st.clean_laps_window as usize;
        let (per_lap, clean) = match self.fuel.average(window) {
            Some((f, _, n)) => (f, n),
            None => (self.frame.game_fuel_per_lap_l, 0),
        };
        let avg = self.avg_lap_ms(window);
        let laps_to_go = fuel::laps_to_go_timed(self.frame.session_time_left_ms, self.frame.spline, avg);
        fuel::strategy(self.frame.fuel_l, per_lap, clean, avg, laps_to_go, st.fuel_reserve_laps, st.fuel_reserve_liters, st.max_save_fraction)
    }

    pub fn player_position(&self) -> i32 {
        self.player_car().map(|c| c.position).filter(|p| *p > 0).unwrap_or(self.frame.position)
    }

    pub fn standings(&self, trend_window: usize) -> Vec<StandingRow> {
        let me = self.frame.player_car_id;
        let session_best = self.cars.iter().filter_map(|c| c.best_lap_ms).min();
        let mut rows: Vec<StandingRow> = self
            .cars
            .iter()
            .map(|c| {
                let r = self.rivals.get(&c.id);
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
                }
            })
            .collect();
        let _ = trend_window;
        rows.sort_by_key(|r| if r.position > 0 { r.position } else { i32::MAX });
        rows
    }

    /// Car directly ahead/behind in the race order.
    pub fn neighbour(&self, offset: i32) -> Option<&CarState> {
        let pos = self.player_position();
        self.cars.iter().find(|c| c.position == pos + offset && !c.is_player)
    }

    pub fn road_temp_trend_c_per_10min(&self) -> Option<f32> {
        let (t0, v0) = *self.road_temp_history.iter().rev().find(|(t, _)| self.t_s() - t >= 540.0)?;
        let (t1, v1) = *self.road_temp_history.back()?;
        let dt = (t1 - t0) as f32;
        (dt > 0.0).then(|| (v1 - v0) / dt * 600.0)
    }

    pub fn best_valid_lap(&self) -> Option<&LapRecord> {
        self.laps.iter().filter(|l| l.valid).min_by_key(|l| l.time_ms)
    }
}
