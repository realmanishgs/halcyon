// Winamp 2.x classic skin (.wsz) support: a .wsz is a ZIP of BMP sprite sheets
// plus viscolor.txt. Sprite rectangles and window positions follow the classic
// skin format as documented by the skinning community (and used by Webamp).

/** Decompresses a ZIP archive (stored or deflate) into name → bytes. Names are lower-cased, folders stripped. */
export async function unzip(buf: ArrayBuffer): Promise<Map<string, Uint8Array>> {
  const v = new DataView(buf);
  const out = new Map<string, Uint8Array>();
  // End of central directory: search backwards for its signature.
  let eocd = -1;
  for (let i = buf.byteLength - 22; i >= Math.max(0, buf.byteLength - 65557); i--) {
    if (v.getUint32(i, true) === 0x06054b50) { eocd = i; break; }
  }
  if (eocd < 0) throw new Error("Not a ZIP/WSZ file");
  const count = v.getUint16(eocd + 10, true);
  let p = v.getUint32(eocd + 16, true);
  for (let n = 0; n < count && p + 46 <= buf.byteLength; n++) {
    if (v.getUint32(p, true) !== 0x02014b50) break;
    const method = v.getUint16(p + 10, true);
    const csize = v.getUint32(p + 20, true);
    const nameLen = v.getUint16(p + 28, true);
    const extraLen = v.getUint16(p + 30, true);
    const commentLen = v.getUint16(p + 32, true);
    const local = v.getUint32(p + 42, true);
    const name = new TextDecoder("latin1").decode(new Uint8Array(buf, p + 46, nameLen));
    p += 46 + nameLen + extraLen + commentLen;
    if (name.endsWith("/")) continue;
    const lNameLen = v.getUint16(local + 26, true);
    const lExtraLen = v.getUint16(local + 28, true);
    const start = local + 30 + lNameLen + lExtraLen;
    const data = new Uint8Array(buf, start, Math.min(csize, buf.byteLength - start));
    let bytes: Uint8Array | null = null;
    if (method === 0) bytes = data.slice();
    else if (method === 8) {
      const s = new Blob([data]).stream().pipeThrough(new DecompressionStream("deflate-raw"));
      bytes = new Uint8Array(await new Response(s).arrayBuffer());
    }
    if (bytes) out.set(name.split(/[\\/]/).pop()!.toLowerCase(), bytes);
  }
  return out;
}

export type Rect = [x: number, y: number, w: number, h: number];

/** Sprite rectangles inside each sheet. */
export const SPRITES = {
  main: { bg: [0, 0, 275, 116] as Rect },
  titlebar: {
    active: [27, 0, 275, 14] as Rect, inactive: [27, 15, 275, 14] as Rect,
    options: [0, 0, 9, 9] as Rect, optionsDown: [0, 9, 9, 9] as Rect,
    minimize: [9, 0, 9, 9] as Rect, minimizeDown: [9, 9, 9, 9] as Rect,
    close: [18, 0, 9, 9] as Rect, closeDown: [18, 9, 9, 9] as Rect,
    shade: [0, 18, 9, 9] as Rect, shadeDown: [9, 18, 9, 9] as Rect,
  },
  cbuttons: {
    prev: [0, 0, 23, 18] as Rect, play: [23, 0, 23, 18] as Rect, pause: [46, 0, 23, 18] as Rect,
    stop: [69, 0, 23, 18] as Rect, next: [92, 0, 22, 18] as Rect, eject: [114, 0, 22, 16] as Rect,
  },
  playpaus: { playing: [0, 0, 9, 9] as Rect, paused: [9, 0, 9, 9] as Rect, stopped: [18, 0, 9, 9] as Rect },
  monoster: { stereoOn: [0, 0, 29, 12] as Rect, stereoOff: [0, 12, 29, 12] as Rect, monoOn: [29, 0, 27, 12] as Rect, monoOff: [29, 12, 27, 12] as Rect },
  posbar: { bg: [0, 0, 248, 10] as Rect, thumb: [248, 0, 29, 10] as Rect, thumbDown: [278, 0, 29, 10] as Rect },
  shufrep: {
    repeat: [0, 0, 28, 15] as Rect, shuffle: [28, 0, 47, 15] as Rect,
    eq: [0, 61, 23, 12] as Rect, pl: [23, 61, 23, 12] as Rect,
  },
  volumeThumb: [15, 422, 14, 11] as Rect,
  volumeThumbDown: [0, 422, 14, 11] as Rect,
};

