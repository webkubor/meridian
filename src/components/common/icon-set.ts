/**
 * Icon system (docs/04 §3).
 *
 * Names are the SF-Symbols-style vocabulary from the visual-design evidence
 * (`symbol_*` inventory + the role registry in docs/04 §3). The artwork is
 * OURS: every glyph below is a hand-authored 16×16 stroke drawing — no asset
 * bytes from any other product are used. Elements are stroked with
 * `currentColor` unless `filled` is set, so a glyph inherits text colour and
 * never carries its own palette.
 */

export interface IconSegment {
  d: string;
  filled?: boolean;
}

/** Drawn when a name has no glyph (docs/04 §3: every role has a fallback). */
export const FALLBACK_ICON_NAME = "questionmark.circle";

const dot = (cx: number, cy: number, r: number): IconSegment => ({
  d: `M${cx + r} ${cy}a${r} ${r} 0 1 1 ${-2 * r} 0 ${r} ${r} 0 0 1 ${2 * r} 0z`,
  filled: true,
});

export const ICON_GLYPHS: Record<string, IconSegment[]> = {
  /* ---- Navigation / object types ---------------------------------------- */
  "square.grid.2x2": [
    { d: "M3 3.5h4v4H3z" },
    { d: "M9 3.5h4v4H9z" },
    { d: "M3 9.5h4v4H3z" },
    { d: "M9 9.5h4v4H9z" },
  ],
  clock: [{ d: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11z" }, { d: "M8 5.2v3.1l2 1.4" }],
  "clock.arrow.circlepath": [
    { d: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11z" },
    { d: "M8 5.2v3.1l2 1.4" },
    { d: "M11.4 3.2l1.6 1.6" },
    { d: "M13 2v2.8h-2.8" },
  ],
  "arrow.triangle.branch": [
    { d: "M5.7 4.2a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0z" },
    { d: "M5.7 12a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0z" },
    { d: "M12.7 4.2a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0z" },
    { d: "M4.2 5.7v4.8" },
    { d: "M4.2 9.6c2.6-.3 5-1.2 5-3.9" },
  ],
  "arrow.triangle.merge": [
    { d: "M5.7 4.2a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0z" },
    { d: "M5.7 12a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0z" },
    { d: "M12.7 4.2a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0z" },
    { d: "M4.2 5.7v4.8" },
    { d: "M11.2 5.7v1c0 2.3-1.9 3.3-4.2 3.6" },
  ],
  "arrow.triangle.pull": [
    { d: "M5.7 12a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0z" },
    { d: "M5.7 4.2a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0z" },
    { d: "M12.7 4.2a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0z" },
    { d: "M4.2 5.7v4.8" },
    { d: "M11.2 5.7v1.2c0 1.9-1.5 3.2-3.4 3.4" },
    { d: "M9.2 8.9l-1.6 1.4 1.4 1.6" },
  ],
  "arrow.triangle.2.circlepath": [
    { d: "M3.4 6.6A5 5 0 0 1 12 5.2" },
    { d: "M12.3 2.6L12 5.2l-2.5-.3" },
    { d: "M12.6 9.4A5 5 0 0 1 4 10.8" },
    { d: "M3.7 13.4L4 10.8l2.5.3" },
  ],
  "arrow.clockwise": [
    { d: "M12.9 8a4.9 4.9 0 1 1-1.4-3.4" },
    { d: "M13.2 2.8v2.9h-2.9" },
  ],
  "arrow.left.arrow.right": [
    { d: "M3 5.6h9.4" },
    { d: "M9.9 3.4l2.2 2.2-2.2 2.2" },
    { d: "M13 10.4H3.6" },
    { d: "M6.1 8.2 3.9 10.4l2.2 2.2" },
  ],
  "arrow.up.arrow.down": [
    { d: "M5.4 3.6v9" },
    { d: "M3.2 10.4l2.2 2.2 2.2-2.2" },
    { d: "M10.6 12.4v-9" },
    { d: "M8.4 5.6l2.2-2.2 2.2 2.2" },
  ],
  "arrow.uturn.backward": [
    { d: "M6.6 4 3.6 7l3 3" },
    { d: "M3.6 7h6.6a2.9 2.9 0 0 1 0 5.8H8.6" },
  ],
  "arrow.uturn.forward": [
    { d: "M9.4 4l3 3-3 3" },
    { d: "M12.4 7H5.8a2.9 2.9 0 0 0 0 5.8h1.6" },
  ],
  "square.on.square": [{ d: "M5.6 2.6h7.8v7.8" }, { d: "M2.6 5.6h7.8v7.8h-7.8z" }],
  "square.stack.3d.up": [
    { d: "M8 2.4l4.6 2.4L8 7.2 3.4 4.8z" },
    { d: "M3.4 7.6 8 10l4.6-2.4" },
    { d: "M3.4 10.6 8 13l4.6-2.4" },
  ],
  cloud: [{ d: "M4.9 12.6a2.9 2.9 0 0 1-.3-5.8 4 4 0 0 1 7.7-.6 2.9 2.9 0 0 1 .2 6.4z" }],
  gearshape: [
    { d: "M8 2.2v2.2" },
    { d: "M8 11.6v2.2" },
    { d: "M2.2 8h2.2" },
    { d: "M11.6 8h2.2" },
    { d: "M3.9 3.9l1.6 1.6" },
    { d: "M10.5 10.5l1.6 1.6" },
    { d: "M12.1 3.9l-1.6 1.6" },
    { d: "M5.5 10.5l-1.6 1.6" },
    { d: "M8 5.6a2.4 2.4 0 1 0 0 4.8 2.4 2.4 0 0 0 0-4.8z" },
  ],
  magnifyingglass: [
    { d: "M7 2.6a4.4 4.4 0 1 0 0 8.8 4.4 4.4 0 0 0 0-8.8z" },
    { d: "M10.3 10.3 13.6 13.6" },
  ],
  "sidebar.left": [{ d: "M2.6 3.4h10.8v9.2H2.6z" }, { d: "M6.6 3.4v9.2" }],

  /* ---- Objects ----------------------------------------------------------- */
  folder: [{ d: "M2.4 4.2h4l1.3 1.8h5.9v7.8H2.4z" }],
  "folder.badge.plus": [
    { d: "M2.4 4.2h4l1.3 1.8h6v4.4" },
    { d: "M2.4 4.2v9.6h6.6" },
    { d: "M12.4 11.8v2.6" },
    { d: "M11.1 13.1h2.6" },
  ],
  doc: [{ d: "M4.4 2.4h4.8l2.8 2.8v8.4H4.4z" }, { d: "M9.2 2.4v2.8h2.8" }],
  "doc.text": [
    { d: "M4.4 2.4h4.8l2.8 2.8v8.4H4.4z" },
    { d: "M9.2 2.4v2.8h2.8" },
    { d: "M6.4 7.4h4" },
    { d: "M6.4 9.6h4" },
    { d: "M6.4 11.8h2.4" },
  ],
  "doc.on.doc": [
    { d: "M5.6 4.6V2.4H11l2.8 2.8v6.2h-2.2" },
    { d: "M2.6 5.6h5.6l2.6 2.6v5.4H2.6z" },
    { d: "M8.2 5.6v2.6h2.6" },
  ],
  "arrow.down.doc": [
    { d: "M8.4 2.6h3.4l2.4 2.4v7.2H8.4z" },
    { d: "M11.8 2.6v2.4h2.4" },
    { d: "M3.4 5.4v5.2" },
    { d: "M1.4 8.6 3.4 10.6l2-2" },
  ],
  shippingbox: [
    { d: "M8 2.4l5.6 2.8v6.4L8 14.4l-5.6-2.8V5.2z" },
    { d: "M2.4 5.2 8 8l5.6-2.8" },
    { d: "M8 8v6.4" },
  ],
  tag: [{ d: "M2.6 8.4V2.8h5.6l5.2 5.2-5.6 5.6z" }, { d: "M5.2 5.4h.01" }],
  bookmark: [{ d: "M4.6 2.6h6.8v10.8L8 10.6l-3.4 2.8z" }],
  terminal: [
    { d: "M2.6 3.4h10.8v9.2H2.6z" },
    { d: "M5 6.6 7.4 9 5 11.4" },
    { d: "M8.8 11.4h2.6" },
  ],
  "arrow.up.forward.square": [
    { d: "M2.8 2.8h10.4v10.4H2.8z" },
    { d: "M6.2 9.8 9.8 6.2" },
    { d: "M7 6.2h2.8v2.8" },
  ],
  pencil: [
    { d: "M3.2 12.8l.6-2.6 6.4-6.4 2.2 2.2-6.4 6.4z" },
    { d: "M9.4 4.6l2.2 2.2" },
  ],
  trash: [
    { d: "M3.2 4.6h9.6" },
    { d: "M6.4 4.6V3h3.2v1.6" },
    { d: "M4.6 4.6l.7 9.4h5.4l.7-9.4" },
    { d: "M6.8 7.2v4.4" },
    { d: "M9.2 7.2v4.4" },
  ],
  photo: [
    { d: "M2.6 3.6h10.8v8.8H2.6z" },
    { d: "M5.8 6.9a1.1 1.1 0 1 0 0-2.2 1.1 1.1 0 0 0 0 2.2z" },
    { d: "M3.4 11.4 6.8 8l2 2 2.2-2.2 2.4 2.4" },
  ],
  camera: [
    { d: "M2.6 5.2h2.6l1.1-1.8h3.4l1.1 1.8h2.6v8H2.6z" },
    { d: "M8 6.6a2.3 2.3 0 1 0 0 4.6 2.3 2.3 0 0 0 0-4.6z" },
  ],
  key: [
    { d: "M5.4 5.8a2.4 2.4 0 1 0 0 4.8 2.4 2.4 0 0 0 0-4.8z" },
    { d: "M7.8 8.2h6.2" },
    { d: "M11.6 8.2v2" },
    { d: "M9.6 8.2v1.6" },
  ],
  "person.crop.circle": [
    { d: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11z" },
    { d: "M8 5.1a1.8 1.8 0 1 0 0 3.6 1.8 1.8 0 0 0 0-3.6z" },
    { d: "M4.4 12.6a3.9 3.9 0 0 1 7.2 0" },
  ],
  signature: [
    { d: "M3 11.6c2.2 0 2.6-5.4 4.4-5.4 1.4 0 1.2 3.8 2.6 3.8.8 0 1.2-.6 1.4-1.2" },
    { d: "M3 14h10" },
  ],

  /* ---- Actions ----------------------------------------------------------- */
  "checkmark.circle": [
    { d: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11z" },
    { d: "M5.4 8.2 7.2 10l3.5-3.8" },
  ],
  "checkmark.circle.fill": [
    { d: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11z" },
    { d: "M5.4 8.2 7.2 10l3.5-3.8" },
  ],
  checkmark: [{ d: "M3.6 8.4 6.6 11.4 12.4 4.6" }],
  "checkmark.square": [{ d: "M3 3h10v10H3z" }, { d: "M5.4 8.2 7.2 10l3.4-3.6" }],
  square: [{ d: "M3 3h10v10H3z" }],
  plus: [{ d: "M8 3.4v9.2" }, { d: "M3.4 8h9.2" }],
  "plus.circle": [
    { d: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11z" },
    { d: "M8 5.4v5.2" },
    { d: "M5.4 8h5.2" },
  ],
  minus: [{ d: "M3.4 8h9.2" }],
  "minus.circle": [
    { d: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11z" },
    { d: "M5.4 8h5.2" },
  ],
  xmark: [{ d: "M4.4 4.4 11.6 11.6" }, { d: "M11.6 4.4 4.4 11.6" }],
  "xmark.circle.fill": [
    { d: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11z" },
    { d: "M5.9 5.9l4.2 4.2" },
    { d: "M10.1 5.9l-4.2 4.2" },
  ],
  ellipsis: [dot(4, 8, 0.9), dot(8, 8, 0.9), dot(12, 8, 0.9)],
  "ellipsis.circle": [
    { d: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11z" },
    dot(5.6, 8, 0.75),
    dot(8, 8, 0.75),
    dot(10.4, 8, 0.75),
  ],
  "chevron.right": [{ d: "M6.2 4.4 9.8 8l-3.6 3.6" }],
  "chevron.left": [{ d: "M9.8 4.4 6.2 8l3.6 3.6" }],
  "chevron.down": [{ d: "M4.4 6.2 8 9.8l3.6-3.6" }],
  "chevron.up": [{ d: "M4.4 9.8 8 6.2l3.6 3.6" }],
  "chevron.up.chevron.down": [
    { d: "M4.6 6.4 8 3l3.4 3.4" },
    { d: "M4.6 9.6 8 13l3.4-3.4" },
  ],
  "arrow.up": [{ d: "M8 13V3.6" }, { d: "M4.6 7 8 3.6 11.4 7" }],
  "arrow.down": [{ d: "M8 3.6V13" }, { d: "M4.6 9.6 8 13l3.4-3.4" }],
  "arrow.right": [{ d: "M3.4 8h9.2" }, { d: "M9.4 5.2 12.6 8l-3.2 2.8" }],
  "arrow.down.circle": [
    { d: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11z" },
    { d: "M8 5v5.4" },
    { d: "M5.7 8.1 8 10.4l2.3-2.3" },
  ],
  "arrow.up.circle": [
    { d: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11z" },
    { d: "M8 10.4V5" },
    { d: "M5.7 7.3 8 5l2.3 2.3" },
  ],
  "arrow.up.circle.badge.exclamationmark": [
    { d: "M12.6 7.6a5 5 0 1 0-10 0 5 5 0 0 0 10 0z" },
    { d: "M7.6 10.1V5.5" },
    { d: "M5.5 7.6 7.6 5.5l2.1 2.1" },
    { d: "M13.6 10.8v2.1" },
    { d: "M13.6 14.1h.01" },
  ],
  "tray.and.arrow.down": [
    { d: "M2.8 10.2v2.9h10.4v-2.9" },
    { d: "M8 2.6v6" },
    { d: "M5.4 6.2 8 8.8l2.6-2.6" },
  ],
  bolt: [{ d: "M9 2.4 4.6 8.8h2.9L7 13.6 11.4 7H8.5z" }],
  "wand.and.stars": [
    { d: "M2.8 13.2 9.2 6.8" },
    { d: "M7.9 5.5l2.6 2.6" },
    { d: "M12.2 2.4l.5 1.4 1.4.5-1.4.5-.5 1.4-.5-1.4-1.4-.5 1.4-.5z" },
    { d: "M3.4 2.8l.5 1.3 1.3.5-1.3.5-.5 1.3-.5-1.3-1.3-.5 1.3-.5z" },
  ],
  sparkles: [
    { d: "M8 2.4l.9 2.6 2.6.9-2.6.9-.9 2.6-.9-2.6-2.6-.9 2.6-.9z" },
    { d: "M12.4 9.6l.5 1.4 1.4.5-1.4.5-.5 1.4-.5-1.4-1.4-.5 1.4-.5z" },
  ],
  hourglass: [
    { d: "M4.4 2.6h7.2" },
    { d: "M4.4 13.4h7.2" },
    { d: "M5.4 2.6c0 2.6 2.6 3.4 2.6 5.4s-2.6 2.8-2.6 5.4" },
    { d: "M10.6 2.6c0 2.6-2.6 3.4-2.6 5.4s2.6 2.8 2.6 5.4" },
  ],

  /* ---- Status / feedback -------------------------------------------------- */
  "exclamationmark.triangle": [
    { d: "M8 2.7l6 10.4H2z" },
    { d: "M8 6.6v3" },
    { d: "M8 11.8h.01" },
  ],
  "exclamationmark.circle": [
    { d: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11z" },
    { d: "M8 4.9v3.6" },
    { d: "M8 11.2h.01" },
  ],
  "info.circle": [
    { d: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11z" },
    { d: "M8 5h.01" },
    { d: "M8 7.4v3.8" },
  ],
  "questionmark.circle": [
    { d: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11z" },
    { d: "M6.4 6.4a1.7 1.7 0 1 1 2.4 1.6c-.5.3-.8.6-.8 1.1v.4" },
    { d: "M8 11.6h.01" },
  ],
  eye: [
    { d: "M2.4 8S4.6 4.6 8 4.6 13.6 8 13.6 8 11.4 11.4 8 11.4 2.4 8 2.4 8z" },
    { d: "M8 6.4a1.6 1.6 0 1 0 0 3.2 1.6 1.6 0 0 0 0-3.2z" },
  ],
  "eye.slash": [
    { d: "M2.4 8S4.6 4.6 8 4.6 13.6 8 13.6 8 11.4 11.4 8 11.4 2.4 8 2.4 8z" },
    { d: "M8 6.4a1.6 1.6 0 1 0 0 3.2 1.6 1.6 0 0 0 0-3.2z" },
    { d: "M3 3l10 10" },
  ],
  "sun.max": [
    { d: "M8 5.2a2.8 2.8 0 1 0 0 5.6 2.8 2.8 0 0 0 0-5.6z" },
    { d: "M8 1.8v1.6" },
    { d: "M8 12.6v1.6" },
    { d: "M1.8 8h1.6" },
    { d: "M12.6 8h1.6" },
    { d: "M3.6 3.6l1.2 1.2" },
    { d: "M11.2 11.2l1.2 1.2" },
    { d: "M12.4 3.6l-1.2 1.2" },
    { d: "M4.8 11.2l-1.2 1.2" },
  ],
  moon: [{ d: "M12.9 9.4A5.2 5.2 0 1 1 6.6 3.1a4.1 4.1 0 0 0 6.3 6.3z" }],
  "moon.zzz": [
    { d: "M11.4 8.2A4.3 4.3 0 1 1 6.1 3.1a3.4 3.4 0 0 0 5.3 5.1z" },
    { d: "M11.2 10.6h2.6l-2.6 2.8h2.6" },
  ],
  "circle.lefthalf.filled": [
    { d: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11z" },
    { d: "M8 2.5a5.5 5.5 0 0 1 0 11z", filled: true },
  ],

  /* ---- List chrome -------------------------------------------------------- */
  "list.bullet": [
    { d: "M6 4h7.4" },
    { d: "M6 8h7.4" },
    { d: "M6 12h7.4" },
    dot(3.4, 4, 0.8),
    dot(3.4, 8, 0.8),
    dot(3.4, 12, 0.8),
  ],
  "text.alignleft": [
    { d: "M2.6 4h10.8" },
    { d: "M2.6 7h7.2" },
    { d: "M2.6 10h10.8" },
    { d: "M2.6 13h5.6" },
  ],
  "line.3.horizontal.decrease": [
    { d: "M2.6 4.6h10.8" },
    { d: "M4.4 8h7.2" },
    { d: "M6.2 11.4h3.6" },
  ],
  "textformat.size": [
    { d: "M2.4 5.2V3.6h6v1.6" },
    { d: "M5.4 3.6v8.8" },
    { d: "M4.2 12.4h2.4" },
    { d: "M11 9.2V8h4v1.2" },
    { d: "M13 8v4.4" },
    { d: "M12.2 12.4h1.6" },
  ],
};

/* -------------------------------------------------------------------------- */

/**
 * Role registry (docs/04 §3). Components may address an icon by role
 * (`nav.history`) or directly by symbol name (`clock`); roles resolve first.
 */
export const ICON_ROLES: Record<string, string> = {
  "nav.repositories": "square.grid.2x2",
  "nav.history": "clock",
  "nav.branch": "arrow.triangle.branch",
  "nav.workingCopy": "doc.text",
  "nav.stashes": "shippingbox",
  "nav.tags": "tag",
  "nav.submodule": "square.on.square",
  "nav.worktree": "square.stack.3d.up",
  "nav.reflog": "clock.arrow.circlepath",
  "nav.services": "cloud",
  "nav.settings": "gearshape",
  "nav.pullRequests": "arrow.triangle.pull",
  "nav.blame": "person.crop.circle",
  "nav.compare": "arrow.left.arrow.right",
  "badge.unpushed": "arrow.up",
  "badge.unpulled": "arrow.down",
  "badge.stale": "moon.zzz",
  "badge.fullyMerged": "checkmark.circle.fill",
  "badge.conflict": "exclamationmark.triangle",
  "action.commit": "checkmark.circle",
  "action.branch": "arrow.triangle.branch",
  "action.merge": "arrow.triangle.merge",
  "action.rebase": "arrow.triangle.2.circlepath",
  "action.cherryPick": "arrow.down.doc",
  "action.revert": "arrow.uturn.backward",
  "action.fetch": "tray.and.arrow.down",
  "action.pull": "arrow.down.circle",
  "action.push": "arrow.up.circle",
  "action.forcePush": "arrow.up.circle.badge.exclamationmark",
  "action.sync": "arrow.triangle.2.circlepath",
  "action.stash": "shippingbox",
  "action.tag": "tag",
  "action.worktree": "square.stack.3d.up",
  "action.submodule": "square.on.square",
  "action.sign": "signature",
  "action.snapshot": "camera",
  "action.aiMessage": "wand.and.stars",
  "action.quickOpen": "magnifyingglass",
  "action.quickActions": "bolt",
  "action.refresh": "arrow.clockwise",
  "action.discard": "arrow.uturn.backward",
  "action.open": "arrow.up.forward.square",
  "action.terminal": "terminal",
  "action.reveal": "folder",
  "action.copy": "doc.on.doc",
  "action.stage": "plus.circle",
  "action.unstage": "minus.circle",
  "action.fileHistory": "clock.arrow.circlepath",
  "empty.noRepositorySelected": "folder",
  "empty.noBranchSelected": "arrow.triangle.branch",
  "empty.noCommitSelected": "clock",
  "empty.noFileSelected": "doc",
  "empty.multiSelection": "square.on.square",
  "empty.noPullRequestSelected": "arrow.triangle.pull",
  "empty.noReflogEntry": "clock.arrow.circlepath",
  "empty.noStashSelected": "shippingbox",
  "empty.unchangedFile": "checkmark.circle",
  "empty.invalidSubmodule": "exclamationmark.triangle",
  "empty.dropFolder": "folder.badge.plus",
  "empty.fullyMergedBranch": "checkmark.circle.fill",
  "empty.services": "cloud",
  "empty.noActivity": "hourglass",
};

export function resolveIconName(name: string): string {
  return ICON_ROLES[name] ?? name;
}

export function iconGlyph(name: string): IconSegment[] {
  return ICON_GLYPHS[resolveIconName(name)] ?? ICON_GLYPHS[FALLBACK_ICON_NAME];
}
