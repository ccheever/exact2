// The motion parity smoke (LLP 1011.000, LLP 1055 D7): `apps/motion-gallery`
// on each native host against Chrome's at fixed clock times. Chrome (the web
// host in agent mode) is the reference: its animated images are its own
// decoder's frames placed by Chrome's schedule (host/web/image-glue.js), and
// its keyframes are the browser's own, seeked. Each `fx-*` box is cropped by
// its host's layout at one pixel per point, registered within ±2 px, and
// compared: mean |Δ| ≤ 4/255 and ≤ 6% of pixels off by more than 32. Linux
// decodes no GIF or WebP (LLP 1011.000 §4): its image fixtures are declared
// unsupported. Failing crops are written out in pairs.
// Run: `bun scripts/smoke.mjs motion [--hosts linux,macos,ios]`.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { encodePng } from './png.mjs';
import { boxes, register, shot } from './parity.mjs';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const MEAN = 4, OFF = 0.06, BAND = 32, SLOP = 2;
/** Clock times read, ms: inside frames, on none of their edges. */
export const TIMES = [0, 130, 370, 650, 1010, 2590, 6130];
const IMAGES = /^fx-(gif|once|zero|large|webp|alpha|round)$/;
const UNSUPPORTED = { linux: IMAGES };

function webBuild() {
  const dist = resolve(ROOT, 'target/web-dist/motion-gallery');
  if (process.env.EXACT_MOTION_REBUILD === '1' || !existsSync(resolve(dist, '.exact-build.json'))) {
    mkdirSync(dirname(dist), { recursive: true });
    const b = spawnSync('bun', [resolve(ROOT, 'host/web/build.mjs'), 'motion-gallery-web'], { cwd: ROOT, env: { ...process.env, EXACT_WEB_DIST: dist }, stdio: 'inherit' });
    if (b.status !== 0) throw new Error('the motion-gallery web build failed');
  }
  return dist;
}

/** One host's fixtures at each time: `{time: {testId: RGBA crop}}`. */
async function capture(open, host, dir) {
  const env = host === 'linux' ? { EXACT_PAINTER: 'cpu' } : undefined;
  const s = await open({ host, app: 'motion-gallery', env, ...(host === 'web' ? { webDist: webBuild() } : {}) });
  const out = {};
  try {
    for (const t of TIMES) {
      await s.clock(String(t));
      out[t] = boxes(await shot(s, host, dir, `t${t}`), { slop: SLOP });
    }
    return out;
  } finally {
    await s.close?.();
  }
}

/** Run the comparison; `check(ok, what)` records a failure. */
export async function motionParity({ open, check, hosts }) {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-motion-parity-'));
  const ref = await capture(open, 'web', dir);
  const rows = [];
  for (const host of hosts) {
    const got = await capture(open, host, dir);
    for (const t of TIMES) {
      for (const [id, a] of Object.entries(ref[t])) {
        if (UNSUPPORTED[host]?.test(id)) continue;
        const b = got[t]?.[id];
        if (!check(a && b && a.width === b.width && a.height === b.height, `${host} t=${t} ${id}: no box to compare`)) continue;
        const best = register(a, b, { slop: SLOP, band: BAND });
        const ok = best.mean <= MEAN && best.off <= OFF;
        rows.push({ host, t, id, ...best, ok });
        if (!check(ok, `${host} t=${t} ${id}: mean |Δ| ${best.mean.toFixed(2)}/255, ${(best.off * 100).toFixed(2)}% off by > ${BAND}`)) {
          mkdirSync(resolve(dir, 'failed'), { recursive: true });
          writeFileSync(resolve(dir, 'failed', `${host}-t${t}-${id}.png`), encodePng(b));
          writeFileSync(resolve(dir, 'failed', `web-t${t}-${id}.png`), encodePng(a));
        }
      }
    }
  }
  for (const r of rows.filter((r) => !r.ok)) console.log(`motion parity ${r.host.padEnd(6)} t=${String(r.t).padStart(4)} ${r.id.padEnd(10)} mean ${r.mean.toFixed(2).padStart(5)}  off ${(r.off * 100).toFixed(2).padStart(5)}%  FAIL`);
  for (const host of hosts) {
    const mine = rows.filter((r) => r.host === host);
    if (mine.length) console.log(`motion parity ${host}: ${mine.length} crops (${TIMES.length} times), mean |Δ| ${(mine.reduce((a, r) => a + r.mean, 0) / mine.length).toFixed(2)}/255, worst ${Math.max(...mine.map((r) => r.mean)).toFixed(2)} (${mine.reduce((w, r) => (r.mean > w.mean ? r : w)).id}); pixels off by > ${BAND}: worst ${(100 * Math.max(...mine.map((r) => r.off))).toFixed(2)}%${UNSUPPORTED[host] ? '; images declared unsupported' : ''}`);
  }
  console.log(`motion parity: crops in ${dir}`);
}
