import { useEffect, useMemo, useRef, useState } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { api, basename, formatTime, type Analysis, type LabEvent, type ProbeResult } from "./engine";
import { Icon } from "./icons";

const fmt = (v: number, unit: string) => (v <= -199 ? "−∞" : `${v.toFixed(1)} ${unit}`);

/** Loudness timeline: momentary (thin) and short-term (accent), −60…0 LUFS, with playhead. */
function Timeline({ a, timeMs, onSeek }: { a: Analysis; timeMs: number | null; onSeek: (ms: number) => void }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = ref.current;
    if (!c) return;
    const dpr = window.devicePixelRatio || 1;
    const w = c.clientWidth, h = c.clientHeight;
    c.width = w * dpr; c.height = h * dpr;
    const g = c.getContext("2d")!;
    g.scale(dpr, dpr);
    const css = getComputedStyle(c);
    const col = (n: string) => css.getPropertyValue(n).trim();
    g.clearRect(0, 0, w, h);
    const y = (l: number) => h - ((Math.max(-60, Math.min(0, l)) + 60) / 60) * h;
    g.strokeStyle = col("--border"); g.lineWidth = 1; g.font = `10px ${col("--font-mono")}`; g.fillStyle = col("--text-3");
    for (const l of [-50, -40, -30, -23, -14, -6]) {
      g.beginPath(); g.moveTo(0, y(l) + 0.5); g.lineTo(w, y(l) + 0.5); g.stroke();
      g.fillText(`${l}`, 2, y(l) - 2);
    }
    const n = a.timeline.length;
    if (!n) return;
    const draw = (k: 0 | 1, color: string, width: number) => {
      g.strokeStyle = color; g.lineWidth = width; g.beginPath();
      let started = false;
      // One point per pixel column (max over the bucket) keeps long files cheap to draw.
      for (let x = 0; x < w; x++) {
        const i0 = Math.floor((x / w) * n), i1 = Math.max(i0 + 1, Math.floor(((x + 1) / w) * n));
        let v = -200;
        for (let i = i0; i < i1 && i < n; i++) v = Math.max(v, a.timeline[i][k]);
        if (v <= -199) { started = false; continue; }
        if (!started) { g.moveTo(x, y(v)); started = true; } else g.lineTo(x, y(v));
      }
      g.stroke();
    };
    draw(0, col("--text-3"), 1);
    draw(1, col("--accent"), 2);
    // Clip markers.
    g.fillStyle = col("--danger");
    for (const ev of a.clip_events) g.fillRect((ev.time_ms / a.duration_ms) * w - 1, 0, 2, 6);
    if (timeMs !== null && a.duration_ms) {
      const x = (timeMs / a.duration_ms) * w;
      g.strokeStyle = col("--text"); g.lineWidth = 1;
      g.beginPath(); g.moveTo(x, 0); g.lineTo(x, h); g.stroke();
    }
  }, [a, timeMs]);
  return (
    <canvas ref={ref} className="timeline" role="img" aria-label="Loudness over time: momentary and short-term"
      onClick={(e) => { const r = e.currentTarget.getBoundingClientRect(); onSeek(((e.clientX - r.left) / r.width) * a.duration_ms); }} />
  );
}

