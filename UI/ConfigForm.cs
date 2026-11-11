using InputSwitch.Config;
using InputSwitch.Monitors;

namespace InputSwitch.UI;

/// <summary>
/// The configuration window: one configuration per display setup (left), with its monitors and
/// hotkeys (right). Works on a copy of the config; changes take effect on Save.
/// </summary>
internal sealed class ConfigForm : Form
{
    private readonly AppConfig _config;
    private readonly InputSwitcher _switcher;
    private List<MonitorInfo> _connected;
    private string? _activeId;

    private readonly HashSet<string> _detectAttempted = new(StringComparer.OrdinalIgnoreCase);
    private readonly Dictionary<string, int?> _currentInputs = new(StringComparer.OrdinalIgnoreCase);
    private bool _detecting;
    private bool _dirty;
    private bool _loading;

    private readonly ListBox _profiles = new() { Dock = DockStyle.Fill, IntegralHeight = false };
    private readonly TextBox _name = new() { Dock = DockStyle.Fill };
    private readonly CheckBox _enabled = new() { Text = "Enabled — register this configuration's hotkeys when this setup is connected", AutoSize = true };
    private readonly Label _status = new() { AutoSize = true, Dock = DockStyle.Fill, Padding = new Padding(0, 4, 0, 4) };
    private readonly ListView _monitors = NewListView(labelEdit: true);
    private readonly ListView _bindings = NewListView(labelEdit: false);
    private readonly Label _activity = new() { AutoSize = true, ForeColor = SystemColors.GrayText, Anchor = AnchorStyles.Left };
    private readonly CheckBox _startWithWindows = new() { Text = "Start with Windows", AutoSize = true };
    private readonly CheckBox _showNotifications = new() { Text = "Notify when the setup changes", AutoSize = true };
    private readonly Panel _detail = new() { Dock = DockStyle.Fill };

    public event Action<AppConfig>? Saved;

