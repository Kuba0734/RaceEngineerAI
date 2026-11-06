//! Test mode: what the game delivers right now (normalised values with plausibility checks and
//! raw values of the game), and checks of recorded sessions.

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::games::provider::{ProviderEvent, SimStatus};
use crate::race::state::{RaceEvent, RaceState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CheckStatus {
    Ok,
    /// Value present but outside what is plausible.
    Warn,
    /// Expected but not delivered.
    Missing,
    /// Shown for information, no expectation.
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CheckRow {
    pub group: String,
    pub name: String,
    pub value: String,
    pub status: CheckStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DataCheck {
    pub game: String,
    pub frame_hz: f32,
    pub cars_hz: f32,
    pub rows: Vec<CheckRow>,
    /// Raw values straight from the game (name, value).
    pub raw: Vec<(String, String)>,
    pub recording: Option<String>,
}

struct Rows(Vec<CheckRow>);

impl Rows {
    fn add(&mut self, group: &str, name: &str, value: String, status: CheckStatus) {
        self.0.push(CheckRow { group: group.into(), name: name.into(), value, status });
    }
    /// Number with a plausible range; None or NaN = missing.
    fn num(&mut self, group: &str, name: &str, v: Option<f64>, range: (f64, f64), digits: usize) {
        match v.filter(|x| x.is_finite()) {
            Some(x) => self.add(group, name, format!("{x:.digits$}"), if x >= range.0 && x <= range.1 { CheckStatus::Ok } else { CheckStatus::Warn }),
            None => self.add(group, name, "—".into(), CheckStatus::Missing),
        }
    }
    fn info(&mut self, group: &str, name: &str, value: String) {
        self.add(group, name, value, CheckStatus::Info);
    }
}

/// Normalised values of the running session with plausibility checks.
pub fn check(st: &RaceState) -> Vec<CheckRow> {
    let f = &st.frame;
    let s = &st.session;
    let mut r = Rows(Vec::new());
    let live = f.status == SimStatus::Live;
    r.add("telemetry", "status", format!("{:?}", f.status), if live { CheckStatus::Ok } else { CheckStatus::Info });
    r.info("session", "game", s.game_id.clone());
    r.add("session", "track", s.track.clone(), if s.track.is_empty() { CheckStatus::Missing } else { CheckStatus::Ok });
    r.num("session", "track_length_m", Some(f64::from(s.track_length_m)), (500.0, 30_000.0), 0);
    r.add("session", "car", s.car_model.clone(), if s.car_model.is_empty() { CheckStatus::Missing } else { CheckStatus::Ok });
    r.info("session", "car_class", s.car_class.clone());
    r.info("session", "session_type", format!("{:?} / {:?}", s.session_type, s.phase));
    r.num("session", "max_fuel_l", Some(f64::from(s.max_fuel_l)), (1.0, 400.0), 1);
    r.num("session", "race_laps", s.race_laps.map(f64::from), (1.0, 2000.0), 0);
    r.num("session", "race_time_min", s.race_time_s.map(|t| t / 60.0), (1.0, 1500.0), 0);
    r.num("session", "pit_speed_limit_kmh", s.pit_speed_limit_kmh.map(f64::from), (20.0, 120.0), 0);
    r.num("session", "pit_box_pct", s.pit_box_pct.map(f64::from), (0.0, 1.0), 3);
    r.num("session", "pit_entry_pct", st.pit_entry_pct.map(f64::from), (0.0, 1.0), 3);
    r.num("session", "est_lap_s", s.est_lap_s.map(f64::from), (20.0, 900.0), 1);
    r.info("session", "incident_limit", s.incident_limit.map_or("—".into(), |v| v.to_string()));

    r.num("driver", "position", Some(f64::from(st.player_position())), (1.0, 200.0), 0);
    r.num("driver", "position_game", Some(f64::from(f.position)), (1.0, 200.0), 0);
    r.num("driver", "completed_laps", Some(f64::from(f.completed_laps)), (0.0, 2000.0), 0);
    r.num("driver", "progress", f.progress, (-1.0, 2001.0), 3);
    r.num("driver", "spline", Some(f64::from(f.spline)), (0.0, 1.0), 3);
    r.num("driver", "last_lap_s", (f.last_lap_ms > 0).then(|| f64::from(f.last_lap_ms) / 1000.0), (20.0, 900.0), 3);
    r.num("driver", "best_lap_s", (f.best_lap_ms > 0).then(|| f64::from(f.best_lap_ms) / 1000.0), (20.0, 900.0), 3);
    r.num("driver", "delta_s", Some(f64::from(f.delta_ms) / 1000.0), (-60.0, 60.0), 2);
    r.info("driver", "incidents", f.incidents.map_or("—".into(), |v| v.to_string()));

    r.num("inputs", "speed_kmh", Some(f64::from(f.speed_kmh)), (0.0, 450.0), 1);
    r.num("inputs", "throttle", Some(f64::from(f.throttle)), (0.0, 1.0), 2);
    r.num("inputs", "brake", Some(f64::from(f.brake)), (0.0, 1.0), 2);
    r.num("inputs", "clutch", Some(f64::from(f.clutch)), (0.0, 1.0), 2);
    r.num("inputs", "steer", Some(f64::from(f.steer)), (-1.0, 1.0), 2);
    r.info("inputs", "gear / rpm", format!("{} / {}", f.gear, f.rpm));
    r.num("inputs", "fuel_l", Some(f64::from(f.fuel_l)), (0.0, 400.0), 2);

    let w = &f.weather;
    r.num("weather", "air_c", Some(f64::from(w.air_temp_c)), (-30.0, 60.0), 1);
    r.num("weather", "track_c", Some(f64::from(w.road_temp_c)), (-30.0, 80.0), 1);
    r.num("weather", "humidity_pct", w.humidity_pct.map(f64::from), (0.0, 100.0), 0);
    r.num("weather", "wind_dir_deg", w.wind_dir_deg.map(f64::from), (0.0, 360.0), 0);
    r.num("weather", "wind_kmh", Some(f64::from(w.wind_speed_ms) * 3.6), (0.0, 200.0), 1);
    r.info("weather", "rain / grip", format!("{:?} / {:?}", w.rain_now, w.grip));

    r.info("flags", "flag", format!("{:?}", f.flag));
    r.info("flags", "start_light", format!("{:?}", f.start_light));
    r.info("flags", "yellow_sectors", format!("{:?}", f.yellow_sectors));
    r.info("flags", "penalty", f.penalty.map_or("—".into(), |p| format!("{:?}", p.kind)));

    r.info("pit", "in_pit_lane / box", format!("{} / {}", f.in_pit_lane, f.in_pit_box));
    r.info("pit", "limiter", f.engine.pit_limiter.to_string());
    r.info("pit", "service_active", f.pit_service_active.to_string());
    r.info("pit", "menu", f.pit_menu.map_or("—".into(), |m| format!("{} L, tyres {:?}", m.fuel_l, m.tyres)));

    let cars = &st.cars;
    r.num("cars", "count", Some(cars.len() as f64), (1.0, 200.0), 0);
    let mut pos: Vec<i32> = cars.iter().map(|c| st.rank(c)).filter(|p| *p > 0).collect();
    pos.sort_unstable();
    let dupes = pos.windows(2).any(|w| w[0] == w[1]) && !st.multiclass();
    r.add("cars", "positions_unique", (!dupes).to_string(), if dupes { CheckStatus::Warn } else { CheckStatus::Ok });
    r.info("cars", "multiclass", st.multiclass().to_string());
    r.info("cars", "off_track_now", cars.iter().filter(|c| c.off_track).count().to_string());
    r.num("cars", "with_progress", Some(cars.iter().filter(|c| c.progress.is_some()).count() as f64), (1.0, 200.0), 0);
    r.0
}

/// Problems found when a recording is played through the race logic.
pub fn check_recording(path: &Path) -> std::io::Result<Vec<String>> {
    let mut rp = crate::replay::ReplayProvider::open(path, 1.0)?;
    let settings = crate::storage::settings::Settings::default();
    let mut st = RaceState::new();
    let mut problems: Vec<String> = Vec::new();
    let mut finished = 0;
    let mut out = Vec::new();
    let mut spot = Vec::new();
    let mut events = Vec::new();
    let mut t = 0u64;
    let mut seen_cars = 0usize;
    let mut last_pos = 0i32;
    loop {
        out.clear();
        rp.advance_to(t, &mut out);
        if out.is_empty() && rp.done() {
            break;
        }
        for ev in out.drain(..) {
            events.clear();
            match ev {
                ProviderEvent::Frame(f) => st.on_frame(*f, &settings, &mut spot, &mut events),
                ProviderEvent::Session(s) => st.on_session(*s, &mut events),
                ProviderEvent::Cars(c) => {
                    seen_cars += 1;
                    st.on_cars(c, &mut events);
                    let p = st.player_position();
                    if st.race_running() && p == 0 && problems.len() < 50 {
                        problems.push(format!("{:.1} s: own position 0 while the race runs", t as f64 / 1000.0));
                    }
                    if last_pos > 0 && p > 0 && (p - last_pos).abs() > 3 && st.race_running() && problems.len() < 50 {
                        problems.push(format!("{:.1} s: own position jumps P{last_pos} → P{p}", t as f64 / 1000.0));
                    }
                    last_pos = p;
                }
                _ => {}
            }
            finished += events.iter().filter(|e| matches!(e, RaceEvent::Finished)).count();
        }
        t += 50;
    }
    if finished > 1 {
        problems.push(format!("finish detected {finished} times"));
    }
    if seen_cars == 0 {
        problems.push("no car data in the recording".into());
    }
    Ok(problems)
}

#[cfg(test)]
mod tests {
    /// Real sessions placed in `tests/replays/` (recorded in test mode) must play through the
    /// race logic without problems. Without recordings this test checks nothing.
    #[test]
    fn recorded_sessions_play_without_problems() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("replays");
        let Ok(entries) = std::fs::read_dir(&dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "jsonl") {
                let problems = super::check_recording(&p).unwrap();
                assert!(problems.is_empty(), "{}: {problems:#?}", p.display());
            }
        }
    }
}
