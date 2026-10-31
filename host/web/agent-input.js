// The agent's input helpers (LLP 1012): a guest frame's outline, taps and typing, a target's reveal, the boxes and
// scroll ranges `layout` reports, and the glides `clock settle` waits for. Loaded only where the agent drives the
// page (glue.js `agentInputReady`, the JS target's agent.js); the input glue keeps what a person's input needs.
import { backButton } from "./input-glue.js";
// A same-origin guest joins `tree` as a compact, bounded outline. Access to
// a sandboxed or cross-origin document is simply absent (@ref LLP 1020 D4).
export function guestOutline(frame) {
  let doc;
  try { doc = frame.contentDocument; } catch { return null; }
  if (!doc) return null;
  const outline = [];
  const visit = (el, depth) => {
    if (depth > 4 || outline.length >= 32) return;
    const id = el.id || undefined;
    const testId = el.getAttribute("data-testid") ?? el.getAttribute("testId") ?? undefined;
    const text = [...el.childNodes]
      .filter((n) => n.nodeType === Node.TEXT_NODE)
      .map((n) => n.textContent.trim())
      .filter(Boolean)
      .join(" ")
      .replace(/\s+/g, " ")
      .slice(0, 160) || undefined;
    if (id || testId || text) {
      outline.push({ guest: true, depth, tag: el.localName, ...(id ? { id } : {}), ...(testId ? { testId } : {}), ...(text ? { text } : {}) });
    }
    for (const child of el.children) visit(child, depth + 1);
  };
  for (const child of doc.body?.children ?? []) visit(child, 0);
  return outline;
}
function guestDocument(frame) {
  try {
    const document = frame.contentDocument;
    return document ? { document } : { error: "guest is cross-origin" };
  } catch {
    return { error: "guest is cross-origin" };
  }
}
export function guestTap(frame, request) {
  const access = guestDocument(frame);
  if (access.error) return { guest: true, error: access.error };
  const { document } = access;
  const guest = document.defaultView;
  const x = Number.isFinite(request.x) ? request.x : guest.innerWidth / 2;
  const y = Number.isFinite(request.y) ? request.y : guest.innerHeight / 2;
  let target;
  try { target = request.selector ? document.querySelector(request.selector) : null; }
  catch { return { guest: true, malformed: true, error: "guest tap has an invalid selector" }; }
  target ||= document.elementFromPoint(x, y) || document.body;
  if (!target) return { guest: true, error: "guest tap found no target" };
  // Script input is intentionally untrusted (@ref LLP 1020 D4;
  // exact1 20260806-webview-frame-guest-click-delivery).
  target.dispatchEvent(new guest.PointerEvent("pointerdown", { bubbles: true, composed: true, clientX: x, clientY: y, button: 0, buttons: 1 }));
  target.dispatchEvent(new guest.PointerEvent("pointerup", { bubbles: true, composed: true, clientX: x, clientY: y, button: 0, buttons: 0 }));
  target.dispatchEvent(new guest.MouseEvent("click", { bubbles: true, composed: true, clientX: x, clientY: y, button: 0 }));
  return { tapped: request.id, guest: true };
}
export function guestType(frame, request) {
  const access = guestDocument(frame);
  if (access.error) return { guest: true, error: access.error };
  const { document } = access;
  const guest = document.defaultView;
  const active = document.activeElement;
  const editable = active?.matches?.("input,textarea,[contenteditable]") ? active : null;
  let target;
  try { target = request.selector ? document.querySelector(request.selector) : null; }
  catch { return { guest: true, malformed: true, error: "guest type has an invalid selector" }; }
  target ||= editable || document.querySelector("input,textarea,[contenteditable]");
  if (!target) return { guest: true, error: "guest type found no target" };
  // These are the Apple guest script's event shapes, including focus and
  // isTrusted:false (@ref LLP 1020 D4).
  target.focus();
  if (request.key != null) {
    const key = String(request.key);
    target.dispatchEvent(new guest.KeyboardEvent("keydown", { key, bubbles: true, composed: true }));
    target.dispatchEvent(new guest.KeyboardEvent("keyup", { key, bubbles: true, composed: true }));
    return { typed: request.id, guest: true, key, value: guestValue(target) };
  }
  const text = String(request.text ?? "");
  if ("value" in target) target.value = text; else target.textContent = text;
  target.dispatchEvent(new guest.InputEvent("input", { data: text, inputType: "insertText", bubbles: true, composed: true }));
  target.dispatchEvent(new guest.Event("change", { bubbles: true, composed: true }));
  return { typed: request.id, guest: true, value: guestValue(target) };
}
// What a guest field holds, as the reply shows it: a password's is a fixed mark, whatever its length (#134).
const guestValue = (target) => "value" in target ? (target.type === "password" && target.value ? "•••" : target.value) : target.textContent;

