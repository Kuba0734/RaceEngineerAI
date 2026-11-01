//! Fuel & pit: status, critical, save target, pit window, refuel amount, exit position,
//! rivals pitting, undercut, strategy plan, start fuel.

use super::{Ctx, Memory};
use crate::calls::engine::CallRequest;
use crate::race::pit;
use crate::race::state::RaceEvent;
use crate::storage::settings::SessionKind;

pub fn on_event(ctx: &mut Ctx, mem: &mut Memory, ev: &RaceEvent) {
    let s = ctx.state;
    match ev {
        RaceEvent::LapCompleted(_) => {
            let fs = s.fuel_strategy(ctx.settings);
            if fs.per_lap_l <= 0.0 {
                return;
            }
            let race = ctx.kind == SessionKind::Race;
            if race {
                let sub = if fs.enough { "enough" } else { "short" };
                ctx.push(CallRequest::new("fuel.status").sub(sub).p("laps_possible", ctx.fmt.laps(fs.laps_possible)).p("laps_needed", ctx.fmt.laps(fs.laps_to_go.ceil())));
                if let (Some(save), false) = (fs.save_per_lap_l, mem.save_said) {
                    mem.save_said = true;
                    let lift = lift_corners(ctx);
                    let lift_part = if lift.is_empty() { String::new() } else { ctx.frag("fuel.lift_part", &[("corners", lift)]) };
                    ctx.push(CallRequest::new("fuel.save_target").p("save", ctx.fmt.fuel_per_lap(save)).p("lift_part", lift_part));
                }
            }
            // Strategy plan.
            if let Some(plan) = ctx.engine.crew.plan_pit_lap {
                let before = ctx.th("pit.plan", "remind_laps_before") as i32;
                let next_lap = ctx.lap() + 1;
                if before > 0 && plan - next_lap == before - 1 && mem.plan_reminded_for != Some(plan) && plan > next_lap {
                    mem.plan_reminded_for = Some(plan);
                    ctx.push(CallRequest::new("pit.plan").sub("reminder").p("lap", plan.to_string()));
                }
                if next_lap == plan && mem.plan_now_for != Some(plan) {
                    mem.plan_now_for = Some(plan);
                    ctx.push(CallRequest::new("pit.plan").sub("now").p("lap", plan.to_string()));
                }
            }
            // Start fuel for the next race after a few clean laps in practice/quali.
            if !race && !mem.start_fuel_said && fs.from_clean_laps >= 3 {
                mem.start_fuel_said = true;
                let st = &ctx.settings.strategy;
                let f = crate::race::fuel::start_fuel(st.next_race_minutes, fs.per_lap_l, fs.avg_lap_ms, st.formation_lap, st.fuel_reserve_laps, st.fuel_reserve_liters, s.session.max_fuel_l);
                ctx.push(CallRequest::new("prerace.start_fuel").p("minutes", format!("{}", st.next_race_minutes as i32)).p("fuel", ctx.fmt.fuel(f)));
            }
        }
        RaceEvent::PitEntry if ctx.kind == SessionKind::Race => {
            let fs = s.fuel_strategy(ctx.settings);
            if fs.refuel_l > 0.5 {
                ctx.push(CallRequest::new("pit.fuel_amount").p("fuel", ctx.fmt.fuel(fs.refuel_l)));
            }
            let p = pit::predict(s, ctx.settings, 0, 0.0);
            let behind_part = p.exit_behind_car.and_then(|id| s.car(id)).map(|c| ctx.frag("pit.behind_part", &[("car", ctx.fmt.car(c))])).unwrap_or_default();
            ctx.push(CallRequest::new("pit.exit_position").p("position", p.exit_position.to_string()).p("behind_part", behind_part));
            stint_summary(ctx, mem);
        }
        RaceEvent::PitExit { .. } => {
            mem.stint_laps_start = s.laps.len();
            mem.cold_check_pending = true;
        }
        RaceEvent::RivalPitEntry { car_id } if ctx.kind == SessionKind::Race => {
            let Some(car) = s.car(*car_id) else { return };
            let pos = s.player_position();
            let n = ctx.th("pit.rival_pitting", "positions") as i32;
            let name = ctx.fmt.car(car);
            if pit::is_undercut_threat(s, ctx.settings, *car_id) && car.position > pos {
                ctx.push(CallRequest::new("pit.undercut").p("car", name).key(car_id.to_string()));
            } else if (car.position - pos).abs() <= n {
                ctx.push(CallRequest::new("pit.rival_pitting").p("car", name).key(car_id.to_string()));
            }
        }
        _ => {}
    }
}

