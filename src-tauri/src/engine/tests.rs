//! Playback + inspection integration tests against real (generated) media.
//! Run `bash scripts/make-test-media.sh` first. Requires VLC 3.x and ffprobe.
//! Uses VLC's dummy audio/video outputs so tests are silent and headless.

use super::vlc::VlcEngine;
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
