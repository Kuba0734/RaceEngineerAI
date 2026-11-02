//! Estimated iRating change and strength of field. iRacing does not publish its formula; this
//! is the widely used community reconstruction (iRacing forums, also used by other overlays):
//! an Elo-like expectation with base `1600 / ln 2`. Results are estimates, labelled as such.

use std::f64::consts::LN_2;

const BR1: f64 = 1600.0 / LN_2;

/// Chance that a driver rated `a` finishes ahead of one rated `b`.
fn chance(a: f64, b: f64) -> f64 {
    let (ea, eb) = ((-a / BR1).exp(), (-b / BR1).exp());
    ((1.0 - ea) * eb) / ((1.0 - eb) * ea + (1.0 - ea) * eb)
}

/// Strength of field of a class.
pub fn sof(ratings: &[f64]) -> Option<f64> {
    if ratings.is_empty() {
        return None;
    }
    let sum: f64 = ratings.iter().map(|r| (-r / BR1).exp()).sum();
    Some(BR1 * (ratings.len() as f64 / sum).ln())
}

/// Change per driver if the race ended now. `ratings` in finishing order (index 0 = P1).
pub fn changes(ratings: &[f64]) -> Vec<f64> {
    let n = ratings.len() as f64;
    ratings
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let pos = (i + 1) as f64;
            // Expected number of drivers beaten (the comparison with oneself counts 0.5).
            let expected: f64 = ratings.iter().map(|b| chance(*a, *b)).sum::<f64>() - 0.5;
            let fudge = (n / 2.0 - pos) / 100.0;
            (n - pos - expected - fudge) * 200.0 / n
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_field_winner_gains_loser_drops() {
        let c = changes(&[2000.0, 2000.0]);
        assert!((c[0] - 50.0).abs() < 1e-9 && (c[1] + 49.0).abs() < 1e-9, "{c:?}");
    }

    #[test]
    fn favourite_gains_little_for_winning() {
        let c = changes(&[4000.0, 1500.0, 1500.0, 1500.0]);
        let upset = changes(&[1500.0, 4000.0, 1500.0, 1500.0]);
        assert!(c[0] > 0.0 && c[0] < upset[0]);
        assert!(upset[1] < 0.0);
    }

    #[test]
    fn sof_of_equal_field_is_that_rating() {
        assert!((sof(&[2500.0, 2500.0, 2500.0]).unwrap() - 2500.0).abs() < 1e-6);
        let mixed = sof(&[1000.0, 3000.0]).unwrap();
        assert!(mixed > 1000.0 && mixed < 3000.0);
        assert_eq!(sof(&[]), None);
    }
}
