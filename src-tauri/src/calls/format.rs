//! Turning numbers into radio-friendly words per language, precision and units.

use crate::games::provider::CarState;
use crate::storage::settings::{FuelUnit, Language, OpponentNaming, PressureUnit, Settings, SpeedUnit, TempUnit};

#[derive(Clone)]
pub struct Fmt {
    pub lang: Language,
    pub exact: bool,
    pub fuel: FuelUnit,
    pub temp: TempUnit,
    pub pressure: PressureUnit,
    pub speed: SpeedUnit,
    pub naming: Vec<OpponentNaming>,
}

impl Fmt {
    pub fn new(s: &Settings) -> Self {
        Self {
            lang: s.language,
            exact: s.personality.exact_numbers,
            fuel: s.units.fuel,
            temp: s.units.temperature,
            pressure: s.units.pressure,
            speed: s.units.speed,
            naming: s.personality.opponent_naming.clone(),
        }
    }

    fn de(&self) -> bool {
        self.lang == Language::De
    }

    pub fn num(&self, v: f32, decimals: usize) -> String {
        let s = format!("{:.*}", decimals, v);
        if self.de() {
            s.replace('.', ",")
        } else {
            s
        }
    }

    /// Gap in seconds: "1,4" (rounded) / "1,46" (exact). Values < 1 s become tenths.
    pub fn gap(&self, s: f32) -> String {
        let a = s.abs();
        if a < 1.0 && !self.exact {
            return self.tenths(a);
        }
        self.num(a, if self.exact { 2 } else { 1 })
    }

    /// "4 Zehntel" / "4 tenths".
    pub fn tenths(&self, s: f32) -> String {
        let t = (s.abs() * 10.0).round().max(1.0) as i32;
        match (self.de(), t) {
            (true, 1) => "ein Zehntel".into(),
            (true, _) => format!("{t} Zehntel"),
            (false, 1) => "a tenth".into(),
            (false, _) => format!("{t} tenths"),
        }
    }

    pub fn laptime(&self, ms: i32) -> String {
        if ms <= 0 {
            return "-".into();
        }
        let m = ms / 60_000;
        let s = (ms % 60_000) as f32 / 1000.0;
        let sec = if self.exact { format!("{:06.3}", s) } else { format!("{:04.1}", s) };
        let sec = if self.de() { sec.replace('.', ",") } else { sec };
        if m > 0 {
            format!("{m}:{sec}")
        } else {
            sec
        }
    }

    pub fn fuel(&self, liters: f32) -> String {
        match self.fuel {
            FuelUnit::Liters => {
                let v = if self.exact || liters < 10.0 { self.num(liters, 1) } else { format!("{}", liters.round() as i32) };
                if self.de() {
                    format!("{v} Liter")
                } else {
                    format!("{v} litres")
                }
            }
            FuelUnit::Gallons => {
                let g = liters * 0.264_172;
                if self.de() {
                    format!("{} Gallonen", self.num(g, 1))
                } else {
                    format!("{} gallons", self.num(g, 1))
                }
            }
        }
    }

    pub fn fuel_per_lap(&self, liters: f32) -> String {
        match self.fuel {
            FuelUnit::Liters => self.num(liters, if self.exact { 2 } else { 1 }),
            FuelUnit::Gallons => self.num(liters * 0.264_172, 2),
        }
    }

    pub fn temp(&self, c: f32) -> String {
        match self.temp {
            TempUnit::Celsius => format!("{} {}", c.round() as i32, if self.de() { "Grad" } else { "degrees" }),
            TempUnit::Fahrenheit => format!("{} {}", (c * 9.0 / 5.0 + 32.0).round() as i32, if self.de() { "Grad Fahrenheit" } else { "Fahrenheit" }),
        }
    }

    pub fn pressure(&self, psi: f32) -> String {
        match self.pressure {
            PressureUnit::Psi => self.num(psi, 1),
            PressureUnit::Bar => self.num(psi * 0.068_947_6, 2),
        }
    }

    pub fn pressure_delta(&self, psi: f32) -> String {
        let v = self.pressure(psi.abs());
        if psi < 0.0 {
            format!("minus {v}")
        } else {
            format!("plus {v}")
        }
    }

