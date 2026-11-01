//! Tool registry for the AI: query tools read the deterministic race logic, action tools
//! change the crew state. Definitions (name, description, JSON schema) are generated here.

use serde_json::{json, Value};

use crate::calls::commands::QuietUntil;
use crate::calls::engine::CallEngine;
use crate::calls::format::Fmt;
use crate::games::provider::CarState;
use crate::race::{pit, state::RaceState};
use crate::storage::settings::{FuelUnit, Settings};

pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
    pub action: bool,
}

fn obj(props: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": required })
}

pub fn definitions() -> Vec<ToolDef> {
    let none = || obj(json!({}), &[]);
    let car_query = json!({ "type": "string", "description": "Which car: 'ahead', 'behind', 'leader', 'P3' (race position), '#88' (race number), a driver name, a car brand ('Porsche'), or 'fastest'." });
    vec![
        ToolDef { name: "get_standings", description: "Race order with gaps to the player (s, + = ahead), last/best laps, pit stops, tyre age.", parameters: obj(json!({ "limit": { "type": "integer" } }), &[]), action: false },
        ToolDef { name: "get_car", description: "Details of one car: position, number, driver, car, gap to player, trend per lap, last/best lap and sector splits, pit stops, tyre age, in pits.", parameters: obj(json!({ "query": car_query }), &["query"]), action: false },
        ToolDef { name: "get_gaps", description: "Gaps to the cars directly ahead and behind with trends (s/lap) and catch prediction.", parameters: none(), action: false },
        ToolDef { name: "get_lap_history", description: "Player's recent laps: time, sectors, valid, fuel used, position.", parameters: obj(json!({ "count": { "type": "integer" } }), &[]), action: false },
        ToolDef { name: "get_sector_comparison", description: "Compare the player's sectors with another car (last and best splits).", parameters: obj(json!({ "query": car_query }), &["query"]), action: false },
        ToolDef { name: "get_corner_analysis", description: "Corner-by-corner comparison of the last lap with the best lap: time lost per corner, braking point, apex speed, throttle.", parameters: none(), action: false },
        ToolDef { name: "get_fuel_strategy", description: "Fuel now, consumption per lap, laps possible, laps to go, fuel needed, refuel amount, saving target, start fuel for the next race.", parameters: obj(json!({ "race_minutes": { "type": "number", "description": "Optional race length for the start fuel calculation." } }), &[]), action: false },
        ToolDef { name: "simulate_pit_stop", description: "Predict where the player rejoins if pitting in N laps (0 = this lap), with refuel amount.", parameters: obj(json!({ "in_laps": { "type": "integer" }, "extra_seconds": { "type": "number", "description": "Extra stationary time, e.g. tyre change." } }), &[]), action: false },
        ToolDef { name: "get_tyres", description: "Tyre core temperatures, pressures, target window, recommendation for the next stint, wet tyres on.", parameters: none(), action: false },
        ToolDef { name: "get_brakes", description: "Brake temperatures, pad and disc life and estimated laps left.", parameters: none(), action: false },
        ToolDef { name: "get_damage", description: "Car damage per zone and suspension.", parameters: none(), action: false },
        ToolDef { name: "get_weather", description: "Rain now / in 10 / in 30 minutes, track grip, air and track temperature and trend.", parameters: none(), action: false },
        ToolDef { name: "get_session_info", description: "Track, session type, phase, time left, laps, position, best/last lap, pit window, mandatory stop.", parameters: none(), action: false },
        ToolDef { name: "get_penalties", description: "Current penalty, track limits count, flags.", parameters: none(), action: false },
        ToolDef { name: "get_session_summary", description: "Summary so far: positions gained, best lap, consistency, biggest time loss.", parameters: none(), action: false },
        ToolDef { name: "set_quiet", description: "Pause all non-critical radio calls (spotter and critical calls stay). Give laps OR minutes OR until_corner, or off=true to end.", parameters: obj(json!({ "laps": { "type": "integer" }, "minutes": { "type": "number" }, "until_corner": { "type": "integer" }, "off": { "type": "boolean" } }), &[]), action: true },
        ToolDef { name: "set_preset", description: "Switch the talkativeness preset temporarily: spotter_only, minimal, standard, full, endurance, qualifying, or 'normal' to go back.", parameters: obj(json!({ "preset": { "type": "string" } }), &["preset"]), action: true },
        ToolDef { name: "add_recurring_call", description: "Say a call every N laps from now on. call_id e.g. gap.behind, gap.ahead, fuel.status, race.time_left, tyre.pressure_live. every_laps=0 removes it.", parameters: obj(json!({ "call_id": { "type": "string" }, "every_laps": { "type": "integer" } }), &["call_id", "every_laps"]), action: true },
        ToolDef { name: "add_reminder", description: "Remind the driver later. Give in_laps OR at_lap OR in_minutes, plus the text.", parameters: obj(json!({ "in_laps": { "type": "integer" }, "at_lap": { "type": "integer" }, "in_minutes": { "type": "number" }, "text": { "type": "string" } }), &["text"]), action: true },
        ToolDef { name: "set_strategy_plan", description: "Set the planned pit lap (all calculations use it); null/0 clears it.", parameters: obj(json!({ "pit_lap": { "type": "integer" } }), &[]), action: true },
        ToolDef { name: "repeat_last", description: "Repeat the last radio message.", parameters: none(), action: true },
        ToolDef { name: "set_volume", description: "Change crew voice volume: direction 'up' or 'down', or level 0-150 (percent).", parameters: obj(json!({ "direction": { "type": "string" }, "level": { "type": "number" } }), &[]), action: true },
        ToolDef { name: "toggle_call", description: "Enable or disable one radio call permanently, by call_id (e.g. lap.corner_feedback, gap.ahead).", parameters: obj(json!({ "call_id": { "type": "string" }, "enabled": { "type": "boolean" } }), &["call_id", "enabled"]), action: true },
    ]
}

