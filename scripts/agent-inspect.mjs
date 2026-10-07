import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, existsSync, mkdirSync, readFileSync, statSync } from 'node:fs';
import { resolve } from 'node:path';
import { renderAx } from './agent-ax.mjs';

/** @ref LLP 1035.005 D3 / 1035.002 D6 — only the driver reads source maps.
 * The locator discovers candidates; the node's same-reply digest decides whether
 * one is compatible. Identical plans can have different formatting/ranges, so
 * compatibility does not establish the original authored source revision. */
export function sourceMapReader(locator) {
  const maps = new Map(), limit = 64 * 1024 * 1024;
  const hash = bytes => createHash('sha256').update(bytes).digest('hex');
  const digest = value => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
  let lastCard = null, problem = 'no development source map';
  const remote = typeof locator === 'string' && /^https?:\/\//i.test(locator);
  async function fetchBytes(url, maximum = limit) {
    const response = await fetch(url, { signal: AbortSignal.timeout(3000), redirect: 'error', cache: 'no-store' });
    if (!response.ok) throw Error(`source map HTTP ${response.status}`);
    if (Number(response.headers.get('content-length')) > maximum) { await response.body?.cancel(); throw Error('source map exceeds size budget'); }
    const reader = response.body.getReader(), parts = [];
    let length = 0;
    try {
      for (;;) {
        const {done, value} = await reader.read();
        if (done) break;
        length += value.length;
        if (length > maximum) throw Error('source map exceeds size budget');
        parts.push(value);
      }
    } finally { await reader.cancel(); }
    return Buffer.concat(parts, length);
  }
  const location = value => value && typeof value.file === 'string' && value.file.length > 0
    && ['line', 'col', 'end_col'].every(key => Number.isSafeInteger(value[key]) && value[key] > 0)
    && value.end_col >= value.col && typeof value.component === 'string';
  const at = value => ({file: value.file, line: value.line, col: value.col, end_col: value.end_col, component: value.component});
  return {
    /** A map a trace carries (LLP 1079 D5): found by its own digest, as a refreshed one is. */
    add(map) { if (digest(map?.digest) && Array.isArray(map.nodes)) maps.set(map.digest, map); },
    async refresh() {
      if (!locator) return false;
      try {
        let bytes, key = null, expected = null;
        if (remote) {
          const envelopeURL = new URL(new URL(locator).pathname.endsWith('/exact.json') ? locator : '/exact.json', locator);
          const envelope = JSON.parse((await fetchBytes(envelopeURL, 1024 * 1024)).toString('utf8'));
          const card = envelope.dev?.sourceMap;
          if (!card) throw Error('no development source map');
          if (!digest(card.sha256) || !Number.isSafeInteger(card.bytes) || card.bytes < 0 || card.bytes > limit
            || typeof card.url !== 'string' || !digest(envelope.plan?.sha256)) throw Error('invalid source map card');
          const url = new URL(card.url, envelopeURL);
          if (url.origin !== envelopeURL.origin) throw Error('source map URL must use the development origin');
          key = `${url.href}:${card.sha256}:${card.bytes}:${envelope.plan.sha256}`;
          if (key === lastCard) return true;
          bytes = await fetchBytes(url, card.bytes);
          if (bytes.length !== card.bytes || hash(bytes) !== card.sha256) throw Error('source map card mismatch');
          expected = envelope.plan.sha256;
        } else {
          const path = `${resolve(locator)}.map.json`;
          if (statSync(path).size > limit) throw Error('source map exceeds size budget');
          bytes = readFileSync(path);
          if (bytes.length > limit) throw Error('source map exceeds size budget');
          key = hash(bytes);
          if (key === lastCard) return true;
        }
        const map = JSON.parse(bytes.toString('utf8'));
        if (!digest(map.digest) || !Array.isArray(map.nodes) || (expected && map.digest !== expected)) throw Error('invalid or mismatched source map');
        maps.delete(map.digest); maps.set(map.digest, map);
        while (maps.size > 4) maps.delete(maps.keys().next().value);
        lastCard = key; problem = 'source map does not match the running plan';
      } catch (error) { problem = error.code === 'ENOENT' ? 'no development source map' : error.message; lastCard = null; }
      return maps.size > 0;
    },
    attach(node) {
      for (const style of Object.values(node.style ?? {})) delete style.origin;
      const map = digest(node.planDigest) ? maps.get(node.planDigest) : null;
      const entry = Number.isSafeInteger(node.site) && node.site >= 0 ? map?.nodes[node.site] : null;
      if (!entry || !location(entry) || !Array.isArray(entry.chain) || !entry.chain.every(location)
        || !Array.isArray(entry.bindings) || !entry.bindings.every(b => b && typeof b.row === 'string'
          && typeof b.origin === 'string' && /^(own|tag|class:.+)$/.test(b.origin))) {
        node.sourceMap = {status: 'unavailable', reason: map ? 'invalid source location' : problem};
        return;
      }
      node.sourceMap = {status: 'compatible', digest: map.digest, ...at(entry), chain: entry.chain.map(at)};
      for (const {row, origin} of entry.bindings) {
        const style = node.style?.[row];
        if (style && ['authored', 'dynamic'].includes(style.source)) style.origin = origin;
      }
    },
  };
}

/** Several locators as one reader: the first map whose digest is the node's plan's answers; otherwise the last reason. */
export function sourceMapReaders(locators) {
  const readers = locators.length ? locators.map(sourceMapReader) : [sourceMapReader(null)];
  return {
    add(map) { readers[0].add(map); },
    async refresh() { return (await Promise.all(readers.map(r => r.refresh()))).some(Boolean); },
    attach(node) { for (const r of readers) { r.attach(node); if (node.sourceMap?.status === 'compatible') return; } },
  };
}

/** A targeted reply owns its identity. A concurrently fetched tree may already
 * describe a replacement plan with reused view IDs and cannot relabel it. */
