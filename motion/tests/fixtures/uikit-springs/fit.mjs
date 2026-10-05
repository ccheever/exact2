// LLP 1099 §4: checks the proposed mapping against a probe report.
//   bun motion/tests/fixtures/uikit-springs/fit.mjs [report]   (default ios-27.0.txt)
// Prints each family's agreement with what UIKit / Core Animation did.
// Positions are fractions of the move (0 → 1); velocities are moves per second.
import { readFileSync } from 'node:fs';

const EPS = 1e-3;
const file = process.argv[2] ?? new URL('ios-27.0.txt', import.meta.url).pathname;
const lines = readFileSync(file, 'utf8').split('\n');
const fields = (l) => Object.fromEntries([...l.matchAll(/(\w+)=(-?[\d.e+-]+|true|false)/g)].map(([, k, v]) => [k, v === 'true' ? true : v === 'false' ? false : Number(v)]));
const rows = (tag) => lines.filter((l) => l.startsWith(tag + ' ')).map(fields);
const samples = (l) => [...l.matchAll(/ ([\d.e-]+):(-?[\d.e+-]+)/g)].map(([, t, x]) => [Number(t), Number(x)]);

// The duration form: UIKit's settle equation, solved in W = ω·duration with
// u = velocity·duration. Newton from W = 5, twelve steps, halving on a
// non-positive step. ζ above 1 is clamped to 1, as UIKit does.
export function durationW(zeta, u) {
  const z = Math.min(zeta, 1);
  let W = 5;
  for (let i = 0; i < 12; i++) {
    const e = Math.exp(-z * W);
    let g, dg;
    if (z < 1) {
      const s = Math.sqrt(1 - z * z), B = (u / W - z) / s;
      g = Math.abs(B) * e - EPS;
      dg = ((B < 0 ? -1 : 1) * (-u / (W * W)) / s - z * Math.abs(B)) * e;
    } else {
      const P = u - W - 1;
      g = Math.abs(P) * e - EPS;
      dg = (-(P < 0 ? -1 : 1) - Math.abs(P)) * e;
    }
    if (dg === 0) break;
    let next = W - g / dg;
    if (next <= 0) next = W / 2;
    W = next;
  }
  return W;
}
// v = 0 has a closed form: ζ·W = ln(ζ / (ε·√(1−ζ²))); at ζ = 1, (1+W)e^−W = ε.
export const durationW0 = (z) => (z < 1 ? Math.log(z / (EPS * Math.sqrt(1 - z * z))) / z : 9.233413476451585);

// Core Animation's CASpringAnimation, as measured (§4.5): damping clamped to
// critical unless allowsOverdamping; with it, the overdamped solution pairs
// each coefficient with the other exponent.
export function caPos(k, c, v0, t, aod = false) {
  const w = Math.sqrt(k);
  let z = c / (2 * w);
  if (z > 1 && !aod) z = 1;
  if (z < 1) {
    const wd = w * Math.sqrt(1 - z * z), B = (v0 - z * w) / wd;
    return 1 + Math.exp(-z * w * t) * (-Math.cos(wd * t) + B * Math.sin(wd * t));
  }
  if (z === 1) return 1 + (-1 + (v0 - w) * t) * Math.exp(-w * t);
  const r = w * Math.sqrt(z * z - 1), s1 = -z * w + r, s2 = -z * w - r, a = (s2 - v0) / (s1 - s2);
  return 1 + a * Math.exp(s2 * t) + (-1 - a) * Math.exp(s1 * t);
}
// Core Animation's settlingDuration below critical damping:
// e^(−ζωT)·(|A| + |B|) = ε, A = −1 and B the sine coefficient.
export function caSettle(k, c, v0) {
  const w = Math.sqrt(k), z = c / (2 * w), B = (v0 - z * w) / (w * Math.sqrt(1 - z * z));
  return Math.log((1 + Math.abs(B)) / EPS) / (z * w);
}
// The bounce form's end time at or above critical damping: the critically
// damped settle time, the last T with |−1 + (v0 − ω)·T|·e^(−ωT) = ε. Scan
// back from 100/ω in steps of 0.01/ω to the last sign change, then bisect.
export function criticalSettle(w, v0) {
  const f = (T) => Math.abs(-1 + (v0 - w) * T) * Math.exp(-w * T) - EPS;
  let hi = 100 / w, lo = hi;
  while (lo > 0 && f(lo) <= 0) lo -= 0.01 / w;
  if (lo <= 0) return 0;
  hi = lo + 0.01 / w;
  for (let i = 0; i < 200; i++) { const m = (lo + hi) / 2; if (f(m) > 0) lo = m; else hi = m; }
  return lo;
}
export const bounceSpring = (d, b) => ({ k: (2 * Math.PI / d) ** 2, c: b >= 0 ? (4 * Math.PI * (1 - b)) / d : (4 * Math.PI) / (d * (1 + b)) });

