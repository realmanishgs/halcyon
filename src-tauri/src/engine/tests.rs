//! Playback + inspection integration tests against real (generated) media.
//! Run `bash scripts/make-test-media.sh` first. Requires VLC 3.x and ffprobe.
//! Uses VLC's dummy audio/video outputs so tests are silent and headless.

use super::vlc::{self, VlcEngine};
use super::*;
use crate::probe;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};

fn media(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../test-media").join(name);
    assert!(p.is_file(), "missing fixture {name}; run scripts/make-test-media.sh");
    p.canonicalize().unwrap().to_string_lossy().into_owned() // verbatim \\?\ path: exercises plain_path()
}

/// Tests that rebuild libVLC's filter chains are timing-sensitive under parallel
/// load (software decode + filters starve on a busy machine); run them one at a time.
static HEAVY: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn heavy() -> std::sync::MutexGuard<'static, ()> {
    HEAVY.lock().unwrap_or_else(|p| p.into_inner())
}

fn headless() -> VlcEngine {
    VlcEngine::load_with_args(&["--aout=adummy", "--vout=vdummy"]).expect("libVLC loads")
}

fn wait_for(e: &VlcEngine, pred: impl Fn(&PlaybackStatus) -> bool, secs: u64) -> PlaybackStatus {
    let t = Instant::now();
    loop {
        let s = e.status();
        if pred(&s) || t.elapsed() > Duration::from_secs(secs) {
            return s;
        }
        sleep(Duration::from_millis(50));
    }
}

/// Opens, verifies time advances, seeks, pauses. Returns the probe.
fn plays(name: &str, expect_codecs: &[&str]) -> probe::ProbeResult {
    let path = media(name);
    let pr = probe::probe(&path).expect("probe");
    let codecs: Vec<_> = pr.streams.iter().filter_map(|s| s.codec_name.clone()).collect();
    for c in expect_codecs {
        assert!(codecs.iter().any(|x| x == c), "{name}: expected {c}, got {codecs:?}");
    }
    let mut e = headless();
    e.open(&path, None).expect("open");
    let s = wait_for(&e, |s| s.state == PlaybackState::Playing && s.time_ms > 300, 10);
    assert_eq!(s.state, PlaybackState::Playing, "{name}: did not start ({s:?})");
    assert!(s.length_ms > 5_000, "{name}: length {}", s.length_ms);
    e.seek_ms(8_000);
    let s = wait_for(&e, |s| s.time_ms >= 8_000, 5);
    assert!(s.time_ms >= 8_000 && s.time_ms < 12_000, "{name}: seek landed at {}", s.time_ms);
    e.set_paused(true);
    let s = wait_for(&e, |s| s.state == PlaybackState::Paused, 5);
    assert_eq!(s.state, PlaybackState::Paused);
    let t1 = e.status().time_ms;
    sleep(Duration::from_millis(400));
    assert_eq!(t1, e.status().time_ms, "{name}: time moved while paused");
    pr
}

#[test] fn h264_aac_mp4() { plays("h264_aac.mp4", &["h264", "aac"]); }
#[test] fn hevc10_eac3_mkv() { plays("hevc10_eac3.mkv", &["hevc", "eac3"]); }
#[test] fn vp9_opus_webm() { plays("vp9_opus.webm", &["vp9", "opus"]); }
#[test] fn av1_opus_mkv() { plays("av1_opus.mkv", &["av1", "opus"]); }
#[test] fn mpeg2_ts() { plays("mpeg2.ts", &["mpeg2video", "mp2"]); }
#[test] fn mp3() { plays("tone.mp3", &["mp3"]); }
#[test] fn wav_51() {
    let pr = plays("surround51.wav", &["pcm_s24le"]);
    assert_eq!(pr.streams[0].channels, Some(6));
}
#[test] fn unicode_filename() { plays("ünïcødé 日本語 ファイル.mp4", &["h264"]); }

#[test]
fn flac_metadata() {
    plays("tagged.flac", &["flac"]);
    let mut e = headless();
    e.open(&media("tagged.flac"), None).unwrap();
    wait_for(&e, |s| s.state == PlaybackState::Playing, 10);
    sleep(Duration::from_millis(300));
    let m = e.meta();
    assert_eq!(m.title.as_deref(), Some("Sine Study"));
    assert_eq!(m.artist.as_deref(), Some("Halcyon Test"));
    assert_eq!(m.album.as_deref(), Some("Fixtures"));
}

