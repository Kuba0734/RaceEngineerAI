//! Weather forecast, grip changes, track temperature trend.

use super::session::rain_word;
use super::{Ctx, Memory};
use crate::calls::engine::CallRequest;
use crate::games::provider::{RainIntensity, TrackGrip};

pub fn tick(ctx: &mut Ctx, mem: &mut Memory) {
    let w = ctx.state.frame.weather;
    let now = (w.rain_now, w.rain_10min, w.rain_30min);
    if let Some((n0, t10, _t30)) = mem.last_rain {
        if w.rain_now > RainIntensity::None && n0 == RainIntensity::None {
            ctx.push(CallRequest::new("weather.forecast").sub("rain_now").p("intensity", rain_word(ctx, w.rain_now).to_lowercase()));
        } else if w.rain_10min > t10 && w.rain_10min > w.rain_now {
            ctx.push(CallRequest::new("weather.forecast").sub("rain_coming").p("intensity", rain_word(ctx, w.rain_10min)).p("minutes", "10"));
        } else if w.rain_now > RainIntensity::None && w.rain_10min == RainIntensity::None && t10 > RainIntensity::None {
            ctx.push(CallRequest::new("weather.forecast").sub("rain_stopping"));
        }
    } else if w.rain_10min > w.rain_now {
        ctx.push(CallRequest::new("weather.forecast").sub("rain_coming").p("intensity", rain_word(ctx, w.rain_10min)).p("minutes", "10"));
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
                ctx.push(CallRequest::new("track.grip").sub(sub));
            }
        }
    }
    mem.last_grip = Some(w.grip);

    if let Some(trend) = ctx.state.road_temp_trend_c_per_10min() {
        let th = ctx.th("track.temp", "c_per_10min") as f32;
        if trend.abs() >= th {
            let sub = if trend < 0.0 { "falling" } else { "rising" };
            ctx.push(CallRequest::new("track.temp").sub(sub).p("delta", ctx.fmt.temp(trend.abs())));
        }
    }
}
