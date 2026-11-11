using System.Drawing;
using System.Windows.Forms;
using InputSwitch.Config;
using InputSwitch.Hotkeys;
using InputSwitch.Monitors;
using InputSwitch.UI;
using Microsoft.Win32;
using Timer = System.Windows.Forms.Timer;

namespace InputSwitch;

/// <summary>
/// The tray icon and the app's brain: watches for display changes, picks the profile that
/// fits the connected monitors, and keeps that profile's hotkeys registered.
/// </summary>
internal sealed class TrayApp : ApplicationContext
{
    private readonly NotifyIcon _tray;
    private readonly SystemWindow _window;
    private readonly Timer _debounce;
    private readonly InputSwitcher _switcher = new();
    private readonly Icon _icon;

    private AppConfig _config;
    private List<MonitorInfo> _connected = [];
    private Profile? _active;
    private ISettingsWindow? _configForm;
    private Action? _balloonClick;

    public TrayApp(bool openSettings = false)
    {
        _config = ConfigStore.Load();
        bool firstRun = _config.Profiles.Count == 0;

        _icon = TrayIconFactory.Create();
        _tray = new NotifyIcon
        {
            Icon = _icon,
            Text = "InputSwitch",
            Visible = true,
            ContextMenuStrip = new ContextMenuStrip(),
        };
        _tray.ContextMenuStrip.Opening += (_, e) => { BuildMenu(_tray.ContextMenuStrip); e.Cancel = false; };
        _tray.MouseClick += (_, e) => { if (e.Button == MouseButtons.Left) OpenConfig(_active?.Id); };
        _tray.BalloonTipClicked += (_, _) => { var a = _balloonClick; _balloonClick = null; a?.Invoke(); };

        _window = new SystemWindow();
        _window.HotkeyPressed += OnHotkey;
        _window.DisplaysMayHaveChanged += ScheduleReevaluate;

        // Docking produces a burst of events as monitors come up one by one; wait for it to settle.
        _debounce = new Timer { Interval = 2500 };
        _debounce.Tick += (_, _) => { _debounce.Stop(); Evaluate(interactive: false); };

        SystemEvents.DisplaySettingsChanged += OnSystemDisplayEvent;
        SystemEvents.PowerModeChanged += OnPowerModeChanged;
        SystemEvents.SessionSwitch += OnSessionSwitch;

        Evaluate(interactive: firstRun);
        if (openSettings && _configForm is null) OpenConfig(_active?.Id);
    }

    // ------------------------------------------------------------------ setup detection

    private void OnSystemDisplayEvent(object? sender, EventArgs e) => ScheduleReevaluate();
    private void OnPowerModeChanged(object? sender, PowerModeChangedEventArgs e) { if (e.Mode == PowerModes.Resume) ScheduleReevaluate(); }
    private void OnSessionSwitch(object? sender, SessionSwitchEventArgs e) { if (e.Reason == SessionSwitchReason.SessionUnlock) ScheduleReevaluate(); }

    private void ScheduleReevaluate()
    {
        _debounce.Stop();
        _debounce.Start();
    }

    /// <summary>Re-reads the connected monitors and activates the matching profile, creating one if the setup is new.</summary>
    private void Evaluate(bool interactive)
    {
        _connected = MonitorService.Enumerate();
        var fingerprint = ProfileMatcher.Fingerprint(_connected);
        var match = ProfileMatcher.Match(_config, fingerprint, _active);

        Profile? created = null;
        if (match is null && fingerprint.Count > 0)
        {
            created = ProfileMatcher.CreateFor(_connected, _config);
            _config.Profiles.Add(created);
            Save();
            match = created;
        }

        bool changed = match?.Id != _active?.Id;
        _active = match;
        _configForm?.UpdateConnection(_connected, _active?.Id);

        if (_configForm is null)
            RegisterHotkeys(reportFailures: changed);
        UpdateTooltip();

        if (created is not null)
        {
            if (interactive)
            {
                OpenConfig(created.Id);
            }
            else
            {
                Notify("New display setup detected",
                    $"Created \"{created.Name}\" for {Describe(created)}. Click to name it and assign hotkeys.",
                    ToolTipIcon.Info, () => OpenConfig(created.Id));
            }
        }
        else if (changed && _active is not null)
        {
            Notify($"Display setup: {_active.Name}",
                _active.Enabled ? $"{_active.Bindings.Count} hotkey(s) active." : "This configuration is disabled. Click to configure.",
                ToolTipIcon.None, () => OpenConfig(_active.Id));
        }
        else if (interactive && _active is null)
        {
            OpenConfig(null);
        }
    }

    private static string Describe(Profile p) =>
        p.Monitors.Count == 1 ? p.Monitors[0].Name : $"{p.Monitors.Count} monitors";

    // ------------------------------------------------------------------ hotkeys

    private void RegisterHotkeys(bool reportFailures)
    {
        _window.UnregisterAll();
        if (_active is not { Enabled: true }) return;

        var failed = new List<string>();
        foreach (var b in _active.Bindings)
            if (!b.Hotkey.IsEmpty && !_window.Register(b.Hotkey, b.Id))
                failed.Add(b.Hotkey.ToString());

        if (reportFailures && failed.Count > 0)
            Notify("Some hotkeys could not be registered",
                $"{string.Join(", ", failed)} — probably in use by another application.", ToolTipIcon.Warning,
                () => OpenConfig(_active.Id));
    }

    private async void OnHotkey(string bindingId)
    {
        var binding = _active?.Bindings.FirstOrDefault(b => b.Id == bindingId);
        if (binding is null) return;
        await RunBinding(binding);
    }

