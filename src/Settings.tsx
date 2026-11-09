import { Fragment, useEffect, useState } from "react";
import { api, type AudioDevice, type Diagnostics, type EngineConfig } from "./engine";

export const ACCENTS = ["#7d9bff", "#2f5bd3", "#16a34a", "#d97706", "#db2777", "#7c3aed", "#0891b2"];

function hex(n: number) {
  return `#${(n & 0xffffff).toString(16).padStart(6, "0")}`;
}

export function SettingsPage(p: {
  diag: Diagnostics | null;
  cfg: EngineConfig; onCfg: (c: EngineConfig) => void;
  theme: string; onTheme: (t: string) => void;
  accent: string | null; onAccent: (a: string | null) => void;
  devices: AudioDevice[]; device: string; onDevice: (id: string) => void;
  privateMode: boolean; onPrivate: (v: boolean) => void;
  resume: boolean; onResume: (v: boolean) => void;
  onMini: () => void;
}) {
  const [plugins, setPlugins] = useState<[string, string[]][]>([]);
  useEffect(() => { api.plugins().then(setPlugins).catch(() => {}); }, []);
  const c = p.cfg;
  const set = (patch: Partial<EngineConfig>) => p.onCfg({ ...c, ...patch });
  const total = plugins.reduce((a, [, v]) => a + v.length, 0);
  const has = (cat: string, name: string) => plugins.some(([c2, v]) => c2 === cat && v.includes(name));
  return (
    <section className="page">
      <h1>Settings</h1>
      <div className="settings">
        <h2 className="section">Appearance</h2>
        <div className="form">
          <label>Theme</label>
          <div className="chips">
            {[["system", "System"], ["light", "Light"], ["dark", "Dark"]].map(([v, l]) => (
              <button key={v} className={`chip${p.theme === v ? " on" : ""}`} onClick={() => p.onTheme(v)}>{l}</button>
            ))}
          </div>
          <label>Accent</label>
          <div className="row">
            <button className={`swatch auto${p.accent === null ? " on" : ""}`} onClick={() => p.onAccent(null)} title="Default" aria-label="Default accent">A</button>
            {ACCENTS.map((a) => (
              <button key={a} className={`swatch${p.accent === a ? " on" : ""}`} style={{ background: a }} onClick={() => p.onAccent(a)} aria-label={`Accent ${a}`} />
            ))}
            <input type="color" aria-label="Custom accent" value={p.accent ?? "#7d9bff"} onChange={(e) => p.onAccent(e.target.value)} />
          </div>
          <label>Mini player</label>
          <div><button className="btn small" onClick={p.onMini}>Open mini player (always on top)</button></div>
        </div>

        <h2 className="section">Playback</h2>
        <div className="form">
          <label>Hardware decoding</label>
          <label className="check"><input type="checkbox" checked={c.hw_decode} onChange={(e) => set({ hw_decode: e.target.checked })} /> Use the GPU (D3D11VA/DXVA2) when possible</label>
          <label>Resume</label>
          <label className="check"><input type="checkbox" checked={p.resume} onChange={(e) => p.onResume(e.target.checked)} /> Continue long media where it stopped</label>
          <label>Loudness</label>
          <div className="col">
            <label className="check"><input type="checkbox" checked={c.normalize} onChange={(e) => set({ normalize: e.target.checked })} /> Volume normalisation</label>
            <label className="check"><input type="checkbox" checked={c.replay_gain} onChange={(e) => set({ replay_gain: e.target.checked })} /> Apply ReplayGain tags (track)</label>
          </div>
          <label>Visualiser (audio)</label>
          <select className="select" value={c.visualizer ?? ""} onChange={(e) => set({ visualizer: e.target.value || null })}>
            <option value="">None — show artwork</option>
            <option value="spectrum">Spectrum</option><option value="spectrometer">Spectrometer</option>
            <option value="scope">Oscilloscope</option><option value="vuMeter">VU meter</option>
            {has("visualization", "goom") && <option value="goom">Goom</option>}
          </select>
          <label>Output device</label>
          <select className="select" value={p.device} onChange={(e) => p.onDevice(e.target.value)}>
            <option value="">System default</option>
            {p.devices.filter((d) => d.id).map((d) => <option key={d.id} value={d.id}>{d.name}</option>)}
          </select>
        </div>

        <h2 className="section">Subtitles</h2>
        <div className="form">
          <label>Size</label>
          <div className="row"><input type="range" min={50} max={300} step={10} value={c.sub_scale} aria-label="Subtitle size"
            style={{ ["--fill" as string]: `${((c.sub_scale - 50) / 250) * 100}%` }} onChange={(e) => set({ sub_scale: Number(e.target.value) })} /><span className="mono small">{c.sub_scale}%</span></div>
          <label>Colour</label>
          <input type="color" aria-label="Subtitle colour" value={hex(c.sub_color)} onChange={(e) => set({ sub_color: parseInt(e.target.value.slice(1), 16) })} />
          <label>Outline</label>
          <div className="chips">{[[0, "None"], [2, "Thin"], [4, "Normal"], [6, "Thick"]].map(([v, l]) => (
            <button key={v} className={`chip${c.sub_outline === v ? " on" : ""}`} onClick={() => set({ sub_outline: Number(v) })}>{l}</button>
          ))}</div>
          <label>Background</label>
          <div className="row"><input type="range" min={0} max={255} value={c.sub_background} aria-label="Subtitle background opacity"
            style={{ ["--fill" as string]: `${(c.sub_background / 255) * 100}%` }} onChange={(e) => set({ sub_background: Number(e.target.value) })} /><span className="mono small">{Math.round((c.sub_background / 255) * 100)}%</span></div>
          <label>Bottom margin</label>
          <div className="row"><input type="range" min={0} max={200} step={5} value={c.sub_margin} aria-label="Subtitle margin"
            style={{ ["--fill" as string]: `${(c.sub_margin / 200) * 100}%` }} onChange={(e) => set({ sub_margin: Number(e.target.value) })} /><span className="mono small">{c.sub_margin}px</span></div>
        </div>
        <p className="muted small">Engine options restart playback at the same position when they change.</p>

        <h2 className="section">Privacy</h2>
        <div className="form">
          <label>History</label>
          <label className="check"><input type="checkbox" checked={p.privateMode} onChange={(e) => p.onPrivate(e.target.checked)} /> Private session: no recents, resume points or listening stats</label>
          <label>Network</label>
          <span className="small">Halcyon never fetches metadata or artwork online (libVLC <span className="mono">--no-metadata-network-access</span>). Network access happens only for streams and shares you open.</span>
        </div>

        <h2 className="section">Engine &amp; compatibility</h2>
        <dl className="kv">
          <dt>Engine</dt><dd>{p.diag?.engine ? `${p.diag.engine.name} ${p.diag.engine.version}` : "Not loaded"}</dd>
          {p.diag?.engine && (<><dt>Location</dt><dd>{p.diag.engine.location}</dd></>)}
          {p.diag?.engine_error && (<><dt>Error</dt><dd>{p.diag.engine_error}</dd></>)}
          <dt>ffprobe</dt><dd>{p.diag?.ffprobe ?? "Not found on PATH"}</dd>
          <dt>Plugin modules</dt><dd>{total} in {plugins.length} categories</dd>
          <dt>Key capabilities</dt>
          <dd>
            {[
              ["codec", "avcodec", "FFmpeg decoders"], ["codec", "dav1d", "AV1 (dav1d)"], ["codec", "libass", "ASS/SSA subtitles"],
              ["access", "smb", "SMB shares"], ["access", "sftp", "SFTP"], ["services_discovery", "upnp", "UPnP"],
              ["services_discovery", "microdns", "mDNS discovery"], ["audio_filter", "spatialaudio", "Ambisonics (spatialaudio)"],
              ["audio_output", "mmdevice", "WASAPI output"], ["codec", "fluidsynth", "MIDI (FluidSynth)"],
            ].map(([cat, n, l]) => <Fragment key={n}>{has(cat, n) ? "✓" : "✗"} {l}<br /></Fragment>)}
          </dd>
        </dl>
        <details className="plugins">
          <summary>All plugin modules</summary>
          {plugins.map(([cat, names]) => (
            <div key={cat} className="plugin-cat"><strong>{cat}</strong> <span className="muted small">({names.length})</span><p className="mono small">{names.join(" · ")}</p></div>
          ))}
        </details>

        <h2 className="section">Keyboard</h2>
        <dl className="kv">
          {[
            ["Space", "Play / pause"], ["F, F11, double-click", "Fullscreen"], ["Esc", "Leave fullscreen"],
            ["← / →", "Seek 10 s (Shift: 3 s)"], ["↑ / ↓, wheel", "Volume"], ["M", "Mute"], ["N / P", "Next / previous"],
            ["[ ] =", "Speed down / up / reset"], [". or E", "Next frame"], ["J / K", "Audio delay −/+ 50 ms"],
            ["G / H", "Subtitle delay −/+ 50 ms"], ["B / V", "Cycle audio / subtitle track"], ["A", "Cycle aspect ratio"],
            ["L / R", "Repeat / shuffle"], ["0–9", "Jump to 0–90 %"], ["Shift+S", "Snapshot"], ["Q / I / T", "Queue / info / tune panel"],
            ["Ctrl+O, Ctrl+Shift+O, Ctrl+N", "Open files / folder / stream"], ["Ctrl+L", "Library"], ["Ctrl+F", "Favourite current item"],
          ].map(([k, v]) => (<Fragment key={k}><dt>{k}</dt><dd>{v}</dd></Fragment>))}
        </dl>
      </div>
    </section>
  );
}
