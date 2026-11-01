//! Crew state changed by voice commands: quiet mode, temporary preset, recurring calls,
//! reminders, strategy plan, volume. Used by the AI action tools and the fast path.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum QuietUntil {
    /// Until `completed_laps` reaches this value.
    Lap { lap: i32 },
    /// Until the session clock (s) passes this value.
    Time { t: f64 },
    /// Until the player passes this spline position (e.g. corner 1) on `lap` or later.
    Spline { lap: i32, spline: f32 },
    Forever,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Recurring {
    pub call_id: String,
    pub every_laps: i32,
    pub last_lap: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Reminder {
    pub id: u32,
    pub at_lap: Option<i32>,
    pub at_time_s: Option<f64>,
    pub text: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CrewState {
    pub quiet: Option<QuietUntil>,
    pub preset_override: Option<String>,
    pub recurring: Vec<Recurring>,
    pub reminders: Vec<Reminder>,
    pub plan_pit_lap: Option<i32>,
    /// Multiplier on top of the voice volumes (voice command "louder/quieter").
    pub volume: f32,
    pub last_message: Option<String>,
    pub last_message_call: Option<String>,
    /// Session time (s) until which a reply to the radio check is expected.
    pub awaiting_radio_check_until: Option<f64>,
    next_reminder_id: u32,
}

impl CrewState {
    pub fn new() -> Self {
        Self { volume: 1.0, ..Default::default() }
    }

    pub fn is_quiet(&mut self, t: f64, lap: i32, spline: f32) -> bool {
        let active = match &self.quiet {
            None => false,
            Some(QuietUntil::Lap { lap: l }) => lap < *l,
            Some(QuietUntil::Time { t: until }) => t < *until,
            Some(QuietUntil::Spline { lap: l, spline: s }) => lap < *l || (lap == *l && spline < *s),
            Some(QuietUntil::Forever) => true,
        };
        if !active {
            self.quiet = None;
        }
        active
    }

    pub fn add_reminder(&mut self, at_lap: Option<i32>, at_time_s: Option<f64>, text: String) -> u32 {
        self.next_reminder_id += 1;
        self.reminders.push(Reminder { id: self.next_reminder_id, at_lap, at_time_s, text });
        self.next_reminder_id
    }

    /// Returns reminders that are due now and removes them.
    pub fn due_reminders(&mut self, lap: i32, t: f64) -> Vec<Reminder> {
        let (due, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut self.reminders)
            .into_iter()
            .partition(|r| r.at_lap.is_some_and(|l| lap >= l) || r.at_time_s.is_some_and(|x| t >= x));
        self.reminders = keep;
        due
    }

    pub fn add_recurring(&mut self, call_id: &str, every_laps: i32, lap: i32) {
        self.recurring.retain(|r| r.call_id != call_id);
        if every_laps > 0 {
            self.recurring.push(Recurring { call_id: call_id.into(), every_laps, last_lap: lap - every_laps });
        }
    }

    pub fn reset_session(&mut self) {
        let volume = self.volume;
        let preset = self.preset_override.take();
        *self = Self::new();
        self.volume = volume;
        self.preset_override = preset;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_for_laps() {
        let mut c = CrewState::new();
        c.quiet = Some(QuietUntil::Lap { lap: 7 });
        assert!(c.is_quiet(0.0, 5, 0.5));
        assert!(!c.is_quiet(0.0, 7, 0.0));
        assert!(c.quiet.is_none());
    }

    #[test]
    fn reminders_fire_once() {
        let mut c = CrewState::new();
        c.add_reminder(Some(10), None, "Box".into());
        assert!(c.due_reminders(9, 0.0).is_empty());
        assert_eq!(c.due_reminders(10, 0.0).len(), 1);
        assert!(c.due_reminders(11, 0.0).is_empty());
    }
}
