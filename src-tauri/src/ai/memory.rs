//! Conversation memory: the last radio exchanges of the session for follow-up questions.

use std::collections::VecDeque;

use serde_json::{json, Value};

#[derive(Default)]
pub struct Conversation {
    turns: VecDeque<(String, String)>,
}

impl Conversation {
    pub fn push(&mut self, driver: String, engineer: String, keep: usize) {
        self.turns.push_back((driver, engineer));
        while self.turns.len() > keep {
            self.turns.pop_front();
        }
    }

    pub fn clear(&mut self) {
        self.turns.clear();
    }

    pub fn messages(&self) -> Vec<Value> {
        self.turns.iter().flat_map(|(u, a)| [json!({ "role": "user", "content": u }), json!({ "role": "assistant", "content": a })]).collect()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn keeps_last_n() {
        let mut c = super::Conversation::default();
        for i in 0..5 {
            c.push(format!("q{i}"), format!("a{i}"), 2);
        }
        let m = c.messages();
        assert_eq!(m.len(), 4);
        assert_eq!(m[0]["content"], "q3");
    }
}
