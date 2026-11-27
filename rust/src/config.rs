//! Settings: the data model and the JSON file, in the same format as the C# app's config.json.

use crate::hotkey::Hotkey;
use crate::json::{self, Json};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Debug, PartialEq)]
pub struct AppConfig {
    pub profiles: Vec<Profile>,
    pub show_notifications: bool,
    pub theme: Theme,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig { profiles: Vec::new(), show_notifications: true, theme: Theme::System }
    }
}

/// Light or dark windows. System follows the Windows setting (and is what older files get);
/// Light and Dark override it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    /// As the UI toolkit takes it: dark, light, or Windows' setting (None).
    pub fn dark(self) -> Option<bool> {
        match self {
            Theme::System => None,
            Theme::Light => Some(false),
            Theme::Dark => Some(true),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Theme::System => "System",
            Theme::Light => "Light",
            Theme::Dark => "Dark",
        }
    }

    /// Case-insensitive, like the C# reader; anything else follows Windows.
    fn parse(s: Option<&str>) -> Theme {
        match s.map(str::to_ascii_lowercase).as_deref() {
            Some("light") => Theme::Light,
            Some("dark") => Theme::Dark,
            _ => Theme::System,
        }
    }
}

/// A configuration for one display setup (e.g. "Work"), identified by its external monitors.
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub created_utc: String,
    pub monitors: Vec<ProfileMonitor>,
    pub bindings: Vec<Binding>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProfileMonitor {
    pub monitor_id: String,
    pub name: String,
    pub model: String,
    pub serial: Option<String>,
    pub supported_inputs: Vec<u8>,
}

impl ProfileMonitor {
    /// A connected monitor, as it's added to a configuration (its inputs are read later).
    pub fn from_connected(m: &crate::monitors::MonitorInfo) -> ProfileMonitor {
        ProfileMonitor { monitor_id: m.id.clone(), name: m.display_name().to_string(), model: m.model.clone(), serial: m.serial.clone(), supported_inputs: Vec::new() }
    }
}

/// A hotkey for one monitor: one input switches to it, several are stepped through in order.
#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    pub id: String,
    pub monitor_id: String,
    pub hotkey: Hotkey,
    pub inputs: Vec<u8>,
}

impl Profile {
    pub fn monitor(&self, id: &str) -> Option<&ProfileMonitor> {
        self.monitors.iter().find(|m| m.monitor_id.eq_ignore_ascii_case(id))
    }
    pub fn has_monitor(&self, id: &str) -> bool {
        self.monitor(id).is_some()
    }
}

// ------------------------------------------------------------------------------------------ JSON

fn s(v: &str) -> Json {
    Json::Str(v.to_string())
}

fn codes(v: &[u8]) -> Json {
    Json::Arr(v.iter().map(|&c| Json::Num(c as f64)).collect())
}

fn read_codes(v: &[Json]) -> Vec<u8> {
    v.iter().filter_map(Json::as_f64).filter(|n| (0.0..=255.0).contains(n)).map(|n| n as u8).collect()
}

impl AppConfig {
    pub fn to_json(&self) -> Json {
        Json::Obj(vec![
            ("Profiles".into(), Json::Arr(self.profiles.iter().map(Profile::to_json).collect())),
            ("ShowNotifications".into(), Json::Bool(self.show_notifications)),
            ("Theme".into(), s(self.theme.name())),
        ])
    }

    pub fn from_json(v: &Json) -> AppConfig {
        AppConfig {
            profiles: v.arr("Profiles").iter().map(Profile::from_json).collect(),
            show_notifications: v.bool("ShowNotifications").unwrap_or(true),
            theme: Theme::parse(v.str("Theme")),
        }
    }
}

