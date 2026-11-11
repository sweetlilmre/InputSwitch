using System.Reflection;

namespace InputSwitch;

/// <summary>Version information, from the assembly attributes the build stamps from VersionPrefix.</summary>
internal static class AppInfo
{
    /// <summary>Full informational version, including the git commit, e.g. 1.0.0+7c86e8a1….</summary>
    public static string FullVersion { get; } =
        typeof(AppInfo).Assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion ?? "0.0.0";

    /// <summary>Version for display, e.g. 1.0.0 or 1.1.0-beta.1.</summary>
    public static string Version { get; } = FullVersion.Split('+')[0];

    /// <summary>Short commit hash the build came from, if known.</summary>
    public static string? Commit { get; } =
        FullVersion.Split('+') is [_, var sha, ..] ? sha[..Math.Min(7, sha.Length)] : null;

    public static string DisplayVersion => Commit is null ? Version : $"{Version} ({Commit})";
}
