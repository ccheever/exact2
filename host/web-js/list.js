// Virtualized lists on the JS target (LLP 1071 D5): the runner's half of
// LLP 1010 §6 / 1050.000 §6 / 1070, ported from
// `runner/src/instance/collection` — the size index, the window, anchoring,
// the fill limit, edges, one level of nesting and kept positions — over the
// web host's own browser half (`collection-glue.js`, loaded after the first
// paint as the wasm build loads it), which reports the same facts to it.
// Imported by an app's module only when its plan has a virtualized list.
// `scrollIntoView` (LLP 1070.000, into_view.rs) is carried, and Arrange's
// preview (reorder.rs) by reorder.js, which the motion piece loads for a
// reorder drag; not carried (refused at build): a dynamic `virtualized`.
import { sig, effect, scope, end, untracked, write, owner, onEnd, viewId, Views, inflight, After, rev, ticket, journal, Resources, Mutations, unadopted, settled, Refusal, Hosts, exitView } from "./rt.js";

const BOOTSTRAP_ROWS = 16, ESTIMATED = 32, LEAD_SECONDS = 0.25, FAR_VIEWPORTS = 2, KEPT = 4096;
const lead = (port, v) => { const extra = Math.min(Math.abs(v) * LEAD_SECONDS, port * 2); return v > 0 ? [port, port + extra] : [port + extra, port]; };
const same = Object.is;

// ---------------------------------------------------------------- the size index (index.rs)
// A sum tree over row heights; `me` is each leaf's measured epoch (0: an
// estimate), its nodes the minimum, so a band's measurement is O(log N).
function tree(h, me) {
  let base = 1; while (base < Math.max(1, h.length)) base *= 2;
  const t = { sums: new Float64Array(base * 2), me: new Float64Array(base * 2).fill(Infinity), base, len: h.length, zeros: 0 };
  for (let i = 0; i < h.length; i++) { t.sums[base + i] = h[i]; t.me[base + i] = me[i]; if (h[i] === 0) t.zeros++; }
  for (let n = base - 1; n >= 1; n--) { t.sums[n] = t.sums[2 * n] + t.sums[2 * n + 1]; t.me[n] = Math.min(t.me[2 * n], t.me[2 * n + 1]); }
  if (!isFinite(t.sums[1])) throw new Refusal("collection height exceeds finite geometry");
  return t;
}
function split(t, n, before, after) { const l = t.sums[2 * n]; return l === 0 ? before : t.sums[2 * n + 1] === 0 ? after : Math.min(before + l, after); }
function prefixT(t, end) {
  if (end === t.len) return t.sums[1];
  let n = 1, span = t.base, rem = end, before = 0, after = t.sums[1];
  while (n < t.base) { span /= 2; const mid = split(t, n, before, after); n *= 2; if (rem >= span) { before = mid; n++; rem -= span; } else after = mid; }
  return before;
}
function find(t, offset, inclusive) {
  const total = t.sums[1];
  if (!t.len || total === 0) return null;
  const has = b => inclusive ? b >= offset : b > offset;
  if (!has(total)) return null;
  let n = 1, before = 0, after = total;
  while (n < t.base) { const b = split(t, n, before, after); n *= 2; if (has(b)) after = b; else { before = b; n++; } }
  const i = n - t.base;
  return i < t.len ? i : null;
}
function setT(t, i, h) {
  let n = t.base + i, sum = h;
  while (n > 1) { sum += t.sums[n ^ 1]; n >>= 1; }
  if (!isFinite(sum)) throw new Refusal("collection height exceeds finite geometry");
  n = t.base + i;
  t.zeros += (h === 0) - (t.sums[n] === 0);
  t.sums[n] = h;
  while (n > 1) { n >>= 1; t.sums[n] = t.sums[2 * n] + t.sums[2 * n + 1]; }
}
function setEpoch(t, i, e) { let n = t.base + i; t.me[n] = e; while (n > 1) { n >>= 1; t.me[n] = Math.min(t.me[2 * n], t.me[2 * n + 1]); } }
function minEpoch(t, a, b) {
  let l = a + t.base, r = b + t.base, e = Infinity;
  while (l < r) { if (l & 1) e = Math.min(e, t.me[l++]); if (r & 1) e = Math.min(e, t.me[--r]); l >>= 1; r >>= 1; }
  return e;
}
function positive(t, band, out, n = 1, s = 0, e = t.base) {
  if (t.sums[n] === 0 || e <= band[0] || s >= band[1]) return out;
  if (n >= t.base) { const last = out.at(-1); if (last && last[1] === s) last[1] = e; else out.push([s, e]); return out; }
  const m = s + (e - s) / 2;
  positive(t, band, out, 2 * n, s, m);
  return positive(t, band, out, 2 * n + 1, m, e);
}