pub fn openai_tools() -> Value {
    Value::Array(
        definitions()
            .into_iter()
            .map(|d| json!({ "type": "function", "function": { "name": d.name, "description": d.description, "parameters": d.parameters } }))
            .collect(),
    )
}

fn round1(v: f32) -> f64 {
    (f64::from(v) * 10.0).round() / 10.0
}
fn round2(v: f32) -> f64 {
    (f64::from(v) * 100.0).round() / 100.0
}

fn fuel_unit(s: &Settings) -> (&'static str, f32) {
    match s.units.fuel {
        FuelUnit::Liters => ("l", 1.0),
        FuelUnit::Gallons => ("gal", 0.264_172),
    }
}

fn lap_s(ms: Option<i32>) -> Value {
    ms.filter(|m| *m > 0 && *m < i32::MAX).map(|m| json!((f64::from(m) / 1000.0 * 1000.0).round() / 1000.0)).unwrap_or(Value::Null)
}

/// Resolves a free-form car reference.
pub fn find_car<'a>(state: &'a RaceState, q: &str) -> Option<&'a CarState> {
    let q = q.trim().to_lowercase();
    let pos = state.player_position();
    match q.as_str() {
        "ahead" | "vor mir" | "vorne" | "front" => return state.neighbour(-1),
        "behind" | "hinter mir" | "hinten" | "back" => return state.neighbour(1),
        "leader" | "führender" | "p1" => return state.cars.iter().find(|c| c.position == 1),
        "me" | "player" | "ich" => return state.player_car(),
        "fastest" | "schnellster" => return state.cars.iter().filter(|c| c.last_lap_ms.is_some()).min_by_key(|c| c.last_lap_ms.unwrap_or(i32::MAX)),
        _ => {}
    }
    if let Some(p) = q.strip_prefix('p').and_then(|x| x.parse::<i32>().ok()) {
        return state.cars.iter().find(|c| c.position == p);
    }
    if let Ok(n) = q.trim_start_matches('#').trim_start_matches("nummer ").trim_start_matches("number ").parse::<i32>() {
        return state.cars.iter().find(|c| c.race_number == n);
    }
    // Brand or driver; prefer the closest position to the player.
    state
        .cars
        .iter()
        .filter(|c| !c.is_player && (c.car_brand.to_lowercase().contains(&q) || c.car_model.to_lowercase().contains(&q) || c.driver.to_lowercase().contains(&q) || c.team.to_lowercase().contains(&q)))
        .min_by_key(|c| (c.position - pos).abs())
}

