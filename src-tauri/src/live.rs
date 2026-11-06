//! Throttled live data for the windows (main window, overlay, LAN dashboard): one compact
//! frame built from the race state, heavy parts (standings, fuel, …) only every n-th time.

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use ts_rs::TS;

use crate::core::Core;
use crate::games::provider::{Flag, SimStatus};
use crate::race::fuel::FuelStrategy;
use crate::race::spotter::{RadarCar, Side};
use crate::race::state::{ClassSummary, RelativeRow, StandingRow};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MapCar {
    pub id: i32,
    pub number: String,
    #[ts(optional)]
    pub color: Option<String>,
    pub x: f32,
    pub z: f32,
    pub position: i32,
    pub me: bool,
    pub pit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RecentLap {
    pub lap: i32,
    pub time_ms: i32,
    pub valid: bool,
    pub in_lap: bool,
    pub out_lap: bool,
}

/// Tyre and brake values with the target windows of the car class.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TyreView {
    pub core_temp_c: [f32; 4],
    pub pressure_psi: [f32; 4],
    pub brake_temp_c: [f32; 4],
    pub temp_cold: f32,
    pub temp_hot: f32,
    pub pressure_min: f32,
    pub pressure_max: f32,
    pub brake_hot: f32,
    /// Values measured at the last pit stop (game has no live tyre data).
    pub from_pit: bool,
    /// Tread left of the most worn zone per wheel, % (0 = unknown).
    pub wear_pct: [f32; 4],
    /// The game reports live tyre values / brake temperatures at all.
    pub live: bool,
    pub has_brakes: bool,
    /// Temperatures across the tread (outer, middle, inner) per wheel; middle only when the
    /// game reports one value.
    pub zones_c: [[f32; 3]; 4],
}

/// Everything the pit helper widget shows.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PitView {
    #[ts(optional)]
    pub speed_limit_kmh: Option<f32>,
    pub limiter_on: bool,
    pub in_pit_lane: bool,
    /// Distance along the track to the own pit box (m), when known and close.
    #[ts(optional)]
    pub box_in_m: Option<f32>,
    pub service_active: bool,
    /// Pit menu of the game (None = game has none).
    #[ts(optional)]
    pub menu: Option<crate::games::provider::PitMenu>,
    pub refuel_needed_l: f32,
    pub repair_mandatory_s: f32,
    pub repair_optional_s: f32,
    pub pit_loss_s: f32,
    pub exit_position: i32,
    pub mandatory_stops_left: i32,
    #[ts(optional)]
    pub window_open: Option<bool>,
}

/// A car the driver should know about (off track ahead, faster class behind).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CarAlert {
    pub id: i32,
    pub position: i32,
    pub number: String,
    pub driver: String,
    pub class_name: String,
    #[ts(optional)]
    pub class_color: Option<String>,
    /// Along the track (m).
    pub distance_m: f32,
    /// Time gap (s), when it can be told.
    #[ts(optional)]
    pub gap_s: Option<f32>,
    /// Off track (else stopped / very slow on track).
    pub off_track: bool,
    /// For how long (s).
    pub seconds: f32,
}

/// Approaching the pit lane: distance to the speed limit line.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PitEntryView {
    pub distance_m: f32,
    pub speed_kmh: f32,
    #[ts(optional)]
    pub limit_kmh: Option<f32>,
    pub limiter_on: bool,
}

/// Weather as far as the game reports it.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WeatherView {
    pub air_temp_c: f32,
    pub track_temp_c: f32,
    #[ts(optional)]
    pub humidity_pct: Option<f32>,
    #[ts(optional)]
    pub wind_dir_deg: Option<f32>,
    pub wind_kmh: f32,
    pub grip: crate::games::provider::TrackGrip,
    pub rain_now: crate::games::provider::RainIntensity,
    pub rain_10min: crate::games::provider::RainIntensity,
    pub rain_30min: crate::games::provider::RainIntensity,
    /// The game gives a forecast (else 10/30 min are just "as now").
    pub has_forecast: bool,
}

