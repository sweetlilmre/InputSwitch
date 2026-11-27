//! Switching inputs: one input switches to it; several are stepped through in order (two is
//! a toggle). Blocking (DDC/CI is slow): call these off the UI thread.

use super::ddc::{self, Physical};
use super::{inputs, MonitorInfo};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Many monitors go on reporting the old input for a second or two after switching; a switch
/// made more recently than this is trusted over what the monitor says.
const SETTLE: Duration = Duration::from_secs(3);

/// What each monitor was last switched to, and when.
static LAST_SWITCH: Mutex<Option<HashMap<String, (u8, Instant)>>> = Mutex::new(None);

fn last_switch(m: &MonitorInfo) -> Option<(u8, Instant)> {
    LAST_SWITCH.lock().unwrap_or_else(|e| e.into_inner()).as_ref()?.get(&m.id).copied()
}

fn remember(m: &MonitorInfo, input: u8) {
    LAST_SWITCH.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(m.id.clone(), (input, Instant::now()));
}

/// Runs a hotkey's action. Returns the input switched to.
pub fn run_binding(inputs: &[u8], m: &MonitorInfo) -> Result<u8, String> {
    match inputs {
        [] => Err("No inputs are selected for this hotkey.".into()),
        [only] => switch_to(m, *only),
        // Reading the current input and switching happen in one conversation with the monitor,
        // so two quick presses can't both read the same input and switch to the same next one.
        _ => {
            let switched = ddc::with_monitor(m, |p| {
                let next = next_input(inputs, current(m, p));
                p.set_input(next).then_some(next)
            });
            finish(m, switched, inputs[0])
        }
    }
}

/// Switches monitor `m` to `input`.
pub fn switch_to(m: &MonitorInfo, input: u8) -> Result<u8, String> {
    let switched = ddc::with_monitor(m, |p| p.set_input(input).then_some(input));
    finish(m, switched, input)
}

/// Remembers a switch that worked, or explains one that didn't.
fn finish(m: &MonitorInfo, switched: Option<u8>, attempted: u8) -> Result<u8, String> {
    match switched {
        Some(input) => {
            remember(m, input);
            Ok(input)
        }
        None => Err(format!(
            "{} did not accept the switch to {}. Check that DDC/CI is enabled in the monitor's on-screen menu.",
            m.display_name(),
            inputs::short_name(attempted)
        )),
    }
}

/// The input to step from: a recent switch, else what the monitor reports, else the last switch.
fn current(m: &MonitorInfo, p: &Physical) -> Option<u8> {
    let last = last_switch(m);
    if let Some((input, at)) = last {
        if at.elapsed() < SETTLE {
            return Some(input);
        }
    }
    p.current_input().or(last.map(|(input, _)| input))
}

/// The input after `current` in the list (the first if `current` isn't in it).
fn next_input(inputs: &[u8], current: Option<u8>) -> u8 {
    let next = current.and_then(|c| inputs.iter().position(|&x| x == c)).map_or(0, |i| i + 1);
    inputs[next % inputs.len()]
}

#[cfg(test)]
mod tests {
    use super::next_input;

    #[test]
    fn steps_through_the_inputs() {
        assert_eq!(next_input(&[0x0F, 0x11], Some(0x0F)), 0x11);
        assert_eq!(next_input(&[0x0F, 0x11], Some(0x11)), 0x0F);
        assert_eq!(next_input(&[0x1B, 0x0F, 0x11], Some(0x0F)), 0x11);
        assert_eq!(next_input(&[0x0F, 0x11], Some(0x01)), 0x0F); // not in the list: the first
        assert_eq!(next_input(&[0x0F, 0x11], None), 0x0F);
    }
}
