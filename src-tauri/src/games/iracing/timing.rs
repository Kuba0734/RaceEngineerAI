//! Sector times of every car. iRacing only reports lap times per car, so the provider times
//! the three thirds of the lap itself from `CarIdxLapDistPct` and `SessionTime` (60 Hz →
//! about ±16 ms per split).

use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct Splits {
    pub current: [Option<i32>; 3],
    pub last: [Option<i32>; 3],
    pub best: [Option<i32>; 3],
    /// Session time when the current lap started (s).
    pub lap_start: Option<f64>,
    sector_start: Option<f64>,
    sector: usize,
    last_pct: f32,
    /// The lap touched the pit lane: its splits are not representative.
    tainted: bool,
}

#[derive(Default)]
pub struct Timing {
    cars: HashMap<i32, Splits>,
}

fn sector_of(pct: f32) -> usize {
    ((pct * 3.0) as usize).min(2)
}

impl Timing {
    pub fn reset(&mut self) {
        self.cars.clear();
    }

    pub fn get(&self, id: i32) -> Option<&Splits> {
        self.cars.get(&id)
    }

    /// Feeds one sample of a car. `pct` < 0 = not in the world.
    pub fn update(&mut self, id: i32, pct: f32, t: f64, in_pits: bool) {
        let s = self.cars.entry(id).or_default();
        if !(0.0..=1.0).contains(&pct) {
            *s = Splits { best: s.best, last: s.last, ..Default::default() };
            return;
        }
        if in_pits {
            s.tainted = true;
        }
        let prev = s.last_pct;
        s.last_pct = pct;
        let crossed_line = prev > 0.9 && pct < 0.1;
        let sec = sector_of(pct);
        if crossed_line {
            if let Some(st) = s.sector_start {
                if s.sector == 2 {
                    s.current[2] = Some(((t - st) * 1000.0) as i32);
                }
            }
            let complete = s.current.iter().all(|x| x.is_some()) && s.lap_start.is_some();
            if complete {
                s.last = s.current;
                if !s.tainted {
                    for k in 0..3 {
                        s.best[k] = match (s.best[k], s.current[k]) {
                            (Some(b), Some(c)) => Some(b.min(c)),
                            (None, c) => c,
                            (b, None) => b,
                        };
                    }
                }
            }
            s.current = [None; 3];
            s.lap_start = Some(t);
            s.sector_start = Some(t);
            s.sector = 0;
            s.tainted = in_pits;
        } else if sec == s.sector + 1 {
            if let Some(st) = s.sector_start {
                s.current[s.sector] = Some(((t - st) * 1000.0) as i32);
            }
            s.sector = sec;
            s.sector_start = Some(t);
        } else if sec != s.sector {
            // Jumped (tow, reset, joined mid-lap): wait for the next line crossing.
            s.sector = sec;
            s.sector_start = None;
            s.lap_start = None;
        }
    }

    pub fn current_lap_ms(&self, id: i32, t: f64) -> Option<i32> {
        self.cars.get(&id)?.lap_start.map(|st| ((t - st) * 1000.0) as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_three_sectors_and_keeps_best() {
        let mut tm = Timing::default();
        // Lap 1 joined mid-lap, lap 2 = 30/31/32 s, lap 3 = 29/32/32 s (60 Hz).
        let mut t = 0.0;
        let mut drive = |tm: &mut Timing, from: f32, to: f32, secs: f64| {
            let steps = (secs * 60.0) as usize;
            for k in 1..=steps {
                let pct = (from + (to - from) * k as f32 / steps as f32).fract();
                t += 1.0 / 60.0;
                tm.update(1, pct, t, false);
            }
        };
        drive(&mut tm, 0.5, 1.0, 10.0);
        drive(&mut tm, 0.0, 1.0 / 3.0, 30.0);
        drive(&mut tm, 1.0 / 3.0, 2.0 / 3.0, 31.0);
        drive(&mut tm, 2.0 / 3.0, 1.0, 32.0);
        drive(&mut tm, 0.0, 1.0 / 3.0, 29.0);
        drive(&mut tm, 1.0 / 3.0, 2.0 / 3.0, 32.0);
        drive(&mut tm, 2.0 / 3.0, 1.0, 32.0);
        drive(&mut tm, 0.0, 0.05, 1.0);
        let s = tm.get(1).unwrap();
        let near = |v: Option<i32>, ms: i32| v.is_some_and(|v| (v - ms).abs() <= 40);
        assert!(near(s.last[0], 29_000) && near(s.last[1], 32_000) && near(s.last[2], 32_000), "{:?}", s.last);
        assert!(near(s.best[0], 29_000) && near(s.best[1], 31_000) && near(s.best[2], 32_000), "{:?}", s.best);
        assert!(s.current[0].is_none());
    }

    #[test]
    fn pit_lap_does_not_set_best() {
        let mut tm = Timing::default();
        let mut t = 0.0;
        let mut run_lap = |tm: &mut Timing, pit: bool, secs_per_step: f64| {
            for k in 0..=600 {
                t += secs_per_step;
                tm.update(2, k as f32 / 600.0 * 0.999, t, pit && k < 50);
            }
        };
        run_lap(&mut tm, false, 0.1); // joined: incomplete
        run_lap(&mut tm, true, 0.05); // fast but through the pits
        run_lap(&mut tm, false, 0.1); // its start completes the pit lap
        assert!(tm.get(2).unwrap().best[0].is_none());
        assert!(tm.get(2).unwrap().last[0].is_some());
        run_lap(&mut tm, false, 0.1);
        // Best only from the clean lap, never from the pit lap.
        let b = tm.get(2).unwrap().best[0].unwrap();
        assert!(b > 15_000, "{b}");
    }
}
