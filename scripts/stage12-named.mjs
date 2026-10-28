// The gate's pixel comparison and stage 15 B batch 2's two additions to it
// (theme2's 23:16 approval of ~/x2theme/iosfix-s15b2-gate-plan.md):
// NAMED pixel changes, where a copy differs by design from the package
// variant it maps to, and the bindings check of theme2's option B.
//
// A named change is a container on a screen, by its testId, the SMALLEST
// holding the changed instances and what they move, with the change it
// expects in words. Differing pixels inside its box (the union of before's
// and after's boxes) are allowed and reported per name, count and bounding
// box; everywhere else outside the allowlist stays strict zero; a name
// whose box differs nowhere is reported STALE. Before-and-after crops of
// every named box go in the record for theme2 to read.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { crop, decodePng, encodePng } from './png.mjs';

/** The largest channel difference between pixel `i` of `p` and `j` of `q`. */
export const apart = (p, i, q, j) => Math.max(Math.abs(p[i] - q[j]), Math.abs(p[i + 1] - q[j + 1]), Math.abs(p[i + 2] - q[j + 2]), Math.abs(p[i + 3] - q[j + 3]));

/** Whether (x, y) is an anti-aliased edge: a 4-neighbour more than `edge` apart. */
function edgeAt(img, x, y, edge) {
  const i = (y * img.width + x) * 4;
  return [[1, 0], [-1, 0], [0, 1], [0, -1]].some(([dx, dy]) => {
    const nx = x + dx, ny = y + dy;
    return nx >= 0 && ny >= 0 && nx < img.width && ny < img.height && apart(img.data, i, img.data, (ny * img.width + nx) * 4) > edge;
  });
}

const within = ([rx, ry, rw, rh], x, y) => x >= rx && x < rx + rw && y >= ry && y < ry + rh;

/** Pixels differing between two screenshots outside `allow` (rectangles
 * [x, y, w, h], each with a reason): any channel apart, or more than
 * `noise.floor` apart on a before-shot edge pixel (`noise.edge`):
 * {differ, total, size}. A size change is every pixel. A differing pixel
 * inside a `named` box ({target, change, rect}) is that name's, the first
 * whose box holds it, and not in `differ`: `names` gives each one's count
 * and bounding box [x0, y0, x1, y1] (exclusive ends), null if none (only
 * where `named` has entries, so a stage without names reads as before). */
export function pixelDiff(a, b, allow = [], noise = null, named = []) {
  const A = decodePng(a), B = decodePng(b);
  const names = named.map(n => ({ target: n.target, change: n.change, rect: n.rect, differ: 0, box: null }));
  const said = r => (named.length ? { ...r, names } : r);
  if (A.width !== B.width || A.height !== B.height) return said({ differ: A.width * A.height, total: A.width * A.height, size: [A.width, A.height, B.width, B.height] });
  let differ = 0;
  for (let y = 0; y < A.height; y++) {
    for (let x = 0; x < A.width; x++) {
      const i = (y * A.width + x) * 4;
      const d = apart(A.data, i, B.data, i);
      if (d === 0 || allow.some(r => within(r, x, y))) continue;
      if (noise && d <= noise.floor && edgeAt(A, x, y, noise.edge)) continue;
      const n = names.find(m => within(m.rect, x, y));
      if (!n) { differ++; continue; }
      n.differ++;
      n.box = n.box ? [Math.min(n.box[0], x), Math.min(n.box[1], y), Math.max(n.box[2], x + 1), Math.max(n.box[3], y + 1)] : [x, y, x + 1, y + 1];
    }
  }
  return said({ differ, total: A.width * A.height });
}

/** The whole layout a drive's output ends with: the last op reply with nodes and a viewport. */
export function wholeLayout(out) {
  return out.split('\n').filter(l => l.startsWith('{') && l.includes('"nodes"')).map(l => { try { return JSON.parse(l); } catch { return null; } })
    .findLast(o => Array.isArray(o?.nodes) && o.viewport) ?? null;
}

/** `named`'s entries for `screen` (theirs or every screen's, `*`), each with
 * its box in a drive's whole layout: [x, y, w, h] in whole points (a shot
 * is a pixel a point), clipped to the viewport, or null where the layout
 * doesn't show its target. */
