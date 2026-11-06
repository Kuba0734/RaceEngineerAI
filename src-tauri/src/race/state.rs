//! RaceState: single source of truth fed by the provider. Owns all derived models.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::fuel::{self, FuelModel, FuelStrategy};
use super::gaps::GapTracker;
use super::lapcompare::{self, Corner, CornerDelta, Sample, Trace};
use super::spotter::{Spotter, SpotterCall};
pub use super::views::{ClassSummary, RaceReport, RelativeRow, StandingRow};
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
    StartLight(StartLight),
    /// The player took the chequered flag (crossed the line after it, or went into the pits).
    Finished,
}

#[derive(Debug, Clone, Default)]
pub struct RivalInfo {
    pub pit_stops: u32,
    pub last_pit_lap: Option<i32>,
    pub in_pits: bool,
    pub last_location: CarLocation,
    /// First position seen in the race (grid).
    pub start_position: Option<i32>,
    /// Session time (s) the car went off track / became slow on track.
    pub off_since: Option<f64>,
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
    /// What the running game provides (None = everything, e.g. demo/replay).
    pub caps: Option<crate::games::registry::Capabilities>,
    /// Own stops in the pit box this session (for mandatory stops from the settings).
    pub own_stops: u32,
    /// Lap fraction of the pit lane entry (speed limit line), learned from cars driving in.
    pub pit_entry_pct: Option<f32>,
    /// Consumption of earlier sessions with this car and track (loaded from the database).
    pub fuel_history: Option<crate::race::fuel_plan::FuelHistory>,
    /// Test mode: raw values of the game and counters for the data rates.
    pub raw_values: Vec<(String, String)>,
    pub frames_seen: u64,
    pub cars_seen: u64,
    /// Below this speed a car on the racing line counts as stopped (km/h, from the settings).
    pub stopped_kmh: f32,
    was_in_box: bool,
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
    last_start_light: StartLight,
    /// Chequered flag shown while the player is still on the last lap (iRacing shows it when the
    /// leader finishes): the finish counts at the player's next line crossing.
    finish_pending: bool,
    finished: bool,
    mistake_cooldown_t: f64,
    off_track_since: Option<f64>,
    pressure_acc: ([f32; 4], [f32; 4], u32),
    outline_lap: Option<i32>,
    outline_tmp: Vec<[f32; 2]>,
    /// Track outline built from every car: one point per spline bin.
    outline_bins: Vec<Option<[f32; 2]>>,
    outline_filled: usize,
}

/// Lap fraction around the line within which a chequered flag means "finished now".
const FINISH_AT_LINE: f32 = 0.03;

fn is_multiclass(cars: &[CarState]) -> bool {
    let mut ids = cars.iter().map(|c| c.class_id);
    let first = ids.next();
    ids.any(|id| Some(id) != first)
}

