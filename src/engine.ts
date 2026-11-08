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
  rate: number;
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
}

export interface OpenResult { path: string; probe: ProbeResult | null; probe_error: string | null }

export interface Diagnostics {
  engine: { name: string; version: string; location: string } | null;
  engine_error: string | null;
  ffprobe: string | null;
}

export const engine = {
  diagnostics: () => invoke<Diagnostics>("diagnostics"),
  startupFile: () => invoke<string | null>("startup_file"),
  open: (path: string) => invoke<OpenResult>("open_media", { path }),
  play: () => invoke<void>("play"),
  setPaused: (paused: boolean) => invoke<void>("set_paused", { paused }),
  stop: () => invoke<void>("stop"),
  seek: (ms: number) => invoke<void>("seek", { ms: Math.round(ms) }),
  setVolume: (volume: number) => invoke<void>("set_volume", { volume: Math.round(volume) }),
  setRate: (rate: number) => invoke<void>("set_rate", { rate }),
  nextFrame: () => invoke<void>("next_frame"),
  status: () => invoke<PlaybackStatus>("status"),
  tracks: () => invoke<TrackSelection>("tracks"),
  setAudioTrack: (id: number) => invoke<void>("set_audio_track", { id }),
  setSubtitleTrack: (id: number) => invoke<void>("set_subtitle_track", { id }),
  meta: () => invoke<EngineMeta>("meta"),
  setVideoRect: (r: { x: number; y: number; w: number; h: number; visible: boolean }) =>
    invoke<void>("set_video_rect", r),
};

export function formatTime(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

export function basename(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}
