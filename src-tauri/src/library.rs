//! Library indexing (folders → SQLite rows from ffprobe facts), cover art,
//! lyrics (sidecar .lrc or embedded tags) and playlist export.

use crate::db::{Db, MediaRow};
use crate::probe::{self, ProbeResult};
use crate::sources::{MEDIA_EXT, PLAYLIST_EXT};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn ext(p: &Path) -> String {
    p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

/// All media files under `root` (playlists excluded).
pub fn walk(root: &Path, out: &mut Vec<PathBuf>) {
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                if depth < 16 && !e.file_name().to_string_lossy().starts_with('.') {
                    stack.push((p, depth + 1));
                }
            } else {
                let x = ext(&p);
                if MEDIA_EXT.contains(&x.as_str()) && !PLAYLIST_EXT.contains(&x.as_str()) {
                    out.push(p);
                }
            }
        }
    }
}

fn tag(tags: &BTreeMap<String, String>, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some((_, v)) = tags.iter().find(|(tk, v)| tk.eq_ignore_ascii_case(k) && !v.trim().is_empty()) {
            return Some(v.trim().to_string());
        }
    }
    None
}

fn leading_int(s: &str) -> Option<i64> {
    let d: String = s.trim().chars().take_while(|c| c.is_ascii_digit()).collect();
    d.parse().ok()
}

/// Library row from ffprobe output. Tags may live on the container or (Ogg,
/// Opus) on the audio stream; both are consulted.
pub fn row_from_probe(path: &str, p: &ProbeResult, size: i64, mtime: i64) -> MediaRow {
    let mut tags = p.format.tags.clone();
    for s in &p.streams {
        if s.codec_type.as_deref() == Some("audio") {
            for (k, v) in &s.tags {
                tags.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
    }
    let video = p.streams.iter().find(|s| s.codec_type.as_deref() == Some("video") && s.disposition.get("attached_pic").copied().unwrap_or(0) == 0);
    let audio = p.streams.iter().find(|s| s.codec_type.as_deref() == Some("audio"));
    let main = video.or(audio);
    MediaRow {
        path: path.to_string(),
        kind: if video.is_some() { "video" } else if audio.is_some() { "audio" } else { "unknown" }.into(),
        title: tag(&tags, &["title"]),
        artist: tag(&tags, &["artist", "album_artist", "performer"]),
        album: tag(&tags, &["album"]),
        album_artist: tag(&tags, &["album_artist", "albumartist", "album artist"]),
        genre: tag(&tags, &["genre"]),
        year: tag(&tags, &["date", "year", "originaldate"]).and_then(|d| leading_int(&d)).filter(|y| (1000..3000).contains(y)),
        track: tag(&tags, &["track", "tracknumber"]).and_then(|t| leading_int(&t)),
        disc: tag(&tags, &["disc", "discnumber"]).and_then(|t| leading_int(&t)),
        duration_ms: p.format.duration.as_deref().and_then(|d| d.parse::<f64>().ok()).map(|d| (d * 1000.0) as i64),
        codec: main.and_then(|s| s.codec_name.clone()),
        width: video.and_then(|v| v.width),
        height: video.and_then(|v| v.height),
        size: Some(size),
        mtime: Some(mtime),
        lyrics: tag(&tags, &["lyrics", "unsyncedlyrics", "lyrics-eng", "uslt"]),
        ..Default::default()
    }
}

fn stat(p: &Path) -> Option<(i64, i64)> {
    let m = std::fs::metadata(p).ok()?;
    let mtime = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;
    Some((m.len() as i64, mtime))
}

/// Indexes one file (if new or changed). Returns true if the row changed.
pub fn index_file(db: &Db, path: &Path) -> Result<bool, String> {
    let (size, mtime) = stat(path).ok_or("File not readable")?;
    let key = path.to_string_lossy().into_owned();
    if let Some(m) = db.get_media(&key) {
        if m.size == Some(size) && m.mtime == Some(mtime) && m.kind != "unknown" {
            return Ok(false);
        }
    }
    let p = probe::probe(&key)?;
    db.upsert_media(&row_from_probe(&key, &p, size, mtime))?;
    Ok(true)
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ScanProgress {
    pub done: usize,
    pub total: usize,
    pub added: usize,
    pub removed: usize,
    pub failed: usize,
    pub current: String,
    pub finished: bool,
}

/// Rescans every library root: new/changed files are probed (4 workers),
/// rows for vanished files are dropped.
pub fn scan(db: &Db, progress: impl Fn(&ScanProgress) + Sync) -> Result<ScanProgress, String> {
    let roots = db.roots()?;
    let mut files = Vec::new();
    for r in &roots {
        walk(Path::new(r), &mut files);
    }
    files.sort();
    let known = db.known()?;
    let present: std::collections::HashSet<String> = files.iter().map(|p| p.to_string_lossy().into_owned()).collect();
    let mut st = ScanProgress { total: files.len(), ..Default::default() };
    for k in known.keys() {
        let under_root = roots.iter().any(|r| k.starts_with(r.as_str()));
        if under_root && !present.contains(k) {
            db.remove_media(k)?;
            st.removed += 1;
        }
    }
    let todo: Vec<&PathBuf> = files
        .iter()
        .filter(|p| match (known.get(&*p.to_string_lossy()), stat(p)) {
            (Some(k), Some(s)) => *k != s,
            _ => true,
        })
        .collect();
    st.done = st.total - todo.len();
    progress(&st);
    let state = std::sync::Mutex::new(st);
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|sc| {
        for _ in 0..4 {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(p) = todo.get(i) else { break };
                let r = index_file(db, p);
                let mut s = state.lock().unwrap();
                s.done += 1;
                match r {
                    Ok(true) => s.added += 1,
                    Ok(false) => {}
                    Err(_) => s.failed += 1,
                }
                s.current = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                if s.done % 8 == 0 || s.done == s.total {
                    progress(&s);
                }
            });
        }
    });
    let mut st = state.into_inner().unwrap();
    st.finished = true;
    st.current.clear();
    progress(&st);
    Ok(st)
}

fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

/// Cover image for a media file: a sidecar image in its folder, else the
/// embedded picture extracted (once) into `cache`. None if there is neither.
pub fn cover(path: &str, cache: &Path) -> Option<PathBuf> {
    let p = Path::new(path);
    let dir = p.parent()?;
    let stem = p.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    if let Ok(rd) = std::fs::read_dir(dir) {
        let mut imgs: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|f| matches!(ext(f).as_str(), "jpg" | "jpeg" | "png" | "webp"))
            .collect();
        imgs.sort();
        for want in [stem.as_str(), "cover", "folder", "front", "albumart"] {
            if let Some(f) = imgs.iter().find(|f| f.file_stem().map(|s| s.to_string_lossy().to_lowercase().starts_with(want)).unwrap_or(false)) {
                return Some(f.clone());
            }
        }
    }
    let (_, mtime) = stat(p)?;
    let out = cache.join(format!("{:016x}.jpg", fnv(&format!("{path}|{mtime}"))));
    if out.is_file() {
        return Some(out);
    }
    let none = cache.join(format!("{:016x}.none", fnv(&format!("{path}|{mtime}"))));
    if none.is_file() {
        return None;
    }
    std::fs::create_dir_all(cache).ok()?;
    let ff = probe::ffmpeg_path()?;
    let ok = probe::quiet(ff)
        .args(["-v", "error", "-y", "-nostdin", "-i"])
        .arg(crate::engine::plain_path(path))
        .args(["-map", "0:v:0", "-frames:v", "1", "-vf", "scale=512:-2", "-q:v", "3"])
        .arg(&out)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if ok && out.is_file() {
        Some(out)
    } else {
        let _ = std::fs::write(none, b"");
        None
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Lyrics {
    /// (time ms, line) sorted by time; empty for unsynchronised lyrics.
    pub synced: Vec<(i64, String)>,
    pub plain: Option<String>,
    pub source: String,
}

/// Parses LRC: `[mm:ss.xx]text`, several stamps per line, `[offset:+/-ms]`.
pub fn parse_lrc(text: &str) -> Vec<(i64, String)> {
    let mut offset = 0i64;
    let mut out = Vec::new();
    for line in text.lines() {
        let mut rest = line.trim();
        let mut stamps = Vec::new();
        while let Some(r) = rest.strip_prefix('[') {
            let Some(end) = r.find(']') else { break };
            let tag = &r[..end];
            rest = &r[end + 1..];
            if let Some(v) = tag.strip_prefix("offset:") {
                offset = v.trim().parse().unwrap_or(0);
                continue;
            }
            let Some((m, s)) = tag.split_once(':') else { continue };
            let (Ok(m), Ok(s)) = (m.trim().parse::<i64>(), s.trim().replace(':', ".").parse::<f64>()) else { continue };
            stamps.push(m * 60_000 + (s * 1000.0).round() as i64);
        }
        for t in stamps {
            out.push((t, rest.trim().to_string()));
        }
    }
    // A positive offset shows lyrics earlier.
    for (t, _) in out.iter_mut() {
        *t = (*t - offset).max(0);
    }
    out.sort_by_key(|(t, _)| *t);
    out
}

pub fn lyrics(path: &str, embedded: Option<String>) -> Option<Lyrics> {
    let p = Path::new(path);
    for x in ["lrc", "LRC", "txt"] {
        let side = p.with_extension(x);
        if let Ok(b) = std::fs::read(&side) {
            let text = String::from_utf8_lossy(&b).trim_start_matches('\u{feff}').to_string();
            let synced = parse_lrc(&text);
            let name = side.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            return Some(if synced.is_empty() {
                Lyrics { synced, plain: Some(text), source: name }
            } else {
                Lyrics { synced, plain: None, source: name }
            });
        }
    }
    let text = embedded?;
    let synced = parse_lrc(&text);
    Some(if synced.is_empty() {
        Lyrics { synced, plain: Some(text), source: "embedded".into() }
    } else {
        Lyrics { synced, plain: None, source: "embedded".into() }
    })
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Writes `items` as an M3U8 (UTF-8, absolute paths/URLs) or XSPF playlist.
pub fn export_playlist(target: &Path, name: &str, items: &[String]) -> Result<(), String> {
    let body = match ext(target).as_str() {
        "xspf" => {
            let tracks: String = items
                .iter()
                .map(|i| {
                    let loc = if crate::engine::is_url(i) { i.clone() } else { crate::engine::vlc::file_uri(i) };
                    format!("    <track><location>{}</location></track>\n", xml_escape(&loc))
                })
                .collect();
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<playlist version=\"1\" xmlns=\"http://xspf.org/ns/0/\">\n  <title>{}</title>\n  <trackList>\n{tracks}  </trackList>\n</playlist>\n",
                xml_escape(name)
            )
        }
        _ => {
            let mut s = String::from("#EXTM3U\n");
            s.push_str(&format!("#PLAYLIST:{name}\n"));
            for i in items {
                s.push_str(i);
                s.push('\n');
            }
            s
        }
    };
    std::fs::write(target, body).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lrc_parsing_handles_multi_stamps_and_offset() {
        let t = "[ti:Song]\n[offset:500]\n[00:01.00][00:10.50]Chorus\n[00:05.25] Verse\nno stamp\n";
        assert_eq!(parse_lrc(t), vec![(500, "Chorus".to_string()), (4750, "Verse".to_string()), (10000, "Chorus".to_string())]);
    }

    #[test]
    fn tags_from_container_and_stream() {
        let j = r#"{"streams":[{"index":0,"codec_type":"audio","codec_name":"opus","tags":{"ARTIST":"Ana","TRACKNUMBER":"3/12","DATE":"2019-04-01"}}],
                    "format":{"duration":"61.5","tags":{"title":"Song","album":"Blue"}}}"#;
        let p: ProbeResult = serde_json::from_str(j).unwrap();
        let r = row_from_probe("x.opus", &p, 10, 20);
        assert_eq!(r.kind, "audio");
        assert_eq!((r.title.as_deref(), r.artist.as_deref(), r.album.as_deref()), (Some("Song"), Some("Ana"), Some("Blue")));
        assert_eq!((r.track, r.year, r.duration_ms), (Some(3), Some(2019), Some(61500)));
    }

    #[test]
    fn cover_art_attached_pic_is_audio_not_video() {
        let j = r#"{"streams":[{"index":0,"codec_type":"audio","codec_name":"mp3"},
                    {"index":1,"codec_type":"video","codec_name":"mjpeg","disposition":{"attached_pic":1}}],"format":{}}"#;
        let p: ProbeResult = serde_json::from_str(j).unwrap();
        assert_eq!(row_from_probe("x.mp3", &p, 0, 0).kind, "audio");
    }

    #[test]
    fn exported_playlists_read_back() {
        let dir = std::env::temp_dir().join(format!("halcyon-pl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a b é.mp3");
        std::fs::write(&a, b"x").unwrap();
        let items = vec![a.to_string_lossy().into_owned(), "https://h/s.m3u8".to_string()];
        for name in ["t.m3u8", "t.xspf"] {
            let f = dir.join(name);
            export_playlist(&f, "Mix & more", &items).unwrap();
            let back = crate::sources::read_playlist(&f);
            assert_eq!(back.len(), 2, "{name}");
            assert_eq!(std::fs::canonicalize(&back[0]).unwrap(), std::fs::canonicalize(&a).unwrap(), "{name}");
            assert_eq!(back[1], "https://h/s.m3u8");
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
