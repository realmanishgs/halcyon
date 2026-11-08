//! Full stress suite (ignored by default; ~5 min). Generate media first:
//!   bash scripts/make-stress-media.sh
//! Run:
//!   cargo test --release --lib stress -- --ignored --nocapture --test-threads=1
//! Writes test-media/STRESS_REPORT.md. Every format listed in
//! test-media/formats/EXPECT.tsv (plus anything in test-media/corpus/) is opened,
//! decoded, seeked and judged against its expectation using the engine's own
//! decoder counters — nothing is inferred from file names.

use super::vlc::VlcEngine;
use super::*;
use crate::{probe, sources};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, Instant};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../test-media")
}

fn engine() -> VlcEngine {
    VlcEngine::load_with_args(&["--aout=adummy", "--vout=vdummy"]).expect("libVLC loads")
}

fn wait<F: Fn(&PlaybackStatus, &MediaStats) -> bool>(e: &VlcEngine, secs: f32, f: F) -> (PlaybackStatus, MediaStats, bool) {
    let t = Instant::now();
    loop {
        let s = e.status();
        let st = e.stats().unwrap_or_default();
        if f(&s, &st) {
            return (s, st, true);
        }
        if t.elapsed().as_secs_f32() > secs {
            return (s, st, false);
        }
        sleep(Duration::from_millis(20));
    }
}

#[derive(Default)]
struct Row {
    file: String,
    expect: String,
    note: String,
    probe: String,
    open_ms: Option<u128>,
    first_out_ms: Option<u128>,
    engine_tracks: String,
    decoded_v: i64,
    decoded_a: i64,
    lost: i64,
    seek: String,
    verdict: String,
    detail: String,
}

fn describe(p: &probe::ProbeResult) -> String {
    let mut parts = vec![p.format.format_name.clone().unwrap_or_default()];
    for s in &p.streams {
        let mut d = s.codec_name.clone().unwrap_or_else(|| "?".into());
        if let (Some(w), Some(h)) = (s.width, s.height) {
            d += &format!(" {w}x{h}");
        }
        if let Some(pf) = &s.pix_fmt {
            if pf.contains("10") || pf.contains("12") {
                d += &format!(" {pf}");
            }
        }
        if let Some(t) = &s.color_transfer {
            if t == "smpte2084" || t == "arib-std-b67" {
                d += &format!(" {t}");
            }
        }
        if let Some(c) = s.channels {
            d += &format!(" {c}ch");
        }
        if let Some(sr) = &s.sample_rate {
            d += &format!(" {}k", sr.parse::<f64>().unwrap_or(0.0) / 1000.0);
        }
        parts.push(d);
    }
    if !p.stream_groups.is_empty() {
        parts.push(format!("{} stream group(s)", p.stream_groups.len()));
    }
    parts.join(" · ")
}

