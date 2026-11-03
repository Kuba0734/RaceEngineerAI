//! Race time, gaps, rival trends, catch prediction, sector strengths, rival info.

use super::{Ctx, Memory};
use crate::calls::engine::CallRequest;
use crate::games::provider::{CarState, StartLight};
use crate::race::state::RaceEvent;
use crate::storage::settings::SessionKind;

fn trend_part(ctx: &Ctx, ahead: bool, trend: Option<f32>) -> String {
    // trend = change of (other − me) gap per lap. Ahead: negative → I gain. Behind: positive → he closes.
    let Some(t) = trend.filter(|t| t.abs() >= 0.05) else { return String::new() };
    let tenths = ctx.fmt.tenths(t);
    let key = match (ahead, t < 0.0) {
        (true, true) => "gap.trend_part:gaining",
        (true, false) => "gap.trend_part:losing",
        (false, false) => "gap.trend_part:closing",
        (false, true) => "gap.trend_part:dropping",
    };
    ctx.frag(key, &[("tenths", tenths)])
}

fn sector_word(ctx: &Ctx, k: usize) -> String {
    ctx.fmt.ordinal_sector(k)
}

/// Start lights: "ready" when they come on, the call on green. "Set" stays silent so the
/// "go" is not delayed by a call that is still playing.
fn start_lights(ctx: &mut Ctx, light: StartLight) {
    let sub = match light {
        StartLight::Ready => "ready",
        StartLight::Go => "go",
        StartLight::Set | StartLight::Off => return,
    };
    ctx.push(CallRequest::new("race.start_lights").variant(sub));
}

/// Multi-class traffic around the player, from the relative (time gap on track) and the class
/// lap time estimates: a faster class closing from behind, a slower class just ahead.
pub fn traffic(ctx: &mut Ctx) {
    let s = ctx.state;
    let Some(me) = s.player_car() else { return };
    let Some(my_lap) = me.class_est_lap_s else { return };
    if s.frame.in_pit_lane {
        return;
    }
    let behind_s = ctx.th("traffic.faster_class", "behind_s") as f32;
    let faster_pct = ctx.th("traffic.faster_class", "speed_diff_pct") as f32 / 100.0;
    let ahead_s = ctx.th("traffic.slower_ahead", "ahead_s") as f32;
    let slower_pct = ctx.th("traffic.slower_ahead", "speed_diff_pct") as f32 / 100.0;
    for r in s.relative(4) {
        if r.is_player || r.in_pits {
            continue;
        }
        let Some(c) = s.car(r.id) else { continue };
        let Some(lap) = c.class_est_lap_s else { continue };
        if c.class_id == me.class_id {
            continue;
        }
        let class = if c.class_name.is_empty() { c.car_brand.clone() } else { c.class_name.clone() };
        let gap = ctx.fmt.num(r.gap_s.abs(), 1);
        if r.gap_s < 0.0 && -r.gap_s <= behind_s && lap < my_lap * (1.0 - faster_pct) {
            ctx.push(CallRequest::new("traffic.faster_class").p("class", class).p("gap", gap).key(r.id.to_string()));
        } else if r.gap_s > 0.0 && r.gap_s <= ahead_s && lap > my_lap * (1.0 + slower_pct) {
            ctx.push(CallRequest::new("traffic.slower_ahead").p("class", class).p("gap", gap).key(r.id.to_string()));
        }
    }
}

pub fn on_event(ctx: &mut Ctx, mem: &mut Memory, ev: &RaceEvent) {
    if let RaceEvent::StartLight(l) = ev {
        start_lights(ctx, *l);
        return;
    }
    let RaceEvent::LapCompleted(_) = ev else { return };
    let s = ctx.state;
    let me = s.frame.player_car_id;
    let window = ctx.settings.strategy.clean_laps_window as usize;

    // Gap ahead / behind, only when changed enough since last time.
    for (ahead, id) in [(true, "gap.ahead"), (false, "gap.behind")] {
        let Some(car) = s.neighbour(if ahead { -1 } else { 1 }) else { continue };
        let Some(g) = s.gaps.gap(me, car.id).map(|g| g as f32) else { continue };
        let last = if ahead { mem.last_gap_ahead } else { mem.last_gap_behind };
        let min_change = ctx.th(id, "min_change_s") as f32;
        if last.is_some_and(|l| (l - g.abs()).abs() < min_change) {
            continue;
        }
        if ctx.on(id) {
            let tp = trend_part(ctx, ahead, s.gaps.trend(car.id).map(|t| t as f32));
            ctx.push(CallRequest::new(if ahead { "gap.ahead" } else { "gap.behind" }).p("gap", ctx.fmt.gap(g)).p("trend_part", tp));
            if ahead {
                mem.last_gap_ahead = Some(g.abs());
            } else {
                mem.last_gap_behind = Some(g.abs());
            }
        }
    }

    if ctx.kind != SessionKind::Race {
        return;
    }
    // Rivals closing / dropping.
    let around = ctx.th("gap.trend_rival", "cars_around") as i32;
    let th = ctx.th("gap.trend_rival", "tenths_per_lap") as f32 / 10.0;
    let pos = s.player_position();
    for c in s.cars.iter().filter(|c| !c.is_player && (c.position - pos).abs() <= around) {
        let Some(tr) = s.gaps.trend(c.id).map(|t| t as f32) else { continue };
        if tr.abs() < th {
            continue;
        }
        let ahead = c.position < pos;
        let sub = match (ahead, tr < 0.0) {
            (true, true) => "catching",
            (true, false) => "pulling_away",
            (false, false) => "closing",
            (false, true) => "dropping",
        };
        let name = ctx.fmt.car(c);
        ctx.push(CallRequest::new("gap.trend_rival").variant(sub).p("car", name).p("tenths", ctx.fmt.tenths(tr)).key(c.id.to_string()));
    }

    // Catch prediction for the car ahead.
    if let Some(car) = s.neighbour(-1) {
        if let (Some(g), Some(tr)) = (s.gaps.gap(me, car.id), s.gaps.trend(car.id)) {
            if tr < -0.05 && g > 0.0 {
                let laps = (g / -tr).ceil() as f32;
                let left = s.fuel_strategy(ctx.settings).laps_to_go;
                let min_left = ctx.th("gap.catch_prediction", "min_laps_left") as f32;
                if laps < left && left >= min_left {
                    let pace = ctx.fmt.laptime(s.avg_lap_ms(window));
                    ctx.push(CallRequest::new("gap.catch_prediction").p("laps", format!("{}", laps as i32)).p("pace", pace).p("car", ctx.fmt.car(car)).key(car.id.to_string()));
                }
            }
        }
    }

    sector_hints(ctx);

    // Rival info when the car directly ahead changes.
    if let Some(car) = s.neighbour(-1) {
        if mem.last_ahead_id != Some(car.id) {
            mem.last_ahead_id = Some(car.id);
            rival_info(ctx, car);
        }
    }
}

