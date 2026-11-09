# Feature & format matrix

Measured on Windows 11 x64 · libVLC 3.0.24 · FFmpeg 9.0.1 · Intel Arc 140T · 2026-11-08 (formats) and 2026-11-09 (phases 2–7).
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
| ✅ | Network streams over HTTP with range seeks | `streams_over_http_with_seeking` (local test server; no external hosts) |


## Platforms

| OS | Status |
|---|---|
| Windows 10/11 x64 | ✅ development target |
| macOS / Linux | ⚪ engine adapter is portable; the native video surface (`surface.rs`) is Windows-only today |

## Library, playlists, history (Phase 2–3, 6)

| Status | Feature | Evidence |
|---|---|---|
| ✅ | SQLite store: settings, library, playlists, resume points, history | `db::tests` (7) |
| ✅ | Library indexing from ffprobe tags (container + Ogg/Opus stream tags), incremental by size + mtime, 4 workers | `library::tests`, manual scan |
| ✅ | Albums (cover art: sidecar image or embedded picture), artists, songs, videos, search, sort, favourites | `albums_artists_and_track_order`, UI review (mock backend) |
| ✅ | Manual playlists (reorder, add, export M3U8/XSPF, import) and smart playlists (12 fields, all/any, limit, sort) | `smart_playlists_filter_and_reject_unknown_fields`, `exported_playlists_read_back` |
| ✅ | Listening stats (play counted after 30 s or half the item), private session, clear history | `favourites_history_and_stats` |
| ✅ | Lyrics: sidecar `.lrc`/`.txt` or embedded tags, synchronised highlight, click to seek | `lrc_parsing_handles_multi_stamps_and_offset` |
| ⚪ | Protected (PIN) profiles | not built — needs a decision on what a profile restricts |

## Audio, picture, engine options (Phase 4)

| Status | Feature | Evidence |
|---|---|---|
| ✅ | 10-band equalizer + 18 libVLC presets, preamp | `equalizer_adjust_crop_and_devices` |
| ✅ | Output device selection (WASAPI devices) | enumerated by the engine; headless tests use a dummy output |
| ✅ | Volume normalisation, ReplayGain, visualisers (spectrum, scope, VU, spectrometer, goom) | `reconfigure_keeps_position_and_accepts_every_option` |
| ✅ | Brightness, contrast, hue, saturation, gamma; crop presets | `equalizer_adjust_crop_and_devices` |
| ✅ | Rotate 90/180/270, mirror, flip (software decoding + OpenGL output; frames displayed on the real output) | `rotation_decodes_in_software_and_displays` |
| ✅ | Hardware decoding on/off actually takes effect | media option `:avcodec-hw` (instance option is ignored by libVLC 3 — measured) |
| ✅ | Subtitle style: size, colour, outline, background, margin | options accepted by libVLC (test); rendering not yet checked by eye |
| ✅ | Sleep timer (fade + pause, or end of item), mini player (always on top), accent picker, themes | UI review (mock backend); window behaviour not yet checked by eye |

## Network (Phase 4)

| Status | Feature | Evidence |
|---|---|---|
| ✅ | Browse folders as locations (same code path as SMB/FTP/SFTP/NFS shares) | `browses_a_local_folder_as_a_location` |
| ✅ | Unreachable shares fail fast (< 4 s), no hang | same test |
| ✅ | UPnP / mDNS discovery services listed and runnable | same test (no servers on the test LAN, so no results asserted) |
| 🟡 | Real SMB/UPnP servers | not tested — none available here |

## Conversion (Phase 4)

| Status | Feature | Evidence |
|---|---|---|
| ✅ | 9 presets (MP4 H.264/HEVC, WebM VP9, MKV remux, MP3, M4A AAC, Opus, FLAC, WAV), trim, scale, loudness normalisation, progress, cancel with cleanup | `converts_real_media_with_every_preset`, `args_trim_scale_and_normalise` |

## Immersive Audio Lab (Phase 5)

| Status | Feature | Evidence |
|---|---|---|
| ✅ | Integrated loudness, LRA, momentary, short-term (BS.1770-4 / EBU R128) | EBU 1 kHz −23 dBFS → −23.0 LUFS ±0.1; 5.1 dynamic material within 0.2 LU of FFmpeg `ebur128` |
| ✅ | True peak (4× oversampled), sample peak, per-channel RMS and DC offset | inter-sample peak test (fs/4 at 45°): true peak within 0.2 dB, sample peak −3 dB lower |
| ✅ | Clipping (runs ≥ 3 samples at full scale) and silence (≤ −60 dBFS for ≥ 2 s) with timestamps | `finds_clipping_and_silence` |
| ✅ | Timeline graph synced to the playhead, click to play from a point, JSON/CSV reports | UI review (mock backend) |
| ✅ | IAMF stream groups shown; each substream measured and auditioned; A/B with level matching | UI review; substream decode uses FFmpeg |
| ⏸ | IAMF mix rendering (demixing, ambisonics, binaural), AOM conformance vectors | needs libiamf (not built yet) |
| ⏭ | Live meters during playback | needs an audio tap; measurements are offline today |

## Production (Phase 7)

| Status | Item | Evidence |
|---|---|---|
| ✅ | Packages for Windows (MSI, NSIS), macOS (DMG arm64; x64 cross-built), Linux (DEB, RPM, AppImage) | GitHub Actions `release.yml` |
| ✅ | CycloneDX SBOM | `scripts/sbom.mjs` (482 components) |
| ✅ | Randomised parser robustness tests | `src/fuzz.rs` (5 targets, ~100 k inputs; found 1 crash, fixed) |
| ✅ | Accessibility: names, labels, headings, keyboard focus, WCAG AA contrast | in-browser DOM audit, 10 pages × 2 themes, clean after token fix |
| ✅ | Cold start < 1 s, idle CPU < 1 % | `scripts/perf-check.ps1`: 548–869 ms, 0.02 % |
| ⏸ | Code signing, curated LGPL plugin bundle | need certificates / licence decision |
