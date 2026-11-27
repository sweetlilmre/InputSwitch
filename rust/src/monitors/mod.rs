//! The monitors: finding the connected ones (CCD API, with identities from their EDID), their
//! inputs, and switching them over DDC/CI.
//!
//! - [`inputs`]: input codes, names, and the capabilities string
//! - [`edid`]: a monitor's identity
//! - [`ddc`]: talking to a monitor
//! - [`switch`]: running a hotkey's action

mod ddc;
mod edid;
mod inputs;
mod switch;

pub use inputs::{describe_action, long_name, parse_capabilities, short_name, COMMON_INPUTS};
pub use switch::{run_binding, switch_to};

use crate::ffi::*;
use std::collections::HashMap;

/// A connected, active monitor.
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorInfo {
    /// Stable identity: manufacturer + product code + serial (falls back to the device instance).
    pub id: String,
    pub friendly: String,
    pub model: String,
    pub serial: Option<String>,
    /// The laptop's own panel (ignored when matching setups).
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

/// The input monitor `m` is showing, or None if it didn't answer.
pub fn current_input(m: &MonitorInfo) -> Option<u8> {
    ddc::with_monitor(m, |p| p.current_input())
}

/// The MCCS capabilities string. Can take a second or two.
pub fn capabilities(m: &MonitorInfo) -> Option<String> {
    ddc::with_monitor(m, |p| p.capabilities())
}

/// The inputs monitor `m` says it has (empty if it didn't say).
pub fn supported_inputs(m: &MonitorInfo) -> Vec<u8> {
    capabilities(m).map(|c| parse_capabilities(&c)).unwrap_or_default()
}

// ------------------------------------------------------------------------------------------ enumeration

/// What the CCD API says about an active display path.
struct Target {
    gdi: String,
    friendly: String,
    path: String,
    mfg: u16,
    product: u16,
    internal: bool,
}

/// Output technologies of a laptop's own panel: internal, LVDS, embedded DisplayPort, embedded UDI.
const INTERNAL_OUTPUTS: [u32; 4] = [0x8000_0000, 6, 11, 13];

fn active_targets() -> Vec<Target> {
    let mut out = Vec::new();
    unsafe {
        let (mut np, mut nm) = (0u32, 0u32);
        let mut paths = Vec::new();
        // The buffers can be outgrown between sizing and querying if displays change: try again.
        let mut rc = ERROR_INSUFFICIENT_BUFFER;
        for _ in 0..5 {
            if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut np, &mut nm) != 0 {
                return out;
            }
            paths = vec![DISPLAYCONFIG_PATH_INFO::default(); np as usize];
            let mut modes = vec![DISPLAYCONFIG_MODE_INFO { data: [0; 64] }; nm as usize];
            rc = QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &mut np, paths.as_mut_ptr(), &mut nm, modes.as_mut_ptr(), core::ptr::null_mut());
            if rc != ERROR_INSUFFICIENT_BUFFER {
                break;
            }
        }
        if rc != 0 {
            return out;
        }

        for path in &paths[..np as usize] {
            let mut source: DISPLAYCONFIG_SOURCE_DEVICE_NAME = zeroed();
            source.header.type_ = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
            source.header.size = core::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
            source.header.adapterId = path.sourceInfo.adapterId;
            source.header.id = path.sourceInfo.id;
            if DisplayConfigGetDeviceInfo(&mut source.header) != 0 {
                continue;
            }

            let mut target: DISPLAYCONFIG_TARGET_DEVICE_NAME = zeroed();
            target.header.type_ = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
            target.header.size = core::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
            target.header.adapterId = path.targetInfo.adapterId;
            target.header.id = path.targetInfo.id;
            if DisplayConfigGetDeviceInfo(&mut target.header) != 0 {
                continue;
            }

            out.push(Target {
                gdi: from_wide(&source.viewGdiDeviceName),
                friendly: from_wide(&target.monitorFriendlyDeviceName),
                path: from_wide(&target.monitorDevicePath),
                mfg: target.edidManufactureId,
                product: target.edidProductCodeId,
                internal: INTERNAL_OUTPUTS.contains(&target.outputTechnology),
            });
        }
    }
    out
}

/// The connected, active monitors.
pub fn enumerate() -> Vec<MonitorInfo> {
    let mut result = Vec::new();
    let mut per_device: HashMap<String, usize> = HashMap::new();
    for t in active_targets() {
        let edid = edid::read(&t.path).and_then(|e| edid::Edid::parse(&e));
        // CCD returns EDID bytes 8-9 as a little-endian u16: swap them back.
        let mfg = edid.as_ref().map_or_else(|| edid::decode_pnp(t.mfg.swap_bytes()), |e| e.manufacturer.clone());
        let product = edid.as_ref().map_or(t.product, |e| e.product);
        let model = format!("{mfg}{product:04X}");
        let serial = edid.as_ref().and_then(|e| e.serial());
        let id = match &serial {
            Some(s) => format!("{model}#{s}"),
            None => format!("{model}@{}", edid::device_path_to_instance(&t.path).unwrap_or_else(|| t.gdi.clone())),
        };
        let friendly = if !t.friendly.trim().is_empty() {
            t.friendly.clone()
        } else if let Some(name) = edid.as_ref().and_then(|e| e.model_name.clone()) {
            name
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
