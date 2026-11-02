//! Tyres, brakes, damage, assists, lights, wipers, stint time.

use super::{Ctx, Memory};
use crate::calls::engine::CallRequest;
use crate::games::cars;
use crate::games::provider::{RainIntensity, TrackGrip};
use crate::race::state::{RaceEvent, RaceState};

fn class(ctx: &Ctx) -> cars::ClassDef {
    cars::class_def(&ctx.state.session.car_class)
}

fn target_window(ctx: &Ctx) -> [f32; 2] {
    let c = class(ctx);
    if ctx.state.frame.rain_tyres {
        c.wet
    } else {
        c.dry
    }
}

/// Pressure recommendation per wheel (psi delta to the centre of the target window).
pub fn next_stint_deltas(state: &RaceState, window: [f32; 2], min_delta: f32) -> Vec<(usize, f32)> {
    let laps: Vec<_> = state.laps.iter().rev().filter(|l| l.clean && l.avg_pressure[0] > 1.0).take(3).collect();
    if laps.is_empty() {
        return Vec::new();
    }
    let centre = (window[0] + window[1]) / 2.0;
    (0..4)
        .filter_map(|i| {
            let avg = laps.iter().map(|l| l.avg_pressure[i]).sum::<f32>() / laps.len() as f32;
            let d = centre - avg;
            (d.abs() >= min_delta).then_some((i, (d * 10.0).round() / 10.0))
        })
        .collect()
}

pub fn on_event(ctx: &mut Ctx, mem: &mut Memory, ev: &RaceEvent) {
    let s = ctx.state;
    match ev {
        RaceEvent::LapCompleted(rec) => {
            // Live pressures vs window (per axle, hot pressures of the lap).
            let w = target_window(ctx);
            let tol = ctx.th("tyre.pressure_live", "tolerance_psi") as f32;
            if rec.avg_pressure[0] > 1.0 {
                for front in [true, false] {
                    let (a, b) = if front { (0, 1) } else { (2, 3) };
                    let p = (rec.avg_pressure[a] + rec.avg_pressure[b]) / 2.0;
                    let sub = if p > w[1] + tol {
                        Some("high")
                    } else if p < w[0] - tol {
                        Some("low")
                    } else {
                        None
                    };
                    if let Some(sub) = sub {
                        ctx.push(CallRequest::new("tyre.pressure_live").variant(sub).p("where", ctx.fmt.axle(front).to_string()).p("pressure", ctx.fmt.pressure(p)).key(front.to_string()));
                        break;
                    }
                }
            }
            // Pad wear rate.
            let pad = s.frame.wheels[0].pad_life_mm.min(s.frame.wheels[1].pad_life_mm);
            if let Some(start) = mem.pad_life_lap_start {
                let used = start - pad;
                if used > 0.0 && used < 2.0 {
                    mem.pad_wear_per_lap = Some(mem.pad_wear_per_lap.map_or(used, |w| w * 0.7 + used * 0.3));
                }
            }
            mem.pad_life_lap_start = Some(pad);
            if let Some(rate) = mem.pad_wear_per_lap.filter(|r| *r > 0.0) {
                let min_mm = ctx.th("brake.wear", "min_pad_mm") as f32;
                let laps = ((pad - min_mm) / rate).max(0.0);
                let left = s.fuel_strategy(ctx.settings).laps_to_go;
                if laps < left {
                    ctx.push(CallRequest::new("brake.wear").p("laps", ctx.fmt.laps(laps)));
                }
            }
            // Damage: pace loss after the damage lap.
            if let (Some(dlap), Some(before)) = (mem.damage_lap, mem.pace_before_damage_ms) {
                let after: Vec<i32> = s.laps.iter().filter(|l| l.lap > dlap && l.valid && !l.in_lap).map(|l| l.time_ms).collect();
                if after.len() >= 2 {
                    let avg = after.iter().map(|x| *x as i64).sum::<i64>() / after.len() as i64;
                    let loss = (avg as i32 - before) as f32 / 1000.0;
                    let left = s.fuel_strategy(ctx.settings).laps_to_go;
                    let repair = mem.repair_estimate_s.unwrap_or_else(|| ctx.th("damage.report", "repair_estimate_s") as f32);
                    let sub = if loss * left > repair { "worth" } else { "not_worth" };
                    mem.damage_lap = None;
                    if loss > 0.1 {
                        ctx.push(CallRequest::new("damage.report").variant(sub).p("loss", ctx.fmt.tenths(loss)).p("repair", format!("{}", repair as i32)).key("pace"));
                    }
                }
            }
            lockup_report(ctx, mem);
        }
        RaceEvent::PitEntry => {
            let min_delta = ctx.th("tyre.pressure_next_stint", "min_delta_psi") as f32;
            let d = next_stint_deltas(s, target_window(ctx), min_delta);
            if !d.is_empty() {
                let parts: Vec<String> = d.iter().map(|(i, v)| format!("{} {}", ctx.fmt.wheel(*i), ctx.fmt.pressure_delta(*v))).collect();
                ctx.push(CallRequest::new("tyre.pressure_next_stint").p("deltas", parts.join(", ")));
            }
            mem.stint_marks = [false; 2];
        }
        RaceEvent::PitExit { .. } => tyre_after_stop(ctx, mem),
        _ => {}
    }
}

