# Threat model (v0.1)

## Assets
User media files and paths, listening/viewing history (future library DB), device, user trust (incl. minors).

## Trust boundaries
1. **Untrusted media bytes** → demuxers/decoders (libVLC in-process; ffprobe out-of-process).
2. **WebView UI** → Rust commands (Tauri IPC). The UI is local, bundled, CSP-restricted.
3. **Network** (future streams, metadata/lyrics providers).
4. **Plugins/extensions** (future).

## Threats & mitigations

| Threat | Mitigation now | Planned |
|---|---|---|
| Malformed file exploits a parser | Inspection runs in a separate ffprobe process; garbage/truncated files tested | Run libVLC playback in a sandboxed helper process; fuzz probe/IAMF parsers; keep VLC/FFmpeg patched (track CVEs) |
| DLL hijacking when loading libVLC | Loaded by absolute path from known dirs; `LOAD_WITH_ALTERED_SEARCH_PATH` scoped to that dir | Verify Authenticode signature of bundled libVLC; ship private copy |
| Webview compromise reaching the filesystem | Capability file grants only `core:default` + `dialog:allow-open`; asset protocol limited to `$CONFIG/vlc/art/**`; CSP `default-src 'self'` | Per-command path validation; no generic fs plugin |
| Silent data egress / telemetry | No telemetry, no accounts, no network calls in app code; libVLC started with `--no-metadata-network-access` (no online art/meta lookups) | CI test asserting zero outbound connections for local playback |
| Network streams (MITM, SSRF to LAN) | n/a yet | HTTPS by default, warn on HTTP, no auto-follow to private ranges from playlists without consent |
| Exposure of private library to others | n/a yet (no sharing features) | No feature exports library contents without explicit consent; protected profiles; local-only stats |
| Minors | No accounts, ads, social features, or tracking | Optional parental profile with PIN; safe defaults; legal review per region before claiming compliance (e.g., COPPA, GDPR-K, AADC) |

## Known gaps (honest)
libVLC decodes in-process today; a decoder crash ends the app.
libVLC network metadata lookups are disabled by flag, but no egress test exists yet.
