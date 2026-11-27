//! A registry key that closes itself, with the few value types InputSwitch reads and writes.

use crate::ffi::*;
use core::ptr::{null, null_mut};

pub struct RegKey(HKEY);

impl RegKey {
    /// Opens an existing key for reading.
    pub fn open(root: HKEY, path: &str) -> Option<RegKey> {
        let mut key: HKEY = 0;
        let rc = unsafe { RegOpenKeyExW(root, wide(path).as_ptr(), 0, KEY_READ, &mut key) };
        (rc == 0).then_some(RegKey(key))
    }

    /// Opens a key for writing, creating it if needed.
    pub fn create(root: HKEY, path: &str) -> Option<RegKey> {
        let mut key: HKEY = 0;
        let rc = unsafe { RegCreateKeyExW(root, wide(path).as_ptr(), 0, null(), 0, KEY_WRITE, null(), &mut key, null_mut()) };
        (rc == 0).then_some(RegKey(key))
    }

    pub fn exists(&self, name: &str) -> bool {
        unsafe { RegQueryValueExW(self.0, wide(name).as_ptr(), null_mut(), null_mut(), null_mut(), null_mut()) == 0 }
    }

    pub fn dword(&self, name: &str) -> Option<u32> {
        let (mut value, mut len, mut kind) = (0u32, 4u32, 0u32);
        let rc = unsafe { RegQueryValueExW(self.0, wide(name).as_ptr(), null_mut(), &mut kind, &mut value as *mut u32 as *mut u8, &mut len) };
        (rc == 0 && kind == REG_DWORD).then_some(value)
    }

    /// A REG_SZ value of up to 1023 characters.
    pub fn string(&self, name: &str) -> Option<String> {
        let mut buf = [0u16; 1024];
        let (mut len, mut kind) = ((buf.len() * 2) as u32, 0u32);
        let rc = unsafe { RegQueryValueExW(self.0, wide(name).as_ptr(), null_mut(), &mut kind, buf.as_mut_ptr() as *mut u8, &mut len) };
        (rc == 0 && kind == REG_SZ).then(|| from_wide(&buf))
    }

    pub fn set_string(&self, name: &str, value: &str) -> bool {
        let value = wide(value);
        unsafe { RegSetValueExW(self.0, wide(name).as_ptr(), 0, REG_SZ, value.as_ptr() as *const u8, (value.len() * 2) as u32) == 0 }
    }

    /// Deletes a value; true if it's gone (including if it was never there).
    pub fn delete(&self, name: &str) -> bool {
        matches!(unsafe { RegDeleteValueW(self.0, wide(name).as_ptr()) }, 0 | ERROR_FILE_NOT_FOUND)
    }
}

impl Drop for RegKey {
    fn drop(&mut self) {
        unsafe { RegCloseKey(self.0) };
    }
}