class SizeIndex {
  constructor(est) { this.est = est; this.order = []; this.pos = new Map(); this.h = []; this.gen = []; this.me = []; this.epoch = 1; this.next = 0; this.t = tree([], []); }
  get len() { return this.h.length; }
  replace(keys) {
    if (keys.length === this.order.length && keys.every((k, i) => k === this.order[i])) return;
    const pos = new Map(), h = [], gen = [], me = [];
    keys.forEach((k, i) => {
      if (pos.has(k)) throw new Refusal(`duplicate collection key: ${k}`);
      pos.set(k, i);
      const o = this.pos.get(k);
      if (o !== undefined) { h.push(this.h[o]); gen.push(this.gen[o]); me.push(this.me[o]); }
      else { h.push(this.est); gen.push(++this.next); me.push(0); }
    });
    this.t = tree(h, me);
    Object.assign(this, { order: keys, pos, h, gen, me });
  }
  prefix(end) { return prefixT(this.t, end); }
  get total() { return this.t.sums[1]; }
  rowAt(offset) { return find(this.t, offset, false); }
  token(key) { const i = this.pos.get(key); return i === undefined ? null : this.epoch + ":" + this.gen[i]; }
  measured(key) { const i = this.pos.get(key); return i !== undefined && this.me[i] === this.epoch; }
  rangeMeasured(a, b) { return a < b && minEpoch(this.t, a, b) >= this.epoch; }
  invalidateAll() { this.epoch++; }
  invalidateRow(key) { const i = this.pos.get(key); this.gen[i] = ++this.next; this.me[i] = 0; setEpoch(this.t, i, 0); return this.token(key); }
  spread(a, b, delta) {
    if (!isFinite(delta) || Math.abs(delta) < 0.01) return;
    const open = [];
    for (let i = a; i < Math.min(b, this.len); i++) if (this.me[i] !== this.epoch) open.push(i);
    if (!open.length) return;
    const part = delta / open.length;
    for (const i of open) { const h = Math.max(0, this.h[i] + part); setT(this.t, i, h); this.h[i] = h; }
  }
  setMeasured(key, token, h) {
    if (!(isFinite(h) && h >= 0)) throw new Refusal("row height must be finite and nonnegative");
    if (this.token(key) !== token) return false;
    const i = this.pos.get(key);
    setT(this.t, i, h); this.h[i] = h; this.me[i] = this.epoch; setEpoch(this.t, i, this.epoch);
    return true;
  }
  maxOffset(port) { return Math.max(0, this.total - port); }
  clamp(offset, port) { return Math.max(0, Math.min(offset, this.maxOffset(port))); }
  band(s, e) {
    const first = find(this.t, s, false) ?? this.len;
    if (s >= e) return [first, first];
    const last = find(this.t, e, true);
    return [first, last === null ? this.len : last + 1];
  }
  window(offset, port, ld = [port, port], pins = []) {
    offset = this.clamp(offset, port);
    const end = Math.min(offset + port, this.total), visible = this.band(offset, end);
    const overscan = this.band(Math.max(0, offset - ld[0]), Math.min(end + ld[1], this.total));
    const ranges = overscan[0] < overscan[1] ? (this.t.zeros ? positive(this.t, overscan, []) : [overscan.slice()]) : [];
    for (const k of pins) { const i = k == null ? undefined : this.pos.get(k); if (i !== undefined) ranges.push([i, i + 1]); }
    ranges.sort((a, b) => a[0] - b[0]);
    const segments = [];
    for (const r of ranges) { const last = segments.at(-1); if (last && r[0] <= last[1]) last[1] = Math.max(last[1], r[1]); else segments.push(r.slice()); }
    return { offset, visible, overscan, segments };
  }
  anchor(offset, port, follow) {
    offset = this.clamp(offset, port);
    const row = find(this.t, offset, false);
    return { order: this.order, row, within: row === null ? 0 : Math.max(0, offset - this.prefix(row)), follows: follow && port > 0 && this.maxOffset(port) - offset <= 0.5 };
  }
  anchorAt(key, within) { const i = this.pos.get(key); return i === undefined ? null : { order: this.order, row: i, within, follows: false }; }
  restoreAnchor(a, port) {
    const max = this.maxOffset(port);
    if (a.follows) return max;
    if (a.row === null) return 0;
    let i = this.pos.get(a.order[a.row]);
    if (i === undefined) for (const k of [...a.order.slice(a.row + 1), ...a.order.slice(0, a.row).reverse()]) if ((i = this.pos.get(k)) !== undefined) break;
    return i === undefined ? 0 : Math.min(this.prefix(i) + a.within, max);
  }
}

// ---------------------------------------------------------------- DOM pieces (views.rs)
// The wrapper and spacer as the web host styles them (css.rs of the kernel's rows).
const WRAP = { y: "display:flex;flex-direction:column;flex-shrink:0;min-width:0;width:100%;box-sizing:border-box",
  x: "display:flex;flex-direction:column;flex-grow:0;flex-shrink:0;min-height:0;box-sizing:border-box" };
const SPACER = { y: "flex-shrink:0;width:100%", x: "flex-grow:0;flex-shrink:0;align-self:stretch" };
const MAIN = { y: "height", x: "width" };
// css.rs `num`: an f32, whole as an integer, else its shortest decimal.
function num(n) {
  const f = Math.fround(n);
  if (Number.isInteger(f) && Math.abs(f) < 1e9) return String(f);
  for (let p = 1; p < 10; p++) { const s = Number(f.toPrecision(p)); if (Math.fround(s) === f) return String(s); }
  return String(f);
}
function keyText(k) {
  if (typeof k === "string") return "s:" + k;
  if (typeof k === "number" && isFinite(k)) return "n:" + String(k === 0 ? 0 : k);
  if (typeof k === "boolean") return "b:" + k;
  throw new Refusal("a collection key is a string, a finite number or a bool");
}

