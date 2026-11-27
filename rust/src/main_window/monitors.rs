//! The Monitors card: the monitors in the configuration, renaming and removing them, adding the
//! connected ones, and reading their inputs over DDC/CI in the background.

use super::monitor_cards::{self, Action};
use super::*;
use crate::config::ProfileMonitor;
use crate::monitors;
use crate::ui::dialog::{self, Icon};

impl MainWindow {
    pub(super) fn monitor_action(&mut self, action: Action) {
        match action {
            Action::Renaming(row) => self.rename_monitor(row),
            Action::Renamed(row) => self.finish_renaming(row),
            Action::Remove(row) => self.remove_monitor(row),
        }
    }

    pub(super) fn refresh_monitors(&mut self) {
        let rows = self.sel.map_or_else(Vec::new, |i| {
            self.config.profiles[i].monitors.iter().map(|m| self.monitor_row(m)).collect()
        });
        monitor_cards::set_rows(self.c.monitors, rows);
        self.layout();
    }

    fn monitor_row(&self, m: &ProfileMonitor) -> monitor_cards::Row {
        let connected = self.is_connected(&m.monitor_id);
        let current = self.current.get(&m.monitor_id.to_lowercase()).copied().flatten();
        monitor_cards::Row {
            id: m.monitor_id.clone(),
            name: m.name.clone(),
            details: match &m.serial {
                Some(serial) if !serial.is_empty() => format!("{} · S/N {serial}", m.model),
                _ => m.model.clone(),
            },
            connected,
            chips: m.supported_inputs.iter().map(|&c| (monitors::short_name(c), current == Some(c))).collect(),
            hint: match (m.supported_inputs.is_empty(), connected) {
                (false, _) => String::new(),
                (true, true) => "Inputs not detected yet".into(),
                (true, false) => "Connect this monitor to detect its inputs".into(),
            },
        }
    }

    /// Typing a monitor's name: keep it (unless blank) and save shortly.
    fn rename_monitor(&mut self, row: usize) {
        let (Some(i), Some(text)) = (self.sel, monitor_cards::name_text(self.c.monitors, row)) else { return };
        let Some(m) = self.config.profiles[i].monitors.get_mut(row) else { return };
        let text = text.trim();
        if !text.is_empty() && m.name != text {
            m.name = text.to_string();
            self.commit_soon();
        }
    }

    /// Leaving a monitor's name box: put back the saved name if it was left blank, and show the
    /// new name in the hotkeys list.
    fn finish_renaming(&mut self, row: usize) {
        let Some(i) = self.sel else { return };
        let Some(saved) = self.config.profiles[i].monitors.get(row).map(|m| m.name.clone()) else { return };
        if monitor_cards::name_text(self.c.monitors, row).is_some_and(|t| t.trim() != saved) {
            monitor_cards::set_name_text(self.c.monitors, row, &saved);
        }
        self.refresh_hotkeys();
    }

    pub(super) fn add_connected(&mut self) {
        let Some(i) = self.sel else { return };
        let missing: Vec<ProfileMonitor> = self.connected.iter()
            .filter(|m| !m.internal && !self.config.profiles[i].has_monitor(&m.id))
            .map(ProfileMonitor::from_connected)
            .collect();
        if missing.is_empty() {
            dialog::alert(self.hwnd, Icon::Info, "Nothing to add", "All the connected monitors are already in this configuration.");
            return;
        }
        for m in missing {
            self.attempted.remove(&m.monitor_id.to_lowercase()); // always read what was just added
            self.config.profiles[i].monitors.push(m);
        }
        self.commit();
        self.refresh_profile_label();
        self.update_status();
        self.refresh_monitors();
        self.detect(false);
    }

