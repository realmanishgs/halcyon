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
the dev machine). Consequence: HTML cannot be drawn *over* video. Popovers must stay outside the
video box (native `<select>` popups are separate windows, so they are fine). An overlay-capable path
(libVLC 4 `libvlc_video_set_output_callbacks` into a shared D3D11 texture) is tracked in the roadmap.

**State flow.** The engine is the single source of truth. The UI polls `status` at 4 Hz and refreshes
tracks/meta after open; no client-side clocks. (Event-driven updates via libVLC event manager are planned.)

**Threading.** Sync Tauri commands run on the main thread and are O(µs) libVLC getters/setters.
`open_media` is async; ffprobe runs on a blocking worker so the UI never stalls on slow media.

**Path handling.** `plain_path()` converts Win32 verbatim paths (`\\?\…`) which neither libVLC 3 nor
ffprobe accept. UTF-8 paths are passed to `libvlc_media_new_path` (VLC converts to UTF-16 internally);
verified with a CJK/diacritic filename test.