fn car_json(state: &RaceState, c: &CarState, f: &Fmt) -> Value {
    let me = state.frame.player_car_id;
    let r = state.rivals.get(&c.id);
    json!({
        "name": f.car(c),
        "position": c.position,
        "race_number": c.race_number,
        "driver": c.driver,
        "car": c.car_model,
        "laps": c.laps,
        "gap_to_player_s": state.gaps.gap(me, c.id).map(|g| (g * 10.0).round() / 10.0),
        "trend_s_per_lap": state.gaps.trend(c.id).map(|t| (t * 100.0).round() / 100.0),
        "last_lap_s": lap_s(c.last_lap_ms),
        "best_lap_s": lap_s(c.best_lap_ms),
        "last_sectors_s": c.last_splits.iter().map(|s| lap_s(*s)).collect::<Vec<_>>(),
        "best_sectors_s": c.best_splits.iter().map(|s| lap_s(*s)).collect::<Vec<_>>(),
        "in_pits": c.location.in_pits(),
        "pit_stops": r.map(|r| r.pit_stops).unwrap_or(0),
        "tyre_age_laps": r.and_then(|r| r.last_pit_lap).map(|l| c.laps - l).unwrap_or(c.laps),
    })
}

/// Executes a query tool. Never fails: errors become `{"error": ...}` so the model can say so.
pub fn query(name: &str, args: &Value, state: &RaceState, settings: &Settings) -> Value {
    let f = Fmt::new(settings);
    let me = state.frame.player_car_id;
    let (fu, fk) = fuel_unit(settings);
    if !state.has_frame && name != "get_session_info" {
        return json!({ "error": "no live data – the game is not running or not in a session" });
    }
    match name {
        "get_standings" => {
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(30) as usize;
            let rows: Vec<Value> = state.standings(5).iter().take(limit).map(|r| {
                json!({ "pos": r.position, "num": r.race_number, "driver": r.driver, "car": r.brand, "gap_to_player_s": r.gap_to_player_s.map(round1), "trend_s_per_lap": r.trend_s_per_lap.map(round2), "last_s": lap_s(r.last_lap_ms), "best_s": lap_s(r.best_lap_ms), "pit_stops": r.pit_stops, "tyre_age_laps": r.tyre_age_laps, "in_pits": r.in_pits, "is_player": r.is_player })
            }).collect();
            if rows.is_empty() {
                json!({ "error": "no standings – broadcasting data not available" })
            } else {
                json!({ "standings": rows })
            }
        }
        "get_car" => match args.get("query").and_then(|q| q.as_str()).and_then(|q| find_car(state, q)) {
            Some(c) => car_json(state, c, &f),
            None => json!({ "error": "car not found", "hint": "ask the driver which car (position or number)" }),
        },
        "get_gaps" => {
            let mut out = serde_json::Map::new();
            for (k, off) in [("ahead", -1), ("behind", 1)] {
                if let Some(c) = state.neighbour(off) {
                    let mut v = car_json(state, c, &f);
                    if k == "ahead" {
                        if let (Some(g), Some(t)) = (state.gaps.gap(me, c.id), state.gaps.trend(c.id)) {
                            if t < -0.02 && g > 0.0 {
                                v["laps_to_catch"] = json!((g / -t).ceil());
                            }
                        }
                    }
                    out.insert(k.into(), v);
                }
            }
            out.insert("player_position".into(), json!(state.player_position()));
            Value::Object(out)
        }
        "get_lap_history" => {
            let n = args.get("count").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
            let laps: Vec<Value> = state.laps.iter().rev().take(n).map(|l| json!({ "lap": l.lap, "time_s": lap_s(Some(l.time_ms)), "sectors_s": l.sectors.iter().map(|s| lap_s(*s)).collect::<Vec<_>>(), "valid": l.valid, "fuel_used": l.fuel_used_l.map(|x| round2(x * fk)), "position": l.position, "in_lap": l.in_lap, "out_lap": l.out_lap })).collect();
            json!({ "laps": laps, "fuel_unit": fu, "best_s": lap_s(state.best_valid_lap().map(|l| l.time_ms)) })
        }
        "get_sector_comparison" => {
            let Some(c) = args.get("query").and_then(|q| q.as_str()).and_then(|q| find_car(state, q)) else { return json!({ "error": "car not found" }) };
            let mine_last = state.laps.last().map(|l| l.sectors).unwrap_or([None; 3]);
            let mine_best: Vec<Option<i32>> = (0..3).map(|k| state.laps.iter().filter_map(|l| l.sectors[k]).min()).collect();
            let diff = |a: Option<i32>, b: Option<i32>| -> Value { match (a, b) { (Some(a), Some(b)) => json!(((b - a) as f64 / 1000.0 * 100.0).round() / 100.0), _ => Value::Null } };
            json!({
                "car": f.car(c),
                "note": "delta = other − player in seconds; positive means the player is faster in that sector",
                "last_lap_delta_s": (0..3).map(|k| diff(mine_last[k], c.last_splits[k])).collect::<Vec<_>>(),
                "best_delta_s": (0..3).map(|k| diff(mine_best[k], c.best_splits[k])).collect::<Vec<_>>(),
            })
        }
        "get_corner_analysis" => {
            if state.last_corner_deltas.is_empty() {
                return json!({ "error": "no comparison yet – needs a reference lap and a full clean lap" });
            }
            let v: Vec<Value> = state.last_corner_deltas.iter().map(|d| json!({ "corner": d.corner, "time_loss_s": round2(d.time_loss_s), "braked_earlier_by_m": d.brake_point_delta_m.map(|x| x.round()), "apex_speed_delta_kmh": d.apex_speed_delta_kmh.round(), "throttle_later_by_m": d.throttle_delta_m.map(|x| x.round()), "issue": d.issue })).collect();
            json!({ "corners": v, "reference_lap_s": lap_s(state.reference.as_ref().map(|r| r.lap_ms)) })
        }
        "get_fuel_strategy" => {
            let fs = state.fuel_strategy(settings);
            let plan = args.get("_plan_pit_lap").and_then(|v| v.as_i64()).map(|l| l as i32);
            let st = &settings.strategy;
            let minutes = args.get("race_minutes").and_then(|v| v.as_f64()).map(|m| m as f32).unwrap_or(st.next_race_minutes);
            let start = crate::race::fuel::start_fuel(minutes, fs.per_lap_l, fs.avg_lap_ms, st.formation_lap, st.fuel_reserve_laps, st.fuel_reserve_liters, state.session.max_fuel_l);
            json!({
                "unit": fu,
                "fuel_now": round1(fs.fuel_l * fk),
                "per_lap": round2(fs.per_lap_l * fk),
                "from_clean_laps": fs.from_clean_laps,
                "laps_possible": round1(fs.laps_possible),
                "laps_to_go": round1(fs.laps_to_go),
                "fuel_needed_incl_reserve": round1(fs.fuel_needed_l * fk),
                "refuel": round1(fs.refuel_l * fk),
                "enough_without_stop": fs.enough,
                "save_per_lap_to_skip_stop": fs.save_per_lap_l.map(|x| round2(x * fk)),
                "reserve_laps": st.fuel_reserve_laps,
                "planned_pit_lap": plan,
                "fuel_reaches_planned_lap": plan.map(|l| {
                    let laps_until = (l - state.frame.completed_laps - 1) as f32 + (1.0 - state.frame.spline);
                    fs.laps_possible >= laps_until
                }),
                "start_fuel_next_race": { "race_minutes": minutes, "fuel": round1(start * fk) },
            })
        }
        "simulate_pit_stop" => {
            // Without an explicit value the strategy plan (if any) is used.
            let plan_in = args.get("_plan_pit_lap").and_then(|v| v.as_i64()).map(|l| (l as i32 - state.frame.completed_laps - 1).max(0) as u64);
            let in_laps = args.get("in_laps").and_then(|v| v.as_u64()).or(plan_in).unwrap_or(0) as u32;
            let extra = args.get("extra_seconds").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let p = pit::predict(state, settings, in_laps, extra);
            json!({
                "in_laps": in_laps,
                "pit_loss_s": round1(p.pit_loss_s),
                "pit_loss_measured": p.pit_loss_measured,
                "current_position": p.current_position,
                "rejoin_position": p.exit_position,
                "rejoin_behind": p.exit_behind_car.and_then(|id| state.car(id)).map(|c| f.car(c)),
                "gap_to_car_ahead_after_s": p.exit_gap_ahead_s.map(round1),
                "rejoin_ahead_of": p.exit_ahead_of_car.and_then(|id| state.car(id)).map(|c| f.car(c)),
                "gap_to_car_behind_after_s": p.exit_gap_behind_s.map(round1),
                "refuel": round1(p.refuel_l * fk),
                "unit": fu,
            })
        }
        "get_tyres" => {
            let w = &state.frame.wheels;
            let class = crate::games::cars::class_def(&state.session.car_class);
            let window = if state.frame.rain_tyres { class.wet } else { class.dry };
            let rec = crate::calls::triggers::next_stint_deltas(state, window, 0.1);
            json!({
                "order": "FL, FR, RL, RR",
                "core_temp_c": w.iter().map(|x| x.core_temp_c.round()).collect::<Vec<_>>(),
                "pressure_psi": w.iter().map(|x| round1(x.pressure_psi)).collect::<Vec<_>>(),
                "target_hot_pressure_psi": window,
                "cold_below_c": class.core_temp_cold, "hot_above_c": class.core_temp_hot,
                "wet_tyres_on": state.frame.rain_tyres,
                "next_stint_pressure_change_psi": rec.iter().map(|(i, d)| json!({ "wheel": f.wheel(*i), "delta": d })).collect::<Vec<_>>(),
                "pressure_unit_for_speech": format!("{:?}", settings.units.pressure),
            })
        }
        "get_brakes" => {
            let w = &state.frame.wheels;
            json!({ "order": "FL, FR, RL, RR", "temp_c": w.iter().map(|x| x.brake_temp_c.round()).collect::<Vec<_>>(), "pad_mm": w.iter().map(|x| round1(x.pad_life_mm)).collect::<Vec<_>>(), "disc_mm": w.iter().map(|x| round1(x.disc_life_mm)).collect::<Vec<_>>(), "laps_to_go": round1(state.fuel_strategy(settings).laps_to_go) })
        }
        "get_damage" => {
            let d = state.frame.damage;
            json!({ "zones": { "front": round2(d[0]), "rear": round2(d[1]), "left": round2(d[2]), "right": round2(d[3]), "centre": round2(d[4]) }, "suspension_FL_FR_RL_RR": state.frame.suspension_damage.map(round2), "note": "0 = no damage" })
        }
        "get_weather" => {
            let w = state.frame.weather;
            json!({ "rain_now": w.rain_now, "rain_in_10_min": w.rain_10min, "rain_in_30_min": w.rain_30min, "track_grip": w.grip, "air_temp_c": w.air_temp_c.round(), "track_temp_c": w.road_temp_c.round(), "track_temp_trend_c_per_10min": state.road_temp_trend_c_per_10min().map(round1), "wet_tyres_on": state.frame.rain_tyres })
        }
        "get_session_info" => {
            let s = &state.session;
            json!({
                "live": state.has_frame,
                "game": s.game_id, "track": s.track, "track_length_m": s.track_length_m.round(),
                "session": s.session_type, "phase": s.phase, "car": s.car_model, "class": s.car_class,
                "time_left_min": (state.frame.session_time_left_ms / 60_000.0 * 10.0).round() / 10.0,
                "completed_laps": state.frame.completed_laps, "position": state.player_position(),
                "best_lap_s": lap_s(state.best_valid_lap().map(|l| l.time_ms)), "last_lap_s": lap_s(Some(state.frame.last_lap_ms)),
                "mandatory_pit_done": state.frame.mandatory_pit_done, "missing_mandatory_pits": state.frame.missing_mandatory_pits,
                "pit_window_ms": [s.pit_window_start_ms, s.pit_window_end_ms],
                "standings_available": !state.cars.is_empty(),
            })
        }
        "get_penalties" => json!({ "penalty": state.frame.penalty, "penalty_time_s": state.frame.penalty_time_s, "track_limits": state.track_limits, "flag": state.frame.flag, "yellow_sectors": state.frame.yellow_sectors }),
        "get_session_summary" => {
            let pos = state.player_position();
            let valid: Vec<i32> = state.laps.iter().filter(|l| l.valid && l.clean).map(|l| l.time_ms).collect();
            let spread = if valid.len() >= 3 { Some((valid.iter().max().unwrap_or(&0) - valid.iter().min().unwrap_or(&0)) as f32 / 1000.0) } else { None };
            json!({
                "position": pos, "start_position": state.race_start_position,
                "laps": state.laps.len(), "best_lap_s": lap_s(state.best_valid_lap().map(|l| l.time_ms)),
                "clean_lap_spread_s": spread.map(round2), "track_limits": state.track_limits,
                "biggest_corner_loss": state.last_corner_deltas.iter().max_by(|a, b| a.time_loss_s.total_cmp(&b.time_loss_s)).map(|d| json!({ "corner": d.corner, "loss_s": round2(d.time_loss_s) })),
            })
        }
        _ => json!({ "error": format!("unknown tool {name}") }),
    }
}

