//! A monitor's identity from its EDID (read from the registry): manufacturer, product code,
//! serial number and model name.

use crate::ffi::HKEY_LOCAL_MACHINE;
use crate::registry::RegKey;

#[derive(Debug, PartialEq)]
pub struct Edid {
    pub manufacturer: String,
    pub product: u16,
    pub serial_number: u32,
    pub serial_text: Option<String>,
    pub model_name: Option<String>,
}

impl Edid {
    /// The best serial: the text descriptor if there is one, else the numeric field.
    pub fn serial(&self) -> Option<String> {
        match &self.serial_text {
            Some(s) if !s.trim().is_empty() => Some(s.clone()),
            _ if self.serial_number != 0 => Some(format!("{:08X}", self.serial_number)),
            _ => None,
        }
    }

    pub fn parse(e: &[u8]) -> Option<Edid> {
        if e.len() < 128 {
            return None;
        }
        let manufacturer = decode_pnp(u16::from_be_bytes([e[8], e[9]]));
        let product = u16::from_le_bytes([e[10], e[11]]);
        let serial_number = u32::from_le_bytes([e[12], e[13], e[14], e[15]]);
        let (mut serial_text, mut model_name) = (None, None);
        // Four 18-byte descriptors; display descriptors start with three zero bytes and a tag.
        for off in [54usize, 72, 90, 108] {
            let tag = e[off + 3];
            if e[off..off + 3] != [0, 0, 0] || (tag != 0xFF && tag != 0xFC) {
                continue;
            }
            let text = descriptor_text(&e[off + 5..off + 18]);
            if tag == 0xFF { serial_text = Some(text) } else { model_name = Some(text) }
        }
        Some(Edid { manufacturer, product, serial_number, serial_text, model_name })
    }
}

/// A descriptor's text: up to the newline, trimmed. Decoded the way the C# app does
/// (`Encoding.ASCII`: anything above 0x7F becomes '?'), so both make the same monitor ids.
fn descriptor_text(raw: &[u8]) -> String {
    let end = raw.iter().position(|&c| c == b'\n').unwrap_or(raw.len());
    let text: String = raw[..end].iter().map(|&b| if b.is_ascii() { b as char } else { '?' }).collect();
    text.trim().to_string()
}

/// A three-letter PnP manufacturer id ("DEL"), packed five bits per letter.
pub fn decode_pnp(m: u16) -> String {
    [(m >> 10) & 0x1F, (m >> 5) & 0x1F, m & 0x1F].iter().map(|&c| (b'A' - 1 + c as u8) as char).collect()
}

/// `\\?\DISPLAY#DELA0B1#5&2a1b3c&0&UID4352#{guid}` → `DISPLAY\DELA0B1\5&2a1b3c&0&UID4352`.
pub fn device_path_to_instance(path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    let p = path.strip_prefix(r"\\?\").unwrap_or(path);
    let p = match p.rfind("#{") {
        Some(i) if i > 0 => &p[..i],
        _ => p,
    };
    Some(p.replace('#', "\\"))
}

/// The raw EDID Windows keeps for a monitor device path.
pub fn read(device_path: &str) -> Option<Vec<u8>> {
    let instance = device_path_to_instance(device_path)?;
    RegKey::open(HKEY_LOCAL_MACHINE, &format!(r"SYSTEM\CurrentControlSet\Enum\{instance}\Device Parameters"))?.binary("EDID")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_edid(mfg: &str, product: u16, serial: u32, serial_text: Option<&[u8]>, name: Option<&str>) -> Vec<u8> {
        let mut e = vec![0u8; 128];
        e[..8].copy_from_slice(&[0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0]);
        let b = mfg.as_bytes();
        let m = (((b[0] - b'A' + 1) as u16) << 10) | (((b[1] - b'A' + 1) as u16) << 5) | (b[2] - b'A' + 1) as u16;
        e[8..10].copy_from_slice(&m.to_be_bytes());
        e[10..12].copy_from_slice(&product.to_le_bytes());
        e[12..16].copy_from_slice(&serial.to_le_bytes());
        for (off, tag, text) in [(54usize, 0xFFu8, serial_text), (72, 0xFC, name.map(str::as_bytes))] {
            if let Some(t) = text {
                e[off + 3] = tag;
                let mut bytes = [t, b"\n"].concat();
                bytes.resize(13, b' ');
                e[off + 5..off + 18].copy_from_slice(&bytes);
            }
        }
        e
    }

    #[test]
    fn identity() {
        let e = Edid::parse(&build_edid("DEL", 0xD0D9, 0x12345678, Some(b"BPG5ZT2"), Some("DELL P2419H"))).unwrap();
        assert_eq!(e.manufacturer, "DEL");
        assert_eq!(e.product, 0xD0D9);
        assert_eq!(e.serial().as_deref(), Some("BPG5ZT2"));
        assert_eq!(e.model_name.as_deref(), Some("DELL P2419H"));
        assert_eq!(Edid::parse(&build_edid("DEL", 1, 0x12345678, None, None)).unwrap().serial().as_deref(), Some("12345678"));
        assert_eq!(Edid::parse(&build_edid("DEL", 1, 0, None, None)).unwrap().serial(), None);
        assert!(Edid::parse(&[0u8; 64]).is_none());
    }

    #[test]
    fn non_ascii_serials_decode_like_the_csharp_app() {
        let e = Edid::parse(&build_edid("DEL", 1, 0, Some(b"AB\xE9C"), None)).unwrap();
        assert_eq!(e.serial().as_deref(), Some("AB?C"));
    }

    #[test]
    fn device_paths() {
        assert_eq!(
            device_path_to_instance(r"\\?\DISPLAY#DELA0B1#5&2a1b3c&0&UID4352#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}").as_deref(),
            Some(r"DISPLAY\DELA0B1\5&2a1b3c&0&UID4352")
        );
    }
}