const pct = (xs, p) => xs.slice().sort((a, b) => a - b)[Math.min(xs.length - 1, Math.floor(p * xs.length))];
const curveErr = (k1, k2, z, v0, d) => {
  let m = 0;
  for (let i = 0; i <= 400; i++) { const t = (d * i) / 400; m = Math.max(m, Math.abs(caPos(k1, 2 * z * Math.sqrt(k1), v0, t) - caPos(k2, 2 * z * Math.sqrt(k2), v0, t))); }
  return m;
};
const fmt = (x) => x.toExponential(2);
console.log(lines[0]);

// 1. Duration + damping ratio (A grid, W sweep, B/C animators).
for (const [name, set] of [
  ['A grid (d × ζ × v)', rows('A').filter((r) => r.d !== undefined && r.z !== undefined).map((r) => ({ z: r.z, v: r.v, d: r.d, k: r.stiffness, c: r.damping, m: r.mass, v0: r.v0, dur: r.duration }))],
  ['W sweep (d = 1)', rows('W').map((r) => ({ z: r.z, v: r.u, d: 1, k: r.k }))],
  ['B/C property animators', [...rows('B'), ...rows('C')].map((r) => ({ z: r.z, v: r.v, d: r.d, k: r.stiffness, c: r.damping, m: r.mass, v0: r.v0, dur: r.duration }))],
]) {
  let exact = 0, shape = 0, v0exact = 0, v0n = 0;
  const errs = [], miss = [];
  for (const r of set) {
    const z = Math.min(r.z, 1), u = r.v * r.d, W = durationW(r.z, u), Wm = Math.sqrt(r.k) * r.d;
    if (r.c !== undefined && Math.abs(r.c / (2 * Math.sqrt(r.k)) - z) > 1e-9) shape++;
    if (r.m !== undefined && (r.m !== 1 || r.v0 !== r.v || r.dur !== r.d)) shape++;
    const ok = Math.abs(Math.log(W / Wm)) < 1e-5;
    if (ok) exact++;
    if (r.v === 0) { v0n++; if (Math.abs(durationW0(z) / Wm - 1) < 1e-7) v0exact++; }
    const e = curveErr((W / r.d) ** 2, r.k, z, r.v, r.d);
    errs.push(e);
    if (!ok) miss.push(`ζ ${r.z} v·d ${u.toFixed(3)}: UIKit W ${Wm.toFixed(4)}, model ${W.toFixed(4)}, curve Δ ${e.toFixed(4)}`);
  }
  console.log(`\n${name}: ${set.length} calls; damping = 2·min(ζ,1)·√k, mass 1, v0 = v, end = d: ${shape ? shape + ' violations' : 'all'}`);
  const v0rows = set.filter((r) => r.v === 0);
  const wErr = Math.max(0, ...v0rows.map((r) => Math.abs(durationW0(Math.min(r.z, 1)) / (Math.sqrt(r.k) * r.d) - 1)));
  console.log(`  closed form at v = 0: ${v0exact}/${v0n} within 1e-7; worst relative error ${fmt(wErr)} in W, ${fmt((1 + wErr) ** 2 - 1)} in k`);
  console.log(`  model W within 1e-5: ${exact}/${set.length}; curve error over [0, d]: median ${fmt(pct(errs, 0.5))}, p99 ${fmt(pct(errs, 0.99))}, max ${errs.length ? Math.max(...errs).toFixed(4) : '-'}`);
  for (const m of miss.slice(0, 12)) console.log('   miss ' + m);
  if (miss.length > 12) console.log(`   … ${miss.length - 12} more`);
}

