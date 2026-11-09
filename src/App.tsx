import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { Menu } from "@tauri-apps/api/menu";
import {
  engine, api, basename, store, formatTime, RATES, ASPECTS, DEFAULT_CONFIG,
  type AudioDevice, type Diagnostics, type EngineConfig, type EngineEvent, type EngineMeta, type EqualizerInfo,
  type Lyrics, type MediaStats, type OpenResult, type PlaybackStatus, type SurfaceEvent, type TrackSelection, type VideoAdjust,
} from "./engine";
import { Icon } from "./icons";
import { Inspector } from "./Inspector";
import { SeekBar } from "./SeekBar";
import { AudioPanel, PicturePanel, QueuePanel, SleepTimer, TunePanel, type EqState } from "./Panels";
import { LibraryPage, MusicPage, NetworkPage, PlaylistsPage, StatsPage, VideosPage, type PlayActions } from "./Library";
import { LabPage } from "./Lab";
import { ConvertPage } from "./Convert";
import { SettingsPage } from "./Settings";

type View = "player" | "library" | "music" | "videos" | "playlists" | "network" | "lab" | "convert" | "stats" | "settings";
type PanelTab = "queue" | "info" | "tune" | "audio" | "picture";
type Repeat = "off" | "all" | "one";

const NAV: { id: View; label: string; icon: string }[] = [
  { id: "library", label: "Library", icon: "library" },
  { id: "music", label: "Music", icon: "music" },
  { id: "videos", label: "Videos", icon: "video" },
  { id: "playlists", label: "Playlists", icon: "list" },
  { id: "network", label: "Network", icon: "network" },
  { id: "lab", label: "Immersive Audio Lab", icon: "lab" },
  { id: "convert", label: "Convert", icon: "convert" },
  { id: "stats", label: "Listening", icon: "clock" },
];
const PANEL_TABS: [PanelTab, string][] = [["queue", "Queue"], ["info", "Info"], ["tune", "Tune"], ["audio", "Audio"], ["picture", "Picture"]];
const FLAT_EQ = (n: number): EqState => ({ enabled: false, preset: "Flat", preamp: 0, bands: Array(n).fill(0) });

function applyTheme(theme: string, accent: string | null) {
  const r = document.documentElement;
  if (theme === "system") r.removeAttribute("data-theme"); else r.setAttribute("data-theme", theme);
  for (const v of ["--accent", "--accent-hover", "--accent-weak", "--focus"]) r.style.removeProperty(v);
  if (accent) {
    r.style.setProperty("--accent", accent);
    r.style.setProperty("--accent-hover", accent);
    r.style.setProperty("--accent-weak", accent + "22");
    r.style.setProperty("--focus", accent);
  }
}
const MEDIA_EXT = [
  "mp4", "m4v", "mkv", "avi", "mov", "webm", "flv", "wmv", "asf", "ts", "m2ts", "mts", "mpg", "mpeg", "vob",
  "3gp", "ogg", "ogm", "ogv", "mxf", "dv", "mp3", "mp2", "flac", "wav", "aif", "aiff", "aac", "m4a", "opus",
  "oga", "wma", "ape", "wv", "tta", "ac3", "eac3", "ec3", "dts", "thd", "mka", "mid", "midi", "caf", "amr",
  "au", "iamf", "m3u", "m3u8", "pls", "xspf",
];
const SUB_EXT = ["srt", "ass", "ssa", "vtt", "sub", "idx", "smi", "ttml"];
const IDLE: PlaybackStatus = {
  state: "idle", time_ms: 0, length_ms: 0, volume: 100, muted: false, rate: 1, seekable: false,
  has_video: false, chapter: -1, chapter_count: -1, audio_delay_ms: 0, subtitle_delay_ms: 0,
};
const isPlayingState = (s: PlaybackStatus["state"]) => s === "playing" || s === "opening" || s === "buffering";
const clamp = (v: number, a: number, b: number) => Math.min(b, Math.max(a, v));

