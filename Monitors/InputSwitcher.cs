using System.Collections.Concurrent;
using InputSwitch.Config;

namespace InputSwitch.Monitors;

internal sealed record SwitchResult(bool Success, int? Target, string Message);

/// <summary>Carries out a binding's action: switch to one input, or step through several.</summary>
internal sealed class InputSwitcher
{
    // What we last told each monitor, for when it can't report its current input
    // (common right after switching away, while the monitor resyncs).
    private readonly ConcurrentDictionary<string, int> _lastSet = new(StringComparer.OrdinalIgnoreCase);

    public Task<SwitchResult> RunAsync(HotkeyBinding binding, MonitorInfo monitor) => Task.Run(() =>
    {
        var inputs = binding.Inputs;
        if (inputs.Count == 0)
            return new SwitchResult(false, null, "No inputs are selected for this hotkey.");

        int target;
        if (inputs.Count == 1)
        {
            target = inputs[0];
        }
        else
        {
            int? current = MonitorService.GetCurrentInput(monitor);
            if ((current is null || !inputs.Contains(current.Value)) && _lastSet.TryGetValue(monitor.Id, out int last))
                current = last;

            int index = current is null ? -1 : inputs.IndexOf(current.Value);
            target = inputs[(index + 1) % inputs.Count];
        }

        if (!MonitorService.SetInput(monitor, target))
            return new SwitchResult(false, target,
                $"{monitor.DisplayName} did not accept the switch to {InputSources.ShortName(target)}. " +
                "Check that DDC/CI is enabled in the monitor's on-screen menu.");

        _lastSet[monitor.Id] = target;
        return new SwitchResult(true, target, $"{monitor.DisplayName}: {InputSources.ShortName(target)}");
    });

    public Task<SwitchResult> SetAsync(MonitorInfo monitor, int input) => Task.Run(() =>
    {
        if (!MonitorService.SetInput(monitor, input))
            return new SwitchResult(false, input, $"{monitor.DisplayName} did not accept the switch to {InputSources.ShortName(input)}.");
        _lastSet[monitor.Id] = input;
        return new SwitchResult(true, input, $"{monitor.DisplayName}: {InputSources.ShortName(input)}");
    });
}
