//! Qualifying & practice help: position to pole, traffic gap, push now, time for another lap.

use super::{Ctx, Memory};
use crate::calls::engine::CallRequest;
use crate::race::state::RaceEvent;
use crate::storage::settings::SessionKind;

pub fn on_event(ctx: &mut Ctx, mem: &mut Memory, ev: &RaceEvent) {
    if ctx.kind != SessionKind::Qualifying {
        return;
    }
    match ev {
        RaceEvent::PitExit { .. } => mem.quali_out_lap = true,
        RaceEvent::LapCompleted(_) => mem.quali_out_lap = false,
        _ => {}
    }
    if let RaceEvent::LapCompleted(rec) = ev {
        if !rec.valid {
            return;
        }
        let s = ctx.state;
        let pos = s.player_position();
        let pole = s.cars.iter().filter(|c| !c.is_player).filter_map(|c| c.best_lap_ms).min();
        let mine = s.best_valid_lap().map(|l| l.time_ms);
        let gap_part = match (pole, mine) {
            (Some(p), Some(m)) if m > p && pos > 1 => ctx.frag("quali.gap_part", &[("gap", ctx.fmt.gap((m - p) as f32 / 1000.0))]),
            _ => String::new(),
        };
        ctx.push(CallRequest::new("quali.position").p("position", pos.to_string()).p("gap_part", gap_part));
    }
}

pub fn tick(ctx: &mut Ctx, mem: &mut Memory) {
    if ctx.kind != SessionKind::Qualifying {
        return;
    }
    let s = ctx.state;
    let f = &s.frame;
    let lap = ctx.lap();
    // The out-lap ends at the line. Watched on the lap distance itself: iRacing does not count
    // the first crossing out of the pits as a lap (-1 → 0), so the flying lap would still look
    // like an out-lap and get "push now" near its end.
    if mem.quali_last_spline > 0.9 && f.spline < 0.1 {
        mem.quali_out_lap = false;
    }
    mem.quali_last_spline = f.spline;
    let out_lap = mem.quali_out_lap;
    let desired = ctx.th("quali.traffic_gap", "desired_gap_s") as f32;
    let me = f.player_car_id;
    // Nearest car ahead on track (by gap, any position).
    let ahead_gap = s.cars.iter().filter(|c| !c.is_player && !c.location.in_pits()).filter_map(|c| s.gaps.gap(me, c.id)).filter(|g| *g > 0.0).fold(f64::MAX, f64::min);
    if out_lap && !f.in_pit_lane {
        if ahead_gap < f64::from(desired) && mem.traffic_said_lap != Some(lap) {
            mem.traffic_said_lap = Some(lap);
            ctx.push(CallRequest::new("quali.traffic_gap").p("gap", format!("{}", desired as i32)));
        }
        let cold = crate::games::cars::class_def(&s.session.car_class).core_temp_cold;
        // Without live tyre temperatures (iRacing) the end of the out-lap is the cue.
        let warm = !s.has("hasTyreCoreTemps") || f.wheels.iter().all(|w| w.core_temp_c > cold + 5.0);
        if f.spline > 0.85 && warm && ahead_gap >= f64::from(desired) && mem.push_said_lap != Some(lap) {
            mem.push_said_lap = Some(lap);
            ctx.push(CallRequest::new("quali.push_now"));
        }
    }
    // Time for another lap.
    let avg = s.avg_lap_ms(3);
    let left = f.session_time_left_ms;
    if avg > 0 && left > 0.0 && !mem.quali_time_said && left < f64::from(avg) * 2.2 && f.spline < 0.1 {
        mem.quali_time_said = true;
        let laps = (left / f64::from(avg)).floor() as i32;
        let sub = match laps {
            0 => "none",
            1 => "one",
            _ => "two",
        };
        ctx.push(CallRequest::new("quali.time_for_lap").variant(sub).p("minutes", ctx.fmt.minutes(left)));
    }
}
