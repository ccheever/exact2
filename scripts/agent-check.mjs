// `check` (LLP 1116.003 D8): what is wrong with the presented screen, in text, so a builder need not read a
// screenshot back to find it — text clipped against its box, controls overlapping or covered by a bar, the keyboard
// or a sheet, targets under the platform's minimum, horizontal overflow, text below a contrast floor, and a card
// whose fill is what lies behind it. The host observes (host/web-js/check.js, AgentCheckIOS.swift: its presented
// boxes, ink, fills, clips and hit tests, and the fill painted under each text, LLP 1116.006 D5); where no fill is
// known, the background a text stands on is read from a capture's pixels; this file only judges. `check dark` (or `light`) runs it under that `prefers-color-scheme`, `check both` under each.
import { MalformedOperation } from './agent-keys.mjs';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { decodePng, encodePng, shrink } from './png.mjs';

const SHOWN = 12;
const OFFSCREEN = 'a column taller than the screen runs past it: let it scroll (`overflow-y: auto`), or give it `flex: 1` and `min-height: 0` under a header instead of `height: 100%`';
/** The platform colours by every name a host may report (kernel/tables/schema.json `colors`): row name → its light and dark
 *  values in each of the table's columns, Apple's and the neutral one (LLP 1115.000 D3.2). */
const ROLES = (() => {
  const rows = JSON.parse(readFileSync(fileURLToPath(new URL('../kernel/tables/schema.json', import.meta.url)), 'utf8')).colors ?? [];
  const byName = new Map();
  for (const [name, webkit, ios, mac, light, dark, neutralLight, neutralDark] of rows) for (const n of [name, webkit, ios, mac, ios?.replace(/Color$/, '')]) if (n && !byName.has(n)) byName.set(n, { name, apple: { light, dark }, neutral: { light: neutralLight, dark: neutralDark } });
  return byName;
})();
/** Web observations name the neutral column; native Apple observations leave the column unsaid. */
const columnOf = (obs) => obs.look === 'neutral' ? 'neutral' : 'apple';
const role = (raw) => (raw && ROLES.get(String(raw).replace(/^-exact-/, ''))?.name) ?? null;
const hex = (c) => '#' + c.slice(0, 3).map((v) => Math.round(v).toString(16).padStart(2, '0')).join('');
const lum = (c) => { const [r, g, b] = c.slice(0, 3).map((v) => { v /= 255; return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; }); return 0.2126 * r + 0.7152 * g + 0.0722 * b; };
export const ratio = (a, b) => { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p); return (x + 0.05) / (y + 0.05); };
const over = (fg, bg) => { const a = fg[3] ?? 1; return [0, 1, 2].map((i) => fg[i] * a + bg[i] * (1 - a)); };
const near = (a, b, d) => a && b && [0, 1, 2].every((i) => Math.abs(a[i] - b[i]) <= d);
const r0 = (n) => Math.round(n);

/** A pixel of the capture at a viewport point, or null off it. */
function pixel(png, scale, x, y) {
  const px = Math.floor(x * scale), py = Math.floor(y * scale);
  if (px < 0 || py < 0 || px >= png.width || py >= png.height) return null;
  const i = (py * png.width + px) * 4;
  return [png.data[i], png.data[i + 1], png.data[i + 2]];
}

/** What a text stands on: the commonest of its box's four corners (a glyph rarely reaches one), or null where they disagree (an image). */
function backdrop(png, scale, it) {
  const inset = Math.min(1.5, it.w / 4, it.h / 4);
  const corners = [[it.x + inset, it.y + inset], [it.x + it.w - inset, it.y + inset], [it.x + inset, it.y + it.h - inset], [it.x + it.w - inset, it.y + it.h - inset]]
    .map(([x, y]) => pixel(png, scale, x, y)).filter(Boolean);
  let best = null, count = 0;
  for (const c of corners) { const n = corners.filter((d) => near(c, d, 12)).length; if (n > count || (n === count && best && ratio(c, it.fg) > ratio(best, it.fg))) { best = c; count = n; } }
  return count >= 2 ? best : null;
}

/** The contrast a text needs (WCAG 1.4.3, and LLP 1115's platform answers): 4.5:1; 3:1 for large text (24 px, or 18.66 px bold), for a
 *  control's label on its own fill (D3's ink rule), for ink the platform chose (a tab bar's item) and for text in a platform colour (the
 *  secondary label, system red: the platform's palette, as a hand-built app draws it). The platform's tertiary and quaternary labels and its placeholders are its own and not judged,
 *  nor is a disabled control's label (WCAG's inactive component). */
