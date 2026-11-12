//! Finding monitors (CCD API + EDID from the registry) and talking to them over DDC/CI (dxva2).

use crate::ffi::*;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

pub const VCP_INPUT_SOURCE: u8 = 0x60;

/// A connected, active monitor.
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorInfo {
    /// Stable identity: manufacturer + product code + serial (falls back to the device instance).
    pub id: String,
    pub friendly: String,
    pub model: String,
    pub serial: Option<String>,
    pub internal: bool,
    /// GDI device (\\.\DISPLAYn), used to reach the monitor over DDC/CI.
    pub gdi: String,
    /// Position among monitors sharing the same GDI device (clone mode).
    pub index: usize,
}

impl MonitorInfo {
    pub fn display_name(&self) -> &str {
        if self.friendly.trim().is_empty() { &self.model } else { &self.friendly }
    }
}

// ------------------------------------------------------------------------------------------ input names

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

pub fn short_name(code: u8) -> String {
    INPUT_NAMES.iter().find(|(c, _)| *c == code).map(|(_, n)| n.to_string()).unwrap_or_else(|| format!("0x{code:02X}"))
}

pub fn long_name(code: u8) -> String {
    match INPUT_NAMES.iter().find(|(c, _)| *c == code) {
        Some((_, n)) => format!("{n} (0x{code:02X})"),
        None => format!("Input 0x{code:02X}"),
    }
}

/// "Switch to HDMI 1" / "Toggle DisplayPort 1 ⇄ HDMI 1" / "Cycle A → B → C".
pub fn describe_action(inputs: &[u8]) -> String {
    let names: Vec<String> = inputs.iter().map(|&c| short_name(c)).collect();
    match names.len() {
        0 => "(no inputs)".into(),
        1 => format!("Switch to {}", names[0]),
        2 => format!("Toggle {} \u{21c4} {}", names[0], names[1]),
        _ => format!("Cycle {}", names.join(" \u{2192} ")),
    }
}

/// Extracts the allowed values of VCP 0x60 from an MCCS capabilities string.
pub fn parse_capabilities(caps: &str) -> Vec<u8> {
    let mut result = Vec::new();
    let lower = caps.to_ascii_lowercase();
    let Some(start) = lower.find("vcp(") else { return result };
    let b = caps.as_bytes();
    let mut i = start + 4;
    let mut depth = 1;
    while i < b.len() && depth > 0 {
        let c = b[i];
        if c == b')' { depth -= 1; i += 1; continue; }
        if c == b'(' { depth += 1; i += 1; continue; }
        if !c.is_ascii_hexdigit() { i += 1; continue; }

        let tok_start = i;
        while i < b.len() && b[i].is_ascii_hexdigit() { i += 1; }
        let token = &caps[tok_start..i];
        let mut j = i;
        while j < b.len() && b[j] == b' ' { j += 1; }
        let has_values = j < b.len() && b[j] == b'(';

        if depth == 1 && has_values && token.eq_ignore_ascii_case("60") {
            let Some(close) = caps[j..].find(')') else { break };
            for v in caps[j + 1..j + close].split(' ').filter(|s| !s.is_empty()) {
                if let Ok(code) = u8::from_str_radix(v, 16) {
                    if !result.contains(&code) { result.push(code); }
                }
            }
            break;
        }
    }
    result
}

// ------------------------------------------------------------------------------------------ EDID

#[derive(Debug, PartialEq)]
pub struct Edid {
    pub manufacturer: String,
    pub product: u16,
    pub serial_number: u32,
    pub serial_text: Option<String>,
    pub model_name: Option<String>,
}

impl Edid {
    /// Best available serial: the text descriptor if present, else the numeric field.
    pub fn serial(&self) -> Option<String> {
        match &self.serial_text {
            Some(s) if !s.trim().is_empty() => Some(s.clone()),
            _ if self.serial_number != 0 => Some(format!("{:08X}", self.serial_number)),
            _ => None,
        }
    }

    pub fn parse(e: &[u8]) -> Option<Edid> {
        if e.len() < 128 { return None; }
        let m = ((e[8] as u16) << 8) | e[9] as u16;
        let manufacturer = decode_pnp(m);
        let product = e[10] as u16 | ((e[11] as u16) << 8);
        let serial_number = u32::from_le_bytes([e[12], e[13], e[14], e[15]]);
        let (mut serial_text, mut model_name) = (None, None);
        for off in [54usize, 72, 90, 108] {
            if e[off] != 0 || e[off + 1] != 0 || e[off + 2] != 0 { continue; }
            let tag = e[off + 3];
            if tag != 0xFF && tag != 0xFC { continue; }
            let raw = &e[off + 5..off + 18];
            let end = raw.iter().position(|&c| c == b'\n').unwrap_or(raw.len());
            let text = String::from_utf8_lossy(&raw[..end]).trim().to_string();
            if tag == 0xFF { serial_text = Some(text) } else { model_name = Some(text) }
        }
        Some(Edid { manufacturer, product, serial_number, serial_text, model_name })
    }
}

