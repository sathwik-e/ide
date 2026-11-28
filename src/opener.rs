//! Getting files into Orion from anywhere: Finder double-click, "Open With",
//! dropping on the Dock icon, `open -a Orion file`, the command line, or the
//! Open dialog. Also decides how any file's bytes become something viewable.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use gpui_kit::{AnyWindowHandle, App, Global, WeakEntity};

use crate::workspace::Workspace;

/// Paths macOS asked us to open that haven't been handled yet.
#[derive(Clone, Default)]
pub struct Incoming(pub Arc<Mutex<Vec<PathBuf>>>);
impl Global for Incoming {}

impl Incoming {
    pub fn take(&self) -> Vec<PathBuf> {
        self.0.lock().map(|mut v| std::mem::take(&mut *v)).unwrap_or_default()
    }
}

/// The window/workspace that receives opened files.
pub struct OpenTarget {
    pub window: AnyWindowHandle,
    pub workspace: WeakEntity<Workspace>,
}
impl Global for OpenTarget {}

/// `file://` URLs (from Apple Events) or plain paths, to paths.
pub fn paths_from_urls(urls: &[String]) -> Vec<PathBuf> {
    urls.iter()
        .filter_map(|u| {
            if u.starts_with("file://") {
                crate::lsp::uri_to_path(u)
            } else if u.contains("://") {
                None
            } else {
                Some(PathBuf::from(u))
            }
        })
        .map(|p| {
            let s = p.to_string_lossy();
            // Finder sends directories with a trailing slash.
            if s.len() > 1 && s.ends_with('/') {
                PathBuf::from(s.trim_end_matches('/'))
            } else {
                p
            }
        })
        .collect()
}

/// Hand any pending paths to the workspace, if it exists yet. If it
/// doesn't, the workspace picks them up when it starts.
pub fn deliver(cx: &mut App) {
    let Some(incoming) = cx.try_global::<Incoming>().cloned() else { return };
    let Some((window, workspace)) = cx
        .try_global::<OpenTarget>()
        .map(|t| (t.window, t.workspace.clone()))
    else {
        return;
    };
    let paths = incoming.take();
    if paths.is_empty() {
        return;
    }
    cx.activate(true);
    let _ = window.update(cx, |_, window, cx| {
        window.activate_window();
        if let Some(ws) = workspace.upgrade() {
            ws.update(cx, |ws, cx| ws.open_paths(paths, window, cx));
        }
    });
}

/// Decode file bytes as text, or `None` if they're binary.
///
/// Handles UTF-8 (with or without BOM), UTF-16 LE/BE (with BOM, or detected
/// from the byte pattern) and falls back to lossy UTF-8 for stray bytes.
pub fn decode_text(bytes: &[u8]) -> Option<String> {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return Some(String::from_utf8_lossy(rest).into_owned());
    }
    let utf16 = |data: &[u8], le: bool| -> String {
        let units: Vec<u16> = data
            .chunks_exact(2)
            .map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) })
            .collect();
        String::from_utf16_lossy(&units)
    };
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return Some(utf16(rest, true));
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return Some(utf16(rest, false));
    }
    let sample = &bytes[..bytes.len().min(8192)];
    if !sample.contains(&0) {
        return Some(String::from_utf8_lossy(bytes).into_owned());
    }
    // BOM-less UTF-16: NULs only in every other byte, and the rest printable.
    if sample.len() >= 4 {
        let even_nuls = sample.iter().step_by(2).filter(|b| **b == 0).count();
        let odd_nuls = sample.iter().skip(1).step_by(2).filter(|b| **b == 0).count();
        let half = sample.len() / 2;
        if odd_nuls * 10 >= half * 9 && even_nuls == 0 {
            return Some(utf16(bytes, true));
        }
        if even_nuls * 10 >= half * 9 && odd_nuls == 0 {
            return Some(utf16(bytes, false));
        }
    }
    None
}

/// One hex-dump row: offset, hex bytes, printable ASCII.
pub fn hex_row(bytes: &[u8], row: usize) -> (String, String, String) {
    let start = row * 16;
    let chunk = &bytes[start.min(bytes.len())..(start + 16).min(bytes.len())];
    let mut hex = String::with_capacity(48);
    for (i, b) in chunk.iter().enumerate() {
        if i == 8 {
            hex.push(' ');
        }
        hex.push_str(&format!("{b:02x} "));
    }
    let ascii: String = chunk
        .iter()
        .map(|b| if b.is_ascii_graphic() || *b == b' ' { *b as char } else { '·' })
        .collect();
    (format!("{start:08x}"), hex, ascii)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_common_encodings() {
        assert_eq!(decode_text(b"hello\n").as_deref(), Some("hello\n"));
        assert_eq!(decode_text(&[0xEF, 0xBB, 0xBF, b'h', b'i']).as_deref(), Some("hi"));
        let le: Vec<u8> = [0xFF, 0xFE].into_iter().chain("héllo".encode_utf16().flat_map(|u| u.to_le_bytes())).collect();
        assert_eq!(decode_text(&le).as_deref(), Some("héllo"));
        let be: Vec<u8> = [0xFE, 0xFF].into_iter().chain("abc".encode_utf16().flat_map(|u| u.to_be_bytes())).collect();
        assert_eq!(decode_text(&be).as_deref(), Some("abc"));
        let bomless: Vec<u8> = "plain text file".encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        assert_eq!(decode_text(&bomless).as_deref(), Some("plain text file"));
        // Invalid UTF-8 still opens (lossy), it isn't treated as binary.
        assert!(decode_text(&[b'a', 0xFF, b'b']).is_some());
        // Real binary (PNG header with NULs scattered) is binary.
        assert!(decode_text(&[0x89, b'P', b'N', b'G', 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0, 1, 0]).is_none());
    }

    #[test]
    fn converts_urls_to_paths() {
        let paths = paths_from_urls(&[
            "file:///Users/me/My%20Code/main.py".into(),
            "file:///Users/me/project/".into(),
            "/tmp/plain.txt".into(),
            "https://example.com".into(),
        ]);
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/Users/me/My Code/main.py"),
                PathBuf::from("/Users/me/project"),
                PathBuf::from("/tmp/plain.txt"),
            ]
        );
    }

    #[test]
    fn hex_rows() {
        let (off, hex, ascii) = hex_row(b"ABC\x00", 0);
        assert_eq!(off, "00000000");
        assert_eq!(hex, "41 42 43 00 ");
        assert_eq!(ascii, "ABC·");
    }
}
