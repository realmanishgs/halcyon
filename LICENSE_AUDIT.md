# License & distribution audit

Status: **pre-distribution.** Nothing third-party is bundled yet; the dev build uses the user's installed
VLC and FFmpeg. This is an engineering audit, not legal advice; a legal review is required before release.

## Direct dependencies

| Component | Version | License | How used | Obligation |
|---|---|---|---|---|
| Tauri, tauri-build, tauri-plugin-dialog | 2.12 / 2.x | MIT OR Apache-2.0 | Linked | Notices |
| serde, serde_json | 1.x | MIT OR Apache-2.0 | Linked | Notices |
| libloading | 0.9 | ISC | Linked | Notice |
| rusqlite (+ bundled SQLite) | 0.37 / 3.x | MIT / SQLite is public domain | Linked | Notice |
| windows-sys | 0.61 | MIT OR Apache-2.0 | Linked | Notices |
| React, react-dom | 19.3.1 | MIT | Bundled JS | Notice |
| @tauri-apps/api, plugin-dialog | 2.12.4 / 2.8.4 | MIT OR Apache-2.0 | Bundled JS | Notice |
| Vite, TypeScript | 8.3.6 / 5.9 | MIT / Apache-2.0 | Build only | — |
| Fonts | system (Segoe UI, Cascadia) | OS-provided, not redistributed | — | — |
| Icons | drawn for this project | project license | — | — |

## Media engine (runtime-loaded, not yet redistributed)

| Component | License | Finding |
|---|---|---|
| libvlc.dll, libvlccore.dll | LGPL-2.1-or-later | Dynamic runtime loading satisfies relinking intent; must ship source offer/notice if bundled |
| VLC plugins (363 on this install) | **Mixed** | Installed set includes **GPL** plugins: `libx264_plugin`, `libx26410b_plugin`, `libx265_plugin` (encoders), `libfaad_plugin` (FAAD2, GPL). Also `libdvdcss` is not present. Bundling the stock plugin set ⇒ GPL obligations for the distribution. |
| FFmpeg 9.0.1 "full_build" (gyan.dev) | **GPL** (`--enable-gpl`, x264/x265) | Used only as an external `ffprobe` executable on the dev machine; must **not** be bundled. Ship an LGPL build instead. |

**Distribution decision (proposed):** ship a curated libVLC plugin set excluding GPL-only plugins
(encoders, FAAD — AAC decoding remains available through the LGPL avcodec plugin), plus an LGPL ffprobe.
Keep the app itself under a permissive license. Alternative: license Halcyon under GPL-3.0 and ship the full set.
Choose before Phase 7; record the plugin manifest with per-plugin licenses in the SBOM.

## Full dependency inventory

`node scripts/sbom.mjs` writes a CycloneDX 1.5 SBOM of every crate and npm package (482 components on
2026-11-09, every crate with a declared licence). Licences seen: MIT, Apache-2.0, dual MIT/Apache, Unicode-3.0,
Zlib, BSD-3-Clause, ISC, Unlicense and **MPL-2.0** (17 crates, file-level copyleft: modified MPL files must stay
MPL; using them unmodified is fine). The SBOM covers all target platforms, so it lists macOS/Linux-only crates too.

## Conversion

The converter runs the user's own `ffmpeg` executable; nothing from FFmpeg is linked or shipped. Encoder
availability (x264/x265 are GPL) is the user's FFmpeg build's matter.

## Patents / codec licensing (to review per distribution region)

HEVC (multiple pools), AAC, E-AC-3, DTS, TrueHD, VC-1 may carry patent licensing for distributors of
decoders in some regions. AV1, VP8/VP9, Opus, FLAC, Vorbis, Theora are royalty-free by their
licensors' terms. Halcyon will not bypass DRM or ship libdvdcss.

## IAMF components (planned)

`libiamf`: BSD-3-Clause-Clear + PATENTS file (per GitHub README sidebar, 2026-11-08). `iamf-tools`: license to verify.
HRTF data sets (SOFA): license to verify per file; the `hrtfs/` file shipped with VLC is not to be reused without checking its terms.

## Trademarks

No VLC/VideoLAN, Spatial9/Eclipsa, or AOMedia marks or assets are used. "VLC" appears only in factual technical references.
