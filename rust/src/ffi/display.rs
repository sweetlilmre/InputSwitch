//! Monitors: enumeration, the display configuration (CCD) API, and DDC/CI (dxva2).

use super::*;
use core::ffi::c_void;

pub type MONITORENUMPROC = unsafe extern "system" fn(HMONITOR, HDC, *mut RECT, LPARAM) -> BOOL;

#[repr(C)]
pub struct MONITORINFOEXW {
    pub cbSize: u32,
    pub rcMonitor: RECT,
    pub rcWork: RECT,
    pub dwFlags: u32,
    pub szDevice: [u16; 32],
}

#[repr(C)]
pub struct PHYSICAL_MONITOR {
    pub hPhysicalMonitor: HANDLE,
    pub szPhysicalMonitorDescription: [u16; 128],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LUID {
    pub LowPart: u32,
    pub HighPart: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct DISPLAYCONFIG_PATH_SOURCE_INFO {
    pub adapterId: LUID,
    pub id: u32,
    pub modeInfoIdx: u32,
    pub statusFlags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct DISPLAYCONFIG_PATH_TARGET_INFO {
    pub adapterId: LUID,
    pub id: u32,
    pub modeInfoIdx: u32,
    pub outputTechnology: u32,
    pub rotation: u32,
    pub scaling: u32,
    pub refreshNumerator: u32,
    pub refreshDenominator: u32,
    pub scanLineOrdering: u32,
    pub targetAvailable: BOOL,
    pub statusFlags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct DISPLAYCONFIG_PATH_INFO {
    pub sourceInfo: DISPLAYCONFIG_PATH_SOURCE_INFO,
    pub targetInfo: DISPLAYCONFIG_PATH_TARGET_INFO,
    pub flags: u32,
}

/// Opaque 64-byte DISPLAYCONFIG_MODE_INFO (only needed as buffer space).
#[repr(C, align(8))]
#[derive(Clone, Copy)]
pub struct DISPLAYCONFIG_MODE_INFO {
    pub data: [u8; 64],
}

#[repr(C)]
pub struct DISPLAYCONFIG_DEVICE_INFO_HEADER {
    pub type_: u32,
    pub size: u32,
    pub adapterId: LUID,
    pub id: u32,
}

#[repr(C)]
pub struct DISPLAYCONFIG_TARGET_DEVICE_NAME {
    pub header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    pub flags: u32,
    pub outputTechnology: u32,
    pub edidManufactureId: u16,
    pub edidProductCodeId: u16,
    pub connectorInstance: u32,
    pub monitorFriendlyDeviceName: [u16; 64],
    pub monitorDevicePath: [u16; 128],
}

#[repr(C)]
pub struct DISPLAYCONFIG_SOURCE_DEVICE_NAME {
    pub header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    pub viewGdiDeviceName: [u16; 32],
}

pub const QDC_ONLY_ACTIVE_PATHS: u32 = 0x2;
pub const DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME: u32 = 1;
pub const DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME: u32 = 2;

#[link(name = "dxva2")]
extern "system" {
    pub fn GetNumberOfPhysicalMonitorsFromHMONITOR(m: HMONITOR, n: *mut u32) -> BOOL;
    pub fn GetPhysicalMonitorsFromHMONITOR(m: HMONITOR, n: u32, out: *mut PHYSICAL_MONITOR) -> BOOL;
    pub fn DestroyPhysicalMonitors(n: u32, arr: *const PHYSICAL_MONITOR) -> BOOL;
    pub fn GetVCPFeatureAndVCPFeatureReply(h: HANDLE, code: u8, kind: *mut u32, cur: *mut u32, max: *mut u32) -> BOOL;
    pub fn SetVCPFeature(h: HANDLE, code: u8, value: u32) -> BOOL;
    pub fn GetCapabilitiesStringLength(h: HANDLE, len: *mut u32) -> BOOL;
    pub fn CapabilitiesRequestAndCapabilitiesReply(h: HANDLE, buf: *mut u8, len: u32) -> BOOL;
}

#[link(name = "user32")]
extern "system" {
    pub fn EnumDisplayMonitors(hdc: HDC, clip: *const RECT, f: MONITORENUMPROC, data: LPARAM) -> BOOL;
    pub fn GetMonitorInfoW(m: HMONITOR, info: *mut MONITORINFOEXW) -> BOOL;
    pub fn GetDisplayConfigBufferSizes(flags: u32, paths: *mut u32, modes: *mut u32) -> i32;
    pub fn QueryDisplayConfig(flags: u32, np: *mut u32, paths: *mut DISPLAYCONFIG_PATH_INFO, nm: *mut u32,
        modes: *mut DISPLAYCONFIG_MODE_INFO, topology: *mut c_void) -> i32;
    pub fn DisplayConfigGetDeviceInfo(header: *mut DISPLAYCONFIG_DEVICE_INFO_HEADER) -> i32;
}
