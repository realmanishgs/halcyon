//! Turning dropped/opened items into a play queue: folders are scanned for
//! media, playlist files (M3U/M3U8/PLS/XSPF) are expanded, URLs pass through.

use crate::engine::{is_url, plain_path};
use std::path::{Path, PathBuf};

pub const MEDIA_EXT: &[&str] = &[
    "mp4", "m4v", "mkv", "mk3d", "avi", "mov", "qt", "webm", "flv", "f4v", "wmv", "asf", "ts", "m2ts", "mts",
    "m2t", "mpg", "mpeg", "mpe", "m1v", "m2v", "vob", "3gp", "3g2", "ogg", "ogm", "ogv", "mxf", "dv", "rm",
    "rmvb", "divx", "y4m", "mp3", "mp2", "mpa", "flac", "wav", "w64", "aif", "aiff", "aifc", "aac", "adts",
    "m4a", "m4b", "alac", "opus", "oga", "spx", "wma", "ape", "wv", "tta", "ac3", "eac3", "ec3", "dts",
    "dtshd", "thd", "mlp", "mka", "mid", "midi", "amr", "au", "snd", "caf", "dsf", "dff", "iamf",
];
pub const SUBTITLE_EXT: &[&str] = &["srt", "ass", "ssa", "vtt", "sub", "idx", "smi", "sami", "ttml", "usf"];
pub const PLAYLIST_EXT: &[&str] = &["m3u", "m3u8", "pls", "xspf"];

const MAX_ITEMS: usize = 10_000;
const MAX_DEPTH: usize = 8;

fn ext(p: &Path) -> String {
    p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

pub fn is_subtitle(path: &str) -> bool {
    SUBTITLE_EXT.contains(&ext(Path::new(path)).as_str())
}

/// Expands files, folders and playlists into playable sources, in order.
pub fn expand(items: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for it in items {
        if out.len() >= MAX_ITEMS {
            break;
        }
        if is_url(it) {
            out.push(it.clone());
            continue;
        }
        let p = PathBuf::from(plain_path(it));
        if p.is_dir() {
            scan_dir(&p, 0, &mut out);
        } else if PLAYLIST_EXT.contains(&ext(&p).as_str()) {
            out.extend(read_playlist(&p));
        } else if p.is_file() {
            out.push(p.to_string_lossy().into_owned());
        }
    }
    out.truncate(MAX_ITEMS);
    out
}

fn scan_dir(dir: &Path, depth: usize, out: &mut Vec<String>) {
    if depth > MAX_DEPTH || out.len() >= MAX_ITEMS {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    for p in entries {
        if p.is_dir() {
            scan_dir(&p, depth + 1, out);
        } else if MEDIA_EXT.contains(&ext(&p).as_str()) {
            out.push(p.to_string_lossy().into_owned());
        }
    }
}

pub fn read_playlist(path: &Path) -> Vec<String> {
    let Ok(bytes) = std::fs::read(path) else { return vec![] };
    let text = String::from_utf8_lossy(&bytes).trim_start_matches('\u{feff}').to_string();
    let base = path.parent().unwrap_or(Path::new("."));
    let raw = match ext(path).as_str() {
        "pls" => parse_pls(&text),
        "xspf" => parse_xspf(&text),
        _ => parse_m3u(&text),
    };
    raw.into_iter().filter_map(|e| resolve(&e, base)).collect()
}

pub fn parse_m3u(text: &str) -> Vec<String> {
    text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(String::from).collect()
}

pub fn parse_pls(text: &str) -> Vec<String> {
    let mut v: Vec<(u32, String)> = text
        .lines()
        .filter_map(|l| {
            let (k, val) = l.trim().split_once('=')?;
            let n = k.trim().strip_prefix("File").or_else(|| k.trim().strip_prefix("file"))?.parse().ok()?;
            Some((n, val.trim().to_string()))
        })
        .collect();
    v.sort_by_key(|(n, _)| *n);
    v.into_iter().map(|(_, s)| s).collect()
}

pub fn parse_xspf(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("<location>") {
        rest = &rest[i + 10..];
        let Some(j) = rest.find("</location>") else { break };
        out.push(xml_unescape(rest[..j].trim()));
        rest = &rest[j..];
    }
    out
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        // Work on bytes: slicing the &str here could split a multi-byte character.
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = |c: u8| (c as char).to_digit(16);
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Playlist entry → absolute local path or URL. Relative entries resolve
/// against the playlist's folder; `file://` URIs become paths.
fn resolve(entry: &str, base: &Path) -> Option<String> {
    if let Some(rest) = entry.strip_prefix("file://") {
        let rest = percent_decode(rest);
        let local = match rest.strip_prefix('/') {
            Some(r) if r.len() > 1 && r.as_bytes()[1] == b':' => r.to_string(), // file:///C:/x
            _ if rest.starts_with('/') => rest,
            _ => format!("//{rest}"), // file://server/share → UNC
        };
        return Some(local.replace('/', std::path::MAIN_SEPARATOR_STR));
    }
    if is_url(entry) {
        return Some(entry.to_string());
    }
    let p = Path::new(entry);
    let abs = if p.is_absolute() { p.to_path_buf() } else { base.join(p) };
    // Normalise `..` so titles and recents show real paths (libVLC derives its
    // fallback title from the path it was given).
    let abs = std::fs::canonicalize(&abs).map(|c| PathBuf::from(plain_path(&c.to_string_lossy()))).unwrap_or(abs);
    Some(abs.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m3u_skips_comments_and_blank_lines() {
        let t = "#EXTM3U\n#EXTINF:12,Song\nsong.mp3\n\n  https://example.com/live.m3u8 \n";
        assert_eq!(parse_m3u(t), vec!["song.mp3", "https://example.com/live.m3u8"]);
    }

    #[test]
    fn percent_decoding_survives_non_ascii_after_percent() {
        // Found by fuzz::playlist_files_on_disk_never_panic: used to panic on a char boundary.
        assert_eq!(percent_decode("a%\u{1538}b%41"), "a%\u{1538}bA");
        assert_eq!(percent_decode("%"), "%");
        assert_eq!(percent_decode("x%4"), "x%4");
    }

    #[test]
    fn pls_orders_by_index() {
        let t = "[playlist]\nFile2=b.flac\nTitle2=B\nFile1=a.flac\nNumberOfEntries=2\n";
        assert_eq!(parse_pls(t), vec!["a.flac", "b.flac"]);
    }

    #[test]
    fn xspf_reads_locations() {
        let t = r#"<playlist><trackList><track><location>file:///C:/M%C3%BCsic/a%20b.mp3</location></track>
                   <track><location>http://x/y?a=1&amp;b=2</location></track></trackList></playlist>"#;
        assert_eq!(parse_xspf(t), vec!["file:///C:/M%C3%BCsic/a%20b.mp3", "http://x/y?a=1&b=2"]);
    }

    #[cfg(windows)]
    #[test]
    fn resolves_relative_file_uri_and_urls() {
        let base = Path::new(r"C:\lists");
        assert_eq!(resolve("a.mp3", base).unwrap(), r"C:\lists\a.mp3");
        assert_eq!(resolve("file:///C:/M%C3%BCsic/a%20b.mp3", base).unwrap(), r"C:\Müsic\a b.mp3");
        assert_eq!(resolve("https://h/s.m3u8", base).unwrap(), "https://h/s.m3u8");
    }
}