fn decode_pnp(m: u16) -> String {
    [(m >> 10) & 0x1F, (m >> 5) & 0x1F, m & 0x1F].iter().map(|&c| (b'A' - 1 + c as u8) as char).collect()
}

/// `\\?\DISPLAY#DELA0B1#5&2a1b3c&0&UID4352#{guid}` → `DISPLAY\DELA0B1\5&2a1b3c&0&UID4352`.
pub fn device_path_to_instance(path: &str) -> Option<String> {
    if path.is_empty() { return None; }
    let p = path.strip_prefix(r"\\?\").unwrap_or(path);
    let p = match p.rfind("#{") { Some(i) if i > 0 => &p[..i], _ => p };
    Some(p.replace('#', "\\"))
}

fn read_edid(device_path: &str) -> Option<Vec<u8>> {
    let instance = device_path_to_instance(device_path)?;
    let sub = wide(&format!(r"SYSTEM\CurrentControlSet\Enum\{instance}\Device Parameters"));
    unsafe {
        let mut key: HKEY = 0;
        if RegOpenKeyExW(HKEY_LOCAL_MACHINE, sub.as_ptr(), 0, KEY_READ, &mut key) != 0 { return None; }
        let name = wide("EDID");
        let mut len = 0u32;
        let mut result = None;
        if RegQueryValueExW(key, name.as_ptr(), core::ptr::null_mut(), core::ptr::null_mut(), core::ptr::null_mut(), &mut len) == 0 && len > 0 {
            let mut buf = vec![0u8; len as usize];
            if RegQueryValueExW(key, name.as_ptr(), core::ptr::null_mut(), core::ptr::null_mut(), buf.as_mut_ptr(), &mut len) == 0 {
                buf.truncate(len as usize);
                result = Some(buf);
            }
        }
        RegCloseKey(key);
        result
    }
}

// ------------------------------------------------------------------------------------------ enumeration

struct Target {
    gdi: String,
    friendly: String,
    path: String,
    mfg: u16,
    product: u16,
    internal: bool,
}

fn active_targets() -> Vec<Target> {
    let mut out = Vec::new();
    unsafe {
        let (mut np, mut nm) = (0u32, 0u32);
        let mut paths;
        let mut rc;
        loop {
            if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut np, &mut nm) != 0 { return out; }
            paths = vec![DISPLAYCONFIG_PATH_INFO::default(); np as usize];
            let mut modes = vec![DISPLAYCONFIG_MODE_INFO { data: [0; 64] }; nm as usize];
            rc = QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &mut np, paths.as_mut_ptr(), &mut nm, modes.as_mut_ptr(), core::ptr::null_mut());
            if rc != ERROR_INSUFFICIENT_BUFFER { break; }
        }
        if rc != 0 { return out; }

        for path in &paths[..np as usize] {
            let mut source: DISPLAYCONFIG_SOURCE_DEVICE_NAME = zeroed();
            source.header.type_ = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
            source.header.size = core::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
            source.header.adapterId = path.sourceInfo.adapterId;
            source.header.id = path.sourceInfo.id;
            if DisplayConfigGetDeviceInfo(&mut source.header) != 0 { continue; }

            let mut target: DISPLAYCONFIG_TARGET_DEVICE_NAME = zeroed();
            target.header.type_ = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
            target.header.size = core::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
            target.header.adapterId = path.targetInfo.adapterId;
            target.header.id = path.targetInfo.id;
            if DisplayConfigGetDeviceInfo(&mut target.header) != 0 { continue; }

            // Internal, LVDS, embedded DisplayPort, embedded UDI: the laptop's own panel.
            let tech = target.outputTechnology;
            out.push(Target {
                gdi: from_wide(&source.viewGdiDeviceName),
                friendly: from_wide(&target.monitorFriendlyDeviceName),
                path: from_wide(&target.monitorDevicePath),
                mfg: target.edidManufactureId,
                product: target.edidProductCodeId,
                internal: matches!(tech, 0x8000_0000 | 6 | 11 | 13),
            });
        }
    }
    out
}