/// Result of an action tool: JSON for the model + an optional speech side effect.
pub struct ActionResult {
    pub value: Value,
    pub repeat: Option<String>,
    pub settings_changed: bool,
}

pub fn action(name: &str, args: &Value, state: &RaceState, engine: &mut CallEngine, settings: &mut Settings) -> ActionResult {
    let lap = state.frame.completed_laps;
    let t = state.t_s();
    let ok = |v: Value| ActionResult { value: v, repeat: None, settings_changed: false };
    let cmd_off = |id: &str| !settings.commands.is_enabled(id);
    match name {
        "set_quiet" => {
            if cmd_off("quiet") {
                return ok(json!({ "error": "this voice command is disabled in the settings" }));
            }
            if args.get("off").and_then(|v| v.as_bool()).unwrap_or(false) {
                engine.crew.quiet = None;
                return ok(json!({ "ok": true, "quiet": false }));
            }
            let q = if let Some(l) = args.get("laps").and_then(|v| v.as_i64()) {
                QuietUntil::Lap { lap: lap + l as i32 }
            } else if let Some(m) = args.get("minutes").and_then(|v| v.as_f64()) {
                QuietUntil::Time { t: t + m * 60.0 }
            } else if let Some(c) = args.get("until_corner").and_then(|v| v.as_u64()) {
                match state.corners.iter().find(|x| x.number == c as u32) {
                    Some(corner) => {
                        let sp = corner.apex_m / state.track_m();
                        let target_lap = if sp > state.frame.spline { lap } else { lap + 1 };
                        QuietUntil::Spline { lap: target_lap, spline: sp }
                    }
                    None => QuietUntil::Lap { lap: lap + 1 },
                }
            } else {
                QuietUntil::Lap { lap: lap + 2 }
            };
            engine.crew.quiet = Some(q);
            ok(json!({ "ok": true }))
        }
        "set_preset" => {
            if cmd_off("preset") {
                return ok(json!({ "error": "this voice command is disabled in the settings" }));
            }
            let p = args.get("preset").and_then(|v| v.as_str()).unwrap_or("normal");
            if p == "normal" || p == "standard_back" {
                engine.crew.preset_override = None;
            } else if crate::calls::catalog::presets().iter().any(|x| x.id == p) || settings.calls.custom_presets.contains_key(p) {
                engine.crew.preset_override = Some(p.to_string());
            } else {
                return ok(json!({ "error": "unknown preset", "available": crate::calls::catalog::presets().iter().map(|x| x.id.clone()).collect::<Vec<_>>() }));
            }
            engine.refresh(settings);
            ok(json!({ "ok": true, "preset": engine.crew.preset_override }))
        }
        "add_recurring_call" => {
            if cmd_off("recurring") {
                return ok(json!({ "error": "this voice command is disabled in the settings" }));
            }
            let id = args.get("call_id").and_then(|v| v.as_str()).unwrap_or("");
            if crate::calls::catalog::def(id).is_none() {
                return ok(json!({ "error": "unknown call_id" }));
            }
            let n = args.get("every_laps").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
            engine.crew.add_recurring(id, n, lap);
            ok(json!({ "ok": true }))
        }
        "add_reminder" => {
            if cmd_off("reminder") {
                return ok(json!({ "error": "this voice command is disabled in the settings" }));
            }
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let at_lap = args.get("at_lap").and_then(|v| v.as_i64()).map(|x| x as i32).or_else(|| args.get("in_laps").and_then(|v| v.as_i64()).map(|x| lap + x as i32));
            let at_t = args.get("in_minutes").and_then(|v| v.as_f64()).map(|m| t + m * 60.0);
            if at_lap.is_none() && at_t.is_none() {
                return ok(json!({ "error": "need in_laps, at_lap or in_minutes" }));
            }
            let id = engine.crew.add_reminder(at_lap, at_t, text);
            ok(json!({ "ok": true, "id": id, "at_lap": at_lap }))
        }
        "set_strategy_plan" => {
            if cmd_off("plan") {
                return ok(json!({ "error": "this voice command is disabled in the settings" }));
            }
            let l = args.get("pit_lap").and_then(|v| v.as_i64()).filter(|l| *l > 0).map(|l| l as i32);
            engine.crew.plan_pit_lap = l;
            ok(json!({ "ok": true, "pit_lap": l, "current_lap": lap + 1 }))
        }
        "repeat_last" => ActionResult { value: json!({ "ok": true, "last": engine.crew.last_message }), repeat: engine.crew.last_message.clone(), settings_changed: false },
        "set_volume" => {
            if cmd_off("volume") {
                return ok(json!({ "error": "this voice command is disabled in the settings" }));
            }
            let v = &mut settings.voices.master_volume;
            if let Some(level) = args.get("level").and_then(|v| v.as_f64()) {
                *v = (level as f32 / 100.0).clamp(0.0, 1.5);
            } else {
                match args.get("direction").and_then(|v| v.as_str()) {
                    Some("down") => *v = (*v - 0.15).max(0.1),
                    _ => *v = (*v + 0.15).min(1.5),
                }
            }
            ActionResult { value: json!({ "ok": true, "volume_percent": (*v * 100.0).round() }), repeat: None, settings_changed: true }
        }
        "toggle_call" => {
            let id = args.get("call_id").and_then(|v| v.as_str()).unwrap_or("");
            if crate::calls::catalog::def(id).is_none() {
                return ok(json!({ "error": "unknown call_id" }));
            }
            let en = args.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
            settings.calls.overrides.entry(id.to_string()).or_default().enabled = Some(en);
            engine.refresh(settings);
            ActionResult { value: json!({ "ok": true }), repeat: None, settings_changed: true }
        }
        _ => ok(json!({ "error": format!("unknown tool {name}") })),
    }
}