#[test]
fn multi_track_selection() {
    let pr = plays("multi_track.mkv", &["h264", "aac", "subrip"]);
    assert_eq!(pr.streams.iter().filter(|s| s.codec_type.as_deref() == Some("audio")).count(), 2);
    let mut e = headless();
    e.open(&media("multi_track.mkv"), None).unwrap();
    wait_for(&e, |s| s.state == PlaybackState::Playing && s.time_ms > 300, 10);
    let t = e.tracks();
    // libVLC lists "Disable" (-1) plus each real track.
    assert_eq!(t.audio.iter().filter(|x| x.id >= 0).count(), 2, "{:?}", t.audio);
    assert_eq!(t.subtitle.iter().filter(|x| x.id >= 0).count(), 1, "{:?}", t.subtitle);
    let second = t.audio.iter().filter(|x| x.id >= 0).nth(1).unwrap().id;
    e.set_audio_track(second).unwrap();
    sleep(Duration::from_millis(300));
    assert_eq!(e.tracks().current_audio, second);
    let sub = t.subtitle.iter().find(|x| x.id >= 0).unwrap().id;
    e.set_subtitle_track(sub).unwrap();
    sleep(Duration::from_millis(300));
    assert_eq!(e.tracks().current_subtitle, sub);
}

#[test]
fn rate_change() {
    let mut e = headless();
    e.open(&media("h264_aac.mp4"), None).unwrap();
    wait_for(&e, |s| s.state == PlaybackState::Playing, 10);
    e.set_rate(2.0).unwrap();
    assert!((e.status().rate - 2.0).abs() < 0.01);
}

#[test]
fn garbage_is_rejected_cleanly() {
    assert!(probe::probe(&media("garbage.mkv")).is_err());
    let mut e = headless();
    e.open(&media("garbage.mkv"), None).unwrap(); // open is async in libVLC
    let s = wait_for(&e, |s| matches!(s.state, PlaybackState::Error | PlaybackState::Ended), 10);
    assert!(matches!(s.state, PlaybackState::Error | PlaybackState::Ended), "got {s:?}");
}

#[test]
fn truncated_file_does_not_hang() {
    let mut e = headless();
    e.open(&media("truncated.mp4"), None).unwrap();
    let s = wait_for(&e, |s| matches!(s.state, PlaybackState::Error | PlaybackState::Ended), 15);
    assert!(matches!(s.state, PlaybackState::Error | PlaybackState::Ended), "got {s:?}");
}

#[test]
fn verbatim_paths_are_normalised() {
    assert_eq!(plain_path(r"\\?\C:\a\b.mkv"), r"C:\a\b.mkv");
    assert_eq!(plain_path(r"\\?\UNC\nas\share\x.mp4"), r"\\nas\share\x.mp4");
    assert_eq!(plain_path(r"D:\plain.mp3"), r"D:\plain.mp3");
}

#[test]
fn pause_then_resume() {
    let mut e = headless();
    e.open(&media("h264_aac.mp4"), None).unwrap();
    wait_for(&e, |s| s.state == PlaybackState::Playing && s.time_ms > 300, 10);
    e.set_paused(true);
    let s = wait_for(&e, |s| s.state == PlaybackState::Paused, 5);
    assert_eq!(s.state, PlaybackState::Paused);
    let t = s.time_ms;
    e.set_paused(false);
    let s = wait_for(&e, |s| s.state == PlaybackState::Playing && s.time_ms > t + 500, 5);
    assert!(s.time_ms > t + 500, "did not resume: {s:?}");
}

#[test]
fn play_and_seek_after_end() {
    let mut e = headless();
    e.open(&media("tone.mp3"), None).unwrap();
    wait_for(&e, |s| s.state == PlaybackState::Playing && s.length_ms > 0, 10);
    let len = e.status().length_ms;
    e.seek_ms(len - 300);
    let s = wait_for(&e, |s| s.state == PlaybackState::Ended, 10);
    assert_eq!(s.state, PlaybackState::Ended);
    // Seek after the end restarts the input and lands on the target.
    e.seek_ms(5_000);
    let s = wait_for(&e, |s| s.state == PlaybackState::Playing && s.time_ms >= 5_000, 5);
    assert!(s.state == PlaybackState::Playing && s.time_ms >= 5_000 && s.time_ms < 9_000, "{s:?}");
    // Play after the end restarts from the beginning.
    e.seek_ms(len - 300);
    wait_for(&e, |s| s.state == PlaybackState::Ended, 10);
    e.play().unwrap();
    let s = wait_for(&e, |s| s.state == PlaybackState::Playing && s.time_ms > 100, 5);
    assert!(s.state == PlaybackState::Playing && s.time_ms < 3_000, "{s:?}");
}

