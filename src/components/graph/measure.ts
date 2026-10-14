/**
 * A single offscreen 2D context reused for text measurement (ref badge sizing,
 * ellipsis, column truncation). One context for the whole app: `measureText`
 * is pure, and creating a canvas per row would be wasteful in a list that
 * recycles thousands of rows.
 */

let context: CanvasRenderingContext2D | null = null;

export function measureContext(): CanvasRenderingContext2D {
  if (!context) {
    const canvas = document.createElement("canvas");
    const ctx = canvas.getContext("2d");
    if (!ctx) {
      throw new Error("meridian: 2D canvas context unavailable");
    }
    context = ctx;
  }
  return context;
}

/** Width of `text` in the given CSS font, in CSS pixels. */
export function textWidth(text: string, font: string): number {
  const ctx = measureContext();
  ctx.font = font;
  return ctx.measureText(text).width;
}