impl Profile {
    fn to_json(&self) -> Json {
        Json::Obj(vec![
            ("Id".into(), s(&self.id)),
            ("Name".into(), s(&self.name)),
            ("Enabled".into(), Json::Bool(self.enabled)),
            // The C# app can't read a blank date.
            ("CreatedUtc".into(), if self.created_utc.trim().is_empty() { s(&crate::sys::utc_now()) } else { s(&self.created_utc) }),
            ("Monitors".into(), Json::Arr(self.monitors.iter().map(|m| Json::Obj(vec![
                ("MonitorId".into(), s(&m.monitor_id)),
                ("Name".into(), s(&m.name)),
                ("Model".into(), s(&m.model)),
                ("Serial".into(), m.serial.as_deref().map(s).unwrap_or(Json::Null)),
                ("SupportedInputs".into(), codes(&m.supported_inputs)),
            ])).collect())),
            ("Bindings".into(), Json::Arr(self.bindings.iter().map(|b| Json::Obj(vec![
                ("Id".into(), s(&b.id)),
                ("MonitorId".into(), s(&b.monitor_id)),
                ("Hotkey".into(), b.hotkey.to_json()),
                ("Inputs".into(), codes(&b.inputs)),
            ])).collect())),
        ])
    }

    fn from_json(v: &Json) -> Profile {
        Profile {
            id: v.str("Id").map(str::to_string).unwrap_or_else(crate::sys::new_id),
            name: v.str("Name").unwrap_or("Unnamed setup").to_string(),
            enabled: v.bool("Enabled").unwrap_or(false),
            // The C# app can't read an empty date, so a missing or blank one becomes "now".
            created_utc: v.str("CreatedUtc").filter(|s| !s.trim().is_empty()).map(str::to_string).unwrap_or_else(crate::sys::utc_now),
            monitors: v.arr("Monitors").iter().map(|m| ProfileMonitor {
                monitor_id: m.str("MonitorId").unwrap_or("").to_string(),
                name: m.str("Name").unwrap_or("").to_string(),
                model: m.str("Model").unwrap_or("").to_string(),
                serial: m.str("Serial").map(str::to_string),
                supported_inputs: read_codes(m.arr("SupportedInputs")),
            }).collect(),
            bindings: v.arr("Bindings").iter().map(|b| Binding {
                id: b.str("Id").map(str::to_string).unwrap_or_else(crate::sys::new_id),
                monitor_id: b.str("MonitorId").unwrap_or("").to_string(),
                hotkey: b.get("Hotkey").map(Hotkey::from_json).unwrap_or_default(),
                inputs: read_codes(b.arr("Inputs")),
            }).collect(),
        }
    }
}

// ------------------------------------------------------------------------------------------ file

pub const CONFIG_DIR_VARIABLE: &str = "INPUTSWITCH_CONFIG_DIR";

fn override_dir() -> Option<PathBuf> {
    std::env::var_os(CONFIG_DIR_VARIABLE)
        .filter(|v| !v.is_empty())
        .map(|v| std::path::absolute(PathBuf::from(&v)).unwrap_or_else(|_| PathBuf::from(v)))
}

/// %APPDATA%\InputSwitch, or the folder in INPUTSWITCH_CONFIG_DIR (used by the smoke tests).
pub fn dir() -> PathBuf {
    override_dir().unwrap_or_else(|| {
        let app_data = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_default()).join(r"AppData\Roaming")
        });
        app_data.join("InputSwitch")
    })
}

pub fn file_path() -> PathBuf {
    dir().join("config.json")
}

fn backup_path() -> PathBuf {
    dir().join("config.json.bad")
}

