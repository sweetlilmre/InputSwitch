using System.ComponentModel;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Interop;
using System.Windows.Media.Imaging;
using InputSwitch.Config;
using InputSwitch.Monitors;

namespace InputSwitch.UI;

/// <summary>
/// The configuration window: one configuration per display setup (left), with its monitors and
/// hotkeys (right). Works on a copy of the config; changes take effect on Save.
/// </summary>
internal sealed partial class ConfigWindow : Window, ISettingsWindow
{
    private readonly AppConfig _config;
    private readonly InputSwitcher _switcher;
    private List<MonitorInfo> _connected;
    private string? _activeId;

    private readonly List<ProfileRow> _rows = [];
    private readonly HashSet<string> _detectAttempted = new(StringComparer.OrdinalIgnoreCase);
    private readonly Dictionary<string, int?> _currentInputs = new(StringComparer.OrdinalIgnoreCase);
    private bool _detecting;
    private bool _dirty;
    private bool _loading;
    private bool _saved;

    public event Action<AppConfig>? Saved;

    public ConfigWindow(AppConfig config, List<MonitorInfo> connected, string? activeId, string? selectId, InputSwitcher switcher)
    {
        _config = config;
        _connected = connected;
        _activeId = activeId;
        _switcher = switcher;

        InitializeComponent();
        Icon = CreateWindowIcon();

        _loading = true;
        StartWithWindowsBox.IsChecked = Startup.IsEnabled;
        NotifyBox.IsChecked = _config.ShowNotifications;
        _loading = false;

        RefreshProfileList(selectId ?? activeId);
    }

    // ------------------------------------------------------------------ called by the tray app

    public void UpdateConnection(List<MonitorInfo> connected, string? activeId)
    {
        _connected = connected;
        _activeId = activeId;
        RefreshProfileList(SelectedProfile?.Id);
    }

    public void SelectProfile(string? id)
    {
        if (id is not null) RefreshProfileList(id);
    }

    public void BringToFront()
    {
        if (WindowState == WindowState.Minimized) WindowState = WindowState.Normal;
        Activate();
    }

    // ------------------------------------------------------------------ configurations

    private Profile? SelectedProfile => (ProfilesList.SelectedItem as ProfileRow)?.Profile;

    private bool IsConnectedProfile(Profile p) => p.Id == _activeId;

    private void RefreshProfileList(string? selectId)
    {
        _loading = true;
        _rows.Clear();
        _rows.AddRange(_config.Profiles.Select(p => new ProfileRow(p, IsConnectedProfile)));
        ProfilesList.ItemsSource = null;
        ProfilesList.ItemsSource = _rows;

        int index = _rows.FindIndex(r => r.Profile.Id == selectId);
        if (index < 0 && _rows.Count > 0) index = 0;
        ProfilesList.SelectedIndex = index;
        _loading = false;
        ShowProfile();
    }

    private void RefreshSelectedRow() => (ProfilesList.SelectedItem as ProfileRow)?.Refresh();

    private void OnProfileSelected(object sender, SelectionChangedEventArgs e)
    {
        if (!_loading) ShowProfile();
    }

    private void ShowProfile()
    {
        var p = SelectedProfile;
        Editor.Visibility = p is null ? Visibility.Collapsed : Visibility.Visible;
        EmptyState.Visibility = p is null ? Visibility.Visible : Visibility.Collapsed;

        if (p is null)
        {
            EmptyStateText.Text = ProfileMatcher.Fingerprint(_connected).Count == 0
                ? "No external monitors are connected. Connect your monitors and a configuration will be created for them automatically."
                : "No configurations yet. Choose \"New from current setup\" to create one for the monitors connected now.";
            return;
        }

        _loading = true;
        NameBox.Text = p.Name;
        EnabledBox.IsChecked = p.Enabled;
        _loading = false;

        UpdateStatusBanner();
        RefreshMonitors();
        RefreshHotkeys();
        _ = DetectInputs(force: false);
    }