/** `layout`'s CLI arguments (LLP 1012 §7; LLP 1080.001 D1, D2): `layout
 * [<target>]`, `layout <target> at <x> <y>`, `layout <target> native
 * [<depth>]`, `layout agree [<limit>]` — the keyword wins over a testId of
 * that name, which a numeric id still reaches. Returns `layout`'s arguments. */
export function layoutArgs(args) {
  const count = (word) => { const n = Number(word); if (!Number.isInteger(n)) throw new Error(`layout: ${word} is not an integer`); return n; };
  if (args[0] === 'agree') return [undefined, undefined, { agree: true, ...(args[1] != null ? { limit: count(args[1]) } : {}) }];
  if (args[1] === 'native') return [args[0], undefined, { native: args[2] != null ? { depth: count(args[2]) } : {} }];
  if (args[1] === 'at') return [args[0], [Number(args[2]), Number(args[3])]];
  return [args[0]];
}

export function identifyInspectedNode(reply, target) {
  const node = reply.node;
  if (!node) return;
  const testId = node.props?.testId;
  if (typeof target !== 'number' && !/^\d+$/.test(String(target)) && testId !== target) {
    throw Error(`layout: target ${target} changed during inspection; retry`);
  }
  if (testId != null) node.testId = testId;
  const box = reply.nodes?.find(value => value.id === node.id);
  if (box) { box.type = node.type; if (testId != null) box.testId = testId; }
}

/**
 * The transcript form (LLP 1012 §7): the one text rendering of a reply, for
 * eyes — a pure function of the JSON, lossy on purpose (the JSON is
 * complete; only text, value, label and checked ride along), never parsed back.
 * `scripts/fixtures/transcript.txt` pins it. A part in [brackets] appears
 * only when its field is present (not null); strings are JSON-quoted.
 *
 *   tree    epoch E · incarnation I · clock C ms · N nodes
 *           {"  " × depth}{Type}#{id} [{testId}] hatch="…" "{text}" value="…" label="…" checked=true|false ({handlers, comma-separated})
 *           an iframe adds url="…" loading=true|false and `[guest]` outline lines
 *   layout  viewport W×H [· safe-area T R B L · keyboard K, when any is not 0] [· status bar light-content|dark-content (#id), iOS] · clock C ms
 *           #{id} [{testId}] {Type} {x},{y} {w}×{h} scroll {sx},{sy} [overscroll {ox},{oy}]
 *   logs    "(N earlier lines dropped by the journal ring)" when dropped > 0; the journal lines as they are;
 *           the host's lines indented two spaces; "(nothing new)" when there is nothing
 *   state   the JSON, indented two spaces
 *   perf    {target} — seq [A..]B · clock [X..]Y ms · incarnation I [· partial: N walked]
 *           one row per site: component, file:line (or `site N`), then each counter the host has
 *   perf frames  period P ms (source) · presented N · late L · missed M [· overruns O] · segments S, the window's
 *           percentiles, then one line per late frame; `virtual clock: no frame was presented`
 *   others  the JSON on one line
 */