/// Each side of the player in the relative list (the widget shows fewer if set).
const RELATIVE_EACH_SIDE: usize = 6;
/// The pit helper shows the box distance from this far out.
const BOX_DISTANCE_SHOWN_M: f32 = 800.0;

/// How many laps the live frame carries for the lap time widget.
const RECENT_LAPS: usize = 20;

/// Throttled live data for the windows. Heavy parts are optional and sent at a lower rate.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LiveFrame {
    pub live: bool,
    pub speed: f32,
    pub gear: i32,
    pub rpm: i32,
    pub throttle: f32,
    pub brake: f32,
    pub steer: f32,
    pub max_rpm: i32,
    pub tc_level: i32,
    pub abs_level: i32,
    pub tc_active: bool,
    pub abs_active: bool,
    pub fuel_l: f32,
    pub lap: i32,
    pub position: i32,
    pub cars_total: i32,
    pub spline: f32,
    pub current_lap_ms: i32,
    pub last_lap_ms: i32,
    pub best_lap_ms: i32,
    pub delta_ms: i32,
    pub time_left_ms: f64,
    pub flag: Flag,
    pub yellow_sectors: [bool; 3],
    pub penalty: Option<crate::games::provider::Penalty>,
    pub in_pits: bool,
    pub spotter: Side,
    pub radar: Vec<RadarCar>,
    pub map: Vec<MapCar>,
    pub outline_version: u32,
    #[ts(optional)]
    pub standings: Option<Vec<StandingRow>>,
    #[ts(optional)]
    pub fuel: Option<FuelStrategy>,
    #[ts(optional)]
    pub recent_laps: Option<Vec<RecentLap>>,
    #[ts(optional)]
    pub tyres: Option<TyreView>,
    #[ts(optional)]
    pub relative: Option<Vec<RelativeRow>>,
    #[ts(optional)]
    pub classes: Option<Vec<ClassSummary>>,
    #[ts(optional)]
    pub pit: Option<PitView>,
    pub player_class_id: i32,
    #[ts(optional)]
    pub incidents: Option<i32>,
    #[ts(optional)]
    pub incident_limit: Option<i32>,
    /// Time of day in the sim (s since midnight; 0 = unknown).
    pub clock_s: f32,
    /// Laps of the whole session incl. the ones driven (estimate for timed races).
    #[ts(optional)]
    pub laps_total_est: Option<f32>,
    pub session_total_ms: f64,
    pub track_temp_c: f32,
    pub clutch: f32,
    pub start_light: crate::games::provider::StartLight,
    pub weather: WeatherView,
    #[ts(optional)]
    pub fuel_last_lap_l: Option<f32>,
    pub fuel_max_l: f32,
    #[ts(optional)]
    pub offtrack: Option<CarAlert>,
    #[ts(optional)]
    pub faster_class: Option<CarAlert>,
    #[ts(optional)]
    pub pit_entry: Option<PitEntryView>,
    #[ts(optional)]
    pub fuel_plan: Option<crate::race::fuel_plan::FuelPlanView>,
    /// A game is detected (the fuel planner also shows outside the car).
    pub game_running: bool,
    /// The game decides the spotter sides itself (radar shows sides, not exact positions).
    pub native_spotter: bool,
    pub speaking: bool,
    pub listening: crate::voice::ListenState,
    pub mic_level: f32,
    pub out_level: f32,
    pub track: String,
    pub session: crate::games::provider::SessionType,
    pub quiet: bool,
    pub preset: String,
}

pub struct LiveSub {
    pub id: u32,
    pub channel: Channel<LiveFrame>,
    pub hz: u32,
    pub last: std::time::Instant,
    pub heavy_every: u32,
    pub count: u32,
}