export default function App() {
  const [view, setView] = useState<View>("player");
  const [diag, setDiag] = useState<Diagnostics | null>(null);
  const [queue, setQueue] = useState<string[]>([]);
  const [index, setIndex] = useState(-1);
  const [repeat, setRepeat] = useState<Repeat>(() => store.get("repeat", "off"));
  const [shuffle, setShuffle] = useState<boolean>(() => store.get("shuffle", false));
  const [opened, setOpened] = useState<OpenResult | null>(null);
  const [status, setStatus] = useState<PlaybackStatus>(IDLE);
  const [receivedAt, setReceivedAt] = useState(0);
  const [tracks, setTracks] = useState<TrackSelection | null>(null);
  const [meta, setMeta] = useState<EngineMeta | null>(null);
  const [stats, setStats] = useState<MediaStats | null>(null);
  const [videoSize, setVideoSize] = useState<[number, number] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [toast, setToast] = useState<{ text: string; action?: { label: string; run: () => void } } | null>(null);
  const [panel, setPanel] = useState<PanelTab | null>(() => store.get("panel", null));
  const [panelW, setPanelW] = useState<number>(() => store.get("panelW", 340));
  const [sideW, setSideW] = useState<number>(() => store.get("sideW", 216));
  const [sideCollapsed, setSideCollapsed] = useState<boolean>(() => store.get("sideCollapsed", false));
  const [winW, setWinW] = useState(window.innerWidth);
  const [fullscreen, setFullscreen] = useState(false);
  const [controlsShown, setControlsShown] = useState(true);
  const [dragOver, setDragOver] = useState(false);
  const [urlOpen, setUrlOpen] = useState(false);
  const [aspect, setAspect] = useState("default");
  const [deinterlace, setDeinterlace] = useState<string | null>(null);
  const [recents, setRecents] = useState<string[]>(() => store.get("recents", []));
  const [cfg, setCfg] = useState<EngineConfig>(DEFAULT_CONFIG);
  const [eqInfo, setEqInfo] = useState<EqualizerInfo | null>(null);
  const [eq, setEq] = useState<EqState>(() => store.get("equalizer", FLAT_EQ(10)));
  const [labGain, setLabGain] = useState(0);
  const [adjust, setAdjust] = useState<VideoAdjust | null>(null);
  const [crop, setCrop] = useState<string | null>(null);
  const [devices, setDevices] = useState<AudioDevice[]>([]);
  const [device, setDevice] = useState<string>(() => store.get("audioDevice", ""));
  const [theme, setTheme] = useState<string>(() => store.get("theme", "system"));
  const [accent, setAccent] = useState<string | null>(() => store.get("accent", null));
  const [privateMode, setPrivateMode] = useState<boolean>(() => store.get("private", false));
  const [resumeOn, setResumeOn] = useState<boolean>(() => store.get("resumeOn", true));
  const [favorite, setFavorite] = useState(false);
  const [lyrics, setLyrics] = useState<Lyrics | null>(null);
  const [sleepAt, setSleepAt] = useState<number | null>(null);
  const [sleepAtEnd, setSleepAtEnd] = useState(false);
  const [mini, setMini] = useState(false);
  const listened = useRef({ path: "", ms: 0, last: -1, recorded: false });
  const videoRef = useRef<HTMLDivElement>(null);
  const transportRef = useRef<HTMLElement>(null);
  const prevState = useRef<PlaybackStatus["state"]>("idle");
  const pendingResume = useRef<number | null>(null);
  const silentResume = useRef(false);
  const hideTimer = useRef<number | undefined>(undefined);
  const overTransport = useRef(false);

  // ---------- derived ----------
  const current = index >= 0 ? queue[index] : null;
  const probedVideo = opened?.probe
    ? opened.probe.streams.some((s) => s.codec_type === "video" && !s.disposition?.attached_pic)
    : null;
  // Probe when we have it; otherwise trust the engine's video output count.
  // With a visualiser, audio-only media also gets a video output to show.
  const hasVideo = !!opened && ((probedVideo ?? (status.has_video || status.state === "opening")) || (!!cfg.visualizer && status.has_video));
  const playing = isPlayingState(status.state);
  // libVLC reports length 0 before the input starts and after it ends; fall back to the probed duration.
  const lengthMs = status.length_ms || Math.round(Number(opened?.probe?.format.duration ?? 0) * 1000);
  const viewStatus = useMemo(() => ({ ...status, length_ms: lengthMs, seekable: status.seekable || (lengthMs > 0 && !!opened?.probe) }), [status, lengthMs, opened]);
  const narrow = winW < 860;
  const tiny = winW < 600;
  const sidebarHidden = fullscreen || tiny;
  const sidebarMini = !sidebarHidden && (sideCollapsed || narrow);
  const panelVisible = !!panel && !fullscreen;
  const panelTakesOver = panelVisible && tiny;
  const iamf = !!opened?.probe && (
    opened.probe.stream_groups.some((g) => String(g.type ?? "").toUpperCase().includes("IAMF")) ||
    opened.probe.streams.some((s) => s.codec_name === "iamf") ||
    /iamf/.test(opened.probe.format.format_name ?? ""));
  const chapters = useMemo(() => (opened?.probe?.chapters ?? []).map((c) => Number(c.start_time ?? 0) * 1000), [opened]);

  useEffect(() => store.set("repeat", repeat), [repeat]);
  useEffect(() => store.set("shuffle", shuffle), [shuffle]);
  useEffect(() => store.set("panel", panel), [panel]);
  useEffect(() => store.set("panelW", panelW), [panelW]);
  useEffect(() => store.set("sideW", sideW), [sideW]);
  useEffect(() => store.set("sideCollapsed", sideCollapsed), [sideCollapsed]);
  useEffect(() => store.set("equalizer", eq), [eq]);
  useEffect(() => store.set("theme", theme), [theme]);
  useEffect(() => store.set("accent", accent), [accent]);
  useEffect(() => store.set("private", privateMode), [privateMode]);
  useEffect(() => store.set("resumeOn", resumeOn), [resumeOn]);
  useEffect(() => store.set("audioDevice", device), [device]);
  useEffect(() => applyTheme(theme, accent), [theme, accent]);
  useEffect(() => {
    engine.diagnostics().then(setDiag);
    api.engineConfig().then(setCfg).catch(() => {});
    api.equalizerInfo().then((i) => { setEqInfo(i); setEq((e) => (e.bands.length === i.frequencies.length ? e : FLAT_EQ(i.frequencies.length))); }).catch(() => {});
    api.audioDevices().then(setDevices).catch(() => {});
    if (device) api.setAudioDevice(device).catch(() => {});
    const v = store.get<number | null>("volume", null);
    if (v !== null) engine.setVolume(v).catch(() => {});
  }, []); // eslint-disable-line react-hooks/exhaustive-deps
  // Equalizer (plus any Lab level-matching gain) follows the stored state.
  useEffect(() => {
    if (!eqInfo) return;
    const on = eq.enabled || labGain !== 0;
    const base = eq.enabled ? eq : FLAT_EQ(eqInfo.frequencies.length);
    api.setEqualizer(on ? { preamp: Math.max(-20, Math.min(20, base.preamp + labGain)), bands: base.bands } : null).catch(() => {});
  }, [eq, labGain, eqInfo]);
  useEffect(() => {
    const onResize = () => setWinW(window.innerWidth);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  const say = useCallback((text: string, action?: { label: string; run: () => void }) => {
    setToast({ text, action });
  }, []);
  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(null), toast.action ? 6000 : 2500);
    return () => clearTimeout(t);
  }, [toast]);

  // ---------- engine state ----------
  const poll = useCallback(() => {
    engine.status().then((s) => { setStatus(s); setReceivedAt(performance.now()); }).catch(() => {});
  }, []);
  const playingNow = isPlayingState(status.state);
  useEffect(() => { const t = setInterval(poll, playingNow ? 250 : 1000); return () => clearInterval(t); }, [poll, playingNow]);

  const refreshDetails = useCallback(() => {
    engine.tracks().then(setTracks).catch(() => {});
    // Keep the tags already read for this item if the engine was restarted and has not re-read them.
    engine.meta().then((m) => setMeta((old) => (m.title || m.artist || m.album || !old ? m : old))).catch(() => {});
    engine.videoSize().then(setVideoSize).catch(() => {});
  }, []);

  useEffect(() => {
    if (panel !== "info" || !opened) return;
    const t = setInterval(() => engine.stats().then(setStats).catch(() => {}), 1000);
    return () => clearInterval(t);
  }, [panel, opened]);

  // ---------- listening history ----------
  const flushPlay = useCallback(() => {
    const l = listened.current;
    if (!l.path || l.recorded || privateMode) return;
    const len = lengthMs || 0;
    if (l.ms >= 30000 || (len > 0 && l.ms >= len / 2)) {
      l.recorded = true;
      api.recordPlay(l.path, l.ms).catch(() => {});
    }
  }, [privateMode, lengthMs]);
  useEffect(() => {
    const l = listened.current;
    if (status.state === "playing" && current === l.path) {
      if (l.last >= 0) { const d = status.time_ms - l.last; if (d > 0 && d < 2000) l.ms += d; }
      l.last = status.time_ms;
      flushPlay();
    } else l.last = -1;
  }, [status.time_ms]); // eslint-disable-line react-hooks/exhaustive-deps

  // ---------- loading ----------
  const loadIndex = useCallback(async (i: number, list: string[], startAt?: number) => {
    const src = list[i];
    if (!src) return;
    flushPlay();
    listened.current = { path: src, ms: 0, last: -1, recorded: false };
    setIndex(i);
    setError(null);
    setTracks(null);
    setMeta(null);
    setStats(null);
    setVideoSize(null);
    setAspect("default");
    try {
      const r = await engine.open(src);
      setOpened(r);
      setView((v) => (v === "lab" || v === "convert" ? v : "player"));
      const resume = startAt !== undefined ? startAt : resumeOn && !privateMode ? await api.resumeGet(src).catch(() => null) : null;
      pendingResume.current = resume ?? null;
      silentResume.current = startAt !== undefined;
      if (!privateMode) {
        setRecents((old) => {
          const next = [src, ...old.filter((x) => x !== src)].slice(0, 20);
          store.set("recents", next);
          return next;
        });
      }
      api.mediaRow(src).then((m) => setFavorite(!!m?.favorite)).catch(() => setFavorite(false));
      setLyrics(null);
      if (!src.includes("://")) api.lyrics(src).then(setLyrics).catch(() => {});
      setTimeout(refreshDetails, 500);
      setTimeout(refreshDetails, 1800);
      poll();
    } catch (e) {
      setError(`${basename(src)}: ${e}`);
    }
  }, [refreshDetails, poll, flushPlay, resumeOn, privateMode]);

  const playSources = useCallback(async (items: string[], append = false) => {
    const expanded = await engine.expand(items);
    if (!expanded.length) { say("Nothing playable found"); return; }
    if (append && queue.length) {
      setQueue((q) => [...q, ...expanded]);
      say(`Added ${expanded.length} to queue`);
      return;
    }
    setQueue(expanded);
    if (expanded.length > 1) say(`${expanded.length} items queued`);
    await loadIndex(0, expanded);
  }, [queue.length, loadIndex, say]);

  const handleDropped = useCallback(async (paths: string[]) => {
    if (!paths.length) return;
    const subs = paths.filter((p) => SUB_EXT.includes(p.split(".").pop()!.toLowerCase()));
    if (opened && subs.length === paths.length) {
      for (const s of subs) {
        await engine.addSubtitleFile(s).then(() => say(`Subtitles: ${basename(s)}`)).catch((e) => setError(String(e)));
      }
      setTimeout(refreshDetails, 400);
      return;
    }
    await playSources(paths.filter((p) => !subs.includes(p)));
  }, [opened, playSources, refreshDetails, say]);

  useEffect(() => { engine.startupFiles().then((f) => { if (f.length) playSources(f); }); }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const pickFiles = useCallback(async () => {
    const f = await openDialog({
      multiple: true,
      filters: [{ name: "Media & playlists", extensions: MEDIA_EXT }, { name: "All files", extensions: ["*"] }],
    });
    if (Array.isArray(f) && f.length) playSources(f);
  }, [playSources]);
  const pickFolder = useCallback(async () => {
    const f = await openDialog({ directory: true, multiple: false });
    if (typeof f === "string") playSources([f]);
  }, [playSources]);
  const pickSubtitle = useCallback(async () => {
    const f = await openDialog({ multiple: false, filters: [{ name: "Subtitles", extensions: SUB_EXT }] });
    if (typeof f === "string") handleDropped([f]);
  }, [handleDropped]);

  // ---------- queue navigation ----------
  const nextIndex = useCallback((dir: 1 | -1): number | null => {
    if (!queue.length) return null;
    if (shuffle && queue.length > 1) {
      let j = index;
      while (j === index) j = Math.floor(Math.random() * queue.length);
      return j;
    }
    const j = index + dir;
    if (j >= 0 && j < queue.length) return j;
    if (repeat === "all") return (j + queue.length) % queue.length;
    return null;
  }, [queue.length, index, shuffle, repeat]);

  const next = useCallback(() => {
    const j = nextIndex(1);
    if (j !== null) loadIndex(j, queue); else say("End of queue");
  }, [nextIndex, loadIndex, queue, say]);
  const prev = useCallback(() => {
    if (status.time_ms > 3000) { engine.seek(0); poll(); return; }
    const j = nextIndex(-1);
    if (j !== null) loadIndex(j, queue); else { engine.seek(0); poll(); }
  }, [status.time_ms, nextIndex, loadIndex, queue, poll]);

  // State transitions: end of media, errors, resume.
  useEffect(() => {
    const was = prevState.current;
    prevState.current = status.state;
    if (status.state === "playing" && pendingResume.current !== null) {
      const at = pendingResume.current;
      pendingResume.current = null;
      if (at > 0 && (status.length_ms === 0 || at < status.length_ms - 5000)) {
        engine.seek(at);
        if (!silentResume.current) say(`Resumed at ${formatTime(at)}`, { label: "Start over", run: () => { engine.seek(0); poll(); } });
      }
    }
    if (status.state === "ended" && was !== "ended" && current) {
      api.resumeSet(current, null).catch(() => {});
      flushPlay();
      if (sleepAtEnd) { setSleepAtEnd(false); say("Sleep timer: stopped"); return; }
      if (repeat === "one") { listened.current = { path: current, ms: 0, last: -1, recorded: false }; engine.play(); return; }
      const j = nextIndex(1);
      if (j !== null) loadIndex(j, queue);
    }
    if (status.state === "error" && was !== "error") {
      setError(`The engine could not play ${current ? basename(current) : "this media"}.`);
      const j = nextIndex(1);
      if (j !== null && queue.length > 1) setTimeout(() => loadIndex(j, queue), 1200);
    }
  }, [status.state]); // eslint-disable-line react-hooks/exhaustive-deps

  // Remember position for long media (resume on next open).
  useEffect(() => {
    if (!current || privateMode || !resumeOn || status.state !== "playing" || status.length_ms < 60000) return;
    if (Math.floor(status.time_ms / 5000) === Math.floor((status.time_ms - 250) / 5000)) return;
    const keep = status.time_ms > 10000 && status.time_ms < status.length_ms - 15000;
    api.resumeSet(current, keep ? status.time_ms : null).catch(() => {});
  }, [status.time_ms]); // eslint-disable-line react-hooks/exhaustive-deps

  // ---------- sleep timer: fade the last 10 s, then pause ----------
  useEffect(() => {
    if (!sleepAt) return;
    const startVol = status.volume;
    const t = setInterval(() => {
      const left = sleepAt - Date.now();
      if (left <= 0) {
        engine.setPaused(true).then(() => engine.setVolume(startVol)).finally(poll);
        setSleepAt(null);
        say("Sleep timer: paused");
      } else if (left < 10000) engine.setVolume(Math.round(startVol * (left / 10000)));
    }, 500);
    return () => clearInterval(t);
  }, [sleepAt]); // eslint-disable-line react-hooks/exhaustive-deps

  // ---------- transport actions ----------
  const act = useCallback((p: Promise<unknown>) => { p.catch((e) => setError(String(e))).finally(poll); }, [poll]);
  const togglePlay = useCallback(() => {
    if (!opened) { if (queue.length) loadIndex(Math.max(0, index), queue); else pickFiles(); return; }
    if (["stopped", "ended", "error", "idle"].includes(status.state)) act(engine.play());
    else act(engine.setPaused(playing));
  }, [opened, queue, index, loadIndex, pickFiles, status.state, playing, act]);
  const seekTo = useCallback((ms: number) => act(engine.seek(ms)), [act]);
  const seekBy = useCallback((d: number) => {
    if (lengthMs > 0) seekTo(clamp(status.time_ms + d, 0, lengthMs - 500));
  }, [status.time_ms, lengthMs, seekTo]);
  const setVolume = useCallback((v: number) => {
    const vol = clamp(Math.round(v), 0, 200);
    act(engine.setVolume(vol));
    store.set("volume", vol);
    if (status.muted && vol > 0) act(engine.setMuted(false));
    say(`Volume ${vol}%`);
  }, [act, status.muted, say]);
  const toggleMute = useCallback(() => { act(engine.setMuted(!status.muted)); say(status.muted ? "Unmuted" : "Muted"); }, [act, status.muted, say]);
  const setRate = useCallback((r: number) => {
    const rate = clamp(Math.round(r * 100) / 100, 0.25, 4);
    act(engine.setRate(rate));
    say(`Speed ${rate}×`);
  }, [act, say]);
  const stepRate = (dir: 1 | -1) => {
    const i = RATES.findIndex((x) => x >= status.rate - 0.001);
    setRate(RATES[clamp((i < 0 ? RATES.length - 1 : i) + dir, 0, RATES.length - 1)]);
  };
  const applyAspect = useCallback((a: string) => {
    setAspect(a);
    if (a === "fill") {
      const r = videoRef.current?.getBoundingClientRect();
      act(engine.setAspectRatio(r ? `${Math.round(r.width)}:${Math.round(r.height)}` : null));
    } else act(engine.setAspectRatio(a === "default" ? null : a));
    say(`Aspect: ${a === "fill" ? "stretch to window" : a}`);
  }, [act, say]);
  const applyDeinterlace = useCallback((m: string | null) => {
    setDeinterlace(m);
    act(engine.setDeinterlace(m));
    say(`Deinterlace: ${m ?? "off"}`);
  }, [act, say]);
  const delay = useCallback((kind: "audio" | "subtitle", d: number) => {
    const cur = kind === "audio" ? status.audio_delay_ms : status.subtitle_delay_ms;
    const v = d === 0 ? 0 : cur + d;
    act(kind === "audio" ? engine.setAudioDelay(v) : engine.setSubtitleDelay(v));
    say(`${kind === "audio" ? "Audio" : "Subtitle"} delay ${v} ms`);
  }, [status.audio_delay_ms, status.subtitle_delay_ms, act, say]);
  const cycleTrack = useCallback((kind: "audio" | "subtitle") => {
    if (!tracks) return;
    const list = kind === "audio" ? tracks.audio : tracks.subtitle;
    if (!list.length) return;
    const cur = kind === "audio" ? tracks.current_audio : tracks.current_subtitle;
    const nxt = list[(list.findIndex((t) => t.id === cur) + 1) % list.length];
    const p = kind === "audio" ? engine.setAudioTrack(nxt.id) : engine.setSubtitleTrack(nxt.id);
    act(p.then(refreshDetails));
    say(`${kind === "audio" ? "Audio" : "Subtitles"}: ${nxt.id === -1 ? "off" : nxt.name}`);
  }, [tracks, act, refreshDetails, say]);
  const snapshot = useCallback(() => {
    if (!hasVideo) return;
    engine.snapshot(current ? basename(current).replace(/\.[^.]+$/, "") : "snapshot")
      .then((p) => say(`Snapshot saved: ${basename(p)}`)).catch((e) => setError(String(e)));
  }, [hasVideo, current, say]);
  const toggleFullscreen = useCallback(async (force?: boolean) => {
    const w = getCurrentWindow();
    const next = force ?? !(await w.isFullscreen());
    await w.setFullscreen(next);
    setFullscreen(next);
    setControlsShown(true);
  }, []);
  const openPanel = useCallback((t: PanelTab) => setPanel((p) => (p === t ? null : t)), []);

  const applyConfig = useCallback(async (next: EngineConfig) => {
    setCfg(next);
    try {
      const restarted = await api.configure(next);
      if (restarted && opened) { say("Engine options applied"); setTimeout(refreshDetails, 600); }
    } catch (e) { setError("Engine rejected these options: " + e); }
    poll();
  }, [opened, refreshDetails, poll, say]);
  const toggleFavorite = useCallback(() => {
    if (!current) return;
    const f = !favorite;
    setFavorite(f);
    api.setFavorite(current, f).then(() => say(f ? "Added to favourites" : "Removed from favourites")).catch((e) => setError(String(e)));
  }, [current, favorite, say]);
  const toggleMini = useCallback(async (force?: boolean) => {
    const w = getCurrentWindow();
    const nextMini = force ?? !mini;
    if (nextMini) {
      const sz = await w.innerSize();
      const k = await w.scaleFactor();
      store.set("normalSize", [sz.width / k, sz.height / k]);
      if (fullscreen) { await w.setFullscreen(false); setFullscreen(false); }
      await w.setMinSize(new LogicalSize(320, 60));
      await w.setAlwaysOnTop(true);
      setPanel(null);
    } else {
      const [ww, hh] = store.get<[number, number]>("normalSize", [1200, 760]);
      await w.setAlwaysOnTop(false);
      await w.setSize(new LogicalSize(ww, hh));
      await w.setMinSize(new LogicalSize(420, 300));
    }
    setMini(nextMini);
  }, [mini, fullscreen]);
  // Size the mini player to its content: the control bar (plus a 16:9 picture for video).
  useEffect(() => {
    if (!mini) return;
    const raf = requestAnimationFrame(() => {
      const bar = transportRef.current?.offsetHeight ?? 88;
      getCurrentWindow().setSize(new LogicalSize(420, bar + (hasVideo ? Math.round(420 * 9 / 16) : 0))).catch(() => {});
    });
    return () => cancelAnimationFrame(raf);
  }, [mini, hasVideo]);
  const actions: PlayActions = useMemo(() => ({
    play: (items: string[], start = 0) => { if (!items.length) return; setQueue(items); loadIndex(start, items); },
    enqueue: (items: string[]) => {
      setQueue((q) => [...q, ...items]);
      if (!opened && items.length) loadIndex(queue.length, [...queue, ...items]);
    },
    say: (t: string) => say(t),
  }), [loadIndex, opened, queue, say]);
  const openAt = useCallback((path: string, ms: number) => {
    const i = queue.indexOf(path);
    if (i >= 0) loadIndex(i, queue, ms); else { setQueue([path]); loadIndex(0, [path], ms); }
  }, [queue, loadIndex]);

  // ---------- fullscreen auto-hide ----------
  const wake = useCallback(() => {
    setControlsShown(true);
    window.clearTimeout(hideTimer.current);
    if (!fullscreen) return;
    hideTimer.current = window.setTimeout(() => {
      if (!overTransport.current) setControlsShown(false);
    }, 2500);
  }, [fullscreen]);
  useEffect(() => { wake(); }, [fullscreen, wake]);
  useEffect(() => {
    const hidden = fullscreen && !controlsShown && playing;
    engine.setCursorHidden(hidden);
    document.body.classList.toggle("hide-cursor", hidden);
  }, [fullscreen, controlsShown, playing]);

  // ---------- context menu (native: draws above the video surface) ----------
  const showMenu = useCallback(async () => {
    const t = tracks;
    const items: Parameters<typeof Menu.new>[0] = {
      items: [
        { text: playing ? "Pause" : "Play", action: togglePlay },
        { text: "Stop", enabled: !!opened, action: () => act(engine.stop()) },
        { text: "Previous", enabled: queue.length > 0, action: prev },
        { text: "Next", enabled: queue.length > 1, action: next },
        { item: "Separator" },
        { text: fullscreen ? "Exit fullscreen" : "Fullscreen", action: () => toggleFullscreen() },
        {
          text: "Speed", items: RATES.map((r) => ({ text: `${r}×`, checked: Math.abs(status.rate - r) < 0.01, action: () => setRate(r) })),
        },
        {
          text: "Audio track", enabled: !!t && t.audio.length > 0,
          items: (t?.audio ?? []).map((a) => ({ text: a.name, checked: a.id === t?.current_audio, action: () => act(engine.setAudioTrack(a.id).then(refreshDetails)) })),
        },
        {
          text: "Subtitles", enabled: !!opened,
          items: [
            ...(t?.subtitle ?? []).map((s) => ({ text: s.id === -1 ? "Off" : s.name, checked: s.id === t?.current_subtitle, action: () => act(engine.setSubtitleTrack(s.id).then(refreshDetails)) })),
            { item: "Separator" as const },
            { text: "Add subtitle file…", action: pickSubtitle },
          ],
        },
        {
          text: "Aspect ratio", enabled: hasVideo,
          items: ASPECTS.map((a) => ({ text: a === "default" ? "Default" : a === "fill" ? "Stretch to window" : a, checked: aspect === a, action: () => applyAspect(a) })),
        },
        {
          text: "Deinterlace", enabled: hasVideo,
          items: [null, "blend", "yadif", "yadif2x", "bob", "linear"].map((m) => ({ text: m ?? "Off", checked: deinterlace === m, action: () => applyDeinterlace(m) })),
        },
        { text: "Take snapshot", enabled: hasVideo, action: snapshot },
        { text: favorite ? "Remove from favourites" : "Add to favourites", enabled: !!current, action: toggleFavorite },
        { text: mini ? "Leave mini player" : "Mini player", action: () => toggleMini() },
        { item: "Separator" },
        { text: "Open file…", action: pickFiles },
        { text: "Open folder…", action: pickFolder },
        { text: "Open network stream…", action: () => setUrlOpen(true) },
        { item: "Separator" },
        { text: "Media information", action: () => setPanel("info") },
        { text: "Queue", action: () => setPanel("queue") },
        { text: "Synchronisation…", action: () => setPanel("tune") },
        { text: "Equalizer…", action: () => setPanel("audio") },
        { text: "Picture…", action: () => setPanel("picture") },
      ],
    };
    const m = await Menu.new(items);
    await m.popup();
  }, [tracks, playing, togglePlay, opened, act, queue.length, prev, next, fullscreen, toggleFullscreen, status.rate, setRate, refreshDetails, pickSubtitle, hasVideo, aspect, applyAspect, deinterlace, applyDeinterlace, snapshot, pickFiles, pickFolder, favorite, current, toggleFavorite, mini, toggleMini]);

  // ---------- native surface + window drag & drop ----------
  const pollRef = useRef(poll);
  pollRef.current = poll;
  const refreshRef = useRef(refreshDetails);
  refreshRef.current = refreshDetails;
  const pictureRef = useRef({ adjust, crop });
  pictureRef.current = { adjust, crop };
  const handlers = useRef({ togglePlay, toggleFullscreen, showMenu, setVolume, handleDropped, wake, status });
  handlers.current = { togglePlay, toggleFullscreen, showMenu, setVolume, handleDropped, wake, status };
  useEffect(() => {
    const unSurface = listen<SurfaceEvent>("surface", ({ payload: e }) => {
      const h = handlers.current;
      switch (e.kind) {
        case "down": getCurrentWebview().setFocus().catch(() => {}); h.wake(); break;
        case "click": break; // single click reserved (VLC: none) — avoids pausing on double-click
        case "doubleclick": h.toggleFullscreen(); break;
        case "context": h.showMenu(); break;
        case "move": h.wake(); break;
        case "wheel": h.setVolume(h.status.volume + (e.delta > 0 ? 5 : -5)); break;
        case "drop": h.handleDropped(e.paths); break;
      }
    });
    const unEngine = listen<EngineEvent>("engine", ({ payload }) => {
      pollRef.current();
      if (payload === "tracks" || payload === "vout" || payload === "length") refreshRef.current();
      // A new video output (new file, engine restart, rotation on/off) starts without the
      // picture settings: the Direct3D11 output applies them in its own display stage.
      if (payload === "vout") {
        const pic = pictureRef.current;
        if (pic.adjust) api.setVideoAdjust(pic.adjust).catch(() => {});
        if (pic.crop) api.setCrop(pic.crop).catch(() => {});
      }
    });
    const unDrop = getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload;
      if (p.type === "enter" || p.type === "over") setDragOver(true);
      else if (p.type === "leave") setDragOver(false);
      else if (p.type === "drop") { setDragOver(false); handlers.current.handleDropped(p.paths); }
    });
    return () => { unSurface.then((f) => f()); unDrop.then((f) => f()); unEngine.then((f) => f()); };
  }, []);

  // ---------- keep the native video surface glued to the reserved box ----------
  const surfaceVisible = view === "player" && hasVideo && !urlOpen && !dragOver && !panelTakesOver;
  // Fullscreen and the mini player always show the player.
  useEffect(() => { if (fullscreen || mini) setView("player"); }, [fullscreen, mini]);
  const clipControls = fullscreen && controlsShown;
  useLayoutEffect(() => {
    const el = videoRef.current;
    if (!el || !surfaceVisible) { engine.setVideoRect({ x: 0, y: 0, w: 0, h: 0, visible: false }); return; }
    let last = "";
    let raf = 0;
    const sync = () => {
      // devicePixelRatio (not the window scale factor) is authoritative: WebView2
      // also applies the Windows text-size setting on top of monitor DPI.
      const r = el.getBoundingClientRect();
      const k = window.devicePixelRatio;
      // Fullscreen controls float over the bottom of the box: keep the video above them.
      const bar = clipControls && transportRef.current ? transportRef.current.getBoundingClientRect().height : 0;
      const px = { x: Math.round(r.left * k), y: Math.round(r.top * k), w: Math.round(r.width * k), h: Math.round((r.height - bar) * k), visible: true };
      const key = JSON.stringify(px);
      if (key !== last) { last = key; engine.setVideoRect(px); }
      raf = requestAnimationFrame(sync);
    };
    sync();
    return () => cancelAnimationFrame(raf);
  }, [surfaceVisible, clipControls]);
  // Re-apply "stretch to window" when the box changes shape.
  useEffect(() => {
    if (aspect !== "fill" || !videoRef.current) return;
    const ro = new ResizeObserver(() => {
      const r = videoRef.current!.getBoundingClientRect();
      engine.setAspectRatio(`${Math.round(r.width)}:${Math.round(r.height)}`);
    });
    ro.observe(videoRef.current);
    return () => ro.disconnect();
  }, [aspect, hasVideo]);

  // ---------- keyboard (VLC-style) ----------
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const tgt = e.target as HTMLElement;
      if (tgt.closest("input[type=text], input[type=url], input[type=number], textarea, select")) return;
      const k = e.key;
      const ctrl = e.ctrlKey || e.metaKey;
      wake();
      if (ctrl && k.toLowerCase() === "o") { e.preventDefault(); e.shiftKey ? pickFolder() : pickFiles(); return; }
      if (ctrl && k.toLowerCase() === "n") { e.preventDefault(); setUrlOpen(true); return; }
      if (ctrl && k.toLowerCase() === "q") { e.preventDefault(); getCurrentWindow().close(); return; }
      if (ctrl && k.toLowerCase() === "l") { e.preventDefault(); setView("library"); return; }
      if (ctrl && k.toLowerCase() === "f") { e.preventDefault(); toggleFavorite(); return; }
      if (ctrl && k === "ArrowUp") { e.preventDefault(); setVolume(status.volume + 5); return; }
      if (ctrl && k === "ArrowDown") { e.preventDefault(); setVolume(status.volume - 5); return; }
      if (ctrl || e.altKey) return;
      let handled = true;
      switch (k) {
        case " ": togglePlay(); break; // preventDefault below also stops a focused button re-firing
        case "f": case "F": case "F11": toggleFullscreen(); break;
        case "Escape": if (fullscreen) toggleFullscreen(false); else if (urlOpen) setUrlOpen(false); else if (mini) toggleMini(false); else handled = false; break;
        case "ArrowLeft": seekBy(e.shiftKey ? -3000 : -10000); break;
        case "ArrowRight": seekBy(e.shiftKey ? 3000 : 10000); break;
        case "ArrowUp": setVolume(status.volume + 5); break;
        case "ArrowDown": setVolume(status.volume - 5); break;
        case "m": case "M": toggleMute(); break;
        case "n": case "N": next(); break;
        case "p": case "P": prev(); break;
        case "s": act(engine.stop()); break;
        case "S": snapshot(); break;
        case "e": case ".": act(engine.nextFrame()); break;
        case "[": case "-": stepRate(-1); break;
        case "]": case "+": stepRate(1); break;
        case "=": setRate(1); break;
        case "j": delay("audio", -50); break;
        case "k": delay("audio", 50); break;
        case "g": delay("subtitle", -50); break;
        case "h": delay("subtitle", 50); break;
        case "b": cycleTrack("audio"); break;
        case "v": cycleTrack("subtitle"); break;
        case "a": applyAspect(ASPECTS[(ASPECTS.indexOf(aspect) + 1) % ASPECTS.length]); break;
        case "l": case "L": setRepeat((r) => (r === "off" ? "all" : r === "all" ? "one" : "off")); break;
        case "r": case "R": setShuffle((s) => !s); break;
        case "i": case "I": openPanel("info"); break;
        case "q": case "Q": openPanel("queue"); break;
        case "t": case "T": openPanel("tune"); break;
        case "Home": seekTo(0); break;
        case "End": if (lengthMs) seekTo(lengthMs - 1000); break;
        default:
          if (/^[0-9]$/.test(k) && lengthMs) seekTo((Number(k) / 10) * lengthMs);
          else handled = false;
      }
      if (handled) e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  // ---------- splitters ----------
  const startResize = (which: "side" | "panel") => (e: React.PointerEvent) => {
    e.preventDefault();
    const x0 = e.clientX;
    const w0 = which === "side" ? sideW : panelW;
    const move = (ev: PointerEvent) => {
      const d = ev.clientX - x0;
      if (which === "side") {
        const w = w0 + d;
        if (w < 120) setSideCollapsed(true);
        else { setSideCollapsed(false); setSideW(clamp(w, 160, 360)); }
      } else setPanelW(clamp(w0 - d, 260, Math.max(300, window.innerWidth * 0.6)));
    };
    const up = () => { window.removeEventListener("pointermove", move); window.removeEventListener("pointerup", up); document.body.classList.remove("resizing"); };
    document.body.classList.add("resizing");
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  // ---------- render ----------
  const title = meta?.title ?? (current ? basename(current) : "Nothing playing");
  const subtitle = [meta?.artist, meta?.album].filter(Boolean).join(" — ") ||
    (opened?.probe?.format.format_long_name ?? (current && /:\/\//.test(current) ? "Network stream" : ""));
  const art = meta?.artwork_url?.startsWith("file:///") ? convertFileSrc(decodeURIComponent(meta.artwork_url.slice(8))) : null;
  const engineMissing = diag && !diag.engine;
  const repeatLabel = repeat === "one" ? "Repeat one" : repeat === "all" ? "Repeat all" : "Repeat off";
  const effVolume = status.muted ? 0 : Math.max(0, status.volume);

  return (
    <div className={`app${fullscreen ? " fs" : ""}${fullscreen && !controlsShown ? " idle" : ""}${mini ? " mini" : ""}${mini && !hasVideo ? " mini-audio" : ""}`}
      // WebView2 fires synthetic zero-movement pointermoves after layout changes (e.g. the
      // controls hiding); only real movement may reveal them again.
      onPointerMove={fullscreen ? (e) => { if (e.movementX || e.movementY) wake(); } : undefined}>
      <div className="body">
        {!sidebarHidden && (
          <>
            <nav className={`sidebar${sidebarMini ? " mini" : ""}`} style={{ width: sidebarMini ? 56 : sideW }} aria-label="Primary">
              <div className="brand">
                <button className="icon-btn" onClick={() => setSideCollapsed((c) => !c)} aria-label="Toggle sidebar" title="Toggle sidebar"><Icon name="menu" /></button>
                <span className="brand-name">Halcyon</span>
              </div>
              <button className="nav-item" aria-current={view === "player" ? "page" : undefined} onClick={() => setView("player")} title="Now playing">
                <Icon name="disc" /><span className="nav-label">Now playing</span>
              </button>
              <button className="nav-item" onClick={pickFiles} title="Open files (Ctrl+O)"><Icon name="open" /><span className="nav-label">Open files</span></button>
              <button className="nav-item" onClick={pickFolder} title="Open folder (Ctrl+Shift+O)"><Icon name="folder" /><span className="nav-label">Open folder</span></button>
              <button className="nav-item" onClick={() => setUrlOpen(true)} title="Open network stream (Ctrl+N)"><Icon name="network" /><span className="nav-label">Network stream</span></button>
              <div className="nav-sep" />
              {NAV.map((n) => (
                <button key={n.id} className="nav-item" aria-current={view === n.id ? "page" : undefined} onClick={() => setView(n.id)} title={n.label}>
                  <Icon name={n.icon} /><span className="nav-label">{n.label}</span>
                </button>
              ))}
              <div className="spacer" />
              <button className="nav-item" aria-current={view === "settings" ? "page" : undefined} onClick={() => setView("settings")} title="Settings">
                <Icon name="settings" /><span className="nav-label">Settings</span>
              </button>
            </nav>
            {!sidebarMini && <div className="splitter" onPointerDown={startResize("side")} role="separator" aria-orientation="vertical" aria-label="Resize sidebar" />}
          </>
        )}

        {!panelTakesOver && (
          <main className="stage">
            {(error || engineMissing) && (
              <div className="banner" role="alert">
                <span>{error ?? diag?.engine_error}</span>
                <button className="icon-btn" onClick={() => setError(null)} aria-label="Dismiss"><Icon name="close" /></button>
              </div>
            )}
            {iamf && (
              <div className="banner info" role="status">
                <span>Immersive audio (IAMF / Eclipsa) detected. libVLC 3 plays only one substream as plain audio; the mix is not rendered.</span>
                <button className="btn small" onClick={() => setView("lab")}>Open in Audio Lab</button>
              </div>
            )}
            {view === "settings" ? (
              <SettingsPage diag={diag} cfg={cfg} onCfg={applyConfig} theme={theme} onTheme={setTheme} accent={accent} onAccent={setAccent}
                devices={devices} device={device} onDevice={(id) => { setDevice(id); api.setAudioDevice(id).catch(() => {}); }}
                privateMode={privateMode} onPrivate={setPrivateMode} resume={resumeOn} onResume={setResumeOn} onMini={() => toggleMini(true)} />
            ) : view === "library" ? <LibraryPage actions={actions} />
            : view === "music" ? <MusicPage actions={actions} />
            : view === "videos" ? <VideosPage actions={actions} />
            : view === "playlists" ? <PlaylistsPage actions={actions} queue={queue} />
            : view === "network" ? <NetworkPage actions={actions} />
            : view === "stats" ? <StatsPage privateMode={privateMode} setPrivateMode={setPrivateMode} />
            : view === "lab" ? (
              <LabPage current={current} timeMs={status.time_ms} onAudition={openAt} onSeek={seekTo} onLevel={setLabGain} say={say} />
            ) : view === "convert" ? (
              <ConvertPage current={current} timeMs={status.time_ms} say={say} onPlay={(p) => actions.play([p])} />
            ) : !opened ? (
              <div className="empty">
                <h1>Play anything.</h1>
                <p>Drop files, folders or playlists here — or open them. Everything stays on this device.</p>
                <div className="row">
                  <button className="btn primary" onClick={pickFiles}><Icon name="open" /> Open files</button>
                  <button className="btn" onClick={pickFolder}><Icon name="folder" /> Folder</button>
                  <button className="btn" onClick={() => setUrlOpen(true)}><Icon name="network" /> Stream URL</button>
                </div>
                {recents.length > 0 && (
                  <div className="recents">
                    <h2>Recent</h2>
                    {recents.slice(0, 8).map((r) => (
                      <button key={r} className="recent" onClick={() => playSources([r])} title={r}>
                        <span className="recent-name">{basename(r)}</span><span className="recent-path">{r}</span>
                      </button>
                    ))}
                  </div>
                )}
                <span className="hint"><span className="kbd">Space</span> play · <span className="kbd">F</span> fullscreen · <span className="kbd">←/→</span> seek · <span className="kbd">Q</span> queue · <span className="kbd">I</span> info · right-click video for more</span>
              </div>
            ) : hasVideo ? (
              <div className="video-box" ref={videoRef} onDoubleClick={() => toggleFullscreen()} onContextMenu={(e) => { e.preventDefault(); showMenu(); }}
                onWheel={(e) => setVolume(status.volume + (e.deltaY < 0 ? 5 : -5))} />
            ) : (
              <div className={"audio-hero" + (lyrics ? " with-lyrics" : "")} onContextMenu={(e) => { e.preventDefault(); showMenu(); }} onWheel={(e) => setVolume(status.volume + (e.deltaY < 0 ? 5 : -5))}>
                {art ? <img className="art" src={art} alt="" /> : <div className="art"><Icon name="music" /></div>}
                <div className={"hero-text" + (lyrics ? " has-lyrics" : "")}>
                  <h1>{title}</h1>
                  <div className="sub">{subtitle}</div>
                  {lyrics && <LyricsView lyrics={lyrics} timeMs={status.time_ms} onSeek={seekTo} />}
                </div>
              </div>
            )}
            {dragOver && <div className="drop-overlay"><Icon name="open" /><span>Drop to play · subtitles are added to the current video</span></div>}
            {urlOpen && <UrlDialog onClose={() => setUrlOpen(false)} onOpen={(u) => { setUrlOpen(false); playSources([u]); }} />}
          </main>
        )}

        {panelVisible && (
          <>
            {!tiny && <div className="splitter" onPointerDown={startResize("panel")} role="separator" aria-orientation="vertical" aria-label="Resize panel" />}
            <aside className="panel" style={{ width: tiny ? "100%" : panelW }}>
              <div className="tabs" role="tablist">
                {PANEL_TABS.map(([t, l]) => (
                  <button key={t} role="tab" aria-selected={panel === t} className="tab" onClick={() => setPanel(t)}>
                    {t === "queue" && queue.length ? l + " · " + queue.length : l}
                  </button>
                ))}
                <span className="spacer" />
                <button className="icon-btn" onClick={() => setPanel(null)} aria-label="Close panel"><Icon name="close" /></button>
              </div>
              <div className="panel-body">
                {panel === "queue" && (
                  <QueuePanel queue={queue} index={index}
                    onPlay={(i) => loadIndex(i, queue)}
                    onRemove={(i) => { setQueue((q) => q.filter((_, j) => j !== i)); if (i < index) setIndex(index - 1); }}
                    onMove={(i, d) => {
                      const j = i + d;
                      if (j < 0 || j >= queue.length) return;
                      setQueue((q) => { const c = [...q]; [c[i], c[j]] = [c[j], c[i]]; return c; });
                      if (index === i) setIndex(j); else if (index === j) setIndex(i);
                    }}
                    onClear={() => { setQueue(current ? [current] : []); setIndex(current ? 0 : -1); }}
                    onAdd={async () => {
                      const f = await openDialog({ multiple: true, filters: [{ name: "Media & playlists", extensions: MEDIA_EXT }] });
                      if (Array.isArray(f) && f.length) playSources(f, true);
                    }} />
                )}
                {panel === "info" && <Inspector opened={opened} meta={meta} diag={diag} stats={stats} videoSize={videoSize} status={status} />}
                {panel === "tune" && (
                  <TunePanel status={status} aspect={aspect} deinterlace={deinterlace} hasVideo={hasVideo}
                    onDelay={delay} onRate={setRate} onAspect={applyAspect} onDeinterlace={applyDeinterlace}
                    onAddSubtitle={pickSubtitle} onSnapshot={snapshot}
                    onChapter={(c) => act(engine.setChapter(c))}>
                    <SleepTimer endsAt={sleepAt} atEnd={sleepAtEnd}
                      onSet={(m, atEnd) => { setSleepAt(m ? Date.now() + m * 60000 : null); setSleepAtEnd(!!atEnd); if (m) say("Sleep timer: " + m + " min"); }} />
                  </TunePanel>
                )}
                {panel === "audio" && (
                  <AudioPanel info={eqInfo} eq={eq} onEq={setEq} devices={devices} device={device}
                    onDevice={(id) => { setDevice(id); api.setAudioDevice(id).catch(() => {}); }}
                    normalize={cfg.normalize} onNormalize={(v) => applyConfig({ ...cfg, normalize: v })} />
                )}
                {panel === "picture" && (
                  <PicturePanel adjust={adjust} onAdjust={(a) => { setAdjust(a); api.setVideoAdjust(a).catch(() => {}); }}
                    crop={crop} onCrop={(c) => { setCrop(c); api.setCrop(c).catch(() => {}); }}
                    transform={cfg.transform} onTransform={(t) => applyConfig({ ...cfg, transform: t })} hasVideo={hasVideo} />
                )}
              </div>
            </aside>
          </>
        )}
      </div>

      <footer className={`transport${fullscreen && !controlsShown ? " hidden" : ""}`} ref={transportRef}
        onPointerEnter={() => { overTransport.current = true; }} onPointerLeave={() => { overTransport.current = false; wake(); }}>
        <SeekBar status={viewStatus} receivedAt={receivedAt} onSeek={seekTo} chapters={chapters} />
        <div className="controls">
          <div className="now">
            {art ? <img className="thumb" src={art} alt="" /> : <div className="thumb"><Icon name={hasVideo ? "video" : "music"} /></div>}
            <div className="now-text">
              <div className="t" title={current ?? undefined}>{title}</div>
              <div className="a">{toast ? (
                <span className="toast" role="status">{toast.text}{toast.action && <button className="link" onClick={() => { toast.action!.run(); setToast(null); }}>{toast.action.label}</button>}</span>
              ) : (subtitle || " ")}</div>
            </div>
          </div>

          <div className="buttons">
            <button className="icon-btn hide-xs" onClick={() => setShuffle((s) => !s)} aria-pressed={shuffle} aria-label="Shuffle" title="Shuffle (R)"><Icon name="shuffle" /></button>
            <button className="icon-btn" onClick={prev} disabled={!opened} aria-label="Previous" title="Previous / restart (P)"><Icon name="prev" filled /></button>
            <button className="icon-btn hide-xs" onClick={() => seekBy(-10000)} disabled={!opened} aria-label="Back 10 seconds" title="Back 10 s (←)"><Icon name="back" /></button>
            <button className="icon-btn play" onClick={togglePlay} aria-label={playing ? "Pause" : "Play"} title="Play / pause (Space)">
              <Icon name={playing ? "pause" : "play"} filled />
            </button>
            <button className="icon-btn hide-xs" onClick={() => seekBy(10000)} disabled={!opened} aria-label="Forward 10 seconds" title="Forward 10 s (→)"><Icon name="fwd" /></button>
            <button className="icon-btn" onClick={next} disabled={queue.length < 2 && repeat !== "all"} aria-label="Next" title="Next (N)"><Icon name="next" filled /></button>
            <button className="icon-btn hide-xs" onClick={() => setRepeat((r) => (r === "off" ? "all" : r === "all" ? "one" : "off"))} aria-pressed={repeat !== "off"} aria-label={repeatLabel} title={`${repeatLabel} (L)`}>
              <Icon name={repeat === "one" ? "repeat1" : "repeat"} />
            </button>
          </div>

          <div className="right">
            <button className="icon-btn hide-xs" onClick={toggleFavorite} disabled={!current} aria-pressed={favorite} aria-label={favorite ? "Remove from favourites" : "Add to favourites"} title="Favourite (Ctrl+F)">
              <Icon name="heart" filled={favorite} />
            </button>
            <button className="chip hide-sm" onClick={() => setRate(status.rate === 1 ? 1.5 : 1)} title="Speed — [ ] to step, = to reset" disabled={!opened}>{Number(status.rate.toFixed(2))}×</button>
            <button className="icon-btn" onClick={toggleMute} aria-pressed={status.muted} aria-label={status.muted ? "Unmute" : "Mute"} title="Mute (M)">
              <Icon name={effVolume === 0 ? "mute" : "volume"} />
            </button>
            <input className="volume hide-sm" type="range" aria-label="Volume" min={0} max={150} value={Math.min(150, effVolume)}
              style={{ ["--fill" as string]: `${(Math.min(150, effVolume) / 150) * 100}%` }}
              onChange={(e) => setVolume(Number(e.target.value))}
              onWheel={(e) => setVolume(status.volume + (e.deltaY < 0 ? 5 : -5))} title={`${effVolume}%`} />
            {mini ? (
              <button className="icon-btn" onClick={() => toggleMini(false)} aria-label="Leave mini player" title="Back to full window (Esc)">
                <Icon name="expand" />
              </button>
            ) : (
              <>
                <button className="icon-btn" onClick={() => openPanel("queue")} aria-pressed={panel === "queue"} aria-label="Queue" title="Queue (Q)"><Icon name="list" /></button>
                <button className="icon-btn hide-xs" onClick={() => openPanel("info")} aria-pressed={panel === "info"} aria-label="Media info" title="Media info (I)"><Icon name="panel" /></button>
                <button className="icon-btn hide-xs" onClick={() => toggleMini(true)} aria-label="Mini player" title="Mini player (always on top)"><Icon name="mini" /></button>
                <button className="icon-btn" onClick={() => toggleFullscreen()} aria-label={fullscreen ? "Exit fullscreen" : "Fullscreen"} title="Fullscreen (F, double-click)">
                  <Icon name={fullscreen ? "unfull" : "full"} />
                </button>
              </>
            )}
          </div>
        </div>
      </footer>
    </div>
  );
}

function UrlDialog({ onOpen, onClose }: { onOpen: (u: string) => void; onClose: () => void }) {
  const [v, setV] = useState("");
  const ok = /^(https?|rtsp|rtp|udp|mms|ftp|sftp|smb|nfs|srt):\/\//i.test(v.trim());
  return (
    <div className="modal-backdrop" onClick={onClose}>
      <form className="modal" onClick={(e) => e.stopPropagation()} onSubmit={(e) => { e.preventDefault(); if (ok) onOpen(v.trim()); }}>
        <h2>Open network stream</h2>
        <p className="muted">HTTP(S), HLS (.m3u8), DASH (.mpd), RTSP, RTP/UDP, SMB, FTP/SFTP. The address is passed to the engine as-is.</p>
        <input autoFocus type="url" className="input" placeholder="https://example.com/live/stream.m3u8" value={v} onChange={(e) => setV(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Escape") onClose(); }} />
        <div className="row end">
          <button type="button" className="btn" onClick={onClose}>Cancel</button>
          <button type="submit" className="btn primary" disabled={!ok}>Play</button>
        </div>
      </form>
    </div>
  );
}

function LyricsView({ lyrics, timeMs, onSeek }: { lyrics: Lyrics; timeMs: number; onSeek: (ms: number) => void }) {
  const ref = useRef<HTMLDivElement>(null);
  let idx = -1;
  for (let i = 0; i < lyrics.synced.length && lyrics.synced[i][0] <= timeMs + 150; i++) idx = i;
  useEffect(() => {
    const el = ref.current?.querySelector<HTMLElement>(".ly.on");
    const box = ref.current;
    if (!el || !box) return;
    // Centre the current line inside the lyrics box (offsetTop is relative to the page
    // layout, not the box, which scrolled the current line out of view).
    const top = el.getBoundingClientRect().top - box.getBoundingClientRect().top + box.scrollTop;
    box.scrollTo({ top: top - box.clientHeight / 2 + el.clientHeight / 2, behavior: "smooth" });
  }, [idx]);
  return (
    <div className="lyrics" ref={ref} aria-label={"Lyrics (" + lyrics.source + ")"}>
      {lyrics.synced.length
        ? lyrics.synced.map(([t, line], i) => (
          <button key={i} className={"ly" + (i === idx ? " on" : "")} onClick={() => onSeek(t)}>{line || "♪"}</button>
        ))
        : <pre className="ly-plain">{lyrics.plain}</pre>}
    </div>
  );
}
