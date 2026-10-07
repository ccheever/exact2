// `tap <target> drag …` (LLP 1080.000 §11), the driver's half: validation,
// the viewport, and the route to the touch runner (`host/apple/touches.mjs`)
// or to the carrier's own contact phases. `agent.mjs` calls it from `tap`.
import { DRAG_BOUNDS } from '../host/apple/touches.mjs';
import { layoutArgs } from './agent-inspect.mjs';

/** A drag's `during` op is a read or the clock, not a second finger (drums R8).
 * A filmed screenshot (`over … every`) loops on the clock. The test parser
 * checks the same set. */
export function duringAllowed(op) {
  const w = String(op).trim().split(/\s+/);
  return ['tree', 'layout', 'state', 'logs', 'screenshot', 'clock'].includes(w[0]) && !(w[0] === 'screenshot' && w[2] === 'over');
}

/** Run one allowed `during` op on the session. The reply is the op's. */
export async function duringOp(s, op) {
  if (!duringAllowed(op)) throw new Error(`tap … drag … during: ${JSON.stringify(op)} is not a read or the clock`);
  const args = String(op).trim().split(/\s+/), word = args[0];
  if (word === 'clock') return s.clock(args.slice(1).join(' ') || 'settle');
  if (word === 'logs') return s.logs();
  if (word === 'tree') return args[1] === '--ax' ? s.tree(args[2], { ax: true }) : s.tree(args[1], args[2] === 'under' ? args[3] : undefined);
  if (word === 'layout') return s.layout(...layoutArgs(args.slice(1)));
  if (word === 'screenshot') return s.screenshot(args[1] ?? 'screenshot.png', args[3] === 'save' ? args[2] : args[2] === 'window', args[3]);
  return s.state(args[1], args[2] === 'under' ? args[3] : undefined, args[2] === 'pose', args[2] === 'busy', {
    ...(args.includes('from') ? { from: Number(args[args.indexOf('from') + 1]) } : {}),
    ...(args.includes('limit') ? { limit: Number(args[args.indexOf('limit') + 1]) } : {}),
    ...(args.includes('resources') ? { resources: true } : {}),
  });
}

/**
 * `tap <target> drag …` (LLP 1080.000 §11): one whole gesture from `from`
 * (an offset from the target's box, its middle by default), a finger or,
 * with `mouse`, the left button, `modifiers` held throughout: press `press`
 * ms, one straight drag by (dx, dy) over `over` ms, hold `hold` ms, lift;
 * `during` thunks run while the finger is down after the move, before the
 * hold (kanban F14: a screenshot during a drag shows it moved), when no
 * input is accepted (`s.held`). The start and the end must be in the viewport. A real touch
 * under `--touch platform`; elsewhere the carrier's own contact phases,
 * refused where the carrier refuses them.
 */
// What the host journaled about a reorder during the drag (LLP 1102 §3.17): a lift refused while
// the last drop's session holds, or a touch the browser took, which a reply would otherwise read as
// a success. Advice only: a journal read that fails or is slow says nothing.
const quick = (p) => Promise.race([p.catch(() => null), new Promise((done) => setTimeout(() => done(null), 2000))]);
async function journalMark(s) { return (await quick(s.op({ op: 'logs', since: Number.MAX_SAFE_INTEGER })))?.next ?? null; }
async function reorderNotes(s, since) {
  if (since == null) return [];
  const j = await quick(s.op({ op: 'logs', since }));
  return (Array.isArray(j?.lines) ? j.lines : []).map((l) => (typeof l === 'string' ? l : JSON.stringify(l)).replace(/^t=\S+ /, '')).filter((l) => l.startsWith('reorder: '));
}
const noted = (r, notes) => (notes.length ? { ...r, note: [r.note, ...notes].filter(Boolean).join('; ') } : r);

