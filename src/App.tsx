import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  engine, formatTime, basename,
  type Diagnostics, type EngineMeta, type OpenResult, type PlaybackStatus, type TrackSelection,
} from "./engine";
import { Icon } from "./icons";
import { Inspector } from "./Inspector";

type View = "player" | "settings";

const NAV: { id: string; label: string; icon: string; phase?: number }[] = [
  { id: "home", label: "Home", icon: "home", phase: 2 },
  { id: "library", label: "Library", icon: "library", phase: 2 },
  { id: "music", label: "Music", icon: "music", phase: 6 },
  { id: "videos", label: "Videos", icon: "video", phase: 6 },
  { id: "playlists", label: "Playlists", icon: "list", phase: 3 },
  { id: "favorites", label: "Favorites", icon: "heart", phase: 6 },
  { id: "recent", label: "Recently played", icon: "clock", phase: 2 },
  { id: "lab", label: "Immersive Audio Lab", icon: "lab", phase: 5 },
  { id: "network", label: "Network sources", icon: "network", phase: 4 },
];

const MEDIA_EXT = [
  "mp4", "m4v", "mkv", "avi", "mov", "webm", "flv", "wmv", "asf", "ts", "m2ts", "mts", "mpg", "mpeg", "vob",
  "3gp", "ogg", "ogm", "ogv", "mxf", "mp3", "flac", "wav", "aif", "aiff", "aac", "m4a", "alac", "opus", "oga",
  "wma", "ape", "wv", "ac3", "eac3", "dts", "thd", "mka", "mid", "midi", "iamf",
];
const RATES = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 1.75, 2, 3];
const IDLE: PlaybackStatus = { state: "idle", time_ms: 0, length_ms: 0, volume: 100, rate: 1 };