pub fn build_live(core: &Core, heavy: bool) -> LiveFrame {
    let st = core.state.lock();
    let settings = core.settings.read();
    let f = &st.frame;
    let me = f.player_car_id;
    let mut map: Vec<MapCar> = st
        .cars
        .iter()
        // Cars without a position yet (no map built) are left out instead of piling up at 0/0.
        .filter(|c| c.x != 0.0 || c.z != 0.0)
        .map(|c| MapCar { id: c.id, number: if c.number_text.is_empty() { c.race_number.to_string() } else { c.number_text.clone() }, color: c.class_color.clone(), x: c.x, z: c.z, position: c.position, me: c.is_player || c.id == me, pit: c.location.in_pits() })
        .collect();
    if map.is_empty() {
        // Without broadcasting: positions from shared memory.
        map = f.cars.iter().map(|c| MapCar { id: c.id, number: String::new(), color: None, x: c.x, z: c.z, position: 0, me: c.id == me, pit: false }).collect();
    }
    let (quiet, preset) = {
        let e = core.engine.lock();
        (e.crew.quiet.is_some(), e.crew.preset_override.clone().unwrap_or_else(|| settings.calls.preset.clone()))
    };
    LiveFrame {
        live: st.has_frame && f.status == SimStatus::Live,
        speed: f.speed_kmh,
        gear: f.gear,
        rpm: f.rpm,
        throttle: f.throttle,
        brake: f.brake,
        steer: f.steer,
        max_rpm: st.session.max_rpm,
        tc_level: f.tc_level,
        abs_level: f.abs_level,
        tc_active: f.tc_active,
        abs_active: f.abs_active,
        fuel_l: f.fuel_l,
        lap: f.completed_laps + 1,
        position: st.player_position(),
        cars_total: st.cars.len().max(f.cars.len()) as i32,
        spline: f.spline,
        current_lap_ms: f.current_lap_ms,
        last_lap_ms: f.last_lap_ms,
        best_lap_ms: st.best_valid_lap().map(|l| l.time_ms).unwrap_or(f.best_lap_ms),
        delta_ms: f.delta_ms,
        time_left_ms: f.session_time_left_ms,
        flag: f.flag,
        yellow_sectors: f.yellow_sectors,
        penalty: f.penalty,
        in_pits: f.in_pit_lane,
        spotter: st.spotter.state(),
        radar: st.spotter.radar.clone(),
        map,
        outline_version: st.outline_version,
        standings: heavy.then(|| st.standings(5)),
        fuel: heavy.then(|| st.fuel_strategy(&settings)),
        tyres: heavy.then(|| {
            let c = crate::games::cars::class_def(&st.session.car_class);
            let window = if f.rain_tyres { c.wet } else { c.dry };
            // Without live tyre data: the values measured at the last stop (middle of the tread).
            let pit = f.tyre_check.filter(|_| !st.has("hasTyreCoreTemps"));
            TyreView {
                core_temp_c: match pit {
                    Some(c) => c.temp_c.map(|z| z[1]),
                    None => f.wheels.map(|w| w.core_temp_c),
                },
                // Hot pressures measured after the stint, if the game reported them for this stop.
                pressure_psi: match pit {
                    Some(c) => c.hot_pressure_psi.unwrap_or([0.0; 4]),
                    None => f.wheels.map(|w| w.pressure_psi),
                },
                zones_c: match pit {
                    Some(c) => c.temp_c,
                    None => f.wheels.map(|w| [w.core_temp_c; 3]),
                },
                from_pit: pit.is_some(),
                live: st.has("hasTyreCoreTemps"),
                has_brakes: st.has("hasBrakeWear"),
                wear_pct: pit.map(|c| c.wear.map(|z| z.iter().copied().fold(1.0f32, f32::min) * 100.0)).unwrap_or([0.0; 4]),
                brake_temp_c: f.wheels.map(|w| w.brake_temp_c),
                temp_cold: c.core_temp_cold,
                temp_hot: c.core_temp_hot,
                pressure_min: window[0],
                pressure_max: window[1],
                brake_hot: c.brake_temp_hot,
            }
        }),
        relative: heavy.then(|| st.relative(RELATIVE_EACH_SIDE)),
        classes: heavy.then(|| st.class_summaries()),
        pit: heavy.then(|| pit_view(&st, &settings)),
        player_class_id: st.player_car().map(|c| c.class_id).unwrap_or(0),
        incidents: f.incidents,
        incident_limit: st.session.incident_limit,
        clock_s: f.clock_s,
        laps_total_est: heavy.then(|| f.completed_laps as f32 + st.fuel_strategy(&settings).laps_to_go).filter(|l| *l > 0.0),
        session_total_ms: st.session.session_end_ms,
        track_temp_c: f.weather.road_temp_c,
        clutch: f.clutch,
        start_light: f.start_light,
        weather: WeatherView {
            air_temp_c: f.weather.air_temp_c,
            track_temp_c: f.weather.road_temp_c,
            humidity_pct: f.weather.humidity_pct,
            wind_dir_deg: f.weather.wind_dir_deg,
            wind_kmh: f.weather.wind_speed_ms * 3.6,
            grip: f.weather.grip,
            rain_now: f.weather.rain_now,
            rain_10min: f.weather.rain_10min,
            rain_30min: f.weather.rain_30min,
            has_forecast: st.has("hasWeatherForecast"),
        },
        fuel_last_lap_l: st.laps.last().and_then(|l| l.fuel_used_l).filter(|v| *v > 0.0),
        fuel_max_l: st.session.max_fuel_l,
        offtrack: offtrack_ahead(&st, &settings.overlay),
        faster_class: faster_behind(&st, settings.overlay.alert_faster_m),
        pit_entry: pit_entry(&st, settings.overlay.pit_entry_show_m),
        fuel_plan: heavy.then(|| fuel_plan(&st, &settings)),
        game_running: core.status.read().active_game.is_some(),
        native_spotter: f.native_side.is_some(),
        recent_laps: heavy.then(|| st.laps.iter().rev().take(RECENT_LAPS).map(|l| RecentLap { lap: l.lap, time_ms: l.time_ms, valid: l.valid, in_lap: l.in_lap, out_lap: l.out_lap }).collect()),
        speaking: core.mixer_status.any_speaking(),
        listening: core.voice.state(),
        mic_level: core.voice.level(),
        out_level: core.mixer_status.level(),
        track: st.session.track.clone(),
        session: st.session.session_type,
        quiet,
        preset,
    }
}

