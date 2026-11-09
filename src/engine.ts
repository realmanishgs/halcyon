// Typed bridge to the Rust application layer. Every value shown in the UI
// comes from these calls — nothing is synthesised on the frontend.
import { invoke } from "@tauri-apps/api/core";

export type PlaybackState =
  | "idle" | "opening" | "buffering" | "playing" | "paused" | "stopped" | "ended" | "error";

export interface PlaybackStatus {
  state: PlaybackState;
  time_ms: number;
  length_ms: number;
  volume: number;
  muted: boolean;
  rate: number;
  seekable: boolean;
  has_video: boolean;
  chapter: number;
  chapter_count: number;
  audio_delay_ms: number;
  subtitle_delay_ms: number;
}

export interface TrackEntry { id: number; name: string }
export interface TrackSelection {
  audio: TrackEntry[];
  subtitle: TrackEntry[];
  current_audio: number;
  current_subtitle: number;
}

export interface EngineMeta {
  title: string | null;
  artist: string | null;
  album: string | null;
  genre: string | null;
  date: string | null;
  artwork_url: string | null;
}

export interface MediaStats {
  read_bytes: number;
  input_bitrate_kbps: number;
  demux_corrupted: number;
  demux_discontinuity: number;
  decoded_video: number;
  decoded_audio: number;
  displayed_pictures: number;
  lost_pictures: number;
  played_abuffers: number;
  lost_abuffers: number;
}

export interface ProbeStream {
  index: number;
  codec_type?: string;
  codec_name?: string;
  codec_long_name?: string;
  profile?: string;
  width?: number;
  height?: number;
  pix_fmt?: string;
  r_frame_rate?: string;
  color_transfer?: string;
  color_primaries?: string;
  sample_rate?: string;
  channels?: number;
  channel_layout?: string;
  sample_fmt?: string;
  bits_per_raw_sample?: string;
  bit_rate?: string;
  disposition?: Record<string, number>;
  tags: Record<string, string>;
}

export interface ProbeResult {
  streams: ProbeStream[];
  format: {
    format_name?: string;
    format_long_name?: string;
    duration?: string;
    size?: string;
    bit_rate?: string;
    tags: Record<string, string>;
  };
  chapters: { start_time?: string; tags: Record<string, string> }[];
  stream_groups: Array<Record<string, unknown>>;
}

export interface OpenResult { path: string; probe: ProbeResult | null; probe_error: string | null }

export interface Diagnostics {
  engine: { name: string; version: string; location: string } | null;
  engine_error: string | null;
  ffprobe: string | null;
}

export type SurfaceEvent =
  | { kind: "down" | "click" | "doubleclick" | "context" | "move" }
  | { kind: "wheel"; delta: number }
  | { kind: "drop"; paths: string[] };

export const engine = {
  diagnostics: () => invoke<Diagnostics>("diagnostics"),
  startupFiles: () => invoke<string[]>("startup_files"),
  expand: (items: string[]) => invoke<string[]>("expand_sources", { items }),
  isSubtitle: (path: string) => invoke<boolean>("is_subtitle_file", { path }),
  open: (path: string) => invoke<OpenResult>("open_media", { path }),
  play: () => invoke<void>("play"),
  setPaused: (paused: boolean) => invoke<void>("set_paused", { paused }),
  stop: () => invoke<void>("stop"),
  seek: (ms: number) => invoke<void>("seek", { ms: Math.max(0, Math.round(ms)) }),
  setVolume: (volume: number) => invoke<void>("set_volume", { volume: Math.round(volume) }),
  setMuted: (muted: boolean) => invoke<void>("set_muted", { muted }),
  setRate: (rate: number) => invoke<void>("set_rate", { rate }),
  nextFrame: () => invoke<void>("next_frame"),
  status: () => invoke<PlaybackStatus>("status"),
  tracks: () => invoke<TrackSelection>("tracks"),
  setAudioTrack: (id: number) => invoke<void>("set_audio_track", { id }),
  setSubtitleTrack: (id: number) => invoke<void>("set_subtitle_track", { id }),
  addSubtitleFile: (path: string) => invoke<void>("add_subtitle_file", { path }),
  setAudioDelay: (ms: number) => invoke<void>("set_audio_delay", { ms: Math.round(ms) }),
  setSubtitleDelay: (ms: number) => invoke<void>("set_subtitle_delay", { ms: Math.round(ms) }),
  setAspectRatio: (aspect: string | null) => invoke<void>("set_aspect_ratio", { aspect }),
  setDeinterlace: (mode: string | null) => invoke<void>("set_deinterlace", { mode }),
  setChapter: (chapter: number) => invoke<void>("set_chapter", { chapter }),
  videoSize: () => invoke<[number, number] | null>("video_size"),
  stats: () => invoke<MediaStats | null>("stats"),
  meta: () => invoke<EngineMeta>("meta"),
  snapshot: (stem: string) => invoke<string>("snapshot", { stem }),
  setVideoRect: (r: { x: number; y: number; w: number; h: number; visible: boolean }) =>
    invoke<void>("set_video_rect", r),
  setCursorHidden: (hidden: boolean) => invoke<void>("set_cursor_hidden", { hidden }),
};

