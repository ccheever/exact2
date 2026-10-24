// Inspect mode on the JS target's development page (LLP 1122 D4, D5; T2b,
// the JS target): the wasm page's (host/web/dev.js), with no runner in the
// page. The dev loop serves it at /__dev/inspect.js and the page's dev client
// loads it; it is in no build's dist. While inspect mode is on, every input
// is taken here (a cover takes the pointer, a gate on the window the rest),
// so nothing reaches the app; a press released where it began selects the
// element under it that stands for a plan site (`data-site`). Its report is
// row for row the runner's `node` op (runner/src/agent.rs): the build's
// inspect table (dev-inspect.json, host/web-js/src/inspect.rs) gives each
// site's own rows, their values in each app tuple and the tokens they read,
// and the regions above it; the page gives the rest: the row an element
// inherits is its nearest site's own above it, an `each` row's key is the
// runtime's (the regions a development build's rt.js lists, `$exactEaches`), a virtualized row's its wrapper's, and a
// row an expression binds is read from the element (`computed`).

const REPORT_BYTES = 60 * 1024;
export const INSPECT_EVENTS = ["change", "input", "focus", "blur", "focusout", "compositionend", "cancel", "gotpointercapture", "lostpointercapture", "scroll", "scrollend", "drag", "dragenter", "dragover", "dragleave", "dragend", "pointerdown", "pointerup", "pointercancel", "pointerover", "pointerout", "pointerenter", "pointerleave", "click", "dblclick", "auxclick", "contextmenu", "mousedown", "mouseup", "mouseover", "mouseout", "touchstart", "touchmove", "touchend", "touchcancel", "wheel", "keydown", "keyup", "keypress", "beforeinput", "gesturestart", "gesturechange", "gestureend", "dragstart", "drop"];

let env = null, table = null, tableFor = null, live = null;
// Where this page keeps inspect mode across its reloads (sessionStorage): the variant gallery's page beside the app's
// on one origin keeps its own (`install`'s `key`).
let key = "exact.dev.inspect";
const inspecting = { on: false, banner: null, outline: null, shield: null, cover: null, close: null, selected: null, press: null, seq: null, left: null, session: null };

/** The build's inspect table, fetched once per plan. */
async function inspectTable() {
  const plan = globalThis.exact?.plan ?? null;
  if (table && tableFor === plan) return table;
  // Beside the page (its `<base>`): the app's at the root, the variant gallery's under its path.
  const got = await (env?.fetch ?? fetch)("dev-inspect.json", { cache: "no-store" });
  if (!got.ok) throw new Error(`this build has no inspect table (${got.status})`);
  const fetched = await got.json();
  // The table of the plan this page runs: a rebuild may have replaced it
  // before this page reloads.
  if (fetched.plan !== plan) throw new Error("this page's plan was rebuilt: select again after it reloads");
  table = fetched; tableFor = plan;
  return table;
}

/** The design the page runs now, when a live edit moved it from the one the
 * table's values were set in (theme2's ruling: a token's row takes its value
 * from the page's current design): its tokens by name, from the dev loop's
 * snapshot when that is the page's design; else none known (`tokens: null`). */
async function liveDesign(t) {
  const now = globalThis.exact?.designDigest ?? null;
  if (!now || now === t.design) return null;
  if (live?.design === now) return live;
  let tokens = null;
  try { const got = await (env?.fetch ?? fetch)("/__dev/design", { cache: "no-store" }); if (got.status === 200) { const s = await got.json(); if (s.design === now) tokens = new Map(s.tokens.map((k) => [k.name, k])); } } catch { /* none known */ }
  return (live = { design: now, tokens });
}
/** A token's value text as `node` prints its row: a colour `#rrggbb` when
 * opaque, a length in px a number. */
