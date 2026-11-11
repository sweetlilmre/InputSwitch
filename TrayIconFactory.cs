using System.Drawing;
using System.Windows.Forms;
using System.Drawing.Drawing2D;
using InputSwitch.Native;

namespace InputSwitch;

/// <summary>Draws the tray icon (a monitor with a swap arrow) so no .ico file is needed.</summary>
internal static class TrayIconFactory
{
    public static Icon Create()
    {
        int size = SystemInformation.SmallIconSize.Width;
        using var bmp = new Bitmap(size, size);
        using (var g = Graphics.FromImage(bmp))
        {
            g.SmoothingMode = SmoothingMode.AntiAlias;
            g.Clear(Color.Transparent);
            float s = size / 16f;

            using var frame = new Pen(Color.White, 1.5f * s);
            using var screen = new SolidBrush(Color.FromArgb(0, 120, 215));
            var body = new RectangleF(1 * s, 2 * s, 14 * s, 9.5f * s);
            g.FillRectangle(screen, body);
            g.DrawRectangle(frame, body.X, body.Y, body.Width, body.Height);
            g.DrawLine(frame, 8 * s, 11.5f * s, 8 * s, 14 * s);
            g.DrawLine(frame, 4.5f * s, 14.5f * s, 11.5f * s, 14.5f * s);

            using var arrow = new Pen(Color.White, 1.2f * s) { EndCap = LineCap.ArrowAnchor };
            g.DrawLine(arrow, 4 * s, 5 * s, 12 * s, 5 * s);
            g.DrawLine(arrow, 12 * s, 8.5f * s, 4 * s, 8.5f * s);
        }

        IntPtr hIcon = bmp.GetHicon();
        try
        {
            // Clone so the icon owns its own handle and the GDI one can be freed now.
            using var temp = Icon.FromHandle(hIcon);
            return (Icon)temp.Clone();
        }
        finally
        {
            NativeMethods.DestroyIcon(hIcon);
        }
    }
}