function floor(it, scheme, column) {
  if (it.inactive) return null;
  const r = role(it.fgRole) ?? drawnRole(it.fg, scheme, column);
  if (r && /^(tertiary|quaternary)-label$|^placeholder$|^GrayText$/.test(r)) return null;
  if (it.size >= 24 || (it.size >= 18.66 && it.weight >= 700) || it.label || it.platformInk || (r && /^secondary-label$|^system-|^link$|^LinkText$|^AccentColor$|^ButtonText$/.test(r))) return 3;
  return 4.5;
}
/** The platform role an ink is, by its value in this scheme and the host's column: UIKit's own labels carry no name, nor
 *  does a text whose colour is inherited or the grouped list's sheet's (a section footer's secondary label on the web).
 *  Never `AccentColorText`, which is an ink only on the accent (the host says where it chose one, `platformInk`): by its
 *  value alone it is the neutral column's black in dark, and an author's black text there is not the platform's. */
function drawnRole(fg, scheme = 'light', column = 'apple') {
  if (!fg) return null;
  for (const r of new Set(ROLES.values())) {
    const h = String(r[column][scheme] ?? ''), n = parseHex(h), a = h.length === 9 ? parseInt(h.slice(7), 16) / 255 : 1;
    if (r.name !== 'AccentColorText' && h.startsWith('#') && near(fg, n, 2) && Math.abs((fg[3] ?? 1) - a) < 0.03) return r.name;
  }
  return null;
}

const inter = (a, b) => ({ w: Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x), h: Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y) });
/** What is presented of an item's box: the host's `vis` (a clipping view, or a bar drawn over it, cuts it), less what the
 * keyboard (its top, `kb`) covers; and whether none is (LLP 1116.007 D3). */
const shown = (it, kb) => { const v = it.vis ? { ...it, ...it.vis } : it; return kb != null && v.y + v.h > kb ? { ...v, h: Math.max(0, kb - v.y) } : v; };
const covers = (it, kb) => { const v = shown(it, kb); return v.w <= 0 || v.h <= 0; };
const holds = (a, b) => a.x <= b.x + 0.5 && a.y <= b.y + 0.5 && a.x + a.w >= b.x + b.w - 0.5 && a.y + a.h >= b.y + b.h - 0.5;

