# Feature & format matrix

Measured on Windows 11 x64 · libVLC 3.0.24 · ffprobe 9.0.1 · Intel Arc 140T · 2026-11-08.
Legend: **✅ Verified** (automated test on real media) · **🟡 Partial** (plays, with a stated gap) ·
**❌ Not supported** (fails cleanly, verified) · **⚪ Planned**.
Format rows come from the stress suite (`cargo test --lib stress -- --ignored`, report in
`test-media/STRESS_REPORT.md`): a ✅ means the engine reached Playing, its **decoder counters** showed
output for every declared stream type, and a seek to 50 % landed. Media is synthetic (FFmpeg lavfi);
real-world files can be added to `test-media/corpus/`.

## Video (container · video · audio)

| ✅ | MP4 H.264+AAC · MP4 HEVC+AAC · MP4 AV1+Opus · M4V H.264+AAC · MKV H.264+FLAC · MKV FFV1 (lossless)+FLAC · WebM VP8+Vorbis · WebM VP9+Opus · WebM AV1+Opus · AVI MPEG-4 ASP (Xvid)+MP3 · AVI MS-MPEG4v3 (DivX 3)+MP3 · AVI MJPEG+PCM · MOV ProRes 422 HQ+PCM 24 · MOV DNxHR+PCM · MOV H.264+ALAC · FLV H.264+AAC · WMV (ASF) WMV2+WMA2 · ASF WMV1+WMA2 · MPEG-PS MPEG-1+MP2 · VOB MPEG-2+AC-3 · MPEG-TS HEVC+E-AC-3 · M2TS H.264+AC-3 · 3GP H.263+AMR-NB · 3G2 MPEG-4+AAC · Ogg Theora+Vorbis · MXF OP1a MPEG-2 4:2:2+PCM · DV PAL · NUT H.264+FLAC · MPEG-2 interlaced (TFF) |
|---|---|
| ❌ | **H.266/VVC** in MKV — audio plays, video is not decoded (no VVC decoder in libVLC 3.0) |

## Hi-res video

| Status | File | Notes |
|---|---|---|
| ✅ | 4K 60 fps H.264 High | 95–123 frames decoded in the window, 0 lost |
| ✅ | 4K HEVC Main10 **HDR10** (PQ, BT.2020, mastering + MaxCLL metadata) | Inspector shows `HDR10 / PQ` |
| ✅ | 4K HEVC Main10 **HLG** | |
| ✅ | 4K **VP9 Profile 2** (10-bit) | |
| ✅ | **8K** (7680×4320) AV1 | |
| ✅ | 1080p **120 fps** H.264 | some frames dropped headless (dummy output) — see report |
| ✅ | **Hardware decoding (D3D11VA)** on Intel Arc 140T for H.264, 4K60 H.264, 4K HEVC Main10, 4K VP9 Profile 2, 8K AV1 | `scripts/hw-decode-check.sh` reads VLC's own log line "Using D3D11VA (…) for hardware decoding"; result in `test-media/HW_DECODE.md`. Verified through vlc.exe with the same libVLC/plugins Halcyon loads, not yet from inside Halcyon's process |
| 🟡 | HDR → SDR tone mapping | libVLC D3D11 output tone-maps when the display is SDR; not yet measured |
| ⚪ | Dolby Vision | ffprobe side-data (DOVI record) is surfaced as a badge when present; no Dolby Vision sample can be generated — add one to `test-media/corpus/` |

## Audio

