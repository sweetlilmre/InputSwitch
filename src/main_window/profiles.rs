//! The configurations: the list on the left, the name box, the status banner, and creating and
//! deleting configurations.

use super::*;
use crate::i18n::{t, tf};
use crate::matcher;
use crate::monitors::MonitorInfo;
use crate::ui::dialog::{self, Icon};
use crate::ui::lists;

impl MainWindow {
    /// The id of the configuration shown.
    pub(super) fn selected_id(&self) -> Option<String> {
        self.sel.and_then(|i| self.config.profiles.get(i)).map(|p| p.id.clone())
    }

    /// Whether configuration `index` is the one matching the connected monitors.
    pub(super) fn is_active(&self, index: usize) -> bool {
        self.active.as_deref() == Some(self.config.profiles[index].id.as_str())
    }

    /// The connected monitor with this id (compared ignoring case), if it's connected.
    pub(super) fn connected_monitor(&self, monitor_id: &str) -> Option<&MonitorInfo> {
        self.connected.iter().find(|m| m.id.eq_ignore_ascii_case(monitor_id))
    }

    pub(super) fn is_connected(&self, monitor_id: &str) -> bool {
        self.connected_monitor(monitor_id).is_some()
    }

    /// "Name\tConnected · 2 monitors · hotkeys off", drawn as two lines by draw_profile.
    fn profile_label(&self, i: usize) -> String {
        let p = &self.config.profiles[i];
        let mut parts = vec![
            if self.is_active(i) { t("common.connected") } else { t("common.not_connected") }.to_string(),
            if p.monitors.len() == 1 { t("profile.one_monitor").to_string() } else { tf("profile.monitors", &[("count", &p.monitors.len())]) },
        ];
        if !p.enabled {
            parts.push(t("profile.hotkeys_off").into());
        }
        let name = if p.name.trim().is_empty() { t("profile.unnamed") } else { p.name.as_str() };
        format!("{name}\t{}", parts.join(" · "))
    }

    /// Fills the list, selecting configuration `select` (or the first), and shows it.
    pub(super) fn refresh_profiles(&mut self, select: Option<&str>) {
        let labels: Vec<String> = (0..self.config.profiles.len()).map(|i| self.profile_label(i)).collect();
        self.sel = select
            .and_then(|id| self.config.profiles.iter().position(|p| p.id == id))
            .or((!labels.is_empty()).then_some(0));
        lists::lb_set(self.c.profiles, &labels, self.sel);
        self.show_profile();
    }

    /// Updates the selected configuration's item in the list (after its name or status changed).
    pub(super) fn refresh_profile_label(&self) {
        if let Some(i) = self.sel {
            lists::lb_replace(self.c.profiles, i, &self.profile_label(i));
        }
    }

    /// A configuration was clicked in the list.
    pub(super) fn select_from_list(&mut self) {
        self.sel = lists::lb_selected(self.c.profiles);
        self.show_profile();
    }

    /// Shows the selected configuration on the right (or why there's none).
    fn show_profile(&mut self) {
        for h in self.c.detail() {
            ui::enable(h, self.sel.is_some());
        }
        match self.sel {
            Some(i) => {
                let p = &self.config.profiles[i];
                ui::set_text(self.c.name.edit, &p.name);
                widgets::set_checked(self.c.enabled, p.enabled);
                self.update_status();
            }
            None => {
                ui::set_text(self.c.name.edit, "");
                widgets::set_checked(self.c.enabled, false);
                let text = if matcher::fingerprint(&self.connected).is_empty() {
                    t("status.no_external")
                } else {
                    t("status.no_configurations")
                };
                self.set_status(text, Severity::Informational);
            }
        }
        self.refresh_monitors();
        self.refresh_hotkeys();
        self.detect(false); // does nothing when no configuration is shown
    }

    /// The status banner: connected (with hotkeys on or off) or not.
    pub(super) fn update_status(&mut self) {
        let Some(i) = self.sel else { return };
        let p = &self.config.profiles[i];
        let present = p.monitors.iter().filter(|m| self.is_connected(&m.monitor_id)).count();
        let monitors = tf("status.present", &[("present", &present), ("count", &p.monitors.len())]);
        let (text, severity) = match (self.is_active(i), p.enabled) {
            (true, true) => (tf("status.active", &[("present", &monitors)]), Severity::Success),
            (true, false) => (tf("status.hotkeys_off", &[("present", &monitors)]), Severity::Warning),
            (false, _) => (t("status.not_connected").into(), Severity::Informational),
        };
        self.set_status(&text, severity);
    }

    /// Sets the status banner's message and tint (the label and checkbox on it take the tint too).
    fn set_status(&mut self, text: &str, severity: Severity) {
        ui::set_text(self.c.status, text);
        self.status = severity;
        self.share_background();
        theme::set_surface(self.c.status, theme::Surface::InfoBar(severity), false);
        theme::set_surface(self.c.enabled, theme::Surface::InfoBar(severity), false);
        let r = self.regions.banner;
        unsafe { RedrawWindow(self.hwnd, &r, 0, RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN) };
    }

    /// The name box was typed in: keep the name, and save shortly.
    pub(super) fn rename_profile(&mut self) {
        let Some(i) = self.sel else { return };
        self.config.profiles[i].name = ui::get_text(self.c.name.edit);
        self.refresh_profile_label();
        self.commit_soon();
    }

    /// "Hotkeys on" was clicked.
    pub(super) fn toggle_enabled(&mut self) {
        let Some(i) = self.sel else { return };
        self.config.profiles[i].enabled = widgets::checked(self.c.enabled);
        self.refresh_profile_label();
        self.update_status();
        self.commit();
    }

    /// "New from current setup": a configuration for the monitors connected now, ready to name.
    pub(super) fn new_from_current(&mut self) {
        let fp = matcher::fingerprint(&self.connected);
        if fp.is_empty() {
            dialog::alert(self.hwnd, Icon::Info, t("profile.no_external"), t("profile.no_external_detail"));
            return;
        }
        if let Some(existing) = self.config.profiles.iter().find(|p| matcher::ids(p) == fp) {
            let msg = tf("profile.duplicate_detail", &[("name", &existing.name)]);
            if !dialog::confirm(self.hwnd, t("profile.duplicate"), &msg, t("common.create")) {
                return;
            }
        }
        let p = matcher::create_for(&self.connected, &self.config);
        let id = p.id.clone();
        self.config.profiles.push(p);
        self.commit();
        self.refresh_profiles(Some(&id));
        unsafe {
            SetFocus(self.c.name.edit); // ready to name it
            SendMessageW(self.c.name.edit, EM_SETSEL, 0, -1);
        }
    }

    /// "Delete configuration", once confirmed.
    pub(super) fn delete_profile(&mut self) {
        let Some(i) = self.sel else { return };
        let p = &self.config.profiles[i];
        let mut msg = match p.bindings.len() {
            1 => t("profile.delete_one_hotkey").to_string(),
            n => tf("profile.delete_hotkeys", &[("count", &n)]),
        };
        if self.is_active(i) {
            msg = format!("{msg} {}", t("profile.delete_active_note"));
        }
        if !dialog::confirm(self.hwnd, &tf("profile.delete", &[("name", &p.name)]), &msg, t("common.delete")) {
            return;
        }
        self.config.profiles.remove(i);
        self.commit();
        self.refresh_profiles(None);
    }
}
