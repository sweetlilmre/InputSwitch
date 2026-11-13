using System.Collections.ObjectModel;
using System.Globalization;
using System.Windows;
using System.Windows.Controls;
using InputSwitch.Config;
using InputSwitch.Monitors;

namespace InputSwitch.UI;

/// <summary>Edits one hotkey: which monitor, which key combination, and which inputs to switch between.</summary>
internal sealed partial class BindingEditorWindow : Window
{
    private readonly Profile _profile;
    private readonly HotkeyBinding? _existing;
    private readonly HotkeyBox _hotkey = new() { HorizontalAlignment = HorizontalAlignment.Stretch };
    private readonly ObservableCollection<InputRow> _inputs = [];

    public HotkeyBinding Result { get; private set; } = new();

    private sealed record MonitorItem(ProfileMonitor Monitor)
    {
        public override string ToString() => $"{Monitor.Name}  ·  {Monitor.Model}";
    }

    public BindingEditorWindow(Profile profile, HotkeyBinding? existing)
    {
        _profile = profile;
        _existing = existing;

        InitializeComponent();
        Icon = ConfigWindow.CreateWindowIcon();
        Title = Heading.Text = existing is null ? "Add hotkey" : "Edit hotkey";

        HotkeyHost.Content = _hotkey;
        InputsList.ItemsSource = _inputs;

        MonitorBox.ItemsSource = profile.Monitors.Select(m => new MonitorItem(m)).ToList();
        if (existing is not null)
        {
            _hotkey.Hotkey = existing.Hotkey;
            MonitorBox.SelectedIndex = Math.Max(0, profile.Monitors.FindIndex(m => m.MonitorId == existing.MonitorId));
        }
        else if (profile.Monitors.Count > 0)
        {
            MonitorBox.SelectedIndex = 0;
        }

        // Start on the Monitor list; the key box only records once the user clicks (or tabs) into it.
        Loaded += (_, _) => MonitorBox.Focus();
    }

    private ProfileMonitor? SelectedMonitor => (MonitorBox.SelectedItem as MonitorItem)?.Monitor;

    private void OnMonitorChanged(object sender, SelectionChangedEventArgs e)
    {
        var monitor = SelectedMonitor;
        var selected = _existing is not null && _existing.MonitorId == monitor?.MonitorId ? _existing.Inputs : [];

        _inputs.Clear();
        if (monitor is null) return;

        // The binding's inputs first, in its order, then the rest of what the monitor offers.
        var available = monitor.SupportedInputs.Count > 0 ? monitor.SupportedInputs : InputSources.Common.ToList();
        foreach (var code in selected.Concat(available.Where(c => !selected.Contains(c))))
            _inputs.Add(new InputRow(code, selected.Contains(code), UpdateSummary));

        UpdateSummary();
    }

    private void OnMoveUp(object sender, RoutedEventArgs e) => MoveSelected(-1);
    private void OnMoveDown(object sender, RoutedEventArgs e) => MoveSelected(+1);

    private void MoveSelected(int delta)
    {
        int i = InputsList.SelectedIndex;
        int j = i + delta;
        if (i < 0 || j < 0 || j >= _inputs.Count) return;
        _inputs.Move(i, j);
        InputsList.SelectedIndex = j;
        UpdateSummary();
    }

    private void OnAddCustom(object sender, RoutedEventArgs e)
    {
        var text = CustomCode.Text.Trim();
        if (text.StartsWith("0x", StringComparison.OrdinalIgnoreCase)) text = text[2..];
        if (!int.TryParse(text, NumberStyles.HexNumber, null, out int code) || code is < 1 or > 0xFF)
        {
            FluentDialog.Show(this, DialogIcon.Warning, "Not a valid input code",
                "Enter a hexadecimal code between 01 and FF, for example 1B.");
            return;
        }

        var row = _inputs.FirstOrDefault(r => r.Code == code);
        if (row is null)
        {
            row = new InputRow(code, true, UpdateSummary);
            _inputs.Add(row);
        }
        row.IsChecked = true;
        InputsList.SelectedItem = row;
        CustomCode.Clear();
        UpdateSummary();
    }

    private List<int> CheckedInputs() => _inputs.Where(r => r.IsChecked).Select(r => r.Code).ToList();

    private void UpdateSummary()
    {
        var monitor = SelectedMonitor;
        if (monitor is null) { Summary.Text = ""; return; }
        var action = new HotkeyBinding { Inputs = CheckedInputs() }.DescribeAction();
        Summary.Text = monitor.SupportedInputs.Count == 0
            ? $"{action}\nThis monitor's inputs haven't been detected, so common inputs are listed."
            : action;
    }

    private void OnAccept(object sender, RoutedEventArgs e)
    {
        var monitor = SelectedMonitor;
        var hotkey = _hotkey.Hotkey;
        var inputs = CheckedInputs();

        // Each problem gets a heading that describes it, rather than one heading for all of them.
        (string Heading, string Message)? problem =
            monitor is null
                ? ("Choose a monitor", "Pick the monitor this hotkey should switch.") :
            hotkey.IsEmpty
                ? ("No key combination yet", "Click the key combination box and press the keys to use.") :
            !hotkey.IsValid
                ? ("That combination can't be used", "Use Ctrl, Alt or Win with the key (or one of F13–F24).") :
            inputs.Count == 0
                ? ("This hotkey isn't finished", "Tick at least one input.") :
            _profile.Bindings.Any(b => b.Id != _existing?.Id && b.Hotkey == hotkey)
                ? ($"{hotkey} is already in use", "Another hotkey in this configuration uses it. Choose a different combination, or change the other hotkey.") :
            null;

        if (problem is { } p)
        {
            FluentDialog.Show(this, DialogIcon.Warning, p.Heading, p.Message);
            return;
        }

        Result = new HotkeyBinding
        {
            Id = _existing?.Id ?? Guid.NewGuid().ToString("N"),
            MonitorId = monitor!.MonitorId,
            Hotkey = hotkey,
            Inputs = inputs,
        };
        DialogResult = true;
    }
}
