// @ref LLP 1080.002 — `tree --ax`: the accessibility tree the platform
// exposes, joined to Exact's views, with the four fixed findings over it.
// The web's is read here, over the carrier's CDP pipe (D4); a native host
// answers `{"op":"tree","ax":true}` itself (D2, D3). Either way the driver
// adds the intent of the views it joined and the findings (D7, D8).
// Observations only: Chrome's names and roles are kept as Chrome gives them.

const ROLES_INTERACTIVE = new Set(['button', 'link', 'textbox', 'searchbox', 'checkbox', 'switch', 'slider', 'tab', 'menuitem', 'combobox', 'option']);
const FIELD = 200, BYTES = 256 * 1024, DEPTH = 64;

/** A request's `limit`, validated (D7): an integer 1–2000, 500 when absent. */
export function axLimit(limit) {
  if (limit == null) return 500;
  if (!Number.isInteger(limit) || limit < 1 || limit > 2000) throw new Error('tree --ax: limit is an integer from 1 to 2000');
  return limit;
}

/** Cut a string field at 200 characters, counting the cut. */
function cut(value, truncated) {
  if (value == null) return undefined;
  const s = String(value);
  if (s.length <= FIELD) return s;
  truncated.fields++;
  return s.slice(0, FIELD - 1) + '…';
}

/** The views' own intent, from the plain `tree` the host already answers:
 * ids to `{inert, ariaHidden, handlers}` and each view's parent. */
export function axIntent(tree) {
  const parent = new Map(), facts = new Map();
  for (const n of tree.nodes ?? []) {
    if (n.parent != null) parent.set(n.id, n.parent);
    for (const c of n.children ?? []) parent.set(c, n.id);
    const p = n.props ?? {}, f = {};
    if (p.inert === true || p.inert === 'true') f.inert = true;
    if (p.accessibilityElementsHidden === true || p.accessibilityElementsHidden === 'true') f.ariaHidden = true;
    if (p.accessibilityModal === true || p.accessibilityModal === 'true') f.modal = true;
    if (p.semanticTag === 'dialog') f.dialog = true;
    if (n.handlers?.length) f.handlers = n.handlers;
    if (p.testId != null) f.testId = p.testId;
    facts.set(n.id, f);
  }
  return { parent, facts };
}

/** Chrome's computed tree, the oracle (D2, D4): one stamp, the AX tree, one
 * DOM snapshot, the stamp again; retried twice when the document moved. */
export async function webAx(carrier, opts = {}) {
  if (carrier.browser !== 'chrome' || !carrier.call) return { ax: { unavailable: true, reason: `the Accessibility domain is CDP's; the ${carrier.browser} carrier has none` } };
  const limit = axLimit(opts.limit);
  if (!carrier.axEnabled) { await carrier.call('Accessibility.enable'); carrier.axEnabled = (await carrier.evaluate("navigator.userAgent.match(/Chrome\\/[\\d.]+/)?.[0] ?? 'Chrome'")).replace('/', ' '); }
  let last;
  for (let attempt = 0; attempt < 3; attempt++) {
    const before = await carrier.ask({ op: 'axStamp' });
    if (before.error) return before;
    const [{ nodes }, snapshot] = [await carrier.call('Accessibility.getFullAXTree'), await carrier.call('DOMSnapshot.captureSnapshot', { computedStyles: [], includeDOMRects: true })];
    const after = await carrier.ask({ op: 'axStamp' });
    last = { reply: collectWeb(nodes, snapshot, limit), before };
    if (['epoch', 'incarnation', 'nonce'].every(k => before[k] === after[k])) break;
    last.spanned = true;
  }
  const { reply, before, spanned } = last;
  reply.ax.platform = carrier.axEnabled;
  return { epoch: before.epoch, incarnation: before.incarnation, clock: before.clock, ...(spanned ? { spanned: true } : {}), ...reply };
}

function collectWeb(nodes, snapshot, limit) {
  const truncated = { fields: 0 };
  const doc = snapshot.documents[0], str = i => (i < 0 ? null : snapshot.strings[i]);
  const dom = new Map(); // backendNodeId → {index}
  doc.nodes.backendNodeId.forEach((b, i) => dom.set(b, i));
  const attrs = i => { const a = doc.nodes.attributes[i] ?? [], o = {}; for (let k = 0; k < a.length; k += 2) o[str(a[k])] = str(a[k + 1]); return o; };
  const parentOf = i => doc.nodes.parentIndex[i];
  const isElement = i => doc.nodes.nodeType[i] === 1;
  const stamp = i => { const v = isElement(i) ? attrs(i)['data-agent-view'] : null; return v == null ? null : Number(v); };
  const bounds = new Map();
  doc.layout.nodeIndex.forEach((n, k) => { if (!bounds.has(n)) bounds.set(n, doc.layout.bounds[k]); });
  const sx = doc.scrollOffsetX ?? 0, sy = doc.scrollOffsetY ?? 0;
  const byId = new Map(nodes.map(n => [n.nodeId, n]));
  const root = nodes.find(n => n.parentId == null) ?? nodes[0];
  const elements = [];
  let visited = 0, more = 0, modal = { present: false };
  const prop = (n, name) => n.properties?.find(p => p.name === name)?.value?.value;
  // Depth-first through childIds; an ignored node's children take its place.
  const walk = (n, parent, depth) => {
    visited++;
    const role = n.role?.value;
    const kept = !n.ignored && role !== 'InlineTextBox' && role !== 'RootWebArea';
    let me = parent;
    if (kept) {
      if (elements.length >= limit) { more++; return; }
      const i = elements.length, d = n.backendDOMNodeId != null ? dom.get(n.backendDOMNodeId) : undefined;
      let id = null, via = 'none';
      // A node the snapshot does not carry (a user-agent shadow tree's, an input's inner editor) stands under its AX parent's view.
      if (d === undefined && parent != null && elements[parent].id != null) { id = elements[parent].id; via = 'ancestor'; }
      if (d !== undefined) {
        const own = stamp(d);
        if (own != null) { id = own; via = 'self'; } else {
          const p = parentOf(d);
          if (!isElement(d) && p >= 0 && stamp(p) != null) { id = stamp(p); via = 'owner'; } else {
            for (let a = p; a >= 0; a = parentOf(a)) if (stamp(a) != null) { id = stamp(a); via = 'ancestor'; break; }
          }
        }
      }
      const states = {};
      for (const name of ['disabled', 'selected', 'expanded', 'focused', 'busy', 'modal']) { const v = prop(n, name); if (v != null) states[name] = v === true || v === 'true'; }
      const checked = prop(n, 'checked'); if (checked != null) states.checked = checked === 'mixed' ? 'mixed' : checked === true || checked === 'true';
      const level = prop(n, 'level'); if (level != null) states.level = Number(level);
      const el = d !== undefined && isElement(d) ? attrs(d) : {};
      const tag = d !== undefined ? str(doc.nodes.nodeName[d]) : null;
      if (tag === 'INPUT' && el.type === 'password') states.protected = true;
      const e = { i, parent: parent ?? null, id, via, role: cut(role, truncated) ?? 'unknown', name: cut(n.name?.value ?? '', truncated) };
      const from = n.name?.sources?.find(s => s.value != null && !s.superseded)?.type;
      if (from) e.nameFrom = from;
      const value = n.value?.value;
      if (value != null && value !== '' && !states.protected) e.value = cut(value, truncated);
      if (n.description?.value) e.description = cut(n.description.value, truncated);
      e.states = states;
      e.interactive = ROLES_INTERACTIVE.has(role);
      const b = d !== undefined ? bounds.get(d) : undefined;
      if (b) e.frame = { x: round(b[0] - sx), y: round(b[1] - sy), w: round(b[2]), h: round(b[3]), source: 'layout' };
      e.native = { role, ...(tag ? { class: tag.toLowerCase() } : {}) };
      if (states.modal) modal = { present: true, element: i, id, by: 'dialog:modal' };
      elements.push(e);
      me = i;
    }
    if (depth >= DEPTH * 4) return;
    for (const c of n.childIds ?? []) { const child = byId.get(c); if (child) walk(child, me, depth + 1); }
  };
  if (root) walk(root, null, 0);
  const excluded = snapshot.documents.length > 1 ? snapshot.documents.slice(1).map(d => ({ frame: str(d.documentURL), reason: 'frame-scoped AX tree; the carrier attaches one page target' })) : [];
  const ax = { source: 'chrome-cdp', platform: '', order: 'tree',
    coverage: { roots: ['document'], complete: more === 0, visited, ...(excluded.length ? { excluded } : {}) },
    modal, elements };
  if (more || truncated.fields) ax.truncated = { elements: more, fields: truncated.fields };
  return { ax };
}
const round = x => Math.round(x * 100) / 100;

/** The joined views' intent and the findings (D7, D8), added to any host's
 * reply; with a target, only its subtree's elements and the chain above. */
export function axFinish(reply, tree, target) {
  const ax = reply.ax;
  if (!ax || ax.unavailable) return reply;
  const { parent, facts } = axIntent(tree);
  const ancestorsOf = id => { const out = []; for (let p = parent.get(id); p != null && out.length < DEPTH; p = parent.get(p)) out.push(p); return out; };
  if (target != null) {
    const hit = tree.nodes.find(n => n.id === target || n.props?.testId === target);
    if (!hit) throw new Error(`tree --ax: no view matches ${target}`);
    const inside = new Set([hit.id]);
    for (const n of tree.nodes) if (ancestorsOf(n.id).includes(hit.id)) inside.add(n.id);
    const all = ax.elements, keep = all.filter(e => e.id != null && inside.has(e.id));
    const first = keep[0], chain = [];
    for (let p = first?.parent; p != null; p = all[p].parent) chain.unshift({ id: all[p].id, role: all[p].role, name: all[p].name });
    ax.elements = keep;
    ax.ancestors = chain;
    ax.target = hit.id;
  }
  const intent = {};
  for (const e of ax.elements) {
    if (e.id == null) continue;
    for (const id of [e.id, ...ancestorsOf(e.id)]) if (!(id in intent)) { const f = facts.get(id); if (f) intent[id] = { ...f, ...(parent.has(id) ? { parent: parent.get(id) } : {}) }; }
    if (e.testId == null && facts.get(e.id)?.testId != null) e.testId = facts.get(e.id).testId;
  }
  ax.intent = intent;
  ax.findings = axFindings(reply);
  const size = JSON.stringify(reply).length;
  if (size > BYTES) {
    const keep = Math.max(1, Math.floor(ax.elements.length * BYTES / size));
    (ax.truncated ??= { elements: 0, fields: 0 }).elements = typeof ax.truncated.elements === 'number' ? ax.truncated.elements + ax.elements.length - keep : 'unknown';
    ax.elements = ax.elements.slice(0, keep);
    ax.coverage.complete = false;
  }
  return reply;
}

/** The four fixed findings over one reply (D8); `parity` comes from axParity. */
export function axFindings(reply) {
  const ax = reply.ax, out = [];
  if (!ax?.elements) return out;
  const intent = ax.intent ?? {};
  const hiddenBy = e => {
    if (e.id == null) return null;
    for (let id = e.id, guard = 0; id != null && guard < DEPTH; guard++) {
      const f = intent[id];
      if (f?.inert || f?.ariaHidden) return { id, why: f.inert ? 'inert' : 'aria-hidden' };
      id = intent[id]?.parent ?? null;
    }
    return null;
  };
  const byIndex = new Map(ax.elements.map(e => [e.i, e])), flagged = new Map();
  const inside = (e, root) => { for (let p = e; p; p = byIndex.get(p.parent)) if (p.i === root) return true; return false; };
  for (const e of ax.elements) {
    if (e.excluded?.length) continue;
    // Platform chrome (`via: none`) is named by the platform's own rules (a scroller part by its subrole); only Exact's views are held to a name.
    if (e.via !== 'none' && (e.interactive || ROLES_INTERACTIVE.has(e.role)) && !(e.name ?? '').trim()) out.push({ kind: 'unnamed', i: e.i, id: e.id, testId: e.testId, detail: e.role });
    // One finding per exposed subtree: an element whose parent was flagged under the same view is that finding's.
    const h = hiddenBy(e);
    if (h) flagged.set(e.i, h.id);
    if (h && flagged.get(e.parent) !== h.id) out.push({ kind: 'exposed-hidden', i: e.i, id: e.id, testId: e.testId, detail: `${e.role} "${e.name}" under #${h.id} (${h.why})`, under: h.id });
    // UIKit's rule is the host's to apply (it knows the modal view's siblings); elsewhere, outside the modal element.
    const leaks = ax.source === 'uikit' ? e.outsideModal === true : ax.modal?.present && ax.modal.element != null && e.via !== 'none' && !inside(e, ax.modal.element);
    if (leaks) out.push({ kind: 'outside-modal', i: e.i, id: e.id, testId: e.testId, detail: `${e.role} "${e.name}"`, basis: ax.source === 'uikit' ? 'documented-rule' : 'platform' });
  }
  return out;
}

/** The fixture's control kinds, normalized (D6); anything else passes through. */
export function axRole(e, source) {
  const raw = e.native?.role;
  if (source === 'uikit') {
    const traits = Array.isArray(raw) ? raw : [];
    if (e.native?.class === 'UITextField' || e.native?.class === 'UITextView') return 'textbox';
    if (traits.includes('button') && (e.value === 'checked' || e.value === 'unchecked')) return 'checkbox';
    if (traits.includes('link')) return 'link';
    if (traits.includes('header')) return 'heading';
    if (traits.includes('button')) return 'button';
    return e.role;
  }
  if (source === 'appkit') return { AXButton: 'button', AXLink: 'link', AXHeading: 'heading', AXTextField: 'textbox', AXTextArea: 'textbox', AXCheckBox: 'checkbox' }[raw] ?? e.role;
  return e.role;
}
const CAN = { 'chrome-cdp': ['checked', 'level', 'disabled'], uikit: ['checked', 'disabled'], appkit: ['checked', 'level', 'disabled'] };
const checkedOf = (e, source) => source === 'uikit' && (e.value === 'checked' || e.value === 'unchecked') ? e.value === 'checked' : e.states?.checked;

/** Parity against the web by unique testId (D6): role, name, and the states
 * both hosts can observe. Refuses a truncated or spanned side. */
export function axParity(web, other, testIds) {
  for (const r of [web, other]) if (r.spanned || r.ax?.truncated || r.ax?.coverage?.complete === false) throw new Error('axParity: a side is truncated, spanned or incomplete');
  // The view's own element first; an owned one (its text) only when the view exposes none.
  const index = r => {
    const m = new Map(), dup = new Set();
    for (const via of ['self', 'owner']) for (const e of r.ax.elements) {
      if (e.testId == null || e.via !== via || (via === 'owner' && m.get(e.testId)?.via === 'self')) continue;
      if (m.has(e.testId)) dup.add(e.testId); else m.set(e.testId, e);
    }
    for (const d of dup) m.delete(d);
    return { m, dup };
  };
  const a = index(web), b = index(other), sa = web.ax.source, sb = other.ax.source;
  const findings = [], unjoined = [];
  for (const t of testIds ?? [...a.m.keys()]) {
    const x = a.m.get(t), y = b.m.get(t);
    if (!x || !y) { unjoined.push(t); continue; }
    const rx = axRole(x, sa), ry = axRole(y, sb);
    if (rx !== ry) findings.push({ kind: 'parity', testId: t, detail: `role ${rx} (web) vs ${ry} (${sb})` });
    const nx = (x.name ?? '').trim().replace(/\s+/g, ' '), ny = (y.name ?? '').trim().replace(/\s+/g, ' ');
    if (nx !== ny) findings.push({ kind: 'parity', testId: t, detail: `name ${JSON.stringify(nx)} (web) vs ${JSON.stringify(ny)} (${sb})` });
    for (const s of CAN[sa].filter(s => CAN[sb].includes(s))) {
      const vx = s === 'checked' ? checkedOf(x, sa) : x.states?.[s], vy = s === 'checked' ? checkedOf(y, sb) : y.states?.[s];
      if ((vx ?? false) !== (vy ?? false) && !(s === 'level' && (vx == null || vy == null))) findings.push({ kind: 'parity', testId: t, detail: `${s} ${vx} (web) vs ${vy} (${sb})${vy == null || vx == null ? ' (missing)' : ''}` });
    }
  }
  return { findings, unjoined };
}

/** Whether a read with no findings means anything (D8): complete, untruncated,
 * unspanned, and every expected testId present and joined self or owner. */
export function axClean(reply, expected = []) {
  const ax = reply.ax;
  if (!ax || ax.unavailable || reply.spanned || ax.truncated || !ax.coverage?.complete) return false;
  return expected.every(t => ax.elements.some(e => e.testId === t && (e.via === 'self' || e.via === 'owner')));
}

