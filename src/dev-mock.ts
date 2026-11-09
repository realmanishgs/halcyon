// Development-only stand-in for the Rust backend, so the UI can be reviewed
// and accessibility-audited in a plain browser (`npm run dev`, then open
// http://localhost:1420/?mock). Loaded by main.tsx only in dev builds when
// `?mock` is present; never part of a production bundle. Data is fictional.

const rows = Array.from({ length: 8 }, (_, i) => ({
  path: `C:\\Music\\Album ${i % 3}\\${i + 1} Song ${i + 1}.flac`, kind: i === 7 ? "video" : "audio",
  title: `Song ${i + 1}`, artist: ["Ana Lima", "Bo Chen", "Cy Okafor"][i % 3], album: `Album ${i % 3}`, album_artist: null,
  genre: "Jazz", year: 2019, track: i + 1, disc: 1, duration_ms: 180000 + i * 7000, codec: "flac",
  width: i === 7 ? 1920 : null, height: i === 7 ? 1080 : null, size: 30000000, favorite: i % 4 === 0, play_count: i,
  last_played: null, lyrics: null,
}));
const timeline = Array.from({ length: 1200 }, (_, i) => [-23 + 6 * Math.sin(i / 40), -23 + 3 * Math.sin(i / 120), -6 + 3 * Math.sin(i / 30)]);
const analysis = {
  path: "C:\\Music\\mix.iamf", stream: 0, channels: 2, layout: "stereo", duration_ms: 120000, integrated_lufs: -18.4,
  loudness_range_lu: 7.2, max_momentary_lufs: -11.2, max_short_term_lufs: -14.9, sample_peak_dbfs: -1.3, true_peak_dbtp: -0.6,
  channel_true_peak_dbtp: [-0.6, -0.9], channel_rms_dbfs: [-21.2, -21.5], channel_dc_offset: [0.0001, -0.0002],
  clip_events: [{ time_ms: 40500, channel: 0, samples: 4 }], clipped_samples: 4, silences: [[100000, 104000]], timeline,
};
const day = Math.floor(Date.now() / 86400000);
const R: Record<string, unknown> = {
  get_settings: {}, startup_files: [], expand_sources: [],
  diagnostics: { engine: { name: "libVLC", version: "3.0.24 (mock)", location: "C:\\Program Files\\VideoLAN\\VLC" }, engine_error: null, ffprobe: "ffprobe.exe" },
  engine_config: { hw_decode: true, normalize: false, replay_gain: false, visualizer: null, transform: "", sub_scale: 100, sub_color: 0xffffff, sub_outline: 4, sub_background: 0, sub_margin: 0 },
  equalizer_info: { frequencies: [60, 170, 310, 600, 1000, 3000, 6000, 12000, 14000, 16000], presets: [["Flat", { preamp: 12, bands: Array(10).fill(0) }], ["Rock", { preamp: 5, bands: [8, 5, -5, -8, -3, 4, 8, 11, 11, 11] }]] },
  audio_devices: [{ id: "", name: "Default" }, { id: "{dev1}", name: "Speakers" }],
  library_roots: ["C:\\Music"], library_list: rows,
  library_albums: [0, 1, 2].map((i) => ({ album: `Album ${i}`, artist: ["Ana Lima", "Bo Chen", "Cy Okafor"][i], year: 2019, tracks: 3, duration_ms: 600000, first_path: rows[i].path })),
  library_artists: [0, 1, 2].map((i) => ({ artist: ["Ana Lima", "Bo Chen", "Cy Okafor"][i], albums: 1, tracks: 3 })),
  library_album_tracks: rows.slice(0, 3), library_artist_tracks: rows.slice(0, 3), cover: null, media_row: rows[0], lyrics: null,
  playlists: [{ id: 1, name: "Road trip", rules: null, count: 3 }, { id: 2, name: "Favourites", rules: { match: "all", rules: [{ field: "favorite", op: "is", value: "1" }], limit: null, sort: "title" }, count: 2 }],
  playlist_items: rows.slice(0, 3).map((r) => r.path),
  listening_stats: { total_ms: 7380000, plays: 41, top_tracks: rows.slice(0, 3).map((r, i) => [r.path, 9 - i]), top_artists: [["Ana Lima", 14], ["Bo Chen", 9]], per_day: Array.from({ length: 12 }, (_, i) => [day - i * 2, 600000 + i * 90000]) },
  convert_presets: [{ id: "mp4-h264", label: "MP4 · H.264 + AAC", ext: "mp4", audio_only: false }, { id: "flac", label: "FLAC · lossless", ext: "flac", audio_only: true }],
  engine_plugins: [["codec", ["avcodec", "dav1d", "libass"]], ["access", ["smb", "sftp"]], ["services_discovery", ["upnp", "microdns"]], ["audio_output", ["mmdevice"]], ["visualization", ["goom", "visual"]]],
  discovery_services: [["upnp", "Universal Plug'n'Play"], ["microdns", "mDNS Network Discovery"]],
  probe_file: {
    streams: [0, 1].map((i) => ({ index: i, codec_type: "audio", codec_name: "opus", channels: 2, channel_layout: "stereo", sample_rate: "48000", tags: {} })),
    format: { format_long_name: "IAMF (mock)", tags: {} }, chapters: [], stream_groups: [{ type: "IAMF Audio Element", id: 1, nb_streams: 2 }],
  },
  status: { state: "idle", time_ms: 0, length_ms: 0, volume: 100, muted: false, rate: 1, seekable: false, has_video: false, chapter: -1, chapter_count: -1, audio_delay_ms: 0, subtitle_delay_ms: 0 },
  tracks: { audio: [], subtitle: [], current_audio: -1, current_subtitle: -1 },
  meta: { title: null, artist: null, album: null, genre: null, date: null, artwork_url: null },
};

export function installMock() {
  let seq = 0;
  const callbacks: Record<number, (e: unknown) => void> = {};
  const listeners: Record<string, number[]> = {};
  const emit = (event: string, payload: unknown) => (listeners[event] ?? []).forEach((h) => callbacks[h]?.({ event, id: 0, payload }));
  const w = window as unknown as Record<string, unknown>;
  w.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
    transformCallback: (f: (e: unknown) => void) => { const id = ++seq; callbacks[id] = f; return id; },
    unregisterCallback: (id: number) => { delete callbacks[id]; },
    convertFileSrc: (p: string) => p,
    invoke: async (cmd: string, args: Record<string, unknown>) => {
      if (cmd === "plugin:event|listen") { (listeners[args.event as string] ??= []).push(args.handler as number); return args.handler; }
      if (cmd === "lab_analyze") { setTimeout(() => emit("lab-progress", { id: 7, progress: 1, result: analysis, error: null }), 300); return 7; }
      if (cmd.startsWith("plugin:window|is_fullscreen")) return false;
      if (cmd === "plugin:dialog|open") return (args.options as { directory?: boolean })?.directory ? "C:\\Music" : "C:\\Music\\mix.iamf";
      return cmd in R ? R[cmd] : null;
    },
  };
  w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
}
