# Halcyon

A local-first universal media player: broad format coverage from a native media engine, an
Immersive Audio Lab for IAMF inspection and rendering (planned), and a restrained, minimal interface.

> Status: **Phase 2 foundation + first vertical slice working on Windows.** See
> [FEATURE_MATRIX.md](FEATURE_MATRIX.md) for exactly what is implemented and verified, and
> [ROADMAP.md](ROADMAP.md) for what comes next.

## What works today (verified)

- Open a local file (dialog, drag & drop, or command-line argument / "Open with").
- Identify container and every stream (codec, profile, resolution, frame rate, pixel format,
  HDR transfer, sample rate, channels, layout, bitrate) via an **out-of-process** ffprobe.
- Native playback through **libVLC 3.0.x**, rendered into a native child window (Direct3D11) inside the app.
- Play / pause / stop / seek / ±10 s / frame step / speed / volume / mute.
- Audio-track and subtitle-track selection (embedded tracks).
- Real tag metadata (title, artist, album, genre, date, artwork when the file carries it).
- Keyboard: `Space` play/pause · `←/→` ±5 s (`Shift` ±30 s) · `↑/↓` volume · `.` next frame · `S` stop · `I` inspector · `Ctrl+O` open.

## Requirements (development, Windows 10/11 x64)

| Tool | Tested version | Why |
|---|---|---|
| VLC (64-bit) | 3.0.24 | Provides `libvlc.dll` + plugins, loaded at runtime |
| FFmpeg (ffprobe on `PATH`) | 9.0.1 | Media inspection |
| Rust (MSVC toolchain) | 1.98 | Backend |
| Node.js | 26 | Frontend tooling |
| WebView2 runtime | system | Tauri UI |

Overrides: `HALCYON_LIBVLC_DIR` (folder containing `libvlc.dll`), `HALCYON_FFPROBE` (path to ffprobe).

## Run

```bash
npm install
```

```bash
npm run tauri dev
```

## Test

```bash
bash scripts/make-test-media.sh
```

```bash
cd src-tauri && cargo test
```

The engine tests play real generated media headlessly (VLC dummy audio/video outputs). See [TESTING.md](TESTING.md).

## Documents

[ARCHITECTURE](ARCHITECTURE.md) · [ENGINE_DECISIONS](ENGINE_DECISIONS.md) · [FEATURE_MATRIX](FEATURE_MATRIX.md) ·
[LICENSE_AUDIT](LICENSE_AUDIT.md) · [REVERSE_ENGINEERING_NOTES](REVERSE_ENGINEERING_NOTES.md) ·
[DESIGN_SYSTEM](DESIGN_SYSTEM.md) · [ROADMAP](ROADMAP.md) · [TESTING](TESTING.md) · [THREAT_MODEL](THREAT_MODEL.md)

Halcyon is an independent project. It is not affiliated with or endorsed by VideoLAN, Spatial9 or AOMedia.