// 2. Duration + bounce (iOS 17): stiffness, damping, and where it ends.
{
  const E = [...rows('E'), ...rows('E2')];
  let kc = 0, bits = 0, kcWorst = 0, endWorst = 0;
  for (const r of E) {
    const { k, c } = bounceSpring(r.d, r.b);
    kcWorst = Math.max(kcWorst, Math.abs(k / r.stiffness - 1), Math.abs(c / r.damping - 1));
    if (Math.abs(k / r.stiffness - 1) < 1e-12 && Math.abs(c / r.damping - 1) < 1e-12) kc++;
    if (k === r.stiffness && c === r.damping) bits++;
    const w = Math.sqrt(k), v0 = r.v ?? 0;
    const end = c / (2 * w) < 1 ? caSettle(k, c, v0) : criticalSettle(w, v0);
    endWorst = Math.max(endWorst, Math.abs(end / r.duration - 1));
  }
  console.log(`\nE duration + bounce: ${E.length} calls; k = (2π/d)², c = 4π(1−b)/d or 4π/(d(1+b)): ${kc}/${E.length} within 1e-12 (worst ${fmt(kcWorst)}; ${bits} bit-identical); end time worst relative error ${fmt(endWorst)}`);
}

// 3. Core Animation's settlingDuration below critical damping (every row that reports one).
{
  let n = 0, worst = 0;
  for (const tag of ['A', 'B', 'C', 'D', 'E']) for (const r of rows(tag)) {
    if (r.damping / (2 * Math.sqrt(r.stiffness)) >= 1 || r.settling === undefined) continue;
    n++; worst = Math.max(worst, Math.abs(caSettle(r.stiffness, r.damping, r.v0) / r.settling - 1));
  }
  console.log(`\nCA settlingDuration (ζ < 1): ${n} rows, worst relative error ${fmt(worst)}`);
  const D = rows('D');
  console.log(`D mass/stiffness/damping parameters: end = settlingDuration in ${D.filter((r) => r.duration === r.settling).length}/${D.length}`);
}

// 4. SwiftUI's Spring fields.
{
  const F = rows('F'), G = rows('G');
  const okF = F.filter((r) => r.z <= 1 && Math.abs(r.stiffness / (2 * Math.PI / r.response) ** 2 - 1) < 1e-9 && Math.abs(r.damping / ((4 * Math.PI * r.z) / r.response) - 1) < 1e-9).length;
  const over = [...F.filter((r) => r.z > 1).map((r) => [r.z, r.stiffness / (2 * Math.PI / r.response) ** 2]), ...G.filter((r) => r.bounce < 0).map((r) => [1 / (1 + r.bounce), r.stiffness / (2 * Math.PI / r.duration) ** 2])];
  console.log(`\nSwiftUI Spring(response:dampingRatio:), ζ ≤ 1: ${okF}/${F.filter((r) => r.z <= 1).length} match k = (2π/r)², c = 4πζ/r`);
  console.log(`  above 1, stiffness / (2π/r)² = ${over.map(([z, f]) => `${f.toFixed(4)} at ζ ${z} (2ζ²−1 = ${(2 * z * z - 1).toFixed(4)})`).join('; ')}`);
  const G2 = rows('G2');
  const A = rows('A');
  const gap = Math.max(...G2.map((r) => { const u = A.find((a) => a.d === r.settling && a.z === r.z && a.v === 0); return Math.abs(r.stiffness / u.stiffness - 1); }));
  console.log(`  Spring(settlingDuration:dampingRatio:) vs UIKit's duration form at v = 0 (same d, ζ): worst relative stiffness gap ${fmt(gap)}`);
}

