//! Session start (radio check, greeting, spotter intro), briefing, formation, summaries.

use super::{Ctx, Memory};
use crate::calls::engine::CallRequest;
use crate::games::provider::{RainIntensity, SessionPhase, SessionType, SimStatus};
use crate::race::state::RaceEvent;
use crate::storage::settings::{GreetingScope, Language, RadioCheckWhen, SessionKind};

fn session_name(ctx: &Ctx) -> String {
    let de = ctx.fmt.lang == Language::De;
    match (ctx.state.session.session_type, de) {
        (SessionType::Race, true) => "Rennen".into(),
        (SessionType::Race, false) => "Race".into(),
        (SessionType::Qualifying | SessionType::Superpole, true) => "Qualifying".into(),
        (SessionType::Qualifying | SessionType::Superpole, false) => "Qualifying".into(),
        (SessionType::Hotlap | SessionType::Hotstint, _) => "Hotlap".into(),
        (_, true) => "Training".into(),
        (_, false) => "Practice".into(),
    }
}

pub fn rain_word(ctx: &Ctx, r: RainIntensity) -> String {
    let de = ctx.fmt.lang == Language::De;
    match (r, de) {
        (RainIntensity::None, true) => "kein Regen".into(),
        (RainIntensity::None, false) => "no rain".into(),
        (RainIntensity::Drizzle, true) => "Nieselregen".into(),
        (RainIntensity::Drizzle, false) => "Drizzle".into(),
        (RainIntensity::Light, true) => "Leichter Regen".into(),
        (RainIntensity::Light, false) => "Light rain".into(),
        (RainIntensity::Medium, true) => "Mittlerer Regen".into(),
        (RainIntensity::Medium, false) => "Medium rain".into(),
        (RainIntensity::Heavy, true) => "Starker Regen".into(),
        (RainIntensity::Heavy, false) => "Heavy rain".into(),
        (RainIntensity::Thunderstorm, true) => "Gewitter".into(),
        (RainIntensity::Thunderstorm, false) => "Thunderstorm".into(),
    }
}

fn weather_part(ctx: &Ctx) -> String {
    let w = &ctx.state.frame.weather;
    let de = ctx.fmt.lang == Language::De;
    let now = if w.rain_now == RainIntensity::None {
        if de { "Trocken".to_string() } else { "Dry".to_string() }
    } else {
        rain_word(ctx, w.rain_now)
    };
    let fc = if w.rain_30min > w.rain_now {
        if de { format!(" In 30 Minuten {}.", rain_word(ctx, w.rain_30min)) } else { format!(" {} expected in 30 minutes.", rain_word(ctx, w.rain_30min)) }
    } else {
        String::new()
    };
    // The track temperature is 0 until the game delivers it: rather no number than a wrong one.
    let road = (w.road_temp_c > 0.0).then(|| ctx.fmt.temp(w.road_temp_c));
    match (road, de) {
        (Some(road), true) => format!(" {now}, Strecke {road}.{fc}"),
        (Some(road), false) => format!(" {now}, track {road}.{fc}"),
        (None, _) => format!(" {now}.{fc}"),
    }
}

pub fn on_event(ctx: &mut Ctx, mem: &mut Memory, ev: &RaceEvent) {
    match ev {
        RaceEvent::NewSession => {
            let first = mem.first_session_of_day_done;
            *mem = Memory::new();
            mem.first_session_of_day_done = first;
        }
        RaceEvent::SessionPhaseChanged(SessionPhase::Formation) if !mem.formation_done => {
            mem.formation_done = true;
            ctx.push(CallRequest::new("prerace.formation"));
        }
        RaceEvent::FlagChanged { to: crate::games::provider::Flag::Checkered, .. } | RaceEvent::SessionPhaseChanged(SessionPhase::SessionOver) if ctx.kind == SessionKind::Race && !mem.summary_done => {
            mem.summary_done = true;
            race_summary(ctx);
        }
        _ => {}
    }
}

pub fn race_summary(ctx: &mut Ctx) {
    let s = ctx.state;
    let pos = s.player_position();
    let gained = s.race_start_position.map(|start| start - pos).unwrap_or(0);
    let gained_part = match gained {
        g if g > 0 => ctx.frag("race.gained_part:gained", &[("n", g.to_string())]),
        g if g < 0 => ctx.frag("race.gained_part:lost", &[("n", (-g).to_string())]),
        _ => String::new(),
    };
    let best = s.best_valid_lap().map(|l| ctx.fmt.laptime(l.time_ms)).unwrap_or_else(|| "-".into());
    let loss_part = s
        .last_corner_deltas
        .iter()
        .max_by(|a, b| a.time_loss_s.total_cmp(&b.time_loss_s))
        .filter(|d| d.time_loss_s > 0.05)
        .map(|d| ctx.frag("race.loss_part", &[("corner", d.corner.to_string())]))
        .unwrap_or_default();
    ctx.push(CallRequest::new("race.summary").p("position", pos.to_string()).p("gained_part", gained_part).p("best", best).p("loss_part", loss_part));
}