#[test]
fn path_over_260_chars_plays() {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../test-media/robust");
    for i in 1..=10 {
        p.push(format!("very_long_directory_name_{i:02}"));
    }
    p.push("deep_file_beyond_260_characters.opus");
    if !p.is_file() {
        eprintln!("skipped: run scripts/make-stress-media.sh");
        return;
    }
    let s = p.to_string_lossy().to_string();
    assert!(s.chars().count() > 260);
    assert!(engine_path(&s).chars().count() < 260, "short form expected: {}", engine_path(&s));
    let mut e = headless();
    e.open(&s, None).unwrap();
    let st = wait_for(&e, |s| s.state == PlaybackState::Playing && s.time_ms > 200, 10);
    assert_eq!(st.state, PlaybackState::Playing);
}

#[test]
fn events_reach_the_sink() {
    use std::sync::{Arc, Mutex};
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut e = headless();
    let s2 = seen.clone();
    e.set_event_sink(Arc::new(move |ev| s2.lock().unwrap().push(ev)));
    e.open(&media("h264_aac.mp4"), None).unwrap();
    wait_for(&e, |s| s.state == PlaybackState::Playing, 10);
    sleep(Duration::from_millis(300));
    e.seek_ms(e.status().length_ms - 400);
    wait_for(&e, |s| s.state == PlaybackState::Ended, 10);
    sleep(Duration::from_millis(200));
    let v = seen.lock().unwrap().clone();
    for want in [EngineEvent::Opening, EngineEvent::Playing, EngineEvent::Length, EngineEvent::Tracks, EngineEvent::Ended] {
        assert!(v.contains(&want), "missing {want:?} in {v:?}");
    }
}

#[test]
fn reconfigure_keeps_position_and_accepts_every_option() {
    let _serial = heavy();
    let mut e = headless();
    e.open(&media("h264_aac.mp4"), None).unwrap();
    wait_for(&e, |s| s.state == PlaybackState::Playing, 10);
    e.seek_ms(6_000);
    wait_for(&e, |s| s.time_ms >= 6_000, 5);
    e.set_volume(70);
    let cfg = EngineConfig {
        hw_decode: false,
        normalize: true,
        replay_gain: true,
        visualizer: Some("spectrum".into()),
        transform: "90".into(),
        sub_scale: 150,
        sub_color: 0xFFFF00,
        sub_outline: 6,
        sub_background: 128,
        sub_margin: 40,
    };
    assert!(e.configure(&cfg).unwrap(), "new options recreate the instance");
    assert!(!e.configure(&cfg).unwrap(), "same options are a no-op");
    let s = wait_for(&e, |s| s.state == PlaybackState::Playing && s.time_ms >= 6_000, 10);
    assert_eq!(s.state, PlaybackState::Playing);
    assert!(s.time_ms >= 6_000 && s.time_ms < 9_000, "resumed at {}", s.time_ms);
    // (Volume restore is checked in the app: the dummy audio output has no volume control.)
    // (Frames through the filters are checked with a real output in rotation_decodes_in_software_and_displays.)
    // Every visualiser and transform is accepted by libVLC.
    for v in ["scope", "vuMeter", "spectrometer", "goom"] {
        e.configure(&EngineConfig { visualizer: Some(v.into()), ..Default::default() }).unwrap_or_else(|err| panic!("{v}: {err}"));
    }
    for t in ["180", "270", "hflip", "vflip"] {
        let cfg = EngineConfig { transform: t.into(), ..Default::default() };
        assert!(cfg.media_options().contains(&":avcodec-hw=none".to_string()), "rotation forces software decoding");
        e.configure(&cfg).unwrap_or_else(|err| panic!("{t}: {err}"));
        // Display with each transform is covered by rotation_decodes_in_software_and_displays.
    }
}

#[test]
fn equalizer_adjust_crop_and_devices() {
    let _serial = heavy();
    let mut e = headless();
    let info = e.equalizer_info();
    assert_eq!(info.frequencies.len(), 10);
    assert!(info.presets.len() >= 15, "{} presets", info.presets.len());
    assert!(info.presets.iter().any(|(n, _)| n == "Rock"));
    e.open(&media("h264_aac.mp4"), None).unwrap();
    wait_for(&e, |s| s.state == PlaybackState::Playing, 10);
    let rock = info.presets.iter().find(|(n, _)| n == "Rock").unwrap().1.clone();
    e.set_equalizer(Some(&rock)).unwrap();
    e.set_equalizer(None).unwrap();
    e.set_video_adjust(Some(&VideoAdjust { brightness: 1.2, contrast: 1.1, hue: 10.0, saturation: 1.5, gamma: 1.0 }));
    e.set_crop(Some("16:9"));
    e.set_crop(None);
    e.set_video_adjust(None);
    // Filter changes rebuild the video chain; give it time under parallel test load.
    let before = e.stats().unwrap().decoded_video;
    sleep(Duration::from_millis(1500));
    assert!(e.stats().unwrap().decoded_video > before, "decoding continues after picture changes");
    // adummy exposes no devices; the real output is checked in the app.
    let _ = e.audio_devices();
}

