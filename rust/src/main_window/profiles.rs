//! The configurations: the list on the left, the name box, the status banner, and creating and
//! deleting configurations.

use super::*;
use crate::matcher;
use crate::monitors::MonitorInfo;
use crate::ui::dialog::{self, Icon};
use crate::ui::lists;

pub(super) const UNNAMED: &str = "Unnamed setup";
pub(super) const CONNECTED: &str = "Connected";

impl MainWindow {
    pub(super) fn selected_id(&self) -> Option<String> {
        self.sel.and_then(|i| self.config.profiles.get(i)).map(|p| p.id.clone())
    }

    pub(super) fn is_active(&self, index: usize) -> bool {
        self.active.as_deref() == Some(self.config.profiles[index].id.as_str())
    }

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
            if self.is_active(i) { CONNECTED.to_string() } else { "Not connected".to_string() },
            if p.monitors.len() == 1 { "1 monitor".to_string() } else { format!("{} monitors", p.monitors.len()) },
        ];
        if !p.enabled {
            parts.push("hotkeys off".into());
        }
        let name = if p.name.trim().is_empty() { UNNAMED } else { p.name.as_str() };
        format!("{name}\t{}", parts.join(" · "))
    }

    /// Fills the list, selecting configuration `select` (or the first), and shows it.
    pub(super) fn refresh_profiles(&mut self, select: Option<&str>) {
        let labels: Vec<String> = (0..self.config.profiles.len()).map(|i| self.profile_label(i)).collect();
        self.sel = select
            .and_then(|id| self.config.profiles.iter().position(|p| p.id == id))
            .or(if labels.is_empty() { None } else { Some(0) });
        lists::lb_set(self.c.profiles, &labels, self.sel);
        self.show_profile();
    }

    pub(super) fn refresh_profile_label(&self) {
        if let Some(i) = self.sel {
            lists::lb_replace(self.c.profiles, i, &self.profile_label(i));
        }
    }

    pub(super) fn select_from_list(&mut self) {
        self.sel = lists::lb_selected(self.c.profiles);
        self.show_profile();
    }

    /// Shows the selected configuration on the right (or why there's none).
    fn show_profile(&mut self) {
        for h in self.c.detail() {
            ui::enable(h, self.sel.is_some());
        }
        let Some(i) = self.sel else {
            ui::set_text(self.c.name.edit, "");
            widgets::set_checked(self.c.enabled, false);
            let text = if matcher::fingerprint(&self.connected).is_empty() {
                "No external monitors are connected. Connect your monitors and a configuration will be created for them automatically."
            } else {
                "No configurations yet. Choose \"New from current setup\" to create one for the monitors connected now."
            };
            self.set_status(text, Severity::Informational);
            self.refresh_monitors();
            self.refresh_hotkeys();
            return;
        };
        let p = &self.config.profiles[i];
        ui::set_text(self.c.name.edit, &p.name);
        widgets::set_checked(self.c.enabled, p.enabled);
        self.update_status();
        self.refresh_monitors();
        self.refresh_hotkeys();
        self.detect(false);
    }

    /// The status banner: connected (with hotkeys on or off) or not.
    pub(super) fn update_status(&mut self) {
        let Some(i) = self.sel else { return };
        let p = &self.config.profiles[i];
        let present = p.monitors.iter().filter(|m| self.is_connected(&m.monitor_id)).count();
        let monitors = format!("({present} of {} monitors present)", p.monitors.len());
        let (text, severity) = match (self.is_active(i), p.enabled) {
            (true, true) => (format!("Connected now {monitors}. Hotkeys are active."), Severity::Success),
            (true, false) => (format!("Connected now {monitors}. Turn hotkeys on to use them."), Severity::Warning),
            (false, _) => ("Not connected now. Inputs can only be detected and tested while this setup is connected.".into(), Severity::Informational),
        };
        self.set_status(&text, severity);
    }

    /// Sets the status banner's message and tint (the label and checkbox on it take the tint too).
    fn set_status(&mut self, text: &str, severity: Severity) {
        ui::set_text(self.c.status, text);
        self.status = severity;
        theme::set_surface(self.c.status, theme::Surface::InfoBar(severity), false);
        theme::set_surface(self.c.enabled, theme::Surface::InfoBar(severity), false);
        let r = self.regions.banner;
        unsafe { RedrawWindow(self.hwnd, &r, 0, RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN) };
    }

    pub(super) fn rename_profile(&mut self) {
        let Some(i) = self.sel else { return };
        self.config.profiles[i].name = ui::get_text(self.c.name.edit);
        self.refresh_profile_label();
        self.commit_soon();
    }

    pub(super) fn toggle_enabled(&mut self) {
        let Some(i) = self.sel else { return };
        self.config.profiles[i].enabled = widgets::checked(self.c.enabled);
        self.refresh_profile_label();
        self.update_status();
        self.commit();
    }

    pub(super) fn new_from_current(&mut self) {
        let fp = matcher::fingerprint(&self.connected);
        if fp.is_empty() {
            dialog::alert(self.hwnd, Icon::Info, "No external monitors connected",
                "Connect the monitors for this setup, then try again. The laptop's own screen doesn't count.");
            return;
        }
        if let Some(existing) = self.config.profiles.iter().find(|p| matcher::ids(p) == fp) {
            let msg = format!("\"{}\" already covers exactly these monitors. When two configurations match, the first one in the list is used.", existing.name);
            if !dialog::confirm(self.hwnd, "Create another configuration?", &msg, "Create") {
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

    pub(super) fn delete_profile(&mut self) {
        let Some(i) = self.sel else { return };
        let p = &self.config.profiles[i];
        let hotkeys = if p.bindings.len() == 1 { "its hotkey".to_string() } else { format!("its {} hotkeys", p.bindings.len()) };
        let note = if self.is_active(i) { " This setup is connected now, so a fresh, empty configuration will be created for it when you close this window." } else { "" };
        let msg = format!("The configuration and {hotkeys} will be removed.{note}");
        if !dialog::confirm(self.hwnd, &format!("Delete \"{}\"?", p.name), &msg, "Delete") {
            return;
        }
        self.config.profiles.remove(i);
        self.commit();
        self.refresh_profiles(None);
    }
}
