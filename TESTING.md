# Testing

## Fixtures
`scripts/make-test-media.sh` generates license-free synthetic media with FFmpeg lavfi sources
(`testsrc2`, `sine`) into `test-media/` (gitignored): H.264/AAC MP4, HEVC Main10/E-AC-3 MKV, VP9/Opus WebM,
AV1/Opus MKV, MPEG-2/MP2 TS, multi-audio + SRT MKV, tagged FLAC, MP3, 5.1 PCM WAV, a Unicode-named copy,
a truncated MP4, and random bytes named `.mkv`. Real-world corpora and AOM IAMF conformance vectors are Phase 3/5.

## Engine integration tests — `src-tauri/src/engine/tests.rs`
Real libVLC with `--aout=adummy --vout=vdummy` (silent, headless). Each playable fixture: probe codecs
match → reaches `Playing` with time > 300 ms → length > 5 s → seek to 8 s lands in [8 s, 12 s) → pause holds time.

```bash
cd src-tauri && cargo test
```

### Results — 2026-11-08, Windows 11 x64, libVLC 3.0.24, ffprobe 9.0.1
`14 passed; 0 failed` (≈3 s). Tests: h264_aac_mp4, hevc10_eac3_mkv, vp9_opus_webm, av1_opus_mkv, mpeg2_ts,
mp3, wav_51, unicode_filename, flac_metadata, multi_track_selection, rate_change, garbage_is_rejected_cleanly,
truncated_file_does_not_hang, verbatim_paths_are_normalised.

Bug found by tests and fixed: libVLC 3 and ffprobe both fail on `\\?\` verbatim paths → `plain_path()`.

## Manual UI verification (recorded)
App launched with a file argument; window captured via Win32 `CopyFromScreen`; child-window geometry
dumped with `EnumChildWindows`.
- HEVC10 MKV renders inside `.video-box` exactly (surface rect == box rect × devicePixelRatio).
  Bug found and fixed: surface used the window scale factor (1.5) while WebView2's DPR was ≈1.59 (Windows text scaling).
- Inspector toggled with `I`: container, duration, size, per-stream codec/profile/resolution/fps/pix_fmt and audio details shown from ffprobe; video re-laid-out alongside.
- FLAC shows title/artist/album from tags in hero and transport.

## Not yet covered
Frontend unit/UI tests (planned: Vitest + Playwright against WebView2 via tauri-driver), volume/frame-step
automation, HW decode verification, long-session soak, >260-char paths, large-library scans, packaging tests.
