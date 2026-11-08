import { useEffect, useRef, useState } from "react";
import { formatTime, type PlaybackStatus } from "./engine";

interface Props {
  status: PlaybackStatus;
  /** performance.now() when `status` was received from the engine. */
  receivedAt: number;
  onSeek: (ms: number) => void;
  chapters: number[]; // chapter start times, ms
}

/**
 * Playhead that tracks the engine precisely:
 * - interpolates between 4 Hz engine samples using the real playback rate,
 *   never running more than 1 s ahead of the last engine sample;
 * - after a seek, holds the target until the engine reports a time near it
 *   (no snap-back to the old position);
 * - pointer capture keeps a drag alive when the pointer leaves the bar.
 */
export function SeekBar({ status, receivedAt, onSeek, chapters }: Props) {
  const track = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<number | null>(null);
  const [hover, setHover] = useState<{ x: number; ms: number } | null>(null);
  const [remaining, setRemaining] = useState(false);
  const pending = useRef<{ ms: number; at: number } | null>(null);
  const [, setFrame] = useState(0);
  const len = status.length_ms;
  const running = status.state === "playing";

  // Re-render every frame only while something moves.
  useEffect(() => {
    if (!running && !pending.current) return;
    let raf = 0;
    const tick = () => { setFrame((f) => (f + 1) % 1e6); raf = requestAnimationFrame(tick); };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [running, receivedAt]);

  const now = performance.now();
  let pos: number;
  const p = pending.current;
  if (p && (now - p.at > 2500 || (receivedAt > p.at + 30 && Math.abs(status.time_ms - p.ms) < 1500))) {
    pending.current = null;
  }
  if (drag !== null) {
    pos = drag;
  } else if (pending.current) {
    pos = pending.current.ms;
  } else if (status.state === "ended") {
    pos = len;
  } else if (status.state === "stopped" || status.state === "idle") {
    pos = 0;
  } else {
    const ahead = running ? Math.min(1000, (now - receivedAt) * status.rate) : 0;
    pos = status.time_ms + Math.max(0, ahead);
  }
  pos = len > 0 ? Math.min(Math.max(0, pos), len) : 0;
  const frac = len > 0 ? pos / len : 0;

  const msAt = (clientX: number) => {
    const r = track.current!.getBoundingClientRect();
    return Math.min(1, Math.max(0, (clientX - r.left) / r.width)) * len;
  };
  const commit = (ms: number) => {
    pending.current = { ms, at: performance.now() };
    setDrag(null);
    onSeek(ms);
  };
  const enabled = len > 0 && status.seekable;

  return (
    <div className="seek">
      <span className="time" aria-label="Elapsed">{formatTime(pos)}</span>
      <div
        ref={track}
        className={`seek-track${enabled ? "" : " disabled"}${drag !== null ? " dragging" : ""}`}
        role="slider"
        aria-label="Seek"
        aria-valuemin={0}
        aria-valuemax={Math.round(len / 1000)}
        aria-valuenow={Math.round(pos / 1000)}
        aria-valuetext={`${formatTime(pos)} of ${formatTime(len)}`}
        aria-disabled={!enabled}
        onPointerDown={(e) => {
          if (!enabled || e.button !== 0) return;
          e.currentTarget.setPointerCapture(e.pointerId);
          setDrag(msAt(e.clientX));
        }}
        onPointerMove={(e) => {
          if (len > 0) {
            const r = track.current!.getBoundingClientRect();
            setHover({ x: Math.min(r.width, Math.max(0, e.clientX - r.left)), ms: msAt(e.clientX) });
          }
          if (drag !== null) setDrag(msAt(e.clientX));
        }}
        onPointerUp={(e) => { if (drag !== null) commit(msAt(e.clientX)); }}
        onLostPointerCapture={() => { if (drag !== null) commit(drag); }}
        onPointerLeave={() => setHover(null)}
      >
        <div className="seek-rail" />
        <div className="seek-fill" style={{ width: `${frac * 100}%` }} />
        {len > 0 && chapters.filter((c) => c > 0 && c < len).map((c) => (
          <div key={c} className="seek-chapter" style={{ left: `${(c / len) * 100}%` }} />
        ))}
        <div className="seek-thumb" style={{ left: `${frac * 100}%` }} />
        {hover && enabled && <div className="seek-tip" style={{ left: hover.x }}>{formatTime(hover.ms)}</div>}
      </div>
      <button className="time time-btn" onClick={() => setRemaining((r) => !r)} title="Toggle remaining time">
        {len > 0 ? (remaining ? `-${formatTime(len - pos)}` : formatTime(len)) : "--:--"}
      </button>
    </div>
  );
}