    public ConfigForm(AppConfig config, List<MonitorInfo> connected, string? activeId, string? selectId, InputSwitcher switcher)
    {
        _config = config;
        _connected = connected;
        _activeId = activeId;
        _switcher = switcher;

        Text = "InputSwitch — Display configurations";
        Icon = TrayIconFactory.Create();
        StartPosition = FormStartPosition.CenterScreen;
        AutoScaleMode = AutoScaleMode.Dpi;
        ClientSize = new Size(920, 620);
        MinimumSize = new Size(760, 500);

        BuildLayout();

        _startWithWindows.Checked = Startup.IsEnabled;
        _showNotifications.Checked = _config.ShowNotifications;
        _startWithWindows.CheckedChanged += (_, _) => _dirty = true;
        _showNotifications.CheckedChanged += (_, _) => _dirty = true;

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

    // ------------------------------------------------------------------ layout

    private void BuildLayout()
    {
        var split = new SplitContainer { Dock = DockStyle.Fill, FixedPanel = FixedPanel.Panel1, SplitterDistance = 230 };

        // Left: list of configurations.
        var left = new TableLayoutPanel { Dock = DockStyle.Fill, RowCount = 3, Padding = new Padding(8, 8, 0, 8) };
        left.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        left.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        left.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        left.Controls.Add(new Label { Text = "Configurations", AutoSize = true, Font = new Font(Font, FontStyle.Bold) }, 0, 0);
        left.Controls.Add(_profiles, 0, 1);
        var leftButtons = new FlowLayoutPanel { AutoSize = true, Dock = DockStyle.Fill, FlowDirection = FlowDirection.TopDown };
        leftButtons.Controls.Add(MakeButton("New from current setup", (_, _) => NewFromCurrent(), 200));
        leftButtons.Controls.Add(MakeButton("Delete configuration", (_, _) => DeleteProfile(), 200));
        left.Controls.Add(leftButtons, 0, 2);
        split.Panel1.Controls.Add(left);

        // Right: details of the selected configuration.
        var right = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 2, RowCount = 5, Padding = new Padding(8) };
        right.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        right.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        right.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        right.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        right.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        right.RowStyles.Add(new RowStyle(SizeType.Percent, 45));
        right.RowStyles.Add(new RowStyle(SizeType.Percent, 55));

        right.Controls.Add(new Label { Text = "Name:", AutoSize = true, Anchor = AnchorStyles.Left }, 0, 0);
        right.Controls.Add(_name, 1, 0);
        right.Controls.Add(_enabled, 1, 1);
        right.Controls.Add(_status, 0, 2);
        right.SetColumnSpan(_status, 2);

        var monitorsGroup = Group("Monitors in this configuration", _monitors,
            MakeButton("Rename", (_, _) => RenameMonitor()),
            MakeButton("Detect inputs", (_, _) => _ = DetectInputs(force: true)),
            MakeButton("Add connected monitors", (_, _) => AddConnectedMonitors(), 170),
            MakeButton("Remove", (_, _) => RemoveMonitor()));
        right.Controls.Add(monitorsGroup, 0, 3);
        right.SetColumnSpan(monitorsGroup, 2);

        var bindingsGroup = Group("Hotkeys", _bindings,
            MakeButton("Add…", (_, _) => AddBinding()),
            MakeButton("Edit…", (_, _) => EditBinding()),
            MakeButton("Remove", (_, _) => RemoveBinding()),
            MakeButton("Test", (_, _) => _ = TestBinding()));
        right.Controls.Add(bindingsGroup, 0, 4);
        right.SetColumnSpan(bindingsGroup, 2);

        _detail.Controls.Add(right);
        split.Panel2.Controls.Add(_detail);

        _monitors.Columns.Add("Name", 170);
        _monitors.Columns.Add("Model", 80);
        _monitors.Columns.Add("Serial", 110);
        _monitors.Columns.Add("Status", 95);
        _monitors.Columns.Add("Current input", 105);
        _monitors.Columns.Add("Available inputs", 260);
        _monitors.AfterLabelEdit += OnMonitorRenamed;

        _bindings.Columns.Add("Hotkey", 150);
        _bindings.Columns.Add("Monitor", 170);
        _bindings.Columns.Add("Action", 320);
        _bindings.DoubleClick += (_, _) => EditBinding();

        // Bottom: global options and Save/Cancel.
        var bottom = new TableLayoutPanel { Dock = DockStyle.Bottom, AutoSize = true, ColumnCount = 5, Padding = new Padding(8, 4, 8, 8) };
        bottom.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        bottom.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        bottom.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        bottom.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        bottom.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        bottom.Controls.Add(_startWithWindows, 0, 0);
        bottom.Controls.Add(_showNotifications, 1, 0);
        bottom.Controls.Add(_activity, 2, 0);
        var save = MakeButton("Save", (_, _) => SaveAndClose());
        var cancel = MakeButton("Cancel", (_, _) => Close());
        bottom.Controls.Add(save, 3, 0);
        bottom.Controls.Add(cancel, 4, 0);

        Controls.Add(split);
        Controls.Add(bottom);

        _profiles.SelectedIndexChanged += (_, _) => ShowProfile();
        _name.TextChanged += (_, _) =>
        {
            if (_loading || SelectedProfile is not { } p) return;
            p.Name = _name.Text;
            _dirty = true;
            RefreshProfileItemText();
        };
        _enabled.CheckedChanged += (_, _) =>
        {
            if (_loading || SelectedProfile is not { } p) return;
            p.Enabled = _enabled.Checked;
            _dirty = true;
            RefreshProfileItemText();
        };
    }

    private static GroupBox Group(string title, Control body, params Button[] buttons)
    {
        var group = new GroupBox { Text = title, Dock = DockStyle.Fill, Padding = new Padding(6) };
        var table = new TableLayoutPanel { Dock = DockStyle.Fill, RowCount = 2 };
        table.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        table.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        table.Controls.Add(body, 0, 0);
        var row = new FlowLayoutPanel { AutoSize = true, Dock = DockStyle.Fill, Margin = Padding.Empty };
        row.Controls.AddRange(buttons);
        table.Controls.Add(row, 0, 1);
        group.Controls.Add(table);
        return group;
    }

    private static ListView NewListView(bool labelEdit) => new()
    {
        Dock = DockStyle.Fill,
        View = View.Details,
        FullRowSelect = true,
        MultiSelect = false,
        HideSelection = false,
        LabelEdit = labelEdit,
        HeaderStyle = ColumnHeaderStyle.Nonclickable,
    };

