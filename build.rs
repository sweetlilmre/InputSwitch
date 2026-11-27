// Build script, with no build dependencies:
// - embeds the application manifest (visual styles, DPI awareness) via the MSVC linker;
// - writes a Windows resource file (.res) with the app icon and version information, which the
//   linker includes directly (no resource compiler needed). The version information gives the
//   app its name ("InputSwitch") in notifications, Task Manager and file properties;
// - records the git commit for the version string.

use std::process::Command;

fn main() {
    let dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=assets/InputSwitch.ico");

    let commit = git_commit();
    println!("cargo:rustc-env=GIT_COMMIT={commit}");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");

    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return;
    }
    println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}", dir.join("app.manifest").display());

    let ico = std::fs::read(dir.join("assets/InputSwitch.ico")).expect("assets/InputSwitch.ico");
    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let full = if commit.is_empty() { version.clone() } else { format!("{version}+{commit}") };
    let res = resources(&ico, &version, &full);
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("InputSwitch.res");
    std::fs::write(&out, res).unwrap();
    println!("cargo:rustc-link-arg-bins={}", out.display());
}

/// The commit being built, or "" if git isn't available (or this isn't a checkout).
fn git_commit() -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

// ------------------------------------------------------------------------------------------ .res

const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
const RT_VERSION: u16 = 16;

// Memory flags, as rc.exe sets them for each kind of resource.
const MOVEABLE: u16 = 0x0010;
const PURE: u16 = 0x0020;
const DISCARDABLE: u16 = 0x1000;

fn pad4(v: &mut Vec<u8>) {
    while !v.len().is_multiple_of(4) {
        v.push(0);
    }
}

fn utf16z(s: &str) -> Vec<u8> {
    s.encode_utf16().chain(Some(0)).flat_map(|c| c.to_le_bytes()).collect()
}

/// One entry of a .res file: header (numeric type and name) followed by the data.
fn entry(out: &mut Vec<u8>, kind: u16, name: u16, flags: u16, data: &[u8]) {
    out.extend((data.len() as u32).to_le_bytes()); // DataSize
    out.extend(32u32.to_le_bytes()); // HeaderSize
    out.extend([0xFF, 0xFF]);
    out.extend(kind.to_le_bytes());
    out.extend([0xFF, 0xFF]);
    out.extend(name.to_le_bytes());
    out.extend(0u32.to_le_bytes()); // DataVersion
    out.extend(flags.to_le_bytes()); // MemoryFlags
    out.extend(0x0409u16.to_le_bytes()); // LanguageId: English (US)
    out.extend(0u32.to_le_bytes()); // Version
    out.extend(0u32.to_le_bytes()); // Characteristics
    out.extend(data);
    pad4(out);
}

/// The whole .res file: the app icon (every size in the .ico) and the version information.
fn resources(ico: &[u8], version: &str, full_version: &str) -> Vec<u8> {
    let mut out = Vec::new();
    entry(&mut out, 0, 0, 0, &[]); // the empty first entry every .res file starts with
    icon(&mut out, ico);
    entry(&mut out, RT_VERSION, 1, MOVEABLE | PURE, &version_info(version, full_version));
    out
}

/// One RT_ICON per frame of the .ico, then an RT_GROUP_ICON (id 1) listing them. The .ico is a
/// 6-byte header (its count at offset 4) and a 16-byte directory entry per frame.
fn icon(out: &mut Vec<u8>, ico: &[u8]) {
    let u16_at = |o: usize| u16::from_le_bytes([ico[o], ico[o + 1]]);
    let u32_at = |o: usize| u32::from_le_bytes([ico[o], ico[o + 1], ico[o + 2], ico[o + 3]]);
    let count = u16_at(4);
    let mut group = Vec::new();
    group.extend(0u16.to_le_bytes()); // reserved
    group.extend(1u16.to_le_bytes()); // type: icon
    group.extend(count.to_le_bytes());
    for i in 0..count as usize {
        let e = 6 + i * 16;
        let (size, offset) = (u32_at(e + 8) as usize, u32_at(e + 12) as usize);
        let id = i as u16 + 1;
        entry(out, RT_ICON, id, MOVEABLE | DISCARDABLE, &ico[offset..offset + size]);
        group.extend(&ico[e..e + 12]); // width, height, colours, reserved, planes, bit count, size
        group.extend(id.to_le_bytes());
    }
    entry(out, RT_GROUP_ICON, 1, MOVEABLE | PURE | DISCARDABLE, &group);
}

/// A VS_VERSIONINFO-style node: header, key, value, children, each part DWORD-aligned.
fn node(key: &str, value: &[u8], value_len: u16, text: bool, children: &[Vec<u8>]) -> Vec<u8> {
    let mut v = vec![0, 0];
    v.extend(value_len.to_le_bytes());
    v.extend((text as u16).to_le_bytes());
    v.extend(utf16z(key));
    pad4(&mut v);
    v.extend(value);
    for child in children {
        pad4(&mut v);
        v.extend(child);
    }
    let len = v.len() as u16;
    v[0..2].copy_from_slice(&len.to_le_bytes());
    v
}

/// The VS_VERSIONINFO resource: the numeric version, and the strings Windows shows.
fn version_info(version: &str, full_version: &str) -> Vec<u8> {
    let parts: Vec<u32> = version.split(['.', '-']).take(3).map(|p| p.parse().unwrap_or(0)).collect();
    let (major, minor, patch) = (parts[0], parts.get(1).copied().unwrap_or(0), parts.get(2).copied().unwrap_or(0));

    let mut fixed = Vec::new();
    for v in [
        0xFEEF04BDu32,              // signature
        0x0001_0000,                // structure version
        (major << 16) | minor,      // file version
        patch << 16,
        (major << 16) | minor,      // product version
        patch << 16,
        0x3F,                       // flags mask
        0,                          // flags
        0x0004_0004,                // VOS_NT_WINDOWS32
        1,                          // VFT_APP
        0, 0, 0,                    // subtype, date
    ] {
        fixed.extend(v.to_le_bytes());
    }

    let string = |key: &str, value: &str| {
        let data = utf16z(value);
        node(key, &data, (data.len() / 2) as u16, true, &[])
    };
    let strings = [
        string("FileDescription", "InputSwitch"),
        string("ProductName", "InputSwitch"),
        string("InternalName", "InputSwitch"),
        string("OriginalFilename", "InputSwitch.exe"),
        string("FileVersion", version),
        string("ProductVersion", full_version),
    ];
    let table = node("040904b0", &[], 0, true, &strings); // English (US), Unicode
    let string_info = node("StringFileInfo", &[], 0, true, &[table]);
    let translation = node("Translation", &0x04B0_0409u32.to_le_bytes(), 4, false, &[]);
    let var_info = node("VarFileInfo", &[], 0, true, &[translation]);
    node("VS_VERSION_INFO", &fixed, fixed.len() as u16, false, &[string_info, var_info])
}
