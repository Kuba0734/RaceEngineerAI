//! Corner feedback, biggest loss, praise (purple sector, PB, consistency), handling,
//! behaviour after mistakes, TC usage per corner.

use super::{Ctx, Memory};
use crate::calls::engine::CallRequest;
use crate::race::lapcompare::{self, CornerIssue, BIN_M};
use crate::race::state::{RaceEvent, RaceState};
use crate::storage::settings::{AfterMistake, Level3};

/// Understeer/oversteer accumulator: yaw response per steering input, entry vs exit.
#[derive(Default)]
pub struct Handling {
    entry_sum: f32,
    entry_n: u32,
    exit_sum: f32,
    exit_n: u32,
    mid_sum: f32,
    mid_n: u32,
    laps_understeer: u32,
    laps_oversteer: u32,
}

pub fn on_frame(state: &RaceState, mem: &mut Memory) {
    let f = &state.frame;
    if f.speed_kmh < 50.0 || f.steer.abs() < 0.05 {
        return;
    }
    // Yaw response normalised by steering and speed (dimensionless enough for a relative comparison).
    let k = (f.yaw_rate / (f.steer * (f.speed_kmh / 100.0))).abs();
    if !k.is_finite() {
        return;
    }
    let h = &mut mem.handling;
    if f.brake > 0.1 {
        h.entry_sum += k;
        h.entry_n += 1;
    } else if f.throttle > 0.7 {
        h.exit_sum += k;
        h.exit_n += 1;
    } else {
        h.mid_sum += k;
        h.mid_n += 1;
    }
}

fn handling_lap(ctx: &mut Ctx, mem: &mut Memory) {
    let h = &mut mem.handling;
    if h.entry_n < 50 || h.exit_n < 50 || h.mid_n < 50 {
        *h = Handling { laps_understeer: h.laps_understeer, laps_oversteer: h.laps_oversteer, ..Default::default() };
        return;
    }
    let (entry, exit, mid) = (h.entry_sum / h.entry_n as f32, h.exit_sum / h.exit_n as f32, h.mid_sum / h.mid_n as f32);
    if entry < mid * 0.75 {
        h.laps_understeer += 1;
    } else {
        h.laps_understeer = 0;
    }
    if exit > mid * 1.35 {
        h.laps_oversteer += 1;
    } else {
        h.laps_oversteer = 0;
    }
    let (u, o) = (h.laps_understeer, h.laps_oversteer);
    *h = Handling { laps_understeer: u, laps_oversteer: o, ..Default::default() };
    // Report only when it shows on two laps in a row.
    if u >= 2 {
        mem.handling.laps_understeer = 0;
        ctx.push(CallRequest::new("setup.handling").sub("understeer_entry"));
    } else if o >= 2 {
        mem.handling.laps_oversteer = 0;
        ctx.push(CallRequest::new("setup.handling").sub("oversteer_exit"));
    }
}

fn praise_allowed(ctx: &Ctx) -> bool {
    match ctx.settings.personality.praise {
        Level3::Off => false,
        Level3::Rare => rand::random_range(0..3) == 0,
        Level3::Often => true,
    }
}