#[test]
fn browses_a_local_folder_as_a_location() {
    let mut e = headless();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../test-media");
    let url = vlc::file_uri(&dir.canonicalize().unwrap().to_string_lossy());
    let items = e.browse(&url, 5000).expect("browse");
    assert!(items.iter().any(|i| i.name == "h264_aac.mp4" && i.kind == "file"), "{items:?}");
    assert!(e.discovery_services().iter().any(|(n, _)| n == "upnp"), "{:?}", e.discovery_services());
    // An unreachable share either errors or lists nothing; it must not hang.
    let t = Instant::now();
    assert!(e.browse("smb://definitely-not-a-host.invalid/share", 1500).map(|v| v.is_empty()).unwrap_or(true));
    assert!(t.elapsed() < Duration::from_secs(4));
}

/// Minimal HTTP/1.1 file server with Range support, for streaming tests.
fn serve(path: PathBuf) -> u16 {
    use std::io::{BufRead, BufReader, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let data = std::fs::read(&path).unwrap();
        for conn in l.incoming().flatten() {
            let data = data.clone();
            std::thread::spawn(move || {
                let mut r = BufReader::new(conn.try_clone().unwrap());
                let mut range = None;
                let mut line = String::new();
                while r.read_line(&mut line).unwrap_or(0) > 0 {
                    let l = line.trim().to_ascii_lowercase();
                    if l.is_empty() {
                        break;
                    }
                    if let Some(v) = l.strip_prefix("range: bytes=") {
                        let (a, b) = v.split_once('-').unwrap();
                        let a: usize = a.parse().unwrap_or(0);
                        let b: usize = b.parse().unwrap_or(data.len() - 1);
                        range = Some((a, b.min(data.len() - 1)));
                    }
                    line.clear();
                }
                let mut w = conn;
                let (a, b) = range.unwrap_or((0, data.len() - 1));
                let head = if range.is_some() {
                    format!("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {a}-{b}/{}\r\n", data.len())
                } else {
                    "HTTP/1.1 200 OK\r\n".to_string()
                };
                let _ = write!(w, "{head}Accept-Ranges: bytes\r\nContent-Type: video/mp4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", b - a + 1);
                let _ = w.write_all(&data[a..=b]);
            });
        }
    });
    port
}

#[test]
fn streams_over_http_with_seeking() {
    let port = serve(PathBuf::from(media("h264_aac.mp4")));
    let mut e = headless();
    e.open(&format!("http://127.0.0.1:{port}/h264_aac.mp4"), None).unwrap();
    let s = wait_for(&e, |s| s.state == PlaybackState::Playing && s.time_ms > 300, 10);
    assert_eq!(s.state, PlaybackState::Playing);
    assert!(s.seekable && s.length_ms > 5_000, "{s:?}");
    e.seek_ms(7_000);
    let s = wait_for(&e, |s| s.time_ms >= 7_000, 8);
    assert!(s.time_ms >= 7_000, "seek over HTTP landed at {}", s.time_ms);
    assert!(e.stats().unwrap().decoded_video > 0);
}

#[test]
fn rotation_decodes_in_software_and_displays() {
    // Uses the engine's real video output choice (a small window appears briefly),
    // because the dummy output hides the stall this guards against.
    let _serial = heavy();
    let log = std::env::temp_dir().join(format!("halcyon-hw-{}.log", std::process::id()));
    let logarg = format!("--logfile={}", log.display());
    let mut e = VlcEngine::load_with_args(&["--aout=adummy", "--file-logging", &logarg, "--log-verbose=2"]).unwrap();
    for t in ["90", "180", "270", "hflip", "vflip"] {
        e.configure(&EngineConfig { transform: t.into(), ..Default::default() }).unwrap();
        e.open(&media("h264_aac.mp4"), None).unwrap();
        wait_for(&e, |s| s.state == PlaybackState::Playing, 10);
        sleep(Duration::from_millis(1200));
        let st = e.stats().unwrap();
        assert!(st.decoded_video > 10 && st.displayed_pictures > 3, "{t}: decoded {} displayed {}", st.decoded_video, st.displayed_pictures);
    }
    drop(e);
    let text = String::from_utf8_lossy(&std::fs::read(&log).unwrap_or_default()).into_owned();
    let _ = std::fs::remove_file(&log);
    assert!(!text.contains("for hardware decoding"), "hardware decoding was used with a rotation filter");
}

