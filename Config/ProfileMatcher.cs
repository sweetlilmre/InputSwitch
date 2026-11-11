using InputSwitch.Monitors;

namespace InputSwitch.Config;

/// <summary>Decides which profile belongs to the set of monitors currently connected.</summary>
internal static class ProfileMatcher
{
    /// <summary>
    /// The external monitors define a setup. The laptop's own panel is ignored, so opening or
    /// closing the lid doesn't look like a different place.
    /// </summary>
    public static HashSet<string> Fingerprint(IEnumerable<MonitorInfo> monitors) =>
        monitors.Where(m => !m.IsInternal).Select(m => m.Id).ToHashSet(StringComparer.OrdinalIgnoreCase);

    /// <summary>
    /// Returns the matching profile, or null when the setup is unknown.
    /// <list type="number">
    /// <item>An exact match on the set of external monitors wins.</item>
    /// <item>Otherwise a profile that contains every connected monitor also matches. This covers
    /// a monitor that drops off the bus after being switched to another input, or one that is
    /// switched off. The currently active profile is preferred, then the largest overlap.</item>
    /// </list>
    /// </summary>
    public static Profile? Match(AppConfig config, HashSet<string> fingerprint, Profile? current)
    {
        if (fingerprint.Count == 0) return null;

        var exact = config.Profiles.FirstOrDefault(p => p.MonitorIds.SetEquals(fingerprint));
        if (exact is not null) return exact;

        var supersets = config.Profiles.Where(p => p.MonitorIds.IsSupersetOf(fingerprint)).ToList();
        if (supersets.Count == 0) return null;

        var stillCurrent = supersets.FirstOrDefault(p => p.Id == current?.Id);
        if (stillCurrent is not null) return stillCurrent;

        // Fewest extra monitors = closest match.
        return supersets.OrderBy(p => p.Monitors.Count).ThenBy(p => p.CreatedUtc).First();
    }

    public static Profile CreateFor(IEnumerable<MonitorInfo> monitors, AppConfig config)
    {
        var externals = monitors.Where(m => !m.IsInternal).ToList();
        var profile = new Profile
        {
            Name = UniqueName(config, $"Setup {config.Profiles.Count + 1}"),
            Enabled = false,
            Monitors = externals.Select(m => new ProfileMonitor
            {
                MonitorId = m.Id,
                Name = m.DisplayName,
                Model = m.Model,
                Serial = m.Serial,
            }).ToList(),
        };
        return profile;
    }

    private static string UniqueName(AppConfig config, string baseName)
    {
        var name = baseName;
        for (int n = 2; config.Profiles.Any(p => p.Name.Equals(name, StringComparison.OrdinalIgnoreCase)); n++)
            name = $"{baseName} ({n})";
        return name;
    }
}
