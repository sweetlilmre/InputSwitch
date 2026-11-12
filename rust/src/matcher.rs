//! Decides which configuration belongs to the monitors connected now.

use crate::config::{AppConfig, Profile, ProfileMonitor};
use crate::monitors::MonitorInfo;
use std::collections::HashSet;

/// The external monitors define a setup; the laptop's own panel is ignored, so opening or closing
/// the lid doesn't look like a different place. IDs are compared case-insensitively.
pub fn fingerprint(monitors: &[MonitorInfo]) -> HashSet<String> {
    monitors.iter().filter(|m| !m.internal).map(|m| m.id.to_lowercase()).collect()
}

fn ids(p: &Profile) -> HashSet<String> {
    p.monitors.iter().map(|m| m.monitor_id.to_lowercase()).collect()
}

/// Index of the matching profile, or None for an unknown setup:
/// 1. a profile with exactly these monitors (first in list order);
/// 2. otherwise one containing all of them (a monitor switched away or off), preferring the
///    current one, then the fewest monitors, then the oldest.
pub fn find(config: &AppConfig, fp: &HashSet<String>, current: Option<&str>) -> Option<usize> {
    if fp.is_empty() {
        return None;
    }
    if let Some(i) = config.profiles.iter().position(|p| ids(p) == *fp) {
        return Some(i);
    }
    let supersets: Vec<usize> = (0..config.profiles.len()).filter(|&i| ids(&config.profiles[i]).is_superset(fp)).collect();
    if let Some(&i) = supersets.iter().find(|&&i| Some(config.profiles[i].id.as_str()) == current) {
        return Some(i);
    }
    supersets.into_iter().min_by(|&a, &b| {
        let (pa, pb) = (&config.profiles[a], &config.profiles[b]);
        pa.monitors.len().cmp(&pb.monitors.len()).then_with(|| pa.created_utc.cmp(&pb.created_utc))
    })
}

/// A new configuration for the connected external monitors, hotkeys off, with a unique name.
pub fn create_for(monitors: &[MonitorInfo], config: &AppConfig) -> Profile {
    let base = format!("Setup {}", config.profiles.len() + 1);
    let mut name = base.clone();
    let mut n = 2;
    while config.profiles.iter().any(|p| p.name.eq_ignore_ascii_case(&name)) {
        name = format!("{base} ({n})");
        n += 1;
    }
    Profile {
        id: crate::sys::new_id(),
        name,
        enabled: false,
        created_utc: crate::sys::utc_now(),
        monitors: monitors.iter().filter(|m| !m.internal).map(|m| ProfileMonitor {
            monitor_id: m.id.clone(),
            name: m.display_name().to_string(),
            model: m.model.clone(),
            serial: m.serial.clone(),
            supported_inputs: Vec::new(),
        }).collect(),
        bindings: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(id: &str, internal: bool) -> MonitorInfo {
        MonitorInfo { id: id.into(), friendly: id.into(), model: id.into(), serial: None, internal, gdi: r"\\.\DISPLAY1".into(), index: 0 }
    }

    fn profile(name: &str, ids: &[&str]) -> Profile {
        Profile {
            id: name.to_lowercase(),
            name: name.into(),
            enabled: false,
            created_utc: String::new(),
            monitors: ids.iter().map(|&i| ProfileMonitor { monitor_id: i.into(), name: i.into(), model: String::new(), serial: None, supported_inputs: vec![] }).collect(),
            bindings: vec![],
        }
    }

    fn set(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_lowercase()).collect()
    }

    fn work_and_home() -> AppConfig {
        AppConfig { profiles: vec![profile("Work", &["W#1", "W#2"]), profile("Home", &["H#1", "H#2"])], show_notifications: true }
    }

    #[test]
    fn laptop_screen_is_ignored() {
        assert_eq!(fingerprint(&[mon("BOE@x", true), mon("A#1", false), mon("B#2", false)]), set(&["A#1", "B#2"]));
    }

    #[test]
    fn exact_match() {
        let c = work_and_home();
        assert_eq!(find(&c, &set(&["H#1", "H#2"]), Some("work")), Some(1));
        assert_eq!(find(&c, &set(&["w#1", "W#2"]), None), Some(0));
    }

    #[test]
    fn missing_monitor_keeps_the_configuration() {
        assert_eq!(find(&work_and_home(), &set(&["W#2"]), None), Some(0));
    }

    #[test]
    fn current_is_kept_then_fewest_monitors() {
        let c = AppConfig { profiles: vec![profile("Big", &["A#1", "B#2", "C#3"]), profile("Small", &["A#1", "B#2"])], show_notifications: true };
        assert_eq!(find(&c, &set(&["A#1"]), None), Some(1));
        assert_eq!(find(&c, &set(&["A#1"]), Some("big")), Some(0));
    }

    #[test]
    fn unknown_or_empty() {
        let c = work_and_home();
        assert_eq!(find(&c, &set(&["W#1", "NEW#9"]), Some("work")), None);
        assert_eq!(find(&c, &set(&[]), Some("work")), None);
    }

    #[test]
    fn new_configuration() {
        let mut c = AppConfig::default();
        c.profiles.push(profile("Setup 2", &[]));
        let p = create_for(&[mon("A#1", false), mon("LAPTOP", true)], &c);
        assert!(!p.enabled);
        assert_eq!(p.monitors.iter().map(|m| m.monitor_id.as_str()).collect::<Vec<_>>(), vec!["A#1"]);
        assert_eq!(p.name, "Setup 2 (2)");
    }
}
