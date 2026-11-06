//! Shared application core: settings, race state, call engine, audio, voice, Groq.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use ts_rs::TS;

use crate::ai::groq::Groq;
use crate::ai::memory::Conversation;
use crate::audio::mixer::{MixerCmd, MixerStatus};
use crate::audio::tts::{SpeakRequest, TtsHandle, TtsJob};
use crate::calls::engine::{CallEngine, CallRequest, RadioLogEntry, Utterance};
use crate::games::registry::GameDef;
use crate::race::state::RaceState;
use crate::storage::db::DbHandle;
use crate::storage::settings::Settings;
use crate::voice::VoiceStatus;

#[derive(Debug, Clone, PartialEq)]
pub enum DemoSource {
    Mock,
    Replay(PathBuf),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RuntimeStatus {
    pub running_games: Vec<String>,
    pub active_game: Option<String>,
    pub connected: bool,
    pub secondary_connected: bool,
    pub live: bool,
    pub demo: bool,
}

pub struct Core {
    pub app: AppHandle,
    pub settings: Arc<RwLock<Settings>>,
    pub state: Arc<Mutex<RaceState>>,
    pub engine: Arc<Mutex<CallEngine>>,
    pub tts: TtsHandle,
    pub mixer: crossbeam_channel::Sender<MixerCmd>,
    pub mixer_status: Arc<MixerStatus>,
    pub voice: Arc<VoiceStatus>,
    pub voice_reload: crossbeam_channel::Sender<()>,
    pub groq: Groq,
    pub conversation: Mutex<Conversation>,
    pub games: Vec<GameDef>,
    pub status: RwLock<RuntimeStatus>,
    pub demo: Mutex<Option<DemoSource>>,
    pub out_sr: u32,
    pub wheel_learn: Arc<Mutex<bool>>,
    pub db: DbHandle,
    pub ai_busy: AtomicBool,
    pub live_subs: Mutex<Vec<crate::live::LiveSub>>,
    pub overlay_edit: AtomicBool,
    pub downloads: Mutex<BTreeSet<String>>,
    /// Test mode: record the running session now.
    pub record_request: AtomicBool,
    /// File of the recording in progress.
    pub recording: Mutex<Option<String>>,
    /// Last sample of the data rates (time, frames, cars) for the data check.
    pub rate_sample: Mutex<Option<(std::time::Instant, u64, u64)>>,
}

impl Core {
    pub fn speak(&self, u: Utterance) {
        self.emit_radio_last();
        self.tts.send(TtsJob::Speak(SpeakRequest { text: u.text, role: u.role, priority: u.priority, expires_at: u.expires_at, call_id: u.call_id, excited: u.excited }));
    }

    /// Submits a call (on request) through the engine and speaks it.
    pub fn say(&self, mut req: CallRequest) {
        req.on_request = true;
        // Command acknowledgement: spoken (default), beep or nothing.
        if req.call_id == "crew.ack" {
            match self.settings.read().commands.acknowledge {
                crate::storage::settings::Acknowledge::Spoken => {}
                crate::storage::settings::Acknowledge::Beep => {
                    let _ = self.mixer.try_send(MixerCmd::Spotter(crate::audio::tts::tone_clip("ack", self.out_sr)));
                    return;
                }
                crate::storage::settings::Acknowledge::None => return,
            }
        }
        let u = {
            let st = self.state.lock();
            let (t, lap, spline, kind) = (st.t_s(), st.frame.completed_laps, st.frame.spline, st.session.session_type.kind());
            drop(st);
            self.engine.lock().submit(req, t, lap, spline, kind)
        };
        if let Some(u) = u {
            self.speak(u);
        }
    }

    pub fn answer_text(&self, text: String) {
        let t = self.state.lock().t_s();
        let u = self.engine.lock().answer(t, text);
        self.speak(u);
    }

    pub fn emit_radio_last(&self) {
        let last = self.engine.lock().log.back().cloned();
        if let Some(e) = last {
            let _ = self.app.emit("radio", &e);
        }
    }

    pub fn emit_radio(&self, e: &RadioLogEntry) {
        let _ = self.app.emit("radio", e);
    }

    pub fn save_settings(&self) {
        let s = self.settings.read().clone();
        if let Err(e) = crate::storage::settings::save(&crate::storage::paths::settings_path(), &s) {
            tracing::error!("saving settings: {e}");
        }
        let _ = self.mixer.try_send(MixerCmd::SetMasterVolume(s.voices.master_volume));
        let _ = self.mixer.try_send(MixerCmd::SetDucking(s.spotter.ducking));
        let _ = self.mixer.try_send(MixerCmd::SetPauseOnSpotter(s.spotter.pause_crew));
        let _ = self.app.emit("settings", &s);
    }

    /// Push-to-talk state with a radio key-up / squelch sound on change.
    pub fn set_ptt(&self, pressed: bool) {
        let before = self.voice.ptt.swap(pressed, std::sync::atomic::Ordering::Relaxed);
        let (on, style) = {
            let s = self.settings.read();
            (s.listening.ptt_sound, (s.listening.ptt_sound_style, s.listening.ptt_sound_volume))
        };
        if before != pressed && on {
            let _ = self.mixer.try_send(MixerCmd::Spotter(crate::audio::radio_fx::ptt_clip_vol(pressed, style.0, self.out_sr, style.1).into()));
        }
    }

    /// Loads the profile of another game and applies everything that depends on it.
    pub fn select_game(&self, id: &str) {
        let s = {
            let mut w = self.settings.write();
            if w.selected_game == id {
                return;
            }
            w.switch_profile(id);
            w.clone()
        };
        tracing::info!("profile: {id}");
        self.engine.lock().refresh(&s);
        self.tts.send(TtsJob::Reload);
        self.tts.send(TtsJob::RenderSpotter(self.engine.lock().spotter_phrases()));
        crate::input::hotkeys::register_all(&self.app, &s);
        self.save_settings();
    }

    pub fn demo_source(&self) -> Option<DemoSource> {
        self.demo.lock().clone()
    }
}
