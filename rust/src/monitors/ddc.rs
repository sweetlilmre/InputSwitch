//! Talking to a monitor over DDC/CI (via dxva2): its current input, switching input, and its
//! capabilities string. All blocking and slow (the I²C bus is): call these off the UI thread.

use super::MonitorInfo;
use crate::ffi::*;
use std::sync::Mutex;

pub const VCP_INPUT_SOURCE: u8 = 0x60;

/// Some monitors misbehave under concurrent requests: one conversation at a time.
static DDC_LOCK: Mutex<()> = Mutex::new(());

/// A monitor, open for DDC/CI for as long as this lives (with the bus to itself).
pub struct Physical {
    handle: HANDLE,
}

impl Physical {
    /// The input the monitor is showing, or None if it didn't answer.
    pub fn current_input(&self) -> Option<u8> {
        retry(3, 60, || {
            let (mut kind, mut cur, mut max) = (0u32, 0u32, 0u32);
            let ok = unsafe { GetVCPFeatureAndVCPFeatureReply(self.handle, VCP_INPUT_SOURCE, &mut kind, &mut cur, &mut max) } != 0;
            ok.then_some((cur & 0xFF) as u8)
        })
    }

    pub fn set_input(&self, code: u8) -> bool {
        retry(3, 60, || (unsafe { SetVCPFeature(self.handle, VCP_INPUT_SOURCE, code as u32) } != 0).then_some(())).is_some()
    }

    /// The MCCS capabilities string. Can take a second or two.
    pub fn capabilities(&self) -> Option<String> {
        retry(2, 100, || unsafe {
            let mut len = 0u32;
            if GetCapabilitiesStringLength(self.handle, &mut len) == 0 || len == 0 {
                return None;
            }
            let mut buf = vec![0u8; len as usize];
            if CapabilitiesRequestAndCapabilitiesReply(self.handle, buf.as_mut_ptr(), len) == 0 {
                return None;
            }
            let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            Some(String::from_utf8_lossy(&buf[..end]).into_owned())
        })
    }
}

/// Tries `f` up to `times` times, pausing `ms` between tries.
fn retry<T>(times: u32, ms: u64, mut f: impl FnMut() -> Option<T>) -> Option<T> {
    for attempt in 0..times {
        if let Some(v) = f() {
            return Some(v);
        }
        if attempt + 1 < times {
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
    }
    None
}

/// Opens monitor `m` for DDC/CI and runs `action` with it. None if it can't be reached.
pub fn with_monitor<T>(m: &MonitorInfo, action: impl FnOnce(&Physical) -> Option<T>) -> Option<T> {
    let hmonitor = find_hmonitor(&m.gdi)?;
    let _bus = DDC_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        let mut count = 0u32;
        if GetNumberOfPhysicalMonitorsFromHMONITOR(hmonitor, &mut count) == 0 || count == 0 {
            return None;
        }
        let mut monitors: Vec<PHYSICAL_MONITOR> = (0..count).map(|_| zeroed()).collect();
        if GetPhysicalMonitorsFromHMONITOR(hmonitor, count, monitors.as_mut_ptr()) == 0 {
            return None;
        }
        // Several physical monitors share an HMONITOR in clone mode: `index` says which.
        let physical = Physical { handle: monitors[m.index.min(count as usize - 1)].hPhysicalMonitor };
        let result = action(&physical);
        DestroyPhysicalMonitors(count, monitors.as_ptr());
        result
    }
}

/// The HMONITOR for a GDI device name (\\.\DISPLAYn).
fn find_hmonitor(gdi: &str) -> Option<HMONITOR> {
    struct Search<'a> {
        gdi: &'a str,
        found: HMONITOR,
    }
    unsafe extern "system" fn visit(h: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        let search = &mut *(data as *mut Search);
        let mut info: MONITORINFOEXW = zeroed();
        info.cbSize = core::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(h, &mut info) != 0 && from_wide(&info.szDevice).eq_ignore_ascii_case(search.gdi) {
            search.found = h;
            return 0; // stop
        }
        1
    }
    let mut search = Search { gdi, found: 0 };
    unsafe { EnumDisplayMonitors(0, core::ptr::null(), visit, &mut search as *mut Search as LPARAM) };
    (search.found != 0).then_some(search.found)
}