export const RATES = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 1.75, 2, 3, 4];
export const ASPECTS = ["default", "16:9", "4:3", "21:9", "2.35:1", "1:1", "5:4", "16:10", "fill"];

export function formatTime(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

export function basename(path: string): string {
  if (/^[a-z][a-z0-9+.-]*:\/\//i.test(path)) {
    try { const u = new URL(path); return decodeURIComponent(u.pathname.split("/").pop() || u.host); } catch { return path; }
  }
  return path.split(/[\\/]/).pop() ?? path;
}

export interface EngineConfig {
  hw_decode: boolean; normalize: boolean; replay_gain: boolean; visualizer: string | null; transform: string;
  sub_scale: number; sub_color: number; sub_outline: number; sub_background: number; sub_margin: number;
}
export interface Equalizer { preamp: number; bands: number[] }
export interface EqualizerInfo { frequencies: number[]; presets: [string, Equalizer][] }
export interface VideoAdjust { brightness: number; contrast: number; hue: number; saturation: number; gamma: number }
export interface AudioDevice { id: string; name: string }
export interface BrowseItem { name: string; url: string; kind: "directory" | "file" | "stream" | "playlist" | "disc" | "unknown" }
export interface MediaRow {
  path: string; kind: "audio" | "video" | "unknown"; title: string | null; artist: string | null; album: string | null;
  album_artist: string | null; genre: string | null; year: number | null; track: number | null; disc: number | null;
  duration_ms: number | null; codec: string | null; width: number | null; height: number | null; size: number | null;
  favorite: boolean; play_count: number; last_played: number | null; lyrics: string | null;
}
export interface Album { album: string; artist: string; year: number | null; tracks: number; duration_ms: number; first_path: string }
export interface Artist { artist: string; albums: number; tracks: number }
export interface Rule { field: string; op: string; value: string }
export interface SmartRules { match: "all" | "any"; rules: Rule[]; limit: number | null; sort: string | null }
export interface Playlist { id: number; name: string; rules: SmartRules | null; count: number }
export interface ScanProgress { done: number; total: number; added: number; removed: number; failed: number; current: string; finished: boolean }
export interface ListeningStats { total_ms: number; plays: number; top_tracks: [string, number][]; top_artists: [string, number][]; per_day: [number, number][] }
export interface Lyrics { synced: [number, string][]; plain: string | null; source: string }
export interface Preset { id: string; label: string; ext: string; audio_only: boolean }
export interface ConvertOptions { start_ms?: number | null; end_ms?: number | null; height?: number | null; normalize?: boolean }
export interface JobEvent { id: number; progress: number; done: boolean; error: string | null; output: string | null }
export interface ClipEvent { time_ms: number; channel: number; samples: number }
export interface Analysis {
  path: string; stream: number | null; channels: number; layout: string | null; duration_ms: number;
  integrated_lufs: number; loudness_range_lu: number; max_momentary_lufs: number; max_short_term_lufs: number;
  sample_peak_dbfs: number; true_peak_dbtp: number; channel_true_peak_dbtp: number[]; channel_rms_dbfs: number[];
  channel_dc_offset: number[]; clip_events: ClipEvent[]; clipped_samples: number; silences: [number, number][];
  timeline: [number, number, number][];
}
export interface LabEvent { id: number; progress: number; result: Analysis | null; error: string | null }
export type EngineEvent = "opening" | "playing" | "paused" | "stopped" | "ended" | "error" | "length" | "tracks" | "vout" | "media";

export const api = {
  settings: () => invoke<Record<string, unknown>>("get_settings"),
  setSetting: (key: string, value: unknown) => invoke<void>("set_setting", { key, value }),
  configure: (cfg: EngineConfig) => invoke<boolean>("configure_engine", { cfg }),
  engineConfig: () => invoke<EngineConfig>("engine_config"),
  equalizerInfo: () => invoke<EqualizerInfo>("equalizer_info"),
  setEqualizer: (eq: Equalizer | null) => invoke<void>("set_equalizer", { eq }),
  setVideoAdjust: (adj: VideoAdjust | null) => invoke<void>("set_video_adjust", { adj }),
  setCrop: (geometry: string | null) => invoke<void>("set_crop", { geometry }),
  audioDevices: () => invoke<AudioDevice[]>("audio_devices"),
  setAudioDevice: (id: string) => invoke<void>("set_audio_device", { id }),
  browse: (url: string) => invoke<BrowseItem[]>("browse", { url }),
  discoveryServices: () => invoke<[string, string][]>("discovery_services"),
  discover: (service: string) => invoke<BrowseItem[]>("discover", { service }),
  plugins: () => invoke<[string, string[]][]>("engine_plugins"),
  roots: () => invoke<string[]>("library_roots"),
  addRoot: (path: string) => invoke<void>("library_add_root", { path }),
  removeRoot: (path: string) => invoke<void>("library_remove_root", { path }),
  scan: () => invoke<ScanProgress>("library_scan"),
  list: (kind: string | null, search: string | null, favorites = false, sort: string | null = null) =>
    invoke<MediaRow[]>("library_list", { kind, search, favorites, sort }),
  albums: () => invoke<Album[]>("library_albums"),
  albumTracks: (album: string, artist: string) => invoke<MediaRow[]>("library_album_tracks", { album, artist }),
  artists: () => invoke<Artist[]>("library_artists"),
  artistTracks: (artist: string) => invoke<MediaRow[]>("library_artist_tracks", { artist }),
  mediaRow: (path: string) => invoke<MediaRow | null>("media_row", { path }),
  setFavorite: (path: string, favorite: boolean) => invoke<void>("set_favorite", { path, favorite }),
  cover: (path: string) => invoke<string | null>("cover", { path }),
  lyrics: (path: string) => invoke<Lyrics | null>("lyrics", { path }),
  playlists: () => invoke<Playlist[]>("playlists"),
  createPlaylist: (name: string, rules: SmartRules | null, items: string[]) => invoke<number>("playlist_create", { name, rules, items }),
  updatePlaylist: (id: number, name: string, rules: SmartRules | null) => invoke<void>("playlist_update", { id, name, rules }),
  deletePlaylist: (id: number) => invoke<void>("playlist_delete", { id }),
  playlistItems: (id: number) => invoke<string[]>("playlist_items", { id }),
  setPlaylistItems: (id: number, items: string[]) => invoke<void>("playlist_set_items", { id, items }),
  addToPlaylist: (id: number, items: string[]) => invoke<void>("playlist_add", { id, items }),
  exportPlaylist: (name: string, items: string[], target: string) => invoke<void>("playlist_export", { name, items, target }),
  importPlaylist: (path: string) => invoke<number>("playlist_import", { path }),
  recordPlay: (path: string, listenedMs: number) => invoke<void>("record_play", { path, listenedMs: Math.round(listenedMs) }),
  stats: () => invoke<ListeningStats>("listening_stats"),
  clearHistory: () => invoke<void>("clear_history"),
  resumeGet: (path: string) => invoke<number | null>("resume_get", { path }),
  resumeSet: (path: string, ms: number | null) => invoke<void>("resume_set", { path, ms: ms === null ? null : Math.round(ms) }),
  presets: () => invoke<Preset[]>("convert_presets"),
  convert: (input: string, output: string, preset: string, options: ConvertOptions) =>
    invoke<number>("convert_start", { input, output, preset, options }),
  cancelJob: (id: number) => invoke<void>("cancel_job", { id }),
  analyze: (path: string, stream: number | null) => invoke<number>("lab_analyze", { path, stream }),
  exportAnalysis: (analysis: Analysis, target: string) => invoke<void>("lab_export", { analysis, target }),
  extractStream: (path: string, stream: number) => invoke<string>("lab_extract", { path, stream }),
  probe: (path: string) => invoke<ProbeResult>("probe_file", { path }),
};

export const DEFAULT_CONFIG: EngineConfig = {
  hw_decode: true, normalize: false, replay_gain: false, visualizer: null, transform: "",
  sub_scale: 100, sub_color: 0xffffff, sub_outline: 4, sub_background: 0, sub_margin: 0,
};

/**
 * Settings store. Values live in the app's SQLite database (loaded once at
 * startup by `loadSettings`) behind a synchronous in-memory cache; writes are
 * debounced per key. Falls back to localStorage when the backend is absent
 * (plain browser preview). Never throws.
 */
const cache: Record<string, unknown> = {};
const timers: Record<string, number> = {};
let backend = false;

export async function loadSettings() {
  try {
    Object.assign(cache, await api.settings());
    backend = true;
    // One-time migration of settings kept in localStorage by earlier versions.
    try {
      for (let i = 0; i < localStorage.length; i++) {
        const k = localStorage.key(i)!;
        if (!(k in cache) && k !== "resume") {
          cache[k] = JSON.parse(localStorage.getItem(k)!);
          api.setSetting(k, cache[k]).catch(() => {});
        }
      }
      localStorage.clear();
    } catch { /* ignore */ }
  } catch { backend = false; }
}

export const store = {
  get<T>(key: string, fallback: T): T {
    if (key in cache) return cache[key] as T;
    if (!backend) {
      try { const v = localStorage.getItem(key); if (v) return JSON.parse(v) as T; } catch { /* ignore */ }
    }
    return fallback;
  },
  set(key: string, value: unknown) {
    cache[key] = value;
    if (!backend) { try { localStorage.setItem(key, JSON.stringify(value)); } catch { /* ignore */ } return; }
    window.clearTimeout(timers[key]);
    timers[key] = window.setTimeout(() => { api.setSetting(key, value ?? null).catch(() => {}); }, 300);
  },
};

export function formatBytes(n: number | null | undefined): string {
  if (!n) return "—";
  const u = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(u.length - 1, Math.floor(Math.log(n) / Math.log(1024)));
  return `${(n / 1024 ** i).toFixed(i ? 1 : 0)} ${u[i]}`;
}

export function displayTitle(m: { title: string | null; path: string }): string {
  return m.title || basename(m.path).replace(/\.[^.]+$/, "");
}
