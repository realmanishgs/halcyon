# Threat model (v0.1)

## Assets
User media files and paths, the local SQLite database (library, playlists, listening history, resume points), device, user trust (incl. minors).

## Trust boundaries
1. **Untrusted media bytes** → demuxers/decoders (libVLC in-process; ffprobe out-of-process).
2. **WebView UI** → Rust commands (Tauri IPC). The UI is local, bundled, CSP-restricted.
3. **Network**: streams and shares the user opens; LAN discovery (UPnP/mDNS). No metadata or lyrics providers.
5. **Untrusted text files**: playlists, LRC lyrics, tags, and smart-playlist rule values typed by the user.
6. **External tools**: ffprobe/ffmpeg run as child processes with argument arrays (no shell).
4. **Plugins/extensions** (future).

## Threats & mitigations

| Threat | Mitigation now | Planned |
|---|---|---|
| Malformed file exploits a parser | Inspection, library indexing, analysis and conversion run in separate FFmpeg processes; damaged files tested; playlist/LRC/tag/JSON parsers fuzzed (`src/fuzz.rs`, found and fixed a panic on `%` + multi-byte text) | Run libVLC playback in a sandboxed helper process; fuzz probe/IAMF parsers; keep VLC/FFmpeg patched (track CVEs) |
| DLL hijacking when loading libVLC | Loaded by absolute path from known dirs; `LOAD_WITH_ALTERED_SEARCH_PATH` scoped to that dir | Verify Authenticode signature of bundled libVLC; ship private copy |
| Webview compromise reaching the filesystem | Capabilities: core, open/save dialogs, menu, fullscreen, window size/always-on-top only; asset protocol limited to `$CONFIG/vlc/art/**`; cover art is returned as data URLs (≤ 4 MB) instead of widening file access; CSP `default-src 'self'` | Per-command path validation; no generic fs plugin |
| Silent data egress / telemetry | No telemetry, no accounts, no network calls in app code; libVLC started with `--no-metadata-network-access` (no online art/meta lookups) | CI test asserting zero outbound connections for local playback |
| Network streams (MITM, SSRF to LAN) | Only locations the user types or picks are opened; browsing runs on a separate libVLC instance with timeouts | HTTPS by default, warn on HTTP, no auto-follow to private ranges from playlists without consent |
| Exposure of private library to others | No sharing or sync; history stays in the local DB; private session records nothing; history can be cleared | No feature exports library contents without explicit consent; protected profiles; local-only stats |
| Minors | No accounts, ads, social features, or tracking | Optional parental profile with PIN; safe defaults; legal review per region before claiming compliance (e.g., COPPA, GDPR-K, AADC) |

| SQL injection via smart-playlist rules | Field names come from a whitelist mapped to fixed SQL expressions; operators are whitelisted per type; values are bound parameters with LIKE wildcards escaped; fuzzed |
| Conversion overwriting user files | Output chosen through the OS save dialog; refuses to write over the input; partial output deleted on failure/cancel |

## Known gaps (honest)
libVLC decodes in-process today; a decoder crash ends the app.
libVLC network metadata lookups are disabled by flag, but no egress test exists yet.
