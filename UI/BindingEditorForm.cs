using InputSwitch.Config;
using InputSwitch.Hotkeys;
using InputSwitch.Monitors;

namespace InputSwitch.UI;

/// <summary>Edits one hotkey: which monitor, which key combination, and which inputs to switch between.</summary>
internal sealed class BindingEditorForm : Form
{
    private readonly Profile _profile;
    private readonly string? _editingId;
    private readonly ComboBox _monitor = new() { DropDownStyle = ComboBoxStyle.DropDownList, Dock = DockStyle.Fill };
    private readonly HotkeyBox _hotkey = new() { Dock = DockStyle.Fill };
    private readonly CheckedListBox _inputs = new() { Dock = DockStyle.Fill, CheckOnClick = true, IntegralHeight = false };
    private readonly TextBox _customCode = new() { Width = 60, PlaceholderText = "e.g. 1B" };
    private readonly Label _summary = new() { AutoSize = true, ForeColor = SystemColors.GrayText, Dock = DockStyle.Fill };

    public HotkeyBinding Result { get; private set; } = new();

    private sealed record InputItem(int Code)
    {
        public override string ToString() => InputSources.Name(Code);
    }

    public BindingEditorForm(Profile profile, HotkeyBinding? existing)
    {
        _profile = profile;
        _editingId = existing?.Id;

        Text = existing is null ? "Add hotkey" : "Edit hotkey";
        FormBorderStyle = FormBorderStyle.FixedDialog;
        MaximizeBox = MinimizeBox = false;
        StartPosition = FormStartPosition.CenterParent;
        AutoScaleMode = AutoScaleMode.Dpi;
        ClientSize = new Size(460, 440);
        ShowInTaskbar = false;

        var layout = new TableLayoutPanel { Dock = DockStyle.Fill, Padding = new Padding(10), ColumnCount = 2, RowCount = 7 };
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        layout.RowStyles.Add(new RowStyle(SizeType.AutoSize));

        layout.Controls.Add(Caption("Monitor:"), 0, 0);
        layout.Controls.Add(_monitor, 1, 0);
        layout.Controls.Add(Caption("Hotkey:"), 0, 1);
        layout.Controls.Add(_hotkey, 1, 1);

        var help = new Label
        {
            Text = "Tick the inputs to use. One input: the hotkey switches to it. Two: it toggles between them. " +
                   "More: it cycles through them in the order shown.",
            AutoSize = true, MaximumSize = new Size(430, 0), Margin = new Padding(3, 10, 3, 3),
        };
        layout.Controls.Add(help, 0, 2);
        layout.SetColumnSpan(help, 2);

        var inputsRow = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 2, Margin = Padding.Empty };
        inputsRow.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        inputsRow.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        inputsRow.Controls.Add(_inputs, 0, 0);
        var moveButtons = new FlowLayoutPanel { FlowDirection = FlowDirection.TopDown, AutoSize = true, Margin = Padding.Empty };
        moveButtons.Controls.Add(MakeButton("Move up", (_, _) => MoveItem(-1)));
        moveButtons.Controls.Add(MakeButton("Move down", (_, _) => MoveItem(+1)));
        inputsRow.Controls.Add(moveButtons, 1, 0);
        layout.Controls.Add(inputsRow, 0, 3);
        layout.SetColumnSpan(inputsRow, 2);

        var custom = new FlowLayoutPanel { AutoSize = true, Dock = DockStyle.Fill, Margin = Padding.Empty };
        custom.Controls.Add(new Label { Text = "Other input code (hex):", AutoSize = true, Margin = new Padding(3, 7, 3, 3) });
        custom.Controls.Add(_customCode);
        custom.Controls.Add(MakeButton("Add", (_, _) => AddCustom()));
        layout.Controls.Add(custom, 0, 4);
        layout.SetColumnSpan(custom, 2);

        layout.Controls.Add(_summary, 0, 5);
        layout.SetColumnSpan(_summary, 2);

        var buttons = new FlowLayoutPanel { FlowDirection = FlowDirection.RightToLeft, AutoSize = true, Dock = DockStyle.Fill };
        var cancel = new Button { Text = "Cancel", DialogResult = DialogResult.Cancel, AutoSize = true };
        var ok = MakeButton("OK", (_, _) => Accept());
        buttons.Controls.Add(cancel);
        buttons.Controls.Add(ok);
        layout.Controls.Add(buttons, 0, 6);
        layout.SetColumnSpan(buttons, 2);

        Controls.Add(layout);
        AcceptButton = ok;
        CancelButton = cancel;

        foreach (var m in profile.Monitors) _monitor.Items.Add(new MonitorItem(m));
        _monitor.SelectedIndexChanged += (_, _) => FillInputs(existing?.MonitorId == SelectedMonitor?.MonitorId ? existing?.Inputs : null);
        // ItemCheck fires before the new state is applied, so refresh afterwards. It also fires while
        // FillInputs populates the list in the constructor, before there's a handle; FillInputs
        // refreshes the summary itself in that case.
        _inputs.ItemCheck += (_, _) => { if (IsHandleCreated) BeginInvoke(UpdateSummary); };

