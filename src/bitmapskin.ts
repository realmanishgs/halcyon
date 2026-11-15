// A common model for bitmap-based player skins of the late-90s/2000s era:
// images + positioned buttons, sliders and text fields. Each skin format has a
// small adapter that turns its own definition file into this model, and one
// renderer (BitmapSkinView) draws them all.

import { glyph, unzip, type Rect } from "./winamp";

export type Action =
  | "prev" | "play" | "pause" | "stop" | "next" | "eject" | "shuffle" | "repeat" | "eq" | "pl"
  | "close" | "minimize" | "settings" | "none";
export type Toggle = "shuffle" | "repeat" | "eq" | "pl";

/** A rectangle taken from a named image. */
export interface Src { img: string; x: number; y: number }

export interface Font {
  img: string;
  cw: number;
  ch: number;
  /** Position of a character in the font image, or null to draw nothing. */
  at: (c: string) => [number, number] | null;
}

export type Element =
  | { type: "button"; action: Action; x: number; y: number; w: number; h: number; pressed?: Src; active?: Src; toggle?: Toggle; title?: string; darken?: boolean }
  | { type: "slider"; kind: "volume" | "seek"; x: number; y: number; w: number; h: number; vertical?: boolean;
      thumb?: { src: Src; w: number; h: number };
      frames?: { img: string; fw: number; fh: number; count: number };
      reveal?: Src }
  | { type: "text"; field: "title" | "time" | "kbps" | "khz" | "track"; x: number; y: number; chars: number; font: Font; scroll?: boolean };

export interface BitmapSkin {
  kind: "bitmap";
  name: string;
  format: string;
  width: number;
  height: number;
  /** Object URLs (transparent colour already removed) by lower-case file name. */
  images: Record<string, string>;
  background: string;
  elements: Element[];
  urls: string[];
}

// ------------------------------------------------------------------ helpers

function findFile(files: Map<string, Uint8Array>, name: string): [string, Uint8Array] | null {
  const n = name.trim().toLowerCase().split(/[\\/]/).pop()!;
  const b = files.get(n);
  return b ? [n, b] : null;
}

/** Decodes a bitmap (BMP/GIF/PNG) and makes `transparent` pixels see-through. */
async function toUrl(bytes: Uint8Array, transparent: [number, number, number] | null): Promise<{ url: string; w: number; h: number }> {
  const bmp = await createImageBitmap(new Blob([bytes as unknown as ArrayBuffer]));
  const c = new OffscreenCanvas(bmp.width, bmp.height);
  const g = c.getContext("2d")!;
  g.drawImage(bmp, 0, 0);
  if (transparent) {
    const d = g.getImageData(0, 0, bmp.width, bmp.height);
    const p = d.data;
    for (let i = 0; i < p.length; i += 4) {
      if (p[i] === transparent[0] && p[i + 1] === transparent[1] && p[i + 2] === transparent[2]) p[i + 3] = 0;
    }
    g.putImageData(d, 0, 0);
  }
  const blob = await c.convertToBlob({ type: "image/png" });
  return { url: URL.createObjectURL(blob), w: bmp.width, h: bmp.height };
}

async function loadImages(files: Map<string, Uint8Array>, names: string[], transparent: [number, number, number] | null) {
  const images: Record<string, string> = {};
  const sizes: Record<string, [number, number]> = {};
  for (const name of new Set(names.filter(Boolean).map((n) => n.trim().toLowerCase().split(/[\\/]/).pop()!))) {
    const f = findFile(files, name);
    if (!f) continue;
    const r = await toUrl(f[1], transparent);
    images[name] = r.url;
    sizes[name] = [r.w, r.h];
  }
  return { images, sizes };
}

const nums = (s: string) => s.split(/[\s,]+/).filter((x) => x !== "").map(Number);
const key = (n: string) => n.trim().toLowerCase().split(/[\\/]/).pop()!;

// ------------------------------------------------------------------ CoolPlayer

