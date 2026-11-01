//! ACC shared memory pages, field-for-field from the official
//! "ACC Shared Memory Documentation v1.8.12" (Kunos). See `docs/acc.md`.
//!
//! Layout: `#pragma pack(4)` in the original header → `#[repr(C, packed(4))]`.
//! Strings are `wchar_t[N]` (UTF-16) → `[u16; N]`. Enums are 4-byte ints.

#![allow(non_snake_case, dead_code)]

#[repr(C, packed(4))]
#[derive(Clone, Copy)]
pub struct SPageFilePhysics {
    pub packetId: i32,
    pub gas: f32,
    pub brake: f32,
    pub fuel: f32,
    pub gear: i32,
    pub rpm: i32,
    pub steerAngle: f32,
    pub speedKmh: f32,
    pub velocity: [f32; 3],
    pub accG: [f32; 3],
    pub wheelSlip: [f32; 4],
    pub wheelLoad: [f32; 4],
    pub wheelPressure: [f32; 4],
    pub wheelAngularSpeed: [f32; 4],
    pub tyreWear: [f32; 4],
    pub tyreDirtyLevel: [f32; 4],
    pub tyreCoreTemp: [f32; 4],
    pub camberRAD: [f32; 4],
    pub suspensionTravel: [f32; 4],
    pub drs: f32,
    pub tc: f32,
    pub heading: f32,
    pub pitch: f32,
    pub roll: f32,
    pub cgHeight: f32,
    pub carDamage: [f32; 5],
    pub numberOfTyresOut: i32,
    pub pitLimiterOn: i32,
    pub abs: f32,
    pub kersCharge: f32,
    pub kersInput: f32,
    pub autoshifterOn: i32,
    pub rideHeight: [f32; 2],
    pub turboBoost: f32,
    pub ballast: f32,
    pub airDensity: f32,
    pub airTemp: f32,
    pub roadTemp: f32,
    pub localAngularVel: [f32; 3],
    pub finalFF: f32,
    pub performanceMeter: f32,
    pub engineBrake: i32,
    pub ersRecoveryLevel: i32,
    pub ersPowerLevel: i32,
    pub ersHeatCharging: i32,
    pub ersIsCharging: i32,
    pub kersCurrentKJ: f32,
    pub drsAvailable: i32,
    pub drsEnabled: i32,
    pub brakeTemp: [f32; 4],
    pub clutch: f32,
    pub tyreTempI: [f32; 4],
    pub tyreTempM: [f32; 4],
    pub tyreTempO: [f32; 4],
    pub isAIControlled: i32,
    pub tyreContactPoint: [[f32; 3]; 4],
    pub tyreContactNormal: [[f32; 3]; 4],
    pub tyreContactHeading: [[f32; 3]; 4],
    pub brakeBias: f32,
    pub localVelocity: [f32; 3],
    pub P2PActivation: i32,
    pub P2PStatus: i32,
    pub currentMaxRpm: f32,
    pub mz: [f32; 4],
    pub fx: [f32; 4],
    pub fy: [f32; 4],
    pub slipRatio: [f32; 4],
    pub slipAngle: [f32; 4],
    pub tcinAction: i32,
    pub absInAction: i32,
    pub suspensionDamage: [f32; 4],
    pub tyreTemp: [f32; 4],
    pub waterTemp: f32,
    pub brakePressure: [f32; 4],
    pub frontBrakeCompound: i32,
    pub rearBrakeCompound: i32,
    pub padLife: [f32; 4],
    pub discLife: [f32; 4],
    pub ignitionOn: i32,
    pub starterEngineOn: i32,
    pub isEngineRunning: i32,
    pub kerbVibration: f32,
    pub slipVibrations: f32,
    pub gVibrations: f32,
    pub absVibrations: f32,
}

