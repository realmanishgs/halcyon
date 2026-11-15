import { useEffect, useRef, useState } from "react";
import type { Action, BitmapSkin, Element, Src } from "./bitmapskin";
import type { ClassicProps } from "./ClassicSkin";

/** A crop of one of the skin's images, placed at (x, y). */
function Crop({ src, url, x, y, w, h }: { src: Src; url?: string; x: number; y: number; w: number; h: number }) {
  if (!url) return null;
  return <div style={{ position: "absolute", left: x, top: y, width: w, height: h, backgroundImage: `url(${url})`,
    backgroundPosition: `-${src.x}px -${src.y}px`, backgroundRepeat: "no-repeat", pointerEvents: "none" }} />;
}

/** Renders any adapter-produced bitmap skin and wires it to playback (same props as the Winamp renderer). */
export function BitmapSkinView(p: Omit<ClassicProps, "skin"> & { skin: BitmapSkin }) {
  const s = p.skin;
  const img = (n: string) => s.images[n];
  const [down, setDown] = useState<number | null>(null);
  const [drag, setDrag] = useState<{ i: number; value: number } | null>(null);
  const [scroll, setScroll] = useState(0);
  const root = useRef<HTMLDivElement>(null);
  const st = p.status;

  const dur = p.lengthMs > 0 ? ` (${Math.floor(p.lengthMs / 60000)}:${String(Math.floor(p.lengthMs / 1000) % 60).padStart(2, "0")})` : "";
  const label = `${p.index + 1}. ${p.title}${dur}`;
  useEffect(() => {
    setScroll(0);
    const t = setInterval(() => setScroll((x) => x + 1), 250);
    return () => clearInterval(t);
  }, [label]);

  const act = (a: Action) => {
    const o = p.on;
    const map: Record<Action, (() => void) | undefined> = {
      prev: o.prev, play: o.play, pause: o.pause, stop: o.stop, next: o.next, eject: o.eject, shuffle: o.shuffle,
      repeat: o.repeat, eq: o.eq, pl: o.pl, close: o.close, minimize: o.minimize, settings: o.close, none: undefined,
    };
    map[a]?.();
  };
  const toggled = (t?: string) => (t === "shuffle" ? p.shuffle : t === "repeat" ? p.repeat : t === "eq" ? p.eqOpen : t === "pl" ? p.plOpen : false);

  const fraction = (e: Element & { type: "slider" }, val?: number) => {
    if (val !== undefined) return val;
    if (e.kind === "volume") return Math.min(100, Math.max(0, st.muted ? 0 : st.volume)) / 100;
    return p.lengthMs > 0 ? Math.min(1, st.time_ms / p.lengthMs) : 0;
  };
  const startDrag = (i: number, e: Element & { type: "slider" }) => (ev: React.PointerEvent) => {
    ev.preventDefault();
    if (e.kind === "seek" && p.lengthMs <= 0) return;
    const calc = (x: number, y: number) => {
      const r = root.current!.getBoundingClientRect();
      const lx = (x - r.left) / p.scale, ly = (y - r.top) / p.scale;
      const f = e.vertical ? 1 - (ly - e.y) / e.h : (lx - e.x) / e.w;
      return Math.max(0, Math.min(1, f));
    };
    let value = calc(ev.clientX, ev.clientY);
    const apply = () => { if (e.kind === "volume") p.on.volume(value * 100); };
    setDrag({ i, value }); apply();
    const move = (m: PointerEvent) => { value = calc(m.clientX, m.clientY); setDrag({ i, value }); apply(); };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      if (e.kind === "seek") p.on.seek(value * p.lengthMs);
      setDrag(null);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  const fieldText = (f: string, chars: number, scrolls?: boolean) => {
    const secs = Math.floor(st.time_ms / 1000);
    switch (f) {
      case "time": return st.state === "idle" || st.state === "stopped" ? "" : `${String(Math.min(99, Math.floor(secs / 60))).padStart(2, "0")}:${String(secs % 60).padStart(2, "0")}`;
      case "kbps": return p.kbps ? String(Math.min(999, p.kbps)).padStart(chars, " ") : "";
      case "khz": return p.khz ? String(p.khz).padStart(chars, " ") : "";
      case "track": return String(p.index + 1).padStart(chars, " ");
      default: {
        if (!scrolls || label.length <= chars) return label;
        const loop = label + "  ***  ";
        return (loop + loop).slice(scroll % loop.length, (scroll % loop.length) + chars);
      }
    }
  };

  return (
    <div className="classic-skin" style={{ width: s.width * p.scale, height: s.height * p.scale }} aria-label={`${s.format} skin: ${s.name}`}>
      <div ref={root} className="classic-inner" style={{ width: s.width, height: s.height, transform: `scale(${p.scale})` }}>
        {/* Background; it also moves the window, as the old players did. */}
        <div data-tauri-drag-region style={{ position: "absolute", inset: 0, backgroundImage: `url(${img(s.background)})`, backgroundRepeat: "no-repeat" }} />
        {s.elements.map((e, i) => {
          if (e.type === "button") {
            const isDown = down === i;
            const on = e.toggle && toggled(e.toggle);
            const src = isDown && e.pressed ? e.pressed : on && e.active ? e.active : null;
            return (
              <div key={i} role="button" aria-label={e.title ?? e.action} title={e.title ?? e.action}
                style={{ position: "absolute", left: e.x, top: e.y, width: e.w, height: e.h, cursor: "pointer",
                  filter: e.darken && (isDown || on) ? "brightness(0.6)" : undefined,
                  backgroundImage: e.darken && (isDown || on) ? `url(${img(s.background)})` : undefined, backgroundPosition: `-${e.x}px -${e.y}px` }}
                onPointerDown={(ev) => { ev.preventDefault(); setDown(i); }}
                onPointerUp={() => setDown(null)} onPointerLeave={() => setDown((d) => (d === i ? null : d))}
                onClick={() => act(e.action)}>
                {src && <Crop src={src} url={img(src.img)} x={0} y={0} w={e.w} h={e.h} />}
              </div>
            );
          }
          if (e.type === "slider") {
            const f = fraction(e, drag?.i === i ? drag.value : undefined);
            return (
              <div key={i} style={{ position: "absolute", left: e.x, top: e.y, width: e.w, height: e.h, cursor: "pointer" }}
                onPointerDown={startDrag(i, e)} title={e.kind === "volume" ? `Volume ${Math.round(f * 100)}%` : "Seek"}>
                {e.reveal && <Crop src={e.reveal} url={img(e.reveal.img)} x={0} y={0} w={Math.round(e.w * f)} h={e.h} />}
                {e.frames && (() => {
                  const n = Math.min(e.frames.count - 1, Math.round(f * (e.frames.count - 1)));
                  return <Crop src={{ img: e.frames.img, x: n * e.frames.fw, y: 0 }} url={img(e.frames.img)} x={0} y={0} w={e.frames.fw} h={e.frames.fh} />;
                })()}
                {e.thumb && (() => {
                  const tx = e.vertical ? 0 : Math.round(f * (e.w - e.thumb.w));
                  const ty = e.vertical ? Math.round((1 - f) * (e.h - e.thumb.h)) : 0;
                  return <Crop src={e.thumb.src} url={img(e.thumb.src.img)} x={tx} y={ty} w={e.thumb.w} h={e.thumb.h} />;
                })()}
              </div>
            );
          }
          const text = fieldText(e.field, e.chars, e.scroll);
          return (
            <div key={i} style={{ position: "absolute", left: e.x, top: e.y, pointerEvents: "none" }}>
              {Array.from(text.slice(0, e.chars)).map((c, j) => {
                const at = e.font.at(c);
                return at ? <Crop key={j} src={{ img: e.font.img, x: at[0], y: at[1] }} url={img(e.font.img)} x={j * e.font.cw} y={0} w={e.font.cw} h={e.font.ch} /> : null;
              })}
            </div>
          );
        })}
      </div>
    </div>
  );
}
