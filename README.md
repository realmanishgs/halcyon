# Halcyon

A local-first universal media player: broad format coverage from a native media engine, an
Immersive Audio Lab for IAMF inspection and rendering (planned), and a restrained, minimal interface.

> Status: **Phase 2 foundation + first vertical slice working on Windows.** See
> [FEATURE_MATRIX.md](FEATURE_MATRIX.md) for exactly what is implemented and verified, and
> [ROADMAP.md](ROADMAP.md) for what comes next.

## What works today (verified)

- **Open anything:** files, folders (recursive), M3U/M3U8/PLS/XSPF playlists, network URLs
  (HTTP/HLS/DASH/RTSP/…), command-line arguments; drag & drop onto the window *or* onto the video.
  Dropping subtitle files onto a playing video attaches them.
- **Playback:** play/pause/stop, precise seek bar (hover preview, chapter ticks, drag anywhere),
  ±10 s/±3 s, frame step, speed 0.25–4×, volume up to 200 %, mute, chapters, restart after end.
- **Queue:** next/previous, auto-advance, repeat off/all/one, shuffle, reorder, resume position for long
  media, recent files.
- **Tracks & sync:** audio/subtitle track switching, external SRT/ASS/VTT, audio and subtitle delay.
- **Picture:** fullscreen (F / double-click, auto-hiding controls, hidden cursor), aspect ratio incl.
  stretch-to-window, deinterlace modes, PNG snapshots (Pictures/Halcyon).
- **Layout:** resizable sidebar and side panel, collapsible sidebar, works down to 420 × 300 like VLC.
- **Inspection:** container + every stream via out-of-process ffprobe, HDR10/HLG/Dolby/DTS/hi-res
  badges, IAMF stream groups, live decoder statistics (decoded/lost frames, bitrate).
- **Native right-click menu** over the video (speed, tracks, subtitles, aspect, deinterlace, snapshot, open…).
- **Library:** index folders (tags via ffprobe), albums with covers, artists, songs, videos, search, favourites.
- **Playlists:** manual and smart (rules), import/export M3U8/XSPF, save the queue.
- **Audio & picture:** 10-band EQ with presets, normalisation, ReplayGain, output device; brightness/contrast/
  hue/saturation/gamma, crop, rotate/flip; subtitle styling; visualisers; hardware-decoding switch.
- **Network:** stream URLs, browse SMB/FTP/SFTP/NFS shares, UPnP/mDNS discovery.
- **Convert:** FFmpeg presets (MP4/WebM/MKV remux/MP3/AAC/Opus/FLAC/WAV), trim, scale, loudness normalisation.
- **Immersive Audio Lab:** EBU R128 / BS.1770-4 loudness, LRA, true peak, clipping and silence detection,
  timeline, JSON/CSV reports, IAMF substream inspection and A/B. (IAMF mix rendering needs libiamf — not built.)
- **Experience:** sleep timer, synced lyrics (LRC/embedded), listening stats (local), private session, mini player,
  themes and accent colours.

Measured support for 78 formats (including 4K/8K, HDR, Dolby, DTS, hi-res audio and IAMF) is in
[FEATURE_MATRIX.md](FEATURE_MATRIX.md); the full machine-generated run is `test-media/STRESS_REPORT.md`.

### Keyboard (VLC-style)

`Space` play/pause · `F`/`F11`/double-click fullscreen · `Esc` leave · `←/→` ±10 s (`Shift` ±3 s) ·
`↑/↓`/wheel volume · `M` mute · `N/P` next/previous · `[ ] =` speed · `.` frame · `J/K` audio delay ·
`G/H` subtitle delay · `B/V` cycle audio/subtitle · `A` aspect · `L` repeat · `R` shuffle · `0–9` jump ·
`Shift+S` snapshot · `Q/I/T` queue/info/tune · `Ctrl+O` files · `Ctrl+Shift+O` folder · `Ctrl+N` stream ·
`Ctrl+L` library · `Ctrl+F` favourite.

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
cd src-tauri && cargo test --lib
```

Full stress suite (78 formats, damaged files, engine torture; ~5 min):

```bash
bash scripts/make-stress-media.sh
```

```bash
cd src-tauri && cargo test --lib stress -- --ignored --nocapture --test-threads=1
```

Other checks: `powershell -File scripts/perf-check.ps1` (start-up and idle CPU budgets),
`node scripts/sbom.mjs` (CycloneDX SBOM), `bash scripts/hw-decode-check.sh` (GPU decoding).
UI preview without the native shell: `npm run dev`, then open `http://localhost:1420/?mock` (fake data).

Put your own real-world files (Dolby Atmos, Dolby Vision, Eclipsa/IAMF exports, …) in
`test-media/corpus/` and the stress suite plays them too. See [TESTING.md](TESTING.md).

## Packages

| Platform | Built where | Output | Runtime requirement | Status |
|---|---|---|---|---|
| Windows x64 | locally (`npx tauri build`) or CI | `.msi`, NSIS `-setup.exe` | VLC 3.x 64-bit installed, or a `vlc/` folder next to `halcyon.exe` | built and smoke-tested locally |
| macOS (Apple silicon, Intel) | CI only (needs a Mac) | `.dmg`, `.app` | `/Applications/VLC.app` (3.x) | CI workflow written, not yet run; video opens in libVLC's own window (embedding not implemented) |
| Linux x64 | CI only | `.deb` (depends on `libvlc5`), `.rpm`, `.AppImage` | system libVLC 3 | CI workflow written, not yet run; video opens in libVLC's own window |

CI: `.github/workflows/release.yml` builds all three on GitHub-hosted runners when a `v*` tag is
pushed (or when run manually) and attaches the installers to a draft release. Installers are unsigned;
Windows SmartScreen and macOS Gatekeeper will warn until code signing is configured.

libVLC is **not bundled**: shipping VLC's full plugin set would bring GPL plugins into the package
(see [LICENSE_AUDIT.md](LICENSE_AUDIT.md)). That decision is still open.

## Documents

[ARCHITECTURE](ARCHITECTURE.md) · [ENGINE_DECISIONS](ENGINE_DECISIONS.md) · [FEATURE_MATRIX](FEATURE_MATRIX.md) ·
[LICENSE_AUDIT](LICENSE_AUDIT.md) · [REVERSE_ENGINEERING_NOTES](REVERSE_ENGINEERING_NOTES.md) ·
[DESIGN_SYSTEM](DESIGN_SYSTEM.md) · [ROADMAP](ROADMAP.md) · [TESTING](TESTING.md) · [THREAT_MODEL](THREAT_MODEL.md)

Halcyon is an independent project. It is not affiliated with or endorsed by VideoLAN, Spatial9 or AOMedia.
