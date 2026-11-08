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
    </div>
  );
}