// ---------------------------------------------------------------- collections
/** Every mounted collection by its list's view id. */
const Lists = new Map();
let Controller = null, Loading = null, Published = "";
const Deferred = []; // [collection, targets]: an end edge waits for the first edge's requests (runner/collection.rs)

class Collection {
  constructor(el, o, own) {
    this.el = el; this.view = viewId(el); this.axis = o.x ? "x" : "y"; this.own = own; this.o = o;
    this.est = o.est ?? ESTIMATED;
    if (!(isFinite(this.est) && this.est > 0)) throw new Refusal("estimated item height must be positive and finite");
    this.index = new SizeIndex(this.est);
    // The list's literal size along its axis bounds an inner list's first rows.
    const sizes = (o.x ? o.pw : o.ph).filter(n => typeof n === "number" && isFinite(n) && n > 0);
    this.port = sizes.length ? Math.min(...sizes) : null;
    this.bootstrap = Math.max(1, Math.min(BOOTSTRAP_ROWS, Math.min(Math.ceil(BOOTSTRAP_ROWS * ESTIMATED / this.est), o.inRow && this.port ? Math.ceil(this.port / this.est) + 1 : Infinity)));
    Object.assign(this, { items: [], idents: [], dups: new Map(), mounted: [], spacers: [], children: [], revision: 0, nextEpoch: 0,
      zeros: new Set(), geometry: null, correction: null, followEnd: false, edgeArmed: [true, true], pending: false, parent: null,
      kept: new Map(), manual: !!o.manual, restored: false, restoredAt: null, startOffset: 0, inner: [], target: null, status: null, preview: null });
    this.edges = [o.start, o.end];
  }
  snapshot() {
    const g = this.geometry;
    return { view: this.view, axis: this.axis, ...(this.parent != null ? { parent: this.parent } : {}), ...(this.restored ? { restored: true } : {}), ...(this.target ? { seeking: true } : {}),
      revision: this.revision, scrollSequence: g ? g.scroll_sequence : 0, count: this.index.len, totalExtent: this.index.total,
      rows: this.mounted.map(m => ({ view: m.view, root: m.root, index: m.position, start: this.index.prefix(m.position), size: this.index.h[m.position], epoch: m.epoch, measured: this.index.measured(this.index.order[m.position]) })),
      pending: this.pending || !!this.target, correction: this.correction }; // an into-view request wants its next report
  }
  // ------------------------------------------------ data (mod.rs update_data)
  update(items, [idents, dups], fresh) {
    const anchor = this.anchor(), last = this.index.order.at(-1);
    const compare = !fresh && items.length === this.items.length;
    const rekeyed = !compare || idents.some((k, i) => k !== this.idents[i]);
    let inPlace = null;
    if (compare && !rekeyed) { inPlace = []; items.forEach((it, p) => { if (!same(it, this.items[p])) inPlace.push(p); }); }
    if (rekeyed) { this.index.replace(idents); this.idents = idents; this.dups = dups; }
    if (this.preview && (rekeyed || !inPlace || inPlace.length)) this.endPreview();
    this.items = items;
    if (this.kept.size) for (const k of [...this.kept.keys()]) if (!this.index.pos.has(k.split("\0")[0])) this.kept.delete(k);
    const previous = inPlace && JSON.stringify(this.snapshot());
    if (inPlace) this.invalidateRows(inPlace); else this.invalidateEstimates();
    if (!this.index.len) this.edgeArmed = [true, true];
    // Rows appended past the end are a new end: the old last row is still
    // here with rows after it, so it re-arms (LLP 1010, the 2026-09-29
    // ruling, provisional). An empty page, a replaced last row or a window
    // that slid past it re-arm nothing.
    if (last !== undefined && this.index.pos.get(last) < this.index.len - 1) this.edgeArmed[1] = true;
    this.restore(anchor);
    this.realize(true, {});
    const now = this.snapshot();
    if (!previous || previous !== JSON.stringify(now)) this.revision++;
  }
  invalidateEstimates() {
    for (const key of this.zeros) { const t = this.index.token(key); if (t) this.index.setMeasured(key, t, this.est); }
    this.zeros.clear();
    this.index.invalidateAll();
  }
  invalidateRows(positions) {
    for (const p of positions) {
      const key = this.index.order[p];
      if (this.zeros.delete(key)) { const t = this.index.token(key); if (t) this.index.setMeasured(key, t, this.est); }
      this.index.invalidateRow(key);
    }
  }
  anchor() { const g = this.geometry; return g && this.index.anchor(g.offset, g.port_main, this.followEnd); }
  restore(a) {
    const g = this.geometry;
    if (!a || !g) return;
    const c = this.index.restoreAnchor(a, g.port_main);
    if (Math.abs(c - g.offset) > 0.01) {
      this.correction = { scrollSequence: g.scroll_sequence, offset: c };
      g.offset = c;
      if (this.restoredAt) this.startOffset = c;
    }
  }
  // ------------------------------------------------ pins (nest.rs)
  pins() {
    const g = this.geometry, out = g ? [g.focus_view, g.interaction_view] : [null, null];
    for (const m of this.mounted) for (const c of m.inner) if (c.geometry) { out[0] ??= c.geometry.focus_view; out[1] ??= c.geometry.interaction_view; }
    return out;
  }
  pin(view) {
    if (view == null) return null;
    const el = Views.get(view);
    const m = this.mounted.find(m => m.view === view || (el && m.wrapper.contains(el)));
    return m ? m.key : null;
  }
  // ------------------------------------------------ the window (mod.rs realize_window)
  realize(update, fill) {
    const limit = update ? null : fill.limit ?? null, g = this.geometry, owed = [];
    let port = null, ranges;
    if (g) {
      const pins = this.pins(), focus = this.pin(pins[0]), interaction = this.pin(pins[1]);
      const w = this.index.window(g.offset, g.port_main, lead(g.port_main, fill.velocity ?? 0), [focus, interaction]);
      owed.push(w.visible);
      for (const k of [focus, interaction]) { const i = k == null ? undefined : this.index.pos.get(k); if (i !== undefined) owed.push([i, i + 1]); }
      port = [w.offset, w.offset + g.port_main];
      ranges = w.segments;
    } else {
      const first = this.index.rowAt(this.startOffset) ?? 0;
      ranges = [[first, Math.min(this.index.len, first + this.bootstrap)]];
    }
    const old = new Map(this.mounted.map(m => [m.key, m]));
    this.mounted = [];
    const limited = limit !== null && port !== null;
    const isOwed = p => owed.some(r => p >= r[0] && p < r[1]);
    const toward = (fill.velocity ?? 0) < 0;
    let pending = false;
    const admitted = new Set();
    if (limited) {
      const optional = [];
      for (const [a, b] of ranges) for (let p = a; p < b; p++) if (!isOwed(p) && !old.has(this.index.order[p])) { const [before, d] = this.distance(p, port); optional.push([before !== toward, d, p]); }
      optional.sort((a, b) => a[0] - b[0] || a[1] - b[1] || a[2] - b[2]);
      pending = optional.length > limit;
      for (const x of optional.slice(0, limit)) admitted.add(x[2]);
    }
    for (const [a, b] of ranges) for (let p = a; p < b; p++) {
      const key = this.index.order[p];
      let m = old.get(key);
      if (m) { old.delete(key); this.reposition(m, p); }
      else {
        if (limited && !isOwed(p) && !admitted.has(p)) continue;
        const token = this.index.invalidateRow(key);
        m = this.createRow(p, key);
        m.token = token; m.epoch = ++this.nextEpoch;
      }
      this.settle(m);
    }
    const leaving = [];
    for (const [key, m] of old) {
      const p = this.index.pos.get(key);
      if (limited && p !== undefined) leaving.push([this.distance(p, port)[1], key, m]);
      else { if (p !== undefined) this.keep(m); this.retire(m, p === undefined); }
    }
    if (limited) {
      let cap = limit === 0 ? 0 : Math.max(2 * limit, 4);
      cap = Math.max(cap, leaving.length - this.mounted.length);
      leaving.sort((a, b) => b[0] - a[0]);
      let far = 0; while (far < leaving.length && leaving[far][0] > FAR_VIEWPORTS * (port[1] - port[0])) far++;
      const kept = leaving.splice(Math.min(Math.max(far, cap), leaving.length));
      for (const [, , m] of leaving) { this.keep(m); this.retire(m); }
      pending ||= kept.length > 0;
      for (const [, key, m] of kept) { this.reposition(m, this.index.pos.get(key)); this.settle(m); }
      this.mounted.sort((a, b) => a.position - b.position);
    }
    this.pending = pending;
    this.emit();
    if (this.preview) this.emitPreview();
  }
  distance(p, [top, end]) {
    const start = this.index.prefix(p), finish = start + this.index.h[p];
    return finish <= top ? [true, top - finish] : [false, Math.max(0, start - end)];
  }
  reposition(m, p) { m.position = p; write(m.item.n, this.items[p]); write(m.index.n, p); }
  settle(m) {
    const count = this.index.len;
    if (m.published[0] !== m.position || m.published[1] !== count) {
      m.wrapper.setAttribute("aria-posinset", m.position + 1); m.wrapper.setAttribute("aria-setsize", count);
      m.published = [m.position, count];
    }
    const t = this.index.token(m.key);
    if (t !== m.token) { m.token = t; m.epoch = ++this.nextEpoch; }
    this.mounted.push(m);
  }
  createRow(p, key) {
    const w = document.createElement("div");
    w.style.cssText = WRAP[this.axis];
    w.setAttribute("role", "listitem");
    w.setAttribute("data-listitemkey", key);
    const m = { key, position: p, wrapper: w, item: sig(this.items[p]), index: sig(p), published: [-1, -1], inner: [], list: this };
    // The row's scope names its row, for a list inside it (at any time).
    m.s = unadopted(() => scope(() => { owner().$row = m; this.o.row(w, m.item, m.index); }, this.own));
    m.view = viewId(w); w.dataset.view = m.view;
    m.root = w.firstElementChild ? viewId(w.firstElementChild) : m.view;
    if (this.dups.get(p)) journal.push(`list: a key repeats; this row is ${key}`);
    this.adoptNested(m);
    return m;
  }
  // A row whose item left the data leaves as its wrapper, where the window
  // placed it, with its root's exit animation (kernel txn.rs `leaving_with`);
  // one the window scrolled away, at once.
  retire(m, left) {
    end(m.s);
    if (!(left && exitView(m.wrapper, m.wrapper.firstElementChild?.style.getPropertyValue("--exact-exit-animation").trim()))) m.wrapper.remove();
    Views.delete(m.view); Views.delete(m.root);
  }
  emit() {
    const kids = [];
    let cursor = 0, n = 0;
    for (let i = 0; i <= this.mounted.length; i++) {
      const p = i < this.mounted.length ? this.mounted[i].position : this.index.len;
      const gap = this.index.prefix(p) - this.index.prefix(cursor);
      if (gap > 0) {
        let s = this.spacers[n];
        if (!s) { const el = document.createElement("div"); el.setAttribute("aria-hidden", "true"); el.style.cssText = SPACER[this.axis]; s = this.spacers[n] = { el, size: null }; }
        if (s.size !== gap) { s.el.style.setProperty(MAIN[this.axis], num(gap) + "px"); s.size = gap; }
        kids.push(s.el); n++;
      }
      if (i < this.mounted.length) { kids.push(this.mounted[i].wrapper); cursor = p + 1; }
    }
    for (const s of this.spacers.splice(n)) s.el.remove();
    if (kids.length === this.children.length && kids.every((k, i) => k === this.children[i])) return;
    // In place, as glue.js's `children` op: kept rows keep their elements.
    const el = this.el;
    // A leaving row stays where it was until its animation ends (moving it would cancel it).
    const past = n => { while (n?.hasAttribute("data-exiting")) n = n.nextElementSibling; return n; };
    let at = past(el.firstElementChild);
    for (const k of kids) { if (k === at) { at = past(at.nextElementSibling); continue; } el.insertBefore(k, at); }
    while (at) { const next = past(at.nextElementSibling); at.remove(); at = next; }
    this.children = kids;
  }
  // ------------------------------------------------ nesting (nest.rs)
  keep(m) {
    for (const c of m.inner) {
      if (c.manual) continue;
      const slot = m.key + "\0" + c.site;
      this.kept.delete(slot);
      const at = c.position();
      if (!at) continue;
      this.kept.set(slot, at);
      while (this.kept.size > KEPT) this.kept.delete(this.kept.keys().next().value);
    }
  }
  position() {
    const g = this.geometry;
    if (!g) {
      if (!this.restoredAt) return null;
      const i = this.index.pos.get(this.restoredAt[0]);
      return i === undefined ? null : [this.restoredAt[0], this.restoredAt[1], this.index.prefix(i)];
    }
    const offset = Math.min(Math.max(g.offset, 0), this.index.maxOffset(g.port_main));
    if (offset <= 0) return null;
    const row = this.index.rowAt(offset);
    if (row === null) return null;
    const start = this.index.prefix(row);
    return [this.index.order[row], offset - start, start];
  }
  adoptNested(m) {
    const g = this.geometry;
    for (const c of m.inner) {
      c.parent = this.view;
      let again = false;
      if (g) {
        let estimate = c.axis === this.axis ? g.port_main : g.cross;
        if (c.port) estimate = Math.min(c.port, estimate);
        const rows = Math.max(1, Math.min(64, Math.ceil(estimate / c.est) + 1));
        if (rows > c.bootstrap && !c.geometry) { c.bootstrap = rows; again = true; }
      }
      const kept = !c.manual && this.kept.get(m.key + "\0" + c.site);
      if (kept) c.restorePosition(...kept);
      else if (again) c.realize(false, {});
    }
  }
  restorePosition(key, within, start) {
    const p = this.index.pos.get(key);
    if (p === undefined) return;
    this.index.spread(0, p, start - this.index.prefix(p));
    const offset = this.index.prefix(p) + within;
    this.startOffset = offset; this.restored = true; this.restoredAt = [key, within];
    this.correction = { scrollSequence: 0, offset };
    this.realize(false, {});
    this.revision++;
  }
  restoring(f) {
    if (!this.restoredAt) return null;
    const [key, within] = this.restoredAt;
    const moved = Math.abs(f.offset - this.startOffset) > 0.5 && f.offset > 0.5;
    if (moved || this.index.measured(key)) this.restoredAt = null;
    return moved ? null : this.index.anchorAt(key, within);
  }
  contains(c) { for (let p = c.up; p; p = p.up) if (p === this) return true; return false; }
  releasePins(cats) {
    if (cats[1] && this.preview) this.losePreviewPin();
    const g = this.geometry;
    if (!g || !((cats[0] && g.focus_view != null) || (cats[1] && g.interaction_view != null))) return false;
    if (cats[0]) g.focus_view = null;
    if (cats[1]) g.interaction_view = null;
    this.realize(false, {});
    this.revision++;
    return true;
  }
  // ------------------------------------------------ feedback (mod.rs)
  prepare(f) {
    const g = this.geometry;
    if (f.view !== this.view || f.revision !== this.revision || (g && f.scroll_sequence < g.scroll_sequence)) return null;
    if ([f.focus_view, f.interaction_view].some(v => v != null && this.pin(v) === null)) return null;
    const byView = new Map(this.mounted.map((m, i) => [m.view, i]));
    if (f.measurements.some(m => !byView.has(m.view) || this.mounted[byView.get(m.view)].epoch !== m.epoch)) return null;
    return byView;
  }
  dims(f) { const g = this.geometry; return g && g.port_cross === f.port_cross && g.port_main === f.port_main && g.cross === f.cross && g.focus_view === f.focus_view && g.interaction_view === f.interaction_view; }
  measure(byView, r) {
    const m = this.mounted[byView.get(r.view)], key = this.index.order[m.position];
    this.index.setMeasured(key, m.token, r.size);
    if (r.size === 0) this.zeros.add(key); else this.zeros.delete(key);
  }
  feedback(f, byView, fill) {
    const within = this.travelWithin(f, byView, fill);
    if (within !== undefined) return [false, within];
    const changedWidth = !this.geometry || this.geometry.cross !== f.cross;
    // Arrange's preview (reorder.js, loaded with the motion piece) ends with
    // the row width and loses its pin with the contact (mod.rs `feedback`).
    if (this.preview && changedWidth) this.endPreview();
    if (this.preview && this.geometry.interaction_view !== f.interaction_view) this.losePreviewPin();
    if (!this.dims(f)) fill.limit = null;
    if (fill.ancestorMoving) fill.limit = 0;
    const previous = this.snapshot();
    this.followIntoView(fill);
    let measurements = f.measurements;
    if (this.axis === "x" && !changedWidth) {
      const first = [];
      for (const r of measurements) { const m = this.mounted[byView.get(r.view)]; if (this.index.measured(this.index.order[m.position])) this.measure(byView, r); else first.push(r); }
      measurements = first;
    }
    const height = this.geometry ? this.geometry.port_main : f.port_main;
    const anchor = this.restoring(f) ?? this.index.anchor(f.offset, height, this.followEnd);
    this.geometry = { ...f, measurements: [] };
    this.correction = null;
    if (changedWidth) this.invalidateEstimates();
    else for (const r of measurements) this.measure(byView, r);
    if (this.preview) this.checkPreviewHeight();
    this.restore(anchor);
    this.settleIntoView(f.offset);
    this.realize(false, fill);
    const now = this.snapshot();
    now.scrollSequence = previous.scrollSequence;
    const changed = JSON.stringify(previous) !== JSON.stringify(now);
    if (changed) this.revision++;
    return [changed, this.edge()];
  }
  travelWithin(f, byView, fill) {
    const g = this.geometry;
    if (!g) return undefined;
    const remeasures = r => {
      const m = this.mounted[byView.get(r.view)], key = this.index.order[m.position];
      return this.index.token(key) === m.token && !(this.index.measured(key) && this.index.h[m.position] === r.size && (r.size === 0) === this.zeros.has(key));
    };
    if (f.measurements.some(remeasures) || this.restoredAt || this.target || this.pending || this.correction || !this.dims(f)) return undefined;
    const a = this.index.anchor(f.offset, g.port_main, this.followEnd);
    if (Math.abs(this.index.restoreAnchor(a, g.port_main) - f.offset) > 0.01) return undefined;
    const pins = this.pins();
    const w = this.index.window(f.offset, f.port_main, lead(f.port_main, fill.velocity ?? 0), [this.pin(pins[0]), this.pin(pins[1])]);
    let k = 0;
    for (const [a2, b] of w.segments) for (let p = a2; p < b; p++) if (this.mounted[k++]?.position !== p) return undefined;
    if (k !== this.mounted.length) return undefined;
    this.geometry = { ...f, measurements: [] };
    return this.edge();
  }
  // ------------------------------------------------ scrollIntoView (into_view.rs)
  /** A key's row identity here, or why it is refused (LLP 1070.000 §2.2);
   * the agent types a key as text, which a list keyed by numbers reads as one. */
  resolve(key) {
    const exactly = k => {
      let text; try { text = keyText(k); } catch { return [null, "the key is not a string, number or bool"]; }
      if (!this.index.pos.has(text)) return [null, `no row keyed ${text} in the list`];
      if (this.index.pos.has("d1:" + text)) return [null, `the list holds ${text} more than once`];
      return [text];
    };
    if (typeof key === "string" && key.trim() !== "" && isFinite(Number(key))) { const r = exactly(Number(key)); if (r[0]) return r; }
    return exactly(key);
  }
  /** The row root's margins on this axis: its wrapper encloses them, and the
   * web aligns the element's border box. */
  margins(p) {
    const m = this.mounted.find(m => m.position === p), root = m?.wrapper.firstElementChild;
    if (!root) return [0, 0];
    const cs = getComputedStyle(root), n = v => parseFloat(v) || 0;
    return this.axis === "y" ? [n(cs.marginTop), n(cs.marginBottom)] : [n(cs.marginLeft), n(cs.marginRight)];
  }
  aligned(p, align, current) {
    const [before, after] = this.margins(p);
    const start = this.index.prefix(p) + before, size = Math.max(0, this.index.h[p] - before - after), port = this.geometry?.port_main ?? 0;
    const at = align === "start" ? start : align === "center" ? start + size / 2 - port / 2 : align === "end" ? start + size - port
      : start < current ? start : start + size > current + port ? (size > port ? start : start + size - port) : current;
    return Math.min(Math.max(at, 0), Math.max(this.index.total - port, 0));
  }
  /** Start a request: its window is built at the destination now, and the
   * host told to move there before it paints (the correction). */
  intoView(key, align) {
    const p = this.index.pos.get(key), g = this.geometry;
    const offset = this.aligned(p, align, g ? g.offset : this.startOffset);
    this.restoredAt = null;
    this.target = { key, align, reports: 0, travelling: 0, aligned: 0 };
    this.status = [key, "pending"];
    if (g) { g.offset = offset; this.correction = { scrollSequence: g.scroll_sequence, offset }; }
    else { this.startOffset = offset; this.correction = { scrollSequence: 0, offset }; }
    this.realize(false, {});
    this.revision++;
  }
  end(status) { this.status = [this.target.key, status]; this.target = null; }
  /** The reader's own travel, two reports running, cancels it. */
  followIntoView(fill) {
    const t = this.target;
    if (!t) return;
    t.travelling = (fill.velocity ?? 0) !== 0 ? t.travelling + 1 : 0;
    if (t.travelling >= 2) this.end("cancelled");
  }
  /** After a report's measurements: where the target now aligns, judged
   * where the host says the port is; done when it holds for two reports
   * with every row up to it measured, else corrected, six times at most. */
  settleIntoView(reported) {
    const t = this.target, g = this.geometry;
    if (!t) return;
    const p = this.index.pos.get(t.key);
    if (p === undefined) return this.end("cancelled");
    if (!g) return;
    const desired = this.aligned(p, t.align, reported);
    const first = Math.min(this.mounted[0]?.position ?? p, p);
    if (Math.abs(desired - reported) <= 0.5) {
      t.aligned = this.index.rangeMeasured(first, p + 1) && !this.pending ? t.aligned + 1 : 0;
      if (t.aligned >= 2) this.end("done");
      return;
    }
    t.aligned = 0;
    if (t.reports >= 6) return this.end("unconverged");
    g.offset = desired;
    this.correction = { scrollSequence: g.scroll_sequence, offset: desired };
    t.reports++;
  }
  edge() {
    const reached = [false, false], g = this.geometry;
    if (g && this.index.len) {
      const w = this.index.window(g.offset, g.port_main);
      const n = this.index.len - 1;
      reached[0] = w.segments.some(r => 0 >= r[0] && 0 < r[1]);
      reached[1] = w.segments.some(r => n >= r[0] && n < r[1]);
      if (!(this.edgeArmed[0] && this.edgeArmed[1]) && this.index.rangeMeasured(...w.overscan)) for (const i of [0, 1]) this.edgeArmed[i] ||= !reached[i];
    }
    const ready = [0, 1].map(i => reached[i] && this.edgeArmed[i] && !!this.edges[i]);
    const i = ready.indexOf(true);
    if (i < 0) return null;
    this.edgeArmed[i] = false;
    return { first: i, endAfterNoop: ready[0] && ready[1] };
  }
}

