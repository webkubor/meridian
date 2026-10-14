#!/usr/bin/env python3
"""Render structural variants of the Meridian mark side by side.

The bow-off-a-spine construction kept reading as a letterform (capital B, then
a key), so rather than tuning the same curve, this lays out six genuinely
different structures to pick from.

Usage: python3 -I variants.py   ->   /tmp/meridian-variants.png
"""

import math
import pathlib
import subprocess
import tempfile

HERE = pathlib.Path(__file__).resolve().parent
CANVAS = 1024
INSET = 100
BOX = CANVAS - 2 * INSET

NAVY_DEEP, NAVY_LIFT = "#070F1A", "#1B2430"
CYAN, AMBER, MIST = "#60E1FF", "#FFB020", "#E5E9F0"


def squircle(cx, cy, r, n=5, steps=720):
    pts = []
    for i in range(steps):
        t = 2 * math.pi * i / steps
        ct, st = math.cos(t), math.sin(t)
        pts.append(
            f"{cx + r * math.copysign(abs(ct) ** (2 / n), ct):.2f},"
            f"{cy + r * math.copysign(abs(st) ** (2 / n), st):.2f}"
        )
    return "M" + " L".join(pts) + " Z"


def diamond(cx, cy, h):
    return f"M{cx},{cy - h} L{cx + h},{cy} L{cx},{cy + h} L{cx - h},{cy} Z"


HEAD = f"""<defs>
 <radialGradient id="body" cx="50%" cy="34%" r="82%">
  <stop offset="0%" stop-color="{NAVY_LIFT}"/><stop offset="58%" stop-color="#101A29"/>
  <stop offset="100%" stop-color="{NAVY_DEEP}"/></radialGradient>
 <linearGradient id="thread" gradientUnits="userSpaceOnUse" x1="0" y1="150" x2="0" y2="700">
  <stop offset="0%" stop-color="#FFFFFF"/><stop offset="62%" stop-color="{MIST}"/>
  <stop offset="100%" stop-color="#B9C6DA"/></linearGradient>
 <linearGradient id="rim" x1="0" y1="0" x2="0.35" y2="1">
  <stop offset="0%" stop-color="{CYAN}" stop-opacity="0.30"/>
  <stop offset="46%" stop-color="#3B82F6" stop-opacity="0.10"/>
  <stop offset="100%" stop-color="{CYAN}" stop-opacity="0.34"/></linearGradient>
 <radialGradient id="halo" cx="50%" cy="50%" r="50%">
  <stop offset="0%" stop-color="{AMBER}" stop-opacity="0.55"/>
  <stop offset="55%" stop-color="{AMBER}" stop-opacity="0.16"/>
  <stop offset="100%" stop-color="{AMBER}" stop-opacity="0"/></radialGradient>
 <radialGradient id="sun" cx="42%" cy="36%" r="72%">
  <stop offset="0%" stop-color="#FFE7A8"/><stop offset="46%" stop-color="{AMBER}"/>
  <stop offset="100%" stop-color="#E8871A"/></radialGradient>
</defs>"""


def wrap(inner):
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{CANVAS}" height="{CANVAS}" '
        f'viewBox="0 0 {CANVAS} {CANVAS}">{HEAD}'
        f'<g transform="translate({INSET},{INSET})">'
        f'<path d="{squircle(BOX / 2, BOX / 2, BOX / 2)}" fill="url(#body)"/>'
        f'<path d="{squircle(BOX / 2, BOX / 2, BOX / 2 - 1.5)}" fill="none" '
        f'stroke="url(#rim)" stroke-width="3"/>{inner}</g></svg>'
    )


def sun(cx, cy, r):
    return (
        f'<circle cx="{cx}" cy="{cy}" r="{r * 3.4}" fill="url(#halo)"/>'
        f'<circle cx="{cx}" cy="{cy}" r="{r * 1.9}" fill="url(#halo)"/>'
        f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="url(#sun)"/>'
    )


def thread(paths, nodes, w):
    p = "".join(f'<path d="{d}"/>' for d in paths)
    n = "".join(
        f'<path d="{diamond(x, y, h)}" fill="url(#thread)" stroke="url(#thread)" '
        f'stroke-width="{w * 0.42}" stroke-linejoin="round"/>'
        for x, y, h in nodes
    )
    return (
        f'<g fill="none" stroke="url(#thread)" stroke-width="{w}" '
        f'stroke-linecap="round">{p}</g>{n}'
    )


