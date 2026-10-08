#!/usr/bin/env python3
"""A terminal frame with SGR colours (reset, bold, italic, 24-bit fore- and background) as an SVG picture.
   python3 tools/ansi2svg.py FRAME > out.svg   (used by tools/screenshot.sh for the README picture)"""
import re, sys, unicodedata
from xml.sax.saxutils import escape

BG, FG = "#1e1e2e", "#cdd6f4"          # Catppuccin Mocha base and text: the panel's catppuccin theme
CW, LH, FS, PAD = 8.6, 19, 14.5, 18    # cell width, line height, font size, padding (px)

def width(ch):
    if unicodedata.combining(ch): return 0
    return 2 if unicodedata.east_asian_width(ch) in "WF" else 1

def parse(text):
    """lines of cells: (char, fg, bg, bold, italic)"""
    rows = []
    for line in text.rstrip("\n").split("\n"):
        fg, bg, bold, ital, cells = None, None, False, False, []
        for part in re.split(r"(\x1b\[[0-9;]*m)", line):
            if part.startswith("\x1b["):
                codes = [int(c) if c else 0 for c in part[2:-1].split(";")]
                i = 0
                while i < len(codes):
                    c = codes[i]
                    if c == 0: fg, bg, bold, ital = None, None, False, False
                    elif c == 1: bold = True
                    elif c == 3: ital = True
                    elif c == 22: bold = False
                    elif c == 23: ital = False
                    elif c == 39: fg = None
                    elif c == 49: bg = None
                    elif c in (38, 48) and i + 4 < len(codes) and codes[i + 1] == 2:
                        col = "#%02x%02x%02x" % tuple(codes[i + 2:i + 5])
                        if c == 38: fg = col
                        else: bg = col
                        i += 4
                    i += 1
                continue
            for ch in re.sub(r"\x1b\[[0-9;?]*[A-Za-z]", "", part):
                cells.append((ch, fg, bg, bold, ital))
        rows.append(cells)
    return rows

def svg(rows):
    cols = max(sum(width(c[0]) for c in r) for r in rows)
    w, h = round(cols * CW + 2 * PAD), round(len(rows) * LH + 2 * PAD + 22)
    out = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" '
           f'font-family="ui-monospace, SFMono-Regular, Menlo, Consolas, monospace" font-size="{FS}">',
           f'<rect width="{w}" height="{h}" rx="10" fill="{BG}"/>']
    for k, c in enumerate(("#f38ba8", "#f9e2af", "#a6e3a1")):     # the window's three dots
        out.append(f'<circle cx="{PAD + 6 + k * 18}" cy="16" r="5.5" fill="{c}"/>')
    top = PAD + 22
    for y, r in enumerate(rows):
        x, base = 0, top + y * LH
        runs = []                                                 # (x, text, fg, bg, bold, italic)
        for ch, fg, bg, bold, ital in r:
            key = (fg, bg, bold, ital)
            if runs and runs[-1][2:] == key and runs[-1][0] + len(runs[-1][1]) == x and width(ch) == 1:
                runs[-1] = (runs[-1][0], runs[-1][1] + ch) + key
            else:
                runs.append((x, ch) + key)
            x += width(ch)
        for rx, t, fg, bg, bold, ital in runs:
            if bg:
                out.append(f'<rect x="{PAD + rx * CW:.1f}" y="{base - LH + 5}" width="{len(t) * CW:.1f}" height="{LH}" fill="{bg}"/>')
        for rx, t, fg, bg, bold, ital in runs:
            if not t.strip(): continue
            attrs = f' fill="{fg or FG}"' + (' font-weight="bold"' if bold else "") + (' font-style="italic"' if ital else "")
            out.append(f'<text x="{PAD + rx * CW:.1f}" y="{base}" xml:space="preserve"{attrs}>{escape(t)}</text>')
    out.append("</svg>")
    return "\n".join(out) + "\n"

if __name__ == "__main__":
    with open(sys.argv[1], encoding="utf-8") as f:
        sys.stdout.write(svg(parse(f.read())))