/// Corners with the highest entry speed are the best lift-and-coast spots.
fn lift_corners(ctx: &Ctx) -> String {
    let s = ctx.state;
    let Some(r) = &s.reference else { return String::new() };
    let samples = r.filled();
    let mut v: Vec<(u32, f32)> = s
        .corners
        .iter()
        .map(|c| {
            let i = ((c.start_m / crate::race::lapcompare::BIN_M) as usize).min(samples.len().saturating_sub(1));
            (c.number, samples.get(i).map(|x| x.speed_kmh).unwrap_or(0.0))
        })
        .collect();
    v.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut n: Vec<u32> = v.iter().take(2).map(|x| x.0).collect();
    n.sort();
    let and = if ctx.fmt.lang == crate::storage::settings::Language::De { " und " } else { " and " };
    n.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(and)
}

fn stint_summary(ctx: &mut Ctx, mem: &mut Memory) {
    let s = ctx.state;
    let laps: Vec<_> = s.laps.iter().skip(mem.stint_laps_start).filter(|l| l.clean && l.valid).collect();
    if laps.len() < 3 {
        return;
    }
    let avg = laps.iter().map(|l| l.time_ms as i64).sum::<i64>() / laps.len() as i64;
    let fuel: Vec<f32> = laps.iter().filter_map(|l| l.fuel_used_l).collect();
    let fuel_avg = if fuel.is_empty() { 0.0 } else { fuel.iter().sum::<f32>() / fuel.len() as f32 };
    // Degradation: last third vs first third of the stint.
    let third = (laps.len() / 3).max(1);
    let first = laps.iter().take(third).map(|l| l.time_ms as f32).sum::<f32>() / third as f32;
    let last = laps.iter().rev().take(third).map(|l| l.time_ms as f32).sum::<f32>() / third as f32;
    let deg = if last - first > 500.0 { ctx.frag("stint.deg_high", &[]) } else { ctx.frag("stint.deg_low", &[]) };
    ctx.push(CallRequest::new("stint.summary").p("avg", ctx.fmt.laptime(avg as i32)).p("fuel", ctx.fmt.fuel_per_lap(fuel_avg)).p("deg_part", deg));
}

pub fn tick(ctx: &mut Ctx, mem: &mut Memory) {
    let s = ctx.state;
    let f = &s.frame;
    if ctx.kind != SessionKind::Race || f.in_pit_lane {
        return;
    }
    let fs = s.fuel_strategy(ctx.settings);
    let crit = ctx.th("fuel.critical", "laps") as f32;
    if fs.per_lap_l > 0.0 && !fs.enough && fs.laps_possible < crit {
        ctx.push(CallRequest::new("fuel.critical").p("laps", ctx.fmt.laps(fs.laps_possible)));
    }
    // Pit window (session time based, ms elapsed).
    let (start, end) = (s.session.pit_window_start_ms, s.session.pit_window_end_ms);
    if let Some(total) = mem.session_duration_ms {
        if end > start && end > 0 {
            let elapsed = total - f.session_time_left_ms;
            let (st, en) = (f64::from(start), f64::from(end));
            if elapsed >= st && elapsed < en && !mem.pit_window_open_said {
                mem.pit_window_open_said = true;
                ctx.push(CallRequest::new("pit.window").sub("open"));
            }
            if f.missing_mandatory_pits > 0 && !f.mandatory_pit_done && elapsed >= en - 300_000.0 && elapsed < en && !mem.mandatory_warned {
                mem.mandatory_warned = true;
                ctx.push(CallRequest::new("pit.window").sub("closing"));
            }
        }
    }
}
