# Research & reverse-engineering notes

## Tooling availability

| Item | Observed | How |
|---|---|---|
| REA MCP (Reverse Engineer Anything) | **Not installed.** No REA tooling installed on the development machine. | Config inspection, 2026-11-08 |
| Installation (not run) | `npx rea-agents@latest setup` — modifies agent config; to be reviewed and approved by the user before running | Official README instruction |
| Consequence | All findings below come from source code, installed binaries, and public pages. No binary/runtime RE of third-party apps was performed. | — |

## VLC (source-first; no reverse engineering needed)

| Finding | Evidence | Confidence |
|---|---|---|
| Public API split: `include/vlc/` = libVLC (stable app API); `src/` = libvlccore; `modules/` = plugins | Sparse clone of `videolan/vlc` branch `3.0.x` @ `c128d1f` (include/vlc, lib, doc); directory layout | High |
| Exact FFI signatures used by `engine/vlc.rs` (34 functions, `libvlc_track_description_t`, `libvlc_state_t`, `libvlc_meta_t` ordinals) | `libvlc.h`, `libvlc_media.h`, `libvlc_media_player.h` | High (and runtime-verified by tests) |
| Plugin categories present on target install: access (http/https/ftp/sftp/smb/nfs/rtp/udp/srt/dshow/bluray/dvdnav…), demux (mkv/mp4/ts/ps/avi/asf/ogg/adaptive(HLS/DASH)…), codec (avcodec, dav1d, d3d11va, dxva2, libass, opus, flac…), vout (direct3d11/9, gl), aout (wasapi, mmdevice, directsound) | `Program Files/VideoLAN/VLC/plugins/*` listing (363 DLLs) | High |
| Engine vs UI: playback, demux, decode, filters, sout, playlists-as-input, discovery all live in libvlccore + modules; the Qt/skins UIs (`modules/gui`) only drive libvlccore. Everything we need is reachable via libVLC except some advanced filter chains (exposed as `--option` strings / media options) | Header surface + module layout | Medium-high |
| No IAMF support in VLC 3.0.24 | No iamf module in plugins/ | High |

Capability map (reproduce = via libVLC · integrate = other component · implement = ours):

| Area | Approach |
|---|---|
| Demux/decode/A-V sync/subtitles/HW decode/network access/sout | Reproduce via libVLC |
| Track/meta/chapter APIs | libVLC (3.x track-description API; migrate to 4.x track API later) |
| Library, playlists UI, resume, history, smart playlists | Implement (SQLite) |
| Media inspection | Integrate ffprobe (out-of-process) |
| IAMF decode/render, BS.1770 meters, QC reports | Integrate libiamf + implement analysis |
| Overlay-capable video compositing | libVLC 4 output callbacks (future) |

## Eclipsa Audio Player (public page only)

| Observed (stated on s9.audio/eclipsaplayer) | Method | Confidence | Our implementation |
|---|---|---|---|
| Browser app; `.iamf` and MP4/M4A/MOV with IAMF track | Page text via web fetch | High that it is *claimed*; not independently tested | libiamf + MP4 demux |
| Codecs Opus, AAC-LC, FLAC, LPCM; channel layouts up to 9.1.6; ambisonics up to 3rd order | Same | Claimed | Validate with AOM conformance vectors |
| Stereo fold-down vs binaural headphone render, A/B during playback without stopping | Same | Claimed; HRTF/method **not described** | Dual render bus, switch at output |
| Per-channel live meters (speaker + ambisonic channels) | Same | Claimed | Lock-free ring buffer → UI meters |
| Inspector: profile, codec, audio elements, mix presentations | Same | Claimed | OBU parser |
| QC: BS.1770 integrated loudness, true peak, clipping, silent channels; downloadable report (format **unstated**) | Same | Claimed | BS.1770-4 K-weighting + gating, 4× oversampled true peak; JSON/CSV export |
| "Files never leave the device" | Same | Claim only; not verified (no network inspection done) | We verify our own claim with a no-network test (THREAT_MODEL) |
| Says it is not a reference renderer | Same | High | We will state the same caveat |

Unknowns: UI structure, meter ballistics, report schema — would require authorized runtime observation
(browser devtools on the public web app is permissible; deferred to Phase 5, with REA if installed).
No Spatial9 code, assets, or branding are used.

## Design reference (mcpmarket "Design Minimal" skill)

Stated principles: restrained palette, strong type hierarchy, high contrast, information density, no
shadows/rounded decoration. Its token values are not published on the page (source repo not fetched).
Its constraints (static single-file HTML, desktop-only, no JS frameworks) are deliberately **not** adopted
for an interactive app. The `design-minimal` skill is not installed in this environment.
