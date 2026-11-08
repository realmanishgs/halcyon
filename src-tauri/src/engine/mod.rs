//! Media engine abstraction. The application layer talks only to `MediaEngine`;
//! concrete backends (libVLC today; FFmpeg/libiamf later) live in submodules.

pub mod vlc;
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
    pub rate: f32,
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

#[derive(Debug, Clone, Serialize)]
pub struct EngineInfo {
    pub name: String,
    pub version: String,
    pub location: String,
}

pub trait MediaEngine: Send {
    fn info(&self) -> EngineInfo;
    /// `surface` is a native window handle (HWND on Windows) for video output.
    fn open(&mut self, path: &str, surface: Option<isize>) -> Result<(), String>;
    fn play(&mut self) -> Result<(), String>;
    fn set_paused(&mut self, paused: bool);
    fn stop(&mut self);
    fn seek_ms(&mut self, ms: i64);
    fn set_volume(&mut self, volume: i32);
    fn set_rate(&mut self, rate: f32) -> Result<(), String>;
    fn next_frame(&mut self);
    fn status(&self) -> PlaybackStatus;
    fn tracks(&self) -> TrackSelection;
    fn set_audio_track(&mut self, id: i32) -> Result<(), String>;
    fn set_subtitle_track(&mut self, id: i32) -> Result<(), String>;
    fn meta(&self) -> EngineMeta;
}