// The agent's `reveal` (ledger F7, shop F11): before a tap or a type, a
// target whose middle is out of view is scrolled to the middle of its
// nearest scroll containers, then of the page (across, only as far as it
// takes) — Playwright's actionability scroll, the web's own `scrollIntoView`. A scroll event reaches the app as a
// person's scroll would. `scrolled` is where the middle moved from and to;
// a middle already in view moves nothing.
// Both agents supply their modal-aware inert check; standalone callers default to DOM ancestry.
export function reveal(el, id, inert = el => el.closest("[inert]")) {
  if (!el?.isConnected) return { error: `no view ${id}` };
  // The shadow button does not carry the route control's inert ancestry.
  if (inert(el)) return { revealed: id, scrolled: false };
  el = backButton(el) ?? el;
  const middle = () => { const b = viewBox(el); return [b.left + b.width / 2, b.top + b.height / 2]; };
  const [x, y] = middle();
  let seen = x >= 0 && y >= 0 && x < innerWidth && y < innerHeight;
  for (let a = el.parentElement; seen && a && a.id !== "exact-root"; a = a.parentElement) {
    const cs = getComputedStyle(a), b = a.getBoundingClientRect();
    if ((cs.overflowX !== "visible" || cs.overflowY !== "visible") && !(x >= b.left && x < b.right && y >= b.top && y < b.bottom)) seen = false;
  }
  if (["hidden", "collapse"].includes(getComputedStyle(el).visibility)) return { revealed: id, scrolled: false }; // A hidden stand-in must not scroll the page.
  if (!seen) {
    (folded(el) ? el.parentElement : el).scrollIntoView({ block: "center", inline: "nearest", behavior: "instant" });
    return { revealed: id, scrolled: true, from: [x, y], to: clear(el, middle) ?? middle() };
  }
  const to = clear(el, middle);
  return to ? { revealed: id, scrolled: true, from: [x, y], to } : { revealed: id, scrolled: false };
}

