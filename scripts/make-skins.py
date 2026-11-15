"""Draws Halcyon's built-in classic skins as Winamp 2.x .wsz files (original artwork).

  python scripts/make-skins.py      -> public/skins/silverline.wsz, public/skins/phosphor.wsz

Silverline: brushed silver body, blue LCD, glossy buttons (2000s laptop-media-player mood).
Phosphor:   dark graphite body, green phosphor LCD.
Every sheet follows the classic skin layout so the same renderer also shows any
third-party .wsz skin.
"""
import io
import os
import zipfile

from PIL import Image, ImageDraw

# 3x5 pixel glyphs (drawn at column 1 of each 5x6 text.bmp cell).
FONT = {
    "a": "010101111101101", "b": "110101110101110", "c": "011100100100011", "d": "110101101101110",
    "e": "111100110100111", "f": "111100110100100", "g": "011100101101011", "h": "101101111101101",
    "i": "111010010010111", "j": "001001001101010", "k": "101101110101101", "l": "100100100100111",
    "m": "101111111101101", "n": "110101101101101", "o": "010101101101010", "p": "110101110100100",
    "q": "010101101110011", "r": "110101110101101", "s": "011100010001110", "t": "111010010010010",
    "u": "101101101101111", "v": "101101101101010", "w": "101101111111101", "x": "101101010101101",
    "y": "101101010010010", "z": "111001010100111",
    "0": "111101101101111", "1": "010110010010111", "2": "110001010100111", "3": "110001010001110",
    "4": "101101111001001", "5": "111100110001110", "6": "011100111101111", "7": "111001010010010",
    "8": "111101111101111", "9": "111101111001110",
    '"': "101101000000000", "@": "010101111100011", "…": "000000000000101", ".": "000000000000010",
    ":": "000010000010000", "(": "001010010010001", ")": "100010010010100", "-": "000000111000000",
    "'": "010010000000000", "!": "010010010000010", "_": "000000000000111", "+": "000010111010000",
    "\\": "100100010001001", "/": "001001010100100", "[": "110100100100110", "]": "011001001001011",
    "^": "010101000000000", "&": "010101010101011", "%": "101001010100101", ",": "000000000010100",
    "=": "000111000111000", "$": "011110010011110", "#": "101111101111101",
    "å": "010000111101111", "ö": "101010101101010", "ä": "101010101111101", "?": "110001010000010",
    "*": "101010101000000",
}
ROW0 = "abcdefghijklmnopqrstuvwxyz\"@"
ROW1 = "0123456789….:()-'!_+\\/[]^&%,=$#"
ROW2 = "åöä?*"

THEMES = {
    "silverline": dict(
        body_top=(214, 219, 226), body_bot=(160, 168, 180), brush=6, edge_hi=(250, 252, 255), edge_lo=(96, 104, 118),
        lcd=(10, 24, 46), lcd_edge=(52, 84, 130), lcd_text=(120, 200, 255), lcd_dim=(28, 52, 86),
        title_bg=(40, 70, 130), title_bg2=(90, 130, 200), title_text=(240, 246, 255),
        btn_top=(246, 248, 252), btn_bot=(176, 184, 198), btn_icon=(40, 52, 76), btn_edge=(98, 108, 126),
        accent=(60, 140, 255), accent2=(150, 210, 255), led_off=(70, 80, 96),
        vis=[(10, 24, 46), (40, 70, 110)] + [(min(255, 60 + i * 12), min(255, 140 + i * 6), 255) for i in range(16)]
        + [(200, 230, 255)] * 5 + [(255, 255, 255)],
    ),
    "phosphor": dict(
        body_top=(58, 62, 66), body_bot=(28, 30, 33), brush=3, edge_hi=(110, 116, 122), edge_lo=(10, 10, 12),
        lcd=(4, 14, 6), lcd_edge=(30, 70, 34), lcd_text=(80, 255, 110), lcd_dim=(18, 54, 24),
        title_bg=(20, 22, 24), title_bg2=(48, 52, 56), title_text=(140, 255, 160),
        btn_top=(76, 80, 86), btn_bot=(36, 38, 42), btn_icon=(170, 255, 180), btn_edge=(8, 8, 10),
        accent=(60, 220, 90), accent2=(170, 255, 180), led_off=(30, 44, 32),
        vis=[(4, 14, 6), (20, 50, 24)] + [(min(255, 40 + i * 14), 255 - i * 6, 60) for i in range(16)]
        + [(140, 255, 160)] * 5 + [(255, 255, 255)],
    ),
}


