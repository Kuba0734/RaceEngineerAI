//! Keyboard hotkeys (global shortcuts) and dispatch of wheel buttons:
//! push-to-talk, overlay edit mode, quick commands.

use std::sync::atomic::{AtomicBool, Ordering};
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

static OVERLAY_CREATING: AtomicBool = AtomicBool::new(false);

/// Builds the transparent, click-through overlay covering the chosen monitor.
fn build_overlay(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    let wanted = app.try_state::<Arc<Core>>().and_then(|c| c.settings.read().overlay.monitor.clone());
    let monitor = wanted
        .and_then(|name| app.available_monitors().ok()?.into_iter().find(|m| m.name().is_some_and(|n| *n == name)))
        .or_else(|| app.primary_monitor().ok().flatten());
    let mut b = tauri::WebviewWindowBuilder::new(app, "overlay", tauri::WebviewUrl::App("overlay.html".into()))
        .title("RaceEngineerAI Overlay")
        .transparent(true)
        .background_color(tauri::window::Color(0, 0, 0, 0))
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .shadow(false)
        .visible(false);
    if let Some(m) = monitor {
        let sf = m.scale_factor();
        let (p, sz) = (m.position(), m.size());
        b = b.position(f64::from(p.x) / sf, f64::from(p.y) / sf).inner_size(f64::from(sz.width) / sf, f64::from(sz.height) / sf);
    }
    match b.build() {
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

/// Runs `f` with the overlay window, creating it first if needed. Creation always happens on a
/// helper thread: on Windows, building a webview inside a sync command or event handler deadlocks.
pub fn with_overlay(app: &AppHandle, f: impl FnOnce(tauri::WebviewWindow) + Send + 'static) {
    if let Some(w) = app.get_webview_window("overlay") {
        f(w);
        return;
    }
    if OVERLAY_CREATING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new().name("overlay-create".into()).spawn(move || {
        let w = build_overlay(&app);
        OVERLAY_CREATING.store(false, Ordering::SeqCst);
        if let Some(w) = w {
            f(w);
        }
    });
    if spawned.is_err() {
        OVERLAY_CREATING.store(false, Ordering::SeqCst);
    }
}

pub fn close_overlay(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.destroy();
    }
}

pub fn set_overlay_edit(app: &AppHandle, core: &Core, edit: bool) {
    core.overlay_edit.store(edit, Ordering::Relaxed);
    let _ = app.emit("overlay-edit", edit);
    if edit {
        let a = app.clone();
        with_overlay(app, move |w| {
            let _ = w.set_ignore_cursor_events(false);
            let _ = w.show();
            let _ = w.set_focus();
            // Tell a freshly created window that it is in edit mode.
            let _ = a.emit("overlay-edit", true);
        });
    } else if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.set_ignore_cursor_events(true);
    }
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
                core.set_ptt(ev.state() == ShortcutState::Pressed);
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
                core.set_ptt(pressed);
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
