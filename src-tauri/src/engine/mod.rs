//! Media engine abstraction. The application layer talks only to `MediaEngine`;
//! concrete backends (libVLC today; FFmpeg/libiamf later) live in submodules.

pub mod vlc;
#[cfg(test)]
mod stress;
#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

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

/// Settings that libVLC 3 only reads when its instance is created (video and
/// audio filters, decoder and text-renderer options). Changing them recreates
/// the instance; playback is restored at the same position.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct EngineConfig {
    pub hw_decode: bool,
    pub normalize: bool,
    pub replay_gain: bool,
    /// None, "spectrum", "scope", "vuMeter", "spectrometer", "goom".
    pub visualizer: Option<String>,
    /// 0, 90, 180, 270, "hflip", "vflip" as transform types; "" = none.
    pub transform: String,
    /// Subtitle text scale in percent (10–500).
    pub sub_scale: u32,
    /// 0xRRGGBB.
    pub sub_color: u32,
    /// 0 none, 2 thin, 4 normal, 6 thick.
    pub sub_outline: u32,
    /// 0–255.
    pub sub_background: u32,
    /// Pixels from the bottom.
    pub sub_margin: i32,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            hw_decode: true,
            normalize: false,
            replay_gain: false,
            visualizer: None,
            transform: String::new(),
            sub_scale: 100,
            sub_color: 0xFFFFFF,
            sub_outline: 4,
            sub_background: 0,
            sub_margin: 0,
        }
    }
}

impl EngineConfig {
    /// libVLC command-line options for this configuration.
    pub fn args(&self) -> Vec<String> {
        let mut a = Vec::new();
        if self.normalize {
            a.push("--audio-filter=normvol".into());
        }
        if self.replay_gain {
            a.push("--audio-replay-gain-mode=track".into());
        }
        match self.visualizer.as_deref() {
            Some("goom") => a.push("--audio-visual=goom".into()),
            Some(e @ ("spectrum" | "scope" | "vuMeter" | "spectrometer")) => {
                a.push("--audio-visual=visual".into());
                a.push(format!("--effect-list={e}"));
            }
            _ => {}
        }
        if ["90", "180", "270", "hflip", "vflip", "transpose", "antitranspose"].contains(&self.transform.as_str()) {
            // adjust is in the chain from the start: attaching it to a running OpenGL
            // output afterwards failed intermittently (picture_adjust_survives_rotation).
            a.push("--video-filter=adjust:transform".into());
            a.push(format!("--transform-type={}", self.transform));
            // Measured on Windows (libVLC 3.0.24): with the transform filter the Direct3D11
            // output stalls after the first frame (Direct3D9 and GDI on some transforms);
            // the OpenGL output displays every transform. Use it while a transform is active.
            #[cfg(windows)]
            a.push("--vout=glwin32".into());
        }
        a.push(format!("--sub-text-scale={}", self.sub_scale.clamp(10, 500)));
        a.push(format!("--freetype-color={}", self.sub_color & 0xFFFFFF));
        a.push(format!("--freetype-outline-thickness={}", self.sub_outline.min(6)));
        a.push(format!("--freetype-background-opacity={}", self.sub_background.min(255)));
        a.push(format!("--sub-margin={}", self.sub_margin.clamp(0, 1000)));
        a
    }

    /// Per-media options. Hardware decoding must be set on the media: the
    /// libVLC 3 player object shadows the instance-level `avcodec-hw` option
    /// (measured: the instance flag is ignored, the media option is honoured).
    pub fn media_options(&self) -> Vec<String> {
        // The transform filter cannot take GPU (D3D11) frames, so rotation and
        // flips decode in software.
        let hw = self.hw_decode && self.transform.is_empty();
        vec![format!(":avcodec-hw={}", if hw { "any" } else { "none" })]
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EngineEvent {
    Opening,
    Playing,
    Paused,
    Stopped,
    Ended,
    Error,
    Length,
    Tracks,
    Vout,
    Media,
}

pub type EventSink = std::sync::Arc<dyn Fn(EngineEvent) + Send + Sync>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Equalizer {
    pub preamp: f32,
    pub bands: Vec<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EqualizerInfo {
    pub frequencies: Vec<f32>,
    pub presets: Vec<(String, Equalizer)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VideoAdjust {
    pub brightness: f32,
    pub contrast: f32,
    pub hue: f32,
    pub saturation: f32,
    pub gamma: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BrowseItem {
    pub name: String,
    pub url: String,
    /// "directory", "file", "stream", "playlist", "disc", "unknown".
    pub kind: String,
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
    fn set_event_sink(&mut self, sink: EventSink);
    /// Recreates the engine instance if `cfg` changes its options; returns true if it did.
    fn configure(&mut self, cfg: &EngineConfig) -> Result<bool, String>;
    fn equalizer_info(&self) -> EqualizerInfo;
    /// `None` disables the equalizer.
    fn set_equalizer(&mut self, eq: Option<&Equalizer>) -> Result<(), String>;
    /// `None` disables picture adjustment.
    fn set_video_adjust(&mut self, adj: Option<&VideoAdjust>);
    /// `None` = no crop; otherwise "16:9" (ratio) or "WxH+X+Y".
    fn set_crop(&mut self, geometry: Option<&str>);
    fn audio_devices(&self) -> Vec<AudioDevice>;
    /// Empty id = system default.
    fn set_audio_device(&mut self, id: &str);
    /// Lists a directory-like location (local folder, SMB/FTP/SFTP/NFS share, UPnP server).
    fn browse(&mut self, url: &str, timeout_ms: u32) -> Result<Vec<BrowseItem>, String>;
    /// LAN discovery services (UPnP, mDNS/SMB, SAP) available in this engine.
    fn discovery_services(&self) -> Vec<(String, String)>;
    /// Runs a discovery service for `wait_ms` and returns what it found.
    fn discover(&mut self, service: &str, wait_ms: u32) -> Result<Vec<BrowseItem>, String>;
}