/** CoolPlayer: an INI ([CoolPlayer Skin]) + full-window Up/Down/Switch bitmaps + fonts. */
async function coolplayer(name: string, files: Map<string, Uint8Array>, ini: string): Promise<BitmapSkin> {
  const v: Record<string, string> = {};
  for (const line of ini.split(/\r?\n/)) {
    const m = line.match(/^\s*([A-Za-z0-9]+)\s*=\s*(.*?)\s*$/);
    if (m && !line.trim().startsWith(";")) v[m[1].toLowerCase()] = m[2];
  }
  const tc = parseInt((v.transparentcolor ?? "").replace(/^0x/i, ""), 16);
  const transparent: [number, number, number] | null = Number.isFinite(tc) ? [(tc >> 16) & 255, (tc >> 8) & 255, tc & 255] : null;
  const up = key(v.bmpcoolup ?? ""), down = key(v.bmpcooldown ?? ""), sw = key(v.bmpcoolswitch ?? "");
  const textFont = key(v.bmptextfont ?? ""), timeFont = key(v.bmptimefont ?? "");
  const { images, sizes } = await loadImages(files, [up, down, sw, textFont, timeFont, v.bmptrackfont, v.bmpbitratefont, v.bmpfreqfont], transparent);
  if (!images[up]) throw new Error("CoolPlayer skin is missing its main bitmap");
  const elements: Element[] = [];
  const btn = (k: string, action: Action, toggle?: Toggle) => {
    const n = v[k];
    if (!n) return;
    const p = nums(n.split(/,(?=[^,]*$)/)[0].replace(/[^0-9,\s-]/g, ""));
    if (p.length < 4) return;
    const [x, y, w, h] = p;
    const sx = p[5] ?? x, sy = p[6] ?? y;
    elements.push({ type: "button", action, x, y, w, h, toggle, title: n.split(",").pop()?.trim(),
      pressed: images[down] ? { img: down, x: sx, y: sy } : undefined,
      active: toggle && images[sw] ? { img: sw, x: sx, y: sy } : undefined });
  };
  btn("playswitch", "play"); btn("pauseswitch", "pause"); btn("stopswitch", "stop");
  btn("prevbutton", "prev"); btn("nextbutton", "next"); btn("ejectbutton", "eject"); btn("openbutton", "eject");
  btn("shuffleswitch", "shuffle", "shuffle"); btn("repeatswitch", "repeat", "repeat");
  btn("eqswitch", "eq", "eq"); btn("playlistbutton", "pl", "pl");
  btn("minimizebutton", "minimize"); btn("exitbutton", "close");
  for (const [k, kind] of [["volumeslider", "volume"], ["positionslider", "seek"]] as const) {
    const p = v[k] ? nums(v[k]) : [];
    if (p.length >= 9) {
      elements.push({ type: "slider", kind, x: p[0], y: p[1], w: p[2], h: p[3], vertical: p[4] === 1,
        thumb: images[down] ? { src: { img: down, x: p[5], y: p[6] }, w: p[7], h: p[8] } : undefined });
    }
  }
  const ascii: Font | null = images[textFont] ? (() => {
    const [cw, ch] = [nums(v.songtitletext ?? "0,0,6,7")[2] || 6, nums(v.songtitletext ?? "0,0,6,7")[3] || 7];
    return { img: textFont, cw, ch, at: (c: string) => { const code = c.charCodeAt(0); return code >= 32 && code <= 126 ? [(code - 32) * cw, 0] : null; } };
  })() : null;
  const digitsFont = (img: string, def: string): Font | null => {
    const p = nums(def);
    if (!images[img] || p.length < 4) return null;
    const order = "0123456789:-";
    return { img, cw: p[2], ch: p[3], at: (c) => { const i = order.indexOf(c); return i >= 0 ? [i * p[2], 0] : null; } };
  };
  const t = v.songtitletext ? nums(v.songtitletext) : [];
  if (ascii && t.length >= 2) elements.push({ type: "text", field: "title", x: t[0], y: t[1], chars: t[4] || 20, font: ascii, scroll: true });
  for (const [k, field, img] of [["timetext", "time", timeFont], ["bitratetext", "kbps", key(v.bmpbitratefont ?? timeFont)],
    ["freqtext", "khz", key(v.bmpfreqfont ?? timeFont)], ["tracktext", "track", key(v.bmptrackfont ?? timeFont)]] as const) {
    const f = v[k] ? digitsFont(img, v[k]) : null;
    const p = v[k] ? nums(v[k]) : [];
    if (f && p[2] > 0) elements.push({ type: "text", field, x: p[0], y: p[1], chars: field === "time" ? 5 : 3, font: f });
  }
  const [width, height] = sizes[up];
  return { kind: "bitmap", name, format: "CoolPlayer", width, height, images, background: up, elements, urls: Object.values(images) };
}

// ------------------------------------------------------------------ K-Jofol