/// Tyre values measured in the pit stall (games without live tyre data): most worn tyre and
/// the biggest inside/outside temperature spread.
fn tyre_after_stop(ctx: &mut Ctx, mem: &mut Memory) {
    let Some(c) = ctx.state.frame.tyre_check else { return };
    // Lap 0 = values from sitting in the box before the session: nothing driven yet.
    if c.lap <= 0 || mem.tyre_check_said == Some(c.lap) {
        return;
    }
    mem.tyre_check_said = Some(c.lap);
    let min_wear = |w: &[f32; 3]| w.iter().copied().fold(f32::MAX, f32::min);
    let Some(worn) = (0..4).min_by(|a, b| min_wear(&c.wear[*a]).total_cmp(&min_wear(&c.wear[*b]))) else { return };
    let spread_th = ctx.th("tyre.after_stop", "spread_c") as f32;
    let spread = (0..4)
        .map(|i| (i, c.temp_c[i][0] - c.temp_c[i][2]))
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        .filter(|(_, d)| d.abs() >= spread_th)
        .map(|(i, d)| {
            let key = if d > 0.0 { "tyre.after_stop:spread_inner" } else { "tyre.after_stop:spread_outer" };
            ctx.frag(key, &[("wheel", ctx.fmt.wheel(i).to_string()), ("delta", ctx.fmt.temp(d.abs()))])
        })
        .unwrap_or_default();
    let wear = (min_wear(&c.wear[worn]) * 100.0).round();
    ctx.push(CallRequest::new("tyre.after_stop").p("wheel", ctx.fmt.wheel(worn).to_string()).p("wear", format!("{wear}")).p("spread", spread));
}

/// Engine warning lights (rising edge only).
fn engine(ctx: &mut Ctx, mem: &mut Memory) {
    let e = ctx.state.frame.engine;
    let checks = [(e.water_hot, "water_temp", e.water_temp_c), (e.oil_hot, "oil_temp", e.oil_temp_c), (e.oil_pressure_low, "oil_pressure", 0.0), (e.fuel_pressure_low, "fuel_pressure", 0.0)];
    for (i, (on, sub, temp)) in checks.into_iter().enumerate() {
        if on && !mem.engine_warned[i] {
            ctx.push(CallRequest::new("car.engine").variant(sub).p("temp", ctx.fmt.temp(temp)).key(sub));
        }
        mem.engine_warned[i] = on;
    }
}

