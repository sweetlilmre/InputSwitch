//! Monitor inputs: their MCCS codes (VCP 0x60 values) and names, and the list of inputs a
//! monitor reports in its capabilities string.

const INPUT_NAMES: &[(u8, &str)] = &[
    (0x01, "VGA 1"), (0x02, "VGA 2"), (0x03, "DVI 1"), (0x04, "DVI 2"), (0x05, "Composite 1"), (0x06, "Composite 2"),
    (0x07, "S-Video 1"), (0x08, "S-Video 2"), (0x09, "Tuner 1"), (0x0A, "Tuner 2"), (0x0B, "Tuner 3"),
    (0x0C, "Component 1"), (0x0D, "Component 2"), (0x0E, "Component 3"), (0x0F, "DisplayPort 1"),
    (0x10, "DisplayPort 2"), (0x11, "HDMI 1"), (0x12, "HDMI 2"),
    (0x19, "USB-C"), // Dell U2725QE (Thunderbolt / USB-C)
    (0x1B, "USB-C"), // Dell U2723QE and others
];

/// Inputs offered when a monitor's capabilities couldn't be read.
pub const COMMON_INPUTS: &[u8] = &[0x01, 0x03, 0x04, 0x0F, 0x10, 0x11, 0x12, 0x1B];

fn name(code: u8) -> Option<&'static str> {
    INPUT_NAMES.iter().find(|(c, _)| *c == code).map(|(_, n)| *n)
}

/// "HDMI 1", or "0x7E" for a code without a name.
pub fn short_name(code: u8) -> String {
    name(code).map_or_else(|| format!("0x{code:02X}"), str::to_string)
}

/// "HDMI 1 (0x11)", or "Input 0x7E".
pub fn long_name(code: u8) -> String {
    name(code).map_or_else(|| format!("Input 0x{code:02X}"), |n| format!("{n} (0x{code:02X})"))
}

/// "Switch to HDMI 1" / "Toggle DisplayPort 1 ⇄ HDMI 1" / "Cycle A → B → C".
pub fn describe_action(inputs: &[u8]) -> String {
    let names: Vec<String> = inputs.iter().map(|&c| short_name(c)).collect();
    match names.as_slice() {
        [] => "(no inputs)".into(),
        [one] => format!("Switch to {one}"),
        [a, b] => format!("Toggle {a} \u{21c4} {b}"),
        _ => format!("Cycle {}", names.join(" \u{2192} ")),
    }
}

/// The allowed values of VCP 0x60 in an MCCS capabilities string, e.g. `... vcp(... 60(01 0F 11) ...)`.
pub fn parse_capabilities(caps: &str) -> Vec<u8> {
    let Some(start) = caps.to_ascii_lowercase().find("vcp(") else { return Vec::new() };
    let b = caps.as_bytes();
    let (mut i, mut depth) = (start + 4, 1);
    while i < b.len() && depth > 0 {
        match b[i] {
            b')' => depth -= 1,
            b'(' => depth += 1,
            c if c.is_ascii_hexdigit() => {
                // A VCP code, possibly followed by its allowed values in brackets.
                let code_start = i;
                while i < b.len() && b[i].is_ascii_hexdigit() {
                    i += 1;
                }
                let code = &caps[code_start..i];
                let rest = caps[i..].trim_start_matches(' ');
                if depth == 1 && code.eq_ignore_ascii_case("60") && rest.starts_with('(') {
                    return rest.find(')').map(|close| distinct_hex(&rest[1..close])).unwrap_or_default();
                }
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    Vec::new()
}

/// The hex numbers in a space-separated list, each once, in order. Anything else is skipped.
fn distinct_hex(list: &str) -> Vec<u8> {
    let mut codes = Vec::new();
    for code in list.split_ascii_whitespace().filter_map(|v| u8::from_str_radix(v, 16).ok()) {
        if !codes.contains(&code) {
            codes.push(code);
        }
    }
    codes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_from_real_dell_monitors() {
        let p2419h = "(prot(monitor)type(lcd)model(P2419H)cmds(01 02 03 07 0C E3 F3)vcp(02 04 05 08 10 12 14(05 08 0B 0C) 16 18 1A 52 60(01 0F 11) AA(01 02 04) AC AE B2 B6 C6 C8 C9 CC(02 03 04 06 09 0a 0d 0e) D6(01 04 05) DC(00 03 05 ) DF E0 E1 E2(00 02 04 0E 12 14 1D) F0(00 0C) F1 F2 FD)mccs_ver(2.1)mswhql(1))";
        assert_eq!(parse_capabilities(p2419h), vec![0x01, 0x0F, 0x11]);
        let u2723qe = "(prot(monitor)type(lcd)model(U2723QE)cmds(01 02 03 07 0C E3 F3)vcp(02 04 05 08 10 12 14(01 04 05 06 08 09 0B 0C) 16 18 1A 52 60( 1B 0F 11) AA(01 02 04) AC AE)mccs_ver(2.1)mswhql(1))";
        assert_eq!(parse_capabilities(u2723qe), vec![0x1B, 0x0F, 0x11]);
    }

    #[test]
    fn capabilities_edge_cases() {
        assert_eq!(parse_capabilities("(vcp(14(05 60 0B) 60(11)))"), vec![0x11]);
        assert!(parse_capabilities("").is_empty());
        assert!(parse_capabilities("(prot(monitor)type(lcd))").is_empty());
        assert!(parse_capabilities("(vcp(02 04 60(0F 11").is_empty());
    }

    #[test]
    fn names_and_actions() {
        assert_eq!(short_name(0x0F), "DisplayPort 1");
        assert_eq!(long_name(0x11), "HDMI 1 (0x11)");
        assert_eq!(short_name(0x19), "USB-C");
        assert_eq!(short_name(0x7E), "0x7E");
        assert_eq!(long_name(0x7E), "Input 0x7E");
        assert_eq!(describe_action(&[0x11]), "Switch to HDMI 1");
        assert_eq!(describe_action(&[0x0F, 0x11]), "Toggle DisplayPort 1 \u{21c4} HDMI 1");
        assert_eq!(describe_action(&[0x1B, 0x0F, 0x11]), "Cycle USB-C \u{2192} DisplayPort 1 \u{2192} HDMI 1");
    }
}
