# Engine decisions

Date: 2026-11-08 · Evidence: installed binaries on the dev machine, VLC `3.0.x` headers, fetched project pages.

## Decision 1 — Application shell: Tauri 2 + React/TypeScript + Rust

| Option | Native playback | UI flexibility | Footprint | Packaging | Verdict |
|---|---|---|---|---|---|
| **Tauri 2** (2.12.x) | Via native child surface + Rust FFI | High (web) | Small (system WebView2) | MSI/NSIS, dmg, AppImage/deb | **Chosen** |
| Qt 6 + QML | Excellent (QML video items) | High | Medium-large | Mature | Strong alternative; Qt not installed, LGPL/commercial terms, C++ velocity |
| Electron + native addon | Same embedding problem as Tauri | High | Large (Chromium per app) | Mature | Rejected on memory/idle CPU |
| Platform-native per OS | Best | Three codebases | Small | Per-OS | Rejected on maintainability |

Known cost of Tauri: no HTML over video (see ARCHITECTURE). Mitigation path exists (Decision 4).

## Decision 2 — Playback engine: libVLC 3.0.x, loaded at runtime

| Candidate | Format coverage | Sync / A-V / subtitles | HW decode (Windows) | License of core | Embedding | Notes |
|---|---|---|---|---|---|---|
| **libVLC 3.0.24** | Very broad (363 plugins on this install; uses FFmpeg 8.1 internally per NEWS) | Complete (libass, DVD/Blu-ray subs, CC) | d3d11va / dxva2 plugins present | LGPL-2.1+ (libvlc, libvlccore) | `set_hwnd` | Stable API, shipped widely. **Chosen.** |
| libVLC 4.0 (unreleased) | Same + better track API | Same | Same | LGPL | Adds output callbacks for compositing | Revisit when released |
| libmpv | Broad (FFmpeg) | Excellent | d3d11va | LGPL build possible | `wid` / render API | Viable second adapter |
| FFmpeg libs directly | Broadest decoders | **We'd write** clocking, A/V sync, subtitle render, output | Via hwaccel API | LGPL or GPL by config | n/a | Too much reinvention for playback; ideal for analysis |
| GStreamer | Broad with plugins | Good | d3d11 | LGPL core, plugins vary | Overlay API | Heavy Windows deployment |

Runtime loading (not linking) keeps libVLC a separately replaceable LGPL component and lets the app
start (with a clear error) when it is missing.

## Decision 3 — Inspection: ffprobe out-of-process (now), libavformat in-process later only if sandboxed

Out-of-process parsing isolates demuxer crashes on untrusted files. The dev build currently uses a
user-installed GPL FFmpeg build (not distributed). For distribution, bundle an **LGPL** ffprobe build.

## Decision 4 — IAMF (Immersive Audio Lab): separate pipeline, not libVLC

Findings:
- VLC 3.0.24 install: **no IAMF plugin** (checked `plugins/codec`, `plugins/demux`).
- FFmpeg 9.0.1: has an `iamf` **demuxer** (`ffprobe -demuxers`), no IAMF decoder/renderer listed.
  Substreams (Opus/AAC/FLAC/LPCM) are decodable by FFmpeg, but mix-presentation rendering is not provided.
- AOM `libiamf`: README calls it the reference decoder for IAMF v2.0.0; license BSD-3-Clause-Clear with a
  PATENTS file. **Binaural rendering support was not stated in the README — unverified.**

Plan: a Rust `IamfEngine` adapter wrapping `libiamf` (decode + render to stereo / channel layouts),
FFmpeg (or `iamf-tools` parsing) for OBU inspection, and our own BS.1770-4 meter. Binaural requires an
HRTF renderer — evaluate libiamf's capabilities first; otherwise a SOFA-based renderer with an
appropriately licensed HRTF set. Rendering to both stereo and binaural concurrently enables gapless A/B
(switch the output bus, not the decoder). Nothing is claimed until verified against AOM conformance vectors.
