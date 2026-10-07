#!/usr/bin/env python3
"""The Bhote dog (a Himalayan sheepdog: thick mane, small drop ears, light brow spots, broad dark muzzle) as a pixel grid.
   python3 tools/dog.py png OUT.png   a scaled preview image
   python3 tools/dog.py bash          the grid as letter rows (for bhote)"""
import math, sys, zlib, struct
W, H = 38, 32
cx = (W - 1) / 2
g = [["."] * W for _ in range(H)]

PALETTE = {
    "a": "#9b6a45", "b": "#c98f5c", "c": "#e8b98a", "C": "#f6dcb8",   # mane: dark outer fur -> light inner fur
    "d": "#5d3d2e", "e": "#7b5340",                                   # ears
    "f": "#d8a468", "g": "#ecc088",                                   # face, forehead
    "h": "#fbeedd",                                                   # brow spots
    "i": "#1b1523", "j": "#94e2d5",                                   # eyes, glint
    "k": "#3a2c2b", "l": "#5b4440",                                   # muzzle, muzzle light
    "m": "#120f18", "n": "#bac2de",                                   # nose, shine
    "o": "#1b1523", "p": "#f38ba8", "q": "#d46a8c",                   # mouth, tongue, tongue shade
}

def ell(x, y, ex, ey, rx, ry): return ((x - ex) / rx) ** 2 + ((y - ey) / ry) ** 2
def hashf(a, b): return (math.sin(a * 12.9898 + b * 78.233) * 43758.5453) % 1.0
def fill(fn, ch):
    for y in range(H):
        for x in range(W):
            if fn(x, y): g[y][x] = ch
def tri(p, a, b, c):
    def s(p1, p2, p3): return (p1[0]-p3[0])*(p2[1]-p3[1]) - (p2[0]-p3[0])*(p1[1]-p3[1])
    d1, d2, d3 = s(p, a, b), s(p, b, c), s(p, c, a)
    return not ((d1 < 0 or d2 < 0 or d3 < 0) and (d1 > 0 or d2 > 0 or d3 > 0))

# --- mane: a big ellipse with fur tufts along the edge; tone by distance from the face, with hair-like streaks
MC = (cx, 17.5)
def mane_r(x, y):
    a = math.atan2(y - MC[1], x - MC[0])
    tuft = abs(math.sin(a * 8.5)) * 0.16 + abs(math.sin(a * 21 + 1)) * 0.05
    return ell(x, y, MC[0], MC[1], 17.6 * (1 + tuft), 14.2 * (1 + tuft))