def vgrad(d, box, top, bot):
    x0, y0, x1, y1 = box
    for y in range(y0, y1 + 1):
        t = (y - y0) / max(1, y1 - y0)
        d.line([(x0, y), (x1, y)], fill=tuple(int(top[i] + (bot[i] - top[i]) * t) for i in range(3)))


def bevel(d, box, hi, lo):
    x0, y0, x1, y1 = box
    d.line([(x0, y1), (x0, y0), (x1, y0)], fill=hi)
    d.line([(x1, y0), (x1, y1), (x0, y1)], fill=lo)


def text(img, s, x, y, color, narrow=False):
    px = img.load()
    for ch in s.lower():
        g = FONT.get(ch)
        if g:
            for r in range(5):
                for c in range(3):
                    if g[r * 3 + c] == "1":
                        px[x + c, y + r] = color
        x += 4 if narrow else 5


def brushed(img, box, top, bot, amount, seed=7):
    d = ImageDraw.Draw(img)
    vgrad(d, box, top, bot)
    if amount:
        px = img.load()
        x0, y0, x1, y1 = box
        v = seed
        for y in range(y0, y1 + 1):
            v = (v * 1103515245 + 12345) & 0x7FFFFFFF
            n = (v >> 16) % (amount * 2 + 1) - amount
            for x in range(x0, x1 + 1):
                r, g, b = px[x, y][:3]
                px[x, y] = (max(0, min(255, r + n)), max(0, min(255, g + n)), max(0, min(255, b + n)))


