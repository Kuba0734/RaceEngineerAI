//! Spoken text templates per language with variants (`resources/calls/<lang>.json`).
//!
//! An entry is either a list of variants or an object keyed by style/phrasing
//! (`default`, `calm`, `motivating`, `british`, `formal`, `short`, `normal`, `double`).
//! Sub-variants use `call_id:sub`. Placeholders: `{name}`.

use std::collections::HashMap;
use std::sync::OnceLock;

use parking_lot::Mutex;
use serde_json::Value;

use crate::storage::settings::Language;

type Table = HashMap<String, Value>;

fn table(lang: Language) -> &'static Table {
    static DE: OnceLock<Table> = OnceLock::new();
    static EN: OnceLock<Table> = OnceLock::new();
    let (cell, src) = match lang {
        Language::De => (&DE, include_str!("../../resources/calls/de.json")),
        Language::En => (&EN, include_str!("../../resources/calls/en.json")),
    };
    cell.get_or_init(|| serde_json::from_str(src).unwrap_or_else(|e| {
        tracing::error!("templates {lang:?} invalid: {e}");
        HashMap::new()
    }))
}

fn variants<'a>(v: &'a Value, keys: &[&str]) -> Vec<&'a str> {
    let list = match v {
        Value::Array(_) => Some(v),
        Value::Object(map) => keys.iter().find_map(|k| map.get(*k)).or_else(|| map.get("default")).or_else(|| map.get("normal")).or_else(|| map.values().next()),
        _ => None,
    };
    list.and_then(|l| l.as_array()).map(|a| a.iter().filter_map(|x| x.as_str()).collect()).unwrap_or_default()
}

/// Picks a variant, avoiding the one used last time for the same key.
fn pick<'a>(key: &str, options: &[&'a str]) -> Option<&'a str> {
    static LAST: OnceLock<Mutex<HashMap<String, usize>>> = OnceLock::new();
    if options.is_empty() {
        return None;
    }
    let mut last = LAST.get_or_init(|| Mutex::new(HashMap::new())).lock();
    let prev = last.get(key).copied();
    let mut i = rand::random_range(0..options.len());
    if options.len() > 1 && Some(i) == prev {
        i = (i + 1) % options.len();
    }
    last.insert(key.to_string(), i);
    Some(options[i])
}

pub fn fill(template: &str, params: &[(&str, String)]) -> String {
    let mut out = template.to_string();
    for (k, v) in params {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    // Unfilled placeholders are removed rather than spoken.
    while let (Some(a), Some(b)) = (out.find('{'), out.find('}')) {
        if b < a {
            break;
        }
        out.replace_range(a..=b, "");
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ").replace(" .", ".").replace(" ,", ",")
}

/// Renders `key` (call id or `call_id:sub`). `style_keys` in order of preference.
pub fn render(lang: Language, key: &str, style_keys: &[&str], params: &[(&str, String)]) -> Option<String> {
    let t = table(lang);
    let v = t.get(key)?;
    let opts = variants(v, style_keys);
    pick(key, &opts).map(|tpl| fill(tpl, params))
}

/// All variants (for pre-rendering spotter clips).
pub fn all_variants(lang: Language, key: &str, style_keys: &[&str]) -> Vec<String> {
    table(lang).get(key).map(|v| variants(v, style_keys).into_iter().map(String::from).collect()).unwrap_or_default()
}

pub fn exists(lang: Language, key: &str) -> bool {
    table(lang).contains_key(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_with_params_and_styles() {
        let s = render(Language::De, "fuel.critical", &[], &[("laps", "2".into())]).unwrap();
        assert!(s.contains('2'), "{s}");
        let s = render(Language::De, "spotter.left", &["short"], &[]).unwrap();
        assert_eq!(s, "Links!");
        let s = render(Language::En, "lap.personal_best", &["british"], &[("laptime", "1:47.9".into())]).unwrap();
        assert!(s.contains("Splendid"));
    }

    #[test]
    fn missing_params_are_dropped() {
        assert_eq!(fill("Gap {gap}{trend_part}.", &[("gap", "1.2".into())]), "Gap 1.2.");
    }

    #[test]
    fn every_template_parses_in_both_languages() {
        for lang in [Language::De, Language::En] {
            for (k, v) in table(lang) {
                assert!(!variants(v, &[]).is_empty(), "{lang:?} {k} empty");
            }
        }
    }
}