#[repr(C, packed(4))]
#[derive(Clone, Copy)]
pub struct SPageFileGraphic {
    pub packetId: i32,
    pub status: i32,
    pub session: i32,
    pub currentTime: [u16; 15],
    pub lastTime: [u16; 15],
    pub bestTime: [u16; 15],
    pub split: [u16; 15],
    pub completedLaps: i32,
    pub position: i32,
    pub iCurrentTime: i32,
    pub iLastTime: i32,
    pub iBestTime: i32,
    pub sessionTimeLeft: f32,
    pub distanceTraveled: f32,
    pub isInPit: i32,
    pub currentSectorIndex: i32,
    pub lastSectorTime: i32,
    pub numberOfLaps: i32,
    pub tyreCompound: [u16; 33],
    pub replayTimeMultiplier: f32,
    pub normalizedCarPosition: f32,
    pub activeCars: i32,
    pub carCoordinates: [[f32; 3]; 60],
    pub carID: [i32; 60],
    pub playerCarID: i32,
    pub penaltyTime: f32,
    pub flag: i32,
    pub penalty: i32,
    pub idealLineOn: i32,
    pub isInPitLane: i32,
    pub surfaceGrip: f32,
    pub mandatoryPitDone: i32,
    pub windSpeed: f32,
    pub windDirection: f32,
    pub isSetupMenuVisible: i32,
    pub mainDisplayIndex: i32,
    pub secondaryDisplayIndex: i32,
    pub TC: i32,
    pub TCCut: i32,
    pub EngineMap: i32,
    pub ABS: i32,
    pub fuelXLap: f32,
    pub rainLights: i32,
    pub flashingLights: i32,
    pub lightsStage: i32,
    pub exhaustTemperature: f32,
    pub wiperLV: i32,
    pub driverStintTotalTimeLeft: i32,
    pub driverStintTimeLeft: i32,
    pub rainTyres: i32,
    pub sessionIndex: i32,
    pub usedFuel: f32,
    pub deltaLapTime: [u16; 15],
    pub iDeltaLapTime: i32,
    pub estimatedLapTime: [u16; 15],
    pub iEstimatedLapTime: i32,
    pub isDeltaPositive: i32,
    pub iSplit: i32,
    pub isValidLap: i32,
    pub fuelEstimatedLaps: f32,
    pub trackStatus: [u16; 33],
    pub missingMandatoryPits: i32,
    pub Clock: f32,
    pub directionLightsLeft: i32,
    pub directionLightsRight: i32,
    pub GlobalYellow: i32,
    pub GlobalYellow1: i32,
    pub GlobalYellow2: i32,
    pub GlobalYellow3: i32,
    pub GlobalWhite: i32,
    pub GlobalGreen: i32,
    pub GlobalChequered: i32,
    pub GlobalRed: i32,
    pub mfdTyreSet: i32,
    pub mfdFuelToAdd: f32,
    pub mfdTyrePressureLF: f32,
    pub mfdTyrePressureRF: f32,
    pub mfdTyrePressureLR: f32,
    pub mfdTyrePressureRR: f32,
    pub trackGripStatus: i32,
    pub rainIntensity: i32,
    pub rainIntensityIn10min: i32,
    pub rainIntensityIn30min: i32,
    pub currentTyreSet: i32,
    pub strategyTyreSet: i32,
    pub gapAhead: i32,
    pub gapBehind: i32,
}

