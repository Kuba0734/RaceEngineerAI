//! Car database from `resources/cars.json` (model ids, brands, classes, target windows).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use parking_lot::RwLock;

use crate::storage::settings::ClassTargets;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassDef {
    pub dry: [f32; 2],
    pub wet: [f32; 2],
    pub core_temp_cold: f32,
    pub core_temp_hot: f32,
    pub brake_temp_hot: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CarDef {
    pub id: i32,
    pub key: String,
    pub name: String,
    pub brand: String,
    pub class: String,
}

#[derive(Debug, Deserialize)]
pub struct CarDb {
    pub classes: BTreeMap<String, ClassDef>,
    pub acc: Vec<CarDef>,
}

pub fn db() -> &'static CarDb {
    static DB: OnceLock<CarDb> = OnceLock::new();
    DB.get_or_init(|| {
        serde_json::from_str(include_str!("../../resources/cars.json")).unwrap_or_else(|e| {
            tracing::error!("cars.json invalid: {e}");
            CarDb { classes: BTreeMap::new(), acc: Vec::new() }
        })
    })
}

pub fn acc_by_id(id: i32) -> Option<&'static CarDef> {
    db().acc.iter().find(|c| c.id == id)
}

pub fn acc_by_key(key: &str) -> Option<&'static CarDef> {
    db().acc.iter().find(|c| c.key == key)
}

/// Class for an unknown model key (newer cars not in the documentation).
pub fn guess_class(key: &str) -> &'static str {
    let k = key.to_lowercase();
    if k.contains("gt4") {
        "GT4"
    } else if k.contains("cup") {
        "CUP"
    } else if k.contains("challenge") {
        "CHL"
    } else if k.ends_with("_st") || k.contains("_st_") {
        "ST"
    } else {
        "GT3"
    }
}

fn overrides() -> &'static RwLock<BTreeMap<String, ClassDef>> {
    static O: OnceLock<RwLock<BTreeMap<String, ClassDef>>> = OnceLock::new();
    O.get_or_init(|| RwLock::new(BTreeMap::new()))
}

/// The user's own target windows (settings), checked before `cars.json`.
pub fn set_overrides(map: &BTreeMap<String, ClassTargets>) {
    *overrides().write() = map
        .iter()
        .map(|(k, t)| (k.clone(), ClassDef { dry: t.dry, wet: t.wet, core_temp_cold: t.core_temp_cold, core_temp_hot: t.core_temp_hot, brake_temp_hot: t.brake_temp_hot }))
        .collect();
}

/// Defaults of every known class (for the settings page).
pub fn class_defaults() -> BTreeMap<String, ClassTargets> {
    db().classes
        .iter()
        .map(|(k, c)| (k.clone(), ClassTargets { dry: c.dry, wet: c.wet, core_temp_cold: c.core_temp_cold, core_temp_hot: c.core_temp_hot, brake_temp_hot: c.brake_temp_hot }))
        .collect()
}

pub fn class_def(class: &str) -> ClassDef {
    if let Some(c) = overrides().read().get(class) {
        return c.clone();
    }
    db().classes.get(class).cloned().unwrap_or(ClassDef {
        dry: [26.0, 27.0],
        wet: [29.5, 31.0],
        core_temp_cold: 65.0,
        core_temp_hot: 100.0,
        brake_temp_hot: 750.0,
    })
}

/// Display name for an unknown model key: "porsche_992_gt3_r" → "Porsche".
pub fn brand_from_key(key: &str) -> String {
    let first = key.split('_').next().unwrap_or(key);
    let mut c = first.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup() {
        assert_eq!(acc_by_id(23).unwrap().brand, "Porsche");
        assert_eq!(acc_by_key("bmw_m4_gt4").unwrap().class, "GT4");
        assert_eq!(guess_class("ford_mustang_gt3"), "GT3");
        assert_eq!(brand_from_key("ford_mustang_gt3"), "Ford");
        assert!(class_def("GT3").dry[0] > 20.0);
        let mut own = BTreeMap::new();
        own.insert("TEST".to_string(), ClassTargets { dry: [24.0, 25.0], wet: [28.0, 29.0], core_temp_cold: 60.0, core_temp_hot: 95.0, brake_temp_hot: 700.0 });
        set_overrides(&own);
        assert_eq!(class_def("TEST").dry, [24.0, 25.0]);
        set_overrides(&BTreeMap::new());
        assert!(class_defaults().contains_key("GT3"));
    }
}