/// Plays one file and judges it. Expectation "play": must reach Playing, decode
/// every stream type the container declares, advance time and land a seek.
fn run_one(e: &mut VlcEngine, path: &Path, expect: &str, note: &str) -> Row {
    let mut r = Row {
        file: path.file_name().unwrap().to_string_lossy().into(),
        expect: expect.into(),
        note: note.into(),
        ..Default::default()
    };
    let p = path.to_string_lossy().to_string();
    let probed = probe::probe(&p);
    let (want_v, want_a) = match &probed {
        Ok(pr) => {
            r.probe = describe(pr);
            let v = pr.streams.iter().any(|s| s.codec_type.as_deref() == Some("video") && s.disposition.get("attached_pic") != Some(&1));
            let a = pr.streams.iter().any(|s| s.codec_type.as_deref() == Some("audio"));
            (v, a)
        }
        Err(err) => {
            r.probe = format!("probe failed: {}", err.chars().take(80).collect::<String>());
            (false, false)
        }
    };

    let t0 = Instant::now();
    if let Err(err) = e.open(&p, None) {
        r.verdict = if expect == "play" { "FAIL".into() } else { "UNSUPPORTED (as expected)".into() };
        r.detail = err;
        return r;
    }
    let (s, _, started) = wait(e, 10.0, |s, _| matches!(s.state, PlaybackState::Playing | PlaybackState::Error | PlaybackState::Ended));
    r.open_ms = started.then(|| t0.elapsed().as_millis());
    // Let decoders run, then require the declared stream types to have produced output.
    let (s2, st, decoded) = wait(e, 6.0, |s, st| {
        let v_ok = !want_v || st.decoded_video > 0;
        let a_ok = !want_a || st.decoded_audio > 0;
        (v_ok && a_ok && s.time_ms > 400) || matches!(s.state, PlaybackState::Error | PlaybackState::Ended)
    });
    if decoded && (st.decoded_video > 0 || st.decoded_audio > 0) {
        r.first_out_ms = Some(t0.elapsed().as_millis());
    }
    let tr = e.tracks();
    r.engine_tracks = format!("{} / {}", tr.audio.iter().filter(|t| t.id >= 0).count(), tr.subtitle.iter().filter(|t| t.id >= 0).count());
    r.decoded_v = st.decoded_video;
    r.decoded_a = st.decoded_audio;
    r.lost = st.lost_pictures;
    let reached_play = s.state == PlaybackState::Playing || s2.state == PlaybackState::Playing || s2.state == PlaybackState::Ended;
    let v_ok = !want_v || st.decoded_video > 0;
    let a_ok = !want_a || st.decoded_audio > 0;
    let playable = reached_play && decoded && v_ok && a_ok && s2.state != PlaybackState::Error;

    if playable && s2.state == PlaybackState::Playing && s2.length_ms > 3000 {
        let target = s2.length_ms / 2;
        e.seek_ms(target);
        let (s3, _, landed) = wait(e, 5.0, |s, _| (s.time_ms - target).abs() < 2500 && s.state == PlaybackState::Playing);
        r.seek = if landed { format!("ok ({}→{} ms)", target, s3.time_ms) } else { format!("MISSED ({}→{} ms, {:?})", target, s3.time_ms, s3.state) };
    } else {
        r.seek = "n/a".into();
    }
    e.stop();

    let seek_ok = !r.seek.starts_with("MISSED");
    r.verdict = match (expect, playable && seek_ok) {
        ("play", true) => "PASS".into(),
        ("play", false) => "FAIL".into(),
        ("partial", true) => "PARTIAL (as documented)".into(),
        ("partial", false) => "FAIL".into(),
        (_, false) => "UNSUPPORTED (as expected)".into(),
        (_, true) => "PASS (better than expected)".into(),
    };
    if !playable {
        r.detail = format!(
            "state={:?} decoded v/a={}/{} (needs {}{})",
            s2.state, st.decoded_video, st.decoded_audio, if want_v { "video " } else { "" }, if want_a { "audio" } else { "" }
        );
    }
    r
}

fn mem_mb() -> f64 {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
        use windows_sys::Win32::System::Threading::GetCurrentProcess;
        let mut c: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb);
        return c.PagefileUsage as f64 / 1e6;
    }
    #[allow(unreachable_code)]
    0.0
}