fn fuel_plan(st: &crate::race::state::RaceState, s: &crate::storage::settings::Settings) -> crate::race::fuel_plan::FuelPlanView {
    let st_s = &s.strategy;
    crate::race::fuel_plan::plan(&crate::race::fuel_plan::PlanInput {
        manual_per_lap_l: s.overlay.fuel_calc_per_lap_l,
        // At least 3 clean laps of this session beat older data.
        session: st.fuel.average(s.strategy.clean_laps_window as usize).filter(|(_, _, n)| *n >= 3).map(|(l, ms, _)| (l, ms)),
        history: st.fuel_history,
        game_per_lap_l: st.frame.game_fuel_per_lap_l,
        est_lap_s: st.session.est_lap_s,
        manual_laps: s.overlay.fuel_calc_race_laps,
        session_race_laps: st.session.race_laps,
        session_race_time_s: st.session.race_time_s,
        settings_minutes: st_s.next_race_minutes,
        formation_lap: st_s.formation_lap,
        reserve_laps: st_s.fuel_reserve_laps,
        reserve_liters: st_s.fuel_reserve_liters,
        tank_l: st.session.max_fuel_l,
        fuel_now_l: st.frame.fuel_l,
    })
}

fn alert(st: &crate::race::state::RaceState, c: &crate::games::provider::CarState, distance_m: f32) -> CarAlert {
    let t = st.t_s();
    let seconds = st.rivals.get(&c.id).and_then(|r| r.off_since).map_or(0.0, |s| (t - s).max(0.0) as f32);
    // Time to cover the distance at the car's speed (or own speed when it stands).
    let v = c.speed_kmh.max(st.frame.speed_kmh) / 3.6;
    CarAlert {
        id: c.id,
        position: st.rank(c),
        number: if c.number_text.is_empty() { c.race_number.to_string() } else { c.number_text.clone() },
        driver: c.driver.clone(),
        class_name: c.class_name.clone(),
        class_color: c.class_color.clone(),
        distance_m,
        gap_s: (v > 5.0).then_some(distance_m / v),
        off_track: c.off_track,
        seconds,
    }
}