pub fn enumerate() -> Vec<MonitorInfo> {
    let mut result = Vec::new();
    let mut per_device: HashMap<String, usize> = HashMap::new();
    for t in active_targets() {
        let edid = read_edid(&t.path).and_then(|e| Edid::parse(&e));
        // CCD returns EDID bytes 8-9 as a little-endian u16; swap back.
        let mfg = edid.as_ref().map(|e| e.manufacturer.clone()).unwrap_or_else(|| decode_pnp(t.mfg.swap_bytes()));
        let product = edid.as_ref().map(|e| e.product).unwrap_or(t.product);
        let model = format!("{mfg}{product:04X}");
        let serial = edid.as_ref().and_then(|e| e.serial());
        let id = match &serial {
            Some(s) => format!("{model}#{s}"),
            None => format!("{model}@{}", device_path_to_instance(&t.path).unwrap_or_else(|| t.gdi.clone())),
        };
        let friendly = if !t.friendly.trim().is_empty() {
            t.friendly.clone()
        } else if let Some(n) = edid.as_ref().and_then(|e| e.model_name.clone()) {
            n
        } else if t.internal {
            "Built-in display".into()
        } else {
            model.clone()
        };
        let index = per_device.entry(t.gdi.to_ascii_uppercase()).or_insert(0);
        result.push(MonitorInfo { id, friendly, model, serial, internal: t.internal, gdi: t.gdi, index: *index });
        *index += 1;
    }
    result
}

// ------------------------------------------------------------------------------------------ DDC/CI

/// The I²C bus is slow and some monitors misbehave under concurrent requests: one at a time.
static DDC_LOCK: Mutex<()> = Mutex::new(());

struct Find {
    name: Vec<u16>,
    found: HMONITOR,
}

unsafe extern "system" fn find_proc(h: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
    let f = &mut *(data as *mut Find);
    let mut info: MONITORINFOEXW = zeroed();
    info.cbSize = core::mem::size_of::<MONITORINFOEXW>() as u32;
    if GetMonitorInfoW(h, &mut info) != 0 {
        let dev = from_wide(&info.szDevice);
        if dev.eq_ignore_ascii_case(&from_wide(&f.name)) {
            f.found = h;
            return 0;
        }
    }
    1
}

fn with_physical<T>(m: &MonitorInfo, action: impl FnOnce(HANDLE) -> Option<T>) -> Option<T> {
    unsafe {
        let mut find = Find { name: wide(&m.gdi), found: 0 };
        EnumDisplayMonitors(0, core::ptr::null(), find_proc, &mut find as *mut Find as LPARAM);
        if find.found == 0 { return None; }

        let _guard = DDC_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut count = 0u32;
        if GetNumberOfPhysicalMonitorsFromHMONITOR(find.found, &mut count) == 0 || count == 0 { return None; }
        let mut arr: Vec<PHYSICAL_MONITOR> = (0..count).map(|_| zeroed()).collect();
        if GetPhysicalMonitorsFromHMONITOR(find.found, count, arr.as_mut_ptr()) == 0 { return None; }
        let idx = m.index.min(count as usize - 1);
        let r = action(arr[idx].hPhysicalMonitor);
        DestroyPhysicalMonitors(count, arr.as_ptr());
        r
    }
}

fn pause(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
}

/// The input the monitor is showing, or None if it didn't answer.
pub fn current_input(m: &MonitorInfo) -> Option<u8> {
    with_physical(m, |h| {
        for _ in 0..3 {
            let (mut kind, mut cur, mut max) = (0u32, 0u32, 0u32);
            if unsafe { GetVCPFeatureAndVCPFeatureReply(h, VCP_INPUT_SOURCE, &mut kind, &mut cur, &mut max) } != 0 {
                return Some((cur & 0xFF) as u8);
            }
            pause(60);
        }
        None
    })
}

pub fn set_input(m: &MonitorInfo, code: u8) -> bool {
    with_physical(m, |h| {
        for _ in 0..3 {
            if unsafe { SetVCPFeature(h, VCP_INPUT_SOURCE, code as u32) } != 0 { return Some(true); }
            pause(60);
        }
        Some(false)
    })
    .unwrap_or(false)
}

/// The MCCS capabilities string. Can take a second or two.
pub fn capabilities(m: &MonitorInfo) -> Option<String> {
    with_physical(m, |h| unsafe {
        for _ in 0..2 {
            let mut len = 0u32;
            if GetCapabilitiesStringLength(h, &mut len) != 0 && len > 0 {
                let mut buf = vec![0u8; len as usize];
                if CapabilitiesRequestAndCapabilitiesReply(h, buf.as_mut_ptr(), len) != 0 {
                    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                    return Some(String::from_utf8_lossy(&buf[..end]).into_owned());
                }
            }
            pause(100);
        }
        None
    })
}

