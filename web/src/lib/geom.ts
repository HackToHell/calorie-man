// Geometry helpers for the Terrace charts (ported from the design's render code).
export const P = Math.PI;
export const q = (v: number) => Math.round(v * 10) / 10;

/** Point on an ellipse (or circle when ry is omitted). */
export function pol(cx: number, cy: number, r: number, a: number, ry?: number): [number, number] {
  return [q(cx + r * Math.cos(a)), q(cy + (ry ?? r) * Math.sin(a))];
}

/** Circular arc path from angle a0 to a1 (clockwise). */
export function arc(cx: number, cy: number, r: number, a0: number, a1: number): string {
  const [x0, y0] = pol(cx, cy, r, a0);
  const [x1, y1] = pol(cx, cy, r, a1);
  return `M${x0} ${y0}A${r} ${r} 0 ${a1 - a0 > P + 0.001 ? 1 : 0} 1 ${x1} ${y1}`;
}

/** Regular octagon points around (cx, cy). `rot` 0 = flat top/bottom is P/8. */
export function octagon(cx: number, cy: number, r: number, rot = -P / 2): string {
  return Array.from({ length: 8 }, (_, k) => pol(cx, cy, r, rot + (k * P) / 4).join(',')).join(' ');
}

export const clamp01 = (v: number) => (Number.isFinite(v) ? Math.max(0, Math.min(1, v)) : 0);
