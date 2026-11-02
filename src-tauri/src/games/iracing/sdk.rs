//! iRacing SDK shared memory, field-for-field from the official `irsdk_defines.h`
//! (iRacing.com Motorsport Simulations). See `docs/iracing.md`.
//!
//! Layout: plain C structs of 4-byte ints (natural alignment) → `#[repr(C)]`.
//! Variables are looked up by name in the var header table; their offsets index into one
//! "line" of the triple-buffered data.

use std::collections::HashMap;

pub const MEMMAP_NAME: &str = "Local\\IRSDKMemMapFileName";
pub const IRSDK_VER: i32 = 2;
pub const MAX_BUFS: usize = 4;
pub const MAX_STRING: usize = 32;
pub const MAX_DESC: usize = 64;
/// `irsdk_stConnected`
pub const ST_CONNECTED: i32 = 1;
/// Markers for "unlimited" session laps and time.
pub const UNLIMITED_LAPS: i32 = 32767;
pub const UNLIMITED_TIME_S: f64 = 604_800.0;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VarBuf {
    pub tick_count: i32,
    pub buf_offset: i32,
    pub pad: [i32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Header {
    pub ver: i32,
    pub status: i32,
    pub tick_rate: i32,
    pub session_info_update: i32,
    pub session_info_len: i32,
    pub session_info_offset: i32,
    pub num_vars: i32,
    pub var_header_offset: i32,
    pub num_buf: i32,
    pub buf_len: i32,
    pub pad1: [i32; 2],
    pub var_buf: [VarBuf; MAX_BUFS],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VarHeader {
    pub ty: i32,
    pub offset: i32,
    pub count: i32,
    pub count_as_time: u8,
    pub pad: [u8; 3],
    pub name: [u8; MAX_STRING],
    pub desc: [u8; MAX_DESC],
    pub unit: [u8; MAX_STRING],
}

pub const HEADER_SIZE: usize = std::mem::size_of::<Header>();
pub const VAR_HEADER_SIZE: usize = std::mem::size_of::<VarHeader>();

/// `irsdk_VarType`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarType {
    Char,
    Bool,
    Int,
    BitField,
    Float,
    Double,
}

impl VarType {
    fn from_i32(v: i32) -> Option<Self> {
        Some(match v {
            0 => VarType::Char,
            1 => VarType::Bool,
            2 => VarType::Int,
            3 => VarType::BitField,
            4 => VarType::Float,
            5 => VarType::Double,
            _ => return None,
        })
    }

    pub fn bytes(self) -> usize {
        match self {
            VarType::Char | VarType::Bool => 1,
            VarType::Int | VarType::BitField | VarType::Float => 4,
            VarType::Double => 8,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Var {
    pub ty: VarType,
    pub offset: usize,
    pub count: usize,
}

/// Read access to the shared memory (the real mapping or a byte buffer in tests).
pub trait Memory {
    fn size(&self) -> usize;
    /// Copies `out.len()` bytes from `offset`. Returns false when out of range.
    fn read(&self, offset: usize, out: &mut [u8]) -> bool;
}

impl Memory for Vec<u8> {
    fn size(&self) -> usize {
        self.as_slice().len()
    }

    fn read(&self, offset: usize, out: &mut [u8]) -> bool {
        match self.get(offset..offset + out.len()) {
            Some(src) => {
                out.copy_from_slice(src);
                true
            }
            None => false,
        }
    }
}

fn i32_at(b: &[u8], o: usize) -> i32 {
    b.get(o..o + 4).map(|s| i32::from_le_bytes([s[0], s[1], s[2], s[3]])).unwrap_or(0)
}

fn cstr(b: &[u8]) -> String {
    let end = b.iter().position(|c| *c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

pub fn read_header(mem: &impl Memory) -> Option<Header> {
    let mut b = [0u8; HEADER_SIZE];
    if !mem.read(0, &mut b) {
        return None;
    }
    let mut var_buf = [VarBuf::default(); MAX_BUFS];
    for (i, v) in var_buf.iter_mut().enumerate() {
        let o = 48 + i * 16;
        *v = VarBuf { tick_count: i32_at(&b, o), buf_offset: i32_at(&b, o + 4), pad: [0; 2] };
    }
    Some(Header {
        ver: i32_at(&b, 0),
        status: i32_at(&b, 4),
        tick_rate: i32_at(&b, 8),
        session_info_update: i32_at(&b, 12),
        session_info_len: i32_at(&b, 16),
        session_info_offset: i32_at(&b, 20),
        num_vars: i32_at(&b, 24),
        var_header_offset: i32_at(&b, 28),
        num_buf: i32_at(&b, 32),
        buf_len: i32_at(&b, 36),
        pad1: [0; 2],
        var_buf,
    })
}

/// Name → variable for the current var header table.
pub fn read_vars(mem: &impl Memory, h: &Header) -> HashMap<String, Var> {
    let mut vars = HashMap::new();
    let n = h.num_vars.clamp(0, 4096) as usize;
    let base = h.var_header_offset.max(0) as usize;
    let mut b = [0u8; VAR_HEADER_SIZE];
    for i in 0..n {
        if !mem.read(base + i * VAR_HEADER_SIZE, &mut b) {
            break;
        }
        let Some(ty) = VarType::from_i32(i32_at(&b, 0)) else { continue };
        let (offset, count) = (i32_at(&b, 4), i32_at(&b, 8));
        if offset < 0 || count <= 0 {
            continue;
        }
        let name = cstr(&b[16..16 + MAX_STRING]);
        vars.insert(name, Var { ty, offset: offset as usize, count: count as usize });
    }
    vars
}

/// Index of the buffer with the newest data (like `irsdk_getNewData`).
pub fn latest_buf(h: &Header) -> Option<usize> {
    let n = (h.num_buf.clamp(0, MAX_BUFS as i32)) as usize;
    (0..n).max_by_key(|i| h.var_buf[*i].tick_count)
}

/// Session info YAML. The sim writes ISO-8859-1; every byte maps to the same code point.
pub fn read_session_yaml(mem: &impl Memory, h: &Header) -> Option<String> {
    let len = h.session_info_len.clamp(0, 8 * 1024 * 1024) as usize;
    let mut b = vec![0u8; len];
    if !mem.read(h.session_info_offset.max(0) as usize, &mut b) {
        return None;
    }
    let end = b.iter().position(|c| *c == 0).unwrap_or(b.len());
    Some(b[..end].iter().map(|c| char::from(*c)).collect())
}

/// One copied line of variables.
#[derive(Default)]
pub struct Row {
    pub data: Vec<u8>,
    pub tick: i32,
}

impl Row {
    fn raw(&self, v: &Var, i: usize) -> Option<&[u8]> {
        if i >= v.count {
            return None;
        }
        let o = v.offset + i * v.ty.bytes();
        self.data.get(o..o + v.ty.bytes())
    }

    /// Any numeric type as f64.
    pub fn num(&self, v: &Var, i: usize) -> Option<f64> {
        let b = self.raw(v, i)?;
        Some(match v.ty {
            VarType::Char => f64::from(b[0]),
            VarType::Bool => f64::from(u8::from(b[0] != 0)),
            VarType::Int => f64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            VarType::BitField => f64::from(u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            VarType::Float => f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            VarType::Double => f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]),
        })
    }

    /// Bitfields as raw bits (no float rounding).
    pub fn bits(&self, v: &Var, i: usize) -> Option<u32> {
        let b = self.raw(v, i)?;
        match v.ty {
            VarType::Int | VarType::BitField => Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            _ => self.num(v, i).map(|x| x as u32),
        }
    }
}

/// Copies the newest line. Re-reads once if the sim overwrote it while we copied
/// (its tick count changed), as the SDK does.
pub fn read_row(mem: &impl Memory, row: &mut Row) -> Option<Header> {
    for _ in 0..2 {
        let h = read_header(mem)?;
        let i = latest_buf(&h)?;
        let len = h.buf_len.clamp(0, 1024 * 1024) as usize;
        row.data.resize(len, 0);
        if !mem.read(h.var_buf[i].buf_offset.max(0) as usize, &mut row.data) {
            return None;
        }
        let after = read_header(mem)?;
        if after.var_buf[i].tick_count == h.var_buf[i].tick_count {
            row.tick = h.var_buf[i].tick_count;
            return Some(h);
        }
    }
    None
}

/// The live mapping created by iRacing.
#[cfg(windows)]
pub struct Mapping {
    handle: windows::Win32::Foundation::HANDLE,
    view: windows::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS,
    len: usize,
}

// SAFETY: the mapping is read-only and only accessed through `read`, which copies.
#[cfg(windows)]
unsafe impl Send for Mapping {}

#[cfg(windows)]
impl Mapping {
    /// Opens the mapping. Fails while iRacing is not running.
    pub fn open() -> anyhow::Result<Self> {
        use windows::core::HSTRING;
        use windows::Win32::System::Memory::{MapViewOfFile, OpenFileMappingW, VirtualQuery, FILE_MAP_READ, MEMORY_BASIC_INFORMATION};
        let name = HSTRING::from(MEMMAP_NAME);
        // SAFETY: plain WinAPI calls with valid arguments; handle/view are released in Drop.
        // Size 0 maps the whole section; VirtualQuery tells how large the view is so every
        // later read can be bounds-checked.
        unsafe {
            let handle = OpenFileMappingW(FILE_MAP_READ.0, false, &name)?;
            let view = MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0);
            if view.Value.is_null() {
                let _ = windows::Win32::Foundation::CloseHandle(handle);
                anyhow::bail!("MapViewOfFile failed for {MEMMAP_NAME}");
            }
            let mut info = MEMORY_BASIC_INFORMATION::default();
            let n = VirtualQuery(Some(view.Value), &mut info, std::mem::size_of::<MEMORY_BASIC_INFORMATION>());
            let len = if n == 0 { 0 } else { info.RegionSize };
            let m = Self { handle, view, len };
            if len < HEADER_SIZE {
                anyhow::bail!("iRacing mapping too small ({len} bytes)");
            }
            Ok(m)
        }
    }
}

#[cfg(windows)]
impl Memory for Mapping {
    fn size(&self) -> usize {
        self.len
    }

    fn read(&self, offset: usize, out: &mut [u8]) -> bool {
        if offset.checked_add(out.len()).is_none_or(|end| end > self.len) {
            return false;
        }
        // SAFETY: range checked against the view size above; the sim writes concurrently,
        // torn data is detected by the caller through the tick count.
        unsafe { std::ptr::copy_nonoverlapping((self.view.Value as *const u8).add(offset), out.as_mut_ptr(), out.len()) };
        true
    }
}

#[cfg(windows)]
impl Drop for Mapping {
    fn drop(&mut self) {
        use windows::Win32::System::Memory::UnmapViewOfFile;
        // SAFETY: view and handle were obtained in `open` and are released exactly once.
        unsafe {
            let _ = UnmapViewOfFile(self.view);
            let _ = windows::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

#[cfg(test)]
pub mod test_mem {
    //! Builds a synthetic shared memory image in the documented layout.
    use super::*;

    #[derive(Default)]
    pub struct Builder {
        vars: Vec<(String, VarType, usize, usize)>,
        row: Vec<u8>,
        pub yaml: String,
        pub tick: i32,
        pub session_update: i32,
    }

    impl Builder {
        pub fn new() -> Self {
            Self { tick: 1, session_update: 1, ..Default::default() }
        }

        fn add(&mut self, name: &str, ty: VarType, count: usize) -> usize {
            if let Some(v) = self.vars.iter().find(|v| v.0 == name) {
                return v.2;
            }
            let off = self.row.len();
            self.row.resize(off + ty.bytes() * count, 0);
            self.vars.push((name.into(), ty, off, count));
            off
        }

        pub fn f32(&mut self, name: &str, i: usize, count: usize, v: f32) -> &mut Self {
            let o = self.add(name, VarType::Float, count) + i * 4;
            self.row[o..o + 4].copy_from_slice(&v.to_le_bytes());
            self
        }

        pub fn f64(&mut self, name: &str, v: f64) -> &mut Self {
            let o = self.add(name, VarType::Double, 1);
            self.row[o..o + 8].copy_from_slice(&v.to_le_bytes());
            self
        }

        pub fn i32(&mut self, name: &str, i: usize, count: usize, v: i32) -> &mut Self {
            let o = self.add(name, VarType::Int, count) + i * 4;
            self.row[o..o + 4].copy_from_slice(&v.to_le_bytes());
            self
        }

        pub fn bits(&mut self, name: &str, v: u32) -> &mut Self {
            let o = self.add(name, VarType::BitField, 1);
            self.row[o..o + 4].copy_from_slice(&v.to_le_bytes());
            self
        }

        pub fn bool(&mut self, name: &str, i: usize, count: usize, v: bool) -> &mut Self {
            let o = self.add(name, VarType::Bool, count) + i;
            self.row[o] = u8::from(v);
            self
        }

        /// Header, var headers, YAML and three buffers; the newest line is in buffer 1.
        pub fn build(&self) -> Vec<u8> {
            let var_off = HEADER_SIZE;
            let yaml_off = var_off + self.vars.len() * VAR_HEADER_SIZE;
            let yaml = self.yaml.as_bytes();
            let buf0 = (yaml_off + yaml.len() + 1).div_ceil(16) * 16;
            let len = self.row.len();
            let mut m = vec![0u8; buf0 + 3 * len];
            let put = |m: &mut Vec<u8>, o: usize, v: i32| m[o..o + 4].copy_from_slice(&v.to_le_bytes());
            put(&mut m, 0, IRSDK_VER);
            put(&mut m, 4, ST_CONNECTED);
            put(&mut m, 8, 60);
            put(&mut m, 12, self.session_update);
            put(&mut m, 16, yaml.len() as i32);
            put(&mut m, 20, yaml_off as i32);
            put(&mut m, 24, self.vars.len() as i32);
            put(&mut m, 28, var_off as i32);
            put(&mut m, 32, 3);
            put(&mut m, 36, len as i32);
            for b in 0..3 {
                let o = 48 + b * 16;
                put(&mut m, o, if b == 1 { self.tick } else { self.tick - 3 + b as i32 });
                put(&mut m, o + 4, (buf0 + b * len) as i32);
            }
            for (i, (name, ty, off, count)) in self.vars.iter().enumerate() {
                let o = var_off + i * VAR_HEADER_SIZE;
                put(&mut m, o, *ty as i32);
                put(&mut m, o + 4, *off as i32);
                put(&mut m, o + 8, *count as i32);
                m[o + 16..o + 16 + name.len()].copy_from_slice(name.as_bytes());
            }
            m[yaml_off..yaml_off + yaml.len()].copy_from_slice(yaml);
            // Stale data in the other buffers, the real line in buffer 1.
            m[buf0 + len..buf0 + 2 * len].copy_from_slice(&self.row);
            m
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_mem::Builder;
    use super::*;
    use std::mem::offset_of;

    #[test]
    fn layout_matches_irsdk_defines() {
        assert_eq!(HEADER_SIZE, 112);
        assert_eq!(offset_of!(Header, num_buf), 32);
        assert_eq!(offset_of!(Header, var_buf), 48);
        assert_eq!(std::mem::size_of::<VarBuf>(), 16);
        assert_eq!(VAR_HEADER_SIZE, 144);
        assert_eq!(offset_of!(VarHeader, count_as_time), 12);
        assert_eq!(offset_of!(VarHeader, name), 16);
        assert_eq!(offset_of!(VarHeader, desc), 48);
        assert_eq!(offset_of!(VarHeader, unit), 112);
    }

    #[test]
    fn reads_vars_from_newest_buffer() {
        let mut b = Builder::new();
        b.f32("Speed", 0, 1, 42.5).i32("Gear", 0, 1, 3).f64("SessionTime", 12.25).bits("SessionFlags", 0x8000_0004);
        b.f32("CarIdxLapDistPct", 2, 64, 0.75).bool("OnPitRoad", 0, 1, true);
        b.yaml = "WeekendInfo:\n TrackName: spa\n".into();
        let mem = b.build();
        let h = read_header(&mem).unwrap();
        assert_eq!(h.ver, IRSDK_VER);
        let vars = read_vars(&mem, &h);
        let mut row = Row::default();
        read_row(&mem, &mut row).unwrap();
        assert_eq!(row.num(&vars["Speed"], 0), Some(42.5));
        assert_eq!(row.num(&vars["Gear"], 0), Some(3.0));
        assert_eq!(row.num(&vars["SessionTime"], 0), Some(12.25));
        assert_eq!(row.bits(&vars["SessionFlags"], 0), Some(0x8000_0004));
        assert_eq!(row.num(&vars["CarIdxLapDistPct"], 2), Some(0.75));
        assert_eq!(row.num(&vars["CarIdxLapDistPct"], 64), None);
        assert_eq!(row.num(&vars["OnPitRoad"], 0), Some(1.0));
        assert_eq!(read_session_yaml(&mem, &h).unwrap(), "WeekendInfo:\n TrackName: spa\n");
    }

    #[test]
    fn out_of_range_is_none() {
        let mem = vec![0u8; 20];
        assert!(read_header(&mem).is_none());
    }
}
