import type React from "react";
import { ASPECTS, RATES, basename, type PlaybackStatus } from "./engine";
import { Icon } from "./icons";

export function QueuePanel(p: {
  queue: string[]; index: number;
  onPlay: (i: number) => void; onRemove: (i: number) => void; onMove: (i: number, d: -1 | 1) => void;
  onClear: () => void; onAdd: () => void;
}) {
  return (
    <div className="queue">
      <div className="row">
        <button className="btn small" onClick={p.onAdd}><Icon name="open" /> Add</button>
        <button className="btn small" onClick={p.onClear} disabled={p.queue.length < 2}>Clear others</button>
      </div>
      {p.queue.length === 0 && <p className="muted">The queue is empty. Drop files, folders or playlists anywhere.</p>}
      <ol className="queue-list">
        {p.queue.map((src, i) => (
          <li key={`${i}-${src}`} className={`queue-item${i === p.index ? " current" : ""}`} onDoubleClick={() => p.onPlay(i)} title={src}>
            <span className="qi-num">{i === p.index ? <Icon name="play" filled /> : i + 1}</span>
            <button className="qi-name" onClick={() => p.onPlay(i)}>{basename(src)}</button>
            <span className="qi-actions">
              <button className="icon-btn sm" onClick={() => p.onMove(i, -1)} disabled={i === 0} aria-label="Move up"><Icon name="up" /></button>
              <button className="icon-btn sm" onClick={() => p.onMove(i, 1)} disabled={i === p.queue.length - 1} aria-label="Move down"><Icon name="down" /></button>
              <button className="icon-btn sm" onClick={() => p.onRemove(i)} disabled={i === p.index} aria-label="Remove"><Icon name="close" /></button>
            </span>
          </li>
        ))}
      </ol>
    </div>
  );
}

function Stepper({ label, value, unit, onChange, step, disabled }: {
  label: string; value: number; unit: string; step: number; disabled?: boolean; onChange: (delta: number) => void;
}) {
  return (
    <div className="stepper">
      <span className="stepper-label">{label}</span>
      <button className="icon-btn sm" onClick={() => onChange(-step)} disabled={disabled} aria-label={`${label} minus`}><Icon name="minus" /></button>
      <span className="stepper-value">{value > 0 ? "+" : ""}{value} {unit}</span>
      <button className="icon-btn sm" onClick={() => onChange(step)} disabled={disabled} aria-label={`${label} plus`}><Icon name="plus" /></button>
      <button className="btn small" onClick={() => onChange(0)} disabled={disabled || value === 0}>Reset</button>
    </div>
  );
}

export function TunePanel(p: {
  status: PlaybackStatus; aspect: string; deinterlace: string | null; hasVideo: boolean;
  onDelay: (k: "audio" | "subtitle", d: number) => void; onRate: (r: number) => void;
  onAspect: (a: string) => void; onDeinterlace: (m: string | null) => void;
  onAddSubtitle: () => void; onSnapshot: () => void; onChapter: (c: number) => void;
  children?: React.ReactNode;
}) {
  const s = p.status;
  const live = s.state !== "idle" && s.state !== "stopped";
  return (
    <div className="tune">
      <h2>Synchronisation</h2>
      <Stepper label="Audio delay" value={s.audio_delay_ms} unit="ms" step={50} disabled={!live} onChange={(d) => p.onDelay("audio", d)} />
      <Stepper label="Subtitle delay" value={s.subtitle_delay_ms} unit="ms" step={50} disabled={!live} onChange={(d) => p.onDelay("subtitle", d)} />
      <p className="muted small">Positive values delay audio/subtitles. Resets when the media changes.</p>

      <h2>Speed</h2>
      <div className="chips">
        {RATES.map((r) => (
          <button key={r} className={`chip${Math.abs(s.rate - r) < 0.01 ? " on" : ""}`} onClick={() => p.onRate(r)} disabled={!live}>{r}×</button>
        ))}
      </div>

      <h2>Picture</h2>
      <div className="chips">
        {ASPECTS.map((a) => (
          <button key={a} className={`chip${p.aspect === a ? " on" : ""}`} onClick={() => p.onAspect(a)} disabled={!p.hasVideo}>
            {a === "default" ? "Default" : a === "fill" ? "Stretch" : a}
          </button>
        ))}
      </div>
      <div className="chips">
        {[null, "blend", "yadif", "yadif2x", "bob", "linear"].map((m) => (
          <button key={m ?? "off"} className={`chip${p.deinterlace === m ? " on" : ""}`} onClick={() => p.onDeinterlace(m)} disabled={!p.hasVideo}>
            {m ?? "No deinterlace"}
          </button>
        ))}
      </div>
      <div className="row">
        <button className="btn small" onClick={p.onSnapshot} disabled={!p.hasVideo}><Icon name="camera" /> Snapshot</button>
        <button className="btn small" onClick={p.onAddSubtitle} disabled={!live}><Icon name="open" /> Subtitle file…</button>
      </div>

      {s.chapter_count > 1 && (
        <>
          <h2>Chapters</h2>
          <div className="chips">
            {Array.from({ length: s.chapter_count }, (_, i) => (
              <button key={i} className={`chip${s.chapter === i ? " on" : ""}`} onClick={() => p.onChapter(i)}>{i + 1}</button>
            ))}
          </div>
        </>
      )}
      {p.children}
    </div>
  );
}

