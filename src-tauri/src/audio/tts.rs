//! Local speech synthesis with Piper (VITS) voices through sherpa-onnx, on its own thread.
//! Spotter phrases are pre-rendered into RAM so spotter calls never wait for synthesis.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crossbeam_channel::{Receiver, Sender};
use parking_lot::RwLock;

use super::mixer::{MixerCmd, Pcm, SpeechItem};
use super::{models, radio_fx};
use crate::storage::settings::{Language, Priority, Settings, VoiceProfile, VoiceRole};

pub struct SpeakRequest {
    pub text: String,
    pub role: VoiceRole,
    pub priority: Priority,
    pub expires_at: Instant,
    pub call_id: String,
}

pub enum TtsJob {
    Speak(SpeakRequest),
    /// (call_id, phrase variants) for the spotter cache.
    RenderSpotter(Vec<(String, Vec<String>)>),
    /// Render one phrase for preview and play it now.
    Preview { text: String, role: VoiceRole },
    /// Drop loaded voices (after settings or model changes).
    Reload,
}

/// call_id → rendered variants.
pub type SpotterClips = Arc<RwLock<HashMap<String, Vec<Pcm>>>>;

#[derive(Clone)]
pub struct TtsHandle {
    tx: Sender<TtsJob>,
    pub spotter_clips: SpotterClips,
    next_id: Arc<AtomicU64>,
}

impl TtsHandle {
    pub fn send(&self, job: TtsJob) {
        if self.tx.try_send(job).is_err() {
            tracing::warn!("tts queue full");
        }
    }
    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }
}

struct Synth {
    engines: HashMap<String, sherpa_onnx::OfflineTts>,
    pause_ms: u32,
}