#[repr(C, packed(4))]
#[derive(Clone, Copy)]
pub struct SPageFileStatic {
    pub smVersion: [u16; 15],
    pub acVersion: [u16; 15],
    pub numberOfSessions: i32,
    pub numCars: i32,
    pub carModel: [u16; 33],
    pub track: [u16; 33],
    pub playerName: [u16; 33],
    pub playerSurname: [u16; 33],
    pub playerNick: [u16; 33],
    pub sectorCount: i32,
    pub maxTorque: f32,
    pub maxPower: f32,
    pub maxRpm: i32,
    pub maxFuel: f32,
    pub suspensionMaxTravel: [f32; 4],
    pub tyreRadius: [f32; 4],
    pub maxTurboBoost: f32,
    pub deprecated_1: f32,
    pub deprecated_2: f32,
    pub penaltiesEnabled: i32,
    pub aidFuelRate: f32,
    pub aidTireRate: f32,
    pub aidMechanicalDamage: f32,
    pub allowTyreBlankets: f32,
    pub aidStability: f32,
    pub aidAutoclutch: i32,
    pub aidAutoBlip: i32,
    pub hasDRS: i32,
    pub hasERS: i32,
    pub hasKERS: i32,
    pub kersMaxJ: f32,
    pub engineBrakeSettingsCount: i32,
    pub ersPowerControllerCount: i32,
    pub trackSplineLength: f32,
    /// PDF omits the length; `[33]` per the original AC header. See docs/acc.md.
    pub trackConfiguration: [u16; 33],
    pub ersMaxJ: f32,
    pub isTimedRace: i32,
    pub hasExtraLap: i32,
    pub carSkin: [u16; 33],
    pub reversedGridPositions: i32,
    pub PitWindowStart: i32,
    pub PitWindowEnd: i32,
    pub isOnline: i32,
    pub dryTyresName: [u16; 33],
    pub wetTyresName: [u16; 33],
}

pub fn wstr(s: &[u16]) -> String {
    let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    String::from_utf16_lossy(&s[..end])
}

pub const PHYSICS_NAME: &str = "Local\\acpmf_physics";
pub const GRAPHICS_NAME: &str = "Local\\acpmf_graphics";
pub const STATIC_NAME: &str = "Local\\acpmf_static";

/// Read-only view of a named file mapping.
#[cfg(windows)]
pub struct MappedPage<T: Copy> {
    handle: windows::Win32::Foundation::HANDLE,
    view: windows::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS,
    _t: std::marker::PhantomData<T>,
}

// SAFETY: the view is read-only memory owned by this struct; reads copy the data out.
#[cfg(windows)]
unsafe impl<T: Copy> Send for MappedPage<T> {}

#[cfg(windows)]
impl<T: Copy> MappedPage<T> {
    /// Opens an existing mapping created by the game. Fails if the game has not created it.
    pub fn open(name: &str) -> anyhow::Result<Self> {
        use windows::core::HSTRING;
        use windows::Win32::System::Memory::{MapViewOfFile, OpenFileMappingW, FILE_MAP_READ};
        let wname = HSTRING::from(name);
        // SAFETY: plain WinAPI calls with valid arguments; handle/view are released in Drop.
        unsafe {
            let handle = OpenFileMappingW(FILE_MAP_READ.0, false, &wname)?;
            let view = MapViewOfFile(handle, FILE_MAP_READ, 0, 0, std::mem::size_of::<T>());
            if view.Value.is_null() {
                let _ = windows::Win32::Foundation::CloseHandle(handle);
                anyhow::bail!("MapViewOfFile failed for {name}");
            }
            Ok(Self { handle, view, _t: std::marker::PhantomData })
        }
    }

    /// Copies the page out of shared memory (unaligned read; the game writes concurrently,
    /// callers detect torn frames via `packetId`).
    pub fn read(&self) -> T {
        // SAFETY: the view is at least size_of::<T>() bytes (requested in MapViewOfFile)
        // and T is plain-old-data (only integers/floats/arrays).
        unsafe { std::ptr::read_unaligned(self.view.Value as *const T) }
    }

    /// Reads only the leading `packetId` (i32 at offset 0) cheaply.
    pub fn packet_id(&self) -> i32 {
        // SAFETY: offset 0 is inside the mapped view.
        unsafe { std::ptr::read_volatile(self.view.Value as *const i32) }
    }
}