#[test]
#[ignore]
fn stress() {
    let root = root();
    let formats = root.join("formats");
    assert!(formats.join("EXPECT.tsv").is_file(), "run scripts/make-stress-media.sh first");
    let mut e = engine();
    let mut out = String::new();
    let mut failures: Vec<String> = Vec::new();
    let started = Instant::now();

    // ---------- 1. format matrix ----------
    let mut rows = Vec::new();
    let manifest = std::fs::read_to_string(formats.join("EXPECT.tsv")).unwrap();
    let mut entries: Vec<(PathBuf, String, String)> = manifest
        .lines()
        .filter_map(|l| {
            let mut it = l.split('\t');
            Some((formats.join(it.next()?), it.next()?.to_string(), it.next().unwrap_or("").to_string()))
        })
        .collect();
    if let Ok(rd) = std::fs::read_dir(root.join("corpus")) {
        for f in rd.flatten().map(|d| d.path()).filter(|p| p.is_file()) {
            if sources::MEDIA_EXT.contains(&f.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase().as_str()) {
                entries.push((f, "play".into(), "user corpus".into()));
            }
        }
    }
    for (path, expect, note) in &entries {
        let r = run_one(&mut e, path, expect, note);
        println!("{:<34} {:<28} v/a {:>5}/{:<5} {}", r.file, r.verdict, r.decoded_v, r.decoded_a, r.detail);
        if r.verdict == "FAIL" {
            failures.push(format!("{}: {}", r.file, r.detail));
        }
        rows.push(r);
    }

    // ---------- 2. robustness: must never hang or crash ----------
    let robust = root.join("robust");
    let mut rob = Vec::new();
    let robust_cases: Vec<(PathBuf, &str)> = vec![
        (robust.join("zero_bytes.mp4"), "fail cleanly"),
        (robust.join("random_bytes.mkv"), "fail cleanly"),
        (robust.join("truncated.mp4"), "play what exists or fail cleanly"),
        (robust.join("corrupt_middle.mkv"), "play through corruption"),
        (robust.join("mp3_named_as.mp4"), "play (content sniffing)"),
        (robust.join("weird #1 100% [x] & y's {z}.mp4"), "play"),
        (robust.join("😀 émoji ünïcødé 日本語 ファイル.mp4"), "play"),
    ];
    let mut long_path = robust.clone();
    for i in 1..=10 {
        long_path.push(format!("very_long_directory_name_{i:02}"));
    }
    long_path.push("deep_file_beyond_260_characters.opus");
    let mut cases = robust_cases;
    cases.push((long_path.clone(), "play (path > 260 chars)"));
    for (p, want) in &cases {
        let t = Instant::now();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let opened = e.open(&p.to_string_lossy(), None);
        let (s, st, _) = wait(&e, 12.0, |s, st| {
            matches!(s.state, PlaybackState::Error | PlaybackState::Ended) || (s.state == PlaybackState::Playing && (st.decoded_audio > 0 || st.decoded_video > 0) && s.time_ms > 300)
        });
        let ms = t.elapsed().as_millis();
        let outcome = match (&opened, s.state) {
            (Err(err), _) => format!("open error: {err}"),
            (_, PlaybackState::Playing) => format!("playing (v/a {}/{}, corrupted blocks {})", st.decoded_video, st.decoded_audio, st.demux_corrupted),
            (_, st) => format!("{st:?}"),
        };
        let must_play = want.starts_with("play (") && !want.contains("or fail") || *want == "play";
        let ok = ms < 12_500 && (!must_play || s.state == PlaybackState::Playing);
        if !ok {
            failures.push(format!("robust {name}: {outcome}"));
        }
        let shown = if name.len() > 40 { format!("{}…", name.chars().take(38).collect::<String>()) } else { name };
        rob.push(format!("| `{}` | {} | {} | {} ms | {} |", shown, want, outcome, ms, if ok { "PASS" } else { "FAIL" }));
        e.stop();
    }
    let path_len = long_path.to_string_lossy().chars().count();

    // ---------- 3. engine torture ----------
    let mut ops = Vec::new();
    let f = |n: &str| formats.join(n).to_string_lossy().to_string();
    let mut op = |name: &str, ok: bool, detail: String, failures: &mut Vec<String>| {
        if !ok {
            failures.push(format!("{name}: {detail}"));
        }
        ops.push(format!("| {} | {} | {} |", name, if ok { "PASS" } else { "FAIL" }, detail));
    };

    // 3a. rapid open churn across formats, then the last one must play.
    let churn: Vec<String> = entries.iter().filter(|x| x.1 == "play").map(|x| x.0.to_string_lossy().to_string()).collect();
    let t = Instant::now();
    for i in 0..80 {
        let _ = e.open(&churn[i % churn.len()], None);
        sleep(Duration::from_millis(25));
    }
    e.open(&f("mp4_h264_aac.mp4"), None).unwrap();
    let (s, st, ok) = wait(&e, 10.0, |s, st| s.state == PlaybackState::Playing && st.decoded_video > 0 && s.time_ms > 300);
    op("80 rapid opens (25 ms apart) then play", ok, format!("{:.1} s, final state {:?}, decoded v {}", t.elapsed().as_secs_f32(), s.state, st.decoded_video), &mut failures);

    // 3b. seek storm on 4K.
    e.open(&f("hires_4k60_h264.mp4"), None).unwrap();
    wait(&e, 10.0, |s, _| s.state == PlaybackState::Playing && s.length_ms > 0);
    let len = e.status().length_ms.max(1);
    let mut x: u64 = 0x2545F4914F6CDD1D;
    for _ in 0..200 {
        x ^= x << 13; x ^= x >> 7; x ^= x << 17;
        e.seek_ms((x % len as u64) as i64);
        sleep(Duration::from_millis(5));
    }
    e.seek_ms(len / 3);
    let (s, _, ok) = wait(&e, 6.0, |s, _| s.state == PlaybackState::Playing && (s.time_ms - len / 3).abs() < 1500);
    op("200 random seeks on 4K60 H.264, then precise seek", ok, format!("landed {} ms for target {} ms, state {:?}", s.time_ms, len / 3, s.state), &mut failures);

    // 3c. pause/resume spam.
    for i in 0..100 {
        e.set_paused(i % 2 == 0);
        sleep(Duration::from_millis(3));
    }
    e.set_paused(false);
    let t1 = e.status().time_ms;
    let (s, _, ok) = wait(&e, 4.0, |s, _| s.state == PlaybackState::Playing && s.time_ms > t1 + 300);
    op("100 pause/resume toggles", ok, format!("then advancing: {} → {} ms", t1, s.time_ms), &mut failures);

    // 3d. rate sweep.
    let mut bad = Vec::new();
    for r in [0.25f32, 0.5, 1.0, 1.5, 2.0, 4.0, 1.0] {
        if e.set_rate(r).is_err() || (e.status().rate - r).abs() > 0.01 {
            bad.push(r);
        }
    }
    op("rate sweep 0.25×–4×", bad.is_empty(), if bad.is_empty() { "all applied".into() } else { format!("rejected {bad:?}") }, &mut failures);

    // 3e. live track switching.
    e.open(&root.join("multi_track.mkv").to_string_lossy(), None).unwrap();
    wait(&e, 10.0, |s, _| s.state == PlaybackState::Playing && s.time_ms > 300);
    let tr = e.tracks();
    let ids: Vec<i32> = tr.audio.iter().map(|t| t.id).filter(|&i| i >= 0).collect();
    let mut sw_ok = ids.len() == 2;
    for i in 0..30 {
        if ids.len() == 2 {
            let _ = e.set_audio_track(ids[i % 2]);
            let _ = e.set_subtitle_track(if i % 2 == 0 { -1 } else { tr.subtitle.iter().map(|t| t.id).max().unwrap_or(-1) });
            sleep(Duration::from_millis(30));
        }
    }
    sleep(Duration::from_millis(300));
    sw_ok &= e.status().state == PlaybackState::Playing && e.tracks().current_audio == ids[1];
    op("30 live audio+subtitle track switches", sw_ok, format!("audio ids {:?}, final {}", ids, e.tracks().current_audio), &mut failures);

    // 3f. external subtitle files (SRT, ASS, VTT) attached to a running video.
    for ext in ["srt", "ass", "vtt"] {
        e.open(&f("mp4_h264_aac.mp4"), None).unwrap();
        wait(&e, 10.0, |s, _| s.state == PlaybackState::Playing && s.time_ms > 200);
        let before = e.tracks().subtitle.iter().filter(|t| t.id >= 0).count();
        let added = e.add_subtitle_file(&f(&format!("mp4_h264_aac.{ext}")));
        let (_, _, ok) = wait(&e, 4.0, |_, _| e.tracks().subtitle.iter().filter(|t| t.id >= 0).count() > before);
        let after = e.tracks().subtitle.iter().filter(|t| t.id >= 0).count();
        op(&format!("external .{ext} subtitle loaded while playing"), added.is_ok() && ok, format!("subtitle tracks {before} → {after}"), &mut failures);
    }

    // 3g. delays.
    let ok = e.set_audio_delay_ms(250).is_ok() && e.set_subtitle_delay_ms(-500).is_ok();
    sleep(Duration::from_millis(100));
    let s = e.status();
    op("audio/subtitle delay", ok && s.audio_delay_ms == 250 && s.subtitle_delay_ms == -500, format!("audio {} ms, subtitle {} ms", s.audio_delay_ms, s.subtitle_delay_ms), &mut failures);

    // 3h. chapters.
    e.open(&f("mkv_chapters.mkv"), None).unwrap();
    let (s, _, _) = wait(&e, 10.0, |s, _| s.state == PlaybackState::Playing && s.chapter_count > 0);
    e.set_chapter(2);
    let (s2, _, ok) = wait(&e, 4.0, |s, _| s.chapter == 2 && s.time_ms >= 3800);
    op("chapter navigation", s.chapter_count == 3 && ok, format!("{} chapters, jumped to chapter {} at {} ms", s.chapter_count, s2.chapter, s2.time_ms), &mut failures);

    // 3i. 3-hour file: length and deep seek.
    e.open(&robust.join("long_3h.opus").to_string_lossy(), None).unwrap();
    let (s, _, _) = wait(&e, 10.0, |s, _| s.state == PlaybackState::Playing && s.length_ms > 0);
    let target = 2 * 3600 * 1000 + 59 * 60 * 1000;
    e.seek_ms(target);
    let (s2, _, ok) = wait(&e, 8.0, |s, _| (s.time_ms - target).abs() < 3000);
    op("3-hour file: duration and seek to 2:59:00", ok && s.length_ms > 10_700_000, format!("length {} ms, landed {} ms", s.length_ms, s2.time_ms), &mut failures);

    // 3j. memory growth across 100 open/stop cycles.
    e.stop();
    let m0 = mem_mb();
    for i in 0..100 {
        let _ = e.open(&churn[i % churn.len()], None);
        wait(&e, 3.0, |s, _| matches!(s.state, PlaybackState::Playing | PlaybackState::Error | PlaybackState::Ended));
        e.stop();
    }
    let m1 = mem_mb();
    // Allocator/codec caches grow on first use of each codec; flag only large growth.
    op("100 open/stop cycles: memory growth", m1 - m0 < 150.0, format!("private bytes {:.0} → {:.0} MB ({:+.0} MB)", m0, m1, m1 - m0), &mut failures);

    // 3k. playlists.
    let pl = root.join("playlists");
    for (name, n) in [("relative.m3u", 3usize), ("unicode.m3u8", 2), ("list.pls", 2), ("list.xspf", 2)] {
        let items = sources::expand(&[pl.join(name).to_string_lossy().to_string()]);
        let exist = items.iter().filter(|p| Path::new(p).is_file()).count();
        op(&format!("playlist {name}"), items.len() == n && exist == n, format!("{} entries, {} resolve to files", items.len(), exist), &mut failures);
    }
    let folder = sources::expand(&[formats.to_string_lossy().to_string()]);
    op("folder scan (formats/)", folder.len() >= entries.len().saturating_sub(5), format!("{} media files found", folder.len()), &mut failures);

    // ---------- report ----------
    let pass = rows.iter().filter(|r| r.verdict.starts_with("PASS")).count();
    let partial = rows.iter().filter(|r| r.verdict.starts_with("PARTIAL")).count();
    let unsup = rows.iter().filter(|r| r.verdict.starts_with("UNSUPPORTED")).count();
    let fail = rows.iter().filter(|r| r.verdict == "FAIL").count();
    let _ = writeln!(out, "# Halcyon stress report\n");
    let _ = writeln!(out, "Engine: {} {} · ffprobe: {} · headless (VLC dummy audio/video outputs; decoders run for real) · total {:.0} s\n",
        e.info().name, e.info().version, probe::ffprobe_path().map(|p| p.display().to_string()).unwrap_or_default(), started.elapsed().as_secs_f32());
    let _ = writeln!(out, "**Formats:** {} files — {pass} pass, {partial} partial as documented, {unsup} unsupported as documented, {fail} unexpected failures.  ", rows.len());
    let _ = writeln!(out, "**Robustness:** {} cases. **Engine torture:** {} checks. **Failures overall:** {}.\n", rob.len(), ops.len(), failures.len());
    let _ = writeln!(out, "Verdict rules: `play` files must reach Playing, produce decoded output for every declared stream type (libVLC decoder counters), and land a seek to 50 %. `partial` files must play what the engine supports (see note). `unsupported` files must fail without hanging. *Input start* = libVLC input thread running; *first output* = first decoded audio/video frame (both measured from the open call).\n");
    let _ = writeln!(out, "## Format matrix\n\n| File | Expect | What ffprobe sees | Input start | First output | Engine audio / sub tracks | Decoded video / audio | Lost frames | Seek 50 % | Verdict | Note |\n|---|---|---|---|---|---|---|---|---|---|---|");
    for r in &rows {
        let _ = writeln!(out, "| `{}` | {} | {} | {} | {} | {} | {} / {} | {} | {} | **{}** | {}{} |",
            r.file, r.expect, r.probe.replace('|', "/"), r.open_ms.map(|m| format!("{m} ms")).unwrap_or("—".into()),
            r.first_out_ms.map(|m| format!("{m} ms")).unwrap_or("—".into()), r.engine_tracks,
            r.decoded_v, r.decoded_a, r.lost, r.seek, r.verdict, r.note, if r.detail.is_empty() { String::new() } else { format!(" — {}", r.detail) });
    }
    let _ = writeln!(out, "\n## Robustness (path length of deep file: {path_len} chars)\n\n| File | Requirement | Outcome | Time | Verdict |\n|---|---|---|---|---|");
    for l in &rob {
        let _ = writeln!(out, "{l}");
    }
    let _ = writeln!(out, "\n## Engine torture\n\n| Check | Verdict | Detail |\n|---|---|---|");
    for l in &ops {
        let _ = writeln!(out, "{l}");
    }
    let mut by_verdict: BTreeMap<&str, usize> = BTreeMap::new();
    for r in &rows {
        *by_verdict.entry(r.verdict.as_str()).or_default() += 1;
    }
    std::fs::write(root.join("STRESS_REPORT.md"), &out).unwrap();
    println!("\n{by_verdict:?}\nreport: test-media/STRESS_REPORT.md");
    assert!(failures.is_empty(), "{} failure(s):\n{}", failures.len(), failures.join("\n"));
}
