# Roadmap

Each phase ships as tested vertical slices; a feature moves to ✅ in FEATURE_MATRIX only with a test.

## Phase 1 — Discovery & architecture ✅ (2026-11-08)
Project audit, toolchain inventory, VLC headers + plugin inventory, Eclipsa
and IAMF public research, engine comparison, architecture, design tokens, license audit, this roadmap.
REA not available (see REVERSE_ENGINEERING_NOTES).

## Phase 2 — Foundation ✅ (2026-11-09)
- ✅ Tauri shell, design system, `MediaEngine` trait, libVLC adapter, native video surface, vertical slice
- ✅ SQLite storage (rusqlite, bundled SQLite): settings, recents, resume positions, library, playlists, history
- ✅ libVLC event manager pushes state changes; polling drops to 1 Hz when nothing plays
- ✅ Fullscreen, auto-hide; settings persistence (theme, accent, volume, EQ, device, engine options)

## Phase 3 — Core playback ✅ (2026-11-09)
- ✅ Queue, repeat/shuffle, playlists in/out (M3U/M3U8/PLS/XSPF), folders, drag & drop, resume, recents
- ✅ Library scan (ffprobe-indexed, 4 workers, incremental by size+mtime), search, sort, favourites
- ✅ Subtitles: external files, delay, styling (size, colour, outline, background, margin)
- ✅ Hardware-decoding switch (per media; verified it actually takes effect), compatibility dashboard of plugins
- ✅ Network streaming tested against a local HTTP server with range seeks

## Phase 4 — Advanced ✅ (2026-11-09)
- ✅ Picture: brightness/contrast/hue/saturation/gamma, crop, rotate 90/180/270, mirror/flip, aspect, deinterlace, snapshots
- ✅ Audio: 10-band equalizer with libVLC presets, volume normalisation, ReplayGain, output-device selection
- ✅ Network: HTTP/HLS/DASH/RTSP URLs; SMB/FTP/SFTP/NFS browsing; UPnP and mDNS discovery (separate libVLC instance)
- ✅ Conversion through the user's FFmpeg: 9 presets, trim, scale, loudness normalisation, progress, cancel

## Phase 5 — Immersive Audio Lab (partly ✅)
- ✅ BS.1770-4 / EBU R128 analyser: integrated, LRA, momentary, short-term, sample and true peak (4× oversampled),
  per-channel RMS/DC, clipping and silence detection, timeline graph synced to playback, JSON/CSV reports;
  matches FFmpeg's `ebur128` within 0.2 LU / 0.3 dB on 5.1 material
- ✅ IAMF: stream groups shown, every substream measurable, A/B audition of substreams with level matching
- ⏸ **Blocked:** proper IAMF rendering (mix presentations, demixing, ambisonics, binaural) needs `libiamf`.
  Not built yet: it downloads and builds external codec sources; needs a decision on building it
  from source or using a prebuilt, verified binary. AOM conformance vectors depend on it too.
- ⏭ Live (real-time) meters during playback need an audio tap (libVLC audio callbacks); today's meters are measured offline

## Phase 6 — Experience (mostly ✅)
- ✅ Library views: music (albums with covers, artists, songs), videos; favourites; smart playlists (rules, limit, sort)
- ✅ Accent picker, light/dark/system theme, mini player (always on top), visualisers (spectrum, scope, VU, spectrometer, goom)
- ✅ Sleep timer (minutes with fade, or end of item), LRC / embedded lyrics with sync and click-to-seek
- ✅ Local listening stats (30-day chart, top tracks/artists), private session, clear history
- ⏭ Protected (PIN) profiles — needs a design decision on what a profile restricts

## Phase 7 — Production (partly ✅)
- ✅ CI on Windows, macOS (arm64 + x64) and Linux producing MSI/NSIS, DMG, DEB/RPM/AppImage; release workflow
- ✅ CycloneDX SBOM (`scripts/sbom.mjs`, also built in CI)
- ✅ Randomised robustness tests of every untrusted-input parser (found and fixed a crash in playlist URL decoding)
- ✅ Accessibility audit (names, labels, headings, keyboard focus, WCAG AA contrast) — fixed failing tertiary text tokens
- ✅ Performance budgets measured: cold start 548–869 ms (< 1 s), idle CPU 0.02 % (< 1 %)
- ⏸ Code signing (needs certificates), curated LGPL plugin bundle + LGPL ffprobe (needs the licence decision below)
- ⏭ Embedded video on macOS/Linux (libVLC opens its own window there today); sandboxed decoder process

Open decisions: app license (permissive + curated plugins vs GPL-3.0 + full set); libVLC 4 migration timing;
whether to build/ship libiamf.