    private static Button MakeButton(string text, EventHandler onClick, int minWidth = 90)
    {
        var b = new Button { Text = text, AutoSize = true, MinimumSize = new Size(minWidth, 0) };
        b.Click += onClick;
        return b;
    }

    // ------------------------------------------------------------------ profile list

    private sealed class ProfileItem(ConfigForm owner, Profile profile)
    {
        public Profile Profile { get; } = profile;
        public override string ToString()
        {
            var tags = new List<string>();
            if (Profile.Id == owner._activeId) tags.Add("connected");
            if (!Profile.Enabled) tags.Add("disabled");
            return tags.Count == 0 ? Profile.Name : $"{Profile.Name}  ({string.Join(", ", tags)})";
        }
    }

    private Profile? SelectedProfile => (_profiles.SelectedItem as ProfileItem)?.Profile;

    private void RefreshProfileList(string? selectId)
    {
        _loading = true;
        _profiles.BeginUpdate();
        _profiles.Items.Clear();
        foreach (var p in _config.Profiles)
            _profiles.Items.Add(new ProfileItem(this, p));
        _profiles.EndUpdate();

        int index = _config.Profiles.FindIndex(p => p.Id == selectId);
        if (index < 0 && _profiles.Items.Count > 0) index = 0;
        _profiles.SelectedIndex = index;
        _loading = false;
        ShowProfile();
    }

    private void RefreshProfileItemText()
    {
        int i = _profiles.SelectedIndex;
        if (i < 0) return;
        _loading = true;
        _profiles.Items[i] = _profiles.Items[i];
        _loading = false;
    }

    private void ShowProfile()
    {
        if (_loading) return;
        var p = SelectedProfile;
        _detail.Enabled = p is not null;

        _loading = true;
        _name.Text = p?.Name ?? "";
        _enabled.Checked = p?.Enabled ?? false;
        _loading = false;

        if (p is null)
        {
            _status.Text = ProfileMatcher.Fingerprint(_connected).Count == 0
                ? "No external monitors are connected. Connect your monitors and a configuration will be created automatically."
                : "No configurations yet. Click \"New from current setup\".";
        }
        else
        {
            int present = p.Monitors.Count(m => IsConnected(m.MonitorId));
            _status.Text = p.Id == _activeId
                ? $"This is the display setup connected now ({present} of {p.Monitors.Count} monitors present)."
                : "This display setup is not connected now. Monitor inputs can only be detected and tested while connected.";
            _status.ForeColor = p.Id == _activeId ? Color.DarkGreen : SystemColors.ControlText;
        }

        RefreshMonitors();
        RefreshBindings();
        _ = DetectInputs(force: false);
    }

    private bool IsConnected(string monitorId) =>
        _connected.Any(c => string.Equals(c.Id, monitorId, StringComparison.OrdinalIgnoreCase));

    private MonitorInfo? Connected(string monitorId) =>
        _connected.FirstOrDefault(c => string.Equals(c.Id, monitorId, StringComparison.OrdinalIgnoreCase));

