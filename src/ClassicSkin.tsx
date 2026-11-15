import { useEffect, useRef, useState } from "react";
import { POS, SPRITES, glyph, type Rect, type Skin } from "./winamp";
import type { PlaybackStatus } from "./engine";

/** One sprite from a sheet, placed at (x, y) on the 275×116 window. */
function Sprite({ sheet, rect, x, y, w, h, title, onDown, onUp, onClick, cursor }: {
  sheet?: string; rect: Rect; x: number; y: number; w?: number; h?: number; title?: string; cursor?: string;
  onDown?: () => void; onUp?: () => void; onClick?: () => void;
}) {
  const [sx, sy, sw, sh] = rect;
  return (
    <div
      title={title}
      role={onClick ? "button" : undefined}
      aria-label={onClick ? title : undefined}
      onPointerDown={onDown ? (e) => { e.preventDefault(); onDown(); } : undefined}
      onPointerUp={onUp}
      onPointerLeave={onUp}
      onClick={onClick}
      style={{
        position: "absolute", left: x, top: y, width: w ?? sw, height: h ?? sh,
        backgroundImage: sheet ? `url(${sheet})` : undefined, backgroundPosition: `-${sx}px -${sy}px`,
        backgroundRepeat: "no-repeat", cursor: cursor ?? (onClick ? "pointer" : undefined),
      }}
    />
  );
}

/** A line of text in the skin's 5×6 bitmap font. */
function Text({ sheet, text, x, y, width }: { sheet?: string; text: string; x: number; y: number; width: number }) {
  const n = Math.floor(width / 5);
  return (
    <>
      {Array.from(text.slice(0, n)).map((ch, i) => (
        <Sprite key={i} sheet={sheet} rect={[...glyph(ch), 5, 6]} x={x + i * 5} y={y} />
      ))}
    </>
  );
}

export interface ClassicProps {
  skin: Skin;
  scale: number;
  status: PlaybackStatus;
  lengthMs: number;
  title: string;
  index: number;
  kbps: number | null;
  khz: number | null;
  channels: number | null;
  shuffle: boolean;
  repeat: boolean;
  eqOpen: boolean;
  plOpen: boolean;
  on: {
    prev: () => void; play: () => void; pause: () => void; stop: () => void; next: () => void; eject: () => void;
    seek: (ms: number) => void; volume: (v: number) => void; shuffle: () => void; repeat: () => void;
    eq: () => void; pl: () => void; close: () => void; minimize: () => void;
  };
}