const asRow = (text) => (/^-?\d+(\.\d+)?px$/.test(text) ? Number(text.slice(0, -2)) : text.replace(/#([0-9a-fA-F]{6})(?:ff|FF)\b/g, "#$1").replace(/#[0-9A-F]{6,8}\b/g, (h) => h.toLowerCase()));

const siteOf = (el) => (el?.dataset?.site != null ? Number(el.dataset.site) : null);
const above = (el) => el.parentElement?.closest("[data-site]") ?? null;
const viewOf = (el) => globalThis.$exactViewId?.(el) ?? null;

/** The app modes an element is in (`{axis: value}`): the nearest
 * `data-x-modes` above it, else the window's. */
function modesOf(el) {
  const at = el.closest("[data-x-modes]");
  const text = at ? at.getAttribute("data-x-modes") : null;
  const out = {};
  if (text != null) { for (const pair of text.split(",")) { const [a, v] = pair.split("="); if (a && v) out[a] = v; } return out; }
  const read = globalThis.exact?.modes?.read?.();
  if (Array.isArray(read)) for (const [a, v] of read) out[a] = v;
  else if (read && typeof read === "object") Object.assign(out, read);
  return out;
}
/** The tuple number of `modes`, as the runner numbers it (the last axis the
 * least significant), and its `a=v,…` text. */
function tupleOf(t, modes) {
  let n = 0;
  const said = [];
  for (const [axis, values] of t.axes) {
    const v = values.includes(modes[axis]) ? modes[axis] : values[0];
    n = n * values.length + values.indexOf(v);
    said.push(`${axis}=${v}`);
  }
  return { n, text: said.join(",") };
}
const pick = (v, n) => (v && typeof v === "object" && Array.isArray(v.tuples) ? v.tuples[n] ?? null : v);
// Each row's CSS property as the page writes it (the table's, style.rs `css_property`).
const cssName = (name) => table?.css?.[name] ?? name.replace(/_/g, "-");
const computed = (el, name) => getComputedStyle(el).getPropertyValue(cssName(name)).trim();
// A value that sets no row: CSS's own words for inheriting or none.
const CLEARS = /^(inherit|initial|unset|revert|revert-layer)$/i;
// (As the runner reads them, row by row: a colour's `currentcolor` and a
// colour scheme's `normal` clear too; a font family's `initial` is its own
// row, the initial family.)
const clears = (name, said) => (CLEARS.test(said) && !(name === "font_family" && /^initial$/i.test(said))) || (name === "text_color" && /^currentcolor$/i.test(said)) || (name === "color_scheme" && /^normal$/i.test(said));
/** Whether an expression wrote a clear that inherits (`inherit`, `unset`):
 * the runner keeps it written, so a control's start or a text style does
 * not stand in for it. */
const writtenClear = (el, name) => /^(inherit|unset|revert|revert-layer)$/i.test(el.style.getPropertyValue(cssName(name)).trim());
/** The tokens an inline value names, by their custom properties. */
const tokensIn = (t, said) => [...new Set([...said.matchAll(/var\((--[\w-]+)/g)].map((m) => t.props?.[m[1]]).filter(Boolean))];

/** Row `name` as `el`'s own row `row` gives it: its value (read from the
 * element for a row an expression binds) and the token it reads. */
function ownRow(t, el, row, lv) {
  const { n, text } = tupleOf(t, modesOf(el));
  // A pin's row holds the tuple it makes, plus one (runner bridge.rs): the
  // tuple the boundary's own `data-x-modes` says.
  if (row.n === "mode") return { value: n + 1 };
  const out = valueOf(t, el, row, n, lv);
  if (row.k) out.token = { names: row.k, text: row.x, modes: text, implied: false };
  // A row an expression binds reads the tokens its value names now: their
  // custom properties in what the page wrote (the text is the one token's
  // name when it names one alone).
  else if (row.d) {
    const said = el.style.getPropertyValue(cssName(row.n));
    const names = tokensIn(t, said);
    if (names.length) out.token = { names, text: /^var\(--[\w-]+\)$/.test(said.trim()) ? names[0] : said.trim(), modes: text, implied: false };
  }
  return out;
}
/** Row `row`'s value on `el` in tuple `n`: the table's, or, in a design a
 * live edit moved (`lv`), its one token's value there now; the element's
 * computed CSS for an expression's row or a token's the page can't say. */
function valueOf(t, el, row, n, lv) {
  if (row.d || row.v == null) return { value: computed(el, row.n), computed: true };
  // A length relative to the instance's font or the root: the element's, in px.
  // (Written as the table's value is: a number of px, or CSS text such as a line height's "20px".)
  if (row.u) { const px = computed(el, row.n); return /^-?\d+(\.\d+)?px$/.test(px) ? { value: typeof pick(row.v, n) === "number" ? Number(px.slice(0, -2)) : px } : { value: px, computed: true }; }
  if (!row.k || !lv) return { value: pick(row.v, n) };
  // A live edit that left these tokens as they were leaves the table's value.
  const same = lv.tokens && row.k.every((name) => { const now = lv.tokens.get(name), then = t.tokens?.[name]; return now && then && JSON.stringify([now.value, now.slices ?? []]) === JSON.stringify(then); });
  if (same) return { value: pick(row.v, n) };
  // One token, changed: its value now, as `node` writes a row (a colour
  // `#rrggbb`, a number, a px length as the table's kind); else the CSS.
  const token = lv.tokens && row.k.length === 1 && row.x === row.k[0] ? lv.tokens.get(row.k[0]) : null;
  const text = token ? String(token.slices?.[n] ?? token.value) : null, was = pick(row.v, n);
  if (text != null) {
    if (/^#[0-9a-f]{6}([0-9a-f]{2})?$|^light-dark\(/i.test(text)) return { value: asRow(text) };
    if (/^-?\d*\.?\d+$/.test(text) && typeof was === "number") return { value: Number(text) };
    if (/^-?\d*\.?\d+px$/.test(text)) return { value: typeof was === "number" ? Number(text.slice(0, -2)) : text };
  }
  return { value: computed(el, row.n), computed: true };
}
/** Whether an expression's row is set on `el` now: the page wrote it (a
 * class that is off sets none), and not as a clear (`inherit`, `unset`); a
 * pin that is one the design has (modes.js `$xbad` names a refused one); on
 * a boundary, a colour its binding wrote (`$xi`), not the one modes.js
 * writes for it. */
function setNow(el, name) {
  if (name === "mode") { const pin = el.getAttribute("data-x-pin")?.trim(); return pin != null && pin !== "" && !CLEARS.test(pin) && el.$xbad === undefined; }
  const j = table?.channels?.indexOf(name) ?? -1;
  if (j >= 0 && el.hasAttribute("data-x-b")) return el.$xi?.[j] !== undefined;
  const said = el.style.getPropertyValue(cssName(name)).trim();
  return said !== "" && !clears(name, said);
}

/** The `each` row of region `region` that `el` is in, with the key the app
 * gave it (its key function over its item and index): a development build's
 * runtime lists every live region (`$exactEaches`, module.mjs
 * `withInspectHooks`). */
function rowOf(region, el) {
  for (const e of globalThis.$exactEaches ?? []) {
    if (e.region !== region || !e.live()) continue;
    for (const r of e.rows()) {
      const inside = r.start === r.end ? r.start === el || r.start.contains?.(el) : (r.start.compareDocumentPosition(el) & 4) && (r.end.compareDocumentPosition(el) & 2);
      if (inside) return { key: e.key(() => r.item.n.v, () => r.index.n.v) };
    }
  }
  return null;
}
/** The instance path to `el` from the root: each region above its site, an
 * arm by its index, an `each` row by its key. Null when a row's key can't
 * be found. */
function instanceOf(t, el, entry) {
  const path = [];
  const wrappers = [];
  for (let w = el.closest("[data-listitemkey]"); w; w = w.parentElement?.closest("[data-listitemkey]")) wrappers.unshift(w);
  for (const [region, arm] of entry.c ?? []) {
    if (arm != null) { path.push({ region, arm }); continue; }
    const row = rowOf(region, el);
    if (row) { path.push({ region, key: row.key }); continue; }
    // A virtualized list's row: its wrapper's key (list.js `keyText`).
    const w = wrappers.shift();
    if (!w) return null;
    const k = w.getAttribute("data-listitemkey").replace(/^d\d+:/, "");
    path.push({ region, key: k.startsWith("s:") ? k.slice(2) : k.startsWith("n:") ? Number(k.slice(2)) : k === "b:true" });
  }
  return path;
}

/** The selected element's report body, row for row as the runner's `node`. */
async function nodeOf(el) {
  const t = await inspectTable();
  const site = siteOf(el);
  const entry = t.sites[String(site)];
  if (!entry) return { error: `site ${site} is not in this build's inspect table` };
  const instance = instanceOf(t, el, entry);
  if (!instance) return { error: "the row this view is in was not found" };
  const lv = await liveDesign(t);
  const style = {};
  for (const name of t.order) {
    let own = entry.r?.find((r) => r.n === name), inherits = false;
    if (own?.d && !setNow(el, name)) { inherits = writtenClear(el, name); own = null; }
    if (own) { style[name] = { ...ownRow(t, el, own, lv), source: own.d ? "dynamic" : own.h ? "heading" : "authored" }; continue; }
    const inherited = Object.hasOwn(t.initial, name);
    if (!inherited) continue;
    // A mode boundary's implied row: its own, its token the one above read.
    if (entry.b && t.implied.includes(name)) {
      const row = impliedRow(t, el, name, lv);
      // (An own row the site binds by an expression is `dynamic`, as `node` says, an implied one too.)
      if (row) { style[name] = { ...row, source: entry.r?.some((r) => r.n === name && r.d) ? "dynamic" : "authored" }; continue; }
    }
    // It starts here (a node type's own initial value, or a control's start
    // that stops what is above).
    if (!inherits && entry.i && Object.hasOwn(entry.i, name)) { style[name] = { value: entry.i[name], source: "initial" }; continue; }
    if (!inherits && entry.s?.[name] != null) {
      // The node whose platform text style gives it: this one, or one above.
      const [from, value] = entry.s[name];
      let at = el;
      while (at && siteOf(at) !== from) at = above(at);
      style[name] = { value: pick(value, tupleOf(t, modesOf(el)).n), source: "style", ...(at ? { from: viewOf(at) } : {}) };
      continue;
    }
    let found = null;
    for (let at = above(el); at && !found; at = above(at)) {
      const e = t.sites[String(siteOf(at))];
      const row = e?.r?.find((r) => r.n === name && !(r.d && !setNow(at, name)));
      if (row) found = { at, ...ownRow(t, at, row, lv) };
      else if (e?.b && t.implied.includes(name)) { const row = impliedRow(t, at, name, lv); if (row) found = { at, ...row }; }
    }
    // (And the ancestor's plan site, which the design server names by its source: tamber/1.16 `fromAt`.)
    if (found) { const { at, ...row } = found; style[name] = { ...row, source: "inherited", from: viewOf(at), ...(siteOf(at) != null ? { fromSite: siteOf(at) } : {}) }; continue; }
    style[name] = { value: t.initial[name], source: "initial" };
  }
  const modes = modesOf(el);
  return { type: entry.t, site, instance, modes, style, testId: el.getAttribute("data-testid") };
}
/** A boundary `el`'s implied row `name` (LLP 1121 D7): the text the
 * nearest own row above it reads, set again in the boundary's tuple; none
 * when that row reads no token. */
function impliedRow(t, el, name, lv) {
  for (let at = above(el); at; at = above(at)) {
    const row = t.sites[String(siteOf(at))]?.r?.find((r) => r.n === name && !(r.d && !setNow(at, name)));
    if (!row) continue;
    const { n, text } = tupleOf(t, modesOf(el));
    if (row.k) return { ...valueOf(t, el, row, n, lv), token: { names: row.k, text: row.x, modes: text, implied: true } };
    // An expression's row above: the tokens its value names now, read again here.
    if (row.d) {
      const said = at.style.getPropertyValue(cssName(name)).trim(), names = tokensIn(t, said);
      if (names.length) return { value: computed(el, name), computed: true, token: { names, text: /^var\(--[\w-]+\)$/.test(said) ? names[0] : said, modes: text, implied: true } };
    }
    return null;
  }
  return null;
}

const bytes = (value) => new TextEncoder().encode(JSON.stringify(value)).length;
async function selectedReport(el, click = null) {
  const x = globalThis.exact;
  // Under the facts it began under: a live batch while it read (a design
  // snapshot, the table) reads again, so the rows are that design's.
  let facts, node;
  for (let tries = 0; ; tries++) {
    facts = env.facts();
    try { node = await nodeOf(el); } catch (e) { return { kind: "unsupported", what: "inspect", detail: String(e.message ?? e) }; }
    const now = env.facts();
    if ((now.design === facts.design && now.revision === facts.revision) || tries >= 2) { facts = now; break; }
  }
  if (node.error) return { kind: "unsupported", what: "inspect", detail: node.error };
  const envelope = (selected) => ({ kind: "selected", selected, inspect: inspecting.seq ?? undefined, ...(click ? { click } : {}), plan: facts.plan, design: facts.design, revision: facts.revision });
  let selected = { node: viewOf(el), type: node.type, testId: node.testId, site: node.site, instance: node.instance, planDigest: x?.plan ?? null, tuple: node.modes ?? null, rows: Object.entries(node.style).map(([name, row]) => ({ name, ...row })), held: globalThis.exact?.heldOf?.(el)?.held ?? null, trimmed: false };
  // (The held node, named by this runtime's ids, where the glue could not number it.)
  if (selected.held && selected.held.node == null && selected.held.el) selected.held.node = viewOf(selected.held.el);
  if (bytes(envelope(selected)) > REPORT_BYTES) selected = { ...selected, rows: selected.rows.map(({ name, value, computed: c, source, from, fromSite, token }) => ({ name, value, ...(c ? { computed: c } : {}), source, from, fromSite, token })), trimmed: true };
  while (bytes(envelope(selected)) > REPORT_BYTES && selected.rows.length) selected = { ...selected, rows: selected.rows.slice(0, Math.floor(selected.rows.length / 2)) };
  if (bytes(envelope(selected)) > REPORT_BYTES) return { kind: "unsupported", what: "inspect", detail: "the selected node's identity alone is larger than a report" };
  return envelope(selected);
}

// A click's selection carries a token (tamber/1.7) the outline answering it
// names: that one scrolls nothing. A selection is its element in a page: a
// refresh after a reload finds it again by its site and instance path.
let clicks = 0, asks = 0;
async function select(el, clicked = false, click = null) {
  const seq = inspecting.seq, ask = ++asks;
  if (clicked) click = `${inspecting.session ??= crypto.getRandomValues(new Uint32Array(2)).join("")}.${++clicks}`;
  const r = await selectedReport(el, click);
  // Left, another command came, or a later click or refresh: not published.
  if (!inspecting.on || inspecting.seq !== seq || ask !== asks) return;
  if (r.kind === "selected") {
    inspecting.selected = { el, click, plan: r.selected.planDigest, site: r.selected.site, instance: JSON.stringify(r.selected.instance) };
    // Kept across the dev loop's reload (a logic edit replaces the page), so
    // a refresh after it finds the node again, or says it is gone.
    try { const { el: _, ...kept } = inspecting.selected; sessionStorage.setItem(key + ".selected", JSON.stringify({ ...kept, seq: inspecting.seq, session: inspecting.session, clicks })); } catch { /* storage refused */ }
  }
  env.report(r);
}
// `inspect` `{select: {up, levels, plan, from, instance}}` (tamber/1.16): the selection moves up its call sites, as a click on
// the node it lands on would select it. `levels` are the plan sites of each enclosing copy that owns a node, nearest
// first (past the outermost call, the app's own), by the server's map for the selection at site `from` (instance path `instance`) under `plan`:
// it lands in the first level with a node above the selection, on the outermost there, by this page's own tree.
async function selectUp({ levels, plan, from, instance }) {
  const el = inspecting.selected?.el;
  // (No selection here to move, or none above it of that copy: staleness, not inability; its selection as it is.)
  if (!inspecting.on || !el?.isConnected || !Array.isArray(levels)) { await refresh(); return; }
  // (Only the selection, its instance and the plan the server took its sites from. One that moved since on its way
  // here is no inability (theme2 15:02): the page reports its selection as it is now, and the tool's chain follows.)
  if (inspecting.selected.plan !== (globalThis.exact?.plan ?? null) || plan !== inspecting.selected.plan || from !== inspecting.selected.site || JSON.stringify(instance ?? []) !== inspecting.selected.instance) { await refresh(); return; }
  // The first level with a node above the selection, by this page's tree; in it, the outermost.
  for (const level of levels) {
    const want = new Set(level);
    let found = null;
    for (let at = above(el); at; at = above(at)) if (want.has(siteOf(at))) found = at;
    if (found) { await select(found); return; }
  }
  await refresh();
}
async function refresh() {
  const was = inspecting.selected;
  if (!was || !inspecting.on) return;
  if (!was.el?.isConnected || globalThis.exact?.plan !== was.plan) {
    let found = null;
    if (globalThis.exact?.plan === was.plan) {
      const t = await inspectTable();
      for (const el of document.querySelectorAll(`[data-site="${was.site}"]`)) {
        if (JSON.stringify(instanceOf(t, el, t.sites[String(was.site)] ?? {})) !== was.instance) continue;
        if (found) { found = null; break; }
        found = el;
      }
    }
    if (!found) { inspecting.selected = null; env.report({ kind: "unsupported", what: "inspect", detail: "the selected node is gone in this generation: select it again" }); return; }
    was.el = found;
  }
  await select(was.el, false, was.click);
}

// The element a tap at (x, y) would find: the topmost of the app's (this
// inspector's chrome aside) that stands for a plan site.
function nodeAt(x, y) {
  const root = document.getElementById("exact-root");
  if (inspecting.shield) inspecting.shield.disabled = true;
  let hit;
  try { hit = document.elementsFromPoint(x, y).find((e) => e !== inspecting.cover && !inspecting.banner?.contains(e)); } finally { if (inspecting.shield) inspecting.shield.disabled = false; }
  if (!hit || (root && !root.contains(hit)) || hit.closest("[data-exiting]")) return null;
  return hit.closest("[data-site]");
}
function outline(el) {
  if (!inspecting.outline) return;
  if (!el) { inspecting.outline.style.display = "none"; return; }
  const r = el.getBoundingClientRect();
  Object.assign(inspecting.outline.style, { display: "block", left: `${r.left}px`, top: `${r.top}px`, width: `${r.width}px`, height: `${r.height}px` });
}

function swallow(event) {
  if (!inspecting.on) return;
  // A guest frame took focus: it goes back to the close control, so keys
  // (Escape) meet the gate again.
  if (event.type === "blur" && document.activeElement?.tagName === "IFRAME") inspecting.close?.focus();
  if (event.type === "keydown" && event.key === "Escape") { event.preventDefault(); event.stopImmediatePropagation(); setInspect(false, true); return; }
  if (inspecting.banner?.contains(event.target)) {
    event.stopImmediatePropagation();
    if (event.type === "wheel") event.preventDefault();
    if (event.type === "click" && inspecting.close?.contains(event.target)) setInspect(false, true);
    return;
  }
  event.preventDefault();
  event.stopImmediatePropagation();
  if (event.type === "scroll") place();
  if (event.type === "pointerdown") { chrome(); inspecting.press = { x: event.clientX, y: event.clientY, id: event.pointerId }; }
  if (event.type === "pointerup" && inspecting.press?.id === event.pointerId) {
    const { x, y } = inspecting.press;
    inspecting.press = null;
    if (Math.hypot(event.clientX - x, event.clientY - y) <= 10) { const el = nodeAt(event.clientX, event.clientY); outline(el); if (el) void select(el, true); }
  }
  if (event.type === "pointercancel") inspecting.press = null;
}
function hover(event) { if (!inspecting.on) return; event.stopImmediatePropagation(); chrome(); outline(nodeAt(event.clientX, event.clientY)); }
/** No field or guest frame takes focus meanwhile, and no app handler hears it try. */
function focusing(event) {
  if (!inspecting.on) return;
  event.stopImmediatePropagation();
  if (!inspecting.banner?.contains(event.target)) event.target?.blur?.();
}
/** Every event the page's gate (its dev client, listening on the window in
 * the capture phase from before the app's scripts run) hands here. */
export function gate(event) {
  if (event.type === "pointermove") return hover(event);
  if (event.type === "focusin") return focusing(event);
  return swallow(event);
}

const topLayer = (el) => { try { el.popover = "manual"; el.showPopover(); } catch { /* no popover: a fixed element */ } };
const topModal = () => { try { return [...document.querySelectorAll("dialog")].findLast((d) => d.matches(":modal")) ?? null; } catch { return null; } };
function chrome() {
  const modal = topModal();
  if (inspecting.banner?.isConnected && inspecting.outline?.isConnected && inspecting.shield?.isConnected && inspecting.cover?.isConnected && (!modal || modal.contains(inspecting.banner))) return;
  inspecting.banner?.remove(); inspecting.outline?.remove(); inspecting.shield?.remove(); inspecting.cover?.remove();
  const banner = inspecting.banner = document.createElement("div");
  banner.id = "exact-inspect";
  banner.setAttribute("role", "status");
  banner.style.cssText = "position:fixed;inset:8px auto auto 50%;margin:0;transform:translateX(-50%);z-index:2147483647;display:flex;gap:8px;align-items:center;padding:6px 10px;border:0;border-radius:8px;background:#1d4ed8;color:#fff;font:13px system-ui,sans-serif;box-shadow:0 2px 8px rgba(0,0,0,.3)";
  banner.textContent = "Inspecting: click a view to select it";
  const close = inspecting.close = document.createElement("button");
  close.type = "button";
  close.textContent = "×";
  close.setAttribute("aria-label", "Stop inspecting (Escape)");
  close.style.cssText = "border:0;background:transparent;color:#fff;font-size:16px;cursor:pointer;padding:0 2px";
  banner.append(close);
  const box = inspecting.outline = document.createElement("div");
  box.style.cssText = "position:fixed;display:none;margin:0;padding:0;border:0;z-index:2147483646;pointer-events:none;outline:2px solid #1d4ed8;background:rgba(29,78,216,.12)";
  const shield = inspecting.shield = document.createElement("style");
  shield.textContent = "iframe, frame, embed, object { pointer-events: none !important; }";
  document.head.append(shield);
  const cover = inspecting.cover = document.createElement("div");
  cover.style.cssText = "position:fixed;inset:0;margin:0;padding:0;border:0;width:auto;height:auto;z-index:2147483644;background:transparent;touch-action:none";
  (modal ?? document.body).append(cover, box, banner);
  for (const { mark } of marks) (modal ?? document.body).append(mark);
  topLayer(cover); topLayer(box); topLayer(banner);
  // (Rebuilt with boxes still up: said again.)
  legend(marks.length);
}
function setInspect(on, local = false) {
  if (on === inspecting.on) { if (on) chrome(); return; }
  inspecting.on = on;
  try { if (on) sessionStorage.setItem(key, String(inspecting.seq ?? "")); else sessionStorage.removeItem(key); } catch { /* storage refused */ }
  if (!on) { inspecting.selected = null; try { sessionStorage.removeItem(key + ".selected"); } catch { /* storage refused */ } }
  if (!on && local) {
    inspecting.left = inspecting.seq;
    try { sessionStorage.setItem(key + ".left", String(inspecting.seq ?? "")); } catch { /* storage refused */ }
    const facts = env.facts();
    env.report({ kind: "selected", selected: null, inspect: inspecting.seq, plan: facts.plan, revision: facts.revision });
  }
  if (on) { document.activeElement?.blur?.(); chrome(); }
  else {
    inspecting.banner?.remove(); inspecting.outline?.remove(); inspecting.shield?.remove(); inspecting.cover?.remove(); unmark(); inspecting.session = null;
    inspecting.banner = inspecting.outline = inspecting.shield = inspecting.cover = inspecting.close = inspecting.press = null;
  }
}

// `outline {sites, plan}`: every copy of those sites, the elements with
// their `data-site`, outlined while inspect mode is on; a frame for another
// plan leaves them; an empty one, or `clear`, clears them.
const marks = [];
const resized = typeof ResizeObserver === "function" ? new ResizeObserver(() => place()) : null;
function unmark(quiet = false) { resized?.disconnect(); for (const { mark } of marks.splice(0)) mark.remove(); if (!quiet) legend(0); }
// The banner says what the dashed boxes are while they show (the second hands-on pass, #7): what is outlined, a
// selection's every copy or what the tool's Outline asked; its words written only when they change (a live region).
const legend = (n) => {
  const t = inspecting.banner?.firstChild, words = n > 0 ? `Inspecting: ${n} outlined in dashes (the selection's every copy, or what the tool outlined); click a view to select it` : "Inspecting: click a view to select it";
  if (t?.nodeType === 3 && t.nodeValue !== words) t.nodeValue = words;
};
function place() {
  for (const { mark, el } of marks) {
    const r = el.getBoundingClientRect();
    Object.assign(mark.style, { left: `${r.left}px`, top: `${r.top}px`, width: `${r.width}px`, height: `${r.height}px`, display: el.isConnected ? "block" : "none" });
  }
  // (A view gone takes its box: the count said is of those still shown.)
  if (marks.length) legend(marks.filter(({ el }) => el.isConnected).length);
}
function outlineSites(frame) {
  if (frame.clear) { unmark(); return; }
  if (!inspecting.on) return;
  if (globalThis.exact?.plan !== frame.plan || !Array.isArray(frame.sites)) return;
  unmark(true);
  const wanted = new Set(frame.sites.map(String));
  const found = [...document.querySelectorAll("[data-site]")].filter((el) => wanted.has(el.dataset.site) && !el.closest("[data-exiting]"));
  if (!(inspecting.session && typeof frame.click === "string" && frame.click.startsWith(`${inspecting.session}.`))) found[0]?.scrollIntoView({ block: "nearest" });
  for (const el of found) {
    const mark = document.createElement("div");
    mark.dataset.exactOutline = "";
    mark.style.cssText = "position:fixed;margin:0;padding:0;border:0;z-index:2147483645;pointer-events:none;outline:2px dashed #1d4ed8";
    (inspecting.banner?.parentElement ?? document.body).append(mark);
    marks.push({ mark, el });
    resized?.observe(el);
  }
  place();
  legend(found.length);
}

/** An `inspect` or `outline` frame from the build channel. */
/** `hold {revision, hold}` (LLP 1122 D4, D8; *1.11*): `{site, instance, state}` of this page's plan, the element
 * by its site and instance path (exactly one), held by the input glue as the real state; null lets it go. What it
 * holds or let go, and why, is reported (`held`). */
// One at a time, in order; one a newer frame replaced while it waited is dropped unsaid (the newer one says).
// A revision the page let go of itself (input, disabled, gone) is not taken again when its frame comes again.
let holding = Promise.resolve(), holdLatest = null, applying = null;
const ended = new Set();
// (`epoch`: the server's run the revisions are of; a let-go of an earlier run's hold ends nothing now.)
let lastRevision = 0, epoch = 0, holdStream = null;
const end = (revision, of) => { if (of !== epoch) return; ended.add(revision); if (ended.size > 64) ended.delete(ended.values().next().value); };
const canonical = (v) => JSON.stringify(v ?? [], (_, x) => (x && typeof x === "object" && !Array.isArray(x) ? Object.fromEntries(Object.entries(x).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))) : x));
// The runtime the app runs (this module is served at /__dev/inspect.js): the one beside its app.js.
const runtime = () => new URL("rt.js", document.querySelector('script[type="module"][src$="app.js"]')?.src ?? document.baseURI).href;
function hold(m) {
  // (A lower revision is a new server's: the revisions ended before are forgotten.)
  // (A new stream, which may be a new server's, likewise.)
  // (`stream`: the one it came on, as dev.mjs stamps it on arrival.)
  const stream = m.stream !== undefined ? m.stream : env.nonce?.() ?? null;
  if (m.revision < lastRevision || stream !== holdStream) { ended.clear(); epoch += 1; holdStream = stream; }
  m.stream = stream;
  m.epoch = epoch;
  lastRevision = m.revision;
  holdLatest = m;
  holding = holding.then(() => holdNow(m)).catch((error) => console.error("exact dev:", String(error)));
  return holding;
}
// How long a hold waits for the input glue to load before saying it is not loaded.
const HOLD_WAIT = 60_000;
/** The input glue's `hold` once it has loaded (it says so: `exact-hold`), or null after `ms` or once `stale()`
 * (a newer frame came). host/web/dev.js keeps the same wait. */
export function glueHold(ms, stale = () => false) {
  return new Promise((ok) => {
    if (globalThis.exact?.hold) return ok(globalThis.exact.hold);
    const until = Date.now() + ms;
    const end = () => { clearInterval(tick); globalThis.removeEventListener?.("exact-hold", end); ok(globalThis.exact?.hold ?? null); };
    const tick = setInterval(() => { if (globalThis.exact?.hold || stale() || Date.now() > until) end(); }, 250);
    globalThis.addEventListener?.("exact-hold", end);
  });
}
async function holdNow(m) {
  // (The latest frame, on the stream it came on: a new one's server knows nothing of it.)
  const current = () => holdLatest === m && m.epoch === epoch && m.stream === (env.nonce?.() ?? null);
  // A let-go needs no glue: with none loaded, nothing is held.
  if (!m.hold) { if (!current()) return; globalThis.exact?.hold?.(null); env.report({ kind: "held", state: null, revision: m.revision }); return; }
  // The input glue, which a page with no press may not have loaded yet, loads after paint: waited for (a page
  // reloading after a rebuild, on a loaded machine, takes far longer than a fixed wait), unless a newer frame comes.
  // (The runtime's own handle, which a bundled page has; else its module, where one is served beside app.js.)
  if (!globalThis.exact?.hold) await (globalThis.__exactPressFeedback ? Promise.resolve().then(globalThis.__exactPressFeedback) : import(runtime()).then((rt) => rt.pressFeedback?.())).catch(() => {});
  await glueHold(HOLD_WAIT, () => !current());
  if (!current()) return;
  const holds = globalThis.exact?.hold, h = m.hold, revision = m.revision;
  if (!holds) { env.report({ kind: "unsupported", what: "hold", detail: "the input glue is not loaded", revision }); return; }
  // One ended here is not taken again; one addressing another plan (a rebuild this page reloads to) is nothing,
  // unsaid: the new page's registration replays it, its site corresponded.
  if (ended.has(revision) || (typeof m.plan === "string" && m.plan !== (globalThis.exact?.plan ?? null))) return;
  let t;
  try { t = await inspectTable(); } catch (error) {
    // No table to resolve it by: the hold before is let go, and this one refused by name. A rebuild's table (this
    // page reloads to it) says nothing: the new page's registration replays the hold, its site corresponded.
    if (!current() || /was rebuilt/.test(String(error))) return;
    holds(null);
    env.report({ kind: "unsupported", what: "hold", detail: `the inspect table is unreadable: ${String(error)}`, revision });
    return;
  }
  if (!current()) return;
  let found = null, two = false;
  for (const el of document.querySelectorAll(`[data-site="${Number(h.site)}"]`)) {
    // (An instance path that does not resolve is no match, not the empty path.)
    if (el.closest("[data-exiting]")) continue; // a leaving view is no match
    const instance = instanceOf(t, el, t.sites[String(h.site)] ?? {});
    if (instance == null || canonical(instance) !== canonical(h.instance)) continue;
    if (found) { two = true; break; }
    found = el;
  }
  // (No one element there: the one held before is let go too.)
  // (The same frame taken again, a resent one, replaces nothing.)
  applying = revision;
  const r = found && !two ? holds(found, h.state, (reason) => {
    if (reason === "replaced" && applying === revision) return;
    if (m.epoch !== epoch || m.stream !== (env.nonce?.() ?? null)) return; // a hold of an earlier stream's: no news to this one
    if (reason !== "replaced") end(revision, m.epoch);
    env.report({ kind: "held", state: null, reason, revision });
  }) : (holds(null), { reason: "gone" });
  applying = null;
  if (r.reason) end(revision, m.epoch);
  env.report(r.reason ? { kind: "held", state: null, reason: r.reason, revision } : { kind: "held", state: h.state, revision });
}

export async function frame(m) {
  if (m.kind === "outline") { outlineSites(m); return; }
  if (m.select) { await selectUp(m.select); return; }
  if (m.kind === "hold") { await hold(m); return; }
  if (m.refresh) { if (m.status) await env.status(); await refresh(); return; }
  if (m.on && m.seq != null && m.seq === inspecting.left) { const facts = env.facts(); env.report({ kind: "selected", selected: null, inspect: m.seq, plan: facts.plan, revision: facts.revision }); return; }
  inspecting.left = null;
  try { sessionStorage.removeItem(key + ".left"); } catch { /* storage refused */ }
  if ((m.seq ?? null) !== inspecting.seq) { inspecting.selected = null; try { sessionStorage.removeItem(key + ".selected"); } catch { /* storage refused */ } }
  inspecting.seq = m.seq ?? null;
  setInspect(m.on === true);
  if (inspecting.on) try { sessionStorage.setItem(key, String(inspecting.seq ?? "")); } catch { /* storage refused */ }
}

/** Installed by the page's dev client: `report(body)` sends a report,
 * `status()` the page's status, `facts()` its `{plan, design, revision}`. */
export function install(given) {
  env = given;
  key = given.key ?? key;
  // A held node's instance path, for the held answer (input-glue.js `heldOf`): the inspect table's, once read.
  (globalThis.exact ??= {}).instanceOf = (el) => {
    const entry = table && el?.dataset?.site != null ? table.sites[String(el.dataset.site)] : null;
    return entry ? instanceOf(table, el, entry) : null; // (a site the table lacks has no path to say)
  };
  // The dev client's own stand-in (its cover and shield while this loaded) goes.
  document.getElementById("exact-inspect-early")?.remove();
  document.getElementById("exact-inspect-early-shield")?.remove();
  // (The gate is the dev client's, on the window before the app's scripts
  // ran; it hands each event to `gate`.)
  addEventListener("resize", () => { if (inspecting.on) place(); });
  // A reload of a tab that was inspecting starts inspecting, until the
  // server's replay on registering says otherwise.
  try {
    const kept = sessionStorage.getItem(key), left = sessionStorage.getItem(key + ".left");
    if (kept != null) {
      inspecting.seq = kept === "" ? null : Number(kept);
      setInspect(true);
      // The selection made before the reload, and its click session.
      const was = JSON.parse(sessionStorage.getItem(key + ".selected") ?? "null");
      if (was && (was.seq ?? null) === inspecting.seq) { inspecting.selected = { el: null, click: was.click, plan: was.plan, site: was.site, instance: was.instance }; inspecting.session = was.session ?? null; clicks = was.clicks ?? 0; }
    }
    else if (left) inspecting.left = Number(left);
  } catch { /* storage refused */ }
}
