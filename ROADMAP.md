# Roadmap

Each phase ships as tested vertical slices; a feature moves to ✅ in FEATURE_MATRIX only with a test.

## Phase 1 — Discovery & architecture ✅ (2026-11-08)
Project audit, toolchain inventory, VLC headers + plugin inventory, Eclipsa
and IAMF public research, engine comparison, architecture, design tokens, license audit, this roadmap.
REA not available (see REVERSE_ENGINEERING_NOTES).

## Phase 2 — Foundation (in progress)
- ✅ Tauri shell, design system, navigation skeleton, `MediaEngine` trait, libVLC adapter, native video surface
- ✅ Vertical slice: open → probe streams → play → controls → metadata
- ⏭ Next: SQLite storage (rusqlite): settings, recent files, resume positions
- ⏭ libVLC event manager → push events instead of 4 Hz polling
- ⏭ Fullscreen (window + surface), double-click to toggle; controls outside/auto-hide strip
- ⏭ Settings persistence: theme, volume, last folder

## Phase 3 — Core playback
Queue & playlists (M3U/M3U8/PLS/XSPF import/export), external subtitles + delay + style, audio delay,
chapters navigation, folder library scan (background thread, incremental), artwork cache, HW-decode
diagnostics (`--avcodec-hw` modes, verify d3d11va active via stats), compatibility dashboard.

## Phase 4 — Advanced
Video adjust/crop/aspect/deinterlace/rotation, screenshots, EQ + normalization + device selection,
network URLs (HTTP/HLS/DASH/RTSP) and SMB/NFS/UPnP browsing, conversion via libVLC sout or FFmpeg.

## Phase 5 — Immersive Audio Lab
libiamf build + Rust bindings, OBU inspector, stereo/channel-layout render, binaural render (after
capability verification), dual-bus A/B, live meters, BS.1770-4 loudness + true peak, clipping/silence
detection, JSON/CSV reports, AOM conformance vector suite.

## Phase 6 — Experience
Library views (music/videos/artists/albums), smart playlists, favorites, accent picker, compact layout,
visualizer, sleep timer, embedded/LRC lyrics, optional listening stats (local only), protected profiles.

## Phase 7 — Production
Curated LGPL plugin bundle + LGPL ffprobe, SBOM, installers (MSI/NSIS), code signing, CI on Windows/macOS/Linux,
fuzzing of probe path, a11y audit, perf budgets (cold start < 1 s, idle CPU < 1%), release workflow.

Open decisions: app license (permissive + curated plugins vs GPL-3.0 + full set); libVLC 4 migration timing.