export function LabPage({ current, timeMs, onAudition, onSeek, onLevel, say }: {
  current: string | null;
  timeMs: number;
  /** Plays `path` from `atMs` (A/B switching). */
  onAudition: (path: string, atMs: number) => void;
  onSeek: (ms: number) => void;
  /** Extra gain in dB applied for level-matched comparison (0 = off). */
  onLevel: (db: number) => void;
  say: (t: string) => void;
}) {
  const [path, setPath] = useState<string | null>(current);
  const [probe, setProbe] = useState<ProbeResult | null>(null);
  const [stream, setStream] = useState(0);
  const [job, setJob] = useState<{ id: number; progress: number } | null>(null);
  const [results, setResults] = useState<Record<string, Analysis>>({});
  const [err, setErr] = useState<string | null>(null);
  const [ab, setAb] = useState<{ a: number; b: number; side: "a" | "b"; match: boolean }>({ a: 0, b: 1, side: "a", match: true });
  const [renders, setRenders] = useState<Record<number, string>>({});
  const [playingPath, setPlayingPath] = useState<string | null>(null);

  useEffect(() => { if (!path && current) setPath(current); }, [current, path]);
  useEffect(() => {
    setProbe(null); setStream(0); setErr(null); setRenders({});
    if (path) api.probe(path).then(setProbe).catch((e) => setErr(String(e)));
  }, [path]);
  useEffect(() => {
    const un = listen<LabEvent>("lab-progress", ({ payload: e }) => {
      setJob((j) => (j && j.id === e.id ? (e.result || e.error ? null : { id: e.id, progress: e.progress }) : j));
      if (e.result) setResults((r) => ({ ...r, [`${e.result!.path}|${e.result!.stream ?? 0}`]: e.result! }));
      if (e.error && e.error !== "Cancelled") setErr(e.error);
    });
    return () => { un.then((f) => f()); };
  }, []);
  useEffect(() => () => onLevel(0), []); // eslint-disable-line react-hooks/exhaustive-deps

  const audio = useMemo(() => (probe?.streams ?? []).filter((s) => s.codec_type === "audio"), [probe]);
  const iamfGroups = useMemo(() => (probe?.stream_groups ?? []).filter((g) => String(g.type ?? "").toUpperCase().includes("IAMF")), [probe]);
  const key = path ? `${path}|${stream}` : "";
  const a = results[key] ?? null;
  const analyze = async (s = stream) => {
    if (!path) return;
    setErr(null);
    const id = await api.analyze(path, s);
    setJob({ id, progress: 0 });
  };
  const exportReport = async (kind: "json" | "csv") => {
    if (!a) return;
    const stem = basename(a.path).replace(/\.[^.]+$/, "");
    const t = await saveDialog({ defaultPath: `${stem}-loudness.${kind}`, filters: [{ name: kind.toUpperCase(), extensions: [kind] }] });
    if (t) { await api.exportAnalysis(a, t); say(`Report saved: ${basename(t)}`); }
  };

  // ---------- A/B auditioning of individual audio streams ----------
  const renderOf = async (s: number) => {
    if (renders[s]) return renders[s];
    say(`Decoding stream ${s}…`);
    const out = await api.extractStream(path!, s);
    setRenders((r) => ({ ...r, [s]: out }));
    return out;
  };
  const audition = async (side: "a" | "b", next = ab) => {
    if (!path) return;
    const s = side === "a" ? next.a : next.b;
    try {
      const file = await renderOf(s);
      const ia = results[`${path}|${next.a}`]?.integrated_lufs, ib = results[`${path}|${next.b}`]?.integrated_lufs;
      // Level match: bring the louder side down to the quieter one.
      let gain = 0;
      if (next.match && ia !== undefined && ib !== undefined && ia > -199 && ib > -199) {
        const target = Math.min(ia, ib);
        gain = target - (side === "a" ? ia : ib);
      }
      onLevel(gain);
      onAudition(file, playingPath && playingPath !== file ? timeMs : 0);
      setPlayingPath(file);
      setAb({ ...next, side });
    } catch (e) { setErr(String(e)); }
  };

  const playhead = a && (current === a.path || (playingPath && current === playingPath)) ? timeMs : null;

  return (
    <section className="page lab">
      <div className="page-head">
        <h1>Immersive Audio Lab</h1>
        <div className="row">
          <button className="btn" onClick={async () => { const f = await openDialog({ multiple: false }); if (typeof f === "string") setPath(f); }}><Icon name="open" /> Choose file</button>
          {current && current !== path && <button className="btn" onClick={() => setPath(current)}>Use now playing</button>}
        </div>
      </div>
      {!path ? <p className="muted">Choose a file, or play something and come back. Measurements follow ITU-R BS.1770-4 and EBU R128.</p> : (
        <>
          <p className="lab-file" title={path}><strong>{basename(path)}</strong> <span className="muted small">{probe?.format.format_long_name}</span></p>
          {iamfGroups.length > 0 && (
            <div className="banner info">
              IAMF / Eclipsa detected: {iamfGroups.length} stream group(s), {audio.length} substreams. Each substream can be measured and auditioned
              individually below. Rendering the mix presentation (demixing, ambisonics, binaural) needs libiamf, which is not built into this version.
            </div>
          )}
          <h2 className="section">Audio streams</h2>
          <div className="list">
            {audio.map((s, i) => {
              const r = path ? results[`${path}|${i}`] : undefined;
              return (
                <div key={s.index} className={`list-row static${stream === i ? " on" : ""}`}>
                  <label className="grow row">
                    <input type="radio" name="lab-stream" checked={stream === i} onChange={() => setStream(i)} />
                    <span className="lr-t">#{i} {s.codec_name} · {s.channels}ch {s.channel_layout ?? ""} · {s.sample_rate} Hz{s.tags?.language ? ` · ${s.tags.language}` : ""}</span>
                  </label>
                  <span className="mono small">{r ? `${fmt(r.integrated_lufs, "LUFS")} · ${fmt(r.true_peak_dbtp, "dBTP")}` : ""}</span>
                </div>
              );
            })}
            {probe && !audio.length && <p className="muted">No audio streams.</p>}
          </div>
          <div className="row">
            <button className="btn primary" disabled={!audio.length || !!job} onClick={() => analyze()}>Measure stream #{stream}</button>
            {audio.length > 1 && <button className="btn" disabled={!!job} onClick={async () => { for (let i = 0; i < audio.length; i++) { if (!results[`${path}|${i}`]) { await analyze(i); break; } } }}>Measure next unmeasured</button>}
            {job && (
              <>
                <progress max={1} value={job.progress} />
                <button className="btn small" onClick={() => { api.cancelJob(job.id); setJob(null); }}>Cancel</button>
              </>
            )}
          </div>
          {err && <p className="error" role="alert">{err}</p>}

          {a && (
            <>
              <div className="stat-cards">
                <div className="stat"><span className="stat-v">{fmt(a.integrated_lufs, "LUFS")}</span><span className="stat-l">Integrated</span></div>
                <div className="stat"><span className="stat-v">{a.loudness_range_lu.toFixed(1)} LU</span><span className="stat-l">Loudness range</span></div>
                <div className={`stat${a.true_peak_dbtp > -1 ? " warn" : ""}`}><span className="stat-v">{fmt(a.true_peak_dbtp, "dBTP")}</span><span className="stat-l">True peak</span></div>
                <div className="stat"><span className="stat-v">{fmt(a.sample_peak_dbfs, "dBFS")}</span><span className="stat-l">Sample peak</span></div>
                <div className="stat"><span className="stat-v">{fmt(a.max_momentary_lufs, "LUFS")}</span><span className="stat-l">Max momentary</span></div>
                <div className="stat"><span className="stat-v">{fmt(a.max_short_term_lufs, "LUFS")}</span><span className="stat-l">Max short-term</span></div>
              </div>
              <p className="small muted">
                {a.integrated_lufs > -199 && `Streaming targets: ${(-14 - a.integrated_lufs).toFixed(1)} dB to −14 LUFS · ${(-16 - a.integrated_lufs).toFixed(1)} dB to −16 · ${(-23 - a.integrated_lufs).toFixed(1)} dB to EBU R128 (−23). `}
                {a.true_peak_dbtp > -1 ? "True peak above −1 dBTP: risk of clipping after lossy encoding." : "True peak within −1 dBTP."}
              </p>
              <Timeline a={a} timeMs={playhead} onSeek={(ms) => { if (current !== a.path && !playhead) onAudition(a.path, ms); else onSeek(ms); }} />
              <div className="legend small muted"><span className="lg m" /> momentary (400 ms) <span className="lg s" /> short-term (3 s) <span className="lg c" /> clipping · click to play from there</div>

              <div className="two-col">
                <div>
                  <h2 className="section">Channels ({a.layout ?? `${a.channels} ch`})</h2>
                  <table className="table">
                    <thead><tr><th>Ch</th><th>True peak</th><th>RMS</th><th>DC</th></tr></thead>
                    <tbody>
                      {a.channel_true_peak_dbtp.map((tp, i) => (
                        <tr key={i}><td>{i}</td><td>{fmt(tp, "dBTP")}</td><td>{fmt(a.channel_rms_dbfs[i], "dBFS")}</td><td>{a.channel_dc_offset[i].toFixed(5)}</td></tr>
                      ))}
                    </tbody>
                  </table>
                </div>
                <div>
                  <h2 className="section">Clipping · {a.clipped_samples} samples</h2>
                  {a.clip_events.length === 0 ? <p className="muted small">No runs of 3+ samples at full scale.</p> : (
                    <div className="events">{a.clip_events.slice(0, 50).map((c, i) => (
                      <button key={i} className="chip" onClick={() => (current === a.path ? onSeek(c.time_ms) : onAudition(a.path, c.time_ms))}>{formatTime(c.time_ms)} · ch{c.channel} · {c.samples}</button>
                    ))}</div>
                  )}
                  <h2 className="section">Silence (below −60 dBFS, ≥ 2 s)</h2>
                  {a.silences.length === 0 ? <p className="muted small">None.</p> : (
                    <div className="events">{a.silences.map(([s, e], i) => (
                      <button key={i} className="chip" onClick={() => (current === a.path ? onSeek(s) : onAudition(a.path, s))}>{formatTime(s)}–{formatTime(e)}</button>
                    ))}</div>
                  )}
                </div>
              </div>
              <div className="row">
                <button className="btn small" onClick={() => exportReport("json")}>Export JSON report</button>
                <button className="btn small" onClick={() => exportReport("csv")}>Export CSV timeline</button>
              </div>
            </>
          )}

          {audio.length > 1 && (
            <>
              <h2 className="section">A/B audition</h2>
              <p className="small muted">Decodes each stream to WAV and switches at the same position. Level match uses the measured integrated loudness of both sides (measure them first).</p>
              <div className="row">
                A <select className="select" value={ab.a} onChange={(e) => setAb({ ...ab, a: Number(e.target.value) })}>{audio.map((_, i) => <option key={i} value={i}>#{i}</option>)}</select>
                B <select className="select" value={ab.b} onChange={(e) => setAb({ ...ab, b: Number(e.target.value) })}>{audio.map((_, i) => <option key={i} value={i}>#{i}</option>)}</select>
                <label className="check"><input type="checkbox" checked={ab.match} onChange={(e) => setAb({ ...ab, match: e.target.checked })} /> Level match</label>
                <button className={`btn${ab.side === "a" && playingPath ? " primary" : ""}`} onClick={() => audition("a")}>Play A</button>
                <button className={`btn${ab.side === "b" && playingPath ? " primary" : ""}`} onClick={() => audition("b")}>Play B</button>
                <button className="btn" disabled={!playingPath} onClick={() => audition(ab.side === "a" ? "b" : "a")}>Switch A⇄B</button>
              </div>
            </>
          )}

          {iamfGroups.length > 0 && (
            <>
              <h2 className="section">IAMF structure (as reported by FFmpeg)</h2>
              {iamfGroups.map((g, i) => (
                <details key={i} className="iamf">
                  <summary>{String(g.type)} · id {String(g.id ?? i)} · {String(g.nb_streams ?? "")} streams</summary>
                  <pre className="mono small">{JSON.stringify(g, null, 2)}</pre>
                </details>
              ))}
            </>
          )}
        </>
      )}
    </section>
  );
}
