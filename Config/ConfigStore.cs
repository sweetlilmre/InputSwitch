using System.Text.Json;
using System.Text.Json.Serialization;

namespace InputSwitch.Config;

/// <summary>Loads and saves the configuration as JSON under %APPDATA%\InputSwitch.</summary>
internal static class ConfigStore
{
    public static string Directory { get; } =
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "InputSwitch");

    public static string FilePath { get; } = Path.Combine(Directory, "config.json");

    private static readonly JsonSerializerOptions Options = new()
    {
        WriteIndented = true,
        Converters = { new JsonStringEnumConverter() },
    };

    public static AppConfig Load()
    {
        try
        {
            if (File.Exists(FilePath))
                return JsonSerializer.Deserialize<AppConfig>(File.ReadAllText(FilePath), Options) ?? new AppConfig();
        }
        catch (Exception ex)
        {
            // Keep the unreadable file for inspection rather than silently overwriting it.
            try { File.Copy(FilePath, FilePath + ".bad", overwrite: true); } catch { }
            MessageBox.Show($"Could not read {FilePath}:\n{ex.Message}\n\nA copy was saved as config.json.bad and a fresh configuration will be used.",
                "InputSwitch", MessageBoxButtons.OK, MessageBoxIcon.Warning);
        }
        return new AppConfig();
    }

    public static void Save(AppConfig config)
    {
        System.IO.Directory.CreateDirectory(Directory);
        var temp = FilePath + ".tmp";
        File.WriteAllText(temp, JsonSerializer.Serialize(config, Options));
        File.Move(temp, FilePath, overwrite: true);
    }

    public static AppConfig Clone(AppConfig config) =>
        JsonSerializer.Deserialize<AppConfig>(JsonSerializer.Serialize(config, Options), Options)!;
}
