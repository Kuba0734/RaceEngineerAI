//! Radio silence around the race start: the engineer talks until shortly before the start,
//! then stays quiet until one full race lap is done (the spotter is busiest there).
//!
//! Distance is accumulated from spline deltas instead of lap counters, because it is not
//! documented whether the game counts the formation lap as a completed lap.

use crate::games::provider::SessionPhase;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum Stage {
    #[default]
    Before,
    /// Quiet, waiting for the green flag.
    WaitingForGreen,
    /// Quiet, counting the race lap.
    RaceLap,
    Done,
}

#[derive(Debug, Default)]
pub struct StartHush {
    stage: Stage,
    last_spline: Option<f32>,
    /// Lap fractions driven in the current stage.
    driven: f32,
    in_formation: bool,
}

pub struct HushInput {
    pub enabled: bool,
    pub race: bool,
    pub phase: SessionPhase,
    pub completed_laps: i32,
    pub spline: f32,
    pub speed_kmh: f32,
    /// Share of the formation lap after which silence starts.
    pub formation_quiet_from: f32,
    /// Race laps of silence after the start.
    pub quiet_laps: f32,
}

/// Below this speed the car counts as standing on the grid.
const MOVING_KMH: f32 = 5.0;

impl StartHush {
    fn start_counting(&mut self) {
        self.stage = Stage::RaceLap;
        self.driven = 0.0;
    }

    /// Advances the state machine; returns whether the engineer must stay quiet now.
    pub fn update(&mut self, i: &HushInput) -> bool {
        // Forward movement along the spline, also across the line (wrap-around).
        let delta = self.last_spline.map_or(0.0, |l| (i.spline - l).rem_euclid(1.0));
        self.last_spline = Some(i.spline);
        if delta < 0.5 {
            self.driven += delta;
        }
        if !i.enabled || !i.race {
            return false;
        }
        if matches!(i.phase, SessionPhase::SessionOver | SessionPhase::PostSession | SessionPhase::Results) {
            self.stage = Stage::Done;
        }
        match self.stage {
            Stage::Before => match i.phase {
                SessionPhase::Formation => {
                    if !self.in_formation {
                        self.in_formation = true;
                        self.driven = 0.0;
                    }
                    if self.driven >= i.formation_quiet_from {
                        self.stage = Stage::WaitingForGreen;
                    }
                }
                SessionPhase::PreSession => self.stage = Stage::WaitingForGreen,
                SessionPhase::Session => {
                    if i.completed_laps == 0 {
                        self.start_counting();
                    } else {
                        // Joined a running race: nothing to protect.
                        self.stage = Stage::Done;
                    }
                }
                // No phase data (broadcasting not connected): quiet from the first metres.
                SessionPhase::Unknown => {
                    if i.completed_laps > 0 {
                        self.stage = Stage::Done;
                    } else if i.speed_kmh > MOVING_KMH {
                        self.start_counting();
                    }
                }
                _ => {}
            },
            Stage::WaitingForGreen => {
                if i.phase == SessionPhase::Session {
                    self.start_counting();
                }
            }
            Stage::RaceLap => {
                if self.driven >= i.quiet_laps {
                    self.stage = Stage::Done;
                }
            }
            Stage::Done => {}
        }
        matches!(self.stage, Stage::WaitingForGreen | Stage::RaceLap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(phase: SessionPhase, spline: f32) -> HushInput {
        HushInput { enabled: true, race: true, phase, completed_laps: 0, spline, speed_kmh: 100.0, formation_quiet_from: 0.75, quiet_laps: 1.0 }
    }

    /// Drives `laps` from `start` in small steps; returns (final spline, was quiet at the end).
    fn drive(h: &mut StartHush, phase: SessionPhase, start: f32, laps: f32) -> (f32, bool) {
        let steps = (laps / 0.01) as i32;
        let mut quiet = false;
        let mut s = start;
        for _ in 0..steps {
            s = (s + 0.01).rem_euclid(1.0);
            quiet = h.update(&input(phase, s));
        }
        (s, quiet)
    }

    #[test]
    fn talk_formation_quiet_race_lap_talk() {
        let mut h = StartHush::default();
        // On the grid before the formation lap: talking (grid spline close to the line).
        assert!(!h.update(&input(SessionPhase::PreFormation, 0.95)));
        // Early formation lap: still talking, although the spline value is already high.
        let (s, quiet) = drive(&mut h, SessionPhase::Formation, 0.95, 0.5);
        assert!(!quiet);
        // End of the formation lap: quiet.
        let (s, quiet) = drive(&mut h, SessionPhase::Formation, s, 0.3);
        assert!(quiet);
        // Green: quiet for the whole race lap.
        let (s, quiet) = drive(&mut h, SessionPhase::Session, s, 0.95);
        assert!(quiet);
        // Lap complete: talking again.
        let (_, quiet) = drive(&mut h, SessionPhase::Session, s, 0.1);
        assert!(!quiet);
    }

    #[test]
    fn distance_driven_before_green_does_not_count() {
        let mut h = StartHush::default();
        assert!(h.update(&input(SessionPhase::PreSession, 0.5)));
        let (s, _) = drive(&mut h, SessionPhase::PreSession, 0.5, 2.0);
        let (_, quiet) = drive(&mut h, SessionPhase::Session, s, 0.5);
        assert!(quiet);
    }

    #[test]
    fn joining_a_running_race_stays_talkative() {
        let mut h = StartHush::default();
        let mut i = input(SessionPhase::Session, 0.3);
        i.completed_laps = 5;
        assert!(!h.update(&i));
    }

    #[test]
    fn no_silence_outside_races_or_when_disabled() {
        let mut h = StartHush::default();
        let mut i = input(SessionPhase::Session, 0.3);
        i.race = false;
        assert!(!h.update(&i));
        let mut h = StartHush::default();
        let mut i = input(SessionPhase::Session, 0.3);
        i.enabled = false;
        assert!(!h.update(&i));
    }

    #[test]
    fn without_phase_data_waits_for_the_car_to_move() {
        let mut h = StartHush::default();
        let mut i = input(SessionPhase::Unknown, 0.9);
        i.speed_kmh = 0.0;
        assert!(!h.update(&i));
        i.speed_kmh = 60.0;
        assert!(h.update(&i));
    }
}
