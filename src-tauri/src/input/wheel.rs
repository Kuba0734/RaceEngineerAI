//! Wheel / button box buttons via Windows.Gaming.Input RawGameController (polling, 100 Hz),
//! gilrs as fallback when WGI is unavailable. Bindings look like `wheel:<vid>:<pid>:<button>`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::Sender;
use parking_lot::Mutex;

#[derive(Debug, Clone)]
pub enum ButtonEvent {
    Pressed(String),
    Released(String),
}

pub fn binding(vid: u16, pid: u16, button: usize) -> String {
    format!("wheel:{vid:04x}:{pid:04x}:{button}")
}

#[cfg(windows)]
fn run_wgi(tx: &Sender<ButtonEvent>, learn: &Arc<Mutex<bool>>) -> windows::core::Result<()> {
    use windows::Gaming::Input::{GameControllerSwitchPosition, RawGameController};
    struct Dev {
        c: RawGameController,
        vid: u16,
        pid: u16,
        buttons: Vec<bool>,
        prev: Vec<bool>,
        switches: Vec<GameControllerSwitchPosition>,
        axes: Vec<f64>,
    }
    let mut devs: Vec<Dev> = Vec::new();
    let mut last_enum = Instant::now() - Duration::from_secs(10);
    // binding → last time seen pressed. Wheels often expose several HID collections with the
    // same vendor/product id; a binding is held while ANY of them reports it pressed.
    let mut held: std::collections::HashMap<String, Instant> = std::collections::HashMap::new();
    loop {
        if last_enum.elapsed() > Duration::from_secs(2) {
            last_enum = Instant::now();
            let list = RawGameController::RawGameControllers()?;
            let n = list.Size()?;
            let mut next = Vec::new();
            for i in 0..n {
                let c = list.GetAt(i)?;
                let (vid, pid) = (c.HardwareVendorId()?, c.HardwareProductId()?);
                let nb = c.ButtonCount()?.max(0) as usize;
                let ns = c.SwitchCount()?.max(0) as usize;
                let na = c.AxisCount()?.max(0) as usize;
                let prev = devs.iter().find(|d| d.vid == vid && d.pid == pid).map(|d| d.prev.clone()).unwrap_or_else(|| vec![false; nb]);
                next.push(Dev { c, vid, pid, buttons: vec![false; nb], prev, switches: vec![GameControllerSwitchPosition::Center; ns], axes: vec![0.0; na] });
            }
            devs = next;
        }
        let now = Instant::now();
        for d in &mut devs {
            // On a failed read the previous state is kept (no false release).
            let _ = d.c.GetCurrentReading(&mut d.buttons, &mut d.switches, &mut d.axes);
            for (i, pressed) in d.buttons.iter().enumerate() {
                if *pressed {
                    let b = binding(d.vid, d.pid, i);
                    if !held.contains_key(&b) {
                        *learn.lock() = false;
                        let _ = tx.try_send(ButtonEvent::Pressed(b.clone()));
                    }
                    held.insert(b, now);
                }
            }
            d.prev.clone_from(&d.buttons);
        }
        // Release only after 60 ms without any device reporting the button (debounce).
        held.retain(|b, seen| {
            let keep = now.duration_since(*seen) < Duration::from_millis(60);
            if !keep {
                let _ = tx.try_send(ButtonEvent::Released(b.clone()));
            }
            keep
        });
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn run_gilrs(tx: &Sender<ButtonEvent>) {
    let Ok(mut g) = gilrs::Gilrs::new() else {
        tracing::warn!("no gamepad backend available");
        return;
    };
    loop {
        while let Some(ev) = g.next_event_blocking(Some(Duration::from_millis(500))) {
            let pad = g.gamepad(ev.id);
            let (vid, pid) = (pad.vendor_id().unwrap_or(0), pad.product_id().unwrap_or(0));
            match ev.event {
                gilrs::EventType::ButtonPressed(_, code) => {
                    let _ = tx.try_send(ButtonEvent::Pressed(binding(vid, pid, code.into_u32() as usize)));
                }
                gilrs::EventType::ButtonReleased(_, code) => {
                    let _ = tx.try_send(ButtonEvent::Released(binding(vid, pid, code.into_u32() as usize)));
                }
                _ => {}
            }
        }
    }
}

/// Starts the input thread. `learn` is reset on the next press (used by "press a button to bind").
pub fn start(tx: Sender<ButtonEvent>, learn: Arc<Mutex<bool>>) {
    let spawned = std::thread::Builder::new().name("wheel".into()).spawn(move || {
        #[cfg(windows)]
        {
            if let Err(e) = run_wgi(&tx, &learn) {
                tracing::warn!("Windows.Gaming.Input failed ({e}), falling back to gilrs");
            } else {
                return;
            }
        }
        let _ = &learn;
        run_gilrs(&tx);
    });
    if let Err(e) = spawned {
        tracing::error!("wheel thread: {e}");
    }
}