    private void NewFromCurrent()
    {
        var fingerprint = ProfileMatcher.Fingerprint(_connected);
        if (fingerprint.Count == 0)
        {
            MessageBox.Show(this, "No external monitors are connected.", Text, MessageBoxButtons.OK, MessageBoxIcon.Information);
            return;
        }

        var existing = _config.Profiles.FirstOrDefault(p => p.MonitorIds.SetEquals(fingerprint));
        if (existing is not null &&
            MessageBox.Show(this, $"\"{existing.Name}\" already covers exactly these monitors. Create another configuration anyway?\n\n" +
                                  "(When two configurations match, the first one in the list is used.)",
                Text, MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes)
            return;

        var profile = ProfileMatcher.CreateFor(_connected, _config);
        _config.Profiles.Add(profile);
        _dirty = true;
        RefreshProfileList(profile.Id);
        _name.Focus();
        _name.SelectAll();
    }

    private void DeleteProfile()
    {
        if (SelectedProfile is not { } p) return;
        var note = p.Id == _activeId
            ? "\n\nThis setup is connected now, so a fresh, empty configuration will be created for it when you save."
            : "";
        if (MessageBox.Show(this, $"Delete \"{p.Name}\" and its {p.Bindings.Count} hotkey(s)?{note}", Text,
                MessageBoxButtons.YesNo, MessageBoxIcon.Warning) != DialogResult.Yes)
            return;

        _config.Profiles.Remove(p);
        _dirty = true;
        RefreshProfileList(null);
    }

    // ------------------------------------------------------------------ monitors

    private void RefreshMonitors()
    {
        _monitors.BeginUpdate();
        _monitors.Items.Clear();
        foreach (var m in SelectedProfile?.Monitors ?? [])
        {
            bool connected = IsConnected(m.MonitorId);
            string current = _currentInputs.TryGetValue(m.MonitorId, out var c)
                ? (c is null ? "no reply" : InputSources.ShortName(c.Value)) : "";
            string inputs = m.SupportedInputs.Count > 0
                ? string.Join(", ", m.SupportedInputs.Select(InputSources.ShortName))
                : connected ? "not detected yet" : "";

            var item = new ListViewItem([m.Name, m.Model, m.Serial ?? "", connected ? "Connected" : "Not connected", current, inputs])
            {
                Tag = m,
                ForeColor = connected ? SystemColors.WindowText : SystemColors.GrayText,
            };
            _monitors.Items.Add(item);
        }
        _monitors.EndUpdate();
    }

    private ProfileMonitor? SelectedMonitor =>
        _monitors.SelectedItems.Count > 0 ? _monitors.SelectedItems[0].Tag as ProfileMonitor : null;

    private void RenameMonitor()
    {
        if (_monitors.SelectedItems.Count == 0 && _monitors.Items.Count > 0) _monitors.Items[0].Selected = true;
        if (_monitors.SelectedItems.Count > 0) _monitors.SelectedItems[0].BeginEdit();
    }

    private void OnMonitorRenamed(object? sender, LabelEditEventArgs e)
    {
        if (string.IsNullOrWhiteSpace(e.Label)) { e.CancelEdit = true; return; }
        if (_monitors.Items[e.Item].Tag is ProfileMonitor m)
        {
            m.Name = e.Label.Trim();
            _dirty = true;
            BeginInvoke(RefreshBindings);
        }
    }

    private void AddConnectedMonitors()
    {
        if (SelectedProfile is not { } p) return;
        var ids = p.MonitorIds;
        var missing = _connected.Where(m => !m.IsInternal && !ids.Contains(m.Id)).ToList();
        if (missing.Count == 0)
        {
            MessageBox.Show(this, "All connected external monitors are already in this configuration.", Text,
                MessageBoxButtons.OK, MessageBoxIcon.Information);
            return;
        }

        foreach (var m in missing)
            p.Monitors.Add(new ProfileMonitor { MonitorId = m.Id, Name = m.DisplayName, Model = m.Model, Serial = m.Serial });
        _dirty = true;
        RefreshMonitors();
        _ = DetectInputs(force: false);
    }

    private void RemoveMonitor()
    {
        if (SelectedProfile is not { } p || SelectedMonitor is not { } m) return;
        int used = p.Bindings.Count(b => b.MonitorId == m.MonitorId);
        if (MessageBox.Show(this,
                $"Remove {m.Name} from \"{p.Name}\"{(used > 0 ? $" along with its {used} hotkey(s)" : "")}?\n\n" +
                "The configuration is recognised by its monitors, so removing one changes which setups it matches.",
                Text, MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes)
            return;

        p.Monitors.Remove(m);
        p.Bindings.RemoveAll(b => b.MonitorId == m.MonitorId);
        _dirty = true;
        RefreshMonitors();
        RefreshBindings();
    }

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
            if (force) _activity.Text = "None of this configuration's monitors are connected.";
            return;
        }

        _detecting = true;
        try
        {
            foreach (var (pm, info) in targets)
            {
                _detectAttempted.Add(pm.MonitorId);
                _activity.Text = $"Reading {pm.Name} over DDC/CI…";

                var (inputs, current) = await Task.Run(() => (MonitorService.GetSupportedInputs(info!), MonitorService.GetCurrentInput(info!)));
                if (IsDisposed) return;

                if (inputs.Count > 0 && !inputs.SequenceEqual(pm.SupportedInputs))
                {
                    pm.SupportedInputs = inputs;
                    _dirty = true;
                }
                _currentInputs[pm.MonitorId] = current;
                RefreshMonitors();
            }

            bool anyFailed = targets.Any(t => t.pm.SupportedInputs.Count == 0);
            _activity.Text = anyFailed
                ? "Some monitors did not report their inputs. Check DDC/CI is on in the monitor's menu; common inputs will be offered instead."
                : "Inputs detected.";
        }
        finally
        {
            _detecting = false;
        }
    }

