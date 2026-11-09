import { useEffect, useState } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { api, basename, formatTime, type JobEvent, type Preset } from "./engine";
import { Icon } from "./icons";

interface Job { id: number; name: string; output: string; progress: number; done: boolean; error: string | null }

export function ConvertPage({ current, timeMs, say, onPlay }: { current: string | null; timeMs: number; say: (t: string) => void; onPlay: (p: string) => void }) {
  const [presets, setPresets] = useState<Preset[]>([]);
  const [input, setInput] = useState<string | null>(current && !current.includes("://") ? current : null);
  const [preset, setPreset] = useState("mp4-h264");
  const [start, setStart] = useState<number | null>(null);
  const [end, setEnd] = useState<number | null>(null);
  const [height, setHeight] = useState<number | null>(null);
  const [normalize, setNormalize] = useState(false);
  const [jobs, setJobs] = useState<Job[]>([]);
  useEffect(() => { api.presets().then(setPresets); }, []);
  useEffect(() => {
    const un = listen<JobEvent>("convert-progress", ({ payload: e }) => {
      setJobs((js) => js.map((j) => (j.id === e.id ? { ...j, progress: e.progress, done: e.done, error: e.error } : j)));
      if (e.done) say(e.error ? `Conversion failed: ${e.error}` : `Converted: ${basename(e.output ?? "")}`);
    });
    return () => { un.then((f) => f()); };
  }, [say]);
  const p = presets.find((x) => x.id === preset);
  const start_ = async () => {
    if (!input || !p) return;
    const stem = basename(input).replace(/\.[^.]+$/, "");
    const out = await saveDialog({ defaultPath: `${stem}-${p.id}.${p.ext}`, filters: [{ name: p.label, extensions: [p.ext] }] });
    if (!out) return;
    const id = await api.convert(input, out, preset, { start_ms: start, end_ms: end, height, normalize });
    setJobs((js) => [{ id, name: basename(out), output: out, progress: 0, done: false, error: null }, ...js]);
  };
  const ms = (v: number | null) => (v === null ? "—" : formatTime(v));
  return (
    <section className="page">
      <div className="page-head"><h1>Convert</h1></div>
      <p className="muted small">Runs your installed FFmpeg in a separate process. Nothing is uploaded.</p>
      <div className="form">
        <label>Source</label>
        <div className="row">
          <span className="mono small grow ellipsis" title={input ?? ""}>{input ? basename(input) : "No file selected"}</span>
          <button className="btn small" onClick={async () => { const f = await openDialog({ multiple: false }); if (typeof f === "string") setInput(f); }}><Icon name="open" /> Choose…</button>
          {current && !current.includes("://") && current !== input && <button className="btn small" onClick={() => setInput(current)}>Now playing</button>}
        </div>
        <label>Format</label>
        <select className="select" value={preset} onChange={(e) => setPreset(e.target.value)}>
          {presets.map((x) => <option key={x.id} value={x.id}>{x.label}</option>)}
        </select>
        <label>Trim</label>
        <div className="row">
          <span className="mono small">{ms(start)} → {ms(end)}</span>
          <button className="btn small" disabled={input !== current} onClick={() => setStart(timeMs)} title="Use the playhead">Start = playhead</button>
          <button className="btn small" disabled={input !== current} onClick={() => setEnd(timeMs)}>End = playhead</button>
          <button className="btn small" onClick={() => { setStart(null); setEnd(null); }}>Whole file</button>
        </div>
        {p && !p.audio_only && p.id !== "mkv-remux" && (
          <>
            <label>Resolution</label>
            <select className="select" value={height ?? ""} onChange={(e) => setHeight(e.target.value ? Number(e.target.value) : null)}>
              <option value="">Keep</option>{[2160, 1440, 1080, 720, 480, 360].map((h) => <option key={h} value={h}>{h}p</option>)}
            </select>
          </>
        )}
        {p?.id !== "mkv-remux" && (
          <>
            <label>Loudness</label>
            <label className="check"><input type="checkbox" checked={normalize} onChange={(e) => setNormalize(e.target.checked)} /> Normalise to −16 LUFS / −1 dBTP</label>
          </>
        )}
      </div>
      <div className="row">
        <button className="btn primary" disabled={!input || (start !== null && end !== null && end <= start)} onClick={start_}>Convert…</button>
      </div>
      {jobs.length > 0 && <h2 className="section">Jobs</h2>}
      <div className="list">
        {jobs.map((j) => (
          <div key={j.id} className="list-row static">
            <span className="lr-t grow ellipsis" title={j.output}>{j.name}</span>
            {j.error ? <span className="error small">{j.error}</span> : j.done ? (
              <button className="btn small" onClick={() => onPlay(j.output)}><Icon name="play" filled /> Play</button>
            ) : (
              <>
                <progress max={1} value={j.progress} />
                <span className="mono small">{Math.round(j.progress * 100)}%</span>
                <button className="btn small" onClick={() => api.cancelJob(j.id)}>Cancel</button>
              </>
            )}
          </div>
        ))}
      </div>
    </section>
  );
}
