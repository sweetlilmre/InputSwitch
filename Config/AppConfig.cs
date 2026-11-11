using System.Text.Json.Serialization;
using InputSwitch.Hotkeys;

namespace InputSwitch.Config;

internal sealed class AppConfig
{
    public List<Profile> Profiles { get; set; } = [];
    public bool ShowNotifications { get; set; } = true;
}

/// <summary>
/// A configuration for one physical display setup (e.g. "Work", "Home"), identified by the
/// set of external monitors that make it up.
/// </summary>
internal sealed class Profile
{
    public string Id { get; set; } = Guid.NewGuid().ToString("N");
    public string Name { get; set; } = "New setup";
    public bool Enabled { get; set; }
    public DateTime CreatedUtc { get; set; } = DateTime.UtcNow;
    public List<ProfileMonitor> Monitors { get; set; } = [];
    public List<HotkeyBinding> Bindings { get; set; } = [];

    [JsonIgnore]
    public HashSet<string> MonitorIds => Monitors.Select(m => m.MonitorId).ToHashSet(StringComparer.OrdinalIgnoreCase);

    public ProfileMonitor? FindMonitor(string monitorId) =>
        Monitors.FirstOrDefault(m => string.Equals(m.MonitorId, monitorId, StringComparison.OrdinalIgnoreCase));
}

internal sealed class ProfileMonitor
{
    public string MonitorId { get; set; } = "";
    /// <summary>User-chosen name, e.g. "Left Dell".</summary>
    public string Name { get; set; } = "";
    public string Model { get; set; } = "";
    public string? Serial { get; set; }
    /// <summary>Input codes reported by the monitor's capabilities string (empty until detected).</summary>
    public List<int> SupportedInputs { get; set; } = [];
}

/// <summary>
/// A hotkey bound to a monitor. With one input it switches to that input; with several it
/// cycles through them in order (two inputs = toggle).
/// </summary>
internal sealed class HotkeyBinding
{
    public string Id { get; set; } = Guid.NewGuid().ToString("N");
    public string MonitorId { get; set; } = "";
    public Hotkey Hotkey { get; set; } = new();
    public List<int> Inputs { get; set; } = [];

    public string DescribeAction()
    {
        var names = Inputs.Select(Monitors.InputSources.ShortName).ToList();
        return names.Count switch
        {
            0 => "(no inputs)",
            1 => $"Switch to {names[0]}",
            2 => $"Toggle {names[0]} ⇄ {names[1]}",
            _ => $"Cycle {string.Join(" → ", names)}",
        };
    }
}
