//! Flags & race control: blue, yellow, FCY, green, white, chequered, track limits, penalties.

use super::{Ctx, Memory};
use crate::calls::engine::CallRequest;
use crate::games::provider::{Flag, PenaltyKind, PenaltyReason};
use crate::race::state::RaceEvent;
use crate::storage::settings::Language;

fn penalty_text(ctx: &Ctx, kind: PenaltyKind, reason: PenaltyReason) -> String {
    let de = ctx.fmt.lang == Language::De;
    let k = match (kind, de) {
        (PenaltyKind::DriveThrough, true) => "Drive-Through",
        (PenaltyKind::DriveThrough, false) => "drive-through",
        (PenaltyKind::StopAndGo10, true) => "Stop-and-Go 10 Sekunden",
        (PenaltyKind::StopAndGo10, false) => "ten second stop-and-go",
        (PenaltyKind::StopAndGo20, true) => "Stop-and-Go 20 Sekunden",
        (PenaltyKind::StopAndGo20, false) => "twenty second stop-and-go",
        (PenaltyKind::StopAndGo30, true) => "Stop-and-Go 30 Sekunden",
        (PenaltyKind::StopAndGo30, false) => "thirty second stop-and-go",
        (PenaltyKind::RemoveBestLap, true) => "Bestzeit gestrichen",
        (PenaltyKind::RemoveBestLap, false) => "best lap removed",
        (PenaltyKind::PostRaceTime, true) => "Zeitstrafe nach dem Rennen",
        (PenaltyKind::PostRaceTime, false) => "post-race time penalty",
        (PenaltyKind::Disqualified, true) => "Disqualifikation",
        (PenaltyKind::Disqualified, false) => "disqualification",
        (PenaltyKind::Other, true) => "Strafe",
        (PenaltyKind::Other, false) => "penalty",
    };
    let r = match (reason, de) {
        (PenaltyReason::Cutting, true) => " wegen Abkürzen",
        (PenaltyReason::Cutting, false) => " for cutting",
        (PenaltyReason::PitSpeeding, true) => " wegen Speeding in der Boxengasse",
        (PenaltyReason::PitSpeeding, false) => " for pit lane speeding",
        (PenaltyReason::IgnoredMandatoryPit, true) => " wegen Pflichtstopp",
        (PenaltyReason::IgnoredMandatoryPit, false) => " for the mandatory stop",
        (PenaltyReason::DriverStint, true) => " wegen Stintzeit",
        (PenaltyReason::DriverStint, false) => " for driver stint time",
        (PenaltyReason::Other, _) => "",
    };
    format!("{k}{r}")
}

fn needs_serving(kind: PenaltyKind) -> bool {
    matches!(kind, PenaltyKind::DriveThrough | PenaltyKind::StopAndGo10 | PenaltyKind::StopAndGo20 | PenaltyKind::StopAndGo30)
}

pub fn on_event(ctx: &mut Ctx, mem: &mut Memory, ev: &RaceEvent) {
    match ev {
        RaceEvent::FlagChanged { to, .. } => match to {
            Flag::Green => ctx.push(CallRequest::new("flag.green")),
            Flag::White => ctx.push(CallRequest::new("flag.white")),
            Flag::Checkered => ctx.push(CallRequest::new("flag.checkered").p("position", ctx.state.player_position().to_string())),
            Flag::Blue => blue(ctx),
            _ => {}
        },
        RaceEvent::Game(crate::games::provider::GameEvent::GreenFlag) => ctx.push(CallRequest::new("flag.green")),
        RaceEvent::YellowSectorsChanged(y) => {
            let all = y.iter().all(|x| *x);
            if all && !mem.fcy_active {
                mem.fcy_active = true;
                let advice = if ctx.state.frame.missing_mandatory_pits > 0 { ctx.frag("flag.fcy:advice_pit", &[]) } else { String::new() };
                ctx.push(CallRequest::new("flag.fcy").p("advice", advice));
                return;
            }
            if !all {
                mem.fcy_active = false;
            }
            yellow(ctx, y);
        }
        RaceEvent::TrackLimit { count } => {
            let warn_from = ctx.th("penalty.track_limits", "warn_from") as u32;
            if *count >= warn_from {
                // The game's penalty limit is not exposed; the user can set it (0 = unknown).
                let penalty_at = ctx.th("penalty.track_limits", "penalty_at") as u32;
                let sub = if penalty_at > 0 && *count + 1 == penalty_at { Some("last_warning") } else { None };
                let mut r = CallRequest::new("penalty.track_limits").p("count", count.to_string());
                if let Some(s) = sub {
                    r = r.variant(s);
                }
                ctx.push(r);
            }
        }
        RaceEvent::PenaltyReceived(p) => {
            mem.penalty_lap = Some(ctx.lap());
            let text = penalty_text(ctx, p.kind, p.reason);
            let de = ctx.fmt.lang == Language::De;
            let advice = if needs_serving(p.kind) { if de { " Innerhalb von 3 Runden absitzen.".to_string() } else { " Serve it within 3 laps.".to_string() } } else { String::new() };
            ctx.push(CallRequest::new("penalty.received").p("penalty", text).p("advice", advice));
        }
        RaceEvent::PenaltyCleared => mem.penalty_lap = None,
        RaceEvent::LapCompleted(_) => {
            if let (Some(p), Some(since)) = (ctx.state.frame.penalty, mem.penalty_lap) {
                let after = ctx.th("penalty.reminder", "remind_after_laps") as i32;
                if needs_serving(p.kind) && ctx.lap() - since >= after {
                    let text = penalty_text(ctx, p.kind, p.reason);
                    ctx.push(CallRequest::new("penalty.reminder").p("penalty", text));
                }
            }
        }
        _ => {}
    }
}

