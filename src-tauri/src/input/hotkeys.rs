//! Keyboard hotkeys (global shortcuts) and dispatch of wheel buttons:
//! push-to-talk, overlay edit mode, quick commands.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::core::Core;
use crate::storage::settings::Settings;
use crate::voice::fastpath::{self, FastAnswer, Intent};

/// Quick command ids that can be bound to buttons.
pub const QUICK_COMMANDS: [&str; 10] = ["repeat", "quiet", "normal", "spotter_only", "fuel", "gap_ahead", "gap_behind", "time_left", "louder", "quieter"];

pub fn run_quick_command(core: &Core, id: &str) {
    let intent = match id {
        "repeat" => Intent::Repeat,
        "quiet" => Intent::Quiet,
        "normal" => Intent::Normal,
        "spotter_only" => Intent::SpotterOnly,
        "fuel" => Intent::Fuel,
        "gap_ahead" => Intent::GapAhead,
        "gap_behind" => Intent::GapBehind,
        "time_left" => Intent::TimeLeft,
        "louder" => Intent::Louder,
        "quieter" => Intent::Quieter,
        _ => return,
    };
    let ans = {
        let st = core.state.lock();
        let mut settings = core.settings.write();
        let mut engine = core.engine.lock();
        fastpath::answer(intent, "", &st, &mut settings, &mut engine)
    };
    if matches!(intent, Intent::Louder | Intent::Quieter) {
        core.save_settings();
    }
    match ans {
        Some(FastAnswer::Call(r)) | Some(FastAnswer::Ack(r)) => core.say(r),
        Some(FastAnswer::Text(t)) => core.answer_text(t),
        None => {}
    }
}

/// Creates the overlay window on demand (saves a renderer + GPU memory while no session runs).
pub fn ensure_overlay(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    if let Some(w) = app.get_webview_window("overlay") {
        return Some(w);
    }
    let built = tauri::WebviewWindowBuilder::new(app, "overlay", tauri::WebviewUrl::App("overlay.html".into()))
        .title("RaceEngineerAI Overlay")
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .shadow(false)
        .visible(false)
        .maximized(true)
        .build();
    match built {
        Ok(w) => {
            let _ = w.set_ignore_cursor_events(true);
            Some(w)
        }
        Err(e) => {
            tracing::error!("overlay window: {e}");
            None
        }
    }
}

pub fn close_overlay(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.destroy();
    }
}

pub fn set_overlay_edit(app: &AppHandle, core: &Core, edit: bool) {
    core.overlay_edit.store(edit, Ordering::Relaxed);
    let win = if edit { ensure_overlay(app) } else { app.get_webview_window("overlay") };
    if let Some(w) = win {
        let _ = w.set_ignore_cursor_events(!edit);
        if edit {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
    let _ = app.emit("overlay-edit", edit);
}

fn key_of(binding: &str) -> Option<&str> {
    binding.strip_prefix("key:")
}

/// (Re-)registers all keyboard bindings from the settings.
pub fn register_all(app: &AppHandle, s: &Settings) {
    let gs = app.global_shortcut();
    if let Err(e) = gs.unregister_all() {
        tracing::warn!("unregister shortcuts: {e}");
    }
    let edit = s.overlay.edit_hotkey.clone();
    if !edit.is_empty() {
        let r = gs.on_shortcut(edit.as_str(), |app, _, ev| {
            if ev.state() == ShortcutState::Pressed {
                if let Some(core) = app.try_state::<Arc<Core>>() {
                    let now = !core.overlay_edit.load(Ordering::Relaxed);
                    set_overlay_edit(app, &core, now);
                }
            }
        });
        if let Err(e) = r {
            tracing::warn!("overlay hotkey {edit}: {e}");
        }
    }
    if let Some(k) = key_of(&s.input.push_to_talk.binding) {
        let r = gs.on_shortcut(k, |app, _, ev| {
            if let Some(core) = app.try_state::<Arc<Core>>() {
                core.voice.ptt.store(ev.state() == ShortcutState::Pressed, Ordering::Relaxed);
            }
        });
        if let Err(e) = r {
            tracing::warn!("ptt hotkey {k}: {e}");
        }
    }
    for (cmd, b) in &s.input.quick_commands {
        let Some(k) = key_of(&b.binding) else { continue };
        let cmd = cmd.clone();
        let r = gs.on_shortcut(k, move |app, _, ev| {
            if ev.state() == ShortcutState::Pressed {
                if let Some(core) = app.try_state::<Arc<Core>>() {
                    run_quick_command(&core, &cmd);
                }
            }
        });
        if let Err(e) = r {
            tracing::warn!("quick command hotkey {k}: {e}");
        }
    }
}

/// Dispatches wheel button events (own thread).
pub fn start_wheel_dispatch(core: Arc<Core>, rx: crossbeam_channel::Receiver<super::wheel::ButtonEvent>) {
    let spawned = std::thread::Builder::new().name("wheel-dispatch".into()).spawn(move || {
        for ev in rx {
            let (binding, pressed) = match &ev {
                super::wheel::ButtonEvent::Pressed(b) => (b.clone(), true),
                super::wheel::ButtonEvent::Released(b) => (b.clone(), false),
            };
            // "Press a button to bind": the UI listens for this event.
            if pressed {
                let _ = core.app.emit("wheel-button", &binding);
            }
            let s = core.settings.read().clone();
            if s.input.push_to_talk.binding == binding {
                core.voice.ptt.store(pressed, Ordering::Relaxed);
                continue;
            }
            if pressed {
                if let Some((cmd, _)) = s.input.quick_commands.iter().find(|(_, b)| b.binding == binding) {
                    run_quick_command(&core, cmd);
                }
            }
        }
    });
    if let Err(e) = spawned {
        tracing::error!("wheel dispatch: {e}");
    }
}
