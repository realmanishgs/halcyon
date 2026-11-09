# Architecture

## Stack

**Tauri 2 (Rust) + React/TypeScript UI + libVLC 3 loaded at runtime + ffprobe out-of-process.**
Rationale and alternatives: [ENGINE_DECISIONS.md](ENGINE_DECISIONS.md).

```
┌──────────────────────────── Tauri window (Win32 "Tauri Window") ─────────────────────────────┐
│  WRY_WEBVIEW (WebView2)                         HalcyonVideoSurface (child HWND, z-top)       │
│  ┌──────────── A. Presentation (React) ───────┐  └─ VLC video main → VLC video output (D3D11) │
│  │ Sidebar · Stage · Transport · Inspector    │        ▲ positioned from the web layout      │
│  │ engine.ts (typed invoke bridge)            │        │ (physical px via devicePixelRatio)  │
│  └──────────────┬─────────────────────────────┘        │                                     │
└─────────────────┼──────────────────────────────────────┼─────────────────────────────────────┘
                  │ Tauri IPC (commands)                  │
┌─────────────────▼──── B. Application layer (src-tauri/src/lib.rs) ───────────────────────────┐
│ AppState { engine: Mutex<Box<dyn MediaEngine>>, surface HWND }  · commands · startup file     │
└───────┬───────────────────────────────────────────────┬──────────────────────────────────────┘
        │ C. MediaEngine trait (engine/mod.rs)           │ probe.rs
┌───────▼──────── D. Adapters ───────────┐   ┌──────────▼─────────────────────────┐
│ engine/vlc.rs  libVLC 3.x via           │   │ ffprobe child process → JSON       │
│ libloading (no import lib, no headers)  │   │ (crash-isolated parsing)           │
│ planned: ffmpeg/libiamf "lab" pipeline  │   └────────────────────────────────────┘
└─────────────────────────────────────────┘
E. Storage: db.rs — SQLite (rusqlite, bundled) in the app data dir: settings, library, playlists, history.
F. Out-of-process tools: library.rs (index via ffprobe, covers via ffmpeg), lab.rs (decode → BS.1770
   analyser), convert.rs (ffmpeg jobs). Browsing/discovery use a second, headless libVLC instance.
G. Extensions (planned): versioned Rust traits + capability-scoped Tauri permissions.
```

## Layer responsibilities

| Layer | Code | Owns | Must not |
|---|---|---|---|
| Presentation | `src/` | Layout, input, accessibility, rendering of engine state | Invent values; decode media |
| Application | `src-tauri/src/lib.rs` | Command surface, state ownership, threading | Know backend specifics |
| Engine abstraction | `src-tauri/src/engine/mod.rs` | `MediaEngine` trait + data types | — |
| Adapters | `engine/vlc.rs` | FFI, resource lifetime, error mapping | Leak raw pointers upward |
| Inspection | `probe.rs` | Container/stream facts | Run in-process on untrusted input |
| Storage | `db.rs` | Schema, queries, smart-rule → SQL translation (whitelisted) | Interpolate user values into SQL |
| Library | `library.rs` | Folder walk, tag extraction, covers, lyrics, playlist export | Block the UI thread |
| Audio Lab | `lab.rs` | Streaming loudness/peak/clip/silence analysis, reports | Hold whole files in memory |
| Conversion | `convert.rs` | FFmpeg presets, progress, cancel, cleanup | Overwrite the input |
| Pages | `src/Library.tsx`, `Lab.tsx`, `Convert.tsx`, `Settings.tsx`, `Panels.tsx` | Views over the commands above | — |

## Key mechanics

**Runtime loading of libVLC.** `VlcEngine::load()` searches `HALCYON_LIBVLC_DIR`, `<exe>/vlc`, then
`%ProgramFiles%\VideoLAN\VLC`, loads `libvlc.dll` with `LOAD_WITH_ALTERED_SEARCH_PATH` so
`libvlccore.dll` and `plugins/` resolve beside it. Every symbol is resolved up-front; a missing
symbol is a load error shown in the UI, not a crash. FFI signatures were transcribed from
`include/vlc/libvlc_*.h` on VLC branch `3.0.x` (commit `c128d1f`).

