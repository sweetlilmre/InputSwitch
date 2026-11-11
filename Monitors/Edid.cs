using System.Text;
using Microsoft.Win32;

namespace InputSwitch.Monitors;

/// <summary>The parts of a monitor's EDID block that identify it.</summary>
internal sealed record EdidInfo(string ManufacturerId, ushort ProductCode, uint SerialNumber, string? SerialString, string? ModelName)
{
    /// <summary>Best available serial: the text descriptor if present, else the numeric field.</summary>
    public string? Serial =>
        !string.IsNullOrWhiteSpace(SerialString) ? SerialString :
        SerialNumber != 0 ? SerialNumber.ToString("X8") : null;

    public static EdidInfo? Parse(byte[]? edid)
    {
        if (edid is null || edid.Length < 128) return null;

        // Bytes 8-9: three 5-bit letters, big-endian.
        int m = (edid[8] << 8) | edid[9];
        string mfg = new([
            (char)('A' - 1 + ((m >> 10) & 0x1F)),
            (char)('A' - 1 + ((m >> 5) & 0x1F)),
            (char)('A' - 1 + (m & 0x1F))]);

        ushort product = (ushort)(edid[10] | (edid[11] << 8));
        uint serial = (uint)(edid[12] | (edid[13] << 8) | (edid[14] << 16) | (edid[15] << 24));

        string? serialText = null, name = null;
        for (int offset = 54; offset <= 108; offset += 18)
        {
            // Display descriptors start with 00 00 00 <tag>.
            if (edid[offset] != 0 || edid[offset + 1] != 0 || edid[offset + 2] != 0) continue;
            byte tag = edid[offset + 3];
            if (tag != 0xFF && tag != 0xFC) continue;

            var text = Encoding.ASCII.GetString(edid, offset + 5, 13);
            int nl = text.IndexOf('\n');
            if (nl >= 0) text = text[..nl];
            text = text.Trim();

            if (tag == 0xFF) serialText = text;
            else name = text;
        }

        return new EdidInfo(mfg, product, serial, serialText, name);
    }

    /// <summary>
    /// Reads the EDID Windows cached for a monitor, given the device path from the CCD API, e.g.
    /// <c>\\?\DISPLAY#DELA0B1#5&amp;2a1b3c&amp;0&amp;UID4352#{e6f07b5f-...}</c>
    /// which maps to <c>HKLM\SYSTEM\CurrentControlSet\Enum\DISPLAY\DELA0B1\5&amp;2a1b3c&amp;0&amp;UID4352</c>.
    /// </summary>
    public static byte[]? ReadFromRegistry(string devicePath)
    {
        var instance = DevicePathToInstanceId(devicePath);
        if (instance is null) return null;
        try
        {
            using var key = Registry.LocalMachine.OpenSubKey($@"SYSTEM\CurrentControlSet\Enum\{instance}\Device Parameters");
            return key?.GetValue("EDID") as byte[];
        }
        catch
        {
            return null;
        }
    }

    public static string? DevicePathToInstanceId(string devicePath)
    {
        if (string.IsNullOrEmpty(devicePath)) return null;
        var p = devicePath;
        if (p.StartsWith(@"\\?\")) p = p[4..];
        int guid = p.LastIndexOf("#{", StringComparison.Ordinal);
        if (guid > 0) p = p[..guid];
        return p.Replace('#', '\\');
    }
}