// ---------------------------------------------------------------- the seam
/** A virtualized `list`'s rows: `o` carries the key and row builders, the
 * edges' handlers, and what the runner reads when the list is created. */
export function vl(el, subject, key, row, o) {
  const own = owner();
  let outer = null;
  for (let s = own; s && !outer; s = s.up) outer = s.$row;
  const c = new Collection(el, { ...o, key, row, inRow: !!outer }, own);
  if (outer) { outer.inner.push(c); c.up = outer.list; c.site = o.site + "/" + outer.inner.length; }
  Lists.set(c.view, c);
  el.$list = c;
  // A rendered page's rows are rebuilt: the window is the runner's.
  while (el.firstChild) el.firstChild.remove();
  el.$n = null;
  el.$jump = (name, at) => { if (el[name] !== at) (jumps ??= []).push([c.view, at, name]); };
  onEnd(() => Lists.delete(c.view));
  let first = true;
  if (o.follow) effect(() => { c.followEnd = !!o.follow(); });
  // The keys are read here, so a key's other inputs rerun the update too.
  effect(() => {
    const items = subject(), idents = [], dups = new Map(), unique = new Set();
    items.forEach((item, i) => {
      const text = keyText(key(() => item, () => i));
      let d = 0, id = text;
      while (unique.has(id)) id = "d" + ++d + ":" + text;
      unique.add(id);
      if (d) dups.set(i, d);
      idents.push(id);
    });
    untracked(() => { c.update(items, [idents, dups], first); first = false; });
  });
  load();
}
let jumps = null;
const ALIGN = ["start", "center", "end", "nearest"];
const Refused = [];
/** The command (`scrollIntoView("id", key, block=, inline=, behavior=,
 * row=)`) and the agent's `tap <list> into <key>` (`view`), as the runner's
 * `scroll_into_view` (runner/src/runner/into_view.rs). */
