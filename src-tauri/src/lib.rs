pub mod ai;
pub mod audio;
pub mod calls;
pub mod commands;
pub mod core;
pub mod error;
pub mod games;
pub mod input;
pub mod race;
pub mod replay;
pub mod runtime;
pub mod storage;
pub mod voice;

use std::collections::BTreeSet;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, OnceLock};

use parking_lot::{Mutex, RwLock};
use tauri::Manager;

use crate::core::Core;
use crate::storage::{paths, settings};

static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

fn init_logging() {
    use tracing_subscriber::prelude::*;
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .max_log_files(7)
        .filename_prefix("raceengineerai")
        .filename_suffix("log")
        .build(paths::logs_dir());
    let filter = tracing_subscriber::EnvFilter::try_from_env("RACEENGINEER_LOG").unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,race_engineer_ai_lib=info"));
    let registry = tracing_subscriber::registry().with(filter).with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr));
    match appender {
        Ok(a) => {
            let (nb, guard) = tracing_appender::non_blocking(a);
            let _ = LOG_GUARD.set(guard);
            registry.with(tracing_subscriber::fmt::layer().with_ansi(false).with_writer(nb)).init();
        }
        Err(_) => registry.init(),
    }
}

pub fn run() {
    let _ = paths::ensure_dirs();
    init_logging();
    secrets::init_store();
    tracing::info!("RaceEngineerAI {} starting", env!("CARGO_PKG_VERSION"));

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let settings = settings::load(&paths::settings_path());
            let shared_settings = Arc::new(RwLock::new(settings.clone()));
            // Audio out + TTS.
            let audio = match audio::mixer::AudioOut::start(settings.voices.output_device.clone()) {
                Ok(a) => a,
                Err(e) => {
                    tracing::error!("audio output: {e}; retrying with default device");
                    audio::mixer::AudioOut::start(None)?
                }
            };
            let tts = audio::tts::start(shared_settings.clone(), audio.tx.clone(), audio.sample_rate);
            let engine = calls::engine::CallEngine::new(&settings);
            tts.send(audio::tts::TtsJob::RenderSpotter(engine.spotter_phrases()));
            let voice_status = Arc::new(voice::VoiceStatus::default());
            let (voice_reload_tx, voice_reload_rx) = crossbeam_channel::bounded(4);
            let core = Arc::new(Core {
                app: app.handle().clone(),
                settings: shared_settings.clone(),
                state: Arc::new(Mutex::new(race::state::RaceState::new())),
                engine: Arc::new(Mutex::new(engine)),
                tts,
                mixer: audio.tx.clone(),
                mixer_status: audio.status.clone(),
                voice: voice_status.clone(),
                voice_reload: voice_reload_tx,
                groq: ai::groq::Groq::new(),
                conversation: Mutex::new(ai::memory::Conversation::default()),
                games: games::registry::load(),
                status: RwLock::new(core::RuntimeStatus::default()),
                demo: Mutex::new(None),
                out_sr: audio.sample_rate,
                wheel_learn: Arc::new(Mutex::new(false)),
                db: storage::db::DbHandle::start(paths::db_path()),
                ai_busy: AtomicBool::new(false),
                live_subs: Mutex::new(Vec::new()),
                overlay_edit: AtomicBool::new(false),
                downloads: Mutex::new(BTreeSet::new()),
            });
            let _ = core.mixer.try_send(audio::mixer::MixerCmd::SetMasterVolume(settings.voices.master_volume));
            let _ = core.mixer.try_send(audio::mixer::MixerCmd::SetDucking(settings.spotter.ducking));
            // Keep the output stream alive for the whole app lifetime.
            app.manage(Arc::new(Mutex::new(Some(audio))));
            app.manage(core.clone());

            core.db.send(storage::db::DbJob::Prune { days: settings.storage.retention_days });

            // Voice in → router.
            let (seg_tx, seg_rx) = crossbeam_channel::bounded::<voice::Segment>(8);
            voice::start(shared_settings.clone(), core.mixer_status.clone(), voice_status, seg_tx, voice_reload_rx);
            let rc = core.clone();
            std::thread::Builder::new().name("segments".into()).spawn(move || {
                for seg in seg_rx {
                    let c = rc.clone();
                    tauri::async_runtime::spawn(ai::router::handle_segment(c, seg));
                }
            })?;

            // Wheel buttons + keyboard hotkeys.
            let (btn_tx, btn_rx) = crossbeam_channel::bounded(64);
            input::wheel::start(btn_tx, core.wheel_learn.clone());
            input::hotkeys::start_wheel_dispatch(core.clone(), btn_rx);
            input::hotkeys::register_all(app.handle(), &settings);

            // The overlay window is created on demand by the runtime when a session goes live.
            if settings.app.start_minimized || std::env::args().any(|a| a == "--minimized") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.minimize();
                }
            }
            runtime::start(core);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::reset_settings,
            commands::get_catalog,
            commands::preview_call,
            commands::preview_voice,
            commands::get_games,
            commands::get_status,
            commands::acc_broadcasting_status,
            commands::acc_broadcasting_apply,
            commands::api_key_status,
            commands::api_key_set,
            commands::api_key_test,
            commands::groq_limits,
            commands::models_status,
            commands::model_install,
            commands::model_remove,
            commands::audio_devices,
            commands::start_demo,
            commands::stop_demo,
            commands::list_replays,
            commands::start_replay,
            commands::ask,
            commands::subscribe_live,
            commands::unsubscribe_live,
            commands::get_track_outline,
            commands::get_radio_log,
            commands::say_call,
            commands::list_sessions,
            commands::session_laps,
            commands::delete_session,
            commands::wheel_learn,
            commands::ptt,
            commands::set_overlay_edit,
            commands::save_overlay_layout,
            commands::show_overlay,
            commands::app_info,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| tracing::error!("tauri: {e}"));
}

use storage::secrets;
