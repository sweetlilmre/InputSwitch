// Smoke tests for InputSwitch's core logic: parsing what monitors report, recognising setups,
// hotkey rules and the settings file. They need no monitors and change nothing on the machine.

using System.Text;
using InputSwitch.Config;
using InputSwitch.Hotkeys;
using InputSwitch.Monitors;
using Keys = System.Windows.Forms.Keys;

var tests = new List<(string Name, Action Body)>
{
    // ---------------------------------------------------------------- monitor capabilities

    ("Capabilities: inputs parsed from a Dell P2419H", () =>
    {
        const string caps = "(prot(monitor)type(lcd)model(P2419H)cmds(01 02 03 07 0C E3 F3)vcp(02 04 05 08 10 12 14(05 08 0B 0C) 16 18 1A 52 60(01 0F 11) AA(01 02 04) AC AE B2 B6 C6 C8 C9 CC(02 03 04 06 09 0a 0d 0e) D6(01 04 05) DC(00 03 05 ) DF E0 E1 E2(00 02 04 0E 12 14 1D) F0(00 0C) F1 F2 FD)mccs_ver(2.1)mswhql(1))";
        Assert.Sequence([0x01, 0x0F, 0x11], InputSources.ParseFromCapabilities(caps));
    }),

    ("Capabilities: inputs parsed from a Dell U2723QE (space after the bracket)", () =>
    {
        const string caps = "(prot(monitor)type(lcd)model(U2723QE)cmds(01 02 03 07 0C E3 F3)vcp(02 04 05 08 10 12 14(01 04 05 06 08 09 0B 0C) 16 18 1A 52 60( 1B 0F 11) AA(01 02 04) AC AE B2 B6 C6 C8 C9 CC(02 03 04 06 09 0A 0D 0E) D6(01 04 05) DC(00 03 05) DF E0 E1 E2(00 02 04 0C 0D 0F 10 11 13 0B 1B 1A 14 23 24 27 3A) E5 E7(02 03) E8 E9(00 01 02 21 22 24) EA EF F0(00 05 06 09 0A 31 32 34 36) F1 F2 FE FD)mccs_ver(2.1)mswhql(1))";
        Assert.Sequence([0x1B, 0x0F, 0x11], InputSources.ParseFromCapabilities(caps));
    }),

    ("Capabilities: a 60 inside another code's value list is not mistaken for inputs", () =>
        Assert.Sequence([0x11], InputSources.ParseFromCapabilities("(vcp(14(05 60 0B) 60(11)))"))),

    ("Capabilities: missing, empty or malformed strings give no inputs", () =>
    {
        Assert.Equal(0, InputSources.ParseFromCapabilities(null).Count);
        Assert.Equal(0, InputSources.ParseFromCapabilities("").Count);
        Assert.Equal(0, InputSources.ParseFromCapabilities("(prot(monitor)type(lcd))").Count);
        Assert.Equal(0, InputSources.ParseFromCapabilities("(vcp(02 04 60(0F 11").Count);
    }),

    ("Input names: known codes named, unknown codes shown in hex", () =>
    {
        Assert.Equal("DisplayPort 1", InputSources.ShortName(0x0F));
        Assert.Equal("HDMI 1 (0x11)", InputSources.Name(0x11));
        Assert.Equal("USB-C", InputSources.ShortName(0x19));
        Assert.Equal("0x7E", InputSources.ShortName(0x7E));
    }),

    // ---------------------------------------------------------------- EDID

    ("EDID: manufacturer, product, serial and model name", () =>
    {
        var edid = EdidInfo.Parse(Edid.Build("DEL", 0xD0D9, 0x12345678, serialText: "BPG5ZT2", name: "DELL P2419H"));
        Assert.NotNull(edid);
        Assert.Equal("DEL", edid!.ManufacturerId);
        Assert.Equal((ushort)0xD0D9, edid.ProductCode);
        Assert.Equal("BPG5ZT2", edid.Serial);
        Assert.Equal("DELL P2419H", edid.ModelName);
    }),

    ("EDID: numeric serial used when there's no serial text; none when both are missing", () =>
    {
        Assert.Equal("12345678", EdidInfo.Parse(Edid.Build("DEL", 1, 0x12345678))!.Serial);
        Assert.Null(EdidInfo.Parse(Edid.Build("DEL", 1, 0))!.Serial);
    }),

    ("EDID: too short to be valid", () =>
    {
        Assert.Null(EdidInfo.Parse(null));
        Assert.Null(EdidInfo.Parse(new byte[64]));
    }),

    ("EDID: device path converted to its registry instance", () =>
        Assert.Equal(@"DISPLAY\DELA0B1\5&2a1b3c&0&UID4352",
            EdidInfo.DevicePathToInstanceId(@"\\?\DISPLAY#DELA0B1#5&2a1b3c&0&UID4352#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}"))),

    // ---------------------------------------------------------------- recognising setups

    ("Setups: the laptop's own screen is left out of the fingerprint", () =>
    {
        var fp = ProfileMatcher.Fingerprint([Monitor("BOE0A89@x", isInternal: true), Monitor("A#1"), Monitor("B#2")]);
        Assert.True(fp.SetEquals(["A#1", "B#2"]), "fingerprint should be the two external monitors");
    }),

    ("Setups: exact match chosen", () =>
    {
        var (config, work, home) = WorkAndHome();
        Assert.Same(home, ProfileMatcher.Match(config, Set("H#1", "H#2"), current: work));
        Assert.Same(work, ProfileMatcher.Match(config, Set("W#1", "W#2"), current: null));
    }),

    ("Setups: a missing monitor keeps the configuration (switched away or off)", () =>
    {
        var (config, work, _) = WorkAndHome();
        Assert.Same(work, ProfileMatcher.Match(config, Set("W#2"), current: null));
    }),

    ("Setups: when several contain the monitors, the current one is kept", () =>
    {
        var config = new AppConfig();
        var big = Profile("Big", "A#1", "B#2", "C#3");
        var small = Profile("Small", "A#1", "B#2");
        config.Profiles.AddRange([big, small]);
        Assert.Same(small, ProfileMatcher.Match(config, Set("A#1"), current: null)); // fewest monitors
        Assert.Same(big, ProfileMatcher.Match(config, Set("A#1"), current: big));    // stays put
    }),

    ("Setups: an unknown monitor means a new setup; no monitors means none", () =>
    {
        var (config, work, _) = WorkAndHome();
        Assert.Null(ProfileMatcher.Match(config, Set("W#1", "NEW#9"), current: work));
        Assert.Null(ProfileMatcher.Match(config, Set(), current: work));
    }),

    ("Setups: a new configuration holds the external monitors, starts off, and gets a unique name", () =>
    {
        var config = new AppConfig();
        config.Profiles.Add(new Profile { Name = "Setup 2" });
        var created = ProfileMatcher.CreateFor([Monitor("A#1"), Monitor("LAPTOP", isInternal: true)], config);
        Assert.False(created.Enabled, "new configurations start with hotkeys off");
        Assert.Sequence(["A#1"], created.Monitors.Select(m => m.MonitorId).ToList());
        Assert.Equal("Setup 2 (2)", created.Name);
    }),

    // ---------------------------------------------------------------- hotkeys

    ("Hotkeys: need Ctrl, Alt or Win, except F13–F24", () =>
    {
        Assert.True(Key(HotkeyModifiers.Control | HotkeyModifiers.Alt, Keys.M).IsValid, "Ctrl+Alt+M");
        Assert.True(Key(HotkeyModifiers.Win, Keys.F1).IsValid, "Win+F1");
        Assert.True(Key(HotkeyModifiers.None, Keys.F13).IsValid, "F13 alone");
        Assert.False(Key(HotkeyModifiers.None, Keys.M).IsValid, "M alone");
        Assert.False(Key(HotkeyModifiers.Shift, Keys.M).IsValid, "Shift+M");
        Assert.False(new Hotkey().IsValid, "empty");
    }),

    ("Hotkeys: shown the way people write them", () =>
    {
        Assert.Equal("Ctrl + Alt + M", Key(HotkeyModifiers.Alt | HotkeyModifiers.Control, Keys.M).ToString());
        Assert.Equal("Ctrl + Shift + 1", Key(HotkeyModifiers.Control | HotkeyModifiers.Shift, Keys.D1).ToString());
        Assert.Equal("(none)", new Hotkey().ToString());
    }),

    ("Hotkeys: actions described by how many inputs are ticked", () =>
    {
        Assert.Equal("Switch to HDMI 1", Binding(0x11).DescribeAction());
        Assert.Equal("Toggle DisplayPort 1 ⇄ HDMI 1", Binding(0x0F, 0x11).DescribeAction());
        Assert.Equal("Cycle USB-C → DisplayPort 1 → HDMI 1", Binding(0x1B, 0x0F, 0x11).DescribeAction());
    }),

    // ---------------------------------------------------------------- settings file

    ("Settings: survive a save/load round trip", () =>
    {
        var (config, work, _) = WorkAndHome();
        work.Enabled = true;
        work.Bindings.Add(new HotkeyBinding
        {
            MonitorId = "W#1",
            Hotkey = Key(HotkeyModifiers.Alt | HotkeyModifiers.Control, Keys.M),
            Inputs = [0x0F, 0x11],
        });

        var copy = ConfigStore.Clone(config);
        var w = copy.Profiles.Single(p => p.Name == "Work");
        Assert.True(w.Enabled, "Enabled kept");
        Assert.Equal(work.Bindings[0].Hotkey, w.Bindings[0].Hotkey);
        Assert.Sequence([0x0F, 0x11], w.Bindings[0].Inputs);
        Assert.Equal(2, copy.Profiles.Count);
    }),
};

