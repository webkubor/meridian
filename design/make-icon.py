#!/usr/bin/env python3
"""Generate Meridian's app icon as SVG, then rasterise the macOS icon set.

The mark is drawn as exact geometry rather than scaled from the generated
artwork: a raster mark cannot be re-scaled to 16px without the hairline
disappearing, and the generated bitmap had visible drift (the spine wandered
between x=619 and x=623, diamonds differed in size). This file is the source
of truth for the icon; the artwork in design/brand/ is the art direction.

Geometry is laid out in an 824x824 "art box" inset 100px into the 1024 canvas,
which is Apple's proportion for macOS app icons. The body is a superellipse
(n=5): its 45-degree corner lands at 770.7/824, within half a pixel of the
0.2237 x size corner radius Apple's own icon mask uses.
"""

import math
import pathlib
import subprocess

HERE = pathlib.Path(__file__).resolve().parent
OUT = HERE / "final"
ICONS = HERE.parent / "src-tauri" / "icons"

CANVAS = 1024
INSET = 100
BOX = CANVAS - 2 * INSET  # 824

# --- brand palette (sampled from the approved brand board) -------------------
NAVY_DEEP = "#070F1A"
NAVY_LIFT = "#1B2430"
AZURE = "#3B82F6"
CYAN = "#60E1FF"
AMBER = "#FFB020"
MIST = "#E5E9F0"

# --- mark geometry, in art-box coordinates ----------------------------------
# The meridian is the spine; the sun sits on it at noon, and one branch forks
# off and ends at a node of its own. The branch deliberately does NOT rejoin:
# three earlier passes had it loop back to the spine, and a closed loop hanging
# off a vertical stem reads as a letterform every time (first a capital "B",
# then a key), however the curve is tuned. A branch that simply ends is also
# the more honest picture of a branch.
SPINE_X = 400.0
SUN_CY = 139.0
SUN_R = 39.0
SPINE_TOP = 174.0
SPINE_BOTTOM = 724.0
FORK_Y = 299.0  # node where the branch leaves the meridian
BRANCH_END = (557.0, 460.0)  # the branch's own node, where it stops
NODE_B = 480.0  # meridian node below the fork
NODE_C = 639.0  # lowest meridian node
STROKE = 18.0
DIA_HALF = 26.0


def squircle(cx: float, cy: float, r: float, n: int = 5, steps: int = 720) -> str:
    """Superellipse |x/r|^n + |y/r|^n = 1 as a closed path."""
    pts = []
    for i in range(steps):
        t = 2 * math.pi * i / steps
        ct, st = math.cos(t), math.sin(t)
        x = r * math.copysign(abs(ct) ** (2 / n), ct)
        y = r * math.copysign(abs(st) ** (2 / n), st)
        pts.append(f"{cx + x:.2f},{cy + y:.2f}")
    return "M" + " L".join(pts) + " Z"


def diamond(cx: float, cy: float, h: float) -> str:
    return (
        f"M{cx:.2f},{cy - h:.2f} L{cx + h:.2f},{cy:.2f} "
        f"L{cx:.2f},{cy + h:.2f} L{cx - h:.2f},{cy:.2f} Z"
    )


