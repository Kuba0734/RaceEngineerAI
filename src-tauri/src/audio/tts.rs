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
}

impl Synth {
    fn engine(&mut self, voice_id: &str) -> Option<&sherpa_onnx::OfflineTts> {
        if !self.engines.contains_key(voice_id) {
            let v = models::voice(voice_id)?;
            if !models::voice_installed(v) {
                return None;
            }
            let f = models::voice_files(v);
            let config = sherpa_onnx::OfflineTtsConfig {
                model: sherpa_onnx::OfflineTtsModelConfig {
                    vits: sherpa_onnx::OfflineTtsVitsModelConfig {
                        model: Some(f.model.to_string_lossy().into_owned()),
                        tokens: Some(f.tokens.to_string_lossy().into_owned()),
                        data_dir: Some(f.data_dir.to_string_lossy().into_owned()),
                        ..Default::default()
                    },
                    num_threads: 2,
                    ..Default::default()
                },
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
        let tts = self.engine(voice)?;
        let cfg = sherpa_onnx::GenerationConfig { speed: p.speed.clamp(0.5, 2.0), sid: p.speaker_id, ..Default::default() };
        let audio = tts.generate_with_config(text, &cfg, None::<fn(&[f32], f32) -> bool>)?;
        let sr = audio.sample_rate() as u32;
        let fx = radio_fx::apply(audio.samples(), sr, &p.radio_fx);
        let vol = p.volume.clamp(0.0, 1.5);
        let out: Vec<f32> = radio_fx::resample(&fx, sr, out_sr).into_iter().map(|s| (s * vol).clamp(-1.0, 1.0)).collect();
        Some(out.into())
    }
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
        let mut synth = Synth { engines: HashMap::new() };
        for job in rx {
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
