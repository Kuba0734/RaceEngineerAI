//! Voice input pipeline: mic → 16 kHz → (PTT | wake word | VAD) → speech segments.
//! Runs on its own thread; segments go to the async router for STT.

pub mod capture;
pub mod fastpath;
pub mod stt_groq;
pub mod wakeword;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::Sender;
use parking_lot::RwLock;

use crate::audio::{mixer::MixerStatus, models};
use crate::storage::settings::Settings;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ListenState {
    Off,
    Idle,
    WakeWindow,
    Recording,
    Muted,
}

#[derive(Default)]
pub struct VoiceStatus {
    pub level: AtomicU32,
    pub state: AtomicU32,
    pub ptt: AtomicBool,
    /// Set by the runtime when listening is allowed (game running or setting off).
    pub enabled: AtomicBool,
}

impl VoiceStatus {
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }
    pub fn state(&self) -> ListenState {
        match self.state.load(Ordering::Relaxed) {
            1 => ListenState::Idle,
            2 => ListenState::WakeWindow,
            3 => ListenState::Recording,
            4 => ListenState::Muted,
            _ => ListenState::Off,
        }
    }
    fn set_state(&self, s: ListenState) {
        let v = match s {
            ListenState::Off => 0,
            ListenState::Idle => 1,
            ListenState::WakeWindow => 2,
            ListenState::Recording => 3,
            ListenState::Muted => 4,
        };
        self.state.store(v, Ordering::Relaxed);
    }
}

/// A finished utterance (16 kHz mono).
pub struct Segment {
    pub samples: Vec<f32>,
    pub via: &'static str,
}

fn make_vad(s: &Settings) -> Option<sherpa_onnx::VoiceActivityDetector> {
    let path = models::vad_path();
    if !path.is_file() {
        return None;
    }
    let l = &s.listening;
    let cfg = sherpa_onnx::VadModelConfig {
        silero_vad: sherpa_onnx::SileroVadModelConfig {
            model: Some(path.to_string_lossy().into_owned()),
            threshold: l.vad_threshold.clamp(0.1, 0.95),
            min_silence_duration: l.end_silence_s.clamp(0.3, 2.0),
            min_speech_duration: l.min_speech_s.max(0.1),
            window_size: 512,
            max_speech_duration: l.max_speech_s.clamp(1.0, 30.0),
        },
        sample_rate: 16_000,
        num_threads: 1,
        provider: Some("cpu".into()),
        debug: false,
        ..Default::default()
    };
    sherpa_onnx::VoiceActivityDetector::create(&cfg, 30.0)
}

pub fn start(settings: Arc<RwLock<Settings>>, mixer: Arc<MixerStatus>, status: Arc<VoiceStatus>, out: Sender<Segment>, reload: crossbeam_channel::Receiver<()>) {
    let spawned = std::thread::Builder::new().name("voice".into()).spawn(move || loop {
        let device = settings.read().listening.input_device.clone();
        let cap = match capture::Capture::start(device) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("microphone unavailable: {e}");
                status.set_state(ListenState::Off);
                if reload.recv_timeout(Duration::from_secs(10)).is_err() {
                    continue;
                }
                continue;
            }
        };
        let snapshot = settings.read().clone();
        let vad = make_vad(&snapshot);
        let mut kws = if snapshot.listening.wake_word { wakeword::WakeWord::new(&snapshot.listening.wake_words, snapshot.listening.wake_sensitivity) } else { None };
        if vad.is_none() {
            tracing::info!("VAD model missing – only push-to-talk works");
        }
        let mut rs = capture::Resampler16k::new(cap.sample_rate);
        let mut buf16 = Vec::with_capacity(4096);
        let mut ptt_buf: Vec<f32> = Vec::new();
        let mut was_ptt = false;
        let mut wake_until: Option<Instant> = None;
        let mut level = 0.0f32;
        let gate = 10f32.powf(snapshot.listening.noise_gate_db / 20.0);
        loop {
            if reload.try_recv().is_ok() {
                break;
            }
            let Ok(chunk) = cap.rx.recv_timeout(Duration::from_millis(200)) else { continue };
            buf16.clear();
            rs.process(&chunk, &mut buf16);
            let peak = buf16.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            level = level * 0.7 + peak * 0.3;
            status.level.store(level.to_bits(), Ordering::Relaxed);

            if !status.enabled.load(Ordering::Relaxed) {
                status.set_state(ListenState::Off);
                ptt_buf.clear();
                continue;
            }
            let l = &snapshot.listening;
            // Push-to-talk: record everything while held (works during crew speech, too).
            let ptt = status.ptt.load(Ordering::Relaxed) && l.push_to_talk;
            if ptt {
                status.set_state(ListenState::Recording);
                ptt_buf.extend_from_slice(&buf16);
                was_ptt = true;
                continue;
            }
            if was_ptt {
                was_ptt = false;
                let n = ptt_buf.len() as f32 / 16_000.0;
                if n >= l.min_speech_s && n <= l.max_speech_s.max(1.0) {
                    let _ = out.try_send(Segment { samples: std::mem::take(&mut ptt_buf), via: "ptt" });
                }
                ptt_buf.clear();
            }
            // Half duplex.
            if mixer.any_speaking() && !l.allow_interrupt {
                status.set_state(ListenState::Muted);
                if let Some(v) = &vad {
                    v.clear();
                }
                continue;
            }
            let gated: &[f32] = if peak < gate { &[] } else { &buf16 };
            if let Some(k) = kws.as_mut() {
                // Feed silence-gated audio as zeros to keep the stream continuous.
                if gated.is_empty() {
                    k.accept(&vec![0.0; buf16.len()]);
                } else {
                    k.accept(gated);
                }
                if let Some(word) = k.poll() {
                    tracing::info!("wake word: {word}");
                    wake_until = Some(Instant::now() + Duration::from_secs_f32(l.wake_window_s.max(1.0)));
                    if let Some(v) = &vad {
                        // The wake word itself is part of the current segment; drop it.
                        v.clear();
                    }
                }
            }
            let in_window = wake_until.is_some_and(|t| Instant::now() < t);
            status.set_state(if in_window { ListenState::WakeWindow } else { ListenState::Idle });
            let Some(v) = &vad else { continue };
            v.accept_waveform(&buf16);
            if v.detected() && (in_window || l.vad_always) {
                status.set_state(ListenState::Recording);
            }
            while !v.is_empty() {
                if let Some(seg) = v.front() {
                    let samples = seg.samples().to_vec();
                    let secs = samples.len() as f32 / 16_000.0;
                    let accept = (l.vad_always || in_window) && secs >= l.min_speech_s;
                    if accept {
                        wake_until = None;
                        let _ = out.try_send(Segment { samples, via: if in_window { "wake" } else { "vad" } });
                    }
                }
                v.pop();
            }
        }
    });
    if let Err(e) = spawned {
        tracing::error!("voice thread: {e}");
    }
}
