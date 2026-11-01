//! Rule-based triggers. Each module turns RaceState + events into CallRequests.
//! `tick` runs at ~5 Hz, `on_event` for every RaceEvent.

mod car;
mod flags;
mod fuel;
mod performance;
mod quali;
mod race;
mod session;
mod weather;

pub use car::next_stint_deltas;
pub use session::{briefing, race_summary};

use super::engine::{CallEngine, CallRequest};
use super::format::Fmt;
use crate::games::provider::{RainIntensity, TrackGrip};
use crate::race::state::{RaceEvent, RaceState};
use crate::storage::settings::{SessionKind, Settings};

pub struct Ctx<'a> {
    pub state: &'a RaceState,
    pub settings: &'a Settings,
    pub engine: &'a CallEngine,
    pub fmt: Fmt,
    pub kind: SessionKind,
    pub t: f64,
    pub out: Vec<CallRequest>,
}

impl<'a> Ctx<'a> {
    pub fn new(state: &'a RaceState, settings: &'a Settings, engine: &'a CallEngine) -> Self {
        Self { state, settings, engine, fmt: Fmt::new(settings), kind: state.session.session_type.kind(), t: state.t_s(), out: Vec::new() }
    }
    pub fn th(&self, id: &str, name: &str) -> f64 {
        self.engine.threshold(id, name)
    }
    pub fn on(&self, id: &str) -> bool {
        self.engine.enabled(id, self.kind)
    }
    pub fn push(&mut self, r: CallRequest) {
        self.out.push(r);
    }
    pub fn lap(&self) -> i32 {
        self.state.frame.completed_laps
    }
    pub fn frag(&self, key: &str, params: &[(&'static str, String)]) -> String {
        self.engine.fragment(key, params)
    }
}

/// Internal state of all triggers for the current session.
#[derive(Default)]
pub struct Memory {
    // session
    pub live_since: Option<f64>,
    pub radio_check_done: bool,
    pub greeting_at: Option<f64>,
    pub spotter_intro_at: Option<f64>,
    pub first_session_of_day_done: bool,
    pub briefing_done: bool,
    pub formation_done: bool,
    pub summary_done: bool,
    pub session_duration_ms: Option<f64>,
    // race
    pub time_marks: [bool; 3],
    pub last_gap_ahead: Option<f32>,
    pub last_gap_behind: Option<f32>,
    pub last_ahead_id: Option<i32>,
    // flags
    pub penalty_lap: Option<i32>,
    pub fcy_active: bool,
    // fuel & pit
    pub save_said: bool,
    pub start_fuel_said: bool,
    pub pit_window_open_said: bool,
    pub mandatory_warned: bool,
    pub plan_reminded_for: Option<i32>,
    pub plan_now_for: Option<i32>,
    pub stint_laps_start: usize,
    // car
    pub cold_check_pending: bool,
    pub wet_state_said: Option<&'static str>,
    pub lights_said: bool,
    pub wipers_said: bool,
    pub stint_marks: [bool; 2],
    pub damage_sum: f32,
    pub damage_lap: Option<i32>,
    pub pace_before_damage_ms: Option<i32>,
    pub lockups_front: u32,
    pub lockups_rear: u32,
    pub locking: bool,
    pub pad_life_lap_start: Option<f32>,
    pub pad_wear_per_lap: Option<f32>,
    // weather
    pub last_rain: Option<(RainIntensity, RainIntensity, RainIntensity)>,
    pub last_grip: Option<TrackGrip>,
    // performance
    pub handling: performance::Handling,
    // quali
    pub quali_time_said: bool,
    pub push_said_lap: Option<i32>,
    pub traffic_said_lap: Option<i32>,
}

impl Memory {
    pub fn new() -> Self {
        Self { cold_check_pending: true, ..Default::default() }
    }
}

pub fn on_event(ctx: &mut Ctx, mem: &mut Memory, ev: &RaceEvent) {
    session::on_event(ctx, mem, ev);
    flags::on_event(ctx, mem, ev);
    race::on_event(ctx, mem, ev);
    fuel::on_event(ctx, mem, ev);
    car::on_event(ctx, mem, ev);
    performance::on_event(ctx, mem, ev);
    quali::on_event(ctx, mem, ev);
}

pub fn tick(ctx: &mut Ctx, mem: &mut Memory) {
    if !ctx.state.has_frame {
        return;
    }
    if mem.session_duration_ms.is_none() && ctx.state.frame.session_time_left_ms > 0.0 {
        mem.session_duration_ms = Some(ctx.state.frame.session_time_left_ms);
    }
    session::tick(ctx, mem);
    flags::tick(ctx, mem);
    race::tick(ctx, mem);
    fuel::tick(ctx, mem);
    car::tick(ctx, mem);
    weather::tick(ctx, mem);
    performance::tick(ctx, mem);
    quali::tick(ctx, mem);
}

/// Called on every frame for cheap continuous detectors (lock-ups, handling).
pub fn on_frame(state: &RaceState, mem: &mut Memory) {
    car::on_frame(state, mem);
    performance::on_frame(state, mem);
}