export function namedOf(named, screen, out) {
  const mine = named.filter(n => n.screen === screen || n.screen === '*');
  if (!mine.length) return [];
  const layout = wholeLayout(out);
  return mine.map(n => {
    const node = layout?.nodes.find(m => (m.testId ?? m.props?.testId) === n.target && m.w > 0 && m.h > 0);
    if (!node) return { target: n.target, change: n.change, rect: null };
    const v = layout.viewport;
    const x0 = Math.max(0, Math.floor(node.x)), y0 = Math.max(0, Math.floor(node.y));
    const x1 = Math.min(Math.ceil(v.w), Math.ceil(node.x + node.w)), y1 = Math.min(Math.ceil(v.h), Math.ceil(node.y + node.h));
    return { target: n.target, change: n.change, rect: x1 > x0 && y1 > y0 ? [x0, y0, x1 - x0, y1 - y0] : null };
  });
}

/** One shot's named boxes from both sides: each target either side shows,
 * its box the union of the two (a change of size moves its edge). */
export function unionNamed(before = [], after = []) {
  const targets = [...new Set([...before, ...after].map(n => n.target))];
  return targets.flatMap(target => {
    const rects = [...before, ...after].filter(n => n.target === target && n.rect).map(n => n.rect);
    if (!rects.length) return [];
    const x0 = Math.min(...rects.map(r => r[0])), y0 = Math.min(...rects.map(r => r[1]));
    const x1 = Math.max(...rects.map(r => r[0] + r[2])), y1 = Math.max(...rects.map(r => r[1] + r[3]));
    const change = [...before, ...after].find(n => n.target === target).change;
    return [{ target, change, rect: [x0, y0, x1 - x0, y1 - y0] }];
  });
}

/** Before's and after's crops of each named box that differs, written to
 * `dir` as `<shot>-<target>-before.png` and `-after.png`: the paths. */
export function namedCrops(beforePng, afterPng, names, dir, shot) {
  const differing = names.filter(n => n.differ > 0);
  if (!differing.length) return [];
  mkdirSync(dir, { recursive: true });
  const A = decodePng(readFileSync(beforePng)), B = decodePng(readFileSync(afterPng));
  const file = s => s.replace(/[^\w.-]+/g, '-');
  return differing.flatMap(n => [['before', A], ['after', B]].map(([side, img]) => {
    const [x, y, w, h] = n.rect;
    const path = join(dir, `${file(shot)}-${file(n.target)}-${side}.png`);
    writeFileSync(path, encodePng(crop(img, x, y, w, h)));
    return path;
  }));
}

/** Each named entry against every comparison's names: its total differing
 * pixels, the shots it differs on, and STALE where it differs on none (a
 * name claiming a change nobody made). An entry no shot showed is MISSING. */
export function namedReport(named, compared) {
  return named.map(n => {
    const hits = compared.flatMap(c => (c.names ?? []).filter(m => m.target === n.target && (n.screen === '*' || c.shot.endsWith(` ${n.screen}`))).map(m => ({ shot: c.shot, ...m })));
    const differ = hits.reduce((t, h) => t + h.differ, 0);
    return { ...n, shots: hits.filter(h => h.differ > 0), seen: hits.length, differ, status: !hits.length ? 'MISSING' : differ ? 'NAMED' : 'STALE' };
  });
}

/** Theme2's option B check on a copy's source map: each binding component
 * draws nothing of its own (no view node declared in it) and instantiates
 * only the package components it binds to (the `if` around FollowButton is
 * a region, no node). Each binding must be instantiated at least once.
 * [{binding, ok, said}]. */
export function bindingsCheck(map, bindings) {
  return bindings.map(({ component, allowed }) => {
    const own = (map.nodes ?? []).filter(n => n.component === component);
    const placed = (map.nodes ?? []).filter(n => n.chain?.[0]?.component === component);
    const others = [...new Set(placed.filter(n => !allowed.includes(n.library)).map(n => n.library ?? `${n.component} (the app's)`))];
    const said = !placed.length ? 'not instantiated' : own.length ? `draws ${own.length} node(s) of its own (${[...new Set(own.map(n => `${n.file.split('/').pop()}:${n.line}`))].slice(0, 4).join(', ')})` : others.length ? `instantiates ${others.join(', ')} besides ${allowed.map(a => a.split('#')[1]).join(' and ')}` : `binds to ${[...new Set(placed.map(n => n.library.split('#')[1]))].join(' and ')} only`;
    return { binding: component, ok: !!placed.length && !own.length && !others.length, said };
  });
}