    private void UpdateStatusBanner()
    {
        if (SelectedProfile is not { } p) return;
        bool connected = IsConnectedProfile(p);
        int present = p.Monitors.Count(m => IsConnected(m.MonitorId));

        StatusText.Text = connected
            ? (p.Enabled
                ? $"Connected now ({present} of {p.Monitors.Count} monitors present). Hotkeys are active."
                : $"Connected now ({present} of {p.Monitors.Count} monitors present). Turn hotkeys on to use them.")
            : "Not connected now. Inputs can only be detected and tested while this setup is connected.";
        StatusIcon.Text = connected ? (p.Enabled ? "" : "") : ""; // check / warning / info
        StatusBanner.SetResourceReference(Border.BackgroundProperty,
            connected ? (p.Enabled ? "SystemFillColorSuccessBackgroundBrush" : "SystemFillColorCautionBackgroundBrush")
                      : "SystemFillColorNeutralBackgroundBrush");
        StatusIcon.SetResourceReference(TextBlock.ForegroundProperty,
            connected ? (p.Enabled ? "SystemFillColorSuccessBrush" : "SystemFillColorCautionBrush") : "TextFillColorSecondaryBrush");
    }

    private void OnNameChanged(object sender, TextChangedEventArgs e)
    {
        if (_loading || SelectedProfile is not { } p) return;
        p.Name = NameBox.Text;
        _dirty = true;
        RefreshSelectedRow();
    }

    private void OnEnabledChanged(object sender, RoutedEventArgs e)
    {
        if (_loading || SelectedProfile is not { } p) return;
        p.Enabled = EnabledBox.IsChecked == true;
        _dirty = true;
        RefreshSelectedRow();
        UpdateStatusBanner();
    }

    private void OnNewFromCurrent(object sender, RoutedEventArgs e)
    {
        var fingerprint = ProfileMatcher.Fingerprint(_connected);
        if (fingerprint.Count == 0)
        {
            Info("No external monitors are connected.");
            return;
        }

        var existing = _config.Profiles.FirstOrDefault(p => p.MonitorIds.SetEquals(fingerprint));
        if (existing is not null &&
            !Ask($"\"{existing.Name}\" already covers exactly these monitors. Create another configuration anyway?\n\n" +
                 "(When two configurations match, the first one in the list is used.)"))
            return;

        var profile = ProfileMatcher.CreateFor(_connected, _config);
        _config.Profiles.Add(profile);
        _dirty = true;
        RefreshProfileList(profile.Id);
        NameBox.Focus();
        NameBox.SelectAll();
    }

    private void OnDeleteProfile(object sender, RoutedEventArgs e)
    {
        if (SelectedProfile is not { } p) return;
        var note = IsConnectedProfile(p)
            ? "\n\nThis setup is connected now, so a fresh, empty configuration will be created for it when you save."
            : "";
        if (!Ask($"Delete \"{p.Name}\" and its {p.Bindings.Count} hotkey(s)?{note}", MessageBoxImage.Warning)) return;

        _config.Profiles.Remove(p);
        _dirty = true;
        RefreshProfileList(null);
    }

    // ------------------------------------------------------------------ monitors

    private bool IsConnected(string monitorId) => Connected(monitorId) is not null;

    private MonitorInfo? Connected(string monitorId) =>
        _connected.FirstOrDefault(c => string.Equals(c.Id, monitorId, StringComparison.OrdinalIgnoreCase));

    private void RefreshMonitors()
    {
        MonitorsList.ItemsSource = (SelectedProfile?.Monitors ?? [])
            .Select(m => new MonitorRow(m, IsConnected(m.MonitorId),
                _currentInputs.TryGetValue(m.MonitorId, out var c) ? c : null,
                () => _dirty = true))
            .ToList();
    }

    private void OnMonitorNameCommitted(object sender, RoutedEventArgs e) => RefreshHotkeys();

