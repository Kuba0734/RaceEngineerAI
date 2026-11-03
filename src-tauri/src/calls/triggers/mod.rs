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
    pub engine_warned: [bool; 4],
    /// Last seen (mandatory, optional) repair time.
    pub last_repair: (f32, f32),
    /// Real repair time from the game, replaces the estimate in the damage decision.
    pub repair_estimate_s: Option<f32>,
    pub tyre_check_said: Option<i32>,
    pub hot_pressure_said: Option<i32>,
    // race control
    pub last_incidents: Option<i32>,
    pub last_pacing: crate::games::provider::Pacing,
    pub wet_declared: Option<bool>,
    /// Weather samples for the rain trend: (time s, precipitation, wetness level).
    pub weather_samples: std::collections::VecDeque<(f64, f32, i32)>,
    // weather
    pub last_rain: Option<(RainIntensity, RainIntensity, RainIntensity)>,
    pub last_grip: Option<TrackGrip>,
    // performance
    pub handling: performance::Handling,
    // quali
    pub quali_time_said: bool,
    pub push_said_lap: Option<i32>,
    pub traffic_said_lap: Option<i32>,
    // race start silence
    pub start_hush: crate::calls::start_hush::StartHush,
}

impl Memory {
    pub fn new() -> Self {
        Self { cold_check_pending: true, ..Default::default() }
    }
}

