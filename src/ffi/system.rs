//! Kernel, registry, errors and cryptography.

use super::*;
use core::ffi::c_void;

#[repr(C)]
pub struct SYSTEMTIME {
    pub wYear: u16,
    pub wMonth: u16,
    pub wDayOfWeek: u16,
    pub wDay: u16,
    pub wHour: u16,
    pub wMinute: u16,
    pub wSecond: u16,
    pub wMilliseconds: u16,
}

pub const HKEY_CURRENT_USER: HKEY = 0x8000_0001u32 as i32 as isize;
pub const HKEY_LOCAL_MACHINE: HKEY = 0x8000_0002u32 as i32 as isize;
pub const KEY_READ: u32 = 0x20019;
pub const KEY_WRITE: u32 = 0x20006;
pub const REG_SZ: u32 = 1;
pub const REG_DWORD: u32 = 4;
pub const ERROR_FILE_NOT_FOUND: i32 = 2;
pub const ERROR_INSUFFICIENT_BUFFER: i32 = 122;
pub const ERROR_ALREADY_EXISTS: u32 = 183;

#[link(name = "advapi32")]
extern "system" {
    pub fn RegOpenKeyExW(key: HKEY, sub: *const u16, opts: u32, sam: u32, out: *mut HKEY) -> i32;
    pub fn RegCreateKeyExW(key: HKEY, sub: *const u16, reserved: u32, class: *const u16, opts: u32, sam: u32,
        sec: *const c_void, out: *mut HKEY, disp: *mut u32) -> i32;
    pub fn RegQueryValueExW(key: HKEY, name: *const u16, reserved: *mut u32, kind: *mut u32, data: *mut u8, len: *mut u32) -> i32;
    pub fn RegSetValueExW(key: HKEY, name: *const u16, reserved: u32, kind: u32, data: *const u8, len: u32) -> i32;
    pub fn RegDeleteValueW(key: HKEY, name: *const u16) -> i32;
    pub fn RegCloseKey(key: HKEY) -> i32;
}

#[link(name = "bcrypt")]
extern "system" {
    pub fn BCryptGenRandom(alg: isize, buf: *mut u8, len: u32, flags: u32) -> i32;
    pub fn BCryptHash(alg: isize, secret: *const u8, secret_len: u32, input: *const u8, input_len: u32, output: *mut u8, output_len: u32) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    pub fn GetModuleHandleW(name: *const u16) -> HINSTANCE;
    pub fn CreateMutexW(attrs: *const c_void, owner: BOOL, name: *const u16) -> HANDLE;
    pub fn GetLastError() -> u32;
    pub fn GetSystemTime(t: *mut SYSTEMTIME);
    pub fn LoadLibraryW(name: *const u16) -> HINSTANCE;
    pub fn GetProcAddress(module: HINSTANCE, name: *const u8) -> *const core::ffi::c_void;
}
