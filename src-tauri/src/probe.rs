//! Out-of-process media inspection via ffprobe.
//!
//! Untrusted files are parsed in a separate process so a demuxer crash cannot
//! take down the player. Results are the container/stream facts ffprobe
//! reports; nothing here is inferred or synthesised.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Stream {
    pub index: i64,
    pub codec_type: Option<String>,
    pub codec_name: Option<String>,
    pub codec_long_name: Option<String>,
    pub profile: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub pix_fmt: Option<String>,
    pub r_frame_rate: Option<String>,
    pub color_transfer: Option<String>,
    pub color_primaries: Option<String>,
    pub sample_rate: Option<String>,
    pub channels: Option<i64>,
    pub channel_layout: Option<String>,
    pub sample_fmt: Option<String>,
    pub bits_per_raw_sample: Option<String>,
    pub bit_rate: Option<String>,
    #[serde(default)]
    pub disposition: std::collections::BTreeMap<String, i64>,
    /// HDR mastering / content light / Dolby Vision configuration, as reported.
    #[serde(default)]
    pub side_data_list: Vec<serde_json::Value>,
    #[serde(default)]
    pub tags: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Format {
    pub format_name: Option<String>,
    pub format_long_name: Option<String>,
    pub duration: Option<String>,
    pub size: Option<String>,
    pub bit_rate: Option<String>,
    #[serde(default)]
    pub tags: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Chapter {
    pub start_time: Option<String>,
    #[serde(default)]
    pub tags: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ProbeResult {
    #[serde(default)]
    pub streams: Vec<Stream>,
    #[serde(default)]
    pub format: Format,
    #[serde(default)]
    pub chapters: Vec<Chapter>,
    /// Stream groups (e.g. IAMF audio elements / mix presentations), passed through as reported.
    #[serde(default)]
    pub stream_groups: Vec<serde_json::Value>,
}

pub fn ffprobe_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("HALCYON_FFPROBE") {
        return Some(PathBuf::from(p));
    }
    let exe = if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" };
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).map(|d| d.join(exe)).find(|p| p.is_file())
    })
}

pub fn probe(path: &str) -> Result<ProbeResult, String> {
    let bin = ffprobe_path().ok_or("ffprobe not found (install FFmpeg or set HALCYON_FFPROBE)")?;
    let mut cmd = Command::new(bin);
    cmd.args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams", "-show_chapters", "-show_stream_groups", "-i"])
        .arg(crate::engine::plain_path(path));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().map_err(|e| format!("ffprobe failed to start: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "Unrecognised or damaged media: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("Unexpected ffprobe output: {e}"))
}