// A target whose middle a sticky or fixed box covers (the design tool's edit dock), as the tap's own hit-test finds it:
// its scroller (or the page) scrolled so the middle lands in the band of its port the box leaves, as a person scrolls a
// row out from under a docked bar. Only a box in the target's own scroller, or the page's fixed one, and never one in
// the top layer (a popover, a modal), which no scroll takes away: the tap still refuses there. The middle it moved to,
// or null when nothing covered it or nothing could move.
function clear(el, middle) {
  const [x, y] = middle(), root = el.getRootNode() ?? document, hit = root.elementFromPoint(x, y);
  if (!hit || el === hit || el.contains(hit)) return null;
  // (Anything of the top layer over it, a popover or a modal, is no scroll's to clear, whatever holds it.)
  const above = a => a.closest(":popover-open, dialog[open], :modal");
  if (above(hit)) return null;
  const pinned = a => { const p = getComputedStyle(a).position; return p === "sticky" || p === "fixed"; };
  // The sticky or fixed box a hit at a point is in, or null.
  const coverAt = (h) => { let c = h; while (c && c.id !== "exact-root" && !pinned(c)) c = c.parentElement; return !c || c.id === "exact-root" || c.contains(el) || above(c) ? null : c; };
  const cover = coverAt(hit);
  if (!cover) return null;
  // A sticky box's scroll container, as CSS finds it (the nearest ancestor whose overflow isn't visible on either axis),
  // which must hold the target; for a fixed one, the target's nearest scroller with travel down, else the page.
  const sticky = a => { for (let s = a.parentElement; s && s !== document.body; s = s.parentElement) { const cs = getComputedStyle(s); if (/(auto|scroll|hidden)/.test(cs.overflowX + cs.overflowY)) return s; } return document.scrollingElement; };
  const travel = a => { for (let s = a.parentElement; s && s !== document.body; s = s.parentElement) if (/(auto|scroll)/.test(getComputedStyle(s).overflowY) && s.scrollHeight > s.clientHeight) return s; return document.scrollingElement; };
  const fixed = getComputedStyle(cover).position === "fixed", port = fixed ? travel(el) : sticky(cover);
  if (port !== document.scrollingElement && !port.contains(el)) return null;
  // Vertical only: a scroller with no travel down (a horizontal one, its sticky column) is no band's to clear.
  if (port.scrollHeight <= port.clientHeight) return null;
  const page = port === document.scrollingElement, box = page ? { top: 0, bottom: innerHeight, height: innerHeight } : port.getBoundingClientRect();
  // What of the port the viewport shows, less each cover: the band above one whose middle is in the lower half, below
  // one in the upper. Its middle hit-tested again: a second box there (a header across from a dock) narrows it too.
  let [top, bottom] = [Math.max(box.top, 0), Math.min(box.bottom, innerHeight)];
  const mid = (top + bottom) / 2;
  for (let c = cover, n = 0; c && n < 4 && bottom > top; n++) {
    const r = c.getBoundingClientRect();
    if ((r.top + r.bottom) / 2 > mid) bottom = Math.min(bottom, r.top); else top = Math.max(top, r.bottom);
    const next = bottom > top ? root.elementFromPoint(x, (top + bottom) / 2) : null;
    // (Only a box of the same port, or a fixed one: a sticky of another scroller doesn't stay where this one moves.)
    c = next && !(next === el || el.contains(next)) ? coverAt(next) : null;
    if (c && getComputedStyle(c).position !== "fixed" && sticky(c) !== port) c = null;
  }
  if (bottom <= top) return null;
  // (A scaled port scrolls by its own pixels: the viewport's distance over its scale.)
  const scale = page || !port.clientHeight ? 1 : box.height / port.offsetHeight || 1;
  const before = port.scrollTop;
  port.scrollBy({ top: (y - (top + bottom) / 2) / scale, behavior: "instant" });
  return port.scrollTop === before ? null : middle();
}

// A text folded into its box's content (LLP 1007.001): `display: contents`, or an inline box under a box that restricts
// touch. A paragraph's inline runs are not `data-exact-text`; only the paragraph is.
const folded = el => { const d = getComputedStyle(el).display; return d === "contents" || d === "inline" && el.hasAttribute("data-exact-text"); };

// A view's box as the agent reports it. A folded text has no block box of its own: its box is the anonymous block its
// text is, as a style-less block child's was — its line boxes along the main axis, and its box's content box across
// when the box stretches its items (a block, or a flex or grid box that stretches). A Back control its route's bar
// stands for (index.html) is where that bar's Back is, so a tap that names it presses that Back, as a person does.
export function viewBox(el) {
  const back = backButton(el, false);
  if (back) return back.getBoundingClientRect();
  if (!folded(el)) return el.getBoundingClientRect();
  const range = document.createRange();
  range.selectNodeContents(el);
  const t = range.getBoundingClientRect(), p = el.parentElement;
  if (!p) return t;
  const cs = getComputedStyle(p), b = p.getBoundingClientRect(), n = s => parseFloat(cs[s]) || 0;
  const left = b.left + n("borderLeftWidth") + n("paddingLeft"), right = b.right - n("borderRightWidth") - n("paddingRight");
  const top = b.top + n("borderTopWidth") + n("paddingTop"), bottom = b.bottom - n("borderBottomWidth") - n("paddingBottom");
  const flex = /flex|grid/.test(cs.display), row = flex && cs.display.includes("flex") && !cs.flexDirection.startsWith("column");
  const stretch = !flex || /normal|stretch/.test(cs.alignItems);
  if (row) return stretch ? new DOMRect(t.x, top, t.width, bottom - top) : t;
  return stretch ? new DOMRect(left, t.y, right - left, t.height) : t;
}