    private void OnAddConnectedMonitors(object sender, RoutedEventArgs e)
    {
        if (SelectedProfile is not { } p) return;
        var ids = p.MonitorIds;
        var missing = _connected.Where(m => !m.IsInternal && !ids.Contains(m.Id)).ToList();
        if (missing.Count == 0)
        {
            Info("All connected external monitors are already in this configuration.");
            return;
        }

        foreach (var m in missing)
            p.Monitors.Add(new ProfileMonitor { MonitorId = m.Id, Name = m.DisplayName, Model = m.Model, Serial = m.Serial });
        _dirty = true;
        RefreshSelectedRow();
        UpdateStatusBanner();
        RefreshMonitors();
        _ = DetectInputs(force: false);
    }

    private void OnRemoveMonitor(object sender, RoutedEventArgs e)
    {
        if (SelectedProfile is not { } p || (sender as FrameworkElement)?.Tag is not MonitorRow row) return;
        var m = row.Monitor;
        int used = p.Bindings.Count(b => b.MonitorId == m.MonitorId);
        if (!Ask($"Remove {m.Name} from \"{p.Name}\"{(used > 0 ? $" along with its {used} hotkey(s)" : "")}?\n\n" +
                 "The configuration is recognised by its monitors, so removing one changes which setups it matches."))
            return;

        p.Monitors.Remove(m);
        p.Bindings.RemoveAll(b => b.MonitorId == m.MonitorId);
        _dirty = true;
        RefreshSelectedRow();
        UpdateStatusBanner();
        RefreshMonitors();
        RefreshHotkeys();
    }

    private void OnDetectInputs(object sender, RoutedEventArgs e) => _ = DetectInputs(force: true);

    /// <summary>Reads each connected monitor's capabilities and current input over DDC/CI (slow, so off the UI thread).</summary>
    private async Task DetectInputs(bool force)
    {
        if (_detecting || SelectedProfile is not { } profile) return;

        var targets = profile.Monitors
            .Select(pm => (pm, info: Connected(pm.MonitorId)))
            .Where(t => t.info is not null && (force || !_detectAttempted.Contains(t.pm.MonitorId)))
            .ToList();
        if (targets.Count == 0)
        {
            if (force) ActivityText.Text = "None of this configuration's monitors are connected.";
            return;
        }

        _detecting = true;
        try
        {
            foreach (var (pm, info) in targets)
            {
                _detectAttempted.Add(pm.MonitorId);
                ActivityText.Text = $"Reading {pm.Name}…";

                var (inputs, current) = await Task.Run(() => (MonitorService.GetSupportedInputs(info!), MonitorService.GetCurrentInput(info!)));
                if (!IsLoaded) return;

                if (inputs.Count > 0 && !inputs.SequenceEqual(pm.SupportedInputs))
                {
                    pm.SupportedInputs = inputs;
                    _dirty = true;
                }
                _currentInputs[pm.MonitorId] = current;
                if (SelectedProfile == profile) RefreshMonitors();
            }

            ActivityText.Text = targets.Any(t => t.pm.SupportedInputs.Count == 0)
                ? "Some monitors did not report their inputs. Check DDC/CI is on in the monitor's menu."
                : "Inputs detected.";
        }
        finally
        {
            _detecting = false;
        }
    }

    // ------------------------------------------------------------------ hotkeys

    private void RefreshHotkeys()
    {
        var p = SelectedProfile;
        var rows = (p?.Bindings ?? [])
            .Select(b => new HotkeyRow(b,
                b.Hotkey.ToString().Split(" + ").ToList(),
                p!.FindMonitor(b.MonitorId)?.Name ?? "(removed monitor)",
                b.DescribeAction()))
            .ToList();
        HotkeysList.ItemsSource = rows;
        NoHotkeys.Visibility = rows.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
    }

