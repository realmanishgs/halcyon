//! Media engine abstraction. The application layer talks only to `MediaEngine`;
//! concrete backends (libVLC today; FFmpeg/libiamf later) live in submodules.

pub mod vlc;
#[cfg(test)]
mod stress;
#[cfg(test)]
mod tests;

use serde::Serialize;

/// libVLC 3 and ffprobe cannot open Win32 verbatim paths (`\\?\C:\...`,
/// `\\?\UNC\server\share`), which Rust's `canonicalize` produces. Convert them.
pub fn plain_path(path: &str) -> String {
    match path.strip_prefix(r"\\?\") {
        Some(rest) if rest.starts_with(r"UNC\") => format!(r"\\{}", &rest[4..]),
        Some(rest) => rest.to_string(),
        None => path.to_string(),
    }
}

/// Path libVLC 3 can open. libVLC 3 uses MAX_PATH-limited Win32 calls and
/// rejects `\\?\` paths, so paths of 260+ characters are handed over as their
/// 8.3 short form (all components exist, so the short name is resolvable).
pub fn engine_path(path: &str) -> String {
    let p = plain_path(path);
    #[cfg(windows)]
    if p.encode_utf16().count() >= 260 {
        use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;
        // canonicalize() resolves `..` and yields the `\\?\` form that bypasses MAX_PATH.
        let Ok(abs) = std::fs::canonicalize(&p) else { return p };
        let long: Vec<u16> = abs.to_string_lossy().encode_utf16().chain(Some(0)).collect();
        let mut buf = vec![0u16; 32768];
        let n = unsafe { GetShortPathNameW(long.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) } as usize;
        if n > 0 && n < buf.len() {
            let short = plain_path(&String::from_utf16_lossy(&buf[..n]));
            if short.encode_utf16().count() < 260 {
                return short;
            }
        }
    }
    p
}

/// `scheme://...` sources (http, https, rtsp, smb, ...) as opposed to local paths.
/// A drive letter (`C:\`) is never a scheme.
pub fn is_url(source: &str) -> bool {
    match source.find("://") {
        Some(i) => i > 1 && source[..i].chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.'),
        None => false,
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackState {
    Idle,
    Opening,
    Buffering,
    Playing,
    Paused,
    Stopped,
    Ended,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaybackStatus {
    pub state: PlaybackState,
    pub time_ms: i64,
    pub length_ms: i64,
    pub volume: i32,
    pub muted: bool,
    pub rate: f32,
    pub seekable: bool,
    pub has_video: bool,
    pub chapter: i32,
    pub chapter_count: i32,
    pub audio_delay_ms: i64,
    pub subtitle_delay_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackEntry {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackSelection {
    pub audio: Vec<TrackEntry>,
    pub subtitle: Vec<TrackEntry>,
    pub current_audio: i32,
    pub current_subtitle: i32,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct EngineMeta {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub date: Option<String>,
    pub artwork_url: Option<String>,
}

/// Decoder / output counters reported by the engine for the current media.
#[derive(Debug, Clone, Copy, Serialize, Default)]
pub struct MediaStats {
    pub read_bytes: i64,
    pub input_bitrate_kbps: f32,
    pub demux_corrupted: i64,
    pub demux_discontinuity: i64,
    pub decoded_video: i64,
    pub decoded_audio: i64,
    pub displayed_pictures: i64,
    pub lost_pictures: i64,
    pub played_abuffers: i64,
    pub lost_abuffers: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EngineInfo {
    pub name: String,
    pub version: String,
    pub location: String,
}

pub trait MediaEngine: Send {
    fn info(&self) -> EngineInfo;
    /// `source` is a local path or a URL. `surface` is a native window handle
    /// (HWND on Windows) for video output.
    fn open(&mut self, source: &str, surface: Option<isize>) -> Result<(), String>;
    /// Starts playback; restarts the current media if it ended, stopped or failed.
    fn play(&mut self) -> Result<(), String>;
    fn set_paused(&mut self, paused: bool);
    fn stop(&mut self);
    fn seek_ms(&mut self, ms: i64);
    fn set_volume(&mut self, volume: i32);
    fn set_muted(&mut self, muted: bool);
    fn set_rate(&mut self, rate: f32) -> Result<(), String>;
    fn next_frame(&mut self);
    fn status(&self) -> PlaybackStatus;
    fn tracks(&self) -> TrackSelection;
    fn set_audio_track(&mut self, id: i32) -> Result<(), String>;
    fn set_subtitle_track(&mut self, id: i32) -> Result<(), String>;
    fn add_subtitle_file(&mut self, path: &str) -> Result<(), String>;
    fn set_audio_delay_ms(&mut self, ms: i64) -> Result<(), String>;
    fn set_subtitle_delay_ms(&mut self, ms: i64) -> Result<(), String>;
    /// `None` = source aspect; otherwise "16:9", "4:3", "W:H" (used for stretch-to-fill).
    fn set_aspect_ratio(&mut self, aspect: Option<&str>);
    /// `None` = off; otherwise a libVLC deinterlace mode ("blend", "yadif", ...).
    fn set_deinterlace(&mut self, mode: Option<&str>);
    fn set_chapter(&mut self, chapter: i32);
    fn snapshot(&mut self, path: &str) -> Result<(), String>;
    fn video_size(&self) -> Option<(u32, u32)>;
    fn stats(&self) -> Option<MediaStats>;
    fn meta(&self) -> EngineMeta;
}