#[cfg(windows)]
impl<T: Copy> Drop for MappedPage<T> {
    fn drop(&mut self) {
        // SAFETY: view and handle were obtained in `open` and are released exactly once.
        unsafe {
            let _ = windows::Win32::System::Memory::UnmapViewOfFile(self.view);
            let _ = windows::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    // Offsets derived by hand from the field order in the v1.8.12 PDF with 4-byte packing.
    #[test]
    fn physics_layout() {
        assert_eq!(offset_of!(SPageFilePhysics, fuel), 12);
        assert_eq!(offset_of!(SPageFilePhysics, wheelPressure), 88);
        assert_eq!(offset_of!(SPageFilePhysics, tyreCoreTemp), 152);
        assert_eq!(offset_of!(SPageFilePhysics, heading), 208);
        assert_eq!(offset_of!(SPageFilePhysics, carDamage), 224);
        assert_eq!(offset_of!(SPageFilePhysics, roadTemp), 292);
        assert_eq!(offset_of!(SPageFilePhysics, brakeTemp), 348);
        assert_eq!(offset_of!(SPageFilePhysics, tyreContactPoint), 420);
        assert_eq!(offset_of!(SPageFilePhysics, slipRatio), 640);
        assert_eq!(offset_of!(SPageFilePhysics, tcinAction), 672);
        assert_eq!(offset_of!(SPageFilePhysics, padLife), 740);
        assert_eq!(offset_of!(SPageFilePhysics, absVibrations), 796);
        assert_eq!(size_of::<SPageFilePhysics>(), 800);
    }

    #[test]
    fn graphics_layout() {
        assert_eq!(offset_of!(SPageFileGraphic, currentTime), 12);
        assert_eq!(offset_of!(SPageFileGraphic, completedLaps), 132);
        assert_eq!(offset_of!(SPageFileGraphic, tyreCompound), 176);
        assert_eq!(offset_of!(SPageFileGraphic, replayTimeMultiplier), 244);
        assert_eq!(offset_of!(SPageFileGraphic, carCoordinates), 256);
        assert_eq!(offset_of!(SPageFileGraphic, carID), 976);
        assert_eq!(offset_of!(SPageFileGraphic, playerCarID), 1216);
        assert_eq!(offset_of!(SPageFileGraphic, fuelXLap), 1284);
        assert_eq!(offset_of!(SPageFileGraphic, iDeltaLapTime), 1360);
        assert_eq!(offset_of!(SPageFileGraphic, iEstimatedLapTime), 1396);
        assert_eq!(offset_of!(SPageFileGraphic, missingMandatoryPits), 1484);
        assert_eq!(offset_of!(SPageFileGraphic, trackGripStatus), 1556);
        assert_eq!(offset_of!(SPageFileGraphic, gapBehind), 1584);
        assert_eq!(size_of::<SPageFileGraphic>(), 1588);
    }

    #[test]
    fn static_layout() {
        assert_eq!(offset_of!(SPageFileStatic, numberOfSessions), 60);
        assert_eq!(offset_of!(SPageFileStatic, carModel), 68);
        assert_eq!(offset_of!(SPageFileStatic, sectorCount), 400);
        assert_eq!(offset_of!(SPageFileStatic, maxFuel), 416);
        assert_eq!(offset_of!(SPageFileStatic, trackConfiguration), 524);
        assert_eq!(offset_of!(SPageFileStatic, ersMaxJ), 592);
        assert_eq!(offset_of!(SPageFileStatic, PitWindowStart), 676);
        assert_eq!(offset_of!(SPageFileStatic, isOnline), 684);
        assert_eq!(size_of::<SPageFileStatic>(), 820);
    }

    #[test]
    fn wide_strings() {
        let mut buf = [0u16; 15];
        for (i, c) in "1:47.912".encode_utf16().enumerate() {
            buf[i] = c;
        }
        assert_eq!(wstr(&buf), "1:47.912");
    }
}