| ✅ | MP3 · MP2 · AAC (ADTS) · M4A AAC · M4A **ALAC 24/96** · **FLAC 24/192** · FLAC 16/44.1 · WAV PCM 16 · **WAV float 32/384 kHz** · Wave64 24 · AIFF 24 · CAF ALAC · Opus · Ogg Vorbis · Ogg FLAC · Speex · WMA 2 · WavPack · TTA · AMR-NB · Sun AU · MKA Opus 5.1 · MKA FLAC 7.1 |
|---|---|
| ❌ | Raw aptX (no demuxer/decoder) · MIDI (libVLC's FluidSynth needs a SoundFont — none configured) |
| ⚪ | APE (Monkey's Audio): libVLC has a decoder; FFmpeg cannot encode a test file — add one to `corpus/` · DSD (DSF/DFF) likewise |

## Dolby & DTS

| Status | Format |
|---|---|
| ✅ | Dolby Digital (AC-3) 5.1 raw · AC-3 2.0 in MKV · AC-3 in VOB/M2TS |
| ✅ | Dolby Digital Plus (E-AC-3) 5.1 raw · E-AC-3 5.1 in MP4 (`ec-3`) · E-AC-3 in TS |
| ✅ | Dolby TrueHD 5.1 in MKV · TrueHD raw · MLP 2.0 |
| ✅ | DTS 5.1 core raw · DTS 5.1 in MKV |
| ⚪ | **Dolby Atmos** (E-AC-3 JOC / TrueHD Atmos), DTS-HD MA / DTS:X — cannot be synthesised; the bed decodes through the same decoders above, object rendering is not provided by libVLC 3. Needs real samples in `corpus/` to verify |
| ⚪ | Passthrough (S/PDIF/HDMI bitstream) — libVLC supports it; not yet exposed in settings |

## Immersive audio — IAMF / Eclipsa

| Status | File | What actually happens |
|---|---|---|
| 🟡 | `.iamf` stereo Opus · stereo LPCM 24 · scalable stereo→5.1 FLAC · first-order ambisonics Opus | libVLC demuxes IAMF through libavformat and decodes **one substream as plain audio**. Mix presentations, demixing, recon gain, ambisonics and binaural rendering are **not** applied — for 5.1/FOA only the first substream is heard. Inspector shows the IAMF Audio Element and Mix Presentation stream groups. |
| ❌ | IAMF in MP4 (`iamf` sample entry) | not decoded |
| ⚪ | Proper IAMF playback, stereo/binaural A/B, meters, BS.1770 | Immersive Audio Lab (Phase 5, `libiamf`) |

The app shows a banner whenever IAMF is detected so users are not misled.

## Subtitles

| Status | Feature |
|---|---|
| ✅ | Embedded ASS/SSA (libass), mov_text (tx3g), WebVTT, SRT-in-MKV; selection while playing |
| ✅ | External SRT / ASS / VTT loaded while playing (Tune panel, context menu, or drop onto video); sidecar auto-load by libVLC |
| ✅ | Subtitle delay (±50 ms steps, G/H) |
| ⚪ | Font/size/position styling UI, DVD/PGS image subtitles verification |

## Playback, robustness, UI

| Status | Feature | Evidence |
|---|---|---|
| ✅ | Play/pause/stop/seek, resume after end, seek after end | `pause_then_resume`, `play_and_seek_after_end`, manual UI run |
| ✅ | Playhead tracking: interpolated, no snap-back after seek, drag released outside bar | manual UI run (TESTING.md) |
| ✅ | Speed 0.25–4×, audio delay, chapters | stress torture |
| ✅ | 200 random seeks on 4K60, 100 pause toggles, 80 rapid opens, 30 live track switches | stress torture |
| ✅ | 3-hour file: duration and seek to 2:59:00 | stress torture |
| ✅ | Zero-byte, random bytes, truncated MP4: fail cleanly in ≈20 ms · corrupted MKV plays through · MP3 named `.mp4` plays | stress robustness |
| ✅ | Unicode/emoji names, `#%[]&'{}` names, **388-character path** (8.3 short-name fallback) | stress robustness, `path_over_260_chars_plays` |
| ✅ | Folders, M3U, M3U8 (UTF-8 BOM, unicode), PLS, XSPF (file URIs) → queue | `sources` unit tests + stress |
| ✅ | Drop onto video surface (`WM_DROPFILES`, Unicode + UNC paths), wheel, double-click | `wm_dropfiles_reaches_sink_with_unicode_paths` |
| ✅ | Fullscreen, auto-hide, Esc, wheel volume, native context menu, resizable/stretchable layout to 420×300 | manual UI run |
| 🟡 | Drop onto the web UI area | Tauri's built-in handler (same code path after the event); automated OLE drag could not be injected in this environment — please confirm by dragging from Explorer |
| ✅ | No memory growth across 100 open/stop cycles over 75 formats | private bytes 156 → 122 MB (stress torture, 2026-11-08) |
| ⚪ | Network streams | UI + `libvlc_media_new_location` wired; no network test run (no external hosts contacted) |
| ⚪ | Library, playlists persistence, EQ, conversion, Immersive Audio Lab | ROADMAP |

## Platforms

| OS | Status |
|---|---|
| Windows 10/11 x64 | ✅ development target |
| macOS / Linux | ⚪ engine adapter is portable; the native video surface (`surface.rs`) is Windows-only today |