fn blue(ctx: &mut Ctx) {
    let s = ctx.state;
    let me = s.frame.player_car_id;
    let my_laps = s.player_car().map(|c| c.laps).unwrap_or(s.frame.completed_laps);
    // The lapping car: behind on track, at least a lap ahead in the race.
    let lapping = s
        .cars
        .iter()
        .filter(|c| !c.is_player && c.laps > my_laps)
        .filter_map(|c| s.gaps.gap(me, c.id).map(|g| (c, g)))
        .filter(|(_, g)| *g < 0.0)
        .max_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((car, gap)) = lapping {
        let name = ctx.fmt.car(car);
        let gap_txt = ctx.fmt.gap(gap as f32);
        ctx.push(CallRequest::new("flag.blue_leader").p("car", name).p("gap", gap_txt).key(car.id.to_string()));
    }
}

fn yellow(ctx: &mut Ctx, y: &[bool; 3]) {
    let s = ctx.state;
    let mine = s.frame.sector_index.clamp(0, 2) as usize;
    // Only yellows ahead: current or next sector.
    for k in [mine, (mine + 1) % 3] {
        if !y[k] {
            continue;
        }
        // Stopped cars on track in that sector.
        let stopped = s.cars.iter().filter(|c| !c.is_player && c.speed_kmh < 20.0 && !c.location.in_pits()).find(|c| ((c.spline * 3.0) as usize).min(2) == k);
        let extra = match stopped {
            Some(c) => {
                let m = c.spline * s.track_m();
                let corner = s.corners.iter().rev().find(|x| x.apex_m <= m).map(|x| x.number);
                let de = ctx.fmt.lang == Language::De;
                match (corner, de) {
                    (Some(n), true) => format!(", Auto steht nach Kurve {n}"),
                    (Some(n), false) => format!(", car stopped after turn {n}"),
                    (None, true) => ", Auto steht auf der Strecke".into(),
                    (None, false) => ", car stopped on track".into(),
                }
            }
            None => String::new(),
        };
        ctx.push(CallRequest::new("flag.yellow_ahead").p("sector", (k + 1).to_string()).p("extra", extra).key(k.to_string()));
        return;
    }
}

/// Incident points: every new incident from `report_from`, always when the limit is close.
fn incidents(ctx: &mut Ctx, mem: &mut Memory) {
    let Some(n) = ctx.state.frame.incidents else { return };
    let Some(prev) = mem.last_incidents.replace(n) else { return };
    if n <= prev {
        return;
    }
    let report_from = ctx.th("penalty.incidents", "report_from") as i32;
    let warn_left = ctx.th("penalty.incidents", "warn_left") as i32;
    match ctx.state.session.incident_limit {
        Some(limit) => {
            let left = (limit - n).max(0);
            if n >= report_from || left <= warn_left {
                ctx.push(CallRequest::new("penalty.incidents").variant("limit").p("count", n.to_string()).p("limit", limit.to_string()).p("left", left.to_string()));
            }
        }
        None if n >= report_from => ctx.push(CallRequest::new("penalty.incidents").p("count", n.to_string())),
        None => {}
    }
}

/// Safety car situations: restart next lap, free pass, wave around, end of line.
fn pacing(ctx: &mut Ctx, mem: &mut Memory) {
    let p = ctx.state.frame.pacing;
    let old = std::mem::replace(&mut mem.last_pacing, p);
    for (now, before, sub) in [
        (p.green_next_lap, old.green_next_lap, "green_next_lap"),
        (p.free_pass, old.free_pass, "free_pass"),
        (p.waved_around, old.waved_around, "waved_around"),
        (p.end_of_line, old.end_of_line, "end_of_line"),
    ] {
        if now && !before {
            ctx.push(CallRequest::new("flag.pace").variant(sub).key(sub));
        }
    }
}

pub fn tick(ctx: &mut Ctx, mem: &mut Memory) {
    incidents(ctx, mem);
    pacing(ctx, mem);
    // Early blue flag warning: lapping car within the threshold before the official flag.
    if ctx.state.frame.flag != Flag::Blue && ctx.on("flag.blue_leader") {
        let s = ctx.state;
        let me = s.frame.player_car_id;
        let my_laps = s.player_car().map(|c| c.laps).unwrap_or(s.frame.completed_laps);
        let th = ctx.th("flag.blue_leader", "seconds") as f32;
        let near = s.cars.iter().filter(|c| !c.is_player && c.laps > my_laps).any(|c| s.gaps.gap(me, c.id).is_some_and(|g| g < 0.0 && (g as f32) > -th));
        if near {
            blue(ctx);
        }
    }
}