/** The findings for one observation (`obs`, the host's reply) and its capture (`png`, decoded; null skips what needs pixels). */
export function judge(obs, png, { host = 'web' } = {}) {
  const vp = obs.viewport, scale = png ? png.width / (obs.capture?.w ?? vp.w) : 1, items = obs.items ?? [], found = [], column = columnOf(obs);
  const add = (kind, it, detail, hint) => found.push({ kind, id: it.id ?? null, ...(it.of != null ? { of: it.of } : {}), name: it.name, text: it.text, box: [r0(it.x), r0(it.y), r0(it.w), r0(it.h)], detail, ...(hint ? { hint } : {}) });
  const unit = host === 'web' ? 'px' : 'pt';
  for (const it of items) {
    // Text squeezed to no width (iOS reports it): nothing of it shows, and nothing else of it can be judged.
    if (it.squeezed) { add('squeezed', it, {}, 'let it keep its words (drop `min-width: 0` or a fixed width beside it), or let the row wrap (`flex-wrap="wrap"`)'); continue; }
    // Covered: something else is over its middle at rest, and no scroller moves it clear.
    if (it.cover && !it.disabled) {
      const what = it.cover.kind === 'keyboard' ? 'the software keyboard' : it.cover.kind === 'sheet' ? 'a sheet' : it.cover.by != null ? { ref: it.cover.by } : it.cover.name;
      add('covered', it, { under: what, bar: it.cover.kind === 'bar' ? it.cover.name : undefined, offscreen: it.cover.kind === 'offscreen' },
        it.cover.kind === 'offscreen' ? OFFSCREEN : it.cover.kind === 'keyboard' ? 'scroll it above the keyboard, or move it before the field' : it.text && !it.control ? 'start the content below what covers it (padding or a divider), or let the bar take its place' : 'move it clear, or give the scroller room at that end');
    }
    // Text that does not fit its box (beyond a clamp the author asked for), or an ancestor's clip cutting it.
    if (it.text && it.over && !it.clamp && (it.over[0] >= 2 || it.over[1] >= 2)) {
      add('clipped', it, { need: it.over.map(r0), clips: !!it.clips, unit }, it.clips ? 'let it wrap, give its box room, or ask for an ellipsis (`text-overflow: ellipsis` or `line-clamp`)' : 'its box is smaller than its text, which spills over what follows: drop the fixed size, or ask for `line-clamp`');
    } else if (it.text && it.cut) add('clipped', it, { cut: [r0(it.cut.dx), r0(it.cut.dy)], by: it.cut.by, edge: it.cut.edge, unit }, it.cut.edge ? OFFSCREEN : 'the clipping box is smaller than its content: let it grow, or scroll');
    // LLP 1116.009 D5: a written percentage width and the padding it does not include, past what its parent gives it: the cause, named.
    if (it.padded) add('overflow', it, { past: r0(it.padded.dx), of: it.padded.by, pad: r0(it.padded.pad), written: it.padded.width, unit }, 'drop the width (a block or a stretched flex item fills its parent) or say `box-sizing="border-box"`');
    else if (it.outX) add('overflow', it, { past: r0(it.outX), width: vp.w, unit }, 'let it wrap or shrink (`min-width: 0`, `flex-wrap: wrap`, `max-width: 100%`), or scroll it inside its own box');
    else if (it.spill) add('overflow', it, { past: r0(it.spill.dx), of: it.spill.by, unit }, 'a flex item\'s width stops at its content: let it shrink (`min-width: 0`, a smaller `padding`), or let the row wrap (`flex-wrap: wrap`) or scroll');
    // A target the author sized under the platform's minimum (WCAG 2.5.8's 24 px, with its spacing exception, on the web; 44 pt on iOS).
    if (it.control && it.sized === 'author' && !it.inline && !it.disabled && !covers(it, obs.keyboard) && (it.w < obs.target - 0.5 || it.h < obs.target - 0.5)) {
      const spaced = host === 'web' && items.every((o) => o === it || !o.control || o.inline || !o.w ||
        Math.hypot((o.x + o.w / 2) - (it.x + it.w / 2), (o.y + o.h / 2) - (it.y + it.h / 2)) >= obs.target);
      if (!spaced) add('target', it, { min: obs.target, unit }, `make its box at least ${obs.target}×${obs.target} (padding, or \`min-height\`/\`min-width\`)`);
    }
    // Contrast against what it stands on: the fill the host found painted under it (LLP 1116.006 D5), else what the
    // pixels show behind it, read where it is presented (a bar's glass over it is not what it stands on, LLP 1116.007 D3.8).
    if (it.text && it.fg && (it.bg || png) && !covers(it, obs.keyboard)) {
      const need = floor(it, obs.scheme, column), bg = need && (it.bg ?? backdrop(png, scale, shown(it, obs.keyboard)));
      if (bg) {
        const ink = over(it.fg, bg), got = ratio(ink, bg);
        if (got < need - 0.005) add('contrast', it, { got: Math.floor(got * 10) / 10, need, fg: hex(ink), bg: hex(bg), ...(it.bg ? { on: it.on ?? 'page' } : {}) },
          it.onAccent ? 'the platform draws a filled button\'s label white on the accent: leave the accent unsaid (the platform\'s blue; a brand is an `app.tamber`)'
            : it.label ? 'name a `color` for the label, or a fill it reads on in this scheme' : role(it.fgRole) ? `-exact-${role(it.fgRole)} is too light for text on this: use -exact-label or -exact-secondary-label, or keep the colour for a fill or a symbol` : 'use a role (`-exact-label`, `-exact-secondary-label`) or a colour that changes with the scheme');
      }
    }
    // A card whose fill is what lies behind it, with no border or shadow: invisible.
    // Only where it is presented: under the keyboard the capture shows the keyboard, not the card's edge (unit-converter, round 9).
    if (it.fill && !it.edge && png && !covers(it, obs.keyboard)) {
      const v = shown(it, obs.keyboard), on = (x, y) => x >= v.x - 2.5 && x <= v.x + v.w + 2.5 && y >= v.y - 2.5 && y <= v.y + v.h + 2.5;
      const sides = [[it.x + 2, it.y + it.h / 2, it.x - 2, it.y + it.h / 2], [it.x + it.w - 2, it.y + it.h / 2, it.x + it.w + 2, it.y + it.h / 2],
        [it.x + it.w / 2, it.y + 2, it.x + it.w / 2, it.y - 2], [it.x + it.w / 2, it.y + it.h - 2, it.x + it.w / 2, it.y + it.h + 2]]
        .filter(([ix, iy, ox, oy]) => on(ix, iy) && on(ox, oy))
        .map(([ix, iy, ox, oy]) => [pixel(png, scale, ix, iy), pixel(png, scale, ox, oy)]).filter(([a, b]) => a && b);
      if (sides.length >= 2 && sides.every(([a, b]) => near(a, b, 4))) {
        const fill = role(it.fillRole), behind = sides[0][1], scheme = obs.scheme ?? 'light';
        const plain = [...ROLES.values()].some((r) => r.name === 'background' && near(behind, parseHex(r[column][scheme]), 3));
        add('card', it, { fill: fill ? `-exact-${fill}` : hex(it.fill), behind: hex(behind) },
          fill === 'secondary-grouped-background' && plain ? 'on a plain page a card is -exact-secondary-background (-exact-secondary-grouped-background is for a -exact-grouped-background page)'
            : 'give it a fill that differs from the page (-exact-secondary-background on a plain page), a border or a shadow');
      }
    }
  }
  // Controls overlapping each other, or a text: neither holds the other and they share more than a sliver of what is
  // presented of each (`vis`: a row its list or scroller cuts overlaps nothing past that edge, LLP 1116.006 D5).
  const controls = items.filter((it) => it.control && !it.inline);
  for (const a of controls) for (const b of items) {
    if (a === b || !(b.control || b.text) || (b.control && b.id < a.id && controls.includes(b))) continue;
    if (!b.control && (b.of === a.id || controls.some((c) => c !== a && holds(c, b)))) continue; // its own label, or another control's
    const i = inter(shown(a, obs.keyboard), shown(b, obs.keyboard));
    if (i.w >= 2 && i.h >= 2 && !holds(a, b) && !holds(b, a)) add('overlap', a, { with: b.id, by: [r0(i.w), r0(i.h)] }, 'give them room (a gap, or let the row wrap)');
  }
  // A control's own text covered with it is one finding: the control's.
  const coveredControls = found.filter((f) => f.kind === 'covered' && items.find((it) => it.id === f.id)?.control);
  const kept = found.filter((f) => !(f.kind === 'covered' && f.text != null && coveredControls.some((c) => c !== f && holds({ x: c.box[0], y: c.box[1], w: c.box[2], h: c.box[3] }, { x: f.box[0], y: f.box[1], w: f.box[2], h: f.box[3] }))));
  const order = ['covered', 'squeezed', 'clipped', 'overflow', 'overlap', 'contrast', 'card', 'target'];
  return kept.sort((p, q) => order.indexOf(p.kind) - order.indexOf(q.kind) || p.box[1] - q.box[1] || p.box[0] - q.box[0]);
}
function parseHex(h) { const n = parseInt(String(h).slice(1, 7), 16); return [n >> 16 & 255, n >> 8 & 255, n & 255]; }