pub fn is_action(name: &str) -> bool {
    definitions().iter().any(|d| d.name == name && d.action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_is_valid_json_for_every_tool() {
        let t = openai_tools();
        assert!(t.as_array().unwrap().len() >= 20);
        for d in definitions() {
            assert_eq!(d.parameters["type"], "object");
        }
    }

    #[test]
    fn find_car_variants() {
        let mut s = RaceState::new();
        let mk = |id, pos, num, brand: &str, driver: &str| CarState { id, position: pos, race_number: num, car_brand: brand.into(), driver: driver.into(), is_player: id == 0, ..Default::default() };
        s.cars = vec![mk(0, 4, 7, "BMW", "Me"), mk(1, 3, 88, "Porsche", "Müller"), mk(2, 5, 31, "Ferrari", "Rossi")];
        s.frame.player_car_id = 0;
        s.frame.position = 4;
        assert_eq!(find_car(&s, "ahead").unwrap().id, 1);
        assert_eq!(find_car(&s, "behind").unwrap().id, 2);
        assert_eq!(find_car(&s, "P3").unwrap().id, 1);
        assert_eq!(find_car(&s, "#31").unwrap().id, 2);
        assert_eq!(find_car(&s, "der porsche".trim_start_matches("der ")).unwrap().id, 1);
        assert_eq!(find_car(&s, "rossi").unwrap().id, 2);
    }

    #[test]
    fn quiet_action() {
        let s = RaceState::new();
        let mut settings = Settings::default();
        let mut e = CallEngine::new(&settings);
        action("set_quiet", &json!({ "laps": 2 }), &s, &mut e, &mut settings);
        assert!(e.crew.quiet.is_some());
        let r = action("set_preset", &json!({ "preset": "spotter_only" }), &s, &mut e, &mut settings);
        assert_eq!(r.value["ok"], true);
        assert!(!e.effective["gap.ahead"].enabled);
    }
}
