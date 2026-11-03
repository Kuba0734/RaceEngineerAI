//! Remote control of the sim through the SDK broadcast message (`irsdk_broadcastMsg`):
//! pit menu and disk telemetry. Message layout as in the SDK: a registered window message
//! `IRSDK_BROADCASTMSG` sent to all windows with `wParam = type | var1 << 16`, `lParam = var2`
//! (reference implementation: pyirsdk `_broadcast_msg`; enums: `irsdk_defines.h`).

use crate::games::PitRequest;

/// `irsdk_BroadcastMsg`
const BROADCAST_PIT_COMMAND: u32 = 9;
const BROADCAST_TELEM_COMMAND: u32 = 10;

/// `irsdk_PitCommandMode` (works only while the driver is in the car).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PitCommand {
    Clear = 0,
    Windshield = 1,
    Fuel = 2,
    LeftFront = 3,
    RightFront = 4,
    LeftRear = 5,
    RightRear = 6,
    ClearTires = 7,
    FastRepair = 8,
    ClearWindshield = 9,
    ClearFastRepair = 10,
    ClearFuel = 11,
}

/// The commands for a request, in the order they are sent.
pub fn pit_commands(r: &PitRequest) -> Vec<(PitCommand, i32)> {
    use PitCommand::*;
    let mut out = Vec::new();
    if r.clear {
        out.push((Clear, 0));
    }
    if let Some(l) = r.fuel_l {
        // Litres to add; the sim rounds to whole litres.
        out.push(if l > 0.0 { (Fuel, l.round() as i32) } else { (ClearFuel, 0) });
    }
    if let Some(t) = r.tyres {
        out.push((ClearTires, 0));
        for (on, c) in t.iter().zip([LeftFront, RightFront, LeftRear, RightRear]) {
            if *on {
                // 0 = keep the pressure set in the garage.
                out.push((c, 0));
            }
        }
    }
    if let Some(fr) = r.fast_repair {
        out.push((if fr { FastRepair } else { ClearFastRepair }, 0));
    }
    if let Some(ws) = r.windshield {
        out.push((if ws { Windshield } else { ClearWindshield }, 0));
    }
    out
}

#[cfg(windows)]
fn send(msg: u32, var1: u32, var2: i32) -> anyhow::Result<()> {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{RegisterWindowMessageW, SendNotifyMessageW, HWND_BROADCAST};
    // SAFETY: plain WinAPI calls with valid arguments; no memory is shared.
    unsafe {
        let id = RegisterWindowMessageW(&HSTRING::from("IRSDK_BROADCASTMSG"));
        if id == 0 {
            anyhow::bail!("RegisterWindowMessage failed");
        }
        SendNotifyMessageW(HWND_BROADCAST, id, WPARAM((msg | (var1 << 16)) as usize), LPARAM(var2 as isize))?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn send(_msg: u32, _var1: u32, _var2: i32) -> anyhow::Result<()> {
    anyhow::bail!("only on Windows")
}

pub fn apply_pit(r: &PitRequest) -> anyhow::Result<()> {
    for (cmd, value) in pit_commands(r) {
        send(BROADCAST_PIT_COMMAND, cmd as u32, value)?;
    }
    Ok(())
}

/// Starts or stops writing .ibt telemetry (`irsdk_TelemCommand_Start` / `_Stop`).
pub fn disk_telemetry(on: bool) -> anyhow::Result<()> {
    send(BROADCAST_TELEM_COMMAND, u32::from(on), 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuel_and_front_tyres() {
        let r = PitRequest { fuel_l: Some(39.6), tyres: Some([true, true, false, false]), ..Default::default() };
        assert_eq!(pit_commands(&r), vec![(PitCommand::Fuel, 40), (PitCommand::ClearTires, 0), (PitCommand::LeftFront, 0), (PitCommand::RightFront, 0)]);
        let none = PitRequest { fuel_l: Some(0.0), tyres: Some([false; 4]), fast_repair: Some(true), ..Default::default() };
        assert_eq!(pit_commands(&none), vec![(PitCommand::ClearFuel, 0), (PitCommand::ClearTires, 0), (PitCommand::FastRepair, 0)]);
    }
}
