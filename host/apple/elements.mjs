// An app's on-screen elements as the touch runner (touches.mjs) reads them:
// fetched, outlined, and found by what a person reads on screen. Shared by the
// bake-off's driver (bench/bakeoff/drive.mjs) and the scroll-feel probe's
// steps (host/apple/feel/probe.mjs).

/** A refusal to act, said to the caller as is (no stack). */
export class Refusal extends Error {}
export const refuse = (message) => { throw new Refusal(message); };

// Elements that only group others: left out of the outline when they carry no text.
const SILENT = new Set(['Other', 'Group', 'Window', 'LayoutArea', 'LayoutItem']);
// What a person taps: of a match nested in another, the outer one when it is one of these.
const TAPPABLE = new Set(['Button', 'Cell', 'Link', 'Switch', 'Toggle', 'TextField', 'SecureTextField', 'SearchField', 'TextView',
  'Slider', 'Stepper', 'SegmentedControl', 'Tab', 'MenuItem', 'PopUpButton', 'MenuButton', 'ToolbarButton', 'CheckBox', 'RadioButton']);
// Identifiers a toolkit generates (Exact's `exact-…`, anything carrying a UUID): they would tell a blind grader how
// an app was built, and no person sees them.
const GENERATED = /^exact-|[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-/i;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** The tree (`{ elements, frame, truncated }`) through `ask` (a touch runner's, returning `{ error }` on failure),
 * generated identifiers dropped. An app launched while no session was open can refuse the first reads
 * (kAXErrorAPIDisabled): retried for 8 s. */
export async function readTree(ask) {
  for (const until = Date.now() + 8000; ;) {
    const t = await ask({ op: 'tree' });
    if (!t.error) {
      for (const e of t.elements) if (e.identifier != null && GENERATED.test(e.identifier)) e.identifier = null;
      return t;
    }
    if (!/APIDisabled/.test(t.error) || Date.now() > until) refuse(t.error + (/not in the foreground/.test(t.error) ? '; launch it first' : ''));
    await sleep(250);
  }
}

/** One element as the outline shows it. */
export function line(e) {
  const [x, y, w, h] = e.frame.map(Math.round);
  const text = (s) => JSON.stringify(s.length > 80 ? s.slice(0, 80) + '…' : s);
  return [e.type, e.label != null ? text(e.label) : null, e.identifier != null ? `id=${text(e.identifier)}` : null,
    e.value != null ? `value=${text(e.value)}` : null, e.placeholder != null ? `placeholder=${text(e.placeholder)}` : null,
    e.enabled ? null : 'disabled', e.selected ? 'selected' : null, e.hasFocus ? 'focused' : null, `[${x},${y} ${w}x${h}]`].filter(Boolean).join(' ');
}

const silent = (e) => SILENT.has(e.type) && e.label == null && e.identifier == null && e.value == null && e.placeholder == null;

/** The tree as an indented outline: wrappers with no text are left out, a keyboard is one line. */
export function outline({ elements, truncated }) {
  const out = [], stack = [], seen = new Set();
  let keyboard = null;
  for (const e of elements) {
    if (keyboard != null && e.depth > keyboard) continue;
    keyboard = null;
    while (stack.length && stack.at(-1).depth >= e.depth) stack.pop();
    const shown = !silent(e);
    const text = '  '.repeat(stack.filter((s) => s.shown).length) + line(e);
    // An element some apps expose twice (a scroll bar, a keyboard's button) is shown once.
    if (shown && !seen.has(text)) out.push(text), seen.add(text);
    stack.push({ depth: e.depth, shown });
    if (e.type === 'Keyboard') keyboard = e.depth;
  }
  if (truncated) out.push(`… (cut at ${elements.length} elements)`);
  return out.join('\n');
}

/** The element `target` names, with its index: by label, identifier, placeholder or title, exactly, then ignoring
 * case, then as part of a label. Of two nested matches the outer is kept when a person would tap it (a button
 * around its text), else the inner; keyboard keys only when nothing else matches. */
export function find(elements, target) {
  const lower = target.toLowerCase();
  const tests = [
    (e) => [e.label, e.identifier, e.placeholder, e.title].includes(target),
    (e) => [e.label, e.identifier, e.placeholder, e.title].some((s) => s?.toLowerCase() === lower),
    (e) => e.label?.toLowerCase().includes(lower),
  ];
  for (const test of tests) {
    let found = elements.map((e, i) => ({ e, i })).filter(({ e }) => test(e));
    if (!found.length) continue;
    if (found.some(({ e }) => e.type !== 'Key')) found = found.filter(({ e }) => e.type !== 'Key');
    const losers = new Set();
    for (const a of found) for (const d of found) {
      if (d.i > a.i && within(elements, a.i, d.i)) losers.add(TAPPABLE.has(a.e.type) ? d.i : a.i);
    }
    const kept = found.filter(({ i }) => !losers.has(i));
    if (kept.length === 1) return kept[0];
    refuse(`${kept.length} elements match ${JSON.stringify(target)}; tap one by its point:\n${kept.slice(0, 12).map(({ e }) => '  ' + line(e)).join('\n')}`);
  }
  refuse(`nothing labelled ${JSON.stringify(target)}; run tree to see the labels, or tap x,y`);
}

/** Whether element `d` is inside element `a` (indices into the flattened tree). */
const within = (elements, a, d) => elements.slice(a + 1, d + 1).every((x) => x.depth > elements[a].depth);

/** Where a person taps element `i`: its middle, or for a switch, the middle of the switch control inside it (a
 * SwiftUI toggle's row is labelled as a switch, but only the control turns it). */
function aim(elements, i) {
  let e = elements[i];
  if (e.type === 'Switch' || e.type === 'Toggle') {
    for (let j = i + 1; j < elements.length && elements[j].depth > e.depth; j++) {
      if (elements[j].type === e.type) { e = elements[j]; break; }
    }
  }
  return e.frame;
}

/** The point (app points) a person taps for `target` in tree `t`, and the element: the middle of its part on screen. */
export function tapPoint(t, target) {
  const { e, i } = find(t.elements, target), f = aim(t.elements, i);
  const [fx, fy, fw, fh] = t.frame;
  const [x0, y0] = [Math.max(f[0], fx), Math.max(f[1], fy)];
  const [x1, y1] = [Math.min(f[0] + f[2], fx + fw), Math.min(f[1] + f[3], fy + fh)];
  if (x1 <= x0 || y1 <= y0) refuse(`${line(e)} is off screen; swipe to bring it into view`);
  return { x: Math.round((x0 + x1) / 2), y: Math.round((y0 + y1) / 2), e };
}