/** Where each element sits on the 275×116 main window. */
export const POS = {
  options: [6, 3], minimize: [244, 3], shade: [254, 3], close: [264, 3],
  prev: [16, 88], play: [39, 88], pause: [62, 88], stop: [85, 88], next: [108, 88], eject: [136, 89],
  playpaus: [26, 28], minus: [36, 32], digits: [[48, 26], [60, 26], [78, 26], [90, 26]],
  marquee: [111, 27, 154, 6], kbps: [111, 43], khz: [156, 43],
  mono: [212, 41], stereo: [239, 41], vis: [24, 43, 76, 16],
  volume: [107, 57, 68, 13], balance: [177, 57, 38, 13], posbar: [16, 72, 248, 10],
  shuffle: [164, 89], repeat: [210, 89], eq: [219, 58], pl: [242, 58],
} as const;

/** text.bmp: 5×6 cells. Returns the [x, y] of a character, or null for unsupported ones (drawn as space). */
const ROW0 = "abcdefghijklmnopqrstuvwxyz\"@";
const ROW1 = "0123456789….:()-'!_+\\/[]^&%,=$#";
const ROW2 = "åöä?*";
export function glyph(ch: string): [number, number] {
  const c = ch.toLowerCase();
  let i = ROW0.indexOf(c);
  if (i >= 0) return [i * 5, 0];
  i = ROW1.indexOf(c);
  if (i >= 0) return [i * 5, 6];
  i = ROW2.indexOf(c);
  if (i >= 0) return [i * 5, 12];
  return [30 * 5, 0]; // space
}

export interface Skin {
  name: string;
  /** Object URLs for each sheet that the skin provides. */
  sheets: Record<string, string>;
  /** 24 visualiser colours (index 0 = background). */
  viscolors: string[];
  urls: string[];
}

const SHEETS = ["main", "titlebar", "cbuttons", "playpaus", "monoster", "posbar", "shufrep", "volume", "balance", "numbers", "nums_ex", "text"];

function parseViscolor(text: string): string[] {
  const cols: string[] = [];
  for (const line of text.split(/\r?\n/)) {
    const m = line.match(/^\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/);
    if (m) cols.push(`rgb(${m[1]},${m[2]},${m[3]})`);
  }
  return cols.length >= 24 ? cols.slice(0, 24) : DEFAULT_VISCOLORS;
}

export const DEFAULT_VISCOLORS = [
  "rgb(0,0,0)", "rgb(24,33,41)", "rgb(239,49,16)", "rgb(206,41,16)", "rgb(214,90,0)", "rgb(214,102,0)",
  "rgb(214,115,0)", "rgb(198,123,8)", "rgb(222,165,24)", "rgb(214,181,33)", "rgb(189,222,41)", "rgb(148,222,33)",
  "rgb(41,206,16)", "rgb(50,190,16)", "rgb(57,181,16)", "rgb(49,156,8)", "rgb(41,148,0)", "rgb(24,132,8)",
  "rgb(255,255,255)", "rgb(214,214,222)", "rgb(181,189,189)", "rgb(160,170,175)", "rgb(148,156,165)", "rgb(150,150,150)",
];

/** Loads a .wsz into a Skin. Missing sheets are tolerated (those parts are drawn empty). */
export async function loadSkin(name: string, data: ArrayBuffer): Promise<Skin> {
  const files = await unzip(data);
  if (!files.has("main.bmp") || !(files.has("cbuttons.bmp") || files.has("titlebar.bmp"))) {
    throw new Error("This is not a Winamp 2.x classic skin (needs main.bmp and cbuttons.bmp)");
  }
  const sheets: Record<string, string> = {};
  const urls: string[] = [];
  for (const s of SHEETS) {
    const bytes = files.get(`${s}.bmp`) ?? files.get(`${s}.png`);
    if (!bytes) continue;
    const url = URL.createObjectURL(new Blob([bytes as unknown as ArrayBuffer], { type: files.has(`${s}.bmp`) ? "image/bmp" : "image/png" }));
    sheets[s] = url;
    urls.push(url);
  }
  // Skins without numbers.bmp use nums_ex.bmp (same layout for the digits).
  if (!sheets.numbers && sheets.nums_ex) sheets.numbers = sheets.nums_ex;
  const vis = files.get("viscolor.txt");
  return { name, sheets, urls, viscolors: vis ? parseViscolor(new TextDecoder("latin1").decode(vis)) : DEFAULT_VISCOLORS };
}

export function freeSkin(s: { urls: string[] } | null) {
  s?.urls.forEach((u) => URL.revokeObjectURL(u));
}