def make(t):
    files = {}

    # ---------------- main.bmp
    m = Image.new("RGB", (275, 116))
    d = ImageDraw.Draw(m)
    brushed(m, (0, 0, 274, 115), t["body_top"], t["body_bot"], t["brush"])
    bevel(d, (0, 0, 274, 115), t["edge_hi"], t["edge_lo"])
    # LCD: time + status + visualiser
    d.rectangle((9, 22, 104, 61), fill=t["lcd"])
    bevel(d, (9, 22, 104, 61), t["edge_lo"], t["edge_hi"])
    d.rectangle((24, 43, 99, 58), fill=t["vis"][0])
    # Marquee, kbps, khz windows
    for box in [(108, 23, 266, 35), (108, 40, 127, 50), (153, 40, 167, 50)]:
        d.rectangle(box, fill=t["lcd"])
        bevel(d, box, t["edge_lo"], t["edge_hi"])
    text(m, "kbps", 130, 43, t["btn_icon"] if t["brush"] > 4 else t["lcd_text"], narrow=True)
    text(m, "khz", 170, 43, t["btn_icon"] if t["brush"] > 4 else t["lcd_text"], narrow=True)
    # Slider and button wells
    for box in [(106, 56, 176, 70), (176, 56, 216, 70), (15, 71, 265, 82), (14, 86, 160, 107)]:
        bevel(d, box, t["edge_lo"], t["edge_hi"])
    text(m, "halcyon", 241, 95, t["accent"], narrow=True)
    files["main.bmp"] = m

    # ---------------- titlebar.bmp (27..302 = bars, 0..27 = small buttons)
    tb = Image.new("RGB", (344, 87), t["body_bot"])
    d = ImageDraw.Draw(tb)
    for y0, a, b in [(0, t["title_bg2"], t["title_bg"]), (15, t["body_top"], t["body_bot"])]:
        vgrad(d, (27, y0, 301, y0 + 13), a, b)
        bevel(d, (27, y0, 301, y0 + 13), t["edge_hi"], t["edge_lo"])
        text(tb, "halcyon", 27 + 112, y0 + 4, t["title_text"] if y0 == 0 else t["edge_lo"])
    def small(x, y, glyph, down):
        vgrad(d, (x, y, x + 8, y + 8), t["btn_bot"] if down else t["btn_top"], t["btn_top"] if down else t["btn_bot"])
        bevel(d, (x, y, x + 8, y + 8), t["edge_lo"] if down else t["edge_hi"], t["btn_edge"])
        for (gx, gy) in glyph:
            d.point((x + gx, y + gy), fill=t["btn_icon"])
    dash = [(2, 6), (3, 6), (4, 6), (5, 6), (6, 6)]
    cross = [(2, 2), (3, 3), (4, 4), (5, 5), (6, 6), (6, 2), (5, 3), (3, 5), (2, 6)]
    shade = [(2, 2), (3, 2), (4, 2), (5, 2), (6, 2), (2, 3), (6, 3)]
    menu = [(2, 2), (3, 2), (4, 2), (5, 2), (6, 2), (2, 4), (3, 4), (4, 4), (5, 4), (6, 4)]
    for down in (0, 1):
        small(0, 9 * down, menu, down)
        small(9, 9 * down, dash, down)
        small(18, 9 * down, cross, down)
        small(9 * down, 18, shade, down)
    files["titlebar.bmp"] = tb

    # ---------------- cbuttons.bmp
    cb = Image.new("RGB", (136, 36), t["body_bot"])
    d = ImageDraw.Draw(cb)
    def tri(x, y, right=True, w=5, h=7):
        pts = [(x, y), (x, y + h - 1), (x + w, y + h // 2)] if right else [(x + w, y), (x + w, y + h - 1), (x, y + h // 2)]
        d.polygon(pts, fill=t["btn_icon"])
    icons = {
        "prev": lambda x, y: (d.rectangle((x + 6, y + 5, x + 7, y + 11), fill=t["btn_icon"]), tri(x + 8, y + 5, False), tri(x + 13, y + 5, False)),
        "play": lambda x, y: tri(x + 9, y + 4, True, 6, 9),
        "pause": lambda x, y: (d.rectangle((x + 8, y + 5, x + 9, y + 12), fill=t["btn_icon"]), d.rectangle((x + 13, y + 5, x + 14, y + 12), fill=t["btn_icon"])),
        "stop": lambda x, y: d.rectangle((x + 8, y + 5, x + 14, y + 11), fill=t["btn_icon"]),
        "next": lambda x, y: (tri(x + 4, y + 5), tri(x + 9, y + 5), d.rectangle((x + 14, y + 5, x + 15, y + 11), fill=t["btn_icon"])),
        "eject": lambda x, y: (d.polygon([(x + 6, y + 9), (x + 15, y + 9), (x + 10, y + 4)], fill=t["btn_icon"]), d.rectangle((x + 6, y + 11, x + 15, y + 12), fill=t["btn_icon"])),
    }
    for name, (x, w, h) in {"prev": (0, 23, 18), "play": (23, 23, 18), "pause": (46, 23, 18), "stop": (69, 23, 18), "next": (92, 22, 18), "eject": (114, 22, 16)}.items():
        for down in (0, 1):
            y = h * down
            a, b = (t["btn_bot"], t["btn_top"]) if down else (t["btn_top"], t["btn_bot"])
            vgrad(d, (x, y, x + w - 1, y + h - 1), a, b)
            bevel(d, (x, y, x + w - 1, y + h - 1), t["edge_lo"] if down else t["edge_hi"], t["btn_edge"])
            icons[name](x + down, y + down)
    files["cbuttons.bmp"] = cb

    # ---------------- playpaus.bmp
    pp = Image.new("RGB", (42, 9), t["lcd"])
    d = ImageDraw.Draw(pp)
    d.polygon([(2, 1), (2, 7), (7, 4)], fill=t["lcd_text"])
    d.rectangle((11, 1, 12, 7), fill=t["lcd_text"]); d.rectangle((15, 1, 16, 7), fill=t["lcd_text"])
    d.rectangle((20, 2, 25, 7), fill=t["lcd_text"])
    files["playpaus.bmp"] = pp

    # ---------------- monoster.bmp
    ms = Image.new("RGB", (56, 24), t["lcd"])
    for y, col in [(0, t["lcd_text"]), (12, t["lcd_dim"])]:
        text(ms, "stereo", 1, y + 4, col, narrow=True)
        text(ms, "mono", 29 + 5, y + 4, col, narrow=True)
    files["monoster.bmp"] = ms

    # ---------------- posbar.bmp
    pb = Image.new("RGB", (307, 10), t["body_bot"])
    d = ImageDraw.Draw(pb)
    d.rectangle((0, 3, 247, 6), fill=t["lcd"])
    bevel(d, (0, 3, 247, 6), t["edge_lo"], t["edge_hi"])
    for x0, down in [(248, 0), (278, 1)]:
        vgrad(d, (x0, 0, x0 + 28, 9), t["accent2"] if down else t["btn_top"], t["accent"] if down else t["btn_bot"])
        bevel(d, (x0, 0, x0 + 28, 9), t["edge_hi"], t["btn_edge"])
        d.line([(x0 + 14, 2), (x0 + 14, 7)], fill=t["btn_edge"])
    files["posbar.bmp"] = pb

    # ---------------- volume.bmp / balance.bmp (28 backgrounds + thumbs at y 422)
    def slider_sheet(kind):
        im = Image.new("RGB", (68, 433), t["body_bot"])
        d = ImageDraw.Draw(im)
        for i in range(28):
            y = i * 15
            x0, w = (0, 68) if kind == "volume" else (9, 38)
            d.rectangle((x0, y + 4, x0 + w - 1, y + 8), fill=t["lcd"])
            fill = int((w - 2) * i / 27)
            if kind == "volume":
                for x in range(fill):
                    f = x / max(1, w - 2)
                    col = tuple(int(t["accent"][c] + (t["accent2"][c] - t["accent"][c]) * f) for c in range(3))
                    d.line([(x0 + 1 + x, y + 5), (x0 + 1 + x, y + 7)], fill=col)
            else:
                mid = x0 + w // 2
                d.rectangle((mid - fill // 2, y + 5, mid + fill // 2, y + 7), fill=t["accent"])
            bevel(d, (x0, y + 4, x0 + w - 1, y + 8), t["edge_lo"], t["edge_hi"])
        for x0, down in [(0, 1), (15, 0)]:
            vgrad(d, (x0, 422, x0 + 13, 432), t["accent2"] if down else t["btn_top"], t["accent"] if down else t["btn_bot"])
            bevel(d, (x0, 422, x0 + 13, 432), t["edge_hi"], t["btn_edge"])
        return im
    files["volume.bmp"] = slider_sheet("volume")
    files["balance.bmp"] = slider_sheet("balance")

    # ---------------- shufrep.bmp
    sr = Image.new("RGB", (92, 85), t["body_bot"])
    d = ImageDraw.Draw(sr)
    def toggle(x, y, w, h, label, sel, down):
        a, b = (t["btn_bot"], t["btn_top"]) if down else (t["btn_top"], t["btn_bot"])
        vgrad(d, (x, y, x + w - 1, y + h - 1), a, b)
        bevel(d, (x, y, x + w - 1, y + h - 1), t["edge_lo"] if down else t["edge_hi"], t["btn_edge"])
        d.rectangle((x + 3, y + h // 2 - 1, x + 5, y + h // 2 + 1), fill=t["accent2"] if sel else t["led_off"])
        text(sr, label, x + 8 + down, y + (h - 5) // 2 + down, t["btn_icon"], narrow=True)
    for i, (sel, down) in enumerate([(0, 0), (0, 1), (1, 0), (1, 1)]):
        toggle(0, i * 15, 28, 15, "rep", sel, down)
        toggle(28, i * 15, 47, 15, "shuffle", sel, down)
    for sel in (0, 1):
        for down in (0, 1):
            toggle(46 * down, 61 + 12 * sel, 23, 12, "eq", sel, down)
            toggle(23 + 46 * down, 61 + 12 * sel, 23, 12, "pl", sel, down)
    files["shufrep.bmp"] = sr

    # ---------------- numbers.bmp (7-segment digits, 9x13 each)
    nb = Image.new("RGB", (99, 13), t["lcd"])
    d = ImageDraw.Draw(nb)
    SEG = {0: "abcdef", 1: "bc", 2: "abged", 3: "abgcd", 4: "fgbc", 5: "afgcd", 6: "afgedc", 7: "abc", 8: "abcdefg", 9: "abcdfg"}
    def seg(x, s, col):
        lines = {"a": [(2, 1), (6, 1)], "b": [(7, 2), (7, 5)], "c": [(7, 7), (7, 10)], "d": [(2, 11), (6, 11)],
                 "e": [(1, 7), (1, 10)], "f": [(1, 2), (1, 5)], "g": [(2, 6), (6, 6)]}
        for k in "abcdefg":
            d.line([(x + p[0], p[1]) for p in lines[k]], fill=col if k in s else t["lcd_dim"], width=1)
    for n in range(10):
        seg(n * 9, SEG[n], t["lcd_text"])
    files["numbers.bmp"] = nb

    # ---------------- text.bmp (5x6 cells; background = marquee background)
    tx = Image.new("RGB", (155, 18), t["lcd"])
    for row, chars in enumerate([ROW0, ROW1, ROW2]):
        for i, ch in enumerate(chars):
            g = FONT.get(ch)
            if g:
                px = tx.load()
                for r in range(5):
                    for c in range(3):
                        if g[r * 3 + c] == "1":
                            px[i * 5 + 1 + c, row * 6 + r] = t["lcd_text"]
    files["text.bmp"] = tx

    out = io.BytesIO()
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        for name, img in files.items():
            b = io.BytesIO()
            img.save(b, "BMP")
            z.writestr(name, b.getvalue())
        z.writestr("viscolor.txt", "\n".join(f"{r},{g},{b}," for r, g, b in t["vis"]) + "\n")
        z.writestr("readme.txt", "Halcyon built-in classic skin. Original artwork drawn by scripts/make-skins.py.\n")
    return out.getvalue()


if __name__ == "__main__":
    root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "public", "skins")
    os.makedirs(root, exist_ok=True)
    for name, theme in THEMES.items():
        data = make(theme)
        with open(os.path.join(root, f"{name}.wsz"), "wb") as f:
            f.write(data)
        print(name, len(data), "bytes")
