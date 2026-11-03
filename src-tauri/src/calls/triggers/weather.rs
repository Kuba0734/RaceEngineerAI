//! Weather forecast, grip changes, track temperature trend.

use super::session::rain_word;
use super::{Ctx, Memory};
use crate::calls::engine::CallRequest;
use crate::games::provider::{RainIntensity, TrackGrip};

/// Rain trend from the raw precipitation and wetness history (games without a forecast):
/// compares now with `window_min` ago. Not a forecast – it says what is already happening.
fn rain_trend(ctx: &mut Ctx, mem: &mut Memory) {
    let w = ctx.state.frame.weather;
    let (Some(rain), Some(wet)) = (w.rain_amount, w.wetness_level) else { return };
    let t = ctx.t;
    let window = ctx.th("weather.trend", "window_min") * 60.0;
    let step = ctx.th("weather.trend", "rain_step_pct") as f32 / 100.0;
    if mem.weather_samples.back().is_none_or(|(ts, _, _)| t - ts >= 20.0) {
        mem.weather_samples.push_back((t, rain, wet));
    }
    while mem.weather_samples.front().is_some_and(|(ts, _, _)| t - ts > window * 2.0) {
        mem.weather_samples.pop_front();
    }
    let Some(&(_, r0, w0)) = mem.weather_samples.iter().rev().find(|(ts, _, _)| t - ts >= window) else { return };
    let sub = if rain - r0 >= step {
        "rain_heavier"
    } else if r0 - rain >= step && r0 > 0.0 {
        "rain_lighter"
    } else if wet > w0 {
        "track_wetter"
    } else if wet < w0 {
        "track_drying"
    } else {
        return;
    };
    // Measured again from here, so one change is said once.
    mem.weather_samples.clear();
    ctx.push(CallRequest::new("weather.trend").variant(sub));
}

pub fn tick(ctx: &mut Ctx, mem: &mut Memory) {
    rain_trend(ctx, mem);
    // Race control declares the track wet (wet tyres become legal).
    if let Some(wet) = ctx.state.frame.wet_tyres_allowed {
        if wet && mem.wet_declared.replace(wet) == Some(false) {
            ctx.push(CallRequest::new("weather.wet_declared"));
        }
        mem.wet_declared = Some(wet);
    }
    let w = ctx.state.frame.weather;
    let now = (w.rain_now, w.rain_10min, w.rain_30min);
    if let Some((n0, t10, _t30)) = mem.last_rain {
        if w.rain_now > RainIntensity::None && n0 == RainIntensity::None {
            ctx.push(CallRequest::new("weather.forecast").variant("rain_now").p("intensity", rain_word(ctx, w.rain_now).to_lowercase()));
        } else if w.rain_10min > t10 && w.rain_10min > w.rain_now {
            ctx.push(CallRequest::new("weather.forecast").variant("rain_coming").p("intensity", rain_word(ctx, w.rain_10min)).p("minutes", "10"));
        } else if w.rain_now > RainIntensity::None && w.rain_10min == RainIntensity::None && t10 > RainIntensity::None {
            ctx.push(CallRequest::new("weather.forecast").variant("rain_stopping"));
        }
    } else if w.rain_10min > w.rain_now {
        ctx.push(CallRequest::new("weather.forecast").variant("rain_coming").p("intensity", rain_word(ctx, w.rain_10min)).p("minutes", "10"));
    }
    mem.last_rain = Some(now);

    if let Some(g) = mem.last_grip {
        if g != w.grip {
            let wet = |x: TrackGrip| matches!(x, TrackGrip::Damp | TrackGrip::Wet | TrackGrip::Flooded);
            let sub = if wet(w.grip) && w.grip > g {
                "wetter"
            } else if wet(g) && !wet(w.grip) || (wet(g) && w.grip < g) {
                "drying"
            } else if !wet(w.grip) && !wet(g) && w.grip < g {
                // Greasy → Optimum/Fast: faster.
                "faster"
            } else {
                ""
            };
            if !sub.is_empty() {
                ctx.push(CallRequest::new("track.grip").variant(sub));
            }
        }
    }
    mem.last_grip = Some(w.grip);

    if let Some(trend) = ctx.state.road_temp_trend_c_per_10min() {
        let th = ctx.th("track.temp", "c_per_10min") as f32;
        if trend.abs() >= th {
            let sub = if trend < 0.0 { "falling" } else { "rising" };
            ctx.push(CallRequest::new("track.temp").variant(sub).p("delta", ctx.fmt.temp(trend.abs())));
        }
    }
}
