//! The Hotkeys card: the configuration's hotkeys, and adding, editing, removing and testing them.

use super::hotkey_cards::{self, Action};
use super::*;
use crate::config::Binding;
use crate::ui::dialog::{self, Icon};
use crate::{hotkey_editor, monitors};

impl MainWindow {
    /// A button was clicked on a hotkey's card.
    pub(super) fn hotkey_action(&mut self, action: Action) {
        match action {
            Action::Test(row) => self.test_hotkey(row),
            Action::Edit(row) => self.edit_hotkey(row),
            Action::Remove(row) => self.remove_hotkey(row),
        }
    }

    /// Shows the selected configuration's hotkeys in the Hotkeys card.
    pub(super) fn refresh_hotkeys(&mut self) {
        let rows = self.sel.map_or_else(Vec::new, |i| {
            let p = &self.config.profiles[i];
            p.bindings.iter().map(|b| hotkey_cards::Row {
                keys: b.hotkey.key_names(),
                action: monitors::describe_action(&b.inputs),
                monitor: p.monitor(&b.monitor_id).map_or_else(|| "(removed monitor)".into(), |m| m.name.clone()),
            }).collect()
        });
        hotkey_cards::set_rows(self.c.hotkeys, rows);
        self.layout();
    }

    /// "Add hotkey": records a new hotkey in the editor.
    pub(super) fn add_hotkey(&mut self) {
        let Some(i) = self.sel else { return };
        if self.config.profiles[i].monitors.is_empty() {
            dialog::alert(self.hwnd, Icon::Info, "No monitors in this configuration", "Connect a monitor and use Add connected before adding a hotkey.");
            return;
        }
        let Some(b) = self.run_editor(i, None) else { return };
        let p = &mut self.config.profiles[i];
        p.bindings.push(b);
        if !p.enabled {
            // Adding a hotkey is a strong sign the configuration should be live.
            p.enabled = true;
            widgets::set_checked(self.c.enabled, true);
            self.refresh_profile_label();
            self.update_status();
        }
        self.commit();
        self.refresh_hotkeys();
    }

    fn edit_hotkey(&mut self, row: usize) {
        let Some(i) = self.sel else { return };
        let Some(existing) = self.config.profiles[i].bindings.get(row).cloned() else { return };
        let Some(b) = self.run_editor(i, Some(&existing)) else { return };
        self.config.profiles[i].bindings[row] = b;
        self.commit();
        self.refresh_hotkeys();
    }

    /// Removes a hotkey, once confirmed.
    fn remove_hotkey(&mut self, row: usize) {
        let Some(i) = self.sel else { return };
        let Some(b) = self.config.profiles[i].bindings.get(row) else { return };
        let msg = format!("The hotkey for {} ({}) will be removed.", self.monitor_name(&b.monitor_id), monitors::describe_action(&b.inputs));
        if !dialog::confirm(self.hwnd, &format!("Remove {}?", b.hotkey.display()), &msg, "Remove") {
            return;
        }
        self.config.profiles[i].bindings.remove(row);
        self.commit();
        self.refresh_hotkeys();
    }

    /// Runs the hotkey's action now (switches the monitor's input).
    fn test_hotkey(&mut self, row: usize) {
        let Some(i) = self.sel else { return };
        let Some(b) = self.config.profiles[i].bindings.get(row).cloned() else { return };
        let Some(m) = self.connected_monitor(&b.monitor_id).cloned() else {
            ui::set_text(self.c.activity, "That monitor is not connected.");
            return;
        };
        ui::set_text(self.c.activity, &format!("Testing {}…", b.hotkey.display()));
        let generation = MAIL.generation(); // if this window closes, its results are dropped
        std::thread::spawn(move || {
            let result = monitors::run_binding(&b.inputs, &m);
            MAIL.post_for(generation, Update::Tested { id: b.monitor_id, result });
        });
    }

    /// Shows the hotkey editor. The tray's hotkeys are paused while it's open, so a combination
    /// that's already in use can be recorded (or the editor used at all) without one firing.
    fn run_editor(&mut self, profile: usize, existing: Option<&Binding>) -> Option<Binding> {
        tray::post(tray::Event::Capturing(true));
        let result = hotkey_editor::run(self.hwnd, self.dpi, &self.config.profiles[profile], existing);
        tray::post(tray::Event::Capturing(false));
        result
    }
}
