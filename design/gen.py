import math, subprocess, os

CANVAS = 1024
INSET  = 100          # macOS icon grid: 824pt body in a 1024 canvas
BODY   = CANVAS - 2*INSET
R_BODY = 185.4        # Apple's corner radius for the 824 body
CX = CY = CANVAS/2

def squircle_path(inset=INSET, r=R_BODY):
    x0, y0 = inset, inset
    x1, y1 = CANVAS-inset, CANVAS-inset
    return (f"M {x0+r} {y0} H {x1-r} A {r} {r} 0 0 1 {x1} {y0+r} "
            f"V {y1-r} A {r} {r} 0 0 1 {x1-r} {y1} H {x0+r} "
            f"A {r} {r} 0 0 1 {x0} {y1-r} V {y0+r} A {r} {r} 0 0 1 {x0+r} {y0} Z")

def pol(cx, cy, r, deg):
    a = math.radians(deg)
    return (cx + r*math.cos(a), cy + r*math.sin(a))

def arc(cx, cy, r, a0, a1, sweep=1):
    x0, y0 = pol(cx, cy, r, a0)
    x1, y1 = pol(cx, cy, r, a1)
    large = 1 if abs(a1-a0) > 180 else 0
    return f"M {x0:.1f} {y0:.1f} A {r} {r} 0 {large} {sweep} {x1:.1f} {y1:.1f}"

def defs(grad_id, c0, c1, vertical=True):
    coords = 'x1="0" y1="0" x2="0" y2="1"' if vertical else 'x1="0" y1="0" x2="1" y2="1"'
    return (f'<defs><linearGradient id="{grad_id}" {coords}>'
            f'<stop offset="0" stop-color="{c0}"/><stop offset="1" stop-color="{c1}"/>'
            f'</linearGradient></defs>')

def bg(ground):
    return f'<path d="{squircle_path()}" fill="{ground}"/>'

# ---------------------------------------------------------------- concepts

def c1_meridian_graph(limb0, limb1, node, accent, ground, R=214, sw=62, gap=26):
    """Globe limb with gaps at the poles; the prime meridian is a commit graph
    that branches mid-way and merges again near the south pole."""
    top = pol(CX, CY, R, 270)
    bot = pol(CX, CY, R, 90)
    s = [defs('g', limb0, limb1)]
    s.append(bg(ground))
    # limb: two arcs, gaps centred on the poles so the meridian can exit
    s.append(f'<path d="{arc(CX,CY,R, 270+gap, 90-gap, 1)}" stroke="url(#g)" stroke-width="{sw}" fill="none" stroke-linecap="round"/>')
    s.append(f'<path d="{arc(CX,CY,R, 270-gap, 90+gap, 1)}" stroke="url(#g)" stroke-width="{sw}" fill="none" stroke-linecap="round"/>')
    line_w = 40
    y_top, y_bot = top[1], bot[1]
    split, join = CY-24, CY+126
    # straight spine, top -> split
    s.append(f'<path d="M {CX} {y_top} V {split}" stroke="url(#g)" stroke-width="{line_w}" fill="none" stroke-linecap="round"/>')
    # branch: bows right, then returns
    s.append(f'<path d="M {CX} {split} C {CX+96} {split+18} {CX+96} {join-18} {CX} {join}" '
             f'stroke="url(#g)" stroke-width="{line_w}" fill="none" stroke-linecap="round"/>')
    # spine continues to the south pole
    s.append(f'<path d="M {CX} {split} V {y_bot}" stroke="url(#g)" stroke-width="{line_w}" fill="none" stroke-linecap="round"/>')
    r_node, r_acc = 27, 32
    s.append(f'<circle cx="{CX}" cy="{y_top+r_node*0.6:.0f}" r="{r_node}" fill="{node}"/>')
    s.append(f'<circle cx="{CX}" cy="{split}" r="{r_acc}" fill="{accent}"/>')
    s.append(f'<circle cx="{CX+88}" cy="{(split+join)/2:.0f}" r="{r_node}" fill="{node}"/>')
    s.append(f'<circle cx="{CX}" cy="{y_bot-r_node*0.6:.0f}" r="{r_node}" fill="{node}"/>')
    return ''.join(s)