export async function dragTap({ s, carrier, node, target, host, timing, tapRefusal, scrolled }, opts) {
  // `drag to B [at x y]` (LLP 1094 D12): the delta from both boxes at the press, to B's middle or to (x, y) from its
  // top left; B unmounted or off screen is refused by name. An autoscrolling drag is `drag dx dy hold ms`.
  if (opts.to !== undefined) opts = { ...opts, ...await toward(s, node, opts) };
  const { dx, dy, from, mouse = false, modifiers, press = 0, over = 250, hold = 0, during = [] } = opts, drag = { dx, dy, press, over, hold, during }, said = { dx, dy, press, over, hold, ...(mouse ? { mouse } : {}), ...(modifiers ? { modifiers } : {}), ...(opts.to !== undefined ? { to: opts.to } : {}) };
  // `mouse` (files diary F10): the left button, where the carrier's contact
  // is otherwise a finger (the web's); a desktop host's contact is the mouse.
  if (mouse && (carrier.touches || ['ios', 'host-ios'].includes(host))) throw new Error('drag: mouse is a desktop pointer\'s; an iOS contact is a finger');
  if (![dx, dy, press, over, hold].every(Number.isFinite) || [press, over, hold].some((v) => v < 0)) throw new Error('drag: expected finite dx, dy and non-negative press, over and hold (ms)');
  if (from !== undefined && !(Array.isArray(from) && from.length === 2 && from.every(Number.isFinite))) throw new Error('drag: from takes two finite numbers, an offset from the target\'s box');
  const moves = dx !== 0 || dy !== 0;
  if (moves && over <= 0) throw new Error('drag: a drag that moves needs over > 0');
  for (const k of ['press', 'over', 'hold']) if (drag[k] > DRAG_BOUNDS[k]) throw new Error(`drag: ${k} ${drag[k]} ms is past its bound, ${DRAG_BOUNDS[k]} ms`);
  if (press + hold + (moves ? over : 0) > DRAG_BOUNDS.total) throw new Error(`drag: the gesture lasts past ${DRAG_BOUNDS.total} ms`);
  // The touch runner lifts at the end of its scripted hold, and its ops start after the press and the move: without a hold they would find the finger lifted.
  if (carrier.touches && during.length && !(hold > 0)) throw new Error('drag: under --touch platform, during runs after the press and the move, inside the scripted hold: give hold <ms>, the time the ops run in');
  if (s.contact) throw new Error('a contact is already down; use `tap up` or `tap cancel` first');
  const layout = await s.layout(), b = layout.nodes.find((n) => n.id === node.id), vp = layout.viewport;
  const mark = await journalMark(s);
  if (!b) throw new Error(`view ${node.id} has no box on screen`);
  const start = from ? [b.x + from[0], b.y + from[1]] : [b.x + b.w / 2, b.y + b.h / 2], end = [start[0] + dx, start[1] + dy];
  const inside = ([x, y]) => x >= 0 && y >= 0 && x <= vp.w && y <= vp.h;
  if (vp && !(inside(start) && inside(end))) throw new Error(`drag: from (${start}) to (${end}) leaves the viewport (${vp.w} × ${vp.h})`);
  // Each op runs with input refused; the runner path bounds each one by the gesture.
  const held = (op) => async () => { s.held = target; try { return await op(); } finally { s.held = null; } };
  if (carrier.touches) {
    let r;
    try { r = await carrier.input(node.id, 'drag', { at: from ? start : undefined, drag: { ...drag, during: during.map(held) } }); }
    catch (error) { throw await tapRefusal(s, target, error); }
    return noted(await s.landed({ ...r, target, ...(scrolled ? { scrolled } : {}), carrier: host, mode: timing }), await reorderNotes(s, mark));
  }
  // The carrier's phases, each reply checked: an error or a refusal releases the contact and throws.
  let down, done = [], up;
  const phase = async (name, opts) => {
    const r = await s.pointer(name, opts);
    if (r.error || r.delivery === 'unsupported') throw new Error(`drag: ${name}: ${r.error ?? r.reason ?? 'unsupported'}`);
    return r;
  };
  try {
    // The contact starts here, through the carrier, at the point `tap … down at` would use (review A1): `tap` refuses
    // `mouse` beside `down`, its click form. `mouse` holds the left button on the web, Linux and Windows; a macOS
    // contact already is the mouse.
    // `modifiers` are held from the press to the lift (#107: a Shift-drag extends a selection).
    try { down = await carrier.input(node.id, 'down', { x: start[0], y: start[1], ...(mouse && !['macos', 'mac', 'host'].includes(host) ? { mouse } : {}), ...(modifiers ? { modifiers } : {}) }); }
    catch (error) { throw await tapRefusal(s, target, error); }
    if (down.error) throw new Error(`drag: down: ${(await tapRefusal(s, target, new Error(down.error))).message}`);
    if (down.delivery !== 'unsupported') {
      s.contact = down.contact === false ? null : { x: down.at[0], y: down.at[1] };
      if (Number.isFinite(down.clock)) s.now = down.clock;
    }
    if (down.delivery === 'unsupported') {
      const { phase: _, ...refused } = down;
      return s.tagged({ ...refused, drag: said, reason: `${down.reason ?? 'no held contact'}; a real drag is --touch platform's (LLP 1080.000 §11)` });
    }
    // press and hold seek the virtual clock (platformer R7). `virtual` tells
    // web and macOS to skip the wall sleep; Linux already seeks and reports `clock`.
    const seek = timing !== 'platform' ? { virtual: true } : {};
    if (press) await phase('hold', { ms: press, ...seek });
    if (moves) await phase('move', { dx, dy, ms: over });
    // The ops run where the finger has moved to, each bounded by the gesture's own bound.
    for (const op of during) {
      let timer;
      const late = new Promise((_, no) => { timer = setTimeout(() => no(new Error(`drag: an op during it outlasted ${DRAG_BOUNDS.total} ms`)), DRAG_BOUNDS.total); });
      const running = held(op)();
      running.catch(() => {});
      try { done.push(await Promise.race([running, late])); } finally { clearTimeout(timer); }
    }
    if (hold) await phase('hold', { ms: hold, ...seek });
    up = await phase('up');
  } catch (error) {
    // Never leave the finger down: cancel, else lift (AppKit has no cancel). If neither is confirmed the contact stays recorded, and says so.
    // A `down` that threw may still have pressed: release at the carrier, whose own contact decides.
    if (!down) for (const name of ['cancel', 'up']) await carrier.input(null, name, {}).catch(() => null);
    for (const name of ['cancel', 'up']) {
      if (!s.contact) break;
      const r = await s.pointer(name).catch(() => null);
      if (r && !r.error && r.delivery !== 'unsupported') s.contact = null;
    }
    if (s.contact) error.message += '; the contact could not be released (tap cancel, or close the session)';
    throw error;
  }
  return noted(await s.tagged({ tapped: node.id, target, ...(scrolled ? { scrolled } : {}), at: down.at, drag: said, lifted: up.at, ...(done.length ? { during: done } : {}), delivery: down.delivery, carrier: host, mode: timing }), await reorderNotes(s, mark));
}

/** `drag to`'s delta: from the start (`from`, else the middle) to B's middle, or `at` from B's top left. */
async function toward(s, node, { to, at, from }) {
  if (at !== undefined && !(Array.isArray(at) && at.length === 2 && at.every(Number.isFinite))) throw new Error('drag to: at takes two finite numbers, an offset from the target\'s box');
  const end = await s.find(to, false);
  if (!end) throw new Error(`drag to: ${to} is not mounted`);
  const layout = await s.layout(), a = layout.nodes.find((n) => n.id === node.id), b = layout.nodes.find((n) => n.id === end.id), vp = layout.viewport;
  if (!a) throw new Error(`view ${node.id} has no box on screen`);
  if (!b || (vp && (b.x + b.w <= 0 || b.y + b.h <= 0 || b.x >= vp.w || b.y >= vp.h))) throw new Error(`drag to: ${to} is off screen; drag dx dy hold ms scrolls a list or a board toward it`);
  const start = from ? [a.x + from[0], a.y + from[1]] : [a.x + a.w / 2, a.y + a.h / 2];
  const point = at ? [b.x + at[0], b.y + at[1]] : [b.x + b.w / 2, b.y + b.h / 2];
  return { dx: point[0] - start[0], dy: point[1] - start[1] };
}