def v_a_bow():  # current: gentle bow, thinner line
    sx, w, h = 398, 14, 26
    out = [f"M{sx},188 L{sx},708",
           f"M{sx},300 C{sx+104},310 {556},362 556,450 "
           f"C{556},538 {sx+100},594 {sx},600"]
    return sun(sx, 152, 36) + thread(out, [
        (sx, 300, h), (556, 450, h), (sx, 450, h), (sx, 600, h)], w)


def v_b_stub():  # branch as a stub that never returns
    sx, w, h = 400, 17, 25
    out = [f"M{sx},188 L{sx},700",
           f"M{sx},318 C{sx+80},318 {sx+132},352 {sx+140},420"]
    return sun(sx, 152, 36) + thread(out, [
        (sx, 318, h), (sx + 140, 420, h), (sx, 470, h), (sx, 610, h)], w)


def v_c_tilt():  # whole mark tilted, kills the letterform axis
    sx, w, h = 400, 17, 25
    out = [f"M{sx},190 L{sx},700",
           f"M{sx},300 C{sx+110},312 566,368 566,452 "
           f"C{566},536 {sx+106},590 {sx},600"]
    body = thread(out, [(sx, 300, h), (566, 452, h), (sx, 452, h), (sx, 600, h)], w)
    return sun(sx, 152, 36) + f'<g transform="rotate(-13 {sx} 440)">{body}</g>'


def v_d_two_branches():  # one fork each side, at different heights
    sx, w, h = 400, 16, 25
    out = [f"M{sx},188 L{sx},700",
           f"M{sx},292 C{sx+96},296 {sx+138},336 {sx+138},392 "
           f"C{sx+138},440 {sx+90},452 {sx},456",
           f"M{sx},470 C{sx-92},476 {sx-132},512 {sx-132},562 "
           f"C{sx-132},608 {sx-86},618 {sx},620"]
    return sun(sx, 152, 36) + thread(out, [
        (sx, 292, h), (sx + 138, 392, h), (sx, 456, h),
        (sx - 132, 562, h), (sx, 620, h)], w)


def v_e_pure_meridian():  # no branch at all: one line, nodes, sun
    sx, w, h = 400, 20, 30
    out = [f"M{sx},190 L{sx},700"]
    return sun(sx, 150, 38) + thread(out, [
        (sx, 300, h), (sx, 428, h), (sx, 556, h)], w)


def v_f_y_fork():  # the branch IS the lower half: a clean Y
    sx, w, h = 400, 17, 26
    out = [f"M{sx},186 L{sx},420",
           f"M{sx},420 C{sx+96},424 {sx+150},470 {sx+150},548 L{sx+150},700"]
    return sun(sx, 152, 36) + thread(out, [
        (sx, 300, h), (sx + 150, 548, h), (sx, 420, h)], w)


VARIANTS = [
    ("A  bow / thin", v_a_bow),
    ("B  stub", v_b_stub),
    ("C  tilted", v_c_tilt),
    ("D  two forks", v_d_two_branches),
    ("E  pure meridian", v_e_pure_meridian),
    ("F  Y fork", v_f_y_fork),
]


def main():
    from PIL import Image, ImageDraw

    tmp = pathlib.Path(tempfile.mkdtemp())
    tiles = []
    for i, (name, fn) in enumerate(VARIANTS):
        svg = tmp / f"v{i}.svg"
        png = tmp / f"v{i}.png"
        svg.write_text(wrap(fn()))
        subprocess.run(["rsvg-convert", "-w", "512", "-h", "512", "-o", str(png), str(svg)], check=True)
        tiles.append((name, Image.open(png).convert("RGBA"), Image.open(png).convert("RGBA").resize((128, 128), Image.LANCZOS)))

    W, H = 3 * 300 + 40, 2 * 400 + 60
    sheet = Image.new("RGB", (W, H), (245, 246, 248))
    d = ImageDraw.Draw(sheet)
    for i, (name, big, small) in enumerate(tiles):
        cx, cy = 20 + (i % 3) * 300, 20 + (i // 3) * 400
        sheet.paste(big, (cx + 20, cy + 30), big)
        d.text((cx + 24, cy + 6), name, fill=(20, 30, 50))
        d.text((cx + 24, cy + 548), "actual size ->", fill=(90, 100, 120))
        for j, sz in enumerate([128, 64, 32, 16]):
            t = big.resize((sz, sz), Image.LANCZOS)
            sheet.paste(t, (cx + 24 + j * 70, cy + 566 - sz), t)
    out = "/tmp/meridian-variants.png"
    sheet.save(out)
    print("wrote", out)


if __name__ == "__main__":
    main()
