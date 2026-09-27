// The SVG parity smoke (LLP 1055.000 §5; QUEUE: LLP 1055's hand comparison
// made a smoke): `apps/svg-gallery` drawn by Chrome (the web host) is the
// reference; each native host draws the same pages, and every `fx-*` box is
// cropped from both screenshots by its own host's layout, brought to one
// pixel per point, registered within ±3 px, and compared. A fixture fails
// beyond mean |Δ| 4/255 or 6% of pixels off by more than 32 (LLP 1055's
// measured residue: antialiasing and Chrome's 1× raster). Failing crops are
// written out in pairs. Run: `bun scripts/smoke.mjs svg [--hosts linux,macos,ios]`.
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { crop, decodePng, encodePng } from './png.mjs';

const MEAN = 4, OFF = 0.06, BAND = 32, SLOP = 3;
// Linux paints text in its pinned font, not Chrome's system font, and does
// not stroke text yet: its text fixtures are held to a looser bound.
const TEXT = /^fx-(axis|baselines|runs|boxcolor)$/;
const limits = (host, id) => (host === 'linux' && TEXT.test(id) ? { mean: 14, off: 0.16 } : { mean: MEAN, off: OFF });

/** An RGBA image averaged down by an integer factor. */
function shrink(img, k) {
  if (k === 1) return img;
  const width = Math.floor(img.width / k), height = Math.floor(img.height / k);
  const data = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
    for (let c = 0; c < 4; c++) {
      let sum = 0;
      for (let j = 0; j < k; j++) for (let i = 0; i < k; i++) sum += img.data[((y * k + j) * img.width + x * k + i) * 4 + c];
      data[(y * width + x) * 4 + c] = Math.round(sum / (k * k));
    }
  }
  return { width, height, data };
}

/** Mean |Δ| and the share of pixels off by more than `BAND`, `b` shifted by (dx, dy) over `a`'s interior. */
function compare(a, b, dx, dy) {
  let sum = 0, off = 0, n = 0;
  for (let y = SLOP; y < a.height - SLOP; y++) for (let x = SLOP; x < a.width - SLOP; x++) {
    const i = (y * a.width + x) * 4, j = ((y + dy) * b.width + x + dx) * 4;
    let m = 0;
    for (let c = 0; c < 3; c++) { const d = Math.abs(a.data[i + c] - b.data[j + c]); sum += d; if (d > m) m = d; }
    if (m > BAND) off++;
    n++;
  }
  return { mean: sum / (3 * n), off: off / n };
}

/** One host's fixtures on every page: `{page: {testId: RGBA crop at 1 px/pt}}`. */
async function capture(open, host, dir) {
  const s = await open({ host, app: 'svg-gallery', env: host === 'linux' ? { EXACT_PAINTER: 'cpu' } : undefined });
  try {
    const tree = await s.tree();
    const pages = tree.nodes.map((n) => n.props?.testId).filter((t) => t?.startsWith('page-')).sort();
    const out = {};
    for (const page of pages) {
      await s.tap(page);
      await s.clock('settle');
      const layout = await s.layout();
      let path = resolve(dir, `${host}-${page}.png`);
      const reply = await s.screenshot(path, host === 'macos');
      if (host === 'macos' || host === 'ios') {
        // A native capture is in the display's profile (Display P3 on a
        // Mac); Chrome's is sRGB.
        const srgb = resolve(dir, `${host}-${page}.srgb.png`);
        const r = spawnSync('sips', ['--matchTo', '/System/Library/ColorSync/Profiles/sRGB Profile.icc', path, '--out', srgb], { encoding: 'utf8' });
        if (r.status !== 0) throw new Error(`sips: ${r.stderr}`);
        path = srgb;
      }
      const img = decodePng(readFileSync(path));
      const k = Math.round(img.width / layout.viewport.w);
      // A window capture carries the title bar above the viewport.
      const top = Math.max(0, Math.round(img.height / k - layout.viewport.h));
      const full = shrink(img, k);
      out[page] = {};
      for (const n of layout.nodes.filter((n) => n.testId?.startsWith('fx-'))) {
        const x = Math.round(n.x) - SLOP, y = Math.round(n.y) + top - SLOP, w = Math.round(n.w) + 2 * SLOP, h = Math.round(n.h) + 2 * SLOP;
        if (x < 0 || y < 0 || x + w > full.width || y + h > full.height) { out[page][n.testId] = null; continue; }
        out[page][n.testId] = crop(full, x, y, w, h);
      }
      void reply;
    }
    return out;
  } finally {
    await s.close?.();
  }
}

/** Run the comparison; `check(ok, what)` records a failure. */
export async function svgParity({ open, check, hosts }) {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-svg-parity-'));
  const ref = await capture(open, 'web', dir);
  const rows = [];
  for (const host of hosts) {
    const got = await capture(open, host, dir);
    for (const [page, fixtures] of Object.entries(ref)) {
      for (const [id, a] of Object.entries(fixtures)) {
        const b = got[page]?.[id];
        if (!check(a && b && a.width === b.width && a.height === b.height, `${host} ${page} ${id}: no box to compare (off screen or missing)`)) continue;
        let best = null;
        for (let dy = -SLOP; dy <= SLOP; dy++) for (let dx = -SLOP; dx <= SLOP; dx++) {
          const r = compare(a, b, dx, dy);
          if (!best || r.mean < best.mean) best = { ...r, dx, dy };
        }
        const limit = limits(host, id);
        const ok = best.mean <= limit.mean && best.off <= limit.off;
        rows.push({ host, page, id, ...best, ok });
        if (!check(ok, `${host} ${page} ${id}: mean |Δ| ${best.mean.toFixed(2)}/255, ${(best.off * 100).toFixed(2)}% off by > ${BAND} (limits ${limit.mean}, ${limit.off * 100}%)`)) {
          const pair = resolve(dir, 'failed');
          mkdirSync(pair, { recursive: true });
          writeFileSync(resolve(pair, `${host}-${page}-${id}.png`), encodePng(b));
          writeFileSync(resolve(pair, `web-${page}-${id}.png`), encodePng(a));
        }
      }
    }
  }
  for (const r of rows) console.log(`svg parity ${r.host.padEnd(6)} ${r.page} ${r.id.padEnd(16)} mean ${r.mean.toFixed(2).padStart(5)}  off ${(r.off * 100).toFixed(2).padStart(5)}%  shift ${r.dx},${r.dy}${r.ok ? '' : '  FAIL'}`);
  for (const host of hosts) {
    const mine = rows.filter((r) => r.host === host);
    if (mine.length) console.log(`svg parity ${host}: ${mine.length} fixtures, mean |Δ| ${(mine.reduce((a, r) => a + r.mean, 0) / mine.length).toFixed(2)}/255, worst ${Math.max(...mine.map((r) => r.mean)).toFixed(2)}; pixels off by > ${BAND}: mean ${(100 * mine.reduce((a, r) => a + r.off, 0) / mine.length).toFixed(2)}%, worst ${(100 * Math.max(...mine.map((r) => r.off))).toFixed(2)}%`);
  }
  console.log(`svg parity: crops in ${dir}`);
}