pub fn tick(ctx: &mut Ctx, mem: &mut Memory) {
    let f = &ctx.state.frame;
    let live = f.status == SimStatus::Live;
    if !live {
        return;
    }
    let t = ctx.t;
    let since = *mem.live_since.get_or_insert(t);
    let ss = &ctx.settings.session_start;

    // Radio check ~3 s after the session went live.
    if !mem.radio_check_done && t - since > 3.0 {
        mem.radio_check_done = true;
        let wanted = match ss.radio_check {
            RadioCheckWhen::EverySession => true,
            RadioCheckWhen::FirstOfDay => !mem.first_session_of_day_done,
            RadioCheckWhen::RaceOnly => ctx.kind == SessionKind::Race,
            RadioCheckWhen::Never => false,
        };
        mem.first_session_of_day_done = true;
        if wanted {
            ctx.push(CallRequest::new("session.radio_check"));
            mem.greeting_at = Some(t + if ss.await_reply { 9.0 } else { 4.0 });
        } else {
            mem.greeting_at = Some(t + 1.0);
        }
        mem.spotter_intro_at = mem.greeting_at.map(|g| g + 6.0);
    }
    if let Some(at) = mem.greeting_at {
        if t >= at {
            mem.greeting_at = None;
            if ss.greeting {
                let minutes = mem.session_duration_ms.filter(|m| *m > 0.0).map(|m| ctx.fmt.minutes(m));
                let de = ctx.fmt.lang == Language::De;
                let minutes_part = minutes.map(|m| if de { format!(", {m} Minuten") } else { format!(", {m} minutes") }).unwrap_or_default();
                let wp = if matches!(ss.greeting_scope, GreetingScope::WithWeather | GreetingScope::WithBriefing) { weather_part(ctx) } else { String::new() };
                ctx.push(CallRequest::new("session.greeting").p("track", ctx.state.session.track.replace('_', " ")).p("session", session_name(ctx)).p("minutes_part", minutes_part).p("weather_part", wp));
                if ss.greeting_scope == GreetingScope::WithBriefing && ctx.kind == SessionKind::Race {
                    mem.briefing_done = true;
                    briefing(ctx);
                }
            }
        }
    }
    if let Some(at) = mem.spotter_intro_at {
        if t >= at {
            mem.spotter_intro_at = None;
            if ss.spotter_intro && ctx.settings.spotter.enabled {
                ctx.push(CallRequest::new("session.spotter_intro"));
            }
        }
    }
    // Race briefing before the start (pre-session phases), after the radio check and greeting.
    if ctx.kind == SessionKind::Race && !mem.briefing_done && mem.radio_check_done && mem.greeting_at.is_none() && matches!(ctx.state.session.phase, SessionPhase::PreFormation | SessionPhase::PreSession | SessionPhase::Starting) {
        mem.briefing_done = true;
        briefing(ctx);
    }
}

pub fn briefing(ctx: &mut Ctx) {
    let s = ctx.state;
    let st = &ctx.settings.strategy;
    let fs = s.fuel_strategy(ctx.settings);
    let avg = s.avg_lap_ms(st.clean_laps_window as usize);
    let minutes = s.frame.session_time_left_ms as f32 / 60_000.0;
    let fuel = match s.frame.laps_left {
        Some(laps) => crate::race::fuel::start_fuel_laps(laps.ceil(), fs.per_lap_l, st.formation_lap, st.fuel_reserve_laps, st.fuel_reserve_liters, s.session.max_fuel_l),
        None => crate::race::fuel::start_fuel(minutes, fs.per_lap_l, avg, st.formation_lap, st.fuel_reserve_laps, st.fuel_reserve_liters, s.session.max_fuel_l),
    };
    let fuel_txt = if fuel > 0.0 { ctx.fmt.fuel(fuel) } else { ctx.fmt.fuel(s.frame.fuel_l) };
    let wp = weather_part(ctx);
    ctx.push(CallRequest::new("prerace.briefing").p("fuel", fuel_txt).p("weather_part", wp));
}
