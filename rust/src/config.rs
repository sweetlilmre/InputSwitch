//! Settings: the data model and the JSON file, in the same format as the C# app's config.json.

use crate::hotkey::Hotkey;
use crate::json::{self, Json};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct AppConfig {
    pub profiles: Vec<Profile>,
    pub show_notifications: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig { profiles: Vec::new(), show_notifications: true }
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
        ])
    }

    pub fn from_json(v: &Json) -> AppConfig {
        AppConfig {
            profiles: v.arr("Profiles").iter().map(Profile::from_json).collect(),
            show_notifications: v.bool("ShowNotifications").unwrap_or(true),
        }
    }
}

impl Profile {
    fn to_json(&self) -> Json {
        Json::Obj(vec![
            ("Id".into(), s(&self.id)),
            ("Name".into(), s(&self.name)),
            ("Enabled".into(), Json::Bool(self.enabled)),
            ("CreatedUtc".into(), s(&self.created_utc)),
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
        PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join("InputSwitch")
    })
}

pub fn file_path() -> PathBuf {
    dir().join("config.json")
}

/// Distinguishes instances using different config folders (so a test copy can run beside the
/// real one). Empty for the normal location.
pub fn instance_key() -> String {
    match override_dir() {
        None => String::new(),
        Some(d) => {
            // FNV-1a: enough to tell folders apart.
            let mut h: u64 = 0xcbf2_9ce4_8422_2325;
            for b in d.to_string_lossy().to_lowercase().bytes() {
                h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
            }
            format!("-{:012x}", h & 0xFFFF_FFFF_FFFF)
        }
    }
}

/// Loads the settings. If the file exists but can't be read, it's kept as config.json.bad and
/// the error is returned alongside fresh settings.
pub fn load() -> (AppConfig, Option<String>) {
    let path = file_path();
    let Ok(text) = std::fs::read_to_string(&path) else { return (AppConfig::default(), None) };
    match json::parse(&text) {
        Ok(v) => (AppConfig::from_json(&v), None),
        Err(e) => {
            let _ = std::fs::copy(&path, path.with_extension("json.bad"));
            (AppConfig::default(), Some(e))
        }
    }
}

/// Saves atomically: write a temporary file, then replace.
pub fn save(cfg: &AppConfig) -> std::io::Result<()> {
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
          "ShowNotifications": true }"#;
        let cfg = AppConfig::from_json(&json::parse(text).unwrap());
        let p = &cfg.profiles[0];
        assert_eq!(p.name, "Work");
        assert!(p.enabled);
        assert_eq!(p.monitors[0].supported_inputs, vec![1, 15, 17]);
        assert_eq!(p.bindings[0].hotkey, Hotkey { mods: ALT | CONTROL, vk: b'M' as u16 });
        assert_eq!(p.bindings[0].inputs, vec![15, 17]);

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
}