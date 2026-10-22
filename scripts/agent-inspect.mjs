import { MalformedOperation, inputJSON } from './agent-keys.mjs';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { renderAx } from './agent-ax.mjs';
import { decodePng, encodePng, shrink } from './png.mjs';

/** Film's bounds (LLP 1012.001.000 D2): a drive's pictures, not a recording, decoded in memory at once. */
export const FILM_FRAMES = 240, FILM_PIXELS = 64e6;
export const WORLD_LIMIT = 256 * 1024 * 1024;
export function worldFile(path) {
  if (statSync(path).size > WORLD_LIMIT) throw new Error('world carrier exceeds 256 MiB limit; inspect `state world:*` and reduce saved entities before `screenshot checkpoint.world world save`');
  const bytes = readFileSync(path);
  if (bytes.length > WORLD_LIMIT) throw new Error('world carrier exceeds 256 MiB limit; inspect `state world:*` and reduce saved entities before `screenshot checkpoint.world world save`');
  return bytes;
}

/** A capture's pixels, decoded once, at the drive's `--scale`: shrunk by area from the capture's own pixels a point, never enlarged. */
export function scaledScreenshot(r, scale, host) {
  const png = decodePng(readFileSync(r.screenshot)), drawn = r.scale ?? png.width / r.w, said = Math.round(drawn * 100) / 100;
  if (scale === undefined) return { image: png, shrunk: false };
  if (!Number.isFinite(drawn)) throw new Error(`--scale: the ${host} capture did not say its size in points`);
  if (scale > drawn + 1e-3) throw new Error(`--scale ${scale}: this ${host} capture is ${said} pixel${said === 1 ? '' : 's'} a point; a screenshot is never enlarged`);
  if (drawn - scale <= 1e-3) return { image: png, shrunk: false };
  return { image: shrink(png, Math.max(1, Math.round(png.width * scale / drawn)), Math.max(1, Math.round(png.height * scale / drawn))), shrunk: true };
}

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
      const origin = o => typeof o === 'string' && /^(own|tag|class:.+)$/.test(o);
      delete node.bare_reason;
      for (const style of Object.values(node.style ?? {})) { delete style.origin; delete style.state; delete style.at; }
      const map = digest(node.planDigest) ? maps.get(node.planDigest) : null;
      const entry = Number.isSafeInteger(node.site) && node.site >= 0 ? map?.nodes[node.site] : null;
      if (!entry || !location(entry) || !Array.isArray(entry.chain) || !entry.chain.every(location)
        || !Array.isArray(entry.bindings) || !entry.bindings.every(b => b && typeof b.row === 'string'
          // A base origin, absent where only a state part writes the row; each part's own (theme2's 03:18).
          && (b.origin === undefined || origin(b.origin)) && (b.states === undefined || Array.isArray(b.states) && b.states.every(p => p && typeof p.state === 'string' && origin(p.origin))))) {
        node.sourceMap = {status: 'unavailable', reason: map ? 'invalid source location' : problem};
        return;
      }
      node.sourceMap = {status: 'compatible', digest: map.digest, ...at(entry), chain: entry.chain.map(at)};
      // @ref LLP 1104 D2 — the first reason belongs only to this compatible plan site.
      if (typeof entry.bare_reason === 'string' && entry.bare_reason.length > 0 && entry.bare_reason.length <= 1024) node.bare_reason = entry.bare_reason;
      for (const b of entry.bindings) {
        const style = node.style?.[b.row];
        if (!style || !['authored', 'dynamic'].includes(style.source)) continue;
        // A row's base origin; a press row has none, and is its `:active` part's, written in its part (LLP 1121 D8).
        const part = b.origin === undefined ? b.states?.find(p => p.state === 'active') : null;
        if (b.origin === undefined && !part) continue;
        style.origin = part ? part.origin : b.origin;
        if (part) style.state = part.state;
        // (Its place where it has one: a conditional class's part has none.)
        const written = part?.at;
        const placed = written && typeof written.file === 'string' && ['line', 'col', 'end_col'].every(k => Number.isSafeInteger(written[k]));
        if (placed) style.at = {file: written.file, line: written.line, col: written.col, end_col: written.end_col};
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

/** LLP 1104 D2 / 1035.002 D3: list reasons only when geometry and tree
 * describe the same settlement; a target read already owns its identity. */
export function identifyLayoutNodes(layout, tree, maps) {
  const same = layout.epoch === tree.epoch && layout.incarnation === tree.incarnation;
  const by = new Map(tree.nodes.map(n => [n.id, n]));
  for (const node of layout.nodes) {
    const record = by.get(node.id);
    if (!record) continue;
    node.type = record.type;
    if (record.props.testId) node.testId = record.props.testId;
    if (maps && same) { node.site = record.site; node.planDigest = tree.planDigest; maps.attach(node); }
  }
}

/** A targeted reply owns its identity. A concurrently fetched tree may already
 * describe a replacement plan with reused view IDs and cannot relabel it. */
/** `layout`'s CLI arguments (LLP 1012 §7; LLP 1080.001 D1, D2): `layout
 * [<target>]`, `layout <target> at <x> <y>`, `layout <target> native
 * [<depth>]`, `layout agree [<limit>]` — the keyword wins over a testId of
 * that name, which a numeric id still reaches. Returns `layout`'s arguments. */
export function layoutArgs(args) {
  const count = (word) => { const n = Number(word); if (!Number.isInteger(n)) throw new MalformedOperation(`layout: ${word} is not an integer`); return n; };
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
 *           #{id} [{testId}] {Type} {x},{y} {w}×{h} scroll {sx},{sy} of {rx},{ry} [port {x},{y} {w}×{h}] [overscroll {ox},{oy}]
 *   logs    "(N earlier lines dropped by the journal ring)" when dropped > 0; the journal lines as they are;
 *           the host's lines indented two spaces; on an iOS simulator, the newest crash report of the build
 *           on that simulator since the last `logs` (its path, how it ended, the crashing thread's top
 *           frames); "(nothing new)" when there is nothing
 *   state   [epoch E · incarnation I · clock C ms], then one line per other section: `{name} {JSON on one line}`;
 *           an app's state (a reply with `slots`) without its empty sections (`emptySection`), a world's whole;
 *           over STATE_CAP its small sections and the others' sizes instead (`stateCap`)
 *   tap, type  [ERROR {error} · ]tapped|typed #{id} "{target}" [· at X,Y, a contact's phase or a drag] · delivery D · epoch E
 *           [· incarnation I, when not 1] · clock C ms [· key=JSON for every other field but carrier and mode];
 *           a contact's phase reads `tap {phase}`, a held request's answer `answered @{ticket} "{choice}"`, a reply with
 *           an error or `delivery: unsupported` `tap`|`type`, never `tapped`|`typed`; a delivered input whose answers did
 *           not land keeps its verb and says `landing failed: …`
 *   perf    {target} — seq [A..]B · clock [X..]Y ms · incarnation I [· partial: N walked]
 *           one row per site: component, file:line (or `site N`), then each counter the host has
 *   perf frames  period P ms (source) · presented N · late L · missed M [· overruns O] · segments S, the window's
 *           percentiles, then one line per late frame; `virtual clock: no frame was presented`
 *   others  one line (`renderLine`): what it did ([clock C ms · settled], [screenshot {path} · W×H], else the op),
 *           the tags, then key=JSON for each other field
 */
/** A `layout` reply's presentations the platform draws scaled (iOS's floating sheet), a line each: its boxes are the
 * authored frames times the factor, which no style asked for and none should undo. */
export const scaledLines = (layout) => (layout?.scaled ?? []).map((p) =>
  `route ${p.route || '?'} is a ${p.presentation} the platform draws at ${p.scale} of its laid-out size: its boxes are the authored frames × ${p.scale}, not the app's doing`);
export function render(op, r, how = {}) {
  if (op === 'ax' || (op === 'tree' && r.ax)) return renderAx(r); // LLP 1080.002 D9
  const q = JSON.stringify;
  switch (op) {
    case 'check': return r.text; // LLP 1116.003 D8 (agent-check.mjs)
    case 'screenshot': return r?.compare ? r.text : renderLine(op, r); // `compare`: the comparison with a reference (agent-compare.mjs)
    case 'tree': {
      const entities = Array.isArray(r.entities) ? r.entities : r.nodes?.some((n) => n.components) ? r.nodes : null;
      if (entities) return [`epoch ${r.epoch} · incarnation ${r.incarnation} · clock ${r.clock} ms · tick ${r.tick} · ${entities.length} entities`, ...entities.map((e) => `${'  '.repeat(e.depth ?? 0)}${e.name ?? ''}#${e.id} (${(e.components ?? []).join(', ')})${e.tags?.length ? ' ' + e.tags.join(' ') : ''}`), ...(r.truncated ? ['(truncated)'] : [])].join('\n');
      const lines = [`epoch ${r.epoch} · incarnation ${r.incarnation} · clock ${r.clock} ms · ${r.nodes.length} nodes`];
      const rootDepth = r.nodes[0]?.depth ?? 0;
      for (const n of r.nodes) {
        const depth = Math.max(0, n.depth - rootDepth);
        const p = n.props ?? {};
        lines.push(`${'  '.repeat(depth)}${n.type}#${n.id}${p.testId != null ? ` [${p.testId}]` : ''}${p.hatch != null ? ` hatch=${q(p.hatch)}` : ''}${p.text != null ? ` ${q(p.text)}` : ''}${p.value != null ? ` value=${q(p.value)}` : ''}${p.accessibilityLabel != null ? ` label=${q(p.accessibilityLabel)}` : ''}${p.checked != null ? ` checked=${p.checked}` : ''}${n.focused ? " [focused]" : ""}${n.inactive ? " [inactive]" : ""}${n.unshown ? " [not in the iOS bar: hidden in the app]" : ""}${n.world ? ` world{${n.world.name}} · ${n.world.entities} entities · tick ${n.world.tick}` : ""}${n.status != null ? ` status ${q(n.status)}` : ""}${n.handlers?.length ? ` (${n.handlers.join(', ')})` : ''}${n.url != null ? ` url=${q(n.url)} loading=${n.loading}` : ''}`);
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
      // The title-bar area beside the window's controls, where the host defines it.
      const titlebar = e && e['titlebar-area-width'] != null ? ` · titlebar-area ${e['titlebar-area-x']},${e['titlebar-area-y']} ${e['titlebar-area-width']}×${e['titlebar-area-height']}` : '';
      const lines = [`viewport ${r.viewport.w}×${r.viewport.h}${past(r.viewport)}${env}${titlebar}${fold}${bar} · clock ${r.clock} ms`].concat((r.nodes ?? []).map((n) => `#${n.id}${n.testId != null ? ` [${n.testId}]` : ''}${n.type != null ? ` ${n.type}` : ''}${n.bare_reason ? ` · bare: \`${n.bare_reason}\`` : ''} ${n.native?.placement === 'window' ? `${n.native.view} · system-owned geometry` : `${n.x},${n.y} ${n.w}×${n.h}${n.sx != null ? ` scroll ${n.sx},${n.sy}${rangeAndPort(n)}` : ''}${past(n)}`}`));
      lines.splice(1, 0, ...scaledLines(r));
      if (r.node) lines.push(...renderNode(r.node));
      return lines.join('\n');
    }
    case 'logs':
      return [...(r.dropped > 0 ? [`(${r.dropped} earlier lines dropped by the journal ring)`] : []), ...r.lines, ...(r.world ?? []).flatMap((w) => [...(w.dropped > 0 ? [`world ${w.canvas ?? w.id ?? ''} (${w.dropped} earlier lines dropped by the journal ring)`] : []), ...(w.lines ?? []).map((line) => 'world ' + line)]), ...(r.host ?? []).map((l) => '  ' + l), ...(r.crash ?? [])].join('\n') || '(nothing new)';
    case 'state': {
      if (!r || typeof r !== 'object') return q(r);
      if (Array.isArray(r)) return stateCap(q(r), r, how);
      if (r.draws || r.visible || r.look) return renderLook(r);
      // One line a section: pretty-printing cost an app's state more than half again its JSON (Caltrain, 2026-10-09).
      const { epoch, incarnation, clock, ...rest } = r, app = 'slots' in r;
      const head = [epoch != null && `epoch ${epoch}`, incarnation != null && `incarnation ${incarnation}`, clock != null && `clock ${clock} ms`].filter(Boolean).join(' · ');
      const sections = Object.entries(rest).filter(([, v]) => !(app && emptySection(v)));
      return stateCap([head, ...sections.map(([k, v]) => `${k} ${q(v)}`)].filter(Boolean).join('\n'), r, how, head, sections);
    }
    case 'perf':
      return renderPerf(r);
    case 'type':
      if (r?.steps) return r.steps.map(step => `${step.op} ${step.args.map(a => typeof a === 'string' ? a : q(a)).join(' ')}\n${step.error ? 'ERROR ' + step.error : render(step.op, step.reply)}`).join('\n');
    // falls through: a single input's reply
    case 'tap': {
      if (!r || typeof r !== 'object') return q(r);
      // `carrier` and `mode` are the drive's own (its host, its timing): the JSON keeps them. So does a press's `at`, a
      // point only a hit-test diagnosis reads; a contact's phase and a drag answer with where the finger is, so theirs shows.
      const { tapped, typed, target, phase, delivery, epoch, incarnation, clock, at, carrier, mode, error, landError, answered, ticket, ...rest } = r, id = tapped ?? typed;
      // An input that did not happen (a failure, a form the carrier cannot deliver) never reads in the past tense, and a
      // failure says so first; a phase names itself (`tap down`), held or not.
      // A held request's answer (`tap @3 cancel`) was given: `answered @3 "cancel"`.
      const verb = error != null || phase != null || delivery === 'unsupported' ? `${op}${phase != null ? ` ${phase}` : ''}` : answered != null ? 'answered' : tapped != null ? 'tapped' : typed != null ? 'typed' : op;
      const head = `${error != null ? `ERROR ${error} · ` : ''}${verb}${ticket != null ? ` @${ticket}` : ''}${answered != null ? ` ${q(answered)}` : ''}${id != null ? ` #${id}` : ''}${target != null ? ` ${q(target)}` : ''}`;
      const where = (phase != null || r.drag != null) && at != null ? (Array.isArray(at) ? `at ${at.join(',')}` : `at=${q(at)}`) : null;
      // Delivered, but what it started did not land (`landed`): said next to the verb, which stays `tapped`.
      return [head, landError != null && `landing failed: ${landError}`, where, delivery != null && `delivery ${delivery}`, epoch != null && `epoch ${epoch}`, incarnation != null && incarnation !== 1 && `incarnation ${incarnation}`, clock != null && `clock ${clock} ms`,
        ...Object.entries(rest).map(([k, v]) => `${k}=${q(v)}`)].filter(Boolean).join(' · ');
    }
    default:
      return renderLine(op, r);
  }
}

/** A reply with no form of its own, on one line: what it did, its tags as a tap's are, then each other field as
 * key=JSON (`carrier` and `mode`, the drive's own, only in the JSON). `clock` leads with where it landed and whether
 * it settled, `screenshot` with its file and size. */
function renderLine(op, r) {
  if (!r || typeof r !== 'object' || Array.isArray(r)) return JSON.stringify(r);
  const { epoch, incarnation, clock, carrier, mode, ...rest } = r;
  const lead = [op];
  if (op === 'clock') {
    if (clock != null) lead[0] = `clock ${clock} ms`;
    if ('settled' in rest) { lead.push(rest.settled ? 'settled' : `NOT settled${rest.reason != null ? ` (${rest.reason})` : ''}`); delete rest.settled; delete rest.reason; }
  } else if (op === 'screenshot' && typeof rest.screenshot === 'string') {
    lead[0] = `screenshot ${rest.screenshot}`;
    if (rest.w != null && rest.h != null) lead.push(`${rest.w}×${rest.h}`);
    delete rest.screenshot; delete rest.w; delete rest.h;
  }
  // What a picture shows that the app does not (a menu the agent painted, the authored bars), each on a line of its
  // own: in the run of key=values it read as one more field, and a painted menu as an app bug (euxy port).
  const notes = op === 'screenshot' ? ['paintedNote', 'chromeNote', 'unshownNote', 'note'].filter((k) => typeof rest[k] === 'string').map((k) => {
    const said = rest[k]; delete rest[k]; return `\n  NOTE: ${said}`;
  }) : [];
  return [...lead, epoch != null && `epoch ${epoch}`, incarnation != null && incarnation !== 1 && `incarnation ${incarnation}`, op !== 'clock' && clock != null && `clock ${clock} ms`,
    ...Object.entries(rest).map(([k, v]) => `${k}=${JSON.stringify(v)}`)].filter(Boolean).join(' · ') + notes.join('');
}

/** The most of a `state` reply its transcript prints whole (`state … --full` prints it all, `--json` every reply):
 * a sequencer's record was 142.6 KB on iOS, mostly resources, derives and presence. Over it, `slots`, `derives`
 * and then the smallest sections still print, up to STATE_SHOWN between them. */
export const STATE_CAP = 8192, STATE_SHOWN = 2048;
const size = (n) => (n < 1024 ? `${n} B` : `${(n / 1024).toFixed(1)} KB`);

/** A `state` transcript over STATE_CAP, in its place: its size, the small sections as they are, each other one's size,
 * largest first, and how to read one or all of it. `name` is what the op named (`state transport`); a world's reply is
 * paged (`from`, `limit`), never addressed by key. */
function stateCap(text, r, { full = false, name = null } = {}, head = '', sections = null) {
  const bytes = Buffer.byteLength(text);
  if (full || bytes <= STATE_CAP) return text;
  const what = name ? `state ${name}` : 'state', world = ['tick', 'entities', 'entity'].some((k) => k in r);
  const sized = sections?.map(([k, v]) => [k, v, Buffer.byteLength(JSON.stringify(v) ?? '')]) ?? [];
  const shown = new Set();
  let room = STATE_SHOWN;
  // The app's own states and derives first, then the smallest of the rest.
  const own = (k) => (r.slots !== undefined && (k === 'slots' || k === 'derives') ? 0 : 1);
  for (const [k, , n] of [...sized].sort((a, b) => own(a[0]) - own(b[0]) || a[2] - b[2])) { if (n > room) { if (own(k)) break; continue; } room -= n; shown.add(k); }
  const rest = sized.filter(([k]) => !shown.has(k)).sort((a, b) => b[2] - a[2]), top = rest[0]?.[0];
  const one = world ? null
    : !sections ? `one item: "state ${name ?? '<name>'}.0"`
    : name ? `one key: "state ${name}.${top}"`
    : `one section: "state ${top}", or a state, derive or resource by name, dotted: "state ${top}.<key>"`;
  return [`${what} is ${size(bytes)}, over the ${size(STATE_CAP)} printed whole${head ? ` · ${head}` : ''}`,
    ...sized.filter(([k]) => shown.has(k)).map(([k, v]) => `${k} ${JSON.stringify(v)}`),
    sections ? `not shown: ${rest.map(([k, , n]) => `${k} ${size(n)}`).join(' · ')}` : `${r.length} items`,
    `${one ? `${one}; ` : ''}all of it: "${what} --full", or --json`].join('\n');
}

/** A section an app's `state` transcript leaves out (LLP 1012 §7; the JSON keeps every one): null, an empty list or an
 * empty record — one that holds nothing, so leaving it out hides no value. Nothing else is judged: a host's defaults
 * (a hidden keyboard, an idle navigation) show, since the renderer cannot know them on every host. A world's or an
 * entity's reply has no `slots` and is never filtered: its empty `busy` or `entities` is the answer. */
export function emptySection(v) {
  return v === null || (typeof v === 'object' && (Array.isArray(v) ? !v.length : !Object.keys(v).length));
}

function rangeAndPort(scroller) {
  const range = scroller.rx != null ? ` of ${scroller.rx},${scroller.ry}` : '', p = scroller.port;
  return range + (p ? ` port ${p[0]},${p[1]} ${p[2]}×${p[3]}` : '');
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
 *     scroll [viewport | #id] sx,sy of rx,ry · … (outermost first)   clip #id overflow|clip-path · …
 *     visible hidden=… inert=… inViewport=… clipped=…       native key=value …
 *     browser {row}="{the browser's computed value}" …      (the web's oracle beside the kernel's answer)
 */
function renderNode(n) {
  const q = JSON.stringify;
  const box = (b) => (b ? `${b.x},${b.y} ${b.w}×${b.h}` : '—');
  const instance = (n.instance ?? []).map((i) => (i.key !== undefined ? q(i.key) : `arm ${i.arm}`)).join(' / ');
  const out = [`node #${n.id}${n.testId != null ? ` [${n.testId}]` : ''} ${n.type}${n.site != null ? ` · site ${n.site}` : ''}${instance ? ` · instance ${instance}` : ''} · epoch ${n.epoch} · incarnation ${n.incarnation}`];
  if (n.bare_reason) out.push(`  bare: \`${n.bare_reason}\``);
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
  for (const [row, v] of sorted(n.style)) out.push(`  ${row} = ${typeof v.value === 'string' ? v.value : q(v.value)} (${v.source}${v.from != null ? ` from #${v.from}` : ''}${v.origin ? `, ${v.origin}` : ''}${v.applied != null ? `, applied ${v.applied}` : ''}${v.token ? `, token ${v.token.names.join(' ') || v.token.text} in ${v.token.modes || 'the one tuple'}${v.token.implied ? ', implied by the boundary' : ''}` : ''})`);
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
  if (n.scroll?.length) out.push(`  scroll ${n.scroll.map((c) => `${c.id != null ? `#${c.id}` : 'viewport'} ${c.sx},${c.sy}${rangeAndPort(c)}`).join(' · ')}`);
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
    if (live !== undefined && !(Number.isInteger(live) && live > 0 && live <= 120000)) throw new MalformedOperation('perf frames live <ms>: a whole number of milliseconds, 1–120000');
    return s.perf(null, { frames: true, late, ...(live !== undefined ? { live } : {}) });
  }
  // `perf streams [start]` (LLP 1133.003 §9): each stream message's receipt to mapper entry, decode and commit.
  if (args[0] === 'streams') return s.op({ op: 'perf', streams: true, ...(args[1] === 'start' ? { start: true } : {}) });
  // `perf hatches` (LLP 1075.003.000.001 §3.3): the hatches' calls and what their code counted.
  if (args[0] === 'hatches') {
    const read = () => s.op({ op: 'perf', hatches: true });
    const at = line.search(/\sduring\s/);
    if (at < 0) return read();
    const ops = [...line.slice(at).matchAll(/"((?:[^"\\]|\\.)*)"/g)].map(m => inputJSON(`"${m[1]}"`));
    if (!ops.length) throw new MalformedOperation('perf hatches during: quote each op, as perf hatches during "tap start" "clock +1000"');
    const before = await read();
    for (const op of ops) await step(op);
    return hatchDelta(before, await read());
  }
  const target = args[0] && args[0] !== 'during' ? args[0] : undefined;
  const at = line.search(/\sduring\s/);
  if (at < 0) return s.perf(target);
  const ops = [...line.slice(at).matchAll(/"((?:[^"\\]|\\.)*)"/g)].map(m => inputJSON(`"${m[1]}"`));
  if (!ops.length) throw new MalformedOperation('perf … during: quote each op, as perf feed during "tap start" "clock +1000"');
  const before = await s.perf(target);
  for (const op of ops) await step(op);
  if (target?.startsWith('world:')) {
    const after = await s.perf(target, {spanSince: before.spanTicks});
    if (after.tick < before.tick || after.restarted !== before.restarted) throw Error('perf world: world restarted between reads');
    return {...after, from:{tick:before.tick}};
  }
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

// `perf streams` (LLP 1133.003 §9): a line per stream, its latencies from receipt in ms.
function renderStreamTiming(t) {
  if (!t.measuring) return 'stream timing is not measuring: `perf streams start` begins it';
  const ms = x => `p50 ${x.p50} p95 ${x.p95} p99 ${x.p99} max ${x.max} (${x.n})`;
  return [`stream timing over ${t.seconds} s`, ...t.streams.map(x => `  ${x.name ?? `ticket ${x.ticket}`}: ${x.messages} messages (${x.messageRate}/s) · ${x.commits} commits (${x.commitRate}/s) · ${x.decodes} decodes (${x.decodeRate}/s, ${x.decodedBytes} bytes)${x.frames != null ? ` · ${x.frames} frames out, ${x.decodeErrors} errors` : ''}${x.refused ? ` · ${x.refused} refused` : ''} · busy ${(x.busy * 100).toFixed(2)}%\n    receipt to entry ${ms(x.toEntry)}\n    receipt to decode ${ms(x.toDecode)}\n    receipt to commit ${ms(x.toCommit)}`)].join('\n');
}
function renderPerf(r) {
  if (r.spans) return [`${r.target} · tick ${r.from ? r.from.tick + '..' : ''}${r.tick} · exclusive ms${r.tickMs ? ` · tick p50 ${r.tickMs.p50} p95 ${r.tickMs.p95} mean ${r.tickMs.mean}` : ''}`,
    ...r.spans.map(s => `  ${s.name} at ${s.at}: p50 ${s.p50} p95 ${s.p95} max ${s.max} perTick ${s.perTick} (${s.count} ticks)`)].join('\n');
  if (r.calls && r.hatches) return renderHatchPerf(r);
  if (r.streamTiming) return renderStreamTiming(r.streamTiming);
  if (r.virtual) return 'virtual clock: no frame was presented (LLP 1079 D4); `perf frames live <ms>` measures a live window';
  if (r.unavailable) return 'this host observes no presented frames';
  if (r.unmeasured) return `frames not measured: ${r.unmeasured}`;
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
  if (Array.isArray(t.traceEvents)) {
    const cpu = t.traceEvents.filter(e => e.cat === 'world.cpu'), gpu = t.traceEvents.filter(e => e.cat === 'world.gpu');
    const frames = new Set(cpu.filter(e => e.name === 'frame').map(e => `${e.pid}:${e.args?.frame}`));
    out.push(`world trace: ${cpu.filter(e=>e.name==='tick').length} ticks, ${cpu.length} CPU intervals under ${frames.size} submitted frames, ${gpu.length} GPU passes (independent GPU clock; joined by frame)`);
  }
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

/** The `state` op's words (LLP 1116.007 D3.9). `state` is the whole record; `state <target>` a world's own (a game's
 * `world:*` or `world:entity`: its entities; `state <canvas> input` its world's input); a name that is a state, derive or resource, or a field of the record
 * (`mediaSession`), reads that, down a dotted path (`groceries.0.done`) as an authored `expect state` does. Any other
 * view answers with the whole record, so a name the record holds reads that even when a view carries it as its testId
 * or text (a sequencer's `transport` bar and the record's `transport`). */
export async function stateOp(s, args) {
  // `state <canvas> draws`, `state <canvas>:<entity> visible` and `state <canvas> look` are a world's look frame (LLP 1127.000 D1, D4).
  const look = ['draws', 'visible', 'look'].includes(args[1]) ? { [args[1]]: true, world: true } : {};
  const page = { ...look, ...(args[1] === 'events' ? {world:true, events:true, ...(args[2] ? {event_type:args[2]} : {})} : {}), ...(args.includes('from') ? { from: Number(args[args.indexOf('from') + 1]) } : {}), ...(args.includes('limit') ? { limit: Number(args[args.indexOf('limit') + 1]) } : {}), ...(args.includes('resources') ? { resources: true } : {}) };
  const read = (target) => s.state(target, args[1] === 'under' ? args[2] : undefined, args[1] === 'pose', args[1] === 'busy', page);
  if (args[0] == null) return read();
  // `state <canvas> input` (LLP 1127.003 D8): the world's controls, the devices and pads held, contexts, bindings, and
  // what fired at the last tick.
  if (args[1] === 'input') {
    const r = await s.op({ op: 'state', ...await s.target(args[0]), world: true });
    if (!r.world?.input) throw new Error(`state ${args[0]} input: ${args[0]} is not a world's canvas`);
    return { tick: r.tick, ...r.world.input };
  }
  const has = (o, k) => o !== null && typeof o === 'object' && Object.hasOwn(o, k);
  const [name, ...fields] = args[0].split('.'), bag = (record) => ({ ...record.resources, ...record.derives, ...record.slots });
  const holds = (record) => has(bag(record), name) || has(record, name);
  const lookup = (record) => {
    const named = bag(record);
    let got = has(named, name) ? named[name] : record[name], path = name;
    for (const field of fields) {
      if (!has(got, field)) throw new Error(`state ${path} has no field "${field}" (${got !== null && typeof got === 'object' ? `its fields: ${Object.keys(got).join(', ')}` : `it is ${JSON.stringify(got)}`})`);
      got = got[field]; path += `.${field}`;
    }
    return got;
  };
  let reply;
  try { reply = await read(args[0]); } catch (error) {
    if (!/^no view matches/.test(error.message)) throw error;
    const record = await s.state();
    if (!holds(record)) { error.message += `; nor is it a state, derive, resource or field of \`state\`: ${Object.keys(bag(record)).join(', ') || 'none declared'}`; throw error; }
    return lookup(record);
  }
  // An app's whole record (it has `slots`; a world's reply has none) answered a plain view: read the name it holds.
  return args.length === 1 && has(reply, 'slots') && holds(reply) ? lookup(reply) : reply;
}

/** The tasks due within 1 ms of `now`, `[name, at]`, soonest first: an `after(1, …)` an input or an answer armed, the web's
 * `setTimeout(f, 0)`, which runs a commit after the one that armed it. A frozen clock reaches it only when it moves, so
 * `clock settle` runs it as part of settling and `clock data`, which fires no timer, names it (euxy FRICTION 7). */
export async function nextCommitTasks(session, now) {
  const tasks = Object.entries((await session.state())?.tasks ?? {});
  return tasks.filter(([, at]) => Number.isFinite(at) && at > now && at <= now + 1).sort((a, b) => a[1] - b[1]);
}

/** `clock settle`'s reply when tasks are still due a commit later after its 16 rounds: not settled, and which. */
export function settleBound(soon) {
  const names = soon.map(([name]) => `\`${name}\``).join(', ');
  return { settled: false, reason: 'tasks', tasks: Object.fromEntries(soon), diagnostic: `clock settle ran 16 rounds of tasks due a commit later and ${names} ${soon.length > 1 ? 'are' : 'is'} due again at ${soon[0][1]} ms: a task that re-arms itself \`after(1, …)\` never settles; \`clock +N\` moves past it` };
}

/** Explain a refused placed-child tap using the world's own visibility. */
export async function tapRefusal(session, target, error) {
  if (error.transport || error.malformed) return error; // App state cannot explain malformed input or a failed transport.
  // What covers a target may leave on one of the app's timers, which the agent's frozen clock never reaches (a boot
  // overlay's `after(900, …)`, euxy FRICTION 9): the tasks still waiting, and the step that runs them.
  if (/covers its middle/.test(error.message ?? '')) try {
    const r = await session.state(), now = r?.clock;
    const due = Object.entries(r?.tasks ?? {}).filter(([, at]) => Number.isFinite(at) && at > now).sort((a, b) => a[1] - b[1]);
    if (Number.isFinite(now) && due.length) {
      const shown = due.slice(0, 3).map(([name, at]) => `\`${name}\` at ${at} ms`).join(', ') + (due.length > 3 ? `, and ${due.length - 3} more` : '');
      error.message += `; the agent's clock stands at ${now} ms, and the app's tasks wait on it (${shown}): if what covers it leaves on a timer, \`clock +${Math.ceil(due[Math.min(2, due.length - 1)][1] - now)}\` runs ${due.length > 1 ? 'them' : 'it'}`;
    }
  } catch { /* the refusal as it was */ }
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
        if (r.tick !== tick || r.hash !== hash || r.tableEdits !== first.tableEdits) throw new Error('world changed while paging; capture on the agent clock with no concurrent drive');
        entities.push(...r.entities);
      }
      return {tick, hash, entities, truncated: all ? false : first.truncated, ...Object.fromEntries(["tableEdits", "developmentEdits", "sessionDevelopmentEdits", "queuedSets"].filter(k => first[k] !== undefined).map(k => [k, first[k]]))};
    },
    /** Every resource's value, as `state world:* resources` reads them. */
    async resources() { return (await session.state(`${name}:*`, undefined, false, false, {limit:1, resources:true})).resources; },
    state: entity => session.state(`${name}:${entity}`),
    save: path => session.screenshot(path, name, 'save'),
    run: ms => {
      if (!Number.isFinite(ms) || ms < 0) throw new MalformedOperation('run duration must be finite and nonnegative');
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

// @ref LLP 1123 D10 — the layout regions, `env(-exact-<region>-inset-<edge>)`: the Apple hosts report UIKit's and
// AppKit's; a host that reports none resolves each to its safe area, which is what its facts then say too.
const REGIONS = ['safe-area-corners', 'layout-margins', 'layout-margins-corners', 'readable-content', 'readable-content-corners'];
export function withRegions(env) {
  if (!env || typeof env !== 'object') return env;
  for (const region of REGIONS) for (const edge of ['top', 'right', 'bottom', 'left']) env[`-exact-${region}-inset-${edge}`] ??= env[`safe-area-inset-${edge}`] ?? 0;
  return env;
}

/** A world's look frame as text (LLP 1127.000): its tells, its draws, or why an entity is or is not drawn. */
export function renderLook(r) {
  const q = JSON.stringify, head = `tick ${r.tick} · hash ${r.hash}`;
  if (r.visible) return `${head}\n${r.entity?.name ?? '#' + r.entity?.id}: ${r.visible} (${r.reason})`;
  if (r.draws) return [head, ...r.draws.map((d) => [d.pass.padEnd(13), d.draw, d.winding, d.submitted != null && `submitted ${d.submitted}`, d.kept != null && `kept ${d.kept}`, d.variant != null && `variant ${d.variant}`, 'fragments' in d && `fragments ${d.fragments ?? 'not measured'}`, 'covered' in d && (d.covered == null ? 'coverage not measured' : d.covered ? 'covered' : 'COVERED NOTHING'), d.draws != null && `${d.draws} draws`, d.summary && 'one entry for all its draws', `writes ${d.writes}`].filter(Boolean).join(' · '))].join('\n');
  return [head, ...lookLines(r.look)].join('\n');
}
function lookLines(look) {
  if (!look) return [];
  const measured = (m, taken) => m == null ? '' : m.unmeasured != null ? `not measured (${m.unmeasured})` : taken(m);
  const particles = look.particles ? ` · particles ${measured(look.particles, (draws) => draws.map((d) => `${d.batch}: ${d.live} live, ${d.covered} px`).join('; '))}` : '';
  const casters = look.sunTexels != null ? ` · sun shadow texels ${measured(look.sunTexels, (n) => n)}${look.sunCasters != null ? ` (casters ${look.sunCasters})` : ''}` : '';
  return [`mean luminance ${look.mean} · clipped ${(look.clipped * 100).toFixed(1)}%${particles}${casters} · readback ${look.readbackBytes} B${look.reduced ? ' · reduced, not judged' : ''}`, ...look.tells.map((t) => t.line), ...(look.gaps ?? []).map((g) => `look: ${g}`)];
}

/** `screenshot <png> <canvas> from <yaw> <pitch> <distance> | from top <height> | from camera [over <ms> every <ms>]`
 * (LLP 1127.000 D3): the world's look frame, the tick the agent's clock has reached drawn offscreen from that view and
 * written as a PNG, with its tells; `over … every` films it on the clock. A form of `screenshot`, never a new operation:
 * presentation only, the hash does not move. A device-less host draws it where the machine has a GPU. */
export async function worldLook(s, args) {
  const [path, target, , ...from] = args;
  const usage = 'screenshot <png> <canvas> from <yaw> <pitch> <distance> | from top <height> | from camera [over <ms> every <ms>]';
  const [view, rest] = from[0] === 'camera' ? [{}, from.slice(1)] : from[0] === 'top' ? [{ top: Number(from[1]) }, from.slice(2)]
    : [{ yaw: Number(from[0]), pitch: Number(from[1]), distance: Number(from[2]) }, from.slice(3)];
  if (Object.values(view).some((v) => !Number.isFinite(v))) throw new MalformedOperation(usage);
  const shoot = (file) => lookPicture(s, file, target, view);
  if (rest.length === 4 && rest[0] === 'over' && rest[2] === 'every') return s.film(path, { over: Number(rest[1]), every: Number(rest[3]), window: shoot });
  if (rest.length) throw new MalformedOperation(usage);
  return s.tagged(await shoot(resolve(path)));
}

/** After `clock settle` (`r` its reply) and every pixel `screenshot`, each shown world's tells where its canvas draws (LLP
 * 1127.000 D1): a look frame of the tick the clock reached, whose `look: …` lines reach the world's `logs`. The reply
 * carries them as `look`, the canvases looked at as `lookSeen`, what a frame could not measure, or a drawing canvas's "no
 * picture", as `lookGaps`, and a look frame that failed as `lookFailed`. The worlds are the ones shown now (a settle's
 * reply lists them; otherwise `state` does). A canvas that draws no frames (the GPU-less host) answers at once and adds
 * none. */
export async function worldTells(s, r) {
  if (r == null || r.error != null) return r;
  const lines = [], gaps = [], failed = [], seen = [];
  const shown = Array.isArray(r.world) ? r.world : await s.op({ op: 'state' }).then((st) => st.world ?? [], (error) => (failed.push(`worlds: ${error.message}`), []));
  for (const id of shown.map((w) => w.canvas).filter((c) => c != null)) {
    const reply = await s.op({ op: 'state', id, world: true, look: true, drawing: true }).catch((error) => ({ error: error.message }));
    if (reply.unavailable) { if (reply.drawing !== false) gaps.push(`canvas ${id}: ${reply.unavailable}`); continue; }
    if (reply.error != null || !reply.look) { failed.push(`canvas ${id}: ${reply.error ?? 'no look in the reply'}`); continue; }
    seen.push(id);
    lines.push(...reply.look.tells.map((t) => t.line));
    gaps.push(...(reply.look.gaps ?? []).map((g) => `canvas ${id}: ${g}`));
  }
  const tags = [['look', lines], ['lookSeen', seen], ['lookGaps', gaps], ['lookFailed', failed]];
  return { ...r, ...Object.fromEntries(tags.filter(([, v]) => v.length)) };
}

/** A film's frames together (`s.film`): each judged frame's tells as one entry of `lookFrames` (a live frame's lines, or
 * a look frame from the game's camera at full resolution; a reduced or other view's frame is a picture), all of them as
 * `look`, and every frame's gaps, failures and canvases looked at. */
export function lookAcross(replies) {
  const frames = [], gaps = [], failed = [], seen = [];
  for (const r of replies) {
    const own = r?.look && !Array.isArray(r.look) && !r.look.reduced && !r.view ? r.look : null;
    const judged = Array.isArray(r?.look) ? r.look : own ? (own.tells ?? []).map((t) => t.line) : null;
    if (judged) frames.push(judged);
    gaps.push(...(r?.lookGaps ?? []), ...(own?.gaps ?? []));
    failed.push(...(r?.lookFailed ?? []));
    seen.push(...(r?.lookSeen ?? []));
  }
  const tags = [['look', frames.flat()], ['lookFrames', frames.some((f) => f.length) ? frames : []], ['lookGaps', gaps], ['lookFailed', failed], ['lookSeen', [...new Set(seen)]]];
  return Object.fromEntries(tags.filter(([, v]) => v.length));
}

/** One look frame of `target` from `view` ({} the game's camera, {top}, or {yaw, pitch, distance}) written to `path`. */
export async function lookPicture(s, path, target, view = {}) {
  mkdirSync(dirname(resolve(path)), { recursive: true });
  const at = await s.target(target);
  const reply = await s.op({ op: 'screenshot', ...at, world: true, form: 'look', ...view });
  if (reply.unavailable) throw Object.assign(new Error(`look ${target}: ${reply.unavailable}`), { reply, unavailable: reply.unavailable });
  if (reply.error || typeof reply.data !== 'string') throw Object.assign(new Error(`look ${target}: ${reply.error ?? 'no pixels in the reply'}`), { reply });
  writeFileSync(resolve(path), encodePng({ width: reply.width, height: reply.height, data: Buffer.from(reply.data, 'base64') }));
  const { tick, hash, w, h, look, bounds } = reply, from = Object.fromEntries(Object.entries(view).filter(([k]) => k !== 'resolution'));
  // `bounds`: the scene's centre and radius an orbit or plan view was drawn within.
  return { screenshot: resolve(path), w, h, tick, hash, canvas: at.id, look, ...(bounds ? { bounds } : {}), ...(Object.keys(from).length ? { view: from } : {}) };
}