/** A node as a builder names it: its testId, else its type and an excerpt of its text, else its id (`tree`'s nodes by id). */
function named(id, nodes, fallback) {
  const n = id == null ? null : nodes.get(id);
  if (!n) return fallback ?? (id == null ? 'a view' : `#${id}`);
  if (n.props?.testId != null) return /\s/.test(n.props.testId) ? JSON.stringify(n.props.testId) : n.props.testId;
  const text = n.props?.text ?? n.props?.accessibilityLabel;
  return `${n.type}${text ? ' ' + JSON.stringify(text.length > 24 ? text.slice(0, 23) + '…' : text) : ''}#${id}`;
}

/** Who a finding is about: a control's label by its control, a native view by its kind and text. */
const who = (f, nodes) => f.of != null && f.text != null ? `${named(f.of, nodes)}'s label ${JSON.stringify(f.text)}` : named(f.id, nodes, f.name ? f.name + (f.text ? ` ${JSON.stringify(f.text)}` : '') : null);

/** The entry of app.json's `verify.accept` that takes a finding as the app's design, or undefined: its kind, and the
 *  testId of its node or of the control whose label it is (`node`; a non-empty prefix and one trailing `*` match a prefix:
 *  a bare `*`, which the schema refuses too, matches nothing, so no entry accepts a kind everywhere). */
