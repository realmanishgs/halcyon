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

/** Local storage that never throws (private mode, blocked storage). */
export const store = {
  get<T>(key: string, fallback: T): T {
    try { const v = localStorage.getItem(key); return v ? (JSON.parse(v) as T) : fallback; } catch { return fallback; }
  },
  set(key: string, value: unknown) {
    try { localStorage.setItem(key, JSON.stringify(value)); } catch { /* ignore */ }
  },
};