pub fn rival_info(ctx: &mut Ctx, car: &CarState) {
    let s = ctx.state;
    let r = s.rivals.get(&car.id);
    let stops = r.map(|r| r.pit_stops).unwrap_or(0);
    let age = r.and_then(|r| r.last_pit_lap).map(|l| car.laps - l).unwrap_or(car.laps);
    ctx.push(CallRequest::new("rival.info").p("car", ctx.fmt.car(car)).p("stops", stops.to_string()).p("tyre_age", age.to_string()).key(car.id.to_string()));
}

/// Per-sector comparison with the cars directly ahead and behind (last laps).
fn sector_hints(ctx: &mut Ctx) {
    let s = ctx.state;
    let Some(mine) = s.laps.last().map(|l| l.sectors) else { return };
    let th_def = ctx.th("rival.sector_strength", "tenths") as f32 / 10.0;
    let th_att = ctx.th("rival.attack_hint", "tenths") as f32 / 10.0;
    if let Some(behind) = s.neighbour(1) {
        if let Some((k, d)) = biggest_sector_diff(&mine, &behind.last_splits) {
            // d > 0: behind is faster by d seconds.
            if d >= th_def {
                let name = ctx.fmt.car(behind);
                ctx.push(CallRequest::new("rival.sector_strength").p("car", name).p("sector", (k + 1).to_string()).p("sector_word", sector_word(ctx, k)).key(behind.id.to_string()));
            }
        }
    }
    if let Some(ahead) = s.neighbour(-1) {
        if let Some((k, d)) = biggest_sector_diff(&ahead.last_splits, &mine) {
            // d > 0: I'm faster than the car ahead in sector k.
            if d >= th_att {
                let name = ctx.fmt.car(ahead);
                ctx.push(CallRequest::new("rival.attack_hint").p("car", name).p("sector", (k + 1).to_string()).p("sector_word", sector_word(ctx, k)).key(ahead.id.to_string()));
            }
        }
    }
}

/// Sector where `b` is fastest relative to `a`: (sector, a − b in seconds).
fn biggest_sector_diff(a: &[Option<i32>; 3], b: &[Option<i32>; 3]) -> Option<(usize, f32)> {
    (0..3).filter_map(|k| Some((k, (a[k]? - b[k]?) as f32 / 1000.0))).max_by(|x, y| x.1.total_cmp(&y.1))
}

pub fn tick(ctx: &mut Ctx, mem: &mut Memory) {
    if !ctx.state.race_running() {
        return;
    }
    let left = ctx.state.frame.session_time_left_ms;
    if left <= 0.0 {
        return;
    }
    for (i, key) in ["mark1_min", "mark2_min", "mark3_min"].iter().enumerate() {
        let m = ctx.th("race.time_left", key);
        if m <= 0.0 || mem.time_marks[i] {
            continue;
        }
        let mark_ms = m * 60_000.0;
        // A mark as long as the race itself is no news ("10 minutes left" in a 10 minute race).
        if mem.session_duration_ms.is_some_and(|total| mark_ms >= total - 30_000.0) {
            mem.time_marks[i] = true;
            continue;
        }
        // Fire once when crossing the mark (only if we were above it at some point).
        if left <= mark_ms && left > mark_ms - 30_000.0 {
            mem.time_marks[i] = true;
            let laps = ctx.state.fuel_strategy(ctx.settings).laps_to_go;
            // Without a known pace there is no lap count to report.
            if laps <= 0.0 {
                continue;
            }
            ctx.push(CallRequest::new("race.time_left").p("minutes", format!("{}", m as i32)).p("laps", ctx.fmt.laps(laps)));
        } else if left < mark_ms - 30_000.0 {
            mem.time_marks[i] = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::biggest_sector_diff;

    #[test]
    fn sector_diff() {
        let a = [Some(30_000), Some(40_500), Some(38_000)];
        let b = [Some(30_100), Some(40_100), Some(38_050)];
        let (k, d) = biggest_sector_diff(&a, &b).unwrap();
        assert_eq!(k, 1);
        assert!((d - 0.4).abs() < 1e-4);
    }
}