/// Distinguishes instances using different config folders, so a test copy can run beside the
/// real one: "-" and the first 12 hex digits of the folder's SHA-256, exactly as the C# app makes
/// it (so the two apps also keep each other out of the same folder). Empty for the normal location.
pub fn instance_key() -> String {
    let Some(d) = override_dir() else { return String::new() };
    let bytes = d.to_string_lossy().to_lowercase().into_bytes();
    let mut hash = [0u8; 32];
    const BCRYPT_SHA256_ALG_HANDLE: isize = 0x41;
    unsafe { crate::ffi::BCryptHash(BCRYPT_SHA256_ALG_HANDLE, core::ptr::null(), 0, bytes.as_ptr(), bytes.len() as u32, hash.as_mut_ptr(), 32) };
    format!("-{}", hash.iter().take(6).map(|b| format!("{b:02X}")).collect::<String>())
}

/// Set when the settings file exists but couldn't be read or backed up: then it's never
/// overwritten, so nothing is lost (saves report an error instead).
static KEEP_FILE: AtomicBool = AtomicBool::new(false);

/// Loads the settings. If the file exists but can't be read, it's kept (as config.json.bad, or
/// left untouched) and the problem is returned alongside fresh settings.
pub fn load() -> (AppConfig, Option<String>) {
    let path = file_path();
    let bytes = match read_with_retries(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (AppConfig::default(), None),
        Err(e) => {
            KEEP_FILE.store(true, Ordering::Relaxed);
            return (AppConfig::default(), Some(format!("{e}. The file was left as it is; changes won't be saved this session.")));
        }
    };
    match json::parse(&decode(&bytes)) {
        Ok(v) => (AppConfig::from_json(&v), None),
        Err(e) => {
            let kept = std::fs::copy(&path, backup_path()).is_ok();
            KEEP_FILE.store(!kept, Ordering::Relaxed);
            let note = if kept { format!("The unreadable file was kept as config.json.bad in {}.", dir().display()) } else { "Changes won't be saved this session.".into() };
            (AppConfig::default(), Some(format!("{e}. {note}")))
        }
    }
}

/// Reads the file, retrying briefly: antivirus and sync tools can hold it open at sign-in.
fn read_with_retries(path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    let mut result = std::fs::read(path);
    for _ in 0..5 {
        match &result {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => std::thread::sleep(std::time::Duration::from_millis(200)),
            _ => break,
        }
        result = std::fs::read(path);
    }
    result
}

/// The file's text: UTF-8 (with or without a byte order mark), UTF-16 with a byte order mark
/// (Notepad's "Unicode"), or else Windows-1252 (Notepad's "ANSI").
fn decode(bytes: &[u8]) -> String {
    let utf16 = |be: bool| {
        let units: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| if be { u16::from_be_bytes([c[0], c[1]]) } else { u16::from_le_bytes([c[0], c[1]]) }).collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        [0xFF, 0xFE, ..] => utf16(false),
        [0xFE, 0xFF, ..] => utf16(true),
        _ => match std::str::from_utf8(bytes) {
            Ok(s) => s.to_string(),
            Err(_) => bytes.iter().map(|&b| windows_1252(b)).collect(),
        },
    }
}

fn windows_1252(b: u8) -> char {
    const HIGH: [char; 32] = ['€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
        '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ'];
    match b {
        0x80..=0x9F => HIGH[(b - 0x80) as usize],
        _ => b as char,
    }
}