// 5. Rendered curves: Core Animation's presentation vs caPos, before the end; the value after it.
console.log('\nRendered curves (presentation layer, 10 ms):');
for (const l of lines.filter((l) => /^[US] /.test(l))) {
  const r = fields(l.replace(/ [\d.e-]+:-?[\d.e+-]+/g, ''));
  const pts = samples(l);
  const inside = pts.filter(([t]) => t < r.dur - 1e-9);
  const err = Math.max(...inside.map(([t, x]) => Math.abs(x - caPos(r.k, r.c, r.v0, t, r.aod))));
  const after = [...new Set(pts.filter(([t]) => t > r.dur + 1e-9).map(([, x]) => +x.toFixed(6)))];
  const label = l.slice(0, l.indexOf(' k='));
  console.log(`  ${label.padEnd(24)} ζ ${(r.c / (2 * Math.sqrt(r.k))).toFixed(3)} aod ${r.aod}: max error ${fmt(err)}, last before end ${inside.at(-1)[1].toFixed(4)}, after end ${after.join(',')}`);
}

// 6. Where the duration form's model misses: per ζ, the velocity·duration
// range that holds every miss in the sweep (the solver's chaotic band).
console.log('\nDuration form, sweep misses by ζ (u = velocity × duration):');
{
  const by = new Map();
  for (const r of rows('W')) {
    const W = durationW(r.z, r.u), Wm = Math.sqrt(r.k);
    const e = curveErr(W * W, r.k, Math.min(r.z, 1), r.u, 1);
    if (!by.has(r.z)) by.set(r.z, { lo: Infinity, hi: -Infinity, n: 0, worst: 0, outside: 0 });
    const b = by.get(r.z);
    if (Math.abs(Math.log(W / Wm)) >= 1e-5) { b.lo = Math.min(b.lo, r.u); b.hi = Math.max(b.hi, r.u); b.n++; b.worst = Math.max(b.worst, e); }
  }
  for (const r of rows('W')) {
    const b = by.get(r.z);
    if (r.u < b.lo || r.u > b.hi) b.outside = Math.max(b.outside, curveErr(durationW(r.z, r.u) ** 2, r.k, Math.min(r.z, 1), r.u, 1));
  }
  for (const [z, b] of by) console.log(`  ζ ${String(z).padEnd(5)} ${b.n ? `misses ${String(b.n).padStart(2)} in u ∈ [${b.lo}, ${b.hi}], worst curve Δ ${b.worst.toFixed(3)}` : 'no misses'}; outside that band worst curve Δ ${fmt(b.outside)}`);
}
console.log('\nSignal-iOS calls (U lines):');
for (const l of lines.filter((l) => l.startsWith('U A '))) {
  const m = l.match(/d=([\d.]+) z=([\d.]+) v=([\d.]+)/), [d, z, v] = m.slice(1).map(Number), r = fields(l.replace(/ [\d.e-]+:-?[\d.e+-]+/g, ''));
  const k = (durationW(z, v * d) / d) ** 2;
  console.log(`  d ${d} ζ ${z} v ${v}: UIKit k ${r.k.toFixed(4)}, model k ${k.toFixed(4)}, curve Δ ${curveErr(k, r.k, Math.min(z, 1), v, d).toFixed(4)}`);
}