    fn remove_monitor(&mut self, row: usize) {
        let Some(i) = self.sel else { return };
        let p = &self.config.profiles[i];
        let Some(m) = p.monitors.get(row) else { return };
        let used = p.bindings.iter().filter(|b| b.monitor_id.eq_ignore_ascii_case(&m.monitor_id)).count();
        let also = match used {
            0 => String::new(),
            1 => " Its hotkey will be removed too.".into(),
            n => format!(" Its {n} hotkeys will be removed too."),
        };
        let msg = format!("Configurations are recognised by their monitors, so this changes which setups \"{}\" matches.{also}", p.name);
        if !dialog::confirm(self.hwnd, &format!("Remove {}?", m.name), &msg, "Remove") {
            return;
        }
        let p = &mut self.config.profiles[i];
        let id = p.monitors.remove(row).monitor_id;
        p.bindings.retain(|b| !b.monitor_id.eq_ignore_ascii_case(&id));
        self.commit();
        self.refresh_profile_label();
        self.update_status();
        self.refresh_monitors();
        self.refresh_hotkeys();
    }

    // -------------------------------------------------------------------------------- detection

    /// Reads the connected monitors' inputs and current input in the background: all of them
    /// (`force`, the Detect inputs button), or only those not read yet this session.
    pub(super) fn detect(&mut self, force: bool) {
        let Some(i) = self.sel else { return };
        if self.detecting {
            self.detect_queued = Some(force || self.detect_queued.unwrap_or(false));
            return;
        }
        let targets: Vec<(String, MonitorInfo)> = self.config.profiles[i].monitors.iter()
            .filter(|m| force || !self.attempted.contains(&m.monitor_id.to_lowercase()))
            .filter_map(|m| self.connected_monitor(&m.monitor_id).map(|info| (m.monitor_id.clone(), info.clone())))
            .collect();
        if targets.is_empty() {
            if force {
                ui::set_text(self.c.activity, "None of this configuration's monitors are connected.");
            }
            return;
        }
        self.attempted.extend(targets.iter().map(|(id, _)| id.to_lowercase()));
        self.detecting = true;
        ui::set_text(self.c.activity, "Reading monitors over DDC/CI…");
        std::thread::spawn(move || {
            for (id, m) in targets {
                let (inputs, current) = (monitors::supported_inputs(&m), monitors::current_input(&m));
                post(Update::Detected { id, inputs, current });
            }
            post(Update::DetectDone);
        });
    }

    /// A monitor's inputs were read. Returns true if the configuration changed (it's saved once
    /// every update has been applied).
    pub(super) fn on_detected(&mut self, id: &str, inputs: Vec<u8>, current: Option<u8>) -> bool {
        let mut changed = false;
        if !inputs.is_empty() {
            for m in self.config.profiles.iter_mut().flat_map(|p| &mut p.monitors) {
                if m.monitor_id.eq_ignore_ascii_case(id) && m.supported_inputs != inputs {
                    m.supported_inputs = inputs.clone();
                    changed = true;
                }
            }
        }
        self.current.insert(id.to_lowercase(), current);
        self.refresh_monitors();
        changed
    }

    pub(super) fn on_detect_done(&mut self) {
        self.detecting = false;
        let missing = self.sel.is_some_and(|i| {
            self.config.profiles[i].monitors.iter().any(|m| self.is_connected(&m.monitor_id) && m.supported_inputs.is_empty())
        });
        ui::set_text(self.c.activity, if missing {
            "Some monitors did not report their inputs. Check DDC/CI is on in the monitor's menu."
        } else {
            "Inputs detected."
        });
        if let Some(force) = self.detect_queued.take() {
            self.detect(force);
        }
    }

    /// A hotkey's Test button switched a monitor (or failed to).
    pub(super) fn on_tested(&mut self, id: &str, result: Result<u8, String>) {
        let text = match result {
            Ok(code) => {
                self.current.insert(id.to_lowercase(), Some(code));
                format!("{}: {}", self.monitor_name(id), monitors::short_name(code))
            }
            Err(e) => e,
        };
        ui::set_text(self.c.activity, &text);
        self.refresh_monitors();
    }

    /// The monitor's name as the user gave it, else as Windows reports it.
    pub(super) fn monitor_name(&self, id: &str) -> String {
        self.sel.and_then(|i| self.config.profiles[i].monitor(id)).map(|m| m.name.clone())
            .or_else(|| self.connected_monitor(id).map(|m| m.display_name().to_string()))
            .unwrap_or_else(|| "The monitor".into())
    }
}