/// Distance along the lap from `from` forward to `to` (m).
fn ahead_m(st: &crate::race::state::RaceState, from: f32, to: f32) -> f32 {
    (to - from).rem_euclid(1.0) * st.track_m()
}

fn on_track(c: &crate::games::provider::CarState) -> bool {
    !c.is_player && !c.location.in_pits() && c.location != crate::games::provider::CarLocation::Unknown
}

/// Nearest car ahead that is off track or stopped on it.
fn offtrack_ahead(st: &crate::race::state::RaceState, o: &crate::storage::settings::OverlaySettings) -> Option<CarAlert> {
    let me = st.frame.spline;
    st.cars
        .iter()
        .filter(|c| on_track(c) && st.rivals.get(&c.id).is_some_and(|r| r.off_since.is_some()))
        .map(|c| (ahead_m(st, me, c.spline), c))
        .filter(|(d, _)| *d <= o.alert_offtrack_m)
        .map(|(d, c)| (d, alert(st, c, d)))
        // Short moments (a wheel over the kerb, a slow hairpin) are no incident.
        .filter(|(_, a)| a.seconds >= if a.off_track { o.alert_offtrack_min_s } else { o.alert_stopped_min_s })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, a)| a)
}

/// Nearest car of a faster class closing from behind.
fn faster_behind(st: &crate::race::state::RaceState, max_m: f32) -> Option<CarAlert> {
    let my_lap = st.player_car().and_then(|c| c.class_est_lap_s)?;
    let me = st.frame.spline;
    st.cars
        .iter()
        .filter(|c| on_track(c) && c.class_est_lap_s.is_some_and(|l| l < my_lap * 0.99))
        .map(|c| (ahead_m(st, c.spline, me), c))
        .filter(|(d, _)| *d <= max_m)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(d, c)| CarAlert { gap_s: (c.speed_kmh > 20.0).then(|| d / (c.speed_kmh / 3.6)), ..alert(st, c, d) })
}

/// Approaching the pit entry line (learned from cars driving in).
fn pit_entry(st: &crate::race::state::RaceState, show_m: f32) -> Option<PitEntryView> {
    let f = &st.frame;
    let line = st.pit_entry_pct?;
    if f.in_pit_lane || f.status != SimStatus::Live {
        return None;
    }
    let d = ahead_m(st, f.spline, line);
    (d <= show_m).then_some(PitEntryView { distance_m: d, speed_kmh: f.speed_kmh, limit_kmh: st.session.pit_speed_limit_kmh, limiter_on: f.engine.pit_limiter })
}

fn pit_view(st: &crate::race::state::RaceState, settings: &crate::storage::settings::Settings) -> PitView {
    let f = &st.frame;
    let fs = st.fuel_strategy(settings);
    let pred = crate::race::pit::predict(st, settings, 0, 0.0);
    let box_in_m = st.session.pit_box_pct.map(|b| (b - f.spline).rem_euclid(1.0) * st.track_m()).filter(|d| f.in_pit_lane || *d < BOX_DISTANCE_SHOWN_M);
    let (start, end) = (st.session.pit_window_start_ms, st.session.pit_window_end_ms);
    let elapsed = st.session.session_end_ms - f.session_time_left_ms;
    PitView {
        speed_limit_kmh: st.session.pit_speed_limit_kmh,
        limiter_on: f.engine.pit_limiter,
        in_pit_lane: f.in_pit_lane,
        box_in_m,
        service_active: f.pit_service_active,
        menu: f.pit_menu,
        refuel_needed_l: fs.refuel_l.max(0.0),
        repair_mandatory_s: f.repair.map(|r| r.mandatory_s).unwrap_or(0.0),
        repair_optional_s: f.repair.map(|r| r.optional_s).unwrap_or(0.0),
        pit_loss_s: pred.pit_loss_s,
        exit_position: pred.exit_position,
        mandatory_stops_left: f.missing_mandatory_pits,
        window_open: (end > start && st.session.session_end_ms > 0.0).then(|| elapsed >= f64::from(start) && elapsed < f64::from(end)),
    }
}