function intoView(list, key, block, inline, row, view) {
  const refuse = why => { journal.push(`scrollIntoView ${list} refused: ${why}`); Refused.push({ list, status: "refused: " + why.replaceAll('"', "'") }); if (Refused.length > 8) Refused.shift(); };
  for (const a of [block, inline]) if (!ALIGN.includes(a)) { if (view != null) throw new Error(`no alignment ${a}: start, center, end or nearest`); return refuse(`no alignment ${a}`); }
  const begin = (c, k) => { const [text, why] = c.resolve(k); if (!text) return refuse(why); c.intoView(text, c.axis === "y" ? block : inline); return true; };
  if (view != null) { const c = Lists.get(view); if (!c) return refuse(`view ${view} is not a mounted virtualized list`); begin(c, key); }
  else if (row == null) { const c = [...Lists.values()].find(c => !c.up && c.el.id === list); if (!c) return refuse(`no virtualized list has id ${list}`); begin(c, key); }
  else {
    const outer = [...Lists.values()].find(c => !c.up && c.resolve(row)[0]);
    if (!outer) { let t = ""; try { t = keyText(row); } catch {} return refuse(`no virtualized list holds a row keyed ${t}`); }
    if (!begin(outer, row)) return;
    const m = outer.mounted.find(m => m.key === outer.resolve(row)[0]);
    if (!m) return refuse("the outer row did not mount");
    const c = m.inner.find(c => c.el.id === list);
    if (!c) return refuse(`the row holds no list with id ${list}`);
    begin(c, key);
  }
  settled();
}
Hosts.scrollIntoView = (list, key, block, inline, behavior, row) => intoView(list, key, block ?? "start", inline ?? "nearest", row);
/** `state.scrollIntoView`: each list's latest request, then refusals. */
const intoViewState = () => [...[...Lists.values()].filter(c => c.status).map(c => ({ list: c.view, key: c.status[0], status: c.status[1] })), ...Refused];
function publish() {
  // As the web host sends `collections`: only when a snapshot changed.
  const snapshots = [...Lists.values()].sort((a, b) => a.view - b.view).map(c => c.snapshot());
  const text = JSON.stringify(snapshots);
  if (text !== Published) { Published = text; Controller?.commit(snapshots); }
  if (Controller && jumps) for (const [view, at, name] of jumps.splice(0)) Controller.jump(view, at, name);
}
// After each commit: wake an end edge whose first edge's requests landed,
// then publish (runner/collection.rs `wake_deferred_edges`).
function wake() {
  for (const d of Deferred.splice(0)) {
    if (!Lists.has(d[0].view)) continue;
    if (d[1].some(t => t.ticket)) Deferred.push(d);
    else { const c = d[0]; for (const m of c.mounted) m.epoch = ++c.nextEpoch; c.revision++; }
  }
}
After.push(() => { wake(); publish(); });
function load() {
  if (Loading || typeof requestAnimationFrame !== "function" || globalThis.__exactRender) return;
  inflight.n++;
  Loading = new Promise(r => requestAnimationFrame(() => r())).then(() => import("./collection-glue.js")).then(({ collectionController }) => {
    Controller = collectionController({ root: document.getElementById("exact-root"), views: Views, report, settled() {} });
    Published = "";
    publish();
  }).catch(e => console.error("exact: collections:", e)).finally(() => inflight.n--);
  (globalThis.exact ??= {}).lists = { settle: () => Controller?.settle(), pending: () => Loading,
    into: (view, key, block = "start", inline = "nearest") => intoView(`#${view}`, key, block, inline, null, view), intoView: intoViewState,
    // Arrange's preview, installed by reorder.js (the motion piece's chunk).
    internals: { Collection, Lists, Views, find, minEpoch, settled, controller: () => Controller } };
}
// A report from the browser half (runner/collection.rs `collection_feedback_filled`).
function report(bytes, f, fill) {
  f = { ...f, revision: Number(f.revision), scroll_sequence: Number(f.scroll_sequence), focus_view: f.focus_view ?? null, interaction_view: f.interaction_view ?? null };
  fill = { velocity: fill?.velocity ?? 0, limit: Number.isInteger(fill?.limit) ? fill.limit : null, ancestorMoving: !!fill?.ancestorMoving };
  const c = Lists.get(f.view);
  const byView = c?.prepare(f);
  if (!byView) return true;
  const cats = [f.focus_view != null, f.interaction_view != null];
  if (cats[0] || cats[1]) for (const o of Lists.values()) if (o !== c && !o.contains(c)) o.releasePins(cats);
  const [, edge] = c.feedback(f, byView, fill);
  settled();
  if (edge) edges(c, edge);
  return true;
}
function edges(c, edge) {
  let endAfterNoop = edge.endAfterNoop;
  for (const [position, i] of [[0, edge.first], [1, 1]]) {
    if (position === 1 && (!endAfterNoop || !c.edgeArmed[1])) break;
    if (position === 1) c.edgeArmed[1] = false;
    if (i === 1 && Deferred.some(d => d[0] === c)) { c.edgeArmed[1] = true; break; }
    const before = rev(), since = ticket();
    const ok = c.edges[i]();
    if (ok === false) { c.edgeArmed[i] = true; break; }
    if (position === 0 && edge.endAfterNoop && rev() !== before) {
      endAfterNoop = false;
      const targets = [...Resources, ...Mutations].filter(t => t.ticket && t.ticket.id > since);
      for (let k = Deferred.length - 1; k >= 0; k--) if (Deferred[k][0] === c) Deferred.splice(k, 1);
      Deferred.push([c, targets]);
      wake();
    }
  }
}