/** K-Jofol: a .rc text file ("Key value...") with x1 y1 x2 y2 rectangles + bitmaps. */
async function kjofol(name: string, files: Map<string, Uint8Array>, rc: string): Promise<BitmapSkin> {
  const v: Record<string, string> = {};
  for (const line of rc.split(/\r?\n/)) {
    if (line.trim().startsWith("#")) continue;
    // Keys are letters, except "...Pressed1/2/3"; some skins forget the space
    // before the numbers ("PreviousSongButton17 101 36 122").
    // Key = letters (digits allowed inside, e.g. "MP3TimeWindow"), then optional trailing digits.
    const m = line.match(/^\s*([A-Za-z](?:[A-Za-z0-9]*[A-Za-z])?)(\d*)\s*(.*?)\s*$/);
    if (!m || !m[1]) continue;
    let k = m[1].toLowerCase();
    let rest = m[3];
    if (m[2]) {
      if (/pressed$/.test(k) && m[2].length === 1) k += m[2];
      else rest = `${m[2]} ${rest}`;
    }
    v[k] = rest;
  }
  const bg = key(v.backgroundimage ?? "");
  const pressedImg = key(v.backgroundimagepressed1 ?? v.backgroundimage ?? "");
  const font = key(v.fontimage ?? ""), timeFont = key(v.timefontimage ?? "");
  const volImg = key(v.volumecontrolimage ?? "");
  const { images, sizes } = await loadImages(files, [bg, pressedImg, font, timeFont, volImg], [255, 0, 255]);
  if (!images[bg]) throw new Error("K-Jofol skin is missing its background image");
  const elements: Element[] = [];
  const rect = (k: string) => { const p = nums((v[k] ?? "").replace(/[A-Za-z].*$/, "")); return p.length >= 4 ? p : null; };
  const btn = (k: string, action: Action, toggle?: Toggle) => {
    const p = rect(k);
    if (!p) return;
    const [x1, y1, x2, y2] = p;
    const darken = /darken/i.test(v[k]);
    elements.push({ type: "button", action, x: x1, y: y1, w: x2 - x1, h: y2 - y1, toggle, darken,
      pressed: !darken && images[pressedImg] && pressedImg !== bg ? { img: pressedImg, x: x1, y: y1 } : undefined });
  };
  btn("playbutton", "play"); btn("pausebutton", "pause"); btn("stopbutton", "stop");
  btn("previoussongbutton", "prev"); btn("nextsongbutton", "next"); btn("openfilebutton", "eject");
  btn("repeatbutton", "repeat", "repeat"); btn("shufflebutton", "shuffle", "shuffle");
  btn("equalizerbutton", "eq", "eq"); btn("playlistbutton", "pl", "pl");
  btn("closebutton", "close"); btn("minimizebutton", "minimize"); btn("preferencesbutton", "settings");
  const vol = rect("volumecontrolbutton");
  if (vol) {
    const fw = Number(v.volumecontrolimagexsize) || 0, count = Number(v.volumecontrolimagenb) || 0;
    elements.push({ type: "slider", kind: "volume", x: vol[0], y: vol[1], w: vol[2] - vol[0], h: vol[3] - vol[1], vertical: true,
      frames: images[volImg] && fw && count ? { img: volImg, fw, fh: sizes[volImg][1], count } : undefined });
  }
  const seek = rect("seekregion");
  if (seek) {
    // seek.bmp is a hidden position map (grey level = position), not something drawn.
    elements.push({ type: "slider", kind: "seek", x: seek[0], y: seek[1], w: seek[2] - seek[0], h: seek[3] - seek[1] });
  }
  const fn = rect("filenamewindow");
  const fs = nums(v.fontsize ?? "5 6");
  if (fn && images[font]) {
    elements.push({ type: "text", field: "title", x: fn[0], y: fn[1], chars: Math.floor((fn[2] - fn[0]) / (fs[0] || 5)), scroll: true,
      font: { img: font, cw: fs[0] || 5, ch: fs[1] || 6, at: (c) => glyph(c) } });
  }
  const tw = rect("mp3timewindow");
  const ts = nums(v.timefontsize ?? "8 10");
  if (tw && images[timeFont]) {
    const order = "0123456789:";
    elements.push({ type: "text", field: "time", x: tw[0], y: tw[1], chars: 5,
      font: { img: timeFont, cw: ts[0] || 8, ch: ts[1] || 10, at: (c) => { const i = order.indexOf(c); return i >= 0 ? [i * (ts[0] || 8), 0] : null; } } });
  }
  const [width, height] = sizes[bg];
  return { kind: "bitmap", name, format: "K-Jofol", width, height, images, background: bg, elements, urls: Object.values(images) };
}

// ------------------------------------------------------------------ detection

/** Tries every bitmap-skin adapter; returns null if the archive is none of them. */
export async function loadBitmapSkin(name: string, files: Map<string, Uint8Array>): Promise<BitmapSkin | null> {
  const dec = (b: Uint8Array) => new TextDecoder("latin1").decode(b);
  for (const [n, b] of files) {
    if (n.endsWith(".ini") && /\[coolplayer skin\]/i.test(dec(b))) return coolplayer(name, files, dec(b));
  }
  for (const [n, b] of files) {
    if (n.endsWith(".rc") && /backgroundimage/i.test(dec(b))) return kjofol(name, files, dec(b));
  }
  return null;
}

export { unzip };
export type { Rect };