// 7. Retargets (R lines): position is the sum of additive components, each
// ending at its own duration; opacity is replaced, from the model value or,
// with .beginFromCurrentState, the presented one, without carried velocity.
console.log('\nRetargets (R lines):');
{
  const A = rows('A'), kOf = (d, z, v) => A.find((r) => r.d === d && r.z === z && r.v === v).stiffness;
  const k0 = kOf(0.4, 0.8, 0), k1 = kOf(0.4, 0.8, 1), c = (k) => 1.6 * Math.sqrt(k);
  for (const l of lines.filter((l) => l.startsWith('R '))) {
    const name = l.split(' ')[1], t1 = Number(l.match(/t1=([\d.]+)/)[1]);
    const pts = samples(l.slice(l.lastIndexOf(']') + 1));
    if (name.startsWith('center')) {
      // 50 → 150 at t 0 (v 0), then → 350 at t1 (v 1); both 0.4 s, ζ 0.8.
      let worst = 0;
      for (const [t, x] of pts) {
        let m = t < t1 - 1e-9 ? 150 : 350;
        if (t < 0.4 - 1e-9) m -= 100 * (1 - caPos(k0, c(k0), 0, t));
        if (t >= t1 - 1e-9 && t < t1 + 0.4 - 1e-9) m -= 200 * (1 - caPos(k1, c(k1), 1, t - t1));
        worst = Math.max(worst, Math.abs(m - x));
      }
      console.log(`  ${name.padEnd(12)} t1 ${t1}: two additive components, worst error ${worst.toFixed(4)} pt of a 300 pt move`);
    } else {
      const from = Number(l.match(/from=Optional\(([\d.]+)\)/)[1]);
      console.log(`  ${name.padEnd(12)} t1 ${t1}: one animation, from ${from} (${Math.abs(from - 0.2) < 1e-6 ? 'the model value: the presented value jumps' : 'the presented value'}), velocity 0`);
    }
  }
}

// 8. The proposed validation and band predicate (LLP 1099 D2): a result is a
// root when |g(W)| ≤ 0.1·ε and 0 < W ≤ 1e4; the band is u > 0 with
// 1 ≤ u/min(ζ,1) ≤ 7.
console.log('\nValidation and band predicate over A, W and the Signal U rows:');
{
  const g = (W, zeta, u) => { const z = Math.min(zeta, 1), e = Math.exp(-z * W); return z < 1 ? Math.abs((u / W - z) / Math.sqrt(1 - z * z)) * e - EPS : Math.abs(u - W - 1) * e - EPS; };
  const pts = [...rows('A').filter((r) => r.z !== undefined && r.d !== undefined).map((r) => [r.z, r.v * r.d, Math.sqrt(r.stiffness) * r.d]), ...rows('W').map((r) => [r.z, r.u, Math.sqrt(r.k)])];
  for (const l of lines.filter((l) => l.startsWith('U A '))) { const [d, z, v] = l.match(/d=([\d.]+) z=([\d.]+) v=([\d.]+)/).slice(1).map(Number); pts.push([z, v * d, Math.sqrt(fields(l.replace(/ [\d.e-]+:-?[\d.e+-]+/g, '')).k) * d]); }
  let miss = 0, inBand = 0, flagged = 0, refused = 0, refusedMatch = 0, worstMatch = 0;
  for (const [z, u, Wm] of pts) {
    const W = durationW(z, u), match = Math.abs(Math.log(W / Wm)) < 1e-5, band = u > 0 && u / Math.min(z, 1) >= 1 && u / Math.min(z, 1) <= 7;
    const root = Number.isFinite(W) && W > 0 && W <= 1e4 && Math.abs(g(W, z, u)) <= 0.1 * EPS;
    if (band) flagged++;
    if (!match) { miss++; if (band) inBand++; }
    if (!root) { refused++; if (match) refusedMatch++; }
    if (match) worstMatch = Math.max(worstMatch, Math.abs(g(W, z, u)) / EPS);
  }
  console.log(`  ${pts.length} calls: ${miss} misses, ${inBand} of them inside the band; the band flags ${flagged}; validation refuses ${refused} (${refusedMatch} of which UIKit matched); worst |g|/ε among matches ${worstMatch.toFixed(3)}`);
}
// 9. Edge cases (X lines), printed for the record.
console.log('\nEdge cases (X lines):');
for (const l of lines.filter((l) => l.startsWith('X '))) console.log('  ' + l.replace(/ mass=1\.0| additive=true/g, ''));