def build(small: bool) -> str:
    """Full mark, or the simplified variant for 16-64px.

    At 32px the full mark's hairline is well under a pixel, so the small
    variant drops the middle node and widens everything.
    """
    stroke = 44.0 if small else STROKE
    dh = 0.0 if small else DIA_HALF
    sun_r = 76.0 if small else SUN_R
    sun_cy = 176.0 if small else SUN_CY
    top = sun_cy + sun_r - 18
    bottom = 690.0
    fork_y = 348.0
    bx, by = (536.0, 494.0) if small else BRANCH_END
    node_b = 512.0
    node_c = 626.0

    line = (
        f'<g fill="none" stroke="url(#thread)" stroke-width="{stroke}" '
        f'stroke-linecap="round">'
        f'<path d="M{SPINE_X:.2f},{top:.2f} L{SPINE_X:.2f},{bottom:.2f}"/>'
        f'<path d="M{SPINE_X:.2f},{fork_y:.2f} '
        f'C{SPINE_X + 96:.2f},{fork_y + 4:.2f} {bx:.2f},{fork_y + 62:.2f} '
        f'{bx:.2f},{by:.2f}"/>'
        "</g>"
    )

    # At 16px a diamond is smaller than a pixel, so the small variant drops them
    # and lets the line's own round caps carry the node rhythm.
    nodes = [] if small else [
        (SPINE_X, fork_y, dh),
        (bx, by, dh),
        (SPINE_X, node_b, dh),
        (SPINE_X, node_c, dh),
    ]
    diamonds = "".join(
        f'<path d="{diamond(x, y, h)}" fill="url(#thread)" '
        f'stroke="url(#thread)" stroke-width="{stroke * 0.42:.2f}" '
        f'stroke-linejoin="round"/>'
        for x, y, h in nodes
    )

    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="{CANVAS}" height="{CANVAS}"
     viewBox="0 0 {CANVAS} {CANVAS}">
  <defs>
    <radialGradient id="body" cx="50%" cy="34%" r="82%">
      <stop offset="0%" stop-color="{NAVY_LIFT}"/>
      <stop offset="58%" stop-color="#101A29"/>
      <stop offset="100%" stop-color="{NAVY_DEEP}"/>
    </radialGradient>
    <!-- userSpaceOnUse, not the default objectBoundingBox: the spine is a pure
         vertical line, whose bounding box has zero width, and objectBoundingBox
         units are undefined there, so the line silently does not render. -->
    <linearGradient id="thread" gradientUnits="userSpaceOnUse"
                    x1="0" y1="150" x2="0" y2="700">
      <stop offset="0%" stop-color="#FFFFFF"/>
      <stop offset="62%" stop-color="{MIST}"/>
      <stop offset="100%" stop-color="#B9C6DA"/>
    </linearGradient>
    <linearGradient id="rim" x1="0" y1="0" x2="0.35" y2="1">
      <stop offset="0%" stop-color="{CYAN}" stop-opacity="0.30"/>
      <stop offset="46%" stop-color="{AZURE}" stop-opacity="0.10"/>
      <stop offset="100%" stop-color="{CYAN}" stop-opacity="0.34"/>
    </linearGradient>
    <radialGradient id="halo" cx="50%" cy="50%" r="50%">
      <stop offset="0%" stop-color="{AMBER}" stop-opacity="0.55"/>
      <stop offset="55%" stop-color="{AMBER}" stop-opacity="0.16"/>
      <stop offset="100%" stop-color="{AMBER}" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="sun" cx="42%" cy="36%" r="72%">
      <stop offset="0%" stop-color="#FFE7A8"/>
      <stop offset="46%" stop-color="{AMBER}"/>
      <stop offset="100%" stop-color="#E8871A"/>
    </radialGradient>
  </defs>

  <g transform="translate({INSET},{INSET})">
    <path d="{squircle(BOX / 2, BOX / 2, BOX / 2)}" fill="url(#body)"/>
    <path d="{squircle(BOX / 2, BOX / 2, BOX / 2 - 1.5)}" fill="none"
          stroke="url(#rim)" stroke-width="3"/>
    <circle cx="{SPINE_X:.2f}" cy="{sun_cy:.2f}" r="{sun_r * 3.4:.2f}" fill="url(#halo)"/>
    <circle cx="{SPINE_X:.2f}" cy="{sun_cy:.2f}" r="{sun_r * 1.9:.2f}" fill="url(#halo)"/>
    {line}
    {diamonds}
    <circle cx="{SPINE_X:.2f}" cy="{sun_cy:.2f}" r="{sun_r:.2f}" fill="url(#sun)"/>
  </g>
</svg>
"""


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    full = build(small=False)
    small = build(small=True)
    (OUT / "meridian-icon.svg").write_text(full)
    (OUT / "meridian-icon-small.svg").write_text(small)

    # macOS icon set. 512px and below get the simplified mark: below that the
    # full mark's hairline stops resolving.
    sizes = [16, 32, 64, 128, 256, 512, 1024]
    iconset = OUT / "Meridian.iconset"
    iconset.mkdir(exist_ok=True)
    for size in sizes:
        src = small if size <= 64 else full
        srcfile = OUT / ("meridian-icon-small.svg" if size <= 64 else "meridian-icon.svg")
        for scale, suffix in ((1, ""), (2, "@2x")):
            px = size * scale
            if px > 1024:
                continue
            name = f"icon_{size}x{size}{suffix}.png"
            subprocess.run(
                ["rsvg-convert", "-w", str(px), "-h", str(px), "-o", str(iconset / name), str(srcfile)],
                check=True,
            )
        if size == 1024:
            subprocess.run(
                ["rsvg-convert", "-w", "1024", "-h", "1024", "-o", str(OUT / "icon-1024.png"), str(srcfile)],
                check=True,
            )
    subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", str(OUT / "Meridian.icns")], check=True)
    print("wrote", OUT / "Meridian.icns")


if __name__ == "__main__":
    main()