// ---------------------------------------------------------------- runner

int failed = 0;
foreach (var (name, body) in tests)
{
    try
    {
        body();
        Console.WriteLine($"  PASS  {name}");
    }
    catch (Exception ex)
    {
        failed++;
        Console.WriteLine($"  FAIL  {name}\n        {ex.Message}");
    }
}
Console.WriteLine();
Console.WriteLine(failed == 0 ? $"All {tests.Count} smoke tests passed." : $"{failed} of {tests.Count} smoke tests FAILED.");
return failed == 0 ? 0 : 1;

// ---------------------------------------------------------------- helpers

static MonitorInfo Monitor(string id, bool isInternal = false) =>
    new(id, id, id.Split('#', '@')[0], null, isInternal, @"\\.\DISPLAY1", 0);

static Profile Profile(string name, params string[] monitorIds) => new()
{
    Name = name,
    Monitors = monitorIds.Select(id => new ProfileMonitor { MonitorId = id, Name = id }).ToList(),
};

static (AppConfig, Profile Work, Profile Home) WorkAndHome()
{
    var work = Profile("Work", "W#1", "W#2");
    var home = Profile("Home", "H#1", "H#2");
    return (new AppConfig { Profiles = [work, home] }, work, home);
}

static HashSet<string> Set(params string[] ids) => ids.ToHashSet(StringComparer.OrdinalIgnoreCase);