// ---------------------------------------------------------------- audio: equalizer + output

export interface EqState { enabled: boolean; preset: string; preamp: number; bands: number[] }

export function AudioPanel(p: {
  info: import("./engine").EqualizerInfo | null; eq: EqState; onEq: (eq: EqState) => void;
  devices: import("./engine").AudioDevice[]; device: string; onDevice: (id: string) => void;
  normalize: boolean; onNormalize: (v: boolean) => void;
}) {
  const freqs = p.info?.frequencies ?? [];
  const label = (f: number) => (f >= 1000 ? `${f / 1000}k` : `${Math.round(f)}`);
  const set = (patch: Partial<EqState>) => p.onEq({ ...p.eq, ...patch, enabled: true });
  return (
    <div className="tune">
      <h2>Equalizer</h2>
      <div className="row">
        <label className="check"><input type="checkbox" checked={p.eq.enabled} onChange={(e) => p.onEq({ ...p.eq, enabled: e.target.checked })} /> On</label>
        <select className="select" aria-label="Preset" value={p.eq.preset} onChange={(e) => {
          const pr = p.info?.presets.find(([n]) => n === e.target.value);
          if (pr) set({ preset: pr[0], preamp: pr[1].preamp, bands: pr[1].bands });
        }}>
          <option value="Custom">Custom</option>
          {p.info?.presets.map(([n]) => <option key={n} value={n}>{n}</option>)}
        </select>
        <button className="btn small" onClick={() => set({ preset: "Flat", preamp: 0, bands: freqs.map(() => 0) })}>Reset</button>
      </div>
      <div className={`eq${p.eq.enabled ? "" : " off"}`}>
        <div className="eq-band">
          <input type="range" min={-20} max={20} step={0.5} value={p.eq.preamp} aria-label="Preamp"
            onChange={(e) => set({ preamp: Number(e.target.value), preset: "Custom" })} />
          <span className="eq-v">{p.eq.preamp > 0 ? "+" : ""}{p.eq.preamp.toFixed(1)}</span><span className="eq-f">pre</span>
        </div>
        {freqs.map((f, i) => (
          <div key={f} className="eq-band">
            <input type="range" min={-20} max={20} step={0.5} value={p.eq.bands[i] ?? 0} aria-label={`${label(f)} Hz`}
              onChange={(e) => { const b = [...p.eq.bands]; b[i] = Number(e.target.value); set({ bands: b, preset: "Custom" }); }} />
            <span className="eq-v">{(p.eq.bands[i] ?? 0).toFixed(1)}</span><span className="eq-f">{label(f)}</span>
          </div>
        ))}
      </div>
      <h2>Loudness</h2>
      <label className="check"><input type="checkbox" checked={p.normalize} onChange={(e) => p.onNormalize(e.target.checked)} /> Volume normalisation (restarts playback at the same position)</label>
      <h2>Output device</h2>
      <select className="select wide" aria-label="Audio output device" value={p.device} onChange={(e) => p.onDevice(e.target.value)}>
        <option value="">System default</option>
        {p.devices.filter((d) => d.id).map((d) => <option key={d.id} value={d.id}>{d.name}</option>)}
      </select>
    </div>
  );
}

