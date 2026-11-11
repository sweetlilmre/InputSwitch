namespace InputSwitch.Monitors;

/// <summary>MCCS VCP code 0x60 (Input Source) values.</summary>
internal static class InputSources
{
    public const byte VcpInputSource = 0x60;

    private static readonly Dictionary<int, string> Names = new()
    {
        [0x01] = "VGA 1",
        [0x02] = "VGA 2",
        [0x03] = "DVI 1",
        [0x04] = "DVI 2",
        [0x05] = "Composite 1",
        [0x06] = "Composite 2",
        [0x07] = "S-Video 1",
        [0x08] = "S-Video 2",
        [0x09] = "Tuner 1",
        [0x0A] = "Tuner 2",
        [0x0B] = "Tuner 3",
        [0x0C] = "Component 1",
        [0x0D] = "Component 2",
        [0x0E] = "Component 3",
        [0x0F] = "DisplayPort 1",
        [0x10] = "DisplayPort 2",
        [0x11] = "HDMI 1",
        [0x12] = "HDMI 2",
        [0x19] = "USB-C",   // Dell U2725QE (Thunderbolt / USB-C)
        [0x1B] = "USB-C",   // Dell U2723QE and others
    };

    /// <summary>Inputs offered when a monitor's capabilities could not be read.</summary>
    public static readonly int[] Common = [0x01, 0x03, 0x04, 0x0F, 0x10, 0x11, 0x12, 0x1B];

    public static string Name(int code) =>
        Names.TryGetValue(code, out var n) ? $"{n} (0x{code:X2})" : $"Input 0x{code:X2}";

    public static string ShortName(int code) =>
        Names.TryGetValue(code, out var n) ? n : $"0x{code:X2}";

    /// <summary>
    /// Extracts the allowed values of VCP 0x60 from an MCCS capabilities string, e.g.
    /// <c>(prot(monitor)type(LCD)model(U2412M)cmds(01 02 03 07 0C E3 F3)vcp(02 04 ... 60(01 03 0F) ...)mccs_ver(2.1))</c>.
    /// </summary>
    public static List<int> ParseFromCapabilities(string? caps)
    {
        var result = new List<int>();
        if (string.IsNullOrEmpty(caps)) return result;

        int vcp = caps.IndexOf("vcp(", StringComparison.OrdinalIgnoreCase);
        if (vcp < 0) return result;

        int i = vcp + 4;
        int depth = 1;
        while (i < caps.Length && depth > 0)
        {
            char c = caps[i];
            if (c == ')') { depth--; i++; continue; }
            if (c == '(') { depth++; i++; continue; }
            if (!Uri.IsHexDigit(c)) { i++; continue; }

            // Read a hex token at the top level of vcp(...).
            int start = i;
            while (i < caps.Length && Uri.IsHexDigit(caps[i])) i++;
            var token = caps[start..i];

            int j = i;
            while (j < caps.Length && caps[j] == ' ') j++;
            bool hasValues = j < caps.Length && caps[j] == '(';

            if (depth == 1 && hasValues && token.Equals("60", StringComparison.OrdinalIgnoreCase))
            {
                int close = caps.IndexOf(')', j);
                if (close < 0) break;
                foreach (var v in caps[(j + 1)..close].Split(' ', StringSplitOptions.RemoveEmptyEntries))
                    if (int.TryParse(v, System.Globalization.NumberStyles.HexNumber, null, out int code) && !result.Contains(code))
                        result.Add(code);
                break;
            }
        }

        return result;
    }
}