pub fn on_event(ctx: &mut Ctx, mem: &mut Memory, ev: &RaceEvent) {
    // Nothing happens to the driver while the game still shows a menu.
    if !ctx.state.in_car() && !matches!(ev, RaceEvent::NewSession | RaceEvent::SessionPhaseChanged(_)) {
        return;
    }
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
    // Largest value seen: before the green flag the game may count down something shorter.
    let left = ctx.state.frame.session_time_left_ms;
    if left > mem.session_duration_ms.unwrap_or(0.0) {
        mem.session_duration_ms = Some(left);
    }
    if !ctx.state.in_car() {
        return;
    }
    session::tick(ctx, mem);
    flags::tick(ctx, mem);
    race::tick(ctx, mem);
    race::traffic(ctx);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::provider::{CarState, SessionPhase, SessionType, SimStatus};

    fn run(state: &mut RaceState, settings: &Settings, engine: &CallEngine, mem: &mut Memory, from_s: u64, to_s: u64) -> Vec<String> {
        let mut ids = Vec::new();
        for t in from_s..to_s {
            state.frame.t_ms = t * 1000;
            let mut ctx = Ctx::new(state, settings, engine);
            tick(&mut ctx, mem);
            ids.extend(ctx.out.into_iter().map(|r| r.call_id.to_string()));
        }
        ids
    }

    /// A 10 minute race, loaded but not started: the driver is still in the menu.
    fn loaded_race() -> RaceState {
        let mut s = RaceState::new();
        s.has_frame = true;
        s.session.session_type = SessionType::Race;
        s.session.phase = SessionPhase::PreSession;
        s.frame.status = SimStatus::Live;
        s.frame.session_time_left_ms = 600_000.0;
        s.frame.game_fuel_per_lap_l = 3.0;
        s
    }

    #[test]
    fn silent_until_the_driver_is_in_the_car() {
        let settings = Settings::default();
        let engine = CallEngine::new(&settings);
        let mut mem = Memory::new();
        let mut s = loaded_race();
        assert_eq!(run(&mut s, &settings, &engine, &mut mem, 0, 60), Vec::<String>::new());
    }

    #[test]
    fn radio_check_comes_first_and_no_race_calls_before_green() {
        let settings = Settings::default();
        let engine = CallEngine::new(&settings);
        let mut mem = Memory::new();
        let mut s = loaded_race();
        s.frame.fuel_l = 40.0;
        s.frame.weather.road_temp_c = 25.0;
        let ids = run(&mut s, &settings, &engine, &mut mem, 0, 60);
        let at = |id: &str| ids.iter().position(|x| x == id);
        assert_eq!(at("session.radio_check"), Some(0), "{ids:?}");
        assert!(at("prerace.briefing").is_some(), "{ids:?}");
        assert!(at("race.time_left").is_none() && at("fuel.critical").is_none(), "{ids:?}");
    }

    #[test]
    fn full_race_length_is_no_time_mark() {
        let settings = Settings::default();
        let engine = CallEngine::new(&settings);
        let mut mem = Memory::new();
        let mut s = loaded_race();
        s.session.phase = SessionPhase::Session;
        s.frame.fuel_l = 40.0;
        s.frame.best_lap_ms = 100_000;
        let ids = run(&mut s, &settings, &engine, &mut mem, 0, 30);
        assert!(!ids.iter().any(|x| x == "race.time_left"), "{ids:?}");
    }

    /// Calls of one detector as "id" or "id:sub".
    fn calls(state: &RaceState, mem: &mut Memory, f: impl Fn(&mut Ctx, &mut Memory)) -> Vec<String> {
        let settings = Settings::default();
        let engine = CallEngine::new(&settings);
        let mut ctx = Ctx::new(state, &settings, &engine);
        f(&mut ctx, mem);
        ctx.out.iter().map(|r| match r.sub { Some(sub) => format!("{}:{sub}", r.call_id), None => r.call_id.to_string() }).collect()
    }

    #[test]
    fn incidents_near_the_limit() {
        let mut s = loaded_race();
        s.session.incident_limit = Some(17);
        let mut mem = Memory::new();
        s.frame.incidents = Some(1);
        assert!(calls(&s, &mut mem, flags::tick).is_empty(), "first value only sets the baseline");
        s.frame.incidents = Some(3);
        assert!(calls(&s, &mut mem, flags::tick).is_empty(), "below report_from (4)");
        s.frame.incidents = Some(14);
        assert_eq!(calls(&s, &mut mem, flags::tick), vec!["penalty.incidents:limit"]);
        assert!(calls(&s, &mut mem, flags::tick).is_empty(), "no repeat without a new incident");
    }

    #[test]
    fn safety_car_restart_and_wet_declared() {
        let mut s = loaded_race();
        let mut mem = Memory::new();
        s.frame.pacing.green_next_lap = true;
        assert_eq!(calls(&s, &mut mem, flags::tick), vec!["flag.pace:green_next_lap"]);
        assert!(calls(&s, &mut mem, flags::tick).is_empty());
        s.frame.wet_tyres_allowed = Some(false);
        assert!(calls(&s, &mut mem, weather::tick).iter().all(|c| c != "weather.wet_declared"));
        s.frame.wet_tyres_allowed = Some(true);
        assert!(calls(&s, &mut mem, weather::tick).iter().any(|c| c == "weather.wet_declared"));
    }

    #[test]
    fn engine_warning_and_repair_time_once() {
        let mut s = loaded_race();
        s.frame.speed_kmh = 200.0;
        s.frame.fuel_l = 40.0;
        let mut mem = Memory::new();
        s.frame.engine.water_hot = true;
        s.frame.engine.water_temp_c = 118.0;
        s.frame.repair = Some(crate::games::provider::Repair { mandatory_s: 0.0, optional_s: 0.0, fast_repairs_left: Some(1) });
        let first = calls(&s, &mut mem, car::tick);
        assert!(first.contains(&"car.engine:water_temp".to_string()), "{first:?}");
        assert!(!calls(&s, &mut mem, car::tick).contains(&"car.engine:water_temp".to_string()));
        s.frame.repair = Some(crate::games::provider::Repair { mandatory_s: 21.0, optional_s: 35.0, fast_repairs_left: Some(1) });
        assert!(calls(&s, &mut mem, car::tick).contains(&"damage.repair:mandatory".to_string()));
        assert_eq!(mem.repair_estimate_s, Some(35.0));
        // Counting down in the pits is not new damage.
        s.frame.repair = Some(crate::games::provider::Repair { mandatory_s: 5.0, optional_s: 35.0, fast_repairs_left: Some(1) });
        assert!(!calls(&s, &mut mem, car::tick).iter().any(|c| c.starts_with("damage.repair")));
    }

    #[test]
    fn pressure_advice_from_measured_hot_pressures() {
        let mut s = loaded_race();
        s.session.car_class = "GT3".into();
        let mut mem = Memory::new();
        let mut check = crate::games::provider::TyreCheck { lap: 8, ..Default::default() };
        s.frame.tyre_check = Some(check);
        assert!(!calls(&s, &mut mem, car::tick).iter().any(|c| c == "tyre.pressure_next_stint"), "no hot pressures yet");
        // GT3 window 26–27 psi: front left far too high, the rest fine.
        check.hot_pressure_psi = Some([28.4, 26.5, 26.4, 26.6]);
        s.frame.tyre_check = Some(check);
        assert!(calls(&s, &mut mem, car::tick).iter().any(|c| c == "tyre.pressure_next_stint"));
        assert!(!calls(&s, &mut mem, car::tick).iter().any(|c| c == "tyre.pressure_next_stint"), "once per stop");
    }

    #[test]
    fn start_lights_and_rain_trend() {
        let s = loaded_race();
        let mut mem = Memory::new();
        let ready = RaceEvent::StartLight(crate::games::provider::StartLight::Ready);
        let set = RaceEvent::StartLight(crate::games::provider::StartLight::Set);
        let go = RaceEvent::StartLight(crate::games::provider::StartLight::Go);
        assert_eq!(calls(&s, &mut mem, |c, m| race::on_event(c, m, &ready)), vec!["race.start_lights:ready"]);
        assert!(calls(&s, &mut mem, |c, m| race::on_event(c, m, &set)).is_empty());
        assert_eq!(calls(&s, &mut mem, |c, m| race::on_event(c, m, &go)), vec!["race.start_lights:go"]);

        // Rain from 0.1 to 0.4 over six minutes: "heavier", once.
        let mut s = loaded_race();
        s.frame.weather.wetness_level = Some(2);
        let mut said = Vec::new();
        for k in 0..20u64 {
            s.frame.t_ms = k * 20_000;
            s.frame.weather.rain_amount = Some(0.1 + 0.3 * (k as f32 / 19.0));
            said.extend(calls(&s, &mut mem, weather::tick).into_iter().filter(|c| c.starts_with("weather.trend")));
        }
        assert_eq!(said, vec!["weather.trend:rain_heavier"]);
    }

    #[test]
    fn faster_class_from_behind() {
        let mut s = loaded_race();
        s.session.est_lap_s = Some(100.0);
        let car = |id: i32, spline: f32, est: f32, class_id: i32, lap: f32, me: bool| CarState {
            id, laps: 3, spline, est_time_s: Some(est), is_player: me, location: crate::games::provider::CarLocation::Track,
            class_id, class_name: if class_id == 1 { "GT3".into() } else { "LMP2".into() }, class_est_lap_s: Some(lap), ..Default::default()
        };
        s.cars = vec![car(0, 0.5, 50.0, 1, 100.0, true), car(1, 0.49, 48.5, 2, 92.0, false), car(2, 0.53, 53.0, 1, 100.5, false)];
        let mut mem = Memory::new();
        let out = calls(&s, &mut mem, |c, _| race::traffic(c));
        assert_eq!(out, vec!["traffic.faster_class"]);
    }

    #[test]
    fn pit_menu_and_tyre_check() {
        let mut s = loaded_race();
        s.frame.fuel_l = 5.0;
        s.frame.best_lap_ms = 100_000;
        s.frame.pit_menu = Some(crate::games::provider::PitMenu { fuel_l: 10.0, tyres: [true; 4], ..Default::default() });
        let mut mem = Memory::new();
        let ev = RaceEvent::PitEntry;
        let menu = |s: &RaceState, mem: &mut Memory| calls(s, mem, |c, m| fuel::on_event(c, m, &ev)).into_iter().filter(|c| c.starts_with("pit.menu_check")).collect::<Vec<_>>();
        assert_eq!(menu(&s, &mut mem), vec!["pit.menu_check:fuel_low"]);
        s.frame.pit_menu = Some(crate::games::provider::PitMenu { fuel_l: 60.0, tyres: [true; 4], ..Default::default() });
        assert_eq!(menu(&s, &mut mem), vec!["pit.menu_check:ok"]);

        let mut check = crate::games::provider::TyreCheck { lap: 12, ..Default::default() };
        check.wear = [[0.9; 3], [0.7, 0.75, 0.8], [0.95; 3], [0.95; 3]];
        check.temp_c = [[80.0, 75.0, 70.0], [95.0, 85.0, 80.0], [70.0; 3], [70.0; 3]];
        s.frame.tyre_check = Some(check);
        let exit = RaceEvent::PitExit { loss_s: None };
        assert_eq!(calls(&s, &mut mem, |c, m| car::on_event(c, m, &exit)), vec!["tyre.after_stop"]);
        assert!(calls(&s, &mut mem, |c, m| car::on_event(c, m, &exit)).is_empty(), "same stop only once");
    }
}