    private async Task RunBinding(HotkeyBinding binding)
    {
        var monitor = _connected.FirstOrDefault(m => string.Equals(m.Id, binding.MonitorId, StringComparison.OrdinalIgnoreCase));
        if (monitor is null)
        {
            var name = _active?.FindMonitor(binding.MonitorId)?.Name ?? "The monitor";
            Notify("Monitor not connected", $"{name} is not currently connected to this computer.", ToolTipIcon.Warning);
            return;
        }

        var result = await _switcher.RunAsync(binding, monitor);
        if (!result.Success)
            Notify("Input switch failed", result.Message, ToolTipIcon.Warning);
    }

    // ------------------------------------------------------------------ configuration dialog

    private void OpenConfig(string? selectProfileId)
    {
        if (_configForm is not null)
        {
            _configForm.SelectProfile(selectProfileId);
            _configForm.BringToFront();
            return;
        }

        // Release the hotkeys while editing so they can be captured in the hotkey boxes.
        _window.UnregisterAll();

        _configForm = SettingsWindow.Create(ConfigStore.Clone(_config), _connected, _active?.Id, selectProfileId, _switcher);
        _configForm.Saved += edited =>
        {
            _config = edited;
            Save();
        };
        _configForm.Closed += (_, _) =>
        {
            _configForm = null;
            _active = null; // profiles may have been edited or deleted; re-match from scratch
            Evaluate(interactive: false); // also re-registers the hotkeys
        };
        _configForm.Show();
        _configForm.Activate();
    }

    private void Save()
    {
        try
        {
            ConfigStore.Save(_config);
        }
        catch (Exception ex)
        {
            Notify("Could not save configuration", ex.Message, ToolTipIcon.Error);
        }
    }

    // ------------------------------------------------------------------ tray menu

    private void BuildMenu(ContextMenuStrip menu)
    {
        menu.Items.Clear();

        string header = _active is null
            ? (ProfileMatcher.Fingerprint(_connected).Count == 0 ? "No external monitors" : "Unknown display setup")
            : $"Setup: {_active.Name}{(_active.Enabled ? "" : " (disabled)")}";
        menu.Items.Add(new ToolStripMenuItem(header) { Enabled = false });

        if (_active is not null)
        {
            foreach (var b in _active.Bindings)
            {
                var monitorName = _active.FindMonitor(b.MonitorId)?.Name ?? "?";
                var item = new ToolStripMenuItem($"{monitorName}: {b.DescribeAction()}")
                {
                    ShortcutKeyDisplayString = b.Hotkey.ToString(),
                };
                var binding = b;
                item.Click += async (_, _) => await RunBinding(binding);
                menu.Items.Add(item);
            }
        }

        var externals = _connected.Where(m => !m.IsInternal).ToList();
        if (externals.Count > 0)
        {
            menu.Items.Add(new ToolStripSeparator());
            foreach (var m in externals)
            {
                var pm = _active?.FindMonitor(m.Id);
                var sub = new ToolStripMenuItem($"Switch {pm?.Name ?? m.DisplayName} to");
                var inputs = pm?.SupportedInputs is { Count: > 0 } s ? s : InputSources.Common.ToList();
                foreach (var code in inputs)
                {
                    var monitor = m;
                    int input = code;
                    sub.DropDownItems.Add(InputSources.Name(code), null, async (_, _) =>
                    {
                        var r = await _switcher.SetAsync(monitor, input);
                        if (!r.Success) Notify("Input switch failed", r.Message, ToolTipIcon.Warning);
                    });
                }
                menu.Items.Add(sub);
            }
        }

        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add("Configure…", null, (_, _) => OpenConfig(_active?.Id)).Font = new Font(menu.Font, FontStyle.Bold);
        menu.Items.Add("Re-detect displays", null, (_, _) => Evaluate(interactive: false));

        var startup = new ToolStripMenuItem("Start with Windows") { Checked = Startup.IsEnabled };
        startup.Click += (_, _) => Startup.Set(!Startup.IsEnabled);
        menu.Items.Add(startup);

        menu.Items.Add("Open config folder", null, (_, _) =>
        {
            Directory.CreateDirectory(ConfigStore.Directory);
            System.Diagnostics.Process.Start("explorer.exe", ConfigStore.Directory);
        });

        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add(new ToolStripMenuItem($"InputSwitch {AppInfo.Version}") { Enabled = false });
        menu.Items.Add("Exit", null, (_, _) => ExitThread());
    }

    private void UpdateTooltip()
    {
        var text = _active is null ? "InputSwitch — no configuration" : $"InputSwitch — {_active.Name}{(_active.Enabled ? "" : " (disabled)")}";
        _tray.Text = text.Length > 63 ? text[..63] : text;
    }

    private void Notify(string title, string text, ToolTipIcon icon, Action? onClick = null)
    {
        // The setting silences routine "switched setup" messages; problems and new setups always show.
        if (!_config.ShowNotifications && icon == ToolTipIcon.None) return;
        _balloonClick = onClick;
        _tray.ShowBalloonTip(4000, title, text, icon);
    }

    protected override void ExitThreadCore()
    {
        SystemEvents.DisplaySettingsChanged -= OnSystemDisplayEvent;
        SystemEvents.PowerModeChanged -= OnPowerModeChanged;
        SystemEvents.SessionSwitch -= OnSessionSwitch;
        _configForm?.Close();
        _debounce.Dispose();
        _window.Dispose();
        _tray.Visible = false;
        _tray.Dispose();
        _icon.Dispose();
        base.ExitThreadCore();
    }
}