// ---------------------------------------------------------------- picture: adjust, crop, rotation

export const NEUTRAL: import("./engine").VideoAdjust = { brightness: 1, contrast: 1, hue: 0, saturation: 1, gamma: 1 };

export function PicturePanel(p: {
  adjust: import("./engine").VideoAdjust | null; onAdjust: (a: import("./engine").VideoAdjust | null) => void;
  crop: string | null; onCrop: (c: string | null) => void;
  transform: string; onTransform: (t: string) => void; hasVideo: boolean;
}) {
  const a = p.adjust ?? NEUTRAL;
  const sliders: [keyof typeof a, string, number, number, number][] = [
    ["brightness", "Brightness", 0, 2, 0.01], ["contrast", "Contrast", 0, 2, 0.01], ["saturation", "Saturation", 0, 3, 0.01],
    ["hue", "Hue", -180, 180, 1], ["gamma", "Gamma", 0.01, 3, 0.01],
  ];
  return (
    <div className="tune">
      <h2>Adjust</h2>
      {sliders.map(([k, l, min, max, step]) => (
        <div key={k} className="slider-row">
          <span className="stepper-label">{l}</span>
          <input type="range" min={min} max={max} step={step} value={a[k]} disabled={!p.hasVideo} aria-label={l}
            style={{ ["--fill" as string]: `${((a[k] - min) / (max - min)) * 100}%` }}
            onChange={(e) => p.onAdjust({ ...a, [k]: Number(e.target.value) })} />
          <span className="stepper-value">{a[k].toFixed(k === "hue" ? 0 : 2)}</span>
        </div>
      ))}
      <button className="btn small" disabled={!p.adjust} onClick={() => p.onAdjust(null)}>Reset picture</button>
      <h2>Crop</h2>
      <div className="chips">
        {[null, "16:9", "4:3", "21:9", "2.39:1", "1:1", "16:10"].map((c) => (
          <button key={c ?? "none"} className={`chip${p.crop === c ? " on" : ""}`} disabled={!p.hasVideo} onClick={() => p.onCrop(c)}>{c ?? "No crop"}</button>
        ))}
      </div>
      <h2>Rotate &amp; flip</h2>
      <div className="chips">
        {[["", "None"], ["90", "90°"], ["180", "180°"], ["270", "270°"], ["hflip", "Mirror"], ["vflip", "Flip"]].map(([v, l]) => (
          <button key={v} className={`chip${p.transform === v ? " on" : ""}`} onClick={() => p.onTransform(v)}>{l}</button>
        ))}
      </div>
      <p className="muted small">Rotation restarts playback at the same position and decodes in software (libVLC 3 cannot rotate GPU frames).</p>
    </div>
  );
}

// ---------------------------------------------------------------- sleep timer

export function SleepTimer({ endsAt, atEnd, onSet }: { endsAt: number | null; atEnd: boolean; onSet: (min: number | null, atEnd?: boolean) => void }) {
  const left = endsAt ? Math.max(0, Math.round((endsAt - Date.now()) / 60000)) : null;
  return (
    <>
      <h2>Sleep timer</h2>
      <div className="chips">
        {[15, 30, 45, 60, 90].map((m) => <button key={m} className="chip" onClick={() => onSet(m)}>{m} min</button>)}
        <button className={`chip${atEnd ? " on" : ""}`} onClick={() => onSet(null, true)}>End of item</button>
        <button className="chip" disabled={!endsAt && !atEnd} onClick={() => onSet(null)}>Off</button>
      </div>
      <p className="muted small">{atEnd ? "Stops when the current item ends." : endsAt ? `Fades out and pauses in ${left} min.` : "Off"}</p>
    </>
  );
}