/// Repair times reported by the game: announce new damage with the real time it costs.
fn repair(ctx: &mut Ctx, mem: &mut Memory) {
    let Some(r) = ctx.state.frame.repair else { return };
    let (m0, o0) = std::mem::replace(&mut mem.last_repair, (r.mandatory_s, r.optional_s));
    // Times only go up with new damage; they count down while the crew repairs.
    let new_mandatory = r.mandatory_s > m0 + 1.0;
    let new_optional = r.optional_s > o0 + 1.0;
    if !new_mandatory && !new_optional {
        return;
    }
    let secs = |s: f32| format!("{}", s.round() as i32);
    if new_mandatory {
        let mut extra = String::new();
        if r.optional_s > 1.0 {
            extra.push_str(&ctx.frag("damage.repair:optional_part", &[("seconds", secs(r.optional_s))]));
        }
        if let Some(n) = r.fast_repairs_left.filter(|n| *n > 0) {
            extra.push_str(&ctx.frag("damage.repair:fast_repair_part", &[("count", n.to_string())]));
        }
        ctx.push(CallRequest::new("damage.repair").variant("mandatory").p("seconds", secs(r.mandatory_s)).p("optional", extra));
    } else {
        ctx.push(CallRequest::new("damage.repair").variant("optional").p("seconds", secs(r.optional_s)));
    }
    // The pace check after damage weighs the loss against the optional repair.
    if r.optional_s > 1.0 {
        mem.repair_estimate_s = Some(r.optional_s);
        if mem.damage_lap.is_none() {
            mem.damage_lap = Some(ctx.lap());
            mem.pace_before_damage_ms = Some(ctx.state.avg_lap_ms(5)).filter(|p| *p > 0);
        }
    }
}

fn lockup_report(ctx: &mut Ctx, mem: &mut Memory) {
    let per_lap = ctx.th("assist.lockup", "per_lap") as u32;
    let (f, r) = (mem.lockups_front, mem.lockups_rear);
    mem.lockups_front = 0;
    mem.lockups_rear = 0;
    if f.max(r) >= per_lap {
        let front = f >= r;
        let advice = ctx.frag(if front { "assist.lockup:advice_front" } else { "assist.lockup:advice_rear" }, &[]);
        ctx.push(CallRequest::new("assist.lockup").p("axle", ctx.fmt.axle(front).to_string()).p("advice", advice));
    }
}

/// Lock-up detection on every frame: braking, wheel slip ratio strongly negative.
pub fn on_frame(state: &RaceState, mem: &mut Memory) {
    let f = &state.frame;
    if f.brake < 0.3 || f.speed_kmh < 60.0 || f.abs_active {
        mem.locking = false;
        return;
    }
    let front = f.wheels[0].slip_ratio.min(f.wheels[1].slip_ratio) < -0.25;
    let rear = f.wheels[2].slip_ratio.min(f.wheels[3].slip_ratio) < -0.25;
    if (front || rear) && !mem.locking {
        mem.locking = true;
        if front {
            mem.lockups_front += 1;
        } else {
            mem.lockups_rear += 1;
        }
    } else if !front && !rear {
        mem.locking = false;
    }
}