pub fn on_event(ctx: &mut Ctx, mem: &mut Memory, ev: &RaceEvent) {
    let s = ctx.state;
    match ev {
        RaceEvent::LapCompleted(rec) => {
            // Corner feedback: 1–2 biggest losses.
            let max = ctx.th("lap.corner_feedback", "max_points").max(1.0) as usize;
            let min_loss = ctx.th("lap.corner_feedback", "min_loss_s") as f32;
            for d in lapcompare::biggest_losses(&s.last_corner_deltas, max, min_loss) {
                let corner = d.corner.to_string();
                let r = match d.issue {
                    CornerIssue::BrakedEarly => CallRequest::new("lap.corner_feedback").sub("braked_early").p("meters", ctx.fmt.meters(d.brake_point_delta_m.unwrap_or(0.0).abs())),
                    CornerIssue::BrakedLate => CallRequest::new("lap.corner_feedback").sub("braked_late"),
                    CornerIssue::SlowApex => CallRequest::new("lap.corner_feedback").sub("slow_apex").p("kmh", format!("{}", d.apex_speed_delta_kmh.abs().round() as i32)),
                    CornerIssue::LateThrottle => CallRequest::new("lap.corner_feedback").sub("late_throttle").p("meters", ctx.fmt.meters(d.throttle_delta_m.unwrap_or(0.0).abs())),
                    CornerIssue::Unknown => CallRequest::new("lap.corner_feedback").sub("unknown").p("loss", ctx.fmt.tenths(d.time_loss_s)),
                };
                ctx.push(r.p("corner", corner.clone()).key(corner));
            }
            let bl_min = ctx.th("lap.biggest_loss", "min_loss_s") as f32;
            if let Some(d) = lapcompare::biggest_losses(&s.last_corner_deltas, 1, bl_min).first() {
                ctx.push(CallRequest::new("lap.biggest_loss").p("corner", d.corner.to_string()).p("loss", ctx.fmt.tenths(d.time_loss_s)));
            }
            // Consistency.
            let window = ctx.th("lap.consistency", "window_laps").max(3.0) as usize;
            let spread_th = ctx.th("lap.consistency", "spread_s") as f32;
            let last: Vec<i32> = s.laps.iter().rev().filter(|l| l.valid && l.clean).take(window).map(|l| l.time_ms).collect();
            if last.len() == window && praise_allowed(ctx) {
                let (mn, mx) = (last.iter().min().copied().unwrap_or(0), last.iter().max().copied().unwrap_or(0));
                let spread = (mx - mn) as f32 / 1000.0;
                if spread <= spread_th {
                    ctx.push(CallRequest::new("lap.consistency").p("laps", window.to_string()).p("spread", ctx.fmt.tenths(spread.max(0.1))));
                }
            }
            // TC usage per corner on the last lap.
            if let Some(trace) = &s.last_trace {
                let share_th = ctx.th("assist.tc", "share_pct") as f32 / 100.0;
                let samples = trace.filled();
                let worst = s
                    .corners
                    .iter()
                    .map(|c| {
                        let a = (c.start_m / BIN_M) as usize;
                        let b = ((c.end_m / BIN_M) as usize).min(samples.len().saturating_sub(1));
                        let n = (a..=b).count().max(1) as f32;
                        (c.number, (a..=b).filter(|i| samples.get(*i).is_some_and(|x| x.tc)).count() as f32 / n)
                    })
                    .max_by(|x, y| x.1.total_cmp(&y.1));
                if let Some((corner, share)) = worst {
                    if share >= share_th {
                        ctx.push(CallRequest::new("assist.tc").p("corner", corner.to_string()).key(corner.to_string()));
                    }
                }
            }
            handling_lap(ctx, mem);
            let _ = rec;
        }
        RaceEvent::PersonalBest { time_ms } => ctx.push(CallRequest::new("lap.personal_best").p("laptime", ctx.fmt.laptime(*time_ms))),
        RaceEvent::SectorCompleted { sector, time_ms } => {
            // Purple: faster than every car's best of that sector.
            let best_other = s.cars.iter().filter(|c| !c.is_player).filter_map(|c| c.best_splits[*sector]).min();
            let my_best = s.laps.iter().filter_map(|l| l.sectors[*sector]).min();
            let purple = best_other.is_some_and(|b| *time_ms < b) && my_best.is_none_or(|m| *time_ms <= m);
            if purple && praise_allowed(ctx) {
                ctx.push(CallRequest::new("lap.praise_sector").p("sector", (sector + 1).to_string()).p("sector_word", ctx.fmt.ordinal_sector(*sector)));
            }
        }
        RaceEvent::Mistake => {
            let sub = match ctx.settings.personality.after_mistake {
                AfterMistake::Reassure => "reassure",
                AfterMistake::Factual => "factual",
                AfterMistake::Silent => return,
            };
            ctx.push(CallRequest::new("driver.after_mistake").sub(sub));
        }
        _ => {}
    }
}

pub fn tick(_ctx: &mut Ctx, _mem: &mut Memory) {}