pub fn supported_inputs(m: &MonitorInfo) -> Vec<u8> {
    capabilities(m).map(|c| parse_capabilities(&c)).unwrap_or_default()
}

// ------------------------------------------------------------------------------------------ switching

/// What each monitor was last switched to, for when it can't report its current input.
fn last_set() -> &'static Mutex<HashMap<String, u8>> {
    static LAST: OnceLock<Mutex<HashMap<String, u8>>> = OnceLock::new();
    LAST.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Runs a binding: one input switches to it; several step through them in order.
/// Blocking (DDC/CI is slow); call it off the UI thread.
pub fn run_binding(inputs: &[u8], m: &MonitorInfo) -> Result<u8, String> {
    if inputs.is_empty() {
        return Err("No inputs are selected for this hotkey.".into());
    }
    let target = if inputs.len() == 1 {
        inputs[0]
    } else {
        let mut current = current_input(m);
        if current.is_none_or(|c| !inputs.contains(&c)) {
            if let Some(&last) = last_set().lock().unwrap_or_else(|e| e.into_inner()).get(&m.id) {
                current = Some(last);
            }
        }
        let next = current.and_then(|c| inputs.iter().position(|&x| x == c)).map(|i| i + 1).unwrap_or(0);
        inputs[next % inputs.len()]
    };
    set(m, target)
}

pub fn set(m: &MonitorInfo, input: u8) -> Result<u8, String> {
    if !set_input(m, input) {
        return Err(format!(
            "{} did not accept the switch to {}. Check that DDC/CI is enabled in the monitor's on-screen menu.",
            m.display_name(),
            short_name(input)
        ));
    }
    last_set().lock().unwrap_or_else(|e| e.into_inner()).insert(m.id.clone(), input);
    Ok(input)
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
        assert_eq!(describe_action(&[0x11]), "Switch to HDMI 1");
        assert_eq!(describe_action(&[0x0F, 0x11]), "Toggle DisplayPort 1 \u{21c4} HDMI 1");
        assert_eq!(describe_action(&[0x1B, 0x0F, 0x11]), "Cycle USB-C \u{2192} DisplayPort 1 \u{2192} HDMI 1");
    }

    fn build_edid(mfg: &str, product: u16, serial: u32, serial_text: Option<&str>, name: Option<&str>) -> Vec<u8> {
        let mut e = vec![0u8; 128];
        e[..8].copy_from_slice(&[0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0]);
        let b = mfg.as_bytes();
        let m = (((b[0] - b'A' + 1) as u16) << 10) | (((b[1] - b'A' + 1) as u16) << 5) | (b[2] - b'A' + 1) as u16;
        e[8] = (m >> 8) as u8;
        e[9] = m as u8;
        e[10..12].copy_from_slice(&product.to_le_bytes());
        e[12..16].copy_from_slice(&serial.to_le_bytes());
        for (off, tag, text) in [(54usize, 0xFFu8, serial_text), (72, 0xFC, name)] {
            if let Some(t) = text {
                e[off + 3] = tag;
                let mut bytes = format!("{t}\n").into_bytes();
                bytes.resize(13, b' ');
                e[off + 5..off + 18].copy_from_slice(&bytes);
            }
        }
        e
    }

    #[test]
    fn edid_identity() {
        let e = Edid::parse(&build_edid("DEL", 0xD0D9, 0x12345678, Some("BPG5ZT2"), Some("DELL P2419H"))).unwrap();
        assert_eq!(e.manufacturer, "DEL");
        assert_eq!(e.product, 0xD0D9);
        assert_eq!(e.serial().as_deref(), Some("BPG5ZT2"));
        assert_eq!(e.model_name.as_deref(), Some("DELL P2419H"));
        assert_eq!(Edid::parse(&build_edid("DEL", 1, 0x12345678, None, None)).unwrap().serial().as_deref(), Some("12345678"));
        assert_eq!(Edid::parse(&build_edid("DEL", 1, 0, None, None)).unwrap().serial(), None);
        assert!(Edid::parse(&[0u8; 64]).is_none());
    }

    #[test]
    fn device_paths() {
        assert_eq!(
            device_path_to_instance(r"\\?\DISPLAY#DELA0B1#5&2a1b3c&0&UID4352#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}").as_deref(),
            Some(r"DISPLAY\DELA0B1\5&2a1b3c&0&UID4352")
        );
    }
}
