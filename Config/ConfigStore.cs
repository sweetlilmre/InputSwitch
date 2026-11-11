using System.Text.Json;
using System.Text.Json.Serialization;

namespace InputSwitch.Config;

/// <summary>
/// Loads and saves the configuration as JSON under %APPDATA%\InputSwitch, or under the folder in
/// the INPUTSWITCH_CONFIG_DIR environment variable if set (used by the smoke tests).
/// </summary>
internal static class ConfigStore
{
    public const string ConfigDirVariable = "INPUTSWITCH_CONFIG_DIR";

    private static readonly string? OverrideDirectory =
        Environment.GetEnvironmentVariable(ConfigDirVariable) is { Length: > 0 } d ? Path.GetFullPath(d) : null;

    public static string Directory { get; } = OverrideDirectory ??
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "InputSwitch");

    /// <summary>
    /// Distinguishes instances using different config folders, so a test copy can run alongside
    /// the user's own copy. Empty for the normal location.
    /// </summary>
    public static string InstanceKey { get; } = OverrideDirectory is null ? "" :
        "-" + Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(
            System.Text.Encoding.UTF8.GetBytes(OverrideDirectory.ToLowerInvariant())))[..12];

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
            UI.AppDialogs.Alert(UI.DialogIcon.Warning, "Your settings couldn't be read",
                $"{ex.Message}\n\nInputSwitch will start with fresh settings. The unreadable file was kept as config.json.bad in {Directory}.");
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
