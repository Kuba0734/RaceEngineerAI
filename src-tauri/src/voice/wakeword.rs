//! Local wake word with sherpa-onnx keyword spotting (English zipformer, BPE tokens).

use std::collections::HashSet;
use std::sync::OnceLock;

use crate::audio::models;

#[derive(serde::Deserialize)]
struct WakeFile {
    words: std::collections::BTreeMap<String, Vec<String>>,
}

fn pretokenized() -> &'static WakeFile {
    static W: OnceLock<WakeFile> = OnceLock::new();
    W.get_or_init(|| serde_json::from_str(include_str!("../../resources/wakewords.json")).unwrap_or(WakeFile { words: Default::default() }))
}

/// Greedy longest-match BPE tokenisation over the model vocabulary (for custom words).
pub fn tokenize(text: &str, vocab: &HashSet<String>) -> Option<Vec<String>> {
    let mut out = Vec::new();
    for word in text.split_whitespace() {
        let w: Vec<char> = format!("▁{}", word.to_uppercase()).chars().collect();
        let mut i = 0;
        while i < w.len() {
            let mut found = None;
            for j in (i + 1..=w.len()).rev() {
                let piece: String = w[i..j].iter().collect();
                if vocab.contains(&piece) {
                    found = Some((piece, j));
                    break;
                }
            }
            let (piece, j) = found?;
            out.push(piece);
            i = j;
        }
    }
    Some(out)
}

pub fn keyword_line(word: &str, vocab: &HashSet<String>) -> Option<String> {
    let w = word.trim().to_uppercase();
    let tokens = pretokenized().words.get(&w).cloned().or_else(|| tokenize(&w, vocab))?;
    Some(format!("{} @{}", tokens.join(" "), w.replace(' ', "_")))
}

pub struct WakeWord {
    kws: sherpa_onnx::KeywordSpotter,
    stream: sherpa_onnx::OnlineStream,
}

impl WakeWord {
    pub fn new(words: &[String], sensitivity: f32) -> Option<Self> {
        if !models::kws_installed() {
            tracing::info!("wake word model missing");
            return None;
        }
        let f = models::kws_files();
        let vocab: HashSet<String> = std::fs::read_to_string(&f.tokens).ok()?.lines().filter_map(|l| l.split_whitespace().next().map(String::from)).collect();
        let lines: Vec<String> = words.iter().filter_map(|w| keyword_line(w, &vocab)).collect();
        if lines.is_empty() {
            return None;
        }
        let cfg = sherpa_onnx::KeywordSpotterConfig {
            model_config: sherpa_onnx::OnlineModelConfig {
                transducer: sherpa_onnx::OnlineTransducerModelConfig {
                    encoder: Some(f.encoder.to_string_lossy().into_owned()),
                    decoder: Some(f.decoder.to_string_lossy().into_owned()),
                    joiner: Some(f.joiner.to_string_lossy().into_owned()),
                },
                tokens: Some(f.tokens.to_string_lossy().into_owned()),
                num_threads: 1,
                provider: Some("cpu".into()),
                ..Default::default()
            },
            // Sensitivity 0..1 → lower threshold = more sensitive.
            keywords_threshold: (0.45 - 0.35 * sensitivity.clamp(0.0, 1.0)).clamp(0.05, 0.6),
            keywords_score: 1.5,
            keywords_buf: Some(lines.join("\n")),
            ..Default::default()
        };
        let kws = sherpa_onnx::KeywordSpotter::create(&cfg)?;
        let stream = kws.create_stream();
        Some(Self { kws, stream })
    }

    pub fn accept(&mut self, samples16k: &[f32]) {
        self.stream.accept_waveform(16_000, samples16k);
    }

    /// Returns the detected keyword, if any.
    pub fn poll(&mut self) -> Option<String> {
        while self.kws.is_ready(&self.stream) {
            self.kws.decode(&self.stream);
            if let Some(r) = self.kws.get_result(&self.stream) {
                if !r.keyword.is_empty() {
                    self.kws.reset(&self.stream);
                    return Some(r.keyword.replace('_', " "));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretokenized_words_used() {
        let vocab = HashSet::new();
        assert_eq!(keyword_line("radio", &vocab).unwrap(), "▁RA DI O @RADIO");
        assert_eq!(keyword_line("Hey Crew", &vocab).unwrap(), "▁HE Y ▁C RE W @HEY_CREW");
    }

    #[test]
    fn greedy_custom() {
        let vocab: HashSet<String> = ["▁GO", "▁HOME", "▁HO", "ME"].iter().map(|s| s.to_string()).collect();
        assert_eq!(tokenize("go home", &vocab).unwrap(), vec!["▁GO", "▁HOME"]);
        assert!(tokenize("xyz", &vocab).is_none());
    }
}
