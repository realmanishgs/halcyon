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
E. Storage (planned, Phase 2b): SQLite via rusqlite — library, playlists, resume, settings.
F. Extensions (planned): versioned Rust traits + capability-scoped Tauri permissions.
```

## Layer responsibilities

| Layer | Code | Owns | Must not |
|---|---|---|---|
| Presentation | `src/` | Layout, input, accessibility, rendering of engine state | Invent values; decode media |
| Application | `src-tauri/src/lib.rs` | Command surface, state ownership, threading | Know backend specifics |
| Engine abstraction | `src-tauri/src/engine/mod.rs` | `MediaEngine` trait + data types | — |
| Adapters | `engine/vlc.rs` | FFI, resource lifetime, error mapping | Leak raw pointers upward |
| Inspection | `probe.rs` | Container/stream facts | Run in-process on untrusted input |

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

**State flow.** The engine is the single source of truth. The UI polls `status` at 4 Hz (and immediately
after every user action) and refreshes tracks/meta after open; decoder statistics are polled at 1 Hz only
while the Info panel is open. Queue, repeat/shuffle, resume positions and recent files live in the UI
(`localStorage`, wrapped so a blocked store never breaks playback).

**Threading.** Sync Tauri commands run on the main thread and are O(µs) libVLC getters/setters.
`open_media` and `expand_sources` are async; ffprobe and folder scans run on blocking workers.

**Path handling.** `plain_path()` converts Win32 verbatim paths (`\?\…`) which neither libVLC 3 nor
ffprobe accept. UTF-8 paths are passed to `libvlc_media_new_path` (VLC converts to UTF-16 internally);
verified with CJK/emoji/`#%[]&'{}` names and a 300+ character path. URLs go through
`libvlc_media_new_location`.