def c2_twin_arcs(limb0, limb1, node, accent, ground, R=206, sw=74):
    """No closed ring: two limb arcs, the graph threading between them."""
    s = [defs('g', limb0, limb1)]
    s.append(bg(ground))
    s.append(f'<path d="{arc(CX,CY,R, 118, 242, 1)}" stroke="url(#g)" stroke-width="{sw}" fill="none" stroke-linecap="round"/>')
    s.append(f'<path d="{arc(CX,CY,R, 298, 62, 1)}" stroke="url(#g)" stroke-width="{sw}" fill="none" stroke-linecap="round"/>')
    lw = 44
    y_top, y_bot = CY-R+86, CY+R-86
    split, join = CY-30, CY+118
    s.append(f'<path d="M {CX} {y_top} V {split}" stroke="url(#g)" stroke-width="{lw}" fill="none" stroke-linecap="round"/>')
    s.append(f'<path d="M {CX} {split} C {CX+108} {split+16} {CX+108} {join-16} {CX} {join}" stroke="url(#g)" stroke-width="{lw}" fill="none" stroke-linecap="round"/>')
    s.append(f'<path d="M {CX} {split} V {y_bot}" stroke="url(#g)" stroke-width="{lw}" fill="none" stroke-linecap="round"/>')
    r_node, r_acc = 30, 34
    s.append(f'<circle cx="{CX}" cy="{y_top}" r="{r_node}" fill="{node}"/>')
    s.append(f'<circle cx="{CX}" cy="{split}" r="{r_acc}" fill="{accent}"/>')
    s.append(f'<circle cx="{CX+100}" cy="{(split+join)/2:.0f}" r="{r_node}" fill="{node}"/>')
    s.append(f'<circle cx="{CX}" cy="{y_bot}" r="{r_node}" fill="{node}"/>')
    return ''.join(s)

def c3_disc_cut(limb0, limb1, node, accent, ground, R=250):
    """Solid globe; the meridian graph is cut out of it in the ground colour."""
    s = [defs('g', limb0, limb1)]
    s.append(bg(ground))
    s.append(f'<circle cx="{CX}" cy="{CY}" r="{R}" fill="url(#g)"/>')
    cut = ground
    lw = 46
    y_top, y_bot = CY-R+78, CY+R-78
    split, join = CY-30, CY+112
    s.append(f'<path d="M {CX} {y_top} V {split}" stroke="{cut}" stroke-width="{lw}" fill="none" stroke-linecap="round"/>')
    s.append(f'<path d="M {CX} {split} C {CX+104} {split+16} {CX+104} {join-16} {CX} {join}" stroke="{cut}" stroke-width="{lw}" fill="none" stroke-linecap="round"/>')
    s.append(f'<path d="M {CX} {split} V {y_bot}" stroke="{cut}" stroke-width="{lw}" fill="none" stroke-linecap="round"/>')
    r_node, r_acc = 30, 36
    s.append(f'<circle cx="{CX}" cy="{y_top}" r="{r_node}" fill="{cut}"/>')
    s.append(f'<circle cx="{CX}" cy="{split}" r="{r_acc}" fill="{cut}" stroke="{accent}" stroke-width="13"/>')
    s.append(f'<circle cx="{CX+96}" cy="{(split+join)/2:.0f}" r="{r_node}" fill="{cut}"/>')
    s.append(f'<circle cx="{CX}" cy="{y_bot}" r="{r_node}" fill="{cut}"/>')
    return ''.join(s)

def svg(inner):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{CANVAS}" height="{CANVAS}" '
            f'viewBox="0 0 {CANVAS} {CANVAS}">{inner}</svg>')

PAL = {
  'celestial': dict(ground='#0A0E1F', limb0='#2F6BFF', limb1='#6FD8FF', node='#EAF2FF', accent='#FFC46B'),
  'instrument': dict(ground='#08201E', limb0='#B98B2E', limb1='#E9CE7A', node='#F6F1E6', accent='#6FE3D2'),
  'graphite':  dict(ground='#101317', limb0='#8FA3BF', limb1='#DCE7F5', node='#FFFFFF', accent='#5AA9FF'),
}

out = {
  'c1-celestial':  c1_meridian_graph(**PAL['celestial']),
  'c1-instrument': c1_meridian_graph(**PAL['instrument']),
  'c1-graphite':   c1_meridian_graph(**PAL['graphite']),
  'c2-celestial':  c2_twin_arcs(**PAL['celestial']),
  'c3-celestial':  c3_disc_cut(**PAL['celestial']),
  'c3-instrument': c3_disc_cut(**PAL['instrument']),
}

for name, inner in out.items():
    open(f'{name}.svg','w').write(svg(inner))
    subprocess.run(['rsvg-convert','-w','512','-h','512',f'{name}.svg','-o',f'{name}.png'], check=True)
    subprocess.run(['rsvg-convert','-w','32','-h','32',f'{name}.svg','-o',f'{name}-32.png'], check=True)
print("rendered", len(out))
