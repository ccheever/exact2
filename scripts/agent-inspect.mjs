import { createHash } from 'node:crypto';
import { readFileSync, statSync } from 'node:fs';
import { resolve } from 'node:path';

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
    async refresh() { return (await Promise.all(readers.map(r => r.refresh()))).some(Boolean); },
    attach(node) { for (const r of readers) { r.attach(node); if (node.sourceMap?.status === 'compatible') return; } },
  };
}

/** A targeted reply owns its identity. A concurrently fetched tree may already
 * describe a replacement plan with reused view IDs and cannot relabel it. */
export function identifyInspectedNode(reply, target) {
  const node = reply.node;
  if (!node) return;
  const testId = node.props?.testId;
  if (typeof target !== 'number' && !/^\d+$/.test(String(target)) && testId !== target) {
    throw Error(`layout: target ${target} changed during inspection; retry`);
  }
  if (testId != null) node.testId = testId;
  const box = reply.nodes.find(value => value.id === node.id);
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
 *           {"  " × depth}{Type}#{id} [{testId}] "{text}" value="…" label="…" checked=true|false ({handlers, comma-separated})
 *           an iframe adds url="…" loading=true|false and `[guest]` outline lines
 *   layout  viewport W×H [· safe-area T R B L · keyboard K, when any is not 0] · clock C ms
 *           #{id} [{testId}] {Type} {x},{y} {w}×{h} scroll {sx},{sy} [overscroll {ox},{oy}]
 *   logs    "(N earlier lines dropped by the journal ring)" when dropped > 0; the journal lines as they are;
 *           the host's lines indented two spaces; "(nothing new)" when there is nothing
 *   state   the JSON, indented two spaces
 *   others  the JSON on one line
 */
export function render(op, r) {
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
        lines.push(`${'  '.repeat(depth)}${n.type}#${n.id}${p.testId != null ? ` [${p.testId}]` : ''}${p.text != null ? ` ${q(p.text)}` : ''}${p.value != null ? ` value=${q(p.value)}` : ''}${p.accessibilityLabel != null ? ` label=${q(p.accessibilityLabel)}` : ''}${p.checked != null ? ` checked=${p.checked}` : ''}${n.focused ? " [focused]" : ""}${n.inactive ? " [inactive]" : ""}${n.accessibleName != null ? ` name=${q(n.accessibleName)}` : ""}${n.world ? ` world{${n.world.name}} · ${n.world.entities} entities · tick ${n.world.tick}` : ""}${n.handlers?.length ? ` (${n.handlers.join(', ')})` : ''}${n.url != null ? ` url=${q(n.url)} loading=${n.loading}` : ''}`);
        for (const g of n.guest ?? []) lines.push(`${'  '.repeat(depth + g.depth + 1)}[guest] ${g.tag}${g.id != null ? `#${g.id}` : ''}${g.testId != null ? ` [${g.testId}]` : ''}${g.text != null ? ` ${q(g.text)}` : ''}`);
      }
      return lines.join('\n');
    }
    case 'layout': {
      if (r.entity) {
        const e = r.entity, b = e.screen, p = e.world?.position;
        return `#${e.id ?? ''} ${e.name ?? ''}${p ? ` world ${Array.isArray(p) ? p.join(',') : [p.x, p.y, p.z].join(',')}` : ''}${b && ['x','y','w','h'].every(k => Number.isFinite(b[k])) ? ` · screen ${b.x},${b.y} ${b.w}×${b.h}` : ' · screen unavailable'}${e.depth != null ? ` · depth ${e.depth}` : ''}${e.visible ? ` · inFrustum ${!!e.visible.inFrustum} · behindCamera ${!!e.visible.behindCamera}` : ''}`;
      }
      if (!r.viewport) return q(r);
      const e = r.env;
      const env = e && Object.values(e).some((v) => v) ? ` · safe-area ${e['safe-area-inset-top']} ${e['safe-area-inset-right']} ${e['safe-area-inset-bottom']} ${e['safe-area-inset-left']} · keyboard ${e['keyboard-inset-height']}` : '';
      // `overscroll` is how far a scroller sits past its own ends — a stretched rubber band, which the offset
      // alone cannot distinguish from an ordinary scroll position. Printed only when there is one.
      const past = (n) => (n.ox != null || n.oy != null ? ` overscroll ${n.ox ?? 0},${n.oy ?? 0}` : '');
      const lines = [`viewport ${r.viewport.w}×${r.viewport.h}${past(r.viewport)}${env} · clock ${r.clock} ms`].concat(r.nodes.map((n) => `#${n.id}${n.testId != null ? ` [${n.testId}]` : ''}${n.type != null ? ` ${n.type}` : ''} ${n.native?.placement === 'window' ? `${n.native.view} · system-owned geometry` : `${n.x},${n.y} ${n.w}×${n.h}${n.sx != null ? ` scroll ${n.sx},${n.sy}` : ''}${past(n)}`}`));
      if (r.node) lines.push(...renderNode(r.node));
      return lines.join('\n');
    }
    case 'logs':
      return [...(r.dropped > 0 ? [`(${r.dropped} earlier lines dropped by the journal ring)`] : []), ...r.lines, ...(r.world ?? []).flatMap((w) => w.lines.map((line) => 'world ' + line)), ...(r.host ?? []).map((l) => '  ' + l)].join('\n') || '(nothing new)';
    case 'state':
      return q(r, null, 2);
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
  const sp = n.space ?? {};
  out.push(`  space viewport ${box(sp.viewport)}${n.frame ? ` · frame ${box(n.frame)} (kernel, in the parent)` : ''}${sp.window ? ` · window ${box(sp.window)}` : ''}${sp.screen ? ` · screen ${box(sp.screen)}` : ''}${sp.capture?.scale != null ? ` · scale ${sp.capture.scale}` : ''}`);
  if (n.scroll?.length) out.push(`  scroll ${n.scroll.map((c) => `${c.id != null ? `#${c.id}` : 'viewport'} ${c.sx},${c.sy}`).join(' · ')}`);
  if (n.clip?.length) out.push(`  clip ${n.clip.map((c) => `#${c.id} ${c.kind}`).join(' · ')}`);
  if (n.visible) out.push(`  visible ${sorted(n.visible).map(([k, v]) => `${k}=${v}`).join(' ')}`);
  if (n.native) out.push(`  native ${sorted(n.native).map(([k, v]) => `${k}=${typeof v === 'string' ? v : q(v)}`).join(' ')}`);
  if (n.browser) out.push(`  browser ${sorted(n.browser).map(([k, v]) => `${k}=${q(v)}`).join(' ')}`);
  return out;
}
