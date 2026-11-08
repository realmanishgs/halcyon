# Feature & format matrix

Legend: **✅ Verified** (automated test on real media, Windows 11 x64, libVLC 3.0.24) ·
**🟡 Expected** (engine plugin present, not yet tested by us) · **⚪ Planned** · **❌ Not supported**.
Nothing is marked ✅ without a test in `src-tauri/src/engine/tests.rs` or a recorded manual check.

## Formats (playback via libVLC; inspection via ffprobe)

| Container | Video codec | Audio codec | Status | Test |
|---|---|---|---|---|
| MP4 | H.264 | AAC | ✅ | `h264_aac_mp4` |
| MKV | HEVC Main10 (yuv420p10le) | E-AC-3 | ✅ (+ visual check, embedded window) | `hevc10_eac3_mkv` |
| WebM | VP9 | Opus | ✅ | `vp9_opus_webm` |
| MKV | AV1 | Opus | ✅ | `av1_opus_mkv` |
| MPEG-TS | MPEG-2 | MP2 | ✅ | `mpeg2_ts` |
| MKV | H.264 | 2× AAC + SRT | ✅ track switching | `multi_track_selection` |
| MP3 | — | MP3 | ✅ | `mp3` |
| FLAC | — | FLAC + Vorbis tags | ✅ metadata | `flac_metadata` |
| WAV | — | PCM s24le 5.1 | ✅ 6 ch detected | `wav_51` |
| AVI, MOV, M4V, FLV, WMV/ASF, MPEG-PS, 3GP, OGG/OGM, MXF | various | various | 🟡 demux plugins present | — |
| — | MPEG-4 Part 2, VC-1, Theora, MPEG-1 | — | 🟡 avcodec/theora plugins | — |
| — | — | AIFF, ALAC, Vorbis, WMA, APE, WavPack, AC-3, DTS, TrueHD | 🟡 | — |
| — | — | MIDI | 🟡 needs a SoundFont for fluidsynth | — |
| IAMF (.iamf, IAMF-in-MP4) | — | Opus/AAC-LC/FLAC/LPCM | ❌ in libVLC 3 · ⚪ Lab pipeline (Phase 5) | — |

## Robustness

| Case | Status | Test |
|---|---|---|
| Unicode (CJK + diacritics) filename | ✅ | `unicode_filename` |
| Win32 verbatim `\\?\` paths | ✅ | `verbatim_paths_are_normalised` |
| Random bytes with media extension | ✅ engine → Error, probe rejects | `garbage_is_rejected_cleanly` |
| Truncated MP4 | ✅ terminates, no hang | `truncated_file_does_not_hang` |
| Very long paths (>260), huge files, 24 h sessions | ⚪ | — |

## Playback features

| Feature | Status | Notes |
|---|---|---|
| Play/pause/stop/seek | ✅ | seek accuracy checked ±4 s window (keyframe) |
| Speed 0.25–3× | ✅ | `rate_change` |
| Frame step | 🟡 wired to `libvlc_media_player_next_frame`, manual only |
| Volume / mute | 🟡 wired, manual only |
| Audio / subtitle track selection (embedded) | ✅ | |
| External subtitle loading, delay, styling | ⚪ Phase 3 | |
| Chapters (shown count) / navigation | 🟡 count / ⚪ nav | |
| Playlists, queue, resume, history, bookmarks | ⚪ Phase 2b/3 | needs storage layer |
| A-B repeat, loop, shuffle, gapless | ⚪ | |
| Video adjust, crop, aspect, deinterlace, screenshots | ⚪ Phase 4 | |
| EQ, normalization, ReplayGain, device selection, passthrough | ⚪ Phase 4 | |
| Hardware decoding | 🟡 d3d11va/dxva2 plugins present; **not yet verified active** | diagnostics planned |
| Network streams (HTTP/HLS/DASH/RTSP/SMB/…) | 🟡 access plugins present / ⚪ UI | |
| Conversion / stream output | ⚪ Phase 4 | |
| Immersive Audio Lab | ⚪ Phase 5 | |

## Platforms

| OS | Status |
|---|---|
| Windows 10/11 x64 | ✅ development target |
| macOS / Linux | ⚪ engine adapter is portable; video surface (`surface.rs`) is Windows-only today |