/** The transcript form (D9): one line per element, then the findings. */
export function renderAx(r) {
  const q = JSON.stringify, ax = r.ax;
  if (!ax || ax.unavailable) return `ax       unavailable: ${ax?.reason ?? 'no reply'}`;
  const tr = ax.truncated ? ` · truncated ${ax.truncated.elements} elements, ${ax.truncated.fields} fields` : '';
  const cov = ax.coverage?.excluded?.length ? ` · coverage excludes ${ax.coverage.excluded.map(x => `${x.frame ?? x.root}: ${x.reason}`).join('; ')}` : ax.coverage?.complete === false ? ' · coverage: incomplete' : '';
  const modal = ax.modal?.present ? ` · modal ${ax.modal.id != null ? '#' + ax.modal.id : '(no view)'} by ${ax.modal.by}` : '';
  const lines = [`ax       ${ax.source}${ax.platform ? ' · ' + ax.platform : ''} · order ${ax.order} · epoch ${r.epoch} · incarnation ${r.incarnation} · clock ${r.clock} ms · ${ax.elements.length} elements${modal}${r.spanned ? ' · spanned' : ''}${tr}${cov}`];
  const depth = new Map();
  const byIndex = new Map(ax.elements.map(e => [e.i, e]));
  for (const e of ax.elements) {
    const d = e.parent != null && depth.has(e.parent) ? depth.get(e.parent) + 1 : 0;
    depth.set(e.i, d);
    // Display only: a text child repeating its parent's name rides on the parent's line.
    const p = byIndex.get(e.parent);
    if (e.role === 'StaticText' && p && p.name === e.name) { depth.set(e.i, d - 1); continue; }
    const st = Object.entries(e.states ?? {}).filter(([, v]) => v !== false).map(([k, v]) => v === true ? k : `${k}=${v}`);
    const join = e.id == null ? ` (${e.native?.class ?? 'platform'})` : ` #${e.id}${e.via === 'owner' ? '^' : e.via === 'ancestor' ? '~' : ''}${e.testId != null ? ` [${e.testId}]` : ''}`;
    const f = e.frame ? ` ${e.frame.x},${e.frame.y} ${e.frame.w}×${e.frame.h}` : '';
    lines.push(`${'  '.repeat(Math.max(0, d))}${e.role} ${q(e.name ?? '')}${e.value != null ? ` value=${q(e.value)}` : ''}${st.length ? ` [${st.join(' ')}]` : ''}${join}${f}${e.excluded?.length ? ` (excluded: ${e.excluded.join(', ')})` : ''}`);
  }
  const findings = ax.findings ?? [];
  if (findings.length) {
    lines.push('findings');
    for (const x of findings) lines.push(`  ! ${x.kind === 'unnamed' ? `unnamed ${x.detail}` : x.kind === 'exposed-hidden' ? 'exposed while hidden:' + ' ' + x.detail : x.kind === 'outside-modal' ? `outside modal: ${x.detail} (${x.basis})` : x.detail} #${x.id ?? '?'}${x.testId != null ? ` [${x.testId}]` : ''}`);
  } else if (!axClean(r)) lines.push('(no findings — coverage incomplete)');
  return lines.join('\n');
}


/** `tree(target, {ax: true})` (D1): the host's or the browser's tree, then
 * the intent and findings from a plain `tree` read at the same epoch. */
export async function axTree(s, target, { limit, excluded, shallow } = {}) {
  if (shallow) throw new Error('tree --ax: shallow is refused with ax');
  axLimit(limit);
  for (let attempt = 0; ; attempt++) {
    const plain = await s.op({ op: 'tree' });
    const reply = s.carrier.host === 'web' ? await webAx(s.carrier, { limit })
      : await s.op({ op: 'tree', ax: true, ...(limit != null ? { limit } : {}), ...(excluded ? { excluded: true } : {}) });
    if (reply.error) throw new Error(`tree: ${reply.error}`);
    const same = reply.epoch === plain.epoch && reply.incarnation === plain.incarnation;
    if (!same && attempt < 2) continue;
    if (!same) reply.spanned = true;
    const t = target == null ? null : typeof target === 'number' || /^\d+$/.test(String(target)) ? Number(target) : target;
    return axFinish(reply, plain, t);
  }
}