export function accepts(entries, f, nodes) {
  const ids = [f.id, f.of].map((id) => (id == null ? null : nodes.get(id)?.props?.testId)).filter((id) => id != null).map(String);
  const takes = (node, id) => (/^[^*]+\*$/.test(node) ? id.startsWith(node.slice(0, -1)) : !node.includes('*') && id === node);
  return entries.find((e) => e.finding === f.kind && ids.some((id) => takes(String(e.node), id)));
}

/** One line a finding. */
export function line(f, nodes) {
  const box = `[${f.box[0]},${f.box[1]} ${f.box[2]}×${f.box[3]}]`, d = f.detail;
  const under = (u) => typeof u === 'object' && u ? named(u.ref, nodes) : u;
  const what = {
    covered: () => d.offscreen ? `its middle is past ${d.under}, and nothing scrolls to it` : `its middle is under ${under(d.under)} at rest${d.bar ? ` (${d.bar})` : ''}, and nothing scrolls it clear`,
    clipped: () => d.need ? `its text is ${[d.need[0] ? `${d.need[0]} ${d.unit} wider` : '', d.need[1] ? `${d.need[1]} ${d.unit} taller` : ''].filter(Boolean).join(' and ')} than its box${d.clips ? ', which clips it' : ', and spills'}` : `${d.edge ? `the ${d.edge} of the screen` : named(d.by, nodes)} cuts ${[d.cut[0] ? `${d.cut[0]} ${d.unit} of its width` : '', d.cut[1] ? `${d.cut[1]} ${d.unit} of its height` : ''].filter(Boolean).join(' and ')}`,
    overflow: () => d.pad != null ? `runs ${d.past} ${d.unit} past ${named(d.of, nodes)}'s side: ${d.written != null ? `its width is ${d.written} of its parent's` : 'its content is as wide as its parent\'s'}, and its ${d.pad} ${d.unit} of padding${d.written != null ? ' and border' : ''} add to that (content-box sizing)` : d.of != null ? `runs ${d.past} ${d.unit} past ${named(d.of, nodes)}'s side, and nothing there scrolls or clips it` : `runs ${d.past} ${d.unit} past the ${d.width}-wide viewport's side, and nothing scrolls it`,
    target: () => `${f.box[2]}×${f.box[3]} ${d.unit}, under the ${d.min}×${d.min} minimum`,
    contrast: () => `${d.got}:1 (${d.fg} on ${d.bg}${d.on == null ? '' : d.on === 'own' ? ', its own fill' : d.on === 'page' ? ', the page' : `, ${named(d.on, nodes)}'s fill`}), needs ${d.need}:1`,
    card: () => `its fill ${d.fill} is what lies behind it (${d.behind}): it does not show`,
    overlap: () => `overlaps ${named(d.with, nodes)} by ${d.by[0]}×${d.by[1]}`,
    squeezed: () => 'its box is squeezed to no width, so none of its text shows',
  }[f.kind]();
  return `  ${f.kind.padEnd(8)} ${who(f, nodes)} ${box}: ${what}${f.hint ? `; ${f.hint}` : ''}`;
}

/** An advisory line (LLP 1116.004 D1.3): a phone's layout at a desktop width, which a composed page may mean. Never a finding. */
function advise(a, width, nodes) {
  const who = named(a.id, nodes);
  return `  advice   ${who}: ${a.kind === 'tabbar'
    ? `a bottom tab bar ${r0(a.w)} px wide along a ${width}-px window; placed as docs/recipes/tabs.md places it (the last child of the root's flex column, unpositioned) a wide window draws it as a sidebar, or lay out a rail of your own when the window is wide`
    : `the list beneath pushed this detail over the whole ${width}-px window where they would fit side by side; a row that pushes draws the disclosure (\`image "symbol:forward-chevron"\`, last in the row) and a wide window shows the list beside its detail, or set \`navigationSplit\` on the list's route (contract vocab navigationSplit)`}`;
}

/** The text a check prints: per scheme, its findings (at most SHOWN, then a count), or one line saying there are none; then its advice. */
export function report(runs, trees, host) {
  const out = [];
  for (const [i, run] of runs.entries()) {
    const nodes = trees[i];
    const head = `check ${host} · ${run.scheme}${run.asked && run.asked !== run.scheme ? ` (asked ${run.asked}: the app keeps its own scheme)` : ''}${run.accepted?.length ? ` (${run.accepted.length} accepted)` : ''}`;
    if (!run.findings.length) out.push(`${head}: no findings (text fits, nothing covered or overlapping, targets ≥ ${run.target}, no horizontal overflow, contrast ≥ 4.5:1 or 3:1 large, cards show)`);
    else {
      out.push(`${head}: ${run.findings.length} finding${run.findings.length === 1 ? '' : 's'}`);
      for (const f of run.findings.slice(0, SHOWN)) out.push(line(f, nodes));
      if (run.findings.length > SHOWN) out.push(`  … and ${run.findings.length - SHOWN} more (--json lists them all)`);
    }
    for (const a of run.advice ?? []) out.push(advise(a, run.viewport?.w, nodes));
  }
  return out.join('\n');
}

/** `check [light|dark|both]` on a session: the host's observation and a capture under each scheme asked, judged; the scheme is put back.
 *  `shots(viewport, scheme)` names a file to keep each capture in (verify's), else they go with the check. A finding an
 *  entry of `accept` (app.json `verify.accept`) takes is the run's `accepted`, each with its line and the entry, not a finding. */
export async function checkOp(s, args = [], { shots, accept = [] } = {}) {
  const word = args[0] ?? 'current';
  if (!['current', 'light', 'dark', 'both'].includes(word) || args.length > 1) throw new MalformedOperation('check [light|dark|both]: the presented screen, judged in text; `check dark` under prefers-color-scheme dark, `check both` under each');
  if (!['web', 'ios', 'host-ios'].includes(s.host)) throw new Error(`check: the web and iOS hosts carry it, not ${s.host}`);
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-check-'));
  const first = await s.op({ op: 'check' });
  const asked = word === 'both' ? ['light', 'dark'] : word === 'current' ? [first.scheme] : [word];
  const runs = [], trees = [];
  let preferred = null;
  try {
    for (const scheme of asked) {
      let obs = first;
      if (scheme !== first.scheme || preferred) { await s.prefer({ 'prefers-color-scheme': scheme }); preferred = scheme; obs = await s.op({ op: 'check' }); }
      const path = shots?.(obs.viewport, scheme) ?? resolve(dir, `${scheme}.png`);
      const taken = await s.carrier.screenshot(path, obs.capture?.window === true); // iOS observes in the window's points: a sheet is not in the viewport's picture
      const png = decodePng(readFileSync(path));
      runs.push({ scheme: obs.scheme, asked: scheme, target: obs.target, viewport: obs.viewport, findings: judge(obs, png, { host: s.host === 'web' ? 'web' : 'ios' }), ...(obs.advice ? { advice: obs.advice } : {}) });
      // The tree as this scheme shows it: a view only one scheme shows is named, and accepted, by its own testId.
      trees.push(new Map(((await s.op({ op: 'tree' })).nodes ?? []).map((n) => [n.id, n])));
      // A capture kept for a person or an agent to look at is a pixel a point once judged, as `agent`'s are: a third of an iPhone's pixels each way.
      if (shots && taken?.scale > 1) writeFileSync(path, encodePng(shrink(png, Math.round(png.width / taken.scale), Math.round(png.height / taken.scale))));
    }
  } finally {
    if (preferred && preferred !== first.scheme) await s.prefer({ 'prefers-color-scheme': first.scheme }).catch(() => {});
    rmSync(dir, { recursive: true, force: true });
  }
  return s.tagged(settle(runs, trees, accept, s.host));
}

/** The runs judged, each against the tree its own scheme showed (`trees[i]`): what `accept` takes is the run's `accepted`, each
 *  with its line and the entry; every finding is named. `{check, text}`, as `check` replies. */
export function settle(runs, trees, accept, host) {
  for (const [n, r] of runs.entries()) {
    const nodes = trees[n], taken = r.findings.map((f) => accepts(accept, f, nodes));
    r.accepted = r.findings.flatMap((f, i) => (taken[i] ? [{ ...f, line: `${line({ ...f, hint: undefined }, nodes)}; accepted: ${taken[i].why}`, accept: taken[i] }] : []));
    r.findings = r.findings.filter((_, i) => !taken[i]);
  }
  const named = (fs, nodes) => fs.map((f) => ({ ...f, node: who(f, nodes) }));
  return { check: runs.map((r, i) => ({ ...r, findings: named(r.findings, trees[i]), accepted: named(r.accepted, trees[i]) })), text: report(runs, trees, host) };
}