#[test]
fn visualisers_create_a_video_output() {
    let _serial = heavy();
    let mut e = VlcEngine::load_with_args(&["--aout=adummy"]).unwrap();
    for v in ["spectrum", "scope", "vuMeter", "spectrometer", "goom"] {
        e.configure(&EngineConfig { visualizer: Some(v.into()), ..Default::default() }).unwrap();
        e.open(&media("tagged.flac"), None).unwrap();
        wait_for(&e, |s| s.state == PlaybackState::Playing, 10);
        sleep(Duration::from_millis(1500));
        // libVLC's picture counters do not include visualiser frames, so this proves the
        // video output exists for audio-only media; what it draws is checked by eye.
        assert!(e.status().has_video, "{v}: no video output for the visualiser");
    }
}

/// Mean per-pixel colour spread (max - min channel) of a PNG snapshot: ~0 for grey.
fn colourfulness(png: &std::path::Path) -> f64 {
    let ff = probe::ffmpeg_path().unwrap();
    let out = probe::quiet(ff).args(["-v", "error", "-i"]).arg(png).args(["-f", "rawvideo", "-pix_fmt", "rgb24", "-s", "64x36", "-"]).output().unwrap();
    let px = out.stdout.chunks_exact(3);
    let n = px.len().max(1) as f64;
    px.map(|p| (*p.iter().max().unwrap() as f64) - (*p.iter().min().unwrap() as f64)).sum::<f64>() / n
}

fn mean_level(png: &std::path::Path) -> f64 {
    let ff = probe::ffmpeg_path().unwrap();
    let out = probe::quiet(ff).args(["-v", "error", "-i"]).arg(png).args(["-f", "rawvideo", "-pix_fmt", "gray", "-s", "64x36", "-"]).output().unwrap();
    out.stdout.iter().map(|b| *b as f64).sum::<f64>() / out.stdout.len().max(1) as f64
}

#[test]
fn picture_adjust_survives_rotation() {
    // Real-use order, as in the app: adjust on the normal output, then rotate
    // (engine restart), then adjust again while rotated.
    let _serial = heavy();
    if probe::ffmpeg_path().is_none() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("halcyon-adj-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let snap = |e: &mut VlcEngine, name: &str| {
        let p = dir.join(name);
        e.snapshot(&p.to_string_lossy()).unwrap();
        sleep(Duration::from_millis(300));
        (colourfulness(&p), mean_level(&p))
    };
    let mut e = VlcEngine::load_with_args(&["--aout=adummy"]).unwrap();
    let grey = VideoAdjust { brightness: 1.0, contrast: 1.0, hue: 0.0, saturation: 0.0, gamma: 1.0 };
    let dark = VideoAdjust { brightness: 0.5, contrast: 1.0, hue: 0.0, saturation: 1.0, gamma: 1.0 };
    e.open(&media("h264_aac.mp4"), None).unwrap();
    wait_for(&e, |s| s.state == PlaybackState::Playing && s.has_video, 10);
    e.set_video_adjust(Some(&grey));
    e.configure(&EngineConfig { transform: "90".into(), ..Default::default() }).unwrap();
    wait_for(&e, |s| s.state == PlaybackState::Playing && s.has_video, 10);
    sleep(Duration::from_millis(1200));
    let after_restart = snap(&mut e, "a.png");
    // The app re-applies picture settings when a video output appears; doing it again must be harmless.
    e.set_video_adjust(Some(&grey));
    sleep(Duration::from_millis(1200));
    let reapplied = snap(&mut e, "a2.png");
    println!("
REAPPLY {reapplied:?}");
    e.set_video_adjust(Some(&dark));
    sleep(Duration::from_millis(1200));
    let changed_while_rotated = snap(&mut e, "b.png");
    e.set_video_adjust(None);
    sleep(Duration::from_millis(1200));
    let reset = snap(&mut e, "c.png");
    println!("
ADJ restart {after_restart:?} dark {changed_while_rotated:?} reset {reset:?}");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(after_restart.0 < 10.0, "adjustment lost across the engine restart: {after_restart:?}");
    assert!(changed_while_rotated.0 > 60.0 && changed_while_rotated.1 < reset.1 - 20.0, "darkening while rotated: {changed_while_rotated:?} vs {reset:?}");
}