/// Splits after . ! ? (not inside numbers like 1.5).
fn sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = text.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        cur.push(*c);
        let end = matches!(c, '.' | '!' | '?') && chars.get(i + 1).is_none_or(|n| n.is_whitespace());
        if end && cur.trim().chars().count() > 3 {
            out.push(cur.trim().to_string());
            cur.clear();
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

impl Synth {
    fn engine(&mut self, voice_id: &str) -> Option<&sherpa_onnx::OfflineTts> {
        if !self.engines.contains_key(voice_id) {
            let v = models::find_voice(voice_id)?;
            if !models::voice_installed(&v) {
                return None;
            }
            let f = models::voice_files(&v);
            let path = |p: &std::path::Path| Some(p.to_string_lossy().into_owned());
            let model = if v.is_kokoro() {
                sherpa_onnx::OfflineTtsModelConfig {
                    kokoro: sherpa_onnx::OfflineTtsKokoroModelConfig {
                        model: path(&f.model),
                        voices: path(&f.voices),
                        tokens: path(&f.tokens),
                        data_dir: path(&f.data_dir),
                        lexicon: Some(f.lexicon.clone()),
                        length_scale: 1.0,
                        ..Default::default()
                    },
                    num_threads: 2,
                    ..Default::default()
                }
            } else {
                sherpa_onnx::OfflineTtsModelConfig {
                    vits: sherpa_onnx::OfflineTtsVitsModelConfig { model: path(&f.model), tokens: path(&f.tokens), data_dir: path(&f.data_dir), ..Default::default() },
                    num_threads: 2,
                    ..Default::default()
                }
            };
            let config = sherpa_onnx::OfflineTtsConfig {
                model,
                max_num_sentences: 1,
                ..Default::default()
            };
            let tts = sherpa_onnx::OfflineTts::create(&config)?;
            tracing::info!("voice loaded: {voice_id}");
            self.engines.insert(voice_id.to_string(), tts);
        }
        self.engines.get(voice_id)
    }

    /// Renders text → mono PCM at `out_sr`, with volume and radio effect applied.
    fn render(&mut self, text: &str, p: &VoiceProfile, lang: Language, out_sr: u32) -> Option<Pcm> {
        let voice = match lang {
            Language::De => &p.voice_de,
            Language::En => &p.voice_en,
        };
        let text = speakable(text, lang);
        let text = text.as_str();
        let pause_ms = self.pause_ms;
        let tts = self.engine(voice)?;
        // Slightly longer pauses at commas, plus a short varied gap between sentences.
        let cfg = sherpa_onnx::GenerationConfig { speed: p.speed.clamp(0.5, 2.0), sid: p.speaker_id, silence_scale: 0.35, ..Default::default() };
        let mut samples: Vec<f32> = Vec::new();
        let mut sr = 22_050u32;
        for (i, s) in sentences(text).iter().enumerate() {
            let audio = tts.generate_with_config(s, &cfg, None::<fn(&[f32], f32) -> bool>)?;
            sr = audio.sample_rate() as u32;
            if i > 0 && pause_ms > 0 {
                let jitter = rand::random_range(0.8..1.25);
                samples.extend(std::iter::repeat_n(0.0, (sr as f32 * pause_ms as f32 / 1000.0 * jitter) as usize));
            }
            samples.extend_from_slice(audio.samples());
        }
        if samples.is_empty() {
            return None;
        }
        let fx = radio_fx::apply(&samples, sr, &p.radio_fx);
        let vol = p.volume.clamp(0.0, 1.5);
        let out: Vec<f32> = radio_fx::resample(&fx, sr, out_sr).into_iter().map(|s| (s * vol).clamp(-1.0, 1.0)).collect();
        Some(out.into())
    }
}

/// Turns symbols and abbreviations into words before synthesis ("29 °C" → "29 Grad").
pub fn speakable(text: &str, lang: Language) -> String {
    let de = lang == Language::De;
    let pairs: [(&str, &str); 12] = [
        ("°C", if de { " Grad" } else { " degrees" }),
        ("° C", if de { " Grad" } else { " degrees" }),
        ("°F", if de { " Grad Fahrenheit" } else { " degrees Fahrenheit" }),
        ("°", if de { " Grad" } else { " degrees" }),
        ("km/h", if de { " Kilometer pro Stunde" } else { " kilometres per hour" }),
        ("mph", " miles per hour"),
        (" psi", " P S I"),
        (" s.", if de { " Sekunden." } else { " seconds." }),
        ("%", if de { " Prozent" } else { " percent" }),
        (" L/", if de { " Liter pro " } else { " litres per " }),
        (" l/", if de { " Liter pro " } else { " litres per " }),
        ("±", if de { "plus minus " } else { "plus or minus " }),
    ];
    let mut out = speak_times(text, de);
    for (a, b) in pairs {
        out = out.replace(a, b);
    }
    // Symbols the voice would read out literally (markdown, semicolons, brackets, dashes).
    for (a, b) in [(";", ","), (":", ","), ("–", ","), ("—", ","), ("(", ", "), (")", ", "), ("*", ""), ("#", ""), ("_", " "), ("`", ""), ("\n", ". "), ("|", ","), ("~", "")] {
        out = out.replace(a, b);
    }
    let out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    out.replace(" ,", ",").replace(",,", ",").replace(", .", ".").trim_matches([',', ' ']).to_string()
}

/// Lap times like 1:47,9 or 1:05.3 the way an engineer says them, so the voice never reads
/// the colon out loud: "1 47 point 9", "1 oh 5 point 3", "58 point 3".
fn speak_times(text: &str, de: bool) -> String {
    let c: Vec<char> = text.chars().collect();
    let digits_from = |i: usize| c[i..].iter().take_while(|x| x.is_ascii_digit()).count();
    let mut out = String::with_capacity(text.len() + 16);
    let mut i = 0;
    while i < c.len() {
        let starts_number = c[i].is_ascii_digit() && (i == 0 || !(c[i - 1].is_ascii_digit() || matches!(c[i - 1], ':' | '.' | ',')));
        if starts_number {
            let minutes = digits_from(i);
            let colon = i + minutes;
            if minutes <= 2 && c.get(colon) == Some(&':') && digits_from(colon + 1) == 2 {
                let sec_at = colon + 1;
                let mut end = sec_at + 2;
                let m: String = c[i..colon].iter().collect();
                let (s1, s2) = (c[sec_at], c[sec_at + 1]);
                if m.chars().any(|d| d != '0') {
                    out.push_str(&m);
                    out.push(' ');
                }
                if s1 == '0' {
                    out.push_str(if de { "null " } else { "oh " });
                    out.push(s2);
                } else {
                    out.push(s1);
                    out.push(s2);
                }
                if matches!(c.get(end), Some('.' | ',')) && digits_from(end + 1) > 0 {
                    let n = digits_from(end + 1);
                    out.push_str(if de { " Komma " } else { " point " });
                    out.extend(&c[end + 1..end + 1 + n]);
                    end += 1 + n;
                }
                i = end;
                continue;
            }
        }
        out.push(c[i]);
        i += 1;
    }
    out
}

/// Fallback tones when no voice is installed: the spotter still works.
pub fn tone_clip(call_id: &str, sr: u32) -> Pcm {
    let beep = |f: f32, ms: f32, out: &mut Vec<f32>| {
        let n = (sr as f32 * ms / 1000.0) as usize;
        for i in 0..n {
            let env = (i.min(n - i) as f32 / (sr as f32 * 0.005)).min(1.0);
            out.push((i as f32 * f * std::f32::consts::TAU / sr as f32).sin() * 0.35 * env);
        }
        out.extend(std::iter::repeat_n(0.0, (sr as f32 * 0.04) as usize));
    };
    let mut v = Vec::new();
    match call_id {
        "spotter.left" => {
            beep(520.0, 90.0, &mut v);
            beep(520.0, 90.0, &mut v);
        }
        "spotter.right" => {
            beep(880.0, 90.0, &mut v);
            beep(880.0, 90.0, &mut v);
        }
        "spotter.three_wide" => {
            beep(520.0, 80.0, &mut v);
            beep(700.0, 80.0, &mut v);
            beep(880.0, 80.0, &mut v);
        }
        "spotter.clear" => {
            beep(660.0, 70.0, &mut v);
            beep(990.0, 110.0, &mut v);
        }
        _ => beep(600.0, 120.0, &mut v),
    }
    v.into()
}

pub fn start(settings: Arc<RwLock<Settings>>, mixer: Sender<MixerCmd>, out_sr: u32) -> TtsHandle {
    let (tx, rx): (Sender<TtsJob>, Receiver<TtsJob>) = crossbeam_channel::bounded(64);
    let clips: SpotterClips = Arc::new(RwLock::new(HashMap::new()));
    let handle = TtsHandle { tx, spotter_clips: clips.clone(), next_id: Arc::new(AtomicU64::new(1)) };
    let next_id = handle.next_id.clone();
    let spawned = std::thread::Builder::new().name("tts".into()).spawn(move || {
        let mut synth = Synth { engines: HashMap::new(), pause_ms: 260 };
        for job in rx {
            synth.pause_ms = settings.read().voices.sentence_pause_ms;
            match job {
                TtsJob::Speak(req) => {
                    if req.expires_at <= Instant::now() {
                        continue;
                    }
                    let (profile, lang) = {
                        let s = settings.read();
                        (match req.role {
                            VoiceRole::Engineer => s.voices.engineer.clone(),
                            VoiceRole::Spotter => s.voices.spotter.clone(),
                        }, s.language)
                    };
                    let Some(pcm) = synth.render(&req.text, &profile, lang, out_sr) else {
                        tracing::debug!("no voice for {} – skipped: {}", req.call_id, req.text);
                        continue;
                    };
                    if req.expires_at <= Instant::now() {
                        continue;
                    }
                    let id = next_id.fetch_add(1, Ordering::Relaxed);
                    let _ = mixer.try_send(MixerCmd::Speech(SpeechItem { id, pcm, priority: req.priority, expires_at: req.expires_at }));
                }
                TtsJob::RenderSpotter(list) => {
                    let (profile, lang) = {
                        let s = settings.read();
                        (s.voices.spotter.clone(), s.language)
                    };
                    let mut map = HashMap::new();
                    for (id, texts) in list {
                        let v: Vec<Pcm> = texts.iter().filter_map(|t| synth.render(t, &profile, lang, out_sr)).collect();
                        let v = if v.is_empty() { vec![tone_clip(&id, out_sr)] } else { v };
                        map.insert(id, v);
                    }
                    *clips.write() = map;
                    tracing::info!("spotter clips rendered");
                }
                TtsJob::Preview { text, role } => {
                    let (profile, lang) = {
                        let s = settings.read();
                        (match role {
                            VoiceRole::Engineer => s.voices.engineer.clone(),
                            VoiceRole::Spotter => s.voices.spotter.clone(),
                        }, s.language)
                    };
                    let pcm = synth.render(&text, &profile, lang, out_sr).unwrap_or_else(|| tone_clip("preview", out_sr));
                    let id = next_id.fetch_add(1, Ordering::Relaxed);
                    let _ = mixer.try_send(MixerCmd::Speech(SpeechItem { id, pcm, priority: Priority::Answer, expires_at: Instant::now() + std::time::Duration::from_secs(30) }));
                }
                TtsJob::Reload => synth.engines.clear(),
            }
        }
    });
    if let Err(e) = spawned {
        tracing::error!("tts thread: {e}");
    }
    handle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_sentences_not_numbers() {
        assert_eq!(sentences("Gap 1.4 behind. Fuel is fine! Ok"), vec!["Gap 1.4 behind.", "Fuel is fine!", "Ok"]);
    }

    #[test]
    fn times_are_spoken_without_colon() {
        assert_eq!(speakable("Best lap 1:47,9.", Language::En), "Best lap 1 47 point 9.");
        assert_eq!(speakable("Beste Runde 1:47,9.", Language::De), "Beste Runde 1 47 Komma 9.");
        assert_eq!(speakable("Last 1:05.3 now", Language::En), "Last 1 oh 5 point 3 now");
        assert_eq!(speakable("Runde 1:05,3", Language::De), "Runde 1 null 5 Komma 3");
        assert_eq!(speakable("0:58.3", Language::En), "58 point 3");
        assert_eq!(speakable("Kurve 3: zu früh gebremst", Language::De), "Kurve 3, zu früh gebremst");
        // Ordinary numbers stay untouched.
        assert_eq!(speakable("Gap 1.4 behind, 12 laps", Language::En), "Gap 1.4 behind, 12 laps");
    }

    #[test]
    fn units_become_words() {
        assert_eq!(speakable("Strecke 29 °C, Luft 21°C.", Language::De), "Strecke 29 Grad, Luft 21 Grad.");
        assert_eq!(speakable("Track 29°C", Language::En), "Track 29 degrees");
        assert_eq!(speakable("Reifen ok; vorne **warm** (85 Grad)", Language::De), "Reifen ok, vorne warm, 85 Grad");
    }
}