// How far a scroller's offset travels on each axis from one end to the other (LLP 1012 `rx`/`ry`), whether or not the
// person can scroll it that way: `overflow: hidden` keeps a programmatic range.
export function scrollRange(el) {
  const r2 = (x) => Math.round(x * 100) / 100;
  return { rx: r2(Math.max(0, el.scrollWidth - el.clientWidth)), ry: r2(Math.max(0, el.scrollHeight - el.clientHeight)) };
}

// The agent reports attached views in viewport coordinates, including folded text.
export function layoutBoxes(views, gpu) {
  const r2 = (x) => Math.round(x * 100) / 100;
  const nodes = [];
  for (const [id, el] of [...views].sort((a, b) => a[0] - b[0])) {
    if (!el.isConnected) continue;
    const r = gpu?.placementHidden(el) ? new DOMRect() : viewBox(el);
    const n = { id, x: r2(r.x), y: r2(r.y), w: r2(r.width), h: r2(r.height) };
    if (el instanceof HTMLIFrameElement) {
      const hit = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
      n.hit = hit === el;
    }
    if (el.dataset.scroll === "true") { n.sx = r2(el.scrollLeft); n.sy = r2(el.scrollTop); Object.assign(n, scrollRange(el)); }
    nodes.push(n);
  }
  return nodes;
}
// The agent's select-all before it types, as a person's ⌘A. HTML's "set the selection range" queues `select` only
// when the selection changes (and Firefox fires none at a disabled control); the promise is that event's, so the
// driver's next input cannot overtake it and leave its handler reading the selection after the input. Its 2 s bound
// only stops a hang where no `select` comes (a control disabled before it is dispatched).
export function selectAll(el) {
  // Email and number fields keep a selection that `select()` sets and their getters do not show (HTML: `select()`
  // applies to them, `selectionStart` does not); Chrome's document selection shows a focused field's. Elsewhere
  // (Firefox's document selection does not) the two reads agree and nothing is awaited.
  const at = () => {
    try { if (el.selectionStart != null) return `${el.selectionStart},${el.selectionEnd},${el.selectionDirection}`; } catch {}
    if (document.activeElement !== el) return null;
    const s = getSelection();
    return `${s.type}:${s.direction}:${s}`;
  }, was = at();
  let heard, timer;
  const event = new Promise(resolve => {
    heard = () => { clearTimeout(timer); el.removeEventListener("select", heard); resolve(); };
    el.addEventListener("select", heard);
  });
  el.select?.();
  const now = at();
  if (el.matches(":disabled") || (was !== null && now === was)) {
    el.removeEventListener("select", heard);
    return null;
  }
  timer = setTimeout(heard, 2000);
  // An email or number field not focused before or after says nothing either way: a changed selection's `select`
  // is dispatched by the next frame (Chromium queues it for one), so it is awaited for two.
  if (was === null || now === null) requestAnimationFrame(() => requestAnimationFrame(heard));
  return event;
}
// LLP 1069.001 D9: `type <id> <value>` sets a control's value as the platform would on a choice or a release: HTML's
// `input`, then `change`. A select takes one of its enabled options by value, else by its one label (Playwright's
// `selectOption`, the diaries' kanban F17 and shop F10); a checkbox takes `true` or `false` and is clicked when that
// differs, as a person would. The page's `inertAncestor` and `settleValue` (navigation.js) are passed in.
export function typeControl(el, request, inertAncestor, settleValue) {
  let text = String(request.text ?? "");
  const id = request.id;
  if (el.disabled || inertAncestor(el)) return { handled: true, error: `view ${id} is disabled or inert` };
  // A read-only field takes no value a person could enter. HTML's `readonly` applies to a textarea and the
  // text-like and date/time inputs only; a checkbox, range, color or select ignores it.
  if (el.readOnly && (el instanceof HTMLTextAreaElement || /^(text|search|url|tel|email|password|date|month|week|time|datetime-local|number)$/.test(el.type))) return { handled: true, error: `view ${id} is read-only` };
  if (el.type === "checkbox") {
    if (text !== "true" && text !== "false") return { handled: true, error: `checkbox ${id} takes true or false, not ${JSON.stringify(text)}` };
    if (el.checked !== (text === "true")) el.click();
    return { typed: id, checked: el.checked, delivery: "recognized", handled: true };
  }
  // A radio is checked by choosing it, and unchecked only by checking another (x2apps survey #2).
  if (el.type === "radio") {
    if (text !== "true") return { handled: true, error: text === "false" ? `radio ${id} is unchecked by checking another of its group` : `radio ${id} takes true, not ${JSON.stringify(text)}` };
    if (!el.checked) el.click();
    return { typed: id, checked: el.checked, delivery: "recognized", handled: true };
  }
  if (el instanceof HTMLSelectElement) {
    const enabled = [...el.options].filter((o) => !o.disabled), labelled = enabled.filter((o) => o.label.trim() === text.trim());
    if (!enabled.some((o) => o.value === text) && labelled.length === 1) text = labelled[0].value;
    else if (!enabled.some((o) => o.value === text))
      return { handled: true, error: `select ${id} has no enabled option ${JSON.stringify(text)}${labelled.length > 1 ? " (that label is on more than one option: choose by value)" : ""} (options: ${enabled.map((o) => `${JSON.stringify(o.value)} ${JSON.stringify(o.label)}`).join(", ")})` };
  }
  if (el.type === "range" && !(text.trim() !== "" && Number.isFinite(Number(text)))) return { handled: true, error: `${JSON.stringify(text)} is not a number` };
  el.value = text;
  if (el.value !== text && el instanceof HTMLInputElement && el.type !== "range") return settleValue(el), { handled: true, error: `${JSON.stringify(text)} is not a value an input type=${el.type} takes; it sanitized to ${JSON.stringify(el.value)}` };
  el.dispatchEvent(new Event("input", { bubbles: true }));
  el.dispatchEvent(new Event("change", { bubbles: true }));
  return { typed: id, value: el.value, delivery: "recognized", handled: true };
}
// A scroller moving on its own after an input (a fling's glide, a smooth scroll, a page that scrolls it every frame):
// its `scroll` events go on. `clock settle` waits until two whole frames pass with none, 4 s at most, as a native
// host waits for a scroll view a finger left moving (agent.mjs reason `scroll`). Two frames: a scroll written in a
// frame's callbacks has its event at the next frame's rendering step. (Not `scrollend`: a page's instant step each
// frame ends each time.) Watched from the agent's first operation on: never in an app's own run.
let scrolls = 0, glides = false;
export function watchGlides() {
  if (glides) return;
  glides = true;
  addEventListener('scroll', () => { scrolls += 1; }, { capture: true, passive: true });
}
/** The `scroll` events seen so far: a settle round compares it before and after its work. */
export const scrollsSeen = () => scrolls;
/** Waits until two frames pass with no `scroll` event: whether they did before scrolling outlasted `budget.ms`, which
 * the wait spends (one budget for a whole `clock settle`, so only time spent waiting on scrolls counts against its
 * 4 s; a round that comes after it is spent still answers true when nothing scrolls). A frame that doesn't come
 * (a throttled page) answers false. */
export async function glidesSettled(budget) {
  const started = performance.now(), deadline = started + budget.ms;
  // (a frame gets at least 250 ms, past the budget too: a page drawing frames, however slowly, shows two quiet ones)
  const frame = () => new Promise(r => {
    const timer = setTimeout(() => r(false), Math.max(250, deadline - performance.now()));
    requestAnimationFrame(() => { clearTimeout(timer); r(true); });
  });
  try {
    for (;;) {
      const before = scrolls;
      if (!(await frame()) || !(await frame())) return false;
      if (scrolls === before) return true;
      if (performance.now() >= deadline) return false;
    }
  } finally {
    budget.ms = Math.max(0, budget.ms - (performance.now() - started));
  }
}
export const agentInput = { backButton, guestOutline, guestTap, guestType, reveal, viewBox, layoutBoxes, scrollRange, selectAll, typeControl, watchGlides, glidesSettled, scrollsSeen };
if (globalThis.exact) Object.assign(globalThis.exact, { agentInput });
