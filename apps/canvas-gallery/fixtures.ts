// The Canvas 2D gallery's fixtures (LLP 1056 §4): plain canvas code, written
// as a web developer writes it. `app.ts` hands them to the recorder on every
// host; `direct.html` runs the same functions against Chrome's own
// CanvasRenderingContext2D, with no recorder and no glue — the API oracle.
// Each is `(ctx, frame, args) => boolean | void`: true asks for a frame.
// Stage 1's members only: shapes, paths, lines, colours, gradients,
// compositing, clipping and the state stack.

import type { Ctx2D, Frame } from './app.contract.d.ts';

const TAU = Math.PI * 2;

type Fixture = (ctx: Ctx2D, frame: Frame, args: number[]) => boolean | void;

export const fixtures: Record<string, Fixture> = {
  // Arc direction under the flip (LLP 1056 §4's first fixture): a clockwise
  // arc from 0 to π bulges down in canvas space, an anticlockwise one up.
  arc(ctx, f) {
    ctx.fillStyle = "#ef4444";
    ctx.beginPath(); ctx.arc(40, 50, 28, 0, Math.PI); ctx.fill();
    ctx.fillStyle = "#2563eb";
    ctx.beginPath(); ctx.arc(110, 50, 28, 0, Math.PI, true); ctx.fill();
    ctx.strokeStyle = "#0f172a"; ctx.lineWidth = 3;
    ctx.beginPath(); ctx.arc(75, 50, 40, -Math.PI / 2, 0); ctx.stroke();
    ctx.beginPath(); ctx.arc(75, 50, 12, 0, TAU, true); ctx.stroke();
    ctx.fillStyle = "#16a34a";
    ctx.beginPath(); ctx.moveTo(75, 85); ctx.arc(75, 85, 12, Math.PI, 1.5 * Math.PI); ctx.closePath(); ctx.fill();
    void f;
  },
  caps(ctx) {
    ctx.strokeStyle = "#0f172a"; ctx.lineWidth = 10;
    (["butt", "round", "square"] as const).forEach((cap, i) => {
      ctx.lineCap = cap; ctx.beginPath(); ctx.moveTo(15, 15 + i * 18); ctx.lineTo(60, 15 + i * 18); ctx.stroke();
    });
    ctx.lineCap = "butt"; ctx.strokeStyle = "#7c3aed"; ctx.lineWidth = 8;
    (["miter", "round", "bevel"] as const).forEach((join, i) => {
      ctx.lineJoin = join; ctx.beginPath(); ctx.moveTo(80 + i * 22, 85); ctx.lineTo(90 + i * 22, 20); ctx.lineTo(100 + i * 22, 85); ctx.stroke();
    });
    ctx.lineJoin = "miter"; ctx.miterLimit = 1.5; ctx.lineWidth = 6; ctx.strokeStyle = "#f97316";
    ctx.beginPath(); ctx.moveTo(10, 90); ctx.lineTo(30, 70); ctx.lineTo(50, 90); ctx.stroke();
  },
  dash(ctx) {
    ctx.strokeStyle = "#0f172a"; ctx.lineWidth = 3;
    ctx.setLineDash([10, 5]); ctx.beginPath(); ctx.moveTo(10, 15); ctx.lineTo(140, 15); ctx.stroke();
    ctx.setLineDash([8, 4, 2]); ctx.beginPath(); ctx.moveTo(10, 35); ctx.lineTo(140, 35); ctx.stroke();
    ctx.lineDashOffset = 6; ctx.beginPath(); ctx.moveTo(10, 55); ctx.lineTo(140, 55); ctx.stroke();
    ctx.lineDashOffset = 0; ctx.setLineDash([12, 6]); ctx.lineWidth = 4; ctx.strokeStyle = "#dc2626";
    ctx.beginPath(); ctx.arc(75, 78, 16, 0, TAU); ctx.stroke();
    ctx.setLineDash([]); ctx.strokeRect(110, 65, 30, 25);
  },
  arcto(ctx) {
    ctx.strokeStyle = "#0f172a"; ctx.lineWidth = 3;
    ctx.beginPath(); ctx.moveTo(10, 90); ctx.arcTo(10, 10, 70, 10, 20); ctx.arcTo(70, 10, 70, 90, 30); ctx.lineTo(70, 90); ctx.stroke();
    ctx.strokeStyle = "#2563eb";
    ctx.beginPath(); ctx.moveTo(85, 20); ctx.arcTo(110, 20, 140, 20, 15); ctx.lineTo(140, 45); ctx.stroke(); // collinear
    ctx.beginPath(); ctx.moveTo(85, 60); ctx.arcTo(140, 60, 140, 90, 0); ctx.lineTo(140, 90); ctx.stroke(); // zero radius
    ctx.fillStyle = "#16a34a"; ctx.beginPath(); ctx.arcTo(100, 80, 120, 90, 10); ctx.lineTo(110, 95); ctx.fill(); // no subpath
  },
  roundrect(ctx) {
    ctx.fillStyle = "#0ea5e9"; ctx.beginPath(); ctx.roundRect(8, 8, 60, 38, 10); ctx.fill();
    ctx.fillStyle = "#a855f7"; ctx.beginPath(); ctx.roundRect(80, 8, 62, 38, [2, 10, 18, 26]); ctx.fill();
    ctx.fillStyle = "#f59e0b"; ctx.beginPath(); ctx.roundRect(8, 54, 60, 38, [{ x: 20, y: 8 }]); ctx.fill();
    ctx.strokeStyle = "#0f172a"; ctx.lineWidth = 3; ctx.beginPath(); ctx.roundRect(142, 54, -62, 38, [4, 16]); ctx.stroke();
    ctx.fillStyle = "#ef4444"; ctx.beginPath(); ctx.roundRect(100, 62, 20, 20, 40); ctx.fill(); // radii scaled down
  },
  ellipse(ctx) {
    ctx.fillStyle = "#16a34a"; ctx.beginPath(); ctx.ellipse(40, 50, 30, 16, 0.5, 0, TAU); ctx.fill();
    ctx.strokeStyle = "#0f172a"; ctx.lineWidth = 2; ctx.beginPath(); ctx.ellipse(40, 50, 30, 16, 0.5, 0, Math.PI, true); ctx.stroke();
    // A five-point star, even-odd: a hole in the middle.
    ctx.fillStyle = "#e11d48"; ctx.beginPath();
    for (let i = 0; i < 5; i++) { const a = -Math.PI / 2 + (i * 4 * Math.PI) / 5; ctx.lineTo(110 + 32 * Math.cos(a), 52 + 32 * Math.sin(a)); }
    ctx.closePath(); ctx.fill("evenodd");
  },
  transform(ctx) {
    ctx.save(); ctx.translate(30, 30); ctx.rotate(0.4); ctx.fillStyle = "#0ea5e9"; ctx.fillRect(-15, -10, 30, 20); ctx.restore();
    ctx.save(); ctx.translate(75, 50); ctx.scale(3, 1); ctx.strokeStyle = "#dc2626"; ctx.lineWidth = 2;
    ctx.beginPath(); ctx.arc(0, 0, 10, 0, TAU); ctx.stroke(); ctx.restore(); // the stroke widens with x
    ctx.save(); ctx.transform(1, 0.3, -0.4, 1, 110, 60); ctx.fillStyle = "#84cc16"; ctx.fillRect(0, 0, 25, 25); ctx.restore();
    ctx.setTransform(1, 0, 0, 1, 10, 70); ctx.strokeStyle = "#7c3aed"; ctx.lineWidth = 4; ctx.strokeRect(0, 0, 40, 20);
    ctx.resetTransform(); ctx.fillStyle = "#0f172a"; ctx.fillRect(140, 5, 5, 5);
  },
  kept(ctx) {
    // The current path survives fill, clip-free clearRect and stroke.
    ctx.beginPath(); ctx.moveTo(20, 80); ctx.lineTo(60, 15); ctx.lineTo(100, 80); ctx.closePath();
    ctx.fillStyle = "#fde047"; ctx.fill();
    ctx.clearRect(50, 50, 20, 20);
    ctx.strokeStyle = "#0f172a"; ctx.lineWidth = 4; ctx.stroke();
    ctx.rect(110, 20, 30, 60); ctx.fillStyle = "rgba(37, 99, 235, 0.5)"; ctx.fill();
  },

  linear(ctx) {
    const g = ctx.createLinearGradient(10, 0, 140, 0);
    g.addColorStop(0, "#ef4444"); g.addColorStop(1, "#2563eb");
    ctx.fillStyle = g; ctx.fillRect(10, 10, 130, 35);
    // A stop added after assignment affects the next paint, not the last.
    g.addColorStop(0.5, "#facc15");
    ctx.fillRect(10, 55, 130, 35);
  },
  radial(ctx) {
    const a = ctx.createRadialGradient(40, 50, 4, 40, 50, 36);
    a.addColorStop(0, "#ffffff"); a.addColorStop(0.6, "hsl(160 80% 40%)"); a.addColorStop(1, "rgba(0, 0, 0, 0)");
    ctx.fillStyle = a; ctx.fillRect(0, 0, 80, 100);
    const b = ctx.createRadialGradient(100, 40, 5, 115, 55, 30);
    b.addColorStop(0, "#f97316"); b.addColorStop(1, "#7c2d12");
    ctx.fillStyle = b; ctx.beginPath(); ctx.arc(115, 55, 30, 0, TAU); ctx.fill();
    ctx.strokeStyle = a; ctx.lineWidth = 6; ctx.strokeRect(85, 8, 55, 12);
  },
  alpha(ctx) {
    ctx.globalAlpha = 0.6;
    ctx.fillStyle = "#ef4444"; ctx.fillRect(15, 15, 60, 50);
    ctx.fillStyle = "#2563eb"; ctx.fillRect(45, 35, 60, 50);
    ctx.globalAlpha = 1; ctx.fillStyle = "rgba(22, 163, 74, 0.5)"; ctx.beginPath(); ctx.arc(110, 40, 26, 0, TAU); ctx.fill();
    ctx.globalAlpha = 0.25; ctx.strokeStyle = "#0f172a"; ctx.lineWidth = 8; ctx.strokeRect(10, 70, 130, 20);
  },
  composite(ctx) {
    const ops: GlobalCompositeOperation[] = ["source-over", "source-atop", "destination-over", "destination-out", "xor", "lighter", "multiply", "screen",
      "overlay", "darken", "lighten", "color-dodge", "color-burn", "hard-light", "soft-light", "difference", "exclusion",
      "hue", "saturation", "color", "luminosity"];
    ops.forEach((op, i) => {
      const x = (i % 7) * 21 + 2, y = Math.floor(i / 7) * 33 + 2;
      ctx.save();
      ctx.beginPath(); ctx.rect(x, y, 20, 32); ctx.clip();
      ctx.fillStyle = "rgba(37, 99, 235, 0.7)"; ctx.fillRect(x, y, 14, 22);
      ctx.globalCompositeOperation = op;
      ctx.fillStyle = "rgba(239, 68, 68, 0.8)"; ctx.beginPath(); ctx.arc(x + 13, y + 20, 9, 0, TAU); ctx.fill();
      ctx.restore();
    });
  },
  clip(ctx) {
    const star = (cx: number, cy: number) => { ctx.beginPath(); for (let i = 0; i < 5; i++) { const a = -Math.PI / 2 + (i * 4 * Math.PI) / 5; ctx.lineTo(cx + 34 * Math.cos(a), cy + 34 * Math.sin(a)); } ctx.closePath(); };
    ctx.save(); star(38, 52); ctx.clip(); ctx.fillStyle = "#0ea5e9"; ctx.fillRect(0, 0, 75, 100); ctx.restore();
    ctx.save(); star(112, 52); ctx.clip("evenodd"); ctx.fillStyle = "#f43f5e"; ctx.fillRect(75, 0, 75, 100); ctx.restore();
    ctx.fillStyle = "#0f172a"; ctx.fillRect(70, 45, 10, 10); // outside every clip again
  },
  clear(ctx) {
    ctx.fillStyle = "#1e293b"; ctx.fillRect(0, 0, 150, 100);
    ctx.save(); ctx.translate(45, 50); ctx.rotate(0.5); ctx.clearRect(-20, -12, 40, 24); ctx.restore();
    ctx.save(); ctx.beginPath(); ctx.arc(110, 50, 25, 0, TAU); ctx.clip(); ctx.clearRect(90, 20, 60, 30); ctx.restore();
  },
  // An explicit bitmap (LLP 1056 D6, r3): 40 × 25 pixels stretched to the
  // box, blurred as the browser blurs it.
  bitmap(ctx, f) {
    ctx.fillStyle = "#e2e8f0"; ctx.fillRect(0, 0, f.width, f.height);
    ctx.fillStyle = "#0f172a";
    for (let y = 0; y < 5; y++) for (let x = 0; x < 8; x++) if ((x + y) % 2) ctx.fillRect(x * 5, y * 5, 5, 5);
    ctx.strokeStyle = "#dc2626"; ctx.lineWidth = 1; ctx.beginPath(); ctx.moveTo(0, 25); ctx.lineTo(40, 0); ctx.stroke();
  },

  // Sequences (page 3): each draw is one step of `step`, on the same context.
  accumulate(ctx, f, [step]) {
    // No clearing: every draw adds a dot; the bitmap keeps the last ones.
    ctx.fillStyle = `hsl(${step * 60} 70% 45%)`;
    ctx.beginPath(); ctx.arc(20 + step * 28, 50, 11, 0, TAU); ctx.fill();
    void f;
  },
  stack(ctx, f, [step]) {
    // A save in one draw and its restore in another: state persists.
    if (step === 0) { ctx.save(); ctx.fillStyle = "#ef4444"; ctx.translate(40, 0); }
    ctx.fillRect(10 + step * 12, 20 + step * 18, 30, 14);
    if (step === 1) ctx.restore();
    void f;
  },
  reset(ctx, f, [step]) {
    // A clip set in one draw bounds the next draw's clearRect; reset drops it.
    if (step === 0) { ctx.fillStyle = "#16a34a"; ctx.fillRect(0, 0, f.width, f.height); ctx.beginPath(); ctx.rect(0, 0, 75, 100); ctx.clip(); }
    if (step === 1) ctx.clearRect(0, 0, f.width, f.height);
    if (step === 2) { ctx.reset(); ctx.fillStyle = "#2563eb"; ctx.fillRect(20, 20, 110, 60); }
    if (step === 3) ctx.clearRect(0, 0, 60, 60);
  },
  throws(ctx, f, [step]) {
    // A throw keeps what was drawn before it (LLP 1056 D4, r3).
    ctx.fillStyle = "#f59e0b"; ctx.fillRect(10 + step * 20, 10, 18, 18);
    if (step % 2 === 1) { ctx.fillStyle = "#0f172a"; ctx.fillRect(10 + step * 20, 40, 18, 18); ctx.arc(0, 0, -1, 0, 1); }
    ctx.fillStyle = "#dc2626"; ctx.fillRect(10 + step * 20, 70, 18, 18);
    void f;
  },
  resize(ctx, f, [step]) {
    // The box grows each step: a new generation, a cleared bitmap and a
    // fresh context (the fill style below is the default black again).
    if (step === 0) ctx.fillStyle = "#7c3aed";
    ctx.fillRect(4, 4, f.width - 8, 14);
    ctx.strokeRect(0.5, 0.5, f.width - 1, f.height - 1);
  },
};

/** Which fixtures take `step`; the rest take nothing. */
export const sequences: string[] = ["accumulate", "stack", "reset", "throws", "resize"];

/** The surface roster: name → arity. */
export const surfaces: Record<string, number> = Object.fromEntries(Object.keys(fixtures).map((name) => [name, sequences.includes(name) ? 1 : 0]));