    private void OnAddHotkey(object sender, RoutedEventArgs e)
    {
        if (SelectedProfile is not { } p) return;
        if (p.Monitors.Count == 0)
        {
            Info("This configuration has no monitors.");
            return;
        }

        var editor = new BindingEditorWindow(p, null) { Owner = this };
        if (editor.ShowDialog() != true) return;

        p.Bindings.Add(editor.Result);
        if (!p.Enabled)
        {
            // Adding a hotkey is a strong sign the user wants this configuration live.
            p.Enabled = true;
            _loading = true; EnabledBox.IsChecked = true; _loading = false;
            RefreshSelectedRow();
            UpdateStatusBanner();
        }
        _dirty = true;
        RefreshHotkeys();
    }

    private void OnEditHotkey(object sender, RoutedEventArgs e)
    {
        if (SelectedProfile is not { } p || (sender as FrameworkElement)?.Tag is not HotkeyRow row) return;
        var editor = new BindingEditorWindow(p, row.Binding) { Owner = this };
        if (editor.ShowDialog() != true) return;

        int index = p.Bindings.IndexOf(row.Binding);
        p.Bindings[index] = editor.Result;
        _dirty = true;
        RefreshHotkeys();
    }

    private void OnRemoveHotkey(object sender, RoutedEventArgs e)
    {
        if (SelectedProfile is not { } p || (sender as FrameworkElement)?.Tag is not HotkeyRow row) return;
        p.Bindings.Remove(row.Binding);
        _dirty = true;
        RefreshHotkeys();
    }

    private async void OnTestHotkey(object sender, RoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.Tag is not HotkeyRow row) return;
        var b = row.Binding;
        var monitor = Connected(b.MonitorId);
        if (monitor is null)
        {
            ActivityText.Text = "That monitor is not connected.";
            return;
        }

        ActivityText.Text = $"Testing {b.Hotkey}…";
        var result = await _switcher.RunAsync(b, monitor);
        if (!IsLoaded) return;
        ActivityText.Text = result.Message;
        if (result.Success) _currentInputs[b.MonitorId] = result.Target;
        RefreshMonitors();
    }

    // ------------------------------------------------------------------ save / close

    private void OnOptionChanged(object sender, RoutedEventArgs e)
    {
        if (!_loading) _dirty = true;
    }

    private void OnSave(object sender, RoutedEventArgs e)
    {
        ApplySave();
        Close();
    }

    private void OnCancel(object sender, RoutedEventArgs e) => Close();

    private void ApplySave()
    {
        foreach (var p in _config.Profiles.Where(p => string.IsNullOrWhiteSpace(p.Name)))
            p.Name = "Unnamed setup";

        _config.ShowNotifications = NotifyBox.IsChecked == true;
        try
        {
            bool start = StartWithWindowsBox.IsChecked == true;
            if (start != Startup.IsEnabled) Startup.Set(start);
        }
        catch (Exception ex)
        {
            Info($"Could not change the startup setting: {ex.Message}", MessageBoxImage.Warning);
        }

        Saved?.Invoke(_config);
        _dirty = false;
        _saved = true;
    }

    protected override void OnClosing(CancelEventArgs e)
    {
        if (_dirty && !_saved)
        {
            var answer = MessageBox.Show(this, "Save your changes?", Title, MessageBoxButton.YesNoCancel, MessageBoxImage.Question);
            if (answer == MessageBoxResult.Cancel) { e.Cancel = true; return; }
            if (answer == MessageBoxResult.Yes) ApplySave();
        }
        base.OnClosing(e);
    }

    // ------------------------------------------------------------------ helpers

    private void Info(string text, MessageBoxImage image = MessageBoxImage.Information) =>
        MessageBox.Show(this, text, Title, MessageBoxButton.OK, image);

    private bool Ask(string text, MessageBoxImage image = MessageBoxImage.Question) =>
        MessageBox.Show(this, text, Title, MessageBoxButton.YesNo, image) == MessageBoxResult.Yes;

    internal static BitmapSource CreateWindowIcon()
    {
        using var icon = TrayIconFactory.Create();
        return Imaging.CreateBitmapSourceFromHIcon(icon.Handle, Int32Rect.Empty, BitmapSizeOptions.FromEmptyOptions());
    }
}