for y in range(H):
    for x in range(W):
        r = mane_r(x, y)
        if r <= 1:
            streak = hashf(x // 1, y // 2)                    # little vertical strands
            t = r + (streak - 0.5) * 0.28
            g[y][x] = "a" if t > 0.80 else "b" if t > 0.55 else "c" if t > 0.32 else "C"

# --- ears: small drop ears lying on the mane, dark with a lighter inner edge
for s in (1, -1):
    def X(v): return cx + s * (v - cx) if s == 1 else cx - (v - cx)
    pts = lambda p: ((p[0] if s == 1 else W - 1 - p[0]), p[1])
    fill(lambda x, y: tri((x, y), pts((3.5, 15)), pts((5.0, 3)), pts((14.5, 8.5))), "d")
    fill(lambda x, y: tri((x, y), pts((6.5, 12)), pts((7.0, 6.2)), pts((12.5, 9))), "e")

# --- head: broad, tan, lighter forehead
fill(lambda x, y: ell(x, y, cx, 16.5, 10.8, 10.2) <= 1, "f")
fill(lambda x, y: ell(x, y, cx, 9.5, 7.5, 4.2) <= 1 and y < 12, "g")

# --- muzzle: broad and dark, lighter on top of the nose bridge
fill(lambda x, y: ell(x, y, cx, 22.2, 8.2, 6.4) <= 1 and y >= 17, "k")
fill(lambda x, y: ell(x, y, cx, 19.4, 4.2, 2.6) <= 1, "l")

# --- brow spots (the "four-eyed" look), eyes with a glint
for s in (-1, 1):
    ex = cx + s * 5.4
    fill(lambda x, y, ex=ex: ell(x, y, ex, 8.0, 2.1, 1.3) <= 1, "h")
    fill(lambda x, y, ex=ex: ell(x, y, ex, 11.4, 1.9, 1.9) <= 1, "i")
    fill(lambda x, y, ex=ex, s=s: ell(x, y, ex - 0.55 * s * -1, 10.7, 0.6, 0.6) <= 1, "j")

# --- nose, mouth, tongue
fill(lambda x, y: ell(x, y, cx, 17.6, 3.3, 2.0) <= 1, "m")
fill(lambda x, y: ell(x, y, cx - 1.0, 16.9, 0.9, 0.5) <= 1, "n")
fill(lambda x, y: abs(x - cx) < 0.7 and 19.4 <= y <= 22, "o")
for s in (-1, 1):
    fill(lambda x, y, s=s: ell(x, y, cx + s * 3.2, 22.2, 3.4, 1.6) <= 1 and y >= 22.0 and s * (x - cx) > 0.5, "o")
fill(lambda x, y: ell(x, y, cx, 24.6, 2.4, 3.0) <= 1 and y >= 22.4, "p")
fill(lambda x, y: abs(x - cx) < 0.5 and 22.8 <= y <= 26.2, "q")

# ---- the small dog is drawn by hand (a scaled-down big one turns into a smudge): the left half, 7 columns, mirrored
MINI_HALF = [
    "...aabb",
    "..abbcc",
    ".addcCf",
    ".addhhf",
    "abdciff",
    "abcciff",
    "abcffmm",
    "abcfkkk",
    "abckkkp",
    "abcckkp",
    ".abbcck",
    "...aabb",
]
def mini_grid():
    return [list(r + r[::-1]) for r in MINI_HALF]

# ---- animation variants: bit 0 = eyes closed, bit 1 = tongue in
def variant(grid, closed, tongue_in):
    v = [row[:] for row in grid]
    h, w = len(v), len(v[0])
    if tongue_in:
        for y in range(h):
            for x in range(w):
                if v[y][x] in "pq": v[y][x] = "k"
    if closed:
        # per eye (left / right half): drop the eye colours, then draw one dark lid line across its middle row
        for half in (range(0, w // 2), range(w // 2, w)):
            px = [(x, y) for y in range(h) for x in half if v[y][x] in "ij"]
            if not px: continue
            for x, y in px: v[y][x] = "f"
            ys = sorted(y for _, y in px); ym = ys[len(ys) // 2]
            xs = [x for x, y in px if y == ym] or [x for x, _ in px]
            if max(xs) - min(xs) >= 1:
                for x in range(min(xs), max(xs) + 1): v[ym][x] = "i"
            else:                      # a 1-pixel-wide eye (the small dog): squint, keep only its lowest pixel
                x0 = px[0][0]
                v[max(y for _, y in px)][x0] = "i"
    return v

def hex2rgb(h): return tuple(int(h[i:i + 2], 16) for i in (1, 3, 5))
def png(path, scale=14, bg=(30, 30, 46)):
    w, h = W * scale, H * scale
    raw = bytearray()
    for y in range(h):
        raw.append(0)
        row = g[y // scale]
        for x in range(w):
            ch = row[x // scale]
            raw += bytes(hex2rgb(PALETTE[ch]) if ch in PALETTE else bg)
    def chunk(t, d): return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    open(path, "wb").write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
                           + chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b""))
if sys.argv[1:2] == ["png"]:
    g = variant(g, "closed" in sys.argv, "tongue_in" in sys.argv)
    png(sys.argv[2])
elif sys.argv[1:2] == ["bash"]:
    for y in range(0, H, 2): print("".join(g[y]) + "|" + "".join(g[y + 1]))

# ---- a small version for the header: blocks of f x f pixels -> one pixel, features (eyes, nose, tongue ...) win over fur
FEATURE_WEIGHT = {"i": 3, "j": 4, "h": 2, "m": 3, "n": 3, "p": 3, "q": 2, "o": 1.5, "d": 1.4}
def downscale(f):
    mw, mh = -(-W // f), -(-H // f)
    mh += mh % 2
    rows = []
    for by in range(mh):
        row = ""
        for bx in range(mw):
            cnt = {}
            for y in range(by * f, min((by + 1) * f, H)):
                for x in range(bx * f, min((bx + 1) * f, W)):
                    cnt[g[y][x]] = cnt.get(g[y][x], 0) + FEATURE_WEIGHT.get(g[y][x], 1)
            total = sum(cnt.values()) or 1
            solid = [k for k in cnt if k != "."]
            row += "." if (not solid or cnt.get(".", 0) * 2 > total) else max(solid, key=lambda k: cnt[k])
        rows.append(row)
    return rows

if sys.argv[1:2] == ["minipng"]:
    f = int(sys.argv[3]) if len(sys.argv) > 3 else 3
    g = mini_grid(); W, H = len(g[0]), len(g)
    png(sys.argv[2], scale=24)


if sys.argv[1:2] == ["bashfn"]:
    # the code between "# >>> dog" and "# <<< dog" in bhote: palette, the grids (4 animation variants each) and the renderer
    def rows_of(grid):
        return ["".join(grid[y]) + "|" + "".join(grid[y + 1]) for y in range(0, len(grid), 2)]
    small = mini_grid()
    out = ["# >>> dog (generated by tools/dog.py bashfn — do not edit by hand)", "dog_rgb() {   # letter -> r;g;b", "    case \"$1\" in"]
    for ch, hx in PALETTE.items():
        r, gg, b = hex2rgb(hx); out.append(f"        {ch}) printf '%s' '{r};{gg};{b}' ;;")
    out += ["    esac", "}", f"DOG_W={W}", f"DOG_H={H}", f"DOG_MINI_W={len(small[0])}", f"DOG_MINI_H={len(small)}"]
    for idx in range(4):   # idx = eyes closed (1) + tongue in (2)
        for name, grid in (("DOG_ROWS", g), ("DOG_MINI_ROWS", small)):
            out.append(f"{name}_{idx}='")
            out += rows_of(variant(grid, bool(idx & 1), bool(idx & 2)))
            out.append("'")
    out.append("""# The escape codes of every colour are made once (DOGC_x = foreground, DOGB_x = background), so drawing needs no
# subshell per cell. Two pixel rows per text row: ▀ with the upper pixel as foreground, the lower one as background.
dog_init() {
    local ch
    for ch in %s; do
        printf -v "DOGC_$ch" '%%s' "$(rgb "$(dog_rgb "$ch")")"
        printf -v "DOGB_$ch" '%%s' "${esc}[48;2;$(dog_rgb "$ch")m"
    done
}
dog_lines() {  # dog_lines <rows> <width>  ->  array DOGL with one coloured string per text row
    local rows=$1 w=$2 line top bot i t b out n=0 fg bg
    DOGL=()
    while IFS= read -r line; do
        [ -z "$line" ] && continue
        top=${line%%|*}; bot=${line##*|}; out=""
        for (( i = 0; i < w; i++ )); do
            t=${top:$i:1}; b=${bot:$i:1}
            if [ "$color" != 1 ]; then
                if [ "$t" = "." ] && [ "$b" = "." ]; then out="${out} "; else out="${out}█"; fi
            elif [ "$t" = "." ] && [ "$b" = "." ]; then out="${out} "
            elif [ "$b" = "." ]; then fg=DOGC_$t; out="${out}${!fg}▀${X}"
            elif [ "$t" = "." ]; then fg=DOGC_$b; out="${out}${!fg}▄${X}"
            else fg=DOGC_$t; bg=DOGB_$b; out="${out}${!fg}${!bg}▀${X}"; fi
        done
        DOGL[$n]=$out; n=$(( n + 1 ))
    done <<< "$rows"
}
dog_rows() {  # dog_rows <big|mini> <variant 0..3>  ->  the rows text of that variant
    local v; if [ "$1" = big ]; then v="DOG_ROWS_$2"; else v="DOG_MINI_ROWS_$2"; fi
    printf '%%s' "${!v}"
}
# The coloured rows of every picture are kept in variables (DOGR_<kind>_<variant>, rows joined by \001): made once in dog_warm
# at the start, so a repaint or an animation step never has to build a picture again.
dog_lines_for() {  # dog_lines_for <big|mini> <variant>  ->  array DOGL
    local v="DOGR_$1_$2" s n=0 w; s=${!v}
    if [ -z "$s" ]; then
        if [ "$1" = big ]; then w=$DOG_W; else w=$DOG_MINI_W; fi
        dog_lines "$(dog_rows "$1" "$2")" "$w"
        for (( n = 0; n < ${#DOGL[@]}; n++ )); do s="$s${DOGL[$n]}"$'\001'; done
        printf -v "$v" '%%s' "$s"; return
    fi
    DOGL=(); n=0
    while [ -n "$s" ]; do DOGL[$n]=${s%%%%$'\001'*}; s=${s#*$'\001'}; n=$(( n + 1 )); done
}
dog_warm() { local k i; for k in big mini; do for i in 0 1 2 3; do dog_lines_for $k $i; done; done; }
dog() {  # the big dog, centred in the frame
    local i
    dog_lines_for big "${DOG_IDX:-0}"
    for (( i = 0; i < ${#DOGL[@]}; i++ )); do mid "$(printf '%%*s' "$DOG_W" '')" "${DOGL[$i]}" "$DOG_W"; done
}
# For the animation only the text rows that differ from what is on screen are written (eyes, tongue), at an absolute
# position. DOG_PREV_B / DOG_PREV_M remember the rows on screen; dog_prev_reset sets them after every full repaint.
DOG_PREV_B=(); DOG_PREV_M=()
dog_prev_reset() {  # dog_prev_reset <splash|main>: only the dog that is on screen
    if [ "$1" = splash ]; then dog_lines_for big "${DOG_IDX:-0}"; DOG_PREV_B=("${DOGL[@]}")
    else dog_lines_for mini "${DOG_IDX:-0}"; DOG_PREV_M=("${DOGL[@]}"); fi
}
dog_at() {  # dog_at <big|mini> <row> <col>
    local kind=$1 row=$2 col=$3 i
    dog_lines_for "$kind" "${DOG_IDX:-0}"
    for (( i = 0; i < ${#DOGL[@]}; i++ )); do
        if [ "$kind" = big ]; then [ "${DOG_PREV_B[$i]:-}" = "${DOGL[$i]}" ] && continue; DOG_PREV_B[$i]=${DOGL[$i]}
        else [ "${DOG_PREV_M[$i]:-}" = "${DOGL[$i]}" ] && continue; DOG_PREV_M[$i]=${DOGL[$i]}; fi
        printf '%%s[%%d;%%dH%%s' "$esc" $(( row + i )) "$col" "${DOGL[$i]}"
    done
}
# <<< dog""" % " ".join(PALETTE.keys()))
    print("\n".join(out))


if sys.argv[1:2] == ["minivariants"]:
    base = mini_grid(); vs = [variant(base, bool(i & 1), bool(i & 2)) for i in range(4)]
    g = [sum((vs[k][y] + list(".") for k in range(4)), []) for y in range(len(base))]
    W, H = len(g[0]), len(g)
    png(sys.argv[2], scale=18)