    pub fn speed(&self, kmh: f32) -> String {
        match self.speed {
            SpeedUnit::Kmh => format!("{}", kmh.round() as i32),
            SpeedUnit::Mph => format!("{}", (kmh * 0.621_371).round() as i32),
        }
    }

    pub fn meters(&self, m: f32) -> String {
        let r = if self.exact { m.round() } else { (m / 5.0).round().max(1.0) * 5.0 };
        if self.de() {
            format!("{} Meter", r as i32)
        } else {
            format!("{} metres", r as i32)
        }
    }

    pub fn laps(&self, n: f32) -> String {
        let v = if self.exact { self.num(n, 1) } else { format!("{}", n.floor().max(0.0) as i32) };
        v
    }

    pub fn minutes(&self, ms: f64) -> String {
        format!("{}", (ms / 60_000.0).round().max(0.0) as i32)
    }

    pub fn ordinal_sector(&self, idx: usize) -> String {
        match (self.de(), idx) {
            (true, 0) => "ersten".into(),
            (true, 1) => "zweiten".into(),
            (true, 2) => "dritten".into(),
            (false, 0) => "first".into(),
            (false, 1) => "second".into(),
            (false, 2) => "third".into(),
            _ => format!("{}", idx + 1),
        }
    }

    pub fn wheel(&self, i: usize) -> &'static str {
        match (self.de(), i) {
            (true, 0) => "vorne links",
            (true, 1) => "vorne rechts",
            (true, 2) => "hinten links",
            (true, _) => "hinten rechts",
            (false, 0) => "front left",
            (false, 1) => "front right",
            (false, 2) => "rear left",
            (false, _) => "rear right",
        }
    }

    pub fn axle(&self, front: bool) -> &'static str {
        match (self.de(), front) {
            (true, true) => "vorne",
            (true, false) => "hinten",
            (false, true) => "front",
            (false, false) => "rear",
        }
    }

    /// Opponent naming per personality settings, e.g. "P3, der Porsche" or "die 88".
    pub fn car(&self, c: &CarState) -> String {
        let mut parts = Vec::new();
        for n in &self.naming {
            let p = match n {
                OpponentNaming::Position if c.position > 0 => Some(format!("P{}", c.position)),
                OpponentNaming::Number if c.race_number > 0 => Some(if self.de() { format!("die {}", c.race_number) } else { format!("the number {}", c.race_number) }),
                OpponentNaming::Car if !c.car_brand.is_empty() => Some(if self.de() { format!("der {}", c.car_brand) } else { format!("the {}", c.car_brand) }),
                OpponentNaming::Driver if !c.driver.is_empty() => Some(c.driver.clone()),
                _ => None,
            };
            if let Some(p) = p {
                parts.push(p);
            }
        }
        if parts.is_empty() {
            // Fallback chain when the preferred data is missing.
            if c.position > 0 {
                parts.push(format!("P{}", c.position));
            } else if c.race_number > 0 {
                parts.push(format!("#{}", c.race_number));
            }
        }
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn german_numbers() {
        let f = Fmt::new(&Settings { language: Language::De, ..Settings::default() });
        assert_eq!(f.gap(1.46), "1,5");
        assert_eq!(f.gap(0.42), "4 Zehntel");
        assert_eq!(f.laptime(107_912), "1:47,9");
        assert_eq!(f.fuel(38.4), "38 Liter");
        assert_eq!(f.meters(8.0), "10 Meter");
    }

    #[test]
    fn opponent_naming() {
        let mut s = Settings { language: Language::De, ..Settings::default() };
        let c = CarState { position: 3, race_number: 88, car_brand: "Porsche".into(), driver: "Müller".into(), ..Default::default() };
        assert_eq!(Fmt::new(&s).car(&c), "P3, der Porsche");
        s.personality.opponent_naming = vec![OpponentNaming::Number];
        assert_eq!(Fmt::new(&s).car(&c), "die 88");
        s.language = Language::En;
        s.personality.opponent_naming = vec![OpponentNaming::Driver, OpponentNaming::Car];
        assert_eq!(Fmt::new(&s).car(&c), "Müller, the Porsche");
    }

    #[test]
    fn exact_mode() {
        let mut s = Settings::default();
        s.personality.exact_numbers = true;
        s.language = Language::En;
        let f = Fmt::new(&s);
        assert_eq!(f.gap(1.456), "1.46");
        assert_eq!(f.laptime(107_912), "1:47.912");
    }
}
