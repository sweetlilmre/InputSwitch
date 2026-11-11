using System.ComponentModel;
using InputSwitch.Config;
using InputSwitch.Monitors;

namespace InputSwitch.UI;

internal abstract class Observable : INotifyPropertyChanged
{
    public event PropertyChangedEventHandler? PropertyChanged;

    /// <summary>Raise with no name to refresh every binding on the object.</summary>
    protected void Raise(string? name = null) => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name ?? string.Empty));
}

/// <summary>One configuration in the navigation list.</summary>
internal sealed class ProfileRow(Profile profile, Func<Profile, bool> isConnected) : Observable
{
    public Profile Profile => profile;
    public string Title => string.IsNullOrWhiteSpace(profile.Name) ? "Unnamed setup" : profile.Name;
    public bool IsConnected => isConnected(profile);

    public string Subtitle
    {
        get
        {
            var parts = new List<string>
            {
                IsConnected ? "Connected" : "Not connected",
                profile.Monitors.Count == 1 ? "1 monitor" : $"{profile.Monitors.Count} monitors",
            };
            if (!profile.Enabled) parts.Add("hotkeys off");
            return string.Join(" · ", parts);
        }
    }

    public void Refresh() => Raise();
}

internal sealed record InputChip(string Text, bool IsCurrent);

/// <summary>A monitor card. The name is edited in place.</summary>
internal sealed class MonitorRow(ProfileMonitor monitor, bool connected, int? current, Action onRenamed) : Observable
{
    public ProfileMonitor Monitor => monitor;

    public string Name
    {
        get => monitor.Name;
        set
        {
            if (value == monitor.Name) return;
            monitor.Name = value;
            onRenamed();
        }
    }

    public string Details => monitor.Serial is null ? monitor.Model : $"{monitor.Model} · S/N {monitor.Serial}";
    public bool IsConnected => connected;
    public string StatusText => connected ? "Connected" : "Not connected";

    public List<InputChip> Inputs =>
        monitor.SupportedInputs.Select(c => new InputChip(InputSources.ShortName(c), c == current)).ToList();

    public string InputsHint =>
        monitor.SupportedInputs.Count > 0 ? "" :
        connected ? "Inputs not detected yet" : "Connect this monitor to detect its inputs";
}

/// <summary>A hotkey row: key caps, target monitor and action.</summary>
internal sealed record HotkeyRow(HotkeyBinding Binding, List<string> Keys, string MonitorName, string Action);

/// <summary>An input in the hotkey editor's list.</summary>
internal sealed class InputRow(int code, bool isChecked, Action onChanged) : Observable
{
    private bool _isChecked = isChecked;
    public int Code => code;
    public string Name => InputSources.Name(code);

    public bool IsChecked
    {
        get => _isChecked;
        set
        {
            if (_isChecked == value) return;
            _isChecked = value;
            Raise(nameof(IsChecked));
            onChanged();
        }
    }
}