export default function App() {
  const [view, setView] = useState<View>("player");
  const [collapsed, setCollapsed] = useState(false);
  const [showInspector, setShowInspector] = useState(false);
  const [diag, setDiag] = useState<Diagnostics | null>(null);
  const [opened, setOpened] = useState<OpenResult | null>(null);
  const [status, setStatus] = useState<PlaybackStatus>(IDLE);
  const [tracks, setTracks] = useState<TrackSelection | null>(null);
  const [meta, setMeta] = useState<EngineMeta | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [scrub, setScrub] = useState<number | null>(null);
  const videoRef = useRef<HTMLDivElement>(null);

  const hasVideo = !!opened && (opened.probe
    ? opened.probe.streams.some((s) => s.codec_type === "video" && !s.disposition?.attached_pic)
    : true); // unknown → give the engine a surface
  const isPlaying = status.state === "playing" || status.state === "opening" || status.state === "buffering";

  useEffect(() => { engine.diagnostics().then(setDiag); }, []);

  const refreshDetails = useCallback(() => {
    engine.tracks().then(setTracks).catch(() => {});
    engine.meta().then(setMeta).catch(() => {});
  }, []);

  const openPath = useCallback(async (path: string) => {
    setError(null);
    setTracks(null);
    setMeta(null);
    try {
      const r = await engine.open(path);
      setOpened(r);
      setView("player");
      // Elementary streams and tags become available once the input thread has started.
      setTimeout(refreshDetails, 600);
      setTimeout(refreshDetails, 2000);
    } catch (e) {
      setError(String(e));
    }
  }, [refreshDetails]);

  useEffect(() => { engine.startupFile().then((f) => { if (f) openPath(f); }); }, [openPath]);

  const pickFile = useCallback(async () => {
    const f = await openDialog({
      multiple: false,
      filters: [{ name: "Media", extensions: MEDIA_EXT }, { name: "All files", extensions: ["*"] }],
    });
    if (typeof f === "string") openPath(f);
  }, [openPath]);

  // Real engine state, polled; the engine is the single source of truth.
  useEffect(() => {
    const t = setInterval(() => { engine.status().then(setStatus).catch(() => {}); }, 250);
    return () => clearInterval(t);
  }, []);

  useEffect(() => {
    if (status.state === "error") setError("The engine could not play this media.");
  }, [status.state]);

  // Drag & drop onto the window.
  useEffect(() => {
    const un = getCurrentWebview().onDragDropEvent((e) => {
      if (e.payload.type === "drop" && e.payload.paths.length) openPath(e.payload.paths[0]);
    });
    return () => { un.then((f) => f()); };
  }, [openPath]);

  // Keep the native video surface glued to the reserved box.
  useLayoutEffect(() => {
    const el = videoRef.current;
    const visible = view === "player" && hasVideo && !!el;
    if (!el) { engine.setVideoRect({ x: 0, y: 0, w: 0, h: 0, visible: false }); return; }
    // Position can change without a resize (sidebar, banners, fonts), so compare
    // every frame and only cross the IPC boundary when the rectangle moved.
    let last = "";
    let raf = 0;
    const sync = () => {
      // devicePixelRatio (not the window scale factor) is authoritative: WebView2
      // also applies the Windows text-size setting on top of monitor DPI.
      const r = el.getBoundingClientRect();
      const k = window.devicePixelRatio;
      const px = { x: Math.round(r.left * k), y: Math.round(r.top * k), w: Math.round(r.width * k), h: Math.round(r.height * k), visible };
      const key = JSON.stringify(px);
      if (key !== last) {
        last = key;
        engine.setVideoRect(px);
      }
      raf = requestAnimationFrame(sync);
    };
    sync();
    return () => cancelAnimationFrame(raf);
  }, [view, hasVideo]);

  const togglePlay = useCallback(() => {
    if (!opened) return void pickFile();
    if (status.state === "stopped" || status.state === "ended") return void engine.play();
    engine.setPaused(isPlaying);
  }, [opened, status.state, isPlaying, pickFile]);

  const seekBy = useCallback((d: number) => {
    if (status.length_ms > 0) engine.seek(Math.min(status.length_ms, Math.max(0, status.time_ms + d)));
  }, [status]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement).closest("input, select, textarea")) return;
      if (e.ctrlKey && e.key.toLowerCase() === "o") { e.preventDefault(); pickFile(); return; }
      switch (e.key) {
        case " ": e.preventDefault(); togglePlay(); break;
        case "ArrowLeft": seekBy(e.shiftKey ? -30000 : -5000); break;
        case "ArrowRight": seekBy(e.shiftKey ? 30000 : 5000); break;
        case "ArrowUp": engine.setVolume(Math.min(200, status.volume + 5)); break;
        case "ArrowDown": engine.setVolume(Math.max(0, status.volume - 5)); break;
        case ".": engine.nextFrame(); break;
        case "i": setShowInspector((v) => !v); break;
        case "s": engine.stop(); break;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [togglePlay, seekBy, pickFile, status.volume]);

  const title = meta?.title ?? (opened ? basename(opened.path) : "Nothing playing");
  const subtitle = [meta?.artist, meta?.album].filter(Boolean).join(" — ") ||
    (opened?.probe?.format.format_long_name ?? "");
  const art = meta?.artwork_url?.startsWith("file:///")
    ? convertFileSrc(decodeURIComponent(meta.artwork_url.slice(8)))
    : null;
  const pos = scrub ?? status.time_ms;
  const fill = status.length_ms ? `${(pos / status.length_ms) * 100}%` : "0%";
  const engineMissing = diag && !diag.engine;

  return (
    <div className={`app${collapsed ? " collapsed" : ""}`}>
      <nav className="sidebar" aria-label="Primary">
        <div className="brand">
          <button className="icon-btn" onClick={() => setCollapsed((c) => !c)} aria-label="Toggle sidebar"><Icon name="menu" /></button>
          <span className="brand-name">Halcyon</span>
        </div>
        <button className="nav-item" aria-current={view === "player" ? "page" : undefined} onClick={() => setView("player")} title="Now playing">
          <Icon name="disc" /><span className="nav-label">Now playing</span>
        </button>
        <div className="nav-sep" />
        {NAV.map((n) => (
          <button key={n.id} className="nav-item" disabled title={`${n.label} — planned for phase ${n.phase}`}>
            <Icon name={n.icon} /><span className="nav-label">{n.label}</span><span className="soon">P{n.phase}</span>
          </button>
        ))}
        <div className="spacer" />
        <button className="nav-item" aria-current={view === "settings" ? "page" : undefined} onClick={() => setView("settings")} title="Settings">
          <Icon name="settings" /><span className="nav-label">Settings</span>
        </button>
      </nav>

      <main className="main">
        {view === "player" ? (
          <section className="stage">
            {(error || engineMissing) && <div className="banner" role="alert">{error ?? diag?.engine_error}</div>}
            {!opened ? (
              <div className="empty">
                <h1>Play anything.</h1>
                <p>Open a file or drop it here. Playback and inspection happen on this device.</p>
                <button className="btn primary" onClick={pickFile}><Icon name="open" /> Open media</button>
                <span className="hint"><span className="kbd">Ctrl O</span> open · <span className="kbd">Space</span> play/pause · <span className="kbd">I</span> inspector</span>
              </div>
            ) : hasVideo ? (
              <div className="video-box" ref={videoRef} onDoubleClick={togglePlay} />
            ) : (
              <div className="audio-hero">
                {art ? <img className="art" src={art} alt="" /> : <div className="art"><Icon name="music" /></div>}
                <div>
                  <h1>{title}</h1>
                  <div className="sub">{subtitle}</div>
                </div>
              </div>
            )}
          </section>
        ) : (
          <Settings diag={diag} />
        )}
        {view === "player" && showInspector && <Inspector opened={opened} meta={meta} diag={diag} />}
      </main>

      <footer className="transport">
        <div className="now">
          {art ? <img className="thumb" src={art} alt="" /> : <div className="thumb" />}
          <div style={{ minWidth: 0 }}>
            <div className="t" title={title}>{title}</div>
            <div className="a">{subtitle || " "}</div>
          </div>
        </div>

        <div className="center">
          <div className="buttons">
            <button className="icon-btn" onClick={() => seekBy(-10000)} disabled={!opened} aria-label="Back 10 seconds" title="Back 10 s (←)"><Icon name="back" /></button>
            <button className="icon-btn" onClick={() => engine.stop()} disabled={!opened} aria-label="Stop" title="Stop (S)"><Icon name="stop" filled /></button>
            <button className="icon-btn play" onClick={togglePlay} aria-label={isPlaying ? "Pause" : "Play"} title="Play/pause (Space)">
              <Icon name={isPlaying ? "pause" : "play"} filled />
            </button>
            <button className="icon-btn" onClick={() => engine.nextFrame()} disabled={!hasVideo} aria-label="Next frame" title="Next frame (.)"><Icon name="step" /></button>
            <button className="icon-btn" onClick={() => seekBy(10000)} disabled={!opened} aria-label="Forward 10 seconds" title="Forward 10 s (→)"><Icon name="fwd" /></button>
          </div>
          <div className="seek">
            <span>{formatTime(pos)}</span>
            <input type="range" aria-label="Seek" min={0} max={status.length_ms || 1} step={100} value={pos}
              disabled={!status.length_ms} style={{ ["--fill" as string]: fill }}
              onChange={(e) => setScrub(Number(e.target.value))}
              onPointerUp={(e) => { engine.seek(Number((e.target as HTMLInputElement).value)); setScrub(null); }}
              onKeyUp={(e) => { engine.seek(Number((e.target as HTMLInputElement).value)); setScrub(null); }} />
            <span>{status.length_ms ? formatTime(status.length_ms) : "--:--"}</span>
          </div>
        </div>

        <div className="right">
          <select className="select" aria-label="Playback speed" value={RATES.includes(Number(status.rate.toFixed(2))) ? Number(status.rate.toFixed(2)) : 1}
            onChange={(e) => engine.setRate(Number(e.target.value)).catch((x) => setError(String(x)))} disabled={!opened}>
            {RATES.map((r) => <option key={r} value={r}>{r}×</option>)}
          </select>
          {tracks && tracks.audio.length > 2 && (
            <select className="select" aria-label="Audio track" value={tracks.current_audio}
              onChange={(e) => engine.setAudioTrack(Number(e.target.value)).then(refreshDetails)}>
              {tracks.audio.map((t) => <option key={t.id} value={t.id}>{t.name}</option>)}
            </select>
          )}
          {tracks && tracks.subtitle.length > 0 && (
            <select className="select" aria-label="Subtitles" value={tracks.current_subtitle}
              onChange={(e) => engine.setSubtitleTrack(Number(e.target.value)).then(refreshDetails)}>
              {tracks.subtitle.map((t) => <option key={t.id} value={t.id}>{t.id === -1 ? "Subtitles off" : t.name}</option>)}
            </select>
          )}
          <button className="icon-btn" onClick={() => engine.setVolume(status.volume > 0 ? 0 : 100)} aria-label="Mute" title="Mute">
            <Icon name={status.volume > 0 ? "volume" : "mute"} />
          </button>
          <input className="volume" type="range" aria-label="Volume" min={0} max={125} value={Math.max(0, status.volume)}
            style={{ ["--fill" as string]: `${(Math.max(0, status.volume) / 125) * 100}%` }}
            onChange={(e) => engine.setVolume(Number(e.target.value))} />
          <button className="icon-btn" onClick={() => setShowInspector((v) => !v)} aria-pressed={showInspector} aria-label="Inspector" title="Inspector (I)">
            <Icon name="panel" />
          </button>
        </div>
      </footer>
    </div>
  );
}

function Settings({ diag }: { diag: Diagnostics | null }) {
  return (
    <section className="page">
      <h1>Engine & compatibility</h1>
      <div className="inspector" style={{ width: "auto", border: 0, padding: 0, background: "none" }}>
        <h2>Playback engine</h2>
        <dl className="kv">
          <dt>Engine</dt><dd>{diag?.engine ? `${diag.engine.name} ${diag.engine.version}` : "Not loaded"}</dd>
          {diag?.engine && (<><dt>Location</dt><dd>{diag.engine.location}</dd></>)}
          {diag?.engine_error && (<><dt>Error</dt><dd>{diag.engine_error}</dd></>)}
        </dl>
        <h2>Inspection</h2>
        <dl className="kv">
          <dt>ffprobe</dt><dd>{diag?.ffprobe ?? "Not found on PATH"}</dd>
        </dl>
        <p className="muted" style={{ marginTop: 24 }}>
          Decoder, plugin and hardware-acceleration listings are planned for phase 3 (see FEATURE_MATRIX.md).
        </p>
      </div>
    </section>
  );
}