        if (existing is not null)
        {
            _hotkey.Hotkey = existing.Hotkey;
            _monitor.SelectedIndex = Math.Max(0, profile.Monitors.FindIndex(m => m.MonitorId == existing.MonitorId));
        }
        else if (_monitor.Items.Count > 0)
        {
            _monitor.SelectedIndex = 0;
        }
    }

    private sealed record MonitorItem(ProfileMonitor Monitor)
    {
        public override string ToString() => $"{Monitor.Name}  ({Monitor.Model})";
    }

    private ProfileMonitor? SelectedMonitor => (_monitor.SelectedItem as MonitorItem)?.Monitor;

    private void FillInputs(List<int>? selected)
    {
        var monitor = SelectedMonitor;
        _inputs.Items.Clear();
        if (monitor is null) return;

        var ordered = new List<int>();
        if (selected is not null) ordered.AddRange(selected);
        var available = monitor.SupportedInputs.Count > 0 ? monitor.SupportedInputs : InputSources.Common.ToList();
        ordered.AddRange(available.Where(c => !ordered.Contains(c)));

        foreach (var code in ordered)
            _inputs.Items.Add(new InputItem(code), selected?.Contains(code) == true);

        UpdateSummary();
    }

    private void MoveItem(int delta)
    {
        int i = _inputs.SelectedIndex;
        int j = i + delta;
        if (i < 0 || j < 0 || j >= _inputs.Items.Count) return;
        var item = _inputs.Items[i];
        bool isChecked = _inputs.GetItemChecked(i);
        _inputs.Items.RemoveAt(i);
        _inputs.Items.Insert(j, item);
        _inputs.SetItemChecked(j, isChecked);
        _inputs.SelectedIndex = j;
        UpdateSummary();
    }

    private void AddCustom()
    {
        var text = _customCode.Text.Trim();
        if (text.StartsWith("0x", StringComparison.OrdinalIgnoreCase)) text = text[2..];
        if (!int.TryParse(text, System.Globalization.NumberStyles.HexNumber, null, out int code) || code is < 1 or > 0xFF)
        {
            MessageBox.Show(this, "Enter an input code between 01 and FF in hex.", Text, MessageBoxButtons.OK, MessageBoxIcon.Warning);
            return;
        }
        for (int i = 0; i < _inputs.Items.Count; i++)
        {
            if (((InputItem)_inputs.Items[i]).Code == code) { _inputs.SetItemChecked(i, true); return; }
        }
        _inputs.Items.Add(new InputItem(code), true);
        _customCode.Clear();
        UpdateSummary();
    }

    private List<int> CheckedInputs()
    {
        var list = new List<int>();
        for (int i = 0; i < _inputs.Items.Count; i++)
            if (_inputs.GetItemChecked(i)) list.Add(((InputItem)_inputs.Items[i]).Code);
        return list;
    }

    private void UpdateSummary()
    {
        var preview = new HotkeyBinding { Inputs = CheckedInputs() };
        var monitor = SelectedMonitor;
        _summary.Text = monitor is null ? "" :
            (monitor.SupportedInputs.Count == 0 ? "Inputs not detected yet — showing common inputs.\n" : "") +
            $"Action: {preview.DescribeAction()}";
    }

    private void Accept()
    {
        var monitor = SelectedMonitor;
        var hotkey = _hotkey.Hotkey;
        var inputs = CheckedInputs();

        string? problem =
            monitor is null ? "Choose a monitor." :
            hotkey.IsEmpty ? "Press the key combination to use in the Hotkey box." :
            !hotkey.IsValid ? "Use at least Ctrl, Alt or Win with the key (or one of F13–F24)." :
            inputs.Count == 0 ? "Tick at least one input." :
            _profile.Bindings.Any(b => b.Id != _editingId && b.Hotkey == hotkey) ? $"{hotkey} is already used in this configuration." :
            null;

        if (problem is not null)
        {
            MessageBox.Show(this, problem, Text, MessageBoxButtons.OK, MessageBoxIcon.Warning);
            return;
        }

        Result = new HotkeyBinding
        {
            Id = _editingId ?? Guid.NewGuid().ToString("N"),
            MonitorId = monitor!.MonitorId,
            Hotkey = hotkey,
            Inputs = inputs,
        };
        DialogResult = DialogResult.OK;
    }

    private static Label Caption(string text) => new() { Text = text, AutoSize = true, Anchor = AnchorStyles.Left, Margin = new Padding(3, 6, 8, 3) };

    private static Button MakeButton(string text, EventHandler onClick)
    {
        var b = new Button { Text = text, AutoSize = true, MinimumSize = new Size(80, 0) };
        b.Click += onClick;
        return b;
    }
}