static Hotkey Key(HotkeyModifiers modifiers, Keys key) => new() { Modifiers = modifiers, Key = key };

static HotkeyBinding Binding(params int[] inputs) => new() { Inputs = [.. inputs] };

internal static class Edid
{
    /// <summary>Builds a minimal 128-byte EDID block with optional serial and name descriptors.</summary>
    public static byte[] Build(string manufacturer, ushort product, uint serial, string? serialText = null, string? name = null)
    {
        var e = new byte[128];
        new byte[] { 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00 }.CopyTo(e, 0);
        int m = ((manufacturer[0] - 'A' + 1) << 10) | ((manufacturer[1] - 'A' + 1) << 5) | (manufacturer[2] - 'A' + 1);
        e[8] = (byte)(m >> 8); e[9] = (byte)m;
        e[10] = (byte)product; e[11] = (byte)(product >> 8);
        BitConverter.GetBytes(serial).CopyTo(e, 12);
        if (serialText is not null) Descriptor(e, 54, 0xFF, serialText);
        if (name is not null) Descriptor(e, 72, 0xFC, name);
        return e;
    }

    private static void Descriptor(byte[] e, int offset, byte tag, string text)
    {
        e[offset + 3] = tag;
        var bytes = Encoding.ASCII.GetBytes((text + "\n").PadRight(13));
        Array.Copy(bytes, 0, e, offset + 5, 13);
    }
}

internal static class Assert
{
    public static void True(bool condition, string what) { if (!condition) throw new Exception($"Expected true: {what}"); }
    public static void False(bool condition, string what) { if (condition) throw new Exception($"Expected false: {what}"); }
    public static void Null(object? value) { if (value is not null) throw new Exception($"Expected null, got {value}"); }
    public static void NotNull(object? value) { if (value is null) throw new Exception("Expected a value, got null"); }

    public static void Equal<T>(T expected, T actual)
    {
        if (!EqualityComparer<T>.Default.Equals(expected, actual))
            throw new Exception($"Expected <{expected}>, got <{actual}>");
    }

    public static void Same(object? expected, object? actual)
    {
        if (!ReferenceEquals(expected, actual))
            throw new Exception($"Expected <{Describe(expected)}>, got <{Describe(actual)}>");
    }

    public static void Sequence<T>(IReadOnlyList<T> expected, IReadOnlyList<T> actual)
    {
        if (!expected.SequenceEqual(actual))
            throw new Exception($"Expected [{string.Join(", ", expected)}], got [{string.Join(", ", actual)}]");
    }

    private static string Describe(object? o) => o is Profile p ? $"profile '{p.Name}'" : o?.ToString() ?? "null";
}