export function render(op, r) {
  if (op === 'ax' || (op === 'tree' && r.ax)) return renderAx(r); // LLP 1080.002 D9
  const q = JSON.stringify;
  switch (op) {
    case 'tree': {
      const entities = Array.isArray(r.entities) ? r.entities : r.nodes?.some((n) => n.components) ? r.nodes : null;
      if (entities) return [`epoch ${r.epoch} · incarnation ${r.incarnation} · clock ${r.clock} ms · tick ${r.tick} · ${entities.length} entities`, ...entities.map((e) => `${'  '.repeat(e.depth ?? 0)}${e.name ?? ''}#${e.id} (${(e.components ?? []).join(', ')})${e.tags?.length ? ' ' + e.tags.join(' ') : ''}`), ...(r.truncated ? ['(truncated)'] : [])].join('\n');
      const lines = [`epoch ${r.epoch} · incarnation ${r.incarnation} · clock ${r.clock} ms · ${r.nodes.length} nodes`];
      const rootDepth = r.nodes[0]?.depth ?? 0;
      for (const n of r.nodes) {
        const depth = Math.max(0, n.depth - rootDepth);
        const p = n.props ?? {};
        lines.push(`${'  '.repeat(depth)}${n.type}#${n.id}${p.testId != null ? ` [${p.testId}]` : ''}${p.hatch != null ? ` hatch=${q(p.hatch)}` : ''}${p.text != null ? ` ${q(p.text)}` : ''}${p.value != null ? ` value=${q(p.value)}` : ''}${p.accessibilityLabel != null ? ` label=${q(p.accessibilityLabel)}` : ''}${p.checked != null ? ` checked=${p.checked}` : ''}${n.focused ? " [focused]" : ""}${n.inactive ? " [inactive]" : ""}${n.world ? ` world{${n.world.name}} · ${n.world.entities} entities · tick ${n.world.tick}` : ""}${n.handlers?.length ? ` (${n.handlers.join(', ')})` : ''}${n.url != null ? ` url=${q(n.url)} loading=${n.loading}` : ''}`);
        for (const g of n.guest ?? []) lines.push(`${'  '.repeat(depth + g.depth + 1)}[guest] ${g.tag}${g.id != null ? `#${g.id}` : ''}${g.testId != null ? ` [${g.testId}]` : ''}${g.text != null ? ` ${q(g.text)}` : ''}`);
      }
      return lines.join('\n');
    }
    case 'layout': {
      if (r.entity) {
        const e = r.entity, b = e.screen, p = e.world?.position;
        return `#${e.id ?? ''} ${e.name ?? ''}${p ? ` world ${Array.isArray(p) ? p.join(',') : [p.x, p.y, p.z].join(',')}` : ''}${b && ['x','y','w','h'].every(k => Number.isFinite(b[k])) ? ` · screen ${b.x},${b.y} ${b.w}×${b.h}` : ' · screen unavailable'}${e.depth != null ? ` · depth ${e.depth}` : ''}${e.visible ? ` · inFrustum ${!!e.visible.inFrustum} · behindCamera ${!!e.visible.behindCamera}` : ''}`;
      }
      if (r.agreement) return renderAgreement(r.agreement);
      if (!r.viewport) return q(r);
      const e = r.env;
      const insets = ['safe-area-inset-top', 'safe-area-inset-right', 'safe-area-inset-bottom', 'safe-area-inset-left', 'keyboard-inset-height'];
      const env = e && insets.some((k) => e[k]) ? ` · safe-area ${e['safe-area-inset-top']} ${e['safe-area-inset-right']} ${e['safe-area-inset-bottom']} ${e['safe-area-inset-left']} · keyboard ${e['keyboard-inset-height']}` : '';
      // The fold (LLP 1078 D7), when the device is not flat: its posture, the segment counts and each segment's box.
      const fold = e && (e['device-posture'] === 'folded' || (e['horizontal-viewport-segments'] ?? 1) * (e['vertical-viewport-segments'] ?? 1) > 1) ? ` · posture ${e['device-posture']} · segments ${e['horizontal-viewport-segments']}×${e['vertical-viewport-segments']}${(e['viewport-segments'] ?? []).map((r) => ` [${r[0]},${r[1]} ${r[2]}×${r[3]}]`).join('')}` : '';
      // `overscroll` is how far a scroller sits past its own ends — a stretched rubber band, which the offset
      // alone cannot distinguish from an ordinary scroll position. Printed only when there is one.
      const past = (n) => (n.ox != null || n.oy != null ? ` overscroll ${n.ox ?? 0},${n.oy ?? 0}` : '');
      // The status bar's style Exact asks for (LLP 1105 D7, iOS), when it is not the default.
      const bar = r.statusBar && r.statusBar.style !== 'default' ? ` · status bar ${r.statusBar.style} (#${r.statusBar.source})` : '';
      const lines = [`viewport ${r.viewport.w}×${r.viewport.h}${past(r.viewport)}${env}${fold}${bar} · clock ${r.clock} ms`].concat((r.nodes ?? []).map((n) => `#${n.id}${n.testId != null ? ` [${n.testId}]` : ''}${n.type != null ? ` ${n.type}` : ''} ${n.native?.placement === 'window' ? `${n.native.view} · system-owned geometry` : `${n.x},${n.y} ${n.w}×${n.h}${n.sx != null ? ` scroll ${n.sx},${n.sy}` : ''}${past(n)}`}`));
      if (r.node) lines.push(...renderNode(r.node));
      return lines.join('\n');
    }
    case 'logs':
      return [...(r.dropped > 0 ? [`(${r.dropped} earlier lines dropped by the journal ring)`] : []), ...r.lines, ...(r.world ?? []).flatMap((w) => w.lines.map((line) => 'world ' + line)), ...(r.host ?? []).map((l) => '  ' + l)].join('\n') || '(nothing new)';
    case 'state':
      return q(r, null, 2);
    case 'perf':
      return renderPerf(r);
    case 'type':
      if (r.steps) return r.steps.map(step => `${step.op} ${step.args.map(a => typeof a === 'string' ? a : q(a)).join(' ')}\n${step.error ? 'ERROR ' + step.error : render(step.op, step.reply)}`).join('\n');
      return q(r);
    default:
      return q(r);
  }
}

/**
 * The block `layout <target>` adds under the listing (LLP 1035.002 D1): the
 * node's identity and site, one line per row with its source, its spaces,
 * the chains above it, its visibility, and what the host mounted. Every
 * part appears only when the host reported it — a space a host cannot
 * observe is absent, never a zero.
 *
 *   node #{id} [{testId}] {Type} · site {n} · instance {keys} · epoch E · incarnation I
 *     {row} = {value} ({authored | inherited from #id | initial}[, applied {…}])
 *     space viewport X,Y W×H · frame X,Y W×H (kernel, in the parent) · window X,Y W×H · screen X,Y W×H · scale S
 *     scroll [viewport | #id] sx,sy · … (outermost first)   clip #id overflow|clip-path · …
 *     visible hidden=… inert=… inViewport=… clipped=…       native key=value …
 *     browser {row}="{the browser's computed value}" …      (the web's oracle beside the kernel's answer)
 */
function renderNode(n) {
  const q = JSON.stringify;
  const box = (b) => (b ? `${b.x},${b.y} ${b.w}×${b.h}` : '—');
  const instance = (n.instance ?? []).map((i) => (i.key !== undefined ? q(i.key) : `arm ${i.arm}`)).join(' / ');
  const out = [`node #${n.id}${n.testId != null ? ` [${n.testId}]` : ''} ${n.type}${n.site != null ? ` · site ${n.site}` : ''}${instance ? ` · instance ${instance}` : ''} · epoch ${n.epoch} · incarnation ${n.incarnation}`];
  const source = n.sourceMap;
  if (source?.status === 'compatible') {
    const at = value => `${value.file}:${value.line}:${value.col}`;
    out.push(`  @ ${at(source)} (${source.component}) · compatible source map`);
    for (const call of source.chain) out.push(`    called from ${call.component} @ ${at(call)}`);
  } else if (source) out.push(`  source unavailable: ${source.reason}`);
  // Rows and keys sort by name: a host that answers through a dictionary
  // (AppKit, UIKit) has no order to offer, and the transcript must not
  // depend on which host answered.
  const sorted = (o) => Object.entries(o ?? {}).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
  for (const [row, v] of sorted(n.style)) out.push(`  ${row} = ${typeof v.value === 'string' ? v.value : q(v.value)} (${v.source}${v.from != null ? ` from #${v.from}` : ''}${v.origin ? `, ${v.origin}` : ''}${v.applied != null ? `, applied ${v.applied}` : ''})`);
  // @ref LLP 1043.000 §3 D4, D7 — web geometry is paragraph-content-local.
  const flowSpace = n.flow?.coordinate_space === 'content' ? 'leaf content box' : 'leaf border box';
  for (const { kind, ...values } of n.flow_shapes ?? []) out.push(`  flow ${kind} ${Object.entries(values).map(([k, v]) => `${k}=${q(v)}`).join(' ')} (${flowSpace})`);
  if (n.flow_skipped ?? n.flow?.skipped) out.push(`  flow skipped: ${n.flow_skipped ?? n.flow.skipped}`);
  // @ref LLP 1043.000 §3 D6 — painted fragments retain logical byte ranges.
  for (const f of n.fragments ?? n.flow?.fragments ?? []) out.push(`  fragment bytes ${f.start}..${f.end} · band ${f.band ?? f.line} · ${f.x},${f.y} ${f.width}×${f.height ?? n.flow?.line_height} (leaf content box)`);
  // @ref LLP 1093 D12 — a box a multi-column flow breaks, and a container's columns.
  for (const f of n.column_fragments ?? []) out.push(`  column fragment ${f.x},${f.y} ${f.w}×${f.h}${f.lines ? ` · lines ${f.lines[0]}..${f.lines[1]}` : ''}`);
  for (const c of n.columns ?? []) out.push(`  column ${c.x},${c.y} ${c.w}×${c.h}${c.holds ? '' : ' · holds nothing'}`);
  if (n.fragment_skipped) out.push(`  kept whole in its column: ${n.fragment_skipped}`);
  const sp = n.space ?? {};
  out.push(`  space viewport ${box(sp.viewport)}${n.frame ? ` · frame ${box(n.frame)} (kernel, in the parent)` : ''}${sp.window ? ` · window ${box(sp.window)}` : ''}${sp.screen ? ` · screen ${box(sp.screen)}` : ''}${sp.capture?.scale != null ? ` · scale ${sp.capture.scale}` : ''}`);
  if (n.scroll?.length) out.push(`  scroll ${n.scroll.map((c) => `${c.id != null ? `#${c.id}` : 'viewport'} ${c.sx},${c.sy}`).join(' · ')}`);
  if (n.clip?.length) out.push(`  clip ${n.clip.map((c) => `#${c.id} ${c.kind}`).join(' · ')}`);
  if (n.visible) out.push(`  visible ${sorted(n.visible).map(([k, v]) => `${k}=${v}`).join(' ')}`);
  if (n.native) out.push(`  native ${sorted(n.native).filter(([k]) => k !== 'subviews').map(([k, v]) => `${k}=${typeof v === 'string' ? v : q(v)}`).join(' ')}`);
  if (n.browser) out.push(`  browser ${sorted(n.browser).map(([k, v]) => `${k}=${q(v)}`).join(' ')}`);
  if (n.native?.subviews) out.push(...renderSubviews(n.native.subviews));
  return out;
}

/** The counters a `perf` site row may carry, in the order they print (LLP 1079 D1). */
const COUNTERS = ['instances', 'created', 'retired', 'evaluated', 'unchanged', 'authored', 'inherited', 'moved'];

/** `perf [<target>] [during "<op>" …]` and `perf frames [live <ms>] [late <n>]`
 * (LLP 1079 D2, D4). `during` reads, drives each quoted op through `step`, reads
 * again, and subtracts: the delta belongs to the driver, a read changes nothing.
 * `live <ms>` lends the page's clock to the wall for that long and measures the
 * frames it presents (the platformer's diary, R11: a game's 60 fps). */
/** `tap <node>/<part>` (LLP 1075.003.000.001 §3.5): a control a hatch drew, by the id `tree` lists under its node. Reached
 *  only as real platform input at its place: the host aims (the part live, in its window, not covered), the driver delivers a
 *  pointer event there (a touch on iOS under `--touch platform`), and the host says where it landed, against the aim's token.
 *  `unsupported` where no real carrier exists; never a fallback, never an activation by name. Null when `target` names no part. */
export async function partTap({ s, host, touch }, target, opts = {}) {
  const cut = typeof target === 'string' ? target.lastIndexOf('/') : -1;
  if (cut <= 0 || cut === target.length - 1) return null;
  const of = target.slice(0, cut), name = target.slice(cut + 1);
  let node;
  try { node = await s.target(of); } catch { return null; }
  if (!['web', 'macos', 'ios'].includes(host) || (host === 'ios' && touch === 'agent')) {
    return { tapped: node.id, part: name, delivery: 'unsupported', reason: host === 'ios' ? 'a part takes a real touch: open the simulator with --touch platform' : `the ${host} host has no real pointer carrier for a part` };
  }
  const aim = await s.op({ op: 'tap', id: node.id, part: name, aim: true });
  if (aim.error) throw new Error(aim.error);
  // One real click at the aimed point (`clicks 1 at x y`, the mouse form that takes a point on any node); a touch on iOS.
  await s.tap(of, { ...opts, ...(host === 'ios' ? {} : { clicks: 1 }), at: aim.aimed.at });
  const landed = await s.op({ op: 'tap', id: node.id, part: name, landed: aim.aimed.token });
  if (landed.error) throw new Error(landed.error);
  return { ...landed, target, at: aim.aimed.at };
}

export async function perfOp(s, args, line, step) {
  if (args[0] === 'frames') {
    const at = word => { const i = args.indexOf(word); return i < 0 ? undefined : Number(args[i + 1]); };
    const live = at('live'), late = at('late');
    if (live !== undefined && !(Number.isInteger(live) && live > 0 && live <= 120000)) throw Error('perf frames live <ms>: a whole number of milliseconds, 1–120000');
    return s.perf(null, { frames: true, late, ...(live !== undefined ? { live } : {}) });
  }
  // `perf hatches` (LLP 1075.003.000.001 §3.3): the hatches' calls and what their code counted.
  if (args[0] === 'hatches') {
    const read
 = () => s.op({ op: 'perf', hatches: true });
    const at = line.search(/\sduring\s/);
    if (at < 0) return read();
    const ops = [...line.slice(at).matchAll(/"((?:[^"\\]|\\.)*)"/g)].map(m => JSON.parse(`"${m[1]}"`));
    if (!ops.length) throw Error('perf hatches during: quote each op, as perf hatches during "tap start" "clock +1000"');
    const before = await read();
    for (const op of ops) await step(op);
    return hatchDelta(before, await read());
  }
  const target = args[0] && args[0] !== 'during' ? args[0] : undefined;
  const at = line.search(/\sduring\s/);
  if (at < 0) return s.perf(target);
  const ops = [...line.slice(at).matchAll(/"((?:[^"\\]|\\.)*)"/g)].map(m => JSON.parse(`"${m[1]}"`));
  if (!ops.length) throw Error('perf … during: quote each op, as perf feed during "tap start" "clock +1000"');
  const before = await s.perf(target);
  for (const op of ops) await step(op);
  return perfDelta(before, await s.perf(target));
}

/** Two `perf` reads' difference, site by site. Refused by name when they
 * describe different runs: another plan, another incarnation, or counters
 * that went backwards (a runner that restarted in place). */
export function perfDelta(a, b) {
  if (!a.plan || !b.plan) throw Error('perf: a read names no plan; no difference is defined');
  // A bounded first read leaves sites out: their whole lifetime would read as new work.
  if (a.truncated) throw Error(`perf: the first read was partial (${a.walked} walked); name a narrower target`);
  if (a.plan !== b.plan) throw Error(`perf: the plan changed between the reads (${a.plan?.slice(0, 12)} → ${b.plan?.slice(0, 12)}); no difference is defined`);
  if (a.incarnation !== b.incarnation) throw Error(`perf: incarnation ${a.incarnation} → ${b.incarnation} between the reads; no difference is defined`);
  if (b.seq < a.seq) throw Error(`perf: seq went back (${a.seq} → ${b.seq}): the runner restarted between the reads`);
  const before = new Map(a.sites.map(x => [x.site, x]));
  const sites = b.sites.map(x => {
    const was = before.get(x.site), d = { ...x };
    for (const k of COUNTERS.slice(1)) if (typeof x[k] === 'number') {
      d[k] = x[k] - (was?.[k] ?? 0);
      if (d[k] < 0) throw Error(`perf: site ${x.site}'s ${k} went back (${was[k]} → ${x[k]}): the runner restarted between the reads`);
    }
    return d;
  });
  return { ...b, sites, from: { seq: a.seq, clock: a.clock } };
}

function renderHatchPerf(r) {
  const ms = v => `${Math.round(v * 100) / 100} ms`;
  const lines = [`perf hatches: seq ${r.seq}${r.from ? ` (from ${r.from.seq})` : ''}, plan ${r.plan?.slice(0, 12) ?? 'unknown'}${r.measuring ? '' : '; not measuring (a production build collects nothing)'}${r.truncated ? '; truncated' : ''}`];
  for (const [name, h] of Object.entries(r.hatches)) lines.push(`  ${name}: ${h.calls} calls, ${ms(h.ms)}, worst ${ms(h.worst)}`);
  for (const c of r.calls) if (c.site != null) lines.push(`    ${c.hatch} site ${c.site} ${c.moment}: ${c.calls} calls, ${ms(c.ms)}`);
  for (const [scope, names] of Object.entries(r.counters)) for (const [name, n] of Object.entries(names)) lines.push(`  count ${scope} ${name}: ${n}`);
  for (const [scope, names] of Object.entries(r.timings)) for (const [name, t] of Object.entries(names))
    lines.push(`  timing ${scope} ${name}: ${t.count} samples, sum ${ms(t.sum)}, max ${ms(t.max)}, p50 ${ms(t.p50)}, p95 ${ms(t.p95)}${t.dropped ? `, ${t.dropped} dropped from the ring` : ''}${t.measured ? ' (measured)' : ''}`);
  if (r.rejected || r.abandoned || r.limited) lines.push(`  refused: ${r.rejected} calls past a bound or badly named, ${r.abandoned} spans abandoned, ${r.limited} log lines over the rate`);
  return lines.join('\n');
}

/** Two `perf hatches` reads' difference: calls, time and counters, each
 * cumulative, subtracted; the samples' percentiles are the later read's.
 * Refused across a changed plan or incarnation, as `perfDelta` is. */
export function hatchDelta(a, b) {
  if (!a.plan || !b.plan) throw Error('perf hatches: a read names no plan; no difference is defined');
  if (a.truncated) throw Error('perf hatches: the first read was partial; no difference is defined');
  if (a.plan !== b.plan) throw Error(`perf hatches: the plan changed between the reads (${a.plan?.slice(0, 12)} → ${b.plan?.slice(0, 12)}); no difference is defined`);
  if (a.incarnation !== b.incarnation) throw Error(`perf hatches: incarnation ${a.incarnation} → ${b.incarnation} between the reads; no difference is defined`);
  const less = (x, was, keys) => { const d = { ...x }; for (const k of keys) if (typeof x[k] === 'number') d[k] = x[k] - (was?.[k] ?? 0); return d; };
  const key = c => `${c.hatch}\n${c.site ?? ''}\n${c.moment}`, before = new Map(a.calls.map(c => [key(c), c]));
  const calls = b.calls.map(c => less(c, before.get(key(c)), ['calls', 'ms']));
  const hatches = Object.fromEntries(Object.entries(b.hatches).map(([name, h]) => [name, less(h, a.hatches[name], ['calls', 'ms'])]));
  const counters = Object.fromEntries(Object.entries(b.counters).map(([scope, names]) => [scope, less(names, a.counters[scope], Object.keys(names))]));
  const timings = Object.fromEntries(Object.entries(b.timings).map(([scope, names]) => [scope,
    Object.fromEntries(Object.entries(names).map(([name, t]) => [name, less(t, a.timings[scope]?.[name], ['count', 'sum'])]))]));
  return { ...b, hatches, calls, counters, timings, from: { seq: a.seq, clock: a.clock } };
}

function renderPerf(r) {
  if (r.calls && r.hatches) return renderHatchPerf(r);
  if (r.virtual) return 'virtual clock: no frame was presented (LLP 1079 D4); `perf frames live <ms>` measures a live window';
  if (r.unavailable) return 'this host observes no presented frames';
  if (r.lifetime) {
    const w = r.window ?? {}, f = n => n == null ? '—' : `${n} ms`;
    const out = [`period ${r.period.ms} ms (${r.period.source}) · presented ${r.lifetime.presented} · late ${r.lifetime.late} · missed ${r.lifetime.missed}${r.lifetime.overruns != null ? ` · overruns ${r.lifetime.overruns}` : ''} · segments ${r.lifetime.segments}${r.covers?.length ? ` · covers ${r.covers.join(', ')}` : ''}`,
      `window t=${w.from}..${w.to} · ${w.samples} samples (${w.dropped} dropped) · p50 ${f(w.p50)} · p95 ${f(w.p95)} · p99 ${f(w.p99)} · max ${f(w.max)}`];
    if (r.live) out.unshift(`live window ${r.live.ms} ms · clock ${r.live.from}..${r.live.to}`);
    for (const w of r.world ?? []) {
      const g = n => n == null ? '—' : `${Math.round(n * 100) / 100} ms`, ms = x => x ? `p50 ${g(x.p50)} p95 ${g(x.p95)} p99 ${g(x.p99)} mean ${g(x.mean)}` : '—';
      out.push(`  world ${w.canvas}: frame ${ms(w.perf.frameMs)} · tick ${ms(w.perf.tickMs)} · feed ${ms(w.perf.feedMs)} · encode ${ms(w.perf.encodeMs)}`);
    }
    for (const l of r.late ?? []) out.push(`  t=${l.t} late: ${l.missed} missed (${l.interval} ms)${l.overrun ? ` · main ${l.overrun} ms past the target` : ''} · seq ${l.seq ? l.seq.join('..') : '—'}${l.apply != null ? ` · apply ${l.apply}` : ''}${l.layout != null ? ` · layout ${l.layout}` : ''}${l.loaf ? ` · loaf script ${l.loaf.script}${l.loaf.styleLayout != null ? ` style+layout ${l.loaf.styleLayout}` : ''}` : ''}`);
    return out.join('\n');
  }
  const round = x => typeof x === 'number' ? Math.round(x * 100) / 100 : x;
  const span = (from, to) => from != null && from !== to ? `${round(from)}..${round(to)}` : `${round(to)}`;
  const cols = COUNTERS.filter(k => r.sites.some(x => typeof x[k] === 'number'));
  // A component used twice is two sites: the nearest call site tells them apart.
  const where = x => x.source?.status === 'compatible' ? `${x.source.file.split('/').pop()}:${x.source.line}${x.source.chain.length ? ` (from :${x.source.chain[0].line})` : ''}` : `site ${x.site}`;
  const rows = r.sites.map(x => [x.source?.status === 'compatible' ? x.source.component : '?', where(x), ...cols.map(k => String(x[k]))]);
  const head = ['component', 'site', ...cols], width = head.map((h, i) => Math.max(h.length, ...rows.map(row => row[i].length)));
  const pad = row => '  ' + row.map((c, i) => c.padEnd(width[i])).join('  ').trimEnd();
  const unmapped = r.sites.find(x => x.source?.status !== 'compatible');
  return [`${r.target ?? 'every root'} — seq ${span(r.from?.seq, r.seq)} · clock ${span(r.from?.clock, r.clock)} ms · incarnation ${r.incarnation}${r.truncated ? ` · partial: ${r.walked} walked` : ''}${unmapped ? ` · source unavailable: ${unmapped.source?.reason ?? 'no development source map'}` : ''}`,
    pad(head), ...rows.map(pad)].join('\n');
}

/** `bun scripts/agent.mjs trace <file>` (LLP 1079 D5): a person's session
 * read back with no app running. The trace's own source map joins its sites;
 * without one, `locators` are tried as the live driver tries them. */
export async function readTrace(file, locate = () => []) {
  const t = JSON.parse(readFileSync(file, 'utf8'));
  const maps = sourceMapReaders(t.map ? [] : locate(t.identity?.app));
  if (t.map) maps.add(t.map); else await maps.refresh();
  const perf = t.perf && !t.perf.error ? { target: 'every root', ...t.perf } : null;
  for (const site of perf?.sites ?? []) { const n = { planDigest: perf.plan ?? t.plan, site: site.site }; maps.attach(n); site.source = n.sourceMap; }
  return { ...t, perf };
}

/** The trace a phone's dev menu saved last (`ExactSession.latestTrace`,
 * `tmp/trace-latest.json` in the app's container), copied into the app's
 * `target/traces/` (LLP 1079 D5): from a simulator's container, which
 * devicectl cannot copy, else off the phone as its screenshots are. */
export function phoneTrace(ph, a, run = spawnSync) {
  const stamp = new Date().toISOString().replace(/[:.]/g, '-');
  const file = resolve(a.target, 'traces', `trace-${String(ph.name ?? ph.udid).replace(/[^\w.-]/g, '_')}-${stamp}.json`);
  mkdirSync(resolve(a.target, 'traces'), { recursive: true });
  const unsaved = `no trace saved in ${a.id} on ${ph.name ?? ph.udid}: Save Trace in its dev menu first (four fingers tapped once)`;
  const container = run('xcrun', ['simctl', 'get_app_container', ph.udid, a.id, 'data'], { encoding: 'utf8' });
  // simctl's 148: no simulator by that id, so a phone. A simulator it knows
  // but cannot answer for (shut down, the app not installed) says why.
  if (container.status !== 0 && container.status !== 148) throw new Error(`${ph.name ?? ph.udid}: ${container.stderr.trim() || 'simctl get_app_container failed'}`);
  if (container.status === 0) {
    const saved = resolve(container.stdout.trim(), 'tmp/trace-latest.json');
    if (!container.stdout.trim() || !existsSync(saved)) throw new Error(unsaved);
    copyFileSync(saved, file);
  } else {
    const copied = run('xcrun', ['devicectl', 'device', 'copy', 'from', '--quiet', '--device', ph.udid,
      '--domain-type', 'appDataContainer', '--domain-identifier', a.id, '--source', 'tmp/trace-latest.json', '--destination', file], { encoding: 'utf8', timeout: 20000 });
    if (copied.status !== 0) throw new Error(`${unsaved}, or the phone is unreachable: ${copied.stderr || copied.error || copied.stdout}`);
  }
  console.error(`trace copied to ${file}`);
  return file;
}

/** A trace as text: who and what made it, each timing's proxy, the frames,
 * each late frame beside the journal lines stamped in its interval and the
 * transactions it carried (D4: juxtaposition, never a cause), the sites. */
export function renderTrace(t) {
  const id = t.identity ?? {};
  const out = [`trace · ${[id.host, id.target].filter(Boolean).join('/')} · ${id.app ?? 'app ?'} · ${id.trust ?? 'trust ?'} · commit ${id.commit?.slice(0, 12) ?? '?'}${id.working_tree ? ' (dirty)' : ''} · ${[id.platform, id.arch, id.cpu, id.os, id.device].filter(Boolean).join(' ')}`];
  if (t.proxies) out.push(`proxies · ${Object.entries(t.proxies).map(([k, v]) => `${k}: ${Array.isArray(v) ? v.join(', ') : v}`).join(' · ')}`);
  if (t.frames?.lifetime) {
    out.push('', renderPerf({ ...t.frames, late: [] }));
    // Beside each late frame, the journal lines appended just before its own
    // line — at most five, none from before the previous late frame: the
    // journal's `t=` is the runner's logical clock, which need not be the
    // frame's (a page adopted from a checkpoint keeps the render's).
    const lines = t.journal?.lines ?? [];
    for (const r of t.frames.late ?? []) {
      const at = lines.findIndex(l => l.includes(` frame late at ${r.t}:`));
      let from = at;
      while (from > 0 && at - from < 5 && !lines[from - 1].includes(' frame late at ')) from--;
      const near = at < 0 ? [] : lines.slice(from, at);
      const n = r.seq ? r.seq[1] - r.seq[0] + 1 : 0;
      out.push(`  late at ${r.t}: ${r.missed} missed (${r.interval} ms)${r.overrun ? ` · main ${r.overrun} ms past the target` : ''} · ${n} transaction${n === 1 ? '' : 's'}${r.seq ? ` (seq ${r.seq.join('..')})` : ''}${r.apply ? ` · apply ${r.apply}` : ''}${r.layout != null ? ` · layout ${r.layout}` : ''}${r.paint != null ? ` · paint ${r.paint}` : ''}${r.loaf ? ` · loaf script ${r.loaf.script}${r.loaf.styleLayout != null ? ` style+layout ${r.loaf.styleLayout}` : ''}` : ''}${at < 0 ? ' · its journal line is gone (the ring turned over)' : ` · ${near.length} journal line${near.length === 1 ? '' : 's'} before it`}`);
      for (const l of near) out.push(`    ${l}`);
    }
  } else if (t.frames) out.push('', renderPerf(t.frames));
  if (t.perf) out.push('', renderPerf(t.perf));
  if (t.hatches?.perf?.calls) out.push('', renderHatchPerf(t.hatches.perf));
  return out.join('\n');
}

const box = (b) => (b ? `${b.x},${b.y} ${b.w}×${b.h}` : '—');

/**
 * `layout <target> native` (LLP 1080.001 D1, D6): the views and flat-leaf
 * layers under the node, one line each, indented by depth — observation,
 * no verdicts.
 *
 *   subviews {root} · N entries, depth D [· truncated …]
 *     view {Class} #{node} | {role} [of #{owner}] [hidden] [alpha A] [inert] [masks] [z Z] [transform …] X,Y W×H [· opaque platform, N children]
 *     layer {Class} {role} #{leaf} … path X,Y W×H [detached] [hidden]
 */
function renderSubviews(s) {
  if (s.unavailable) return [`  subviews unavailable: ${s.unavailable}`];
  const out = [`  subviews ${s.root} · ${s.count} entries, depth ${s.depth}${s.truncated?.length ? ` · truncated ${s.truncated.join(', ')}` : ''}`];
  for (const e of s.entries ?? []) {
    const pad = '    ' + '  '.repeat(e.depth);
    if (e.kind === 'layer') {
      out.push(`${pad}layer ${e.class} ${e.role} ${e.leaves.map((id) => `#${id}`).join(' ')}${e.leafCount > e.leaves.length ? ` (+${e.leafCount - e.leaves.length})` : ''} path ${box(e.path)}${e.path?.empty ? ' (empty)' : ''}${e.attached ? '' : ' [detached]'}${e.hidden ? ' [hidden]' : ''}`);
      continue;
    }
    const who = e.node != null ? `#${e.node}` : `${e.role}${e.owner != null ? ` of #${e.owner}` : ''}`;
    const l = e.layer ?? {};
    const flags = [e.hidden && '[hidden]', e.alpha !== 1 && `[alpha ${e.alpha}]`, e.interactive === false && '[inert]', l.masksToBounds && '[masks]',
      l.zPosition && `[z ${l.zPosition}]`, l.transform != null && `[transform ${Array.isArray(l.transform) ? l.transform.join(',') : l.transform}]`].filter(Boolean);
    out.push(`${pad}view ${e.class} ${who}${flags.length ? ' ' + flags.join(' ') : ''} ${box(e.frame)}${e.opaque ? ` · opaque ${e.opaque}, ${e.children} children` : ''}`);
  }
  return out;
}

/**
 * `layout agree` (LLP 1080.001 D2, D6): whether the walk was complete, what
 * it covered, then one line per disagreement.
 *
 *   agreement complete|INCOMPLETE (reasons) · N disagreements · ±P px · roots …
 *     coverage stray J judged, O opaque · frame C compared, S size-only, K skipped · hidden H compared, L claimed · parked roots R
 *     {kind} #{id} [{testId}] … (frame: kernel X,Y W×H · native X,Y W×H (Δ dx,dy dw×dh); stray: {Class} under #{id} X,Y W×H)
 *     … N more (truncated)
 */
function renderAgreement(a) {
  if (a.unavailable) return `agreement unavailable: ${a.unavailable}`;
  const total = Object.values(a.counts ?? {}).reduce((x, y) => x + y, 0);
  const c = a.coverage ?? {};
  const sum = (o) => Object.values(o ?? {}).reduce((x, y) => x + y, 0);
  const out = [`agreement ${a.complete ? 'complete' : `INCOMPLETE (${a.incomplete.join(', ')})`} · ${total} disagreements · ±${a.tolerance?.px} px · roots ${(a.roots?.walked ?? []).join(', ')}${a.roots?.excluded?.length ? ` · excluded ${a.roots.excluded.join(', ')}` : ''}`,
    `  coverage stray ${c.stray?.judged} judged, ${c.stray?.opaque} opaque · frame ${c.frame?.compared} compared, ${c.frame?.sizeOnly} size-only, ${sum(c.frame?.skipped)} skipped · hidden ${c.hidden?.compared} compared, ${sum(c.hidden?.claimed)} claimed · parked roots ${c.kept?.parkedRoots} · leaving ${c.kept?.leaving}`];
  const tag = (id, t) => (id != null ? `#${id}${t != null ? ` [${t}]` : ''}` : '');
  for (const d of a.disagreements ?? []) {
    if (d.kind === 'frame') out.push(`  frame ${tag(d.id, d.testId)} kernel ${box(d.kernel)} · native ${box(d.native)} (Δ ${d.delta.x},${d.delta.y} ${d.delta.w}×${d.delta.h})${d.sizeOnly ? ' size only' : ''}`);
    else if (d.kind === 'stray') out.push(`  stray ${d.class}${d.retired != null ? ` (retired #${d.retired})` : ''}${d.under != null ? ` under ${tag(d.under, d.underTestId)}` : ''} ${box(d.frame)}`);
    else if (d.kind === 'hidden') out.push(`  hidden ${tag(d.id, d.testId)} native hidden=${d.native?.hidden} · expected hidden=${d.expected?.hidden}${d.hider ? ` (${d.hider})` : ''}`);
    else out.push(`  ${d.kind} ${d.id != null ? tag(d.id, d.testId) : d.class ?? ''}${d.frame ? ` ${box(d.frame)}` : ''}`);
  }
  const listed = a.disagreements?.length ?? 0;
  if (a.truncated?.includes('disagreements')) out.push(`  … ${total - listed} more (truncated)`);
  return out.join('\n');
}

/** Explain a refused placed-child tap using the world's own visibility. */
export async function tapRefusal(session, target, error) {
  if (error.transport) return error; // the carrier failed: no diagnostic read can answer
  try {
    for (const canvas of (await session.tree()).nodes.filter(n => n.world)) {
      const canvasName = canvas.props?.testId ?? canvas.id;
      const outline = await session.tree(canvasName);
      if (!outline.entities?.some(entity => entity.name === target)) continue;
      const owner = `${canvasName}:${target}`;
      const state = await session.state(owner).catch(() => null);
      if (!state?.entity?.placed?.hidden) continue;
      const box = await session.layout(owner);
      const reason = box.entity?.visible?.behindCamera ? 'hidden (behind the camera)' : 'hidden';
      error.message = `${target} is ${reason}: \`layout ${owner}\` (with --json before the quoted operation) shows visibility and any available screen box; layout ${target} shows the child when mounted`;
      return error;
    }
  } catch { /* Preserve the original refusal if the diagnostic target also vanished. */ }
  return error;
}

/** A convenience over state, screenshot and type; wire replies keep all tags. */
export function worldView(session, name) {
  return {
    /** The first page of entities (512), or every page with {all:true}, read at one tick and hash. */
    async snapshot({all = false} = {}) {
      const page = {limit:5000};
      const first = all ? await session.state(`${name}:*`, undefined, false, false, page) : await session.state(`${name}:*`);
      const {tick, hash} = first, entities = [...first.entities];
      for (let r = first; all && r.truncated;) {
        r = await session.state(`${name}:*`, undefined, false, false, {...page, from:r.next});
        if (r.tick !== tick || r.hash !== hash) throw new Error('world changed while paging; capture on the agent clock with no concurrent drive');
        entities.push(...r.entities);
      }
      return {tick, hash, entities, truncated: all ? false : first.truncated};
    },
    /** Every resource's value, as `state world:* resources` reads them. */
    async resources() { return (await session.state(`${name}:*`, undefined, false, false, {limit:1, resources:true})).resources; },
    state: entity => session.state(`${name}:${entity}`),
    save: path => session.screenshot(path, name, 'save'),
    run: ms => {
      if (!Number.isFinite(ms) || ms < 0) throw new Error('run duration must be finite and nonnegative');
      // Like Sim::run, establish the current epoch after deferred assets settle
      // before moving time. Otherwise a newly ready world can eat the first seek.
      return session.clock('+0').then(() => session.clock(`+${ms}`));
    },
    settle: async () => (await session.clock('settle')).settled === true,
    tap: code => session.type(name, {key:code}),
    key_down: code => session.type(name, {key:code, phase:'down'}),
    key_up: code => session.type(name, {key:code, phase:'up'}),
    async local_position(entity) { return (await this.get(entity, 'Transform'))?.position; },
    async global_position(entity) {
      try { return (await session.layout(`${name}:${entity}`)).entity?.world?.position; }
      catch (error) {
        if ((error.reply?.error === `no entity named \`${entity}\`` || error.reply?.error?.startsWith(`no entity named \`${entity}\`; `))) return undefined;
        throw error;
      }
    },
    async get(entity, component) {
      try {
        return (await session.state(`${name}:${entity}`)).entity?.components?.[component];
      } catch (error) {
        if ((error.reply?.error === `no entity named \`${entity}\`` || error.reply?.error?.startsWith(`no entity named \`${entity}\`; `))) return undefined;
        throw error;
      }
    },
    hold: (code, ms) => session.type(name, {key:code, for:ms}),
  };
}
