//! Small system helpers: IDs, timestamps, the version, and "Start with Windows".

use crate::ffi::*;
use crate::registry::RegKey;
use std::sync::atomic::{AtomicU64, Ordering};

/// 32 random hex digits (like a GUID in "N" format).
pub fn new_id() -> String {
    const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 2;
    let mut b = [0u8; 16];
    if unsafe { BCryptGenRandom(0, b.as_mut_ptr(), b.len() as u32, BCRYPT_USE_SYSTEM_PREFERRED_RNG) } != 0 {
        // Never all zeros (ids must differ): the time and a counter instead.
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos() as u64;
        b[..8].copy_from_slice(&now.to_le_bytes());
        b[8..].copy_from_slice(&COUNTER.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Current UTC time as ISO 8601, e.g. 2026-11-12T09:15:03.120Z.
pub fn utc_now() -> String {
    let mut t: SYSTEMTIME = zeroed();
    unsafe { GetSystemTime(&mut t) };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds)
}

pub const APP_NAME: &str = "InputSwitch";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The git commit this was built from, as build.rs found it: empty when git isn't available.
pub const COMMIT: &str = env!("GIT_COMMIT");

/// "0.1.0+<commit>" when the commit is known.
#[allow(clippy::const_is_empty)]
pub fn full_version() -> String {
    if COMMIT.is_empty() { VERSION.into() } else { format!("{VERSION}+{COMMIT}") }
}

/// "0.1.0 (abc1234)".
#[allow(clippy::const_is_empty)]
pub fn display_version() -> String {
    if COMMIT.is_empty() { VERSION.into() } else { format!("{VERSION} ({})", &COMMIT[..COMMIT.len().min(7)]) }
}

// ------------------------------------------------------------------------------------------ startup

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "InputSwitch";

/// What the Run key holds for this copy of the app: its path, quoted.
fn startup_command() -> String {
    let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
    format!("\"{exe}\"")
}

/// Whether Windows starts this copy of the app at sign-in.
pub fn startup_enabled() -> bool {
    RegKey::open(HKEY_CURRENT_USER, RUN_KEY)
        .and_then(|k| k.string(RUN_VALUE))
        .is_some_and(|cmd| cmd.eq_ignore_ascii_case(&startup_command()))
}

/// Turns "Start with Windows" on or off. False if the registry couldn't be written.
pub fn set_startup(enabled: bool) -> bool {
    let Some(key) = RegKey::create(HKEY_CURRENT_USER, RUN_KEY) else { return false };
    if enabled { key.set_string(RUN_VALUE, &startup_command()) } else { key.delete(RUN_VALUE) }
}