    // ------------------------------------------------------------------ hotkeys

    private void RefreshBindings()
    {
        var p = SelectedProfile;
        _bindings.BeginUpdate();
        _bindings.Items.Clear();
        foreach (var b in p?.Bindings ?? [])
        {
            var monitorName = p!.FindMonitor(b.MonitorId)?.Name ?? "(removed monitor)";
            _bindings.Items.Add(new ListViewItem([b.Hotkey.ToString(), monitorName, b.DescribeAction()]) { Tag = b });
        }
        _bindings.EndUpdate();
    }

    private HotkeyBinding? SelectedBinding =>
        _bindings.SelectedItems.Count > 0 ? _bindings.SelectedItems[0].Tag as HotkeyBinding : null;

    private void AddBinding()
    {
        if (SelectedProfile is not { } p) return;
        if (p.Monitors.Count == 0)
        {
            MessageBox.Show(this, "This configuration has no monitors.", Text, MessageBoxButtons.OK, MessageBoxIcon.Information);
            return;
        }

        using var editor = new BindingEditorForm(p, null);
        if (editor.ShowDialog(this) != DialogResult.OK) return;
        p.Bindings.Add(editor.Result);
        if (!p.Enabled)
        {
            // Adding a hotkey is a strong sign the user wants this configuration live.
            p.Enabled = true;
            _loading = true; _enabled.Checked = true; _loading = false;
            RefreshProfileItemText();
        }
        _dirty = true;
        RefreshBindings();
    }

    private void EditBinding()
    {
        if (SelectedProfile is not { } p || SelectedBinding is not { } b) return;
        using var editor = new BindingEditorForm(p, b);
        if (editor.ShowDialog(this) != DialogResult.OK) return;
        int index = p.Bindings.IndexOf(b);
        p.Bindings[index] = editor.Result;
        _dirty = true;
        RefreshBindings();
    }

    private void RemoveBinding()
    {
        if (SelectedProfile is not { } p || SelectedBinding is not { } b) return;
        p.Bindings.Remove(b);
        _dirty = true;
        RefreshBindings();
    }

    private async Task TestBinding()
    {
        if (SelectedBinding is not { } b) return;
        var monitor = Connected(b.MonitorId);
        if (monitor is null)
        {
            _activity.Text = "That monitor is not connected.";
            return;
        }

        _activity.Text = $"Testing {b.Hotkey}…";
        var result = await _switcher.RunAsync(b, monitor);
        if (IsDisposed) return;
        _activity.Text = result.Message;
        if (result.Success) _currentInputs[b.MonitorId] = result.Target;
        RefreshMonitors();
    }

    // ------------------------------------------------------------------ save / close

    private void SaveAndClose()
    {
        ApplySave();
        Close();
    }

    private void ApplySave()
    {
        foreach (var p in _config.Profiles.Where(p => string.IsNullOrWhiteSpace(p.Name)))
            p.Name = "Unnamed setup";

        _config.ShowNotifications = _showNotifications.Checked;
        try
        {
            if (_startWithWindows.Checked != Startup.IsEnabled) Startup.Set(_startWithWindows.Checked);
        }
        catch (Exception ex)
        {
            MessageBox.Show(this, $"Could not change the startup setting: {ex.Message}", Text, MessageBoxButtons.OK, MessageBoxIcon.Warning);
        }

        Saved?.Invoke(_config);
        _dirty = false;
    }

    protected override void OnFormClosing(FormClosingEventArgs e)
    {
        if (_dirty && e.CloseReason == CloseReason.UserClosing)
        {
            var answer = MessageBox.Show(this, "Save your changes?", Text, MessageBoxButtons.YesNoCancel, MessageBoxIcon.Question);
            if (answer == DialogResult.Cancel) { e.Cancel = true; return; }
            if (answer == DialogResult.Yes) ApplySave();
        }
        base.OnFormClosing(e);
    }
}
