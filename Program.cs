using System.Drawing;
using System.Windows.Forms;
using System.Text;
using InputSwitch.Config;
using InputSwitch.Monitors;

namespace InputSwitch;

internal static class Program
{
    [STAThread]
    private static void Main(string[] args)
    {
        if (args.Length > 0 && args[0].Equals("--diagnose", StringComparison.OrdinalIgnoreCase))
        {
            Diagnose(args.Length > 1 ? args[1] : null);
            return;
        }

        using var mutex = new Mutex(initiallyOwned: true, @"Local\InputSwitch-6d3f0b8e", out bool isFirst);
        if (!isFirst)
        {
            UI.AppDialogs.Alert(UI.DialogIcon.Info, "InputSwitch is already running",
                "Look for its icon in the notification area. Click it to open the settings.");
            return;
        }

        ApplicationConfiguration.Initialize();

        bool openSettings = args.Any(a => a.Equals("--settings", StringComparison.OrdinalIgnoreCase));
        Application.Run(new TrayApp(openSettings));
        UI.WpfHost.Shutdown();
    }

    /// <summary>
    /// Read-only report of what the app sees: monitors, their ids, DDC/CI replies and the
    /// profile that would be chosen. Never changes an input.
    /// </summary>
    private static void Diagnose(string? outputPath)
    {
        var sb = new StringBuilder();
        sb.AppendLine($"InputSwitch diagnostics — {DateTime.Now:yyyy-MM-dd HH:mm:ss}");
        sb.AppendLine();

        var monitors = MonitorService.Enumerate();
        foreach (var m in monitors)
        {
            sb.AppendLine($"{m.DisplayName}{(m.IsInternal ? "  [built-in, ignored for setup matching]" : "")}");
            sb.AppendLine($"  Id:            {m.Id}");
            sb.AppendLine($"  Model/serial:  {m.Model} / {m.Serial ?? "(none)"}");
            sb.AppendLine($"  GDI device:    {m.GdiDeviceName} #{m.IndexOnDevice}");
            var current = MonitorService.GetCurrentInput(m);
            sb.AppendLine($"  Current input: {(current is null ? "no DDC/CI reply" : InputSources.Name(current.Value))}");
            var caps = MonitorService.GetCapabilities(m);
            var inputs = InputSources.ParseFromCapabilities(caps);
            sb.AppendLine($"  Inputs:        {(inputs.Count == 0 ? "(none reported)" : string.Join(", ", inputs.Select(InputSources.Name)))}");
            sb.AppendLine($"  Capabilities:  {caps ?? "(no reply)"}");
            sb.AppendLine();
        }

        var config = File.Exists(ConfigStore.FilePath) ? ConfigStore.Load() : new AppConfig();
        var match = ProfileMatcher.Match(config, ProfileMatcher.Fingerprint(monitors), null);
        sb.AppendLine($"Config file:     {ConfigStore.FilePath}");
        sb.AppendLine($"Matching config: {match?.Name ?? "(none — a new one would be created)"}");

        var path = outputPath ?? Path.Combine(Path.GetTempPath(), "InputSwitch-diagnostics.txt");
        File.WriteAllText(path, sb.ToString());
        if (outputPath is null)
            System.Diagnostics.Process.Start("notepad.exe", path);
    }
}
