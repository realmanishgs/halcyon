//! Conversion and remuxing through the user's FFmpeg, out of process.

use crate::probe;
use serde::{Deserialize, Serialize};
use std::io::BufRead;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Serialize)]
pub struct Preset {
    pub id: &'static str,
    pub label: &'static str,
    pub ext: &'static str,
    pub audio_only: bool,
    #[serde(skip)]
    args: &'static [&'static str],
}

pub const PRESETS: &[Preset] = &[
    Preset { id: "mp4-h264", label: "MP4 · H.264 + AAC", ext: "mp4", audio_only: false, args: &["-c:v", "libx264", "-crf", "20", "-preset", "medium", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k", "-movflags", "+faststart"] },
    Preset { id: "mp4-hevc", label: "MP4 · HEVC + AAC", ext: "mp4", audio_only: false, args: &["-c:v", "libx265", "-crf", "24", "-preset", "medium", "-tag:v", "hvc1", "-c:a", "aac", "-b:a", "192k", "-movflags", "+faststart"] },
    Preset { id: "webm-vp9", label: "WebM · VP9 + Opus", ext: "webm", audio_only: false, args: &["-c:v", "libvpx-vp9", "-crf", "32", "-b:v", "0", "-row-mt", "1", "-c:a", "libopus", "-b:a", "160k"] },
    Preset { id: "mkv-remux", label: "MKV · remux (no re-encode)", ext: "mkv", audio_only: false, args: &["-map", "0", "-c", "copy"] },
    Preset { id: "mp3", label: "MP3 · V2", ext: "mp3", audio_only: true, args: &["-vn", "-c:a", "libmp3lame", "-q:a", "2"] },
    Preset { id: "aac", label: "M4A · AAC 256k", ext: "m4a", audio_only: true, args: &["-vn", "-c:a", "aac", "-b:a", "256k"] },
    Preset { id: "opus", label: "Opus · 160k", ext: "opus", audio_only: true, args: &["-vn", "-c:a", "libopus", "-b:a", "160k"] },
    Preset { id: "flac", label: "FLAC · lossless", ext: "flac", audio_only: true, args: &["-vn", "-c:a", "flac"] },
    Preset { id: "wav", label: "WAV · 24-bit PCM", ext: "wav", audio_only: true, args: &["-vn", "-c:a", "pcm_s24le"] },
];

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Options {
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    /// Scale video to this height (keeps aspect). Ignored for remux/audio.
    pub height: Option<u32>,
    /// EBU R128 loudness normalisation (−16 LUFS, −1 dBTP) for audio presets.
    #[serde(default)]
    pub normalize: bool,
}

fn secs(ms: i64) -> String {
    format!("{:.3}", ms as f64 / 1000.0)
}

/// FFmpeg arguments for a job (exposed for tests).
pub fn build_args(input: &str, output: &str, preset: &Preset, o: &Options) -> Vec<String> {
    let mut a: Vec<String> = ["-v", "error", "-nostdin", "-y", "-progress", "pipe:1", "-nostats"].iter().map(|s| s.to_string()).collect();
    if let Some(s) = o.start_ms.filter(|s| *s > 0) {
        a.extend(["-ss".into(), secs(s)]);
    }
    a.extend(["-i".into(), crate::engine::plain_path(input)]);
    if let Some(e) = o.end_ms {
        a.extend(["-t".into(), secs(e - o.start_ms.unwrap_or(0)).to_string()]);
    }
    a.extend(preset.args.iter().map(|s| s.to_string()));
    let remux = preset.id == "mkv-remux";
    if let (Some(h), false, false) = (o.height, preset.audio_only, remux) {
        a.extend(["-vf".into(), format!("scale=-2:{h}")]);
    }
    if o.normalize && !remux {
        a.extend(["-af".into(), "loudnorm=I=-16:TP=-1:LRA=11".into()]);
    }
    a.push(output.into());
    a
}

/// Runs a conversion; `progress` gets 0..1. Deletes the partial output on failure or cancel.
pub fn run(input: &str, output: &str, preset_id: &str, o: &Options, cancel: &AtomicBool, progress: impl Fn(f32)) -> Result<(), String> {
    let preset = PRESETS.iter().find(|p| p.id == preset_id).ok_or("Unknown preset")?;
    if Path::new(output) == Path::new(&crate::engine::plain_path(input)) {
        return Err("Output would overwrite the input".into());
    }
    let total_ms = match (o.start_ms, o.end_ms) {
        (_, Some(e)) => e - o.start_ms.unwrap_or(0),
        _ => {
            let d = probe::probe(input).ok().and_then(|p| p.format.duration).and_then(|d| d.parse::<f64>().ok()).unwrap_or(0.0);
            (d * 1000.0) as i64 - o.start_ms.unwrap_or(0)
        }
    };
    let ff = probe::ffmpeg_path().ok_or("ffmpeg not found (install FFmpeg or set HALCYON_FFMPEG)")?;
    let mut child = probe::quiet(ff)
        .args(build_args(input, output, preset, o))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("ffmpeg failed to start: {e}"))?;
    let stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let err_thread = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = std::io::Read::read_to_string(&mut stderr, &mut s);
        s
    });
    for line in std::io::BufReader::new(stdout).lines() {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            break;
        }
        let Ok(line) = line else { break };
        if let Some(us) = line.strip_prefix("out_time_us=").and_then(|v| v.trim().parse::<i64>().ok()) {
            if total_ms > 0 {
                progress((us as f64 / 1000.0 / total_ms as f64).clamp(0.0, 1.0) as f32);
            }
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let err = err_thread.join().unwrap_or_default();
    if cancel.load(Ordering::Relaxed) {
        let _ = std::fs::remove_file(output);
        return Err("Cancelled".into());
    }
    if !status.success() {
        let _ = std::fs::remove_file(output);
        return Err(format!("Conversion failed: {}", err.trim().lines().last().unwrap_or("unknown error")));
    }
    progress(1.0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_trim_scale_and_normalise() {
        let p = PRESETS.iter().find(|p| p.id == "mp4-h264").unwrap();
        let o = Options { start_ms: Some(1500), end_ms: Some(4000), height: Some(720), normalize: true };
        let a = build_args("in.mkv", "out.mp4", p, &o).join(" ");
        assert!(a.contains("-ss 1.500 -i in.mkv -t 2.500"), "{a}");
        assert!(a.contains("-vf scale=-2:720") && a.contains("loudnorm") && a.ends_with("out.mp4"));
        let r = PRESETS.iter().find(|p| p.id == "mkv-remux").unwrap();
        let a = build_args("in.mp4", "out.mkv", r, &o).join(" ");
        assert!(!a.contains("scale") && !a.contains("loudnorm"));
    }

    #[test]
    fn converts_real_media_with_every_preset() {
        let src = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../test-media/h264_aac.mp4");
        if !src.is_file() || probe::ffmpeg_path().is_none() {
            eprintln!("skipped: run scripts/make-test-media.sh and install FFmpeg");
            return;
        }
        let dir = std::env::temp_dir().join(format!("halcyon-convert-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = src.to_string_lossy().into_owned();
        for p in PRESETS {
            let out = dir.join(format!("out-{}.{}", p.id, p.ext)).to_string_lossy().into_owned();
            let o = Options { end_ms: Some(2000), ..Default::default() };
            let last = std::sync::Mutex::new(0.0f32);
            run(&src, &out, p.id, &o, &AtomicBool::new(false), |f| *last.lock().unwrap() = f).unwrap_or_else(|e| panic!("{}: {e}", p.id));
            assert_eq!(*last.lock().unwrap(), 1.0);
            let r = probe::probe(&out).unwrap();
            let d: f64 = r.format.duration.unwrap().parse().unwrap();
            assert!((d - 2.0).abs() < 0.3, "{} duration {d}", p.id);
            let has_video = r.streams.iter().any(|s| s.codec_type.as_deref() == Some("video"));
            assert_eq!(has_video, !p.audio_only, "{}", p.id);
        }
        // Cancel stops the job and leaves no partial file.
        let out = dir.join("cancel.mp4").to_string_lossy().into_owned();
        let cancel = AtomicBool::new(true);
        assert!(run(&src, &out, "mp4-h264", &Options::default(), &cancel, |_| {}).is_err());
        assert!(!std::path::Path::new(&out).exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