/// Resolution of the outline built from all cars (bins along the spline).
const OUTLINE_BINS: usize = 600;

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
            caps: None,
            own_stops: 0,
            pit_entry_pct: None,
            fuel_history: None,
            raw_values: Vec::new(),
            frames_seen: 0,
            cars_seen: 0,
            stopped_kmh: 30.0,
            was_in_box: false,
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
            last_start_light: StartLight::Off,
            finish_pending: false,
            finished: false,
            mistake_cooldown_t: 0.0,
            off_track_since: None,
            pressure_acc: ([0.0; 4], [0.0; 4], 0),
            outline_lap: None,
            outline_tmp: Vec::new(),
            outline_bins: vec![None; OUTLINE_BINS],
            outline_filled: 0,
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
        let pit_entry = self.pit_entry_pct;
        let caps = self.caps.take();
        let outline = (std::mem::take(&mut self.track_outline), std::mem::take(&mut self.outline_bins), self.outline_filled, self.outline_version);
        *self = Self::new();
        (self.track_outline, self.outline_bins, self.outline_filled, self.outline_version) = outline;
        self.session = session;
        self.reference = reference;
        self.corners = corners;
        self.pit_loss_s = pit_loss;
        self.pit_entry_pct = pit_entry;
        self.caps = caps;
    }

    /// Capability check for data the game may not deliver (unknown game → available).
    pub fn has(&self, name: &str) -> bool {
        self.caps.as_ref().is_none_or(|c| crate::games::registry::has_capability(c, name))
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
        // New map coordinates (exact map replaced an estimated one): the outline starts over.
        if info.map_version != self.session.map_version && !self.session.track.is_empty() {
            self.outline_bins = vec![None; OUTLINE_BINS];
            self.outline_filled = 0;
            self.track_outline.clear();
            self.outline_version += 1;
        }
        let track_changed = info.track != self.session.track;
        let track_or_car_changed = track_changed || info.car_model != self.session.car_model;
        self.session = info;
        if new_session {
            if track_changed {
                self.track_outline.clear();
                self.outline_bins = vec![None; OUTLINE_BINS];
                self.outline_filled = 0;
                self.outline_version += 1;
            }
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
        self.outline_from_cars(&cars);
        let multiclass = is_multiclass(&cars);
        let running = self.race_running();
        let mut pit_entry = None;
        for c in &cars {
            if !c.is_player {
                match c.progress {
                    Some(p) => self.gaps.update_progress(c.id, p, t),
                    None => self.gaps.update(c.id, c.laps, c.spline, t),
                }
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
            // Onto pit road from the track: that spot is the pit entry line.
            if c.location == CarLocation::PitLane && matches!(r.last_location, CarLocation::Track | CarLocation::PitEntry) && c.spline > 0.0 {
                pit_entry = Some(c.spline);
            }
            r.last_location = c.location;
            // Not on the grid / at the start (all cars slow there): only once the car is under way.
            let under_way = c.progress.map_or(c.laps >= 1 || c.spline > 0.2, |p| p > 0.3);
            let stopped = c.location == CarLocation::Track && c.speed_kmh < self.stopped_kmh && running && under_way;
            if c.off_track || stopped {
                r.off_since.get_or_insert(t);
            } else {
                r.off_since = None;
            }
            if r.start_position.is_none() && running {
                // The grid order at the green light, in the same terms as the position shown.
                let rank = if multiclass { c.class_position } else { c.position };
                if rank > 0 {
                    r.start_position = Some(rank);
                }
            }
        }
        if pit_entry.is_some() {
            self.pit_entry_pct = pit_entry;
        }
        let mut cars = cars;
        self.apply_live_order(&mut cars);
        self.cars = cars;
    }

    /// The game updates the official position only at timing points, so an overtake would
    /// show up at the finish line. While the race runs the order comes from the distance
    /// driven instead. Not on the first lap (cars on the grid are still before the line,
    /// their spline looks like a nearly finished lap) and not after the flag (the official
    /// classification counts then).
    fn apply_live_order(&self, cars: &mut [CarState]) {
        // With an absolute distance from the game the order is right from the green light on;
        // otherwise only once the lap counters run.
        let absolute = cars.iter().all(|c| c.progress.is_some());
        if !self.race_running() || self.finished || !(absolute || cars.iter().any(|c| c.laps >= 1)) {
            return;
        }
        // All cars from the same sample (incl. the player): a mix of sources and rates would
        // put cars a lap apart for a moment at the line.
        let mut order: Vec<(usize, f64, i32)> = cars
            .iter()
            .enumerate()
            .map(|(i, c)| (i, c.progress.or_else(|| self.gaps.progress(c.id)).unwrap_or(f64::from(c.laps) + f64::from(c.spline)), c.position))
            .collect();
        order.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.2.cmp(&b.2)));
        let mut per_class: BTreeMap<i32, i32> = BTreeMap::new();
        for (rank, (i, _, _)) in order.into_iter().enumerate() {
            cars[i].position = rank as i32 + 1;
            // Class position from the same live order (the game's is official, at the line).
            let n = per_class.entry(cars[i].class_id).or_insert(0);
            *n += 1;
            cars[i].class_position = *n;
        }
    }

    /// More than one car class in the session: positions are meant within the class.
    pub fn multiclass(&self) -> bool {
        is_multiclass(&self.cars)
    }

    /// A car's position as the driver understands it: in its class in multi-class races.
    pub fn rank(&self, c: &CarState) -> i32 {
        if self.multiclass() { c.class_position } else { c.position }
    }

    /// Cars the player races against for position (own class in multi-class races).
    pub fn is_rival(&self, c: &CarState) -> bool {
        !c.is_player && (!self.multiclass() || self.player_car().is_none_or(|me| me.class_id == c.class_id))
    }

    /// The driver sits in the car and the game delivers real data. In menus the physics
    /// page is still empty (no fuel, track at 0 degrees).
    pub fn in_car(&self) -> bool {
        let f = &self.frame;
        f.status == SimStatus::Live && !f.setup_menu && (f.fuel_l > 0.0 || f.speed_kmh > 1.0 || f.completed_laps > 0)
    }

    /// The race has really started (green flag), not just loaded.
    pub fn race_running(&self) -> bool {
        use crate::storage::settings::SessionKind;
        let f = &self.frame;
        self.session.session_type.kind() == SessionKind::Race
            && f.status == SimStatus::Live
            && match self.session.phase {
                SessionPhase::Session => true,
                // Without a phase from the game only a driven lap proves it.
                SessionPhase::Unknown => f.completed_laps > 0,
                _ => false,
            }
    }

    pub fn player_car(&self) -> Option<&CarState> {
        self.cars.iter().find(|c| c.is_player).or_else(|| self.cars.iter().find(|c| c.id == self.frame.player_car_id))
    }

    pub fn car(&self, id: i32) -> Option<&CarState> {
        self.cars.iter().find(|c| c.id == id)
    }

    pub fn on_frame(&mut self, mut f: TelemetryFrame, settings: &Settings, spotter_out: &mut Vec<SpotterCall>, events: &mut Vec<RaceEvent>) {
        self.apply_pit_rules(&mut f, settings);
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
        // Start lights: an event right away (the 5 Hz tick would be too late for "go").
        if f.start_light != self.last_start_light {
            self.last_start_light = f.start_light;
            events.push(RaceEvent::StartLight(f.start_light));
        }

        if f.status != SimStatus::Live {
            self.frame = f;
            self.has_frame = true;
            return;
        }

        // Player timing (high rate).
        match f.progress {
            Some(p) => self.gaps.update_progress(f.player_car_id, p, t),
            None => self.gaps.update(f.player_car_id, f.completed_laps, f.spline, t),
        }
        self.fuel.on_frame(f.fuel_l);

        // Trace for lap comparison.
        if self.trace.bins.len() <= 1 {
            self.trace = Trace::new(self.track_m());
        }
        self.trace.record(f.spline, Sample { speed_kmh: f.speed_kmh, throttle: f.throttle, brake: f.brake, tc: f.tc_active, abs: f.abs_active });

        // Track outline from the first full lap.
        // Without broadcasting data the outline comes from the player's own first lap.
        if self.cars.is_empty() {
            self.record_outline(&f);
        }

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
            if std::mem::take(&mut self.finish_pending) && !self.finished {
                self.finished = true;
                events.push(RaceEvent::Finished);
            }
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
            if f.flag == Flag::Checkered && !self.finished {
                // At the line right now (ACC shows the flag there): finished. Mid-lap (iRacing
                // shows it when the leader finishes): at the player's next line crossing.
                if f.spline < FINISH_AT_LINE || f.spline > 1.0 - FINISH_AT_LINE {
                    self.finished = true;
                    events.push(RaceEvent::Finished);
                } else {
                    self.finish_pending = true;
                }
            }
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

        if self.race_start_position.is_none() && self.race_running() && self.player_position() > 0 {
            self.race_start_position = Some(self.player_position());
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

    /// Mandatory stops and pit window from the settings, for games that do not report them.
    fn apply_pit_rules(&mut self, f: &mut TelemetryFrame, settings: &Settings) {
        if self.has("hasPitWindow") {
            return;
        }
        let Some(g) = settings.games.get(&self.session.game_id) else { return };
        let race = self.session.session_type.kind() == crate::storage::settings::SessionKind::Race;
        if f.in_pit_box && !self.was_in_box && race && f.status == SimStatus::Live {
            self.own_stops += 1;
        }
        self.was_in_box = f.in_pit_box;
        if g.mandatory_stops > 0 && race {
            f.mandatory_pit_done = self.own_stops >= g.mandatory_stops;
            f.missing_mandatory_pits = g.mandatory_stops.saturating_sub(self.own_stops) as i32;
        }
        let (open, close) = (g.pit_window_open_min, g.pit_window_close_min);
        if race && close > open {
            self.session.pit_window_start_ms = (open * 60_000.0) as i32;
            self.session.pit_window_end_ms = (close * 60_000.0) as i32;
        }
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
        if std::mem::take(&mut self.finish_pending) && !self.finished {
            self.finished = true;
            events.push(RaceEvent::Finished);
        }

        self.sector_times = [None; 3];
        self.lap_out = false;
        self.lap_in = self.was_in_pit_lane;
        self.lap_dirty = false;
        self.lap_was_valid = true;
        self.pressure_acc = ([0.0; 4], [0.0; 4], 0);
    }

    /// Every car on track contributes points by spline position, so the map shows the
    /// track after a few seconds instead of after the player's first lap.
    fn outline_from_cars(&mut self, cars: &[CarState]) {
        let before = self.outline_filled;
        for c in cars {
            if c.location != CarLocation::Track || !(0.0..1.0).contains(&c.spline) || (c.x == 0.0 && c.z == 0.0) {
                continue;
            }
            let i = ((c.spline * OUTLINE_BINS as f32) as usize).min(OUTLINE_BINS - 1);
            if self.outline_bins[i].is_none() {
                self.outline_filled += 1;
            }
            self.outline_bins[i] = Some([c.x, c.z]);
        }
        // Publish when noticeably more of the lap is covered.
        if self.outline_filled >= OUTLINE_BINS / 10 && self.outline_filled >= before + OUTLINE_BINS / 40 || (self.outline_filled == OUTLINE_BINS && before < OUTLINE_BINS) {
            self.track_outline = self.outline_bins.iter().flatten().copied().collect();
            self.outline_version += 1;
        }
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
        // Races over a fixed number of laps report the laps left; timed races are estimated.
        let laps_to_go = self.frame.laps_left.unwrap_or_else(|| fuel::laps_to_go_timed(self.frame.session_time_left_ms, self.frame.spline, avg));
        fuel::strategy(self.frame.fuel_l, per_lap, clean, avg, laps_to_go, st.fuel_reserve_laps, st.fuel_reserve_liters, st.max_save_fraction)
    }

    /// The player's position as the driver understands it (in the class in multi-class races).
    pub fn player_position(&self) -> i32 {
        match self.player_car() {
            Some(c) if self.rank(c) > 0 => self.rank(c),
            // The game's own value is overall; in a multi-class race it would be misleading.
            _ if self.multiclass() => 0,
            _ => self.frame.position,
        }
    }

    /// Car directly ahead/behind in the race order.
    pub fn neighbour(&self, offset: i32) -> Option<&CarState> {
        let pos = self.player_position();
        self.cars.iter().find(|c| self.is_rival(c) && self.rank(c) == pos + offset)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn car(id: i32, position: i32, laps: i32, spline: f32, is_player: bool) -> CarState {
        CarState { id, position, laps, spline, is_player, location: CarLocation::Track, ..Default::default() }
    }

    fn running_race() -> RaceState {
        let mut s = RaceState::new();
        s.session.session_type = SessionType::Race;
        s.session.phase = SessionPhase::Session;
        s.frame.status = SimStatus::Live;
        s.frame.player_car_id = 1;
        s
    }

    #[test]
    fn overtake_counts_immediately_not_at_the_line() {
        let mut s = running_race();
        let mut ev = Vec::new();
        // Official order is stale: the player (id 1) is P3 but already passed the car officially P2.
        s.on_cars(vec![car(0, 1, 3, 0.70, false), car(2, 2, 3, 0.40, false), car(1, 3, 3, 0.42, true)], &mut ev);
        assert_eq!(s.player_position(), 2);
        assert_eq!(s.neighbour(-1).map(|c| c.id), Some(0));
        assert_eq!(s.neighbour(1).map(|c| c.id), Some(2));
    }

    #[test]
    fn official_order_on_first_lap_and_after_the_flag() {
        let mut ev = Vec::new();
        // Grid: the leader already crossed the line (spline small), the others are before it.
        let grid = || vec![car(0, 1, 0, 0.01, false), car(1, 2, 0, 0.99, true), car(2, 3, 0, 0.98, false)];
        let mut s = running_race();
        s.on_cars(grid(), &mut ev);
        assert_eq!(s.player_position(), 2);
        // After the flag cars stop at different distances, the classification decides.
        let mut s = running_race();
        // After the player's own finish (not merely the flag being out) the classification counts.
        s.finished = true;
        s.on_cars(vec![car(0, 1, 10, 0.05, false), car(1, 2, 10, 0.50, true)], &mut ev);
        assert_eq!(s.player_position(), 2);
    }

    #[test]
    fn absolute_progress_orders_live_from_the_green_light() {
        let mut ev = Vec::new();
        let at = |id: i32, pos: i32, progress: f64, me: bool| CarState { progress: Some(progress), ..car(id, pos, progress.floor().max(0.0) as i32, progress.rem_euclid(1.0) as f32, me) };
        let mut s = running_race();
        // First lap: official order stale (player officially P3), player already passed car 2.
        // Car 0 still behind the line (-1 + 0.99) must not count as a lap ahead.
        s.on_cars(vec![at(0, 1, 0.30, false), at(2, 2, 0.20, false), at(1, 3, 0.25, true), at(3, 4, -0.01, false)], &mut ev);
        assert_eq!(s.player_position(), 2);
        // At the line the player's car is a lap further at once, regardless of the lap time gate.
        s.on_cars(vec![at(0, 1, 1.02, false), at(1, 2, 1.01, true), at(2, 3, 0.99, false), at(3, 4, 0.5, false)], &mut ev);
        assert_eq!(s.player_position(), 2);
        s.on_cars(vec![at(0, 1, 1.02, false), at(1, 2, 1.03, true), at(2, 3, 0.99, false), at(3, 4, 0.5, false)], &mut ev);
        assert_eq!(s.player_position(), 1);
    }

    #[test]
    fn readiness_needs_car_and_green() {
        let mut s = running_race();
        assert!(!s.in_car(), "no fuel and standing still: still in the menu");
        s.frame.fuel_l = 40.0;
        assert!(s.in_car());
        s.frame.setup_menu = true;
        assert!(!s.in_car());
        assert!(s.race_running());
        s.session.phase = SessionPhase::PreSession;
        assert!(!s.race_running());
        // Empty tank in the race must not silence the crew.
        s.frame.setup_menu = false;
        s.frame.fuel_l = 0.0;
        s.frame.completed_laps = 5;
        assert!(s.in_car());
    }

    #[test]
    fn outline_from_other_cars() {
        let mut s = RaceState::new();
        let mut ev = Vec::new();
        // 30 cars spread around a circle → outline published without the player driving.
        let cars: Vec<CarState> = (0..30)
            .map(|i| {
                let sp = i as f32 / 30.0;
                let a = sp * std::f32::consts::TAU;
                CarState { id: i, spline: sp, x: 500.0 * a.cos(), z: 300.0 * a.sin(), location: CarLocation::Track, ..Default::default() }
            })
            .collect();
        for k in 0..20 {
            let shifted: Vec<CarState> = cars.iter().map(|c| {
                let sp = (c.spline + k as f32 * 0.0017).fract();
                let a = sp * std::f32::consts::TAU;
                CarState { spline: sp, x: 500.0 * a.cos(), z: 300.0 * a.sin(), ..c.clone() }
            }).collect();
            s.on_cars(shifted, &mut ev);
        }
        assert!(s.track_outline.len() > 100, "{}", s.track_outline.len());
        assert!(s.outline_version > 0);
    }

    #[test]
    fn relative_orders_by_track_gap_and_wraps_the_lap() {
        let mut s = RaceState::new();
        s.session.est_lap_s = Some(100.0);
        let car = |id: i32, laps: i32, spline: f32, est: f32, me: bool| CarState { id, laps, spline, est_time_s: Some(est), is_player: me, location: CarLocation::Track, ..Default::default() };
        s.cars = vec![
            car(0, 5, 0.50, 50.0, true),
            car(1, 5, 0.53, 53.0, false),       // 3 s ahead
            car(2, 4, 0.45, 45.0, false),       // 5 s behind, a lap down
            car(3, 6, 0.02, 2.0, false),        // across the line: 48 s behind on track
        ];
        let r = s.relative(5);
        let ids: Vec<i32> = r.iter().map(|x| x.id).collect();
        assert_eq!(ids, vec![1, 0, 2, 3]);
        assert!((r[0].gap_s - 3.0).abs() < 1e-4 && (r[3].gap_s + 48.0).abs() < 1e-4);
        // Car 2 is a lap down; car 3 is behind on track but a lap up (about to lap the player).
        assert_eq!((r[2].laps_ahead, r[3].laps_ahead), (-1, 1));
        // Only one car each side.
        assert_eq!(s.relative(1).len(), 3);
    }

    #[test]
    fn flag_mid_lap_finishes_at_the_next_line() {
        let mut s = running_race();
        let settings = Settings::default();
        let (mut sp, mut ev) = (Vec::new(), Vec::new());
        let mut f = s.frame.clone();
        f.status = SimStatus::Live;
        f.completed_laps = 9;
        f.spline = 0.5;
        f.flag = Flag::Checkered;
        s.on_frame(f.clone(), &settings, &mut sp, &mut ev);
        assert!(!ev.iter().any(|e| matches!(e, RaceEvent::Finished)), "leader finished, the player still drives");
        f.completed_laps = 10;
        f.spline = 0.01;
        f.last_lap_ms = 100_000;
        ev.clear();
        s.on_frame(f, &settings, &mut sp, &mut ev);
        assert!(ev.iter().any(|e| matches!(e, RaceEvent::Finished)));
    }

    #[test]
    fn multiclass_positions_are_class_positions() {
        let mut s = running_race();
        let mut ev = Vec::new();
        let mk = |id: i32, pos: i32, cpos: i32, class_id: i32, me: bool| CarState { id, position: pos, class_position: cpos, class_id, is_player: me, location: CarLocation::Track, ..Default::default() };
        // Not running yet (grid): the game's positions; the player is P5 overall, P2 in class.
        s.frame.status = SimStatus::Off;
        s.on_cars(vec![mk(0, 1, 1, 1, false), mk(1, 5, 2, 2, true), mk(2, 4, 1, 2, false), mk(3, 6, 3, 2, false)], &mut ev);
        assert_eq!(s.player_position(), 2);
        assert_eq!(s.neighbour(-1).map(|c| c.id), Some(2));
        assert_eq!(s.neighbour(1).map(|c| c.id), Some(3));
    }
}