export function ClassicSkin(p: ClassicProps) {
  const s = p.skin.sheets;
  const [down, setDown] = useState<string | null>(null);
  const [drag, setDrag] = useState<{ kind: "pos" | "vol"; value: number } | null>(null);
  const [scroll, setScroll] = useState(0);
  const root = useRef<HTMLDivElement>(null);

  // Marquee: "1. Title (3:25)  ***  " scrolling one character every 220 ms when it does not fit.
  const dur = p.lengthMs > 0 ? ` (${Math.floor(p.lengthMs / 60000)}:${String(Math.floor(p.lengthMs / 1000) % 60).padStart(2, "0")})` : "";
  const label = `${p.index + 1}. ${p.title}${dur}`;
  const fits = label.length <= POS.marquee[2] / 5;
  useEffect(() => {
    setScroll(0);
    if (fits) return;
    const t = setInterval(() => setScroll((x) => x + 1), 220);
    return () => clearInterval(t);
  }, [label, fits]);
  const loop = label + "  ***  ";
  const shown = fits ? label : (loop + loop).slice(scroll % loop.length);

  const st = p.status;
  const playing = st.state === "playing" || st.state === "buffering" || st.state === "opening";
  const paused = st.state === "paused";
  const ms = drag?.kind === "pos" ? drag.value : st.time_ms;
  const secs = Math.max(0, Math.floor(ms / 1000));
  const mm = Math.min(99, Math.floor(secs / 60));
  const digits = [Math.floor(mm / 10), mm % 10, Math.floor((secs % 60) / 10), secs % 10];
  const showTime = playing || paused;
  const vol = Math.min(100, Math.max(0, drag?.kind === "vol" ? drag.value : st.muted ? 0 : st.volume));
  const posMax = POS.posbar[2] - 29;
  const posX = p.lengthMs > 0 ? Math.round((ms / p.lengthMs) * posMax) : 0;

  const local = (e: React.PointerEvent) => {
    const r = root.current!.getBoundingClientRect();
    return [(e.clientX - r.left) / p.scale, (e.clientY - r.top) / p.scale];
  };
  const startDrag = (kind: "pos" | "vol") => (e: React.PointerEvent) => {
    e.preventDefault();
    const calc = (ev: React.PointerEvent | PointerEvent) => {
      const [x] = local(ev as React.PointerEvent);
      if (kind === "pos") return Math.max(0, Math.min(1, (x - POS.posbar[0] - 14) / posMax)) * p.lengthMs;
      return Math.max(0, Math.min(100, ((x - POS.volume[0] - 7) / (POS.volume[2] - 14)) * 100));
    };
    if (kind === "pos" && p.lengthMs <= 0) return;
    let value = calc(e);
    setDrag({ kind, value });
    if (kind === "vol") p.on.volume(value);
    const move = (ev: PointerEvent) => { value = calc(ev); setDrag({ kind, value }); if (kind === "vol") p.on.volume(value); };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      if (kind === "pos") p.on.seek(value);
      setDrag(null);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  const btn = (id: keyof typeof SPRITES.cbuttons, act: () => void, title: string) => {
    const r = SPRITES.cbuttons[id];
    const pressed: Rect = [r[0], r[1] + r[3], r[2], r[3]];
    const [x, y] = POS[id];
    return (
      <Sprite sheet={s.cbuttons} rect={down === id ? pressed : r} x={x} y={y} title={title}
        onDown={() => setDown(id)} onUp={() => setDown((d) => (d === id ? null : d))} onClick={act} />
    );
  };
  const toggle = (id: "shuffle" | "repeat" | "eq" | "pl", on: boolean, act: () => void, title: string) => {
    const r = SPRITES.shufrep[id];
    const isPair = id === "eq" || id === "pl";
    // shuffle/repeat: +15 pressed, +30 selected; EQ/PL: +12 selected, x+46 pressed.
    const sel = on ? (isPair ? 12 : 30) : 0;
    const prs = down === id;
    const rect: Rect = isPair ? [r[0] + (prs ? 46 : 0), r[1] + sel, r[2], r[3]] : [r[0], r[1] + sel + (prs ? 15 : 0), r[2], r[3]];
    const [x, y] = POS[id];
    return <Sprite sheet={s.shufrep} rect={rect} x={x} y={y} title={title}
      onDown={() => setDown(id)} onUp={() => setDown((d) => (d === id ? null : d))} onClick={act} />;
  };
  const tb = SPRITES.titlebar;
  const volIdx = Math.round((vol / 100) * 27);
  const stereo = (p.channels ?? 0) >= 2;
  const mono = p.channels === 1;

  return (
    <div className="classic-skin" style={{ width: 275 * p.scale, height: 116 * p.scale }} aria-label={`Classic skin: ${p.skin.name}`}>
      <div ref={root} className="classic-inner" style={{ transform: `scale(${p.scale})` }}>
        <Sprite sheet={s.main} rect={SPRITES.main.bg} x={0} y={0} />
        {/* The title bar moves the window, as in Winamp. */}
        <div data-tauri-drag-region style={{ position: "absolute", left: 0, top: 0, width: 275, height: 14,
          backgroundImage: s.titlebar ? `url(${s.titlebar})` : undefined, backgroundPosition: `-${tb.active[0]}px -${tb.active[1]}px`, cursor: "default" }} />
        <Sprite sheet={s.titlebar} rect={down === "min" ? tb.minimizeDown : tb.minimize} x={POS.minimize[0]} y={POS.minimize[1]} title="Minimise"
          onDown={() => setDown("min")} onUp={() => setDown(null)} onClick={p.on.minimize} />
        <Sprite sheet={s.titlebar} rect={down === "close" ? tb.closeDown : tb.close} x={POS.close[0]} y={POS.close[1]} title="Back to the full window"
          onDown={() => setDown("close")} onUp={() => setDown(null)} onClick={p.on.close} />

        <Sprite sheet={s.playpaus} rect={playing ? SPRITES.playpaus.playing : paused ? SPRITES.playpaus.paused : SPRITES.playpaus.stopped} x={POS.playpaus[0]} y={POS.playpaus[1]} />
        {showTime && digits.map((d, i) => (
          <Sprite key={i} sheet={s.numbers} rect={[d * 9, 0, 9, 13]} x={POS.digits[i][0]} y={POS.digits[i][1]} />
        ))}

        <Text sheet={s.text} text={shown} x={POS.marquee[0]} y={POS.marquee[1]} width={POS.marquee[2]} />
        {p.kbps !== null && <Text sheet={s.text} text={String(Math.min(999, p.kbps)).padStart(3, " ")} x={POS.kbps[0]} y={POS.kbps[1]} width={15} />}
        {p.khz !== null && <Text sheet={s.text} text={String(Math.min(99, p.khz)).padStart(2, " ")} x={POS.khz[0]} y={POS.khz[1]} width={10} />}
        <Sprite sheet={s.monoster} rect={mono ? SPRITES.monoster.monoOn : SPRITES.monoster.monoOff} x={POS.mono[0]} y={POS.mono[1]} />
        <Sprite sheet={s.monoster} rect={stereo ? SPRITES.monoster.stereoOn : SPRITES.monoster.stereoOff} x={POS.stereo[0]} y={POS.stereo[1]} />

        <div className="classic-hit" style={{ left: POS.volume[0], top: POS.volume[1], width: POS.volume[2], height: POS.volume[3] }}
          onPointerDown={startDrag("vol")} title={`Volume ${Math.round(vol)}%`}>
          <Sprite sheet={s.volume} rect={[0, volIdx * 15, 68, 13]} x={0} y={0} />
          <Sprite sheet={s.volume} rect={drag?.kind === "vol" ? SPRITES.volumeThumbDown : SPRITES.volumeThumb} x={Math.round((vol / 100) * (POS.volume[2] - 14))} y={1} />
        </div>
        {/* Balance: libVLC 3 has no balance control, so it is shown centred. */}
        <div className="classic-hit" style={{ left: POS.balance[0], top: POS.balance[1], width: POS.balance[2], height: POS.balance[3] }} title="Balance (centre)">
          <Sprite sheet={s.balance} rect={[9, 0, 38, 13]} x={0} y={0} />
          <Sprite sheet={s.balance} rect={SPRITES.volumeThumb} x={12} y={1} />
        </div>

        <div className="classic-hit" style={{ left: POS.posbar[0], top: POS.posbar[1], width: POS.posbar[2], height: POS.posbar[3] }}
          onPointerDown={startDrag("pos")} title="Seek">
          <Sprite sheet={s.posbar} rect={SPRITES.posbar.bg} x={0} y={0} />
          {p.lengthMs > 0 && <Sprite sheet={s.posbar} rect={drag?.kind === "pos" ? SPRITES.posbar.thumbDown : SPRITES.posbar.thumb} x={posX} y={0} />}
        </div>

        {btn("prev", p.on.prev, "Previous")}
        {btn("play", p.on.play, "Play")}
        {btn("pause", p.on.pause, "Pause")}
        {btn("stop", p.on.stop, "Stop")}
        {btn("next", p.on.next, "Next")}
        {btn("eject", p.on.eject, "Open files")}
        {toggle("shuffle", p.shuffle, p.on.shuffle, "Shuffle")}
        {toggle("repeat", p.repeat, p.on.repeat, "Repeat")}
        {toggle("eq", p.eqOpen, p.on.eq, "Equalizer")}
        {toggle("pl", p.plOpen, p.on.pl, "Playlist")}
      </div>
    </div>
  );
}