**Video embedding.** WebView2 cannot host a native decoder surface, so a sibling child HWND
(`surface.rs`) is created on the UI thread at startup and handed to `libvlc_media_player_set_hwnd`.
The web layout reserves a box (`.video-box`); a per-frame check compares its rect and, only on change,
sends physical pixels (`rect × devicePixelRatio`) to `set_video_rect`. `devicePixelRatio` is used instead
of the window scale factor because WebView2 also applies Windows text scaling (measured: 1.5 vs ~1.59 on
the dev machine). Consequence: HTML cannot be drawn *over* video. Anything that must appear above the
picture uses a native window: the right-click menu is a native popup menu (Tauri `Menu.popup`), and
`<select>` popups are separate windows. A window region does **not** work as a cut-out: libVLC's
Direct3D11 flip-model swapchain ignores the parent's clip region (tested), so in fullscreen the surface is
shortened while the control bar is visible and restored when it auto-hides. An overlay-capable path
(libVLC 4 `libvlc_video_set_output_callbacks` into a shared D3D11 texture) is tracked in the roadmap.

**Input over the video.** libVLC's own windows have mouse/key input disabled, so the system routes pointer
messages to `HalcyonVideoSurface`, whose window procedure forwards them as a `surface` event:
double-click (fullscreen), right-click (native context menu), wheel (volume), throttled moves (reveal
auto-hidden controls), and `WM_DROPFILES` (the window is created with `WS_EX_ACCEPTFILES`). Drops over
the rest of the window arrive through Tauri's webview drag-drop event. Both feed `handleDropped`:
subtitle files are attached to the playing video (`libvlc_media_player_add_slave`); everything else is
expanded (folders → media, M3U/M3U8/PLS/XSPF → entries) into the queue (`sources.rs`).

**Playhead model** (`SeekBar.tsx`). The engine is sampled at 4 Hz; between samples the bar interpolates
with the real playback rate but never runs more than 1 s ahead of the last sample. After a seek the
target is held until the engine reports a time within 1.5 s of it (or 2.5 s pass), so the thumb never
snaps back to the pre-seek position. Pointer capture keeps a drag alive outside the bar; release
anywhere commits. Ended shows the full length; stopped shows zero. Length falls back to the probed
duration because libVLC reports 0 before the input starts and after it ends.

**End-of-media and restart.** libVLC 3 ignores `play()` once a media has ended, errored or stopped, and
`set_time()` needs a running input. `VlcEngine::play` therefore stops first in those states, and
`seek_ms` restarts the input (waiting ≤ 1.5 s for Playing) before seeking. Both are covered by
`play_and_seek_after_end`.

**State flow.** The engine is the single source of truth. libVLC's event manager pushes state changes
(opening, playing, paused, stopped, ended, error, length, tracks, video output) as an `engine` event; the
callback runs on a libVLC thread and only forwards the event, never calling back into libVLC. The UI then
polls `status` once, and keeps a 4 Hz poll only while playing (1 Hz otherwise) for the playhead. Settings
live in SQLite behind a synchronous in-memory cache in the UI (`store`), written back debounced.

**Engine options that libVLC 3 reads only at start-up** (visualisers, rotation/flip, normalisation,
ReplayGain, subtitle style) are applied by creating a new instance and player, re-attaching events,
re-applying volume, equalizer and picture adjustment, and reopening the media at the same position.
**Hardware decoding is a media option**, not an instance option: measured, the libVLC 3 player object
shadows the instance-level `avcodec-hw` value, so `--avcodec-hw=none` passed to `libvlc_new` is ignored
while `:avcodec-hw=none` on the media is honoured. Rotation forces software decoding because the
`transform` filter cannot take Direct3D11 frames, and switches the video output to OpenGL (`glwin32`):
measured with the real outputs, Direct3D11 stalls after the first frame with any transform (Direct3D9 and
GDI with some), while OpenGL displays all five transforms.

**Loudness analysis** streams 48 kHz float PCM from an `ffmpeg` child process through K-weighting
biquads into 100 ms sub-blocks; momentary (4 blocks), short-term (30 blocks), gated integrated loudness
and LRA (EBU Tech 3342) are computed from those; true peak uses a 4-phase, 96-tap Kaiser-windowed sinc
interpolator. Memory is constant in file length.

**Threading.** Sync Tauri commands run on the main thread and are O(µs) libVLC getters/setters.
`open_media` and `expand_sources` are async; ffprobe and folder scans run on blocking workers.

**Path handling.** `plain_path()` converts Win32 verbatim paths (`\?\…`) which neither libVLC 3 nor
ffprobe accept. UTF-8 paths are passed to `libvlc_media_new_path` (VLC converts to UTF-16 internally);
verified with CJK/emoji/`#%[]&'{}` names and a 300+ character path. URLs go through
`libvlc_media_new_location`.
