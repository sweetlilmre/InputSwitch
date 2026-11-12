//! Small system helpers: IDs, timestamps, version, "Start with Windows".

use crate::ffi::*;

/// 32 random hex digits (like a GUID in "N" format).
pub fn new_id() -> String {
    let mut b = [0u8; 16];
    // BCRYPT_USE_SYSTEM_PREFERRED_RNG = 2
    unsafe { BCryptGenRandom(0, b.as_mut_ptr(), b.len() as u32, 2) };
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Current UTC time as ISO 8601, e.g. 2026-11-12T09:15:03.120Z.
pub fn utc_now() -> String {
    let mut t: SYSTEMTIME = zeroed();
    unsafe { GetSystemTime(&mut t) };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds)
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const COMMIT: &str = env!("GIT_COMMIT");

/// "0.1.0+<commit>" when the commit is known.
pub fn full_version() -> String {
    if COMMIT.is_empty() { VERSION.into() } else { format!("{VERSION}+{COMMIT}") }
}

/// "0.1.0 (abc1234)".
pub fn display_version() -> String {
    if COMMIT.is_empty() { VERSION.into() } else { format!("{VERSION} ({})", &COMMIT[..COMMIT.len().min(7)]) }
}

// ------------------------------------------------------------------------------------------ startup

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "InputSwitch";

fn startup_command() -> String {
    let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
    format!("\"{exe}\"")
}

pub fn startup_enabled() -> bool {
    unsafe {
        let mut key: HKEY = 0;
        if RegOpenKeyExW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), 0, KEY_READ, &mut key) != 0 {
            return false;
        }
        let mut buf = [0u16; 1024];
        let mut len = (buf.len() * 2) as u32;
        let mut kind = 0u32;
        let ok = RegQueryValueExW(key, wide(RUN_VALUE).as_ptr(), core::ptr::null_mut(), &mut kind, buf.as_mut_ptr() as *mut u8, &mut len) == 0;
        RegCloseKey(key);
        ok && kind == REG_SZ && from_wide(&buf).eq_ignore_ascii_case(&startup_command())
    }
}

pub fn set_startup(enabled: bool) -> bool {
    unsafe {
        let mut key: HKEY = 0;
        if RegCreateKeyExW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), 0, core::ptr::null(), 0, KEY_WRITE,
            core::ptr::null(), &mut key, core::ptr::null_mut()) != 0 {
            return false;
        }
        let name = wide(RUN_VALUE);
        let rc = if enabled {
            let value = wide(&startup_command());
            RegSetValueExW(key, name.as_ptr(), 0, REG_SZ, value.as_ptr() as *const u8, (value.len() * 2) as u32)
        } else {
            let rc = RegDeleteValueW(key, name.as_ptr());
            if rc == 2 { 0 } else { rc } // already absent
        };
        RegCloseKey(key);
        rc == 0
    }
}