/// Saves atomically: write a temporary file, then replace.
pub fn save(cfg: &AppConfig) -> std::io::Result<()> {
    if KEEP_FILE.load(Ordering::Relaxed) {
        return Err(std::io::Error::other("The settings file couldn't be read when InputSwitch started, so it isn't overwritten. Restart InputSwitch to try again."));
    }
    std::fs::create_dir_all(dir())?;
    let path = file_path();
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json::write(&cfg.to_json()))?;
    std::fs::rename(&tmp, &path)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::hotkey::{ALT, CONTROL};

    #[test]
    fn reads_the_csharp_settings_file() {
        let text = r#"{
          "Profiles": [ {
              "Id": "55daf5ea8adb41cbae76cdfb224b8d9c", "Name": "Work", "Enabled": true,
              "CreatedUtc": "2026-11-11T12:47:11.0827479Z",
              "Monitors": [ { "MonitorId": "DELD0D9#BPG5ZT2", "Name": "DELL P2419H", "Model": "DELD0D9",
                              "Serial": "BPG5ZT2", "SupportedInputs": [ 1, 15, 17 ] } ],
              "Bindings": [ { "Id": "ad46", "MonitorId": "DELD0D9#BPG5ZT2",
                              "Hotkey": { "Modifiers": "Alt, Control", "Key": "M" }, "Inputs": [ 15, 17 ] } ] } ],
          "ShowNotifications": true, "Theme": "Dark" }"#;
        let cfg = AppConfig::from_json(&json::parse(text).unwrap());
        let p = &cfg.profiles[0];
        assert_eq!(p.name, "Work");
        assert!(p.enabled);
        assert_eq!(p.monitors[0].supported_inputs, vec![1, 15, 17]);
        assert_eq!(p.bindings[0].hotkey, Hotkey { mods: ALT | CONTROL, vk: b'M' as u16 });
        assert_eq!(p.bindings[0].inputs, vec![15, 17]);
        assert_eq!(cfg.theme, Theme::Dark);

        // And survives a round trip through our writer.
        let again = AppConfig::from_json(&json::parse(&json::write(&cfg.to_json())).unwrap());
        assert_eq!(again, cfg);
    }

    #[test]
    fn missing_or_blank_dates_are_filled_in() {
        // The C# app can't read an empty CreatedUtc, so Rust must never write one.
        let text = r#"{ "Profiles": [ { "Id": "a", "Name": "No date" }, { "Id": "b", "Name": "Blank", "CreatedUtc": "  " },
                                      { "Id": "c", "Name": "Null", "CreatedUtc": null } ] }"#;
        let cfg = AppConfig::from_json(&json::parse(text).unwrap());
        for p in &cfg.profiles {
            assert!(p.created_utc.len() >= 20 && p.created_utc.ends_with('Z'), "{}: '{}'", p.name, p.created_utc);
        }
        let kept = AppConfig::from_json(&json::parse(r#"{ "Profiles": [ { "CreatedUtc": "2026-11-11T12:47:11.0827479Z" } ] }"#).unwrap());
        assert_eq!(kept.profiles[0].created_utc, "2026-11-11T12:47:11.0827479Z");
    }

    #[test]
    fn theme_defaults_to_following_windows() {
        let parse = |t: &str| AppConfig::from_json(&json::parse(t).unwrap()).theme;
        assert_eq!(parse(r#"{ }"#), Theme::System);
        assert_eq!(parse(r#"{ "Theme": "light" }"#), Theme::Light);
        assert_eq!(parse(r#"{ "Theme": "Sepia" }"#), Theme::System);
    }

    #[test]
    fn reads_the_encodings_notepad_saves() {
        let json = "{\"Profiles\": [ { \"Name\": \"Büro\" } ]}";
        let utf16: Vec<u8> = [0xFF, 0xFE].into_iter().chain(json.encode_utf16().flat_map(u16::to_le_bytes)).collect();
        let bom: Vec<u8> = [0xEF, 0xBB, 0xBF].iter().copied().chain(json.bytes()).collect();
        let ansi: Vec<u8> = json.chars().map(|c| if c == 'ü' { 0xFC } else { c as u8 }).collect();
        for bytes in [json.as_bytes().to_vec(), bom, utf16, ansi] {
            let cfg = AppConfig::from_json(&json::parse(&decode(&bytes)).unwrap());
            assert_eq!(cfg.profiles[0].name, "Büro");
        }
    }

    #[test]
    fn never_writes_a_blank_date() {
        let mut p = Profile::from_json(&json::parse(r#"{ "Name": "x" }"#).unwrap());
        p.created_utc = String::new();
        assert!(p.to_json().str("CreatedUtc").is_some_and(|d| d.ends_with('Z')));
    }
}