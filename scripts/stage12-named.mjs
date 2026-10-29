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
      // A named box takes every pixel that differs in it, the web's edge noise too (theme2 23:16 Q2: no rule inside
      // a named box; its crops are read instead); outside, the floor applies as before.
      const n = names.find(m => within(m.rect, x, y));
      if (!n) {
        if (!(noise && d <= noise.floor && edgeAt(A, x, y, noise.edge))) differ++;
        continue;
      }
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

/** Before's and after's crops of each named box, written to
 * `dir` as `<shot>-<target>-before.png` and `-after.png`: the paths. */
export function namedCrops(beforePng, afterPng, names, dir, shot) {
  // Every named box on every shot, differing or not (theme2 23:16 Q2: crops of every named box on every host).
  const differing = names.filter(n => n.rect);
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

/** The library identity a source map file and component name: `@exact/<package>/<file>.contract#<Component>` for a
 * package's file (in a checkout's packages/ or a copy's node_modules/@exact/), else null (the app's own). */
export function identityOf(file, component) {
  const m = /\/(?:packages|node_modules\/@exact)\/([\w-]+)\/([\w-]+)\.contract$/.exec(file ?? '');
  return m ? `@exact/${m[1]}/${m[2]}.contract#${String(component).replace(new RegExp(`__${m[2].replace(/\W/g, '_')}$`), '')}` : null;
}

/** Theme2's option B check on a copy's source map: each binding component draws nothing of its own, and every node
 * drawn under it comes through what it binds to, all the way down: in each node's chain of call sites, the ones
 * inside the binding's call (inner to it) are all in the allowed packages' files, and what the binding itself
 * instantiates is an allowed component (an app helper between them, drawing its own node, fails). Each binding must
 * be instantiated. [{binding, ok, said}]. The body's shape (one PersonRow, at most the `if` around FollowButton in
 * its slot) is `bindingShape`'s, on the source. */
export function bindingsCheck(map, bindings) {
  return bindings.map(({ component, allowed }) => {
    const nodes = map.nodes ?? [];
    const own = nodes.filter(n => n.component === component);
    const under = nodes.flatMap(n => {
      const k = (n.chain ?? []).findIndex(c => c.component === component);
      if (k < 0) return [];
      const inner = n.chain.slice(0, k);
      // What the binding's call at chain[k] instantiates: the node's own component (k 0) or chain[k - 1]'s.
      const direct = k === 0 ? (n.library ?? identityOf(n.file, n.component)) : identityOf(inner[k - 1].file, inner[k - 1].component);
      const app = inner.find(c => !identityOf(c.file, c.component)) ?? (!n.library && !identityOf(n.file, n.component) ? n : null);
      return [{ n, direct, app }];
    });
    const strays = [...new Set(under.filter(u => !allowed.includes(u.direct)).map(u => u.direct ?? `${u.n.component} (the app's)`))];
    const helpers = [...new Set(under.filter(u => u.app).map(u => `${u.app.component} (${String(u.app.file).split('/').pop()}:${u.app.line})`))];
    const where = list => list.slice(0, 4).join(', ');
    const said = !under.length ? 'not instantiated'
      : own.length ? `draws ${own.length} node(s) of its own (${where([...new Set(own.map(n => `${n.file.split('/').pop()}:${n.line}`))])})`
      : helpers.length ? `draws through the app's own ${where(helpers)}`
      : strays.length ? `instantiates ${strays.join(', ')} besides ${allowed.map(a => a.split('#')[1]).join(' and ')}`
      : `binds to ${[...new Set(under.map(u => u.direct.split('#')[1]))].join(' and ')} only`;
    return { binding: component, ok: !!under.length && !own.length && !helpers.length && !strays.length, said };
  });
}

/** Option B's body shape, on the source map (formatting-proof; theme2's 18:40 terms: an `if` is control, not
 * drawing, and any other NODE fails): the binding calls the package PersonRow from exactly one call site, the
 * package FollowButton from at most one, and every FollowButton it draws renders in that PersonRow call's slot (its
 * walk's first step is the PersonRow call, `slot: true`). With bindingsCheck (nothing drawn of its own or through
 * an app helper), that is one PersonRow and in its slot at most one FollowButton, whatever control wraps either:
 * control draws nothing, and the source's words aren't read (a scan of them can't follow Contract's syntax).
 * {ok, said}. */
export function bindingShape(map, { component, allowed }) {
  const [rowId, followId] = allowed;
  const nodes = map.nodes ?? [], steps = map.walk_steps ?? [], walks = map.walks ?? [];
  const site = c => `${c.file}:${c.line}:${c.col}`;
  const direct = id => nodes.filter(n => n.chain?.[0]?.component === component && n.library === id);
  const rows = [...new Set(direct(rowId).map(n => site(n.chain[0])))], follows = direct(followId);
  const followSites = [...new Set(follows.map(n => site(n.chain[0])))];
  if (rows.length !== 1) return { ok: false, said: `calls PersonRow from ${rows.length} sites, not one` };
  if (followSites.length > 1) return { ok: false, said: `calls FollowButton from ${followSites.length} sites, not at most one` };
  const outside = follows.filter(n => { const first = steps[walks[n.walk]?.[0]]; return !(first?.slot && first.component === rowId.split('#')[1] && site(first) === rows[0]); });
  if (outside.length) return { ok: false, said: `draws a FollowButton outside its PersonRow's slot (${outside.length} node(s))` };
  // And the slot holds nothing else: no node renders there but that FollowButton, so content forwarded through a
  // slot of the binding's own (its caller's nodes, whose chains don't name it) fails too (Astra's re-check).
  const inSlot = nodes.filter(n => { const first = steps[walks[n.walk]?.[0]]; return first?.slot && site(first) === rows[0]; });
  const strangers = inSlot.filter(n => !follows.includes(n));
  if (strangers.length) return { ok: false, said: `its PersonRow's slot holds ${[...new Set(strangers.map(n => n.library ?? `${n.component} (the app's)`))].join(', ')}` };
  return { ok: true, said: `one PersonRow call${followSites.length ? ', its one FollowButton call in its slot' : ''}` };
}

/** The compiler's own parse of a component: `contract lean <app> --components` prints every component as a Lean
 * record (`name := "B", …, slot := false, …, view := [(.use 0 "PersonRow" [args] [children]), …]`). This reads the
 * record named `component` from that text into {slot, view}, the view a tree of [kind, …] lists: `use` [name,
 * children], `when` (an if) [then, else], `element` [tag, children], anything else [kind, children…]. Null where the
 * text has no such record. */
export function componentIR(lean, component) {
  const at = lean.indexOf(`name := ${JSON.stringify(component)}, props :=`);
  if (at < 0) return null;
  // The record's own text: to the brace that closes the one before its name.
  let open = lean.lastIndexOf('{', at), depth = 0, end = open;
  for (let i = open, str = false; i < lean.length; i++) {
    const c = lean[i];
    if (str) { if (c === '\\') i++; else if (c === '"') str = false; continue; }
    if (c === '"') str = true; else if (c === '{' || c === '[' || c === '(') depth++; else if (c === '}' || c === ']' || c === ')') { if (--depth === 0) { end = i; break; } }
  }
  const rec = lean.slice(open, end + 1);
  const slot = /\bslot := (true|false)\b/.exec(rec)?.[1] === 'true';
  const v = rec.indexOf('view := ');
  if (v < 0) return { slot, view: null };
  // A small reader: lists `[…]`, tuples `(…)`, strings, words; commas separate.
  let i = v + 'view := '.length;
  const read = () => {
    while (/\s|,/.test(rec[i])) i++;
    const c = rec[i];
    if (c === '[' || c === '(') {
      const close = c === '[' ? ']' : ')', items = [];
      i++;
      for (;;) { while (/\s|,/.test(rec[i])) i++; if (rec[i] === close) { i++; break; } items.push(read()); }
      return c === '[' ? { list: items } : { tuple: items };
    }
    if (c === '"') { let j = i + 1; while (rec[j] !== '"') j += rec[j] === '\\' ? 2 : 1; const t = JSON.parse(rec.slice(i, j + 1)); i = j + 1; return t; }
    const m = /^[^\s,()\[\]]+/.exec(rec.slice(i)); i += m[0].length; return { word: m[0] };
  };
  const tree = read();
  // A view entry: (.use n "Name" [args] [children]), (.when n cond [then] [else]), (.element "tag" … [children]).
  const node = t => {
    const [head, ...rest] = t.tuple ?? [];
    const kind = head?.word?.replace(/^\./, '') ?? '?';
    const lists = rest.filter(x => x.list);
    if (kind === 'use') return { kind, name: rest.find(x => typeof x === 'string'), children: (lists.at(-1)?.list ?? []).map(node) };
    if (kind === 'when') return { kind, then: (lists.at(-2)?.list ?? []).map(node), else: (lists.at(-1)?.list ?? []).map(node) };
    if (kind === 'element') return { kind, tag: rest.find(x => typeof x === 'string'), children: (lists.at(-1)?.list ?? []).map(node) };
    return { kind, children: lists.flatMap(l => l.list).filter(x => x.tuple).map(node) };
  };
  return { slot, view: (tree.list ?? []).map(node) };
}

/** Option B's shape on the compiler's parse (componentIR): no slot of its own (a slot would let its caller's nodes
 * sit beside the row, naming it nowhere); its view exactly one use of `row`; under it nothing, or one `when` (an if)
 * with no else whose only child is one use of `follow`, itself with nothing under it. {ok, said}. */
export function irShape(ir, row = 'PersonRow', follow = 'FollowButton') {
  if (!ir) return { ok: false, said: 'the compiler has no such component' };
  if (ir.slot) return { ok: false, said: 'declares a slot of its own' };
  const view = ir.view ?? [];
  const say = n => (n.kind === 'use' ? `${n.name}` : n.kind === 'element' ? `a ${n.tag}` : `a ${n.kind}`);
  if (view.length !== 1 || view[0].kind !== 'use' || view[0].name !== row) return { ok: false, said: `its view is ${view.map(say).join(', ') || 'empty'}, not one ${row}` };
  const under = view[0].children;
  if (!under.length) return { ok: true, said: `one ${row}, its slot empty` };
  const [w] = under;
  if (under.length !== 1 || w.kind !== 'when' || w.else.length || w.then.length !== 1 || w.then[0].kind !== 'use' || w.then[0].name !== follow || w.then[0].children.length)
    return { ok: false, said: `in its ${row}'s slot: ${under.map(n => (n.kind === 'when' ? `an if of ${n.then.map(say).join(', ') || 'nothing'}${n.else.length ? ` else ${n.else.map(say).join(', ')}` : ''}` : say(n))).join(', ')}, not the if around one ${follow}` };
  return { ok: true, said: `one ${row}, and in its slot the if around one ${follow}` };
}