pub fn tick(ctx: &mut Ctx, mem: &mut Memory) {
    repair(ctx, mem);
    let s = ctx.state;
    let f = &s.frame;
    if f.in_pit_lane || f.speed_kmh < 30.0 {
        return;
    }
    engine(ctx, mem);
    let c = class(ctx);
    // Cold tyres after the start or a stop.
    if mem.cold_check_pending && f.speed_kmh > 60.0 {
        mem.cold_check_pending = false;
        let margin = ctx.th("tyre.cold", "margin_c") as f32;
        if f.wheels.iter().any(|w| w.core_temp_c > 1.0 && w.core_temp_c < c.core_temp_cold + margin) {
            ctx.push(CallRequest::new("tyre.cold"));
        }
    }
    let hot_margin = ctx.th("tyre.overheat", "margin_c") as f32;
    if let Some((i, _)) = f.wheels.iter().enumerate().find(|(_, w)| w.core_temp_c > c.core_temp_hot + hot_margin) {
        ctx.push(CallRequest::new("tyre.overheat").p("wheel", ctx.fmt.wheel(i).to_string()).key(i.to_string()));
    }
    let brake_margin = ctx.th("brake.temp", "margin_c") as f32;
    let front_hot = f.wheels[0].brake_temp_c.max(f.wheels[1].brake_temp_c) > c.brake_temp_hot + brake_margin;
    let rear_hot = f.wheels[2].brake_temp_c.max(f.wheels[3].brake_temp_c) > c.brake_temp_hot + brake_margin;
    if front_hot || rear_hot {
        ctx.push(CallRequest::new("brake.temp").p("axle", ctx.fmt.axle(front_hot).to_string()));
    }
    // Damage detection.
    let dmg: f32 = f.damage.iter().sum();
    if dmg > mem.damage_sum + 0.05 {
        let i = (0..5).max_by(|a, b| f.damage[*a].total_cmp(&f.damage[*b])).unwrap_or(0);
        let de = ctx.fmt.lang == crate::storage::settings::Language::De;
        let where_ = match (i, de) {
            (0, true) => "vorne",
            (1, true) => "hinten",
            (2, true) => "links",
            (3, true) => "rechts",
            (_, true) => "in der Mitte",
            (0, false) => "at the front",
            (1, false) => "at the rear",
            (2, false) => "on the left",
            (3, false) => "on the right",
            (_, false) => "in the centre",
        };
        mem.damage_lap = Some(ctx.lap());
        mem.pace_before_damage_ms = Some(s.avg_lap_ms(5)).filter(|p| *p > 0);
        ctx.push(CallRequest::new("damage.report").variant("unknown").p("where", where_.to_string()));
    }
    mem.damage_sum = dmg;
    // Wet switch.
    let w = &f.weather;
    let aggr = ctx.th("tyre.wet_switch", "aggressiveness") as i32;
    let wet_track = matches!(w.grip, TrackGrip::Wet | TrackGrip::Flooded) || (aggr == 0 && w.grip == TrackGrip::Damp);
    let soon = if aggr == 0 { w.rain_30min.max(w.rain_10min) } else { w.rain_10min };
    let soon_threshold = if aggr == 2 { RainIntensity::Medium } else { RainIntensity::Light };
    // Wet tyres not allowed yet (race control has not declared the track wet).
    let wets_banned = f.wet_tyres_allowed == Some(false);
    let state = if wets_banned && !f.rain_tyres {
        None
    } else if !f.rain_tyres && wet_track {
        Some("to_wet_now")
    } else if !f.rain_tyres && soon >= soon_threshold {
        Some("to_wet_soon")
    } else if f.rain_tyres && w.rain_now == RainIntensity::None && w.rain_10min == RainIntensity::None && matches!(w.grip, TrackGrip::Fast | TrackGrip::Optimum | TrackGrip::Green) {
        Some("to_dry")
    } else {
        None
    };
    if state.is_some() && state != mem.wet_state_said {
        mem.wet_state_said = state;
        if let Some(sub) = state {
            let minutes = if aggr == 0 && w.rain_10min < soon_threshold { "30" } else { "10" };
            ctx.push(CallRequest::new("tyre.wet_switch").variant(sub).p("minutes", minutes));
        }
    } else if state.is_none() {
        mem.wet_state_said = None;
    }
    // Lights (time of day) and wipers.
    let hour = (f.clock_s / 3600.0) % 24.0;
    if (hour >= 19.5 || hour < 6.0) && f.lights_stage == 0 && f.clock_s > 0.0 && !mem.lights_said {
        mem.lights_said = true;
        ctx.push(CallRequest::new("car.lights"));
    }
    if w.rain_now >= RainIntensity::Light && f.wiper_level == 0 && !mem.wipers_said {
        mem.wipers_said = true;
        ctx.push(CallRequest::new("car.wipers"));
    }
    if f.wiper_level > 0 {
        mem.wipers_said = false;
    }
    // Stint time.
    if f.stint_time_left_ms > 0 {
        for (i, key) in ["mark1_min", "mark2_min"].iter().enumerate() {
            let m = ctx.th("car.stint_time", key);
            if m > 0.0 && !mem.stint_marks[i] && f64::from(f.stint_time_left_ms) <= m * 60_000.0 {
                mem.stint_marks[i] = true;
                ctx.push(CallRequest::new("car.stint_time").p("minutes", format!("{}", m as i32)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::race::state::LapRecord;

    #[test]
    fn pressure_recommendation() {
        let mut s = RaceState::new();
        for lap in 1..=3 {
            s.laps.push(LapRecord {
                lap,
                time_ms: 100_000,
                sectors: [None; 3],
                valid: true,
                fuel_used_l: None,
                position: 1,
                clean: true,
                in_lap: false,
                out_lap: false,
                avg_pressure: [27.8, 26.5, 26.6, 26.4],
                avg_core_temp: [80.0; 4],
            });
        }
        let d = next_stint_deltas(&s, [26.0, 27.0], 0.2);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].0, 0);
        assert!((d[0].1 + 1.3).abs() < 1e-4);
    }
}
