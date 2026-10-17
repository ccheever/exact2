// `tap <target> drag …` (LLP 1080.000 §11), the driver's half: validation,
// the viewport, and the route to the touch runner (`host/apple/touches.mjs`)
// or to the carrier's own contact phases. `agent.mjs` calls it from `tap`.
import { MalformedOperation, pointerArguments, tapWords } from './agent-keys.mjs';
import { DRAG_BOUNDS, REACH_MS } from '../host/apple/touches.mjs';
import { layoutArgs, perfOp } from './agent-inspect.mjs';

/** A drag's `during` op is a read or the clock, not a second finger (drums R8).
 * A filmed screenshot (`over … every`) loops on the clock, a comparison settles
 * first, and `perf … during` drives ops of its own. The test parser checks the
 * same set, less `perf`. */
export function duringAllowed(op) {
  const w = String(op).trim().split(/\s+/);
  return ['tree', 'layout', 'state', 'logs', 'screenshot', 'clock', 'perf'].includes(w[0]) && !(w[0] === 'screenshot' && ['over', 'compare'].includes(w[2])) && !(w[0] === 'perf' && (w.includes('during') || w.includes('live')));
}

/** Run one allowed `during` op on the session. The reply is the op's. */
export async function duringOp(s, op) {
  if (!duringAllowed(op)) throw new MalformedOperation(`tap … drag … during: ${JSON.stringify(op)} is not a read or the clock`);
  const args = String(op).trim().split(/\s+/), word = args[0];
  if (word === 'clock') return s.clock(args.slice(1).join(' ') || 'settle');
  if (word === 'logs') return s.logs();
  if (word === 'perf') return perfOp(s, args.slice(1), String(op));
  if (word === 'tree') return args[1] === '--ax' ? s.tree(args[2], { ax: true }) : s.tree(args[1], args[2] === 'under' ? args[3] : undefined);
  if (word === 'layout') return s.layout(...layoutArgs(args.slice(1)));
  // Mid-gesture, as it stands: no settle (LLP 1116.003 D8 settles a screenshot otherwise).
  if (word === 'screenshot') return s.screenshot(args[1] ?? 'screenshot.png', args[3] === 'save' ? args[2] : args[2] === 'window', args[3], true);
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

export function dragArguments(opts) {
  const { dx, dy, from, press = 0, over = 250, hold = 0 } = opts, drag = { press, over, hold };
  if (![...(opts.to === undefined ? [dx, dy] : []), press, over, hold].every(Number.isFinite) || [press, over, hold].some((v) => v < 0)) throw new MalformedOperation('drag: expected finite dx, dy and non-negative press, over and hold (ms)');
  if (from !== undefined && !(Array.isArray(from) && from.length === 2 && from.every(Number.isFinite))) throw new MalformedOperation('drag: from takes two finite numbers, an offset from the target\'s box');
  const moves = opts.to === undefined && (dx !== 0 || dy !== 0);
  if (moves && over <= 0) throw new MalformedOperation('drag: a drag that moves needs over > 0');
  for (const k of ['press', 'over', 'hold']) if (drag[k] > DRAG_BOUNDS[k]) throw new MalformedOperation(`drag: ${k} ${drag[k]} ms is past its bound, ${DRAG_BOUNDS[k]} ms`);
  if (press + hold + (moves ? over : 0) > DRAG_BOUNDS.total) throw new MalformedOperation(`drag: the gesture lasts past ${DRAG_BOUNDS.total} ms`);
  if (opts.at !== undefined && !(Array.isArray(opts.at) && opts.at.length === 2 && opts.at.every(Number.isFinite))) throw new MalformedOperation('drag to: at takes two finite numbers, an offset from the target\'s box');
}

export async function dragTap({ s, carrier, node, target, host, timing, tapRefusal, scrolled }, opts) {
  dragArguments(opts);
  // `drag to B [at x y]` (LLP 1094 D12): the delta from both boxes at the press, to B's middle or to (x, y) from its
  // top left; B unmounted or off screen is refused by name. An autoscrolling drag is `drag dx dy hold ms`.
  if (opts.to !== undefined) {
    opts = { ...opts, ...await toward(s, node, opts) };
    // A failure caused by the layout's delta is a refusal, not malformed input.
    try { dragArguments({ ...opts, to: undefined }); }
    catch (error) { throw new Error(error.message); }
  }
  const { dx, dy, from, mouse = false, modifiers, press = 0, over = 250, hold = 0, during = [] } = opts, drag = { dx, dy, press, over, hold, during }, said = { dx, dy, press, over, hold, ...(mouse ? { mouse } : {}), ...(modifiers ? { modifiers } : {}), ...(opts.to !== undefined ? { to: opts.to } : {}) };
  // `mouse` (files diary F10): the left button, where the carrier's contact
  // is otherwise a finger (the web's); a desktop host's contact is the mouse.
  if (mouse && (carrier.touches || ['ios', 'host-ios'].includes(host))) throw new Error('drag: mouse is a desktop pointer\'s; an iOS contact is a finger');
  const moves = dx !== 0 || dy !== 0;
  // The touch runner lifts at the end of its scripted hold, and its ops start after the press and the move: without a hold they would find the finger lifted.
  if (carrier.touches && during.length && !(hold > 0)) throw new Error('drag: under --touch platform, during runs after the press and the move, inside the scripted hold: give hold <ms>, the time the ops run in');
  if (s.contact) throw new Error('a contact is already down; use `tap up` or `tap cancel` first');
  const layout = await s.layout(), b = layout.nodes.find((n) => n.id === node.id), vp = layout.viewport;
  const mark = await journalMark(s);
  if (!b) throw new Error(`view ${node.id} has no box on screen`);
  const start = from ? [b.x + from[0], b.y + from[1]] : [b.x + b.w / 2, b.y + b.h / 2], end = [start[0] + dx, start[1] + dy];
  const inside = ([x, y]) => x >= 0 && y >= 0 && x <= vp.w && y <= vp.h;
  if (vp && !(inside(start) && inside(end))) {
    // A scroller dragged further than the viewport reaches scrolls the whole way (euxy FRICTION 14). On iOS it is the
    // scroll view's own drag, which a viewport does not bound, under real touches too: XCTest's drag flings at its lift
    // whatever it holds first (300 pt held 300 ms scrolled 540), so real strokes would not scroll what was asked.
    // Elsewhere a finger's strokes (`strokes`).
    if (!(inside(start) && ['ScrollView', 'List'].includes(b.type) && !during.length && opts.to === undefined)) throw new Error(`drag: from (${start}) to (${end}) leaves the viewport (${vp.w} × ${vp.h})`);
    if (!['ios', 'host-ios'].includes(host)) return strokes(s, target, opts, start, vp);
    const r = await carrier.ask({ op: 'tap', id: node.id, scrollDrag: [dx, dy], ...(from ? { from: start } : {}) });
    if (r.error) throw new Error(r.error);
    // Landed as an input is (`s.landed`): what the scroll settled lands its `then`s before the reply, and a landing error is said.
    const real = carrier.touches ? ['past the screen a drag is the scroll view\'s own (recognized), not a real touch: one ends inside the screen'] : [];
    return noted(await s.landed({ ...r, target, ...(scrolled ? { scrolled } : {}), drag: said, carrier: host, mode: timing }), [...real, ...await reorderNotes(s, mark)]);
  }
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
  // A phase's note (a refused write, said once a drive) is the drag's: its reply carries it (`noted`).
  const phaseNotes = [];
  const phase = async (name, opts) => {
    const r = await s.pointer(name, opts);
    if (r.error || r.delivery === 'unsupported') throw new Error(`drag: ${name}: ${r.error ?? r.reason ?? 'unsupported'}`);
    if (r.landError) throw new Error(`drag: ${name} was delivered; landing what it started failed: ${r.landError}`);
    if (r.note) phaseNotes.push(r.note);
    return r;
  };
  try {
    // The contact starts here, through the carrier, at the point `tap … down at` would use (review A1): `tap` refuses
    // `mouse` beside `down`, its click form. `mouse` holds the left button on the web, Linux and Windows; a macOS
    // contact already is the mouse.
    // `modifiers` are held from the press to the lift (#107: a Shift-drag extends a selection).
    // On iOS only a `pan` node's recognized contact drags (`pan`): a finger held elsewhere would move nothing.
    try { down = await carrier.input(node.id, 'down', { x: start[0], y: start[1], ...(mouse && !['macos', 'mac', 'host'].includes(host) ? { mouse } : {}), ...(modifiers ? { modifiers } : {}), ...(['ios', 'host-ios'].includes(host) ? { pan: true } : {}) }); }
    catch (error) { throw await tapRefusal(s, target, error); }
    if (down.error) throw new Error(`drag: down: ${(await tapRefusal(s, target, new Error(down.error))).message}`);
    if (down.delivery !== 'unsupported') {
      s.contact = down.contact === false ? null : { x: down.at[0], y: down.at[1] };
      if (Number.isFinite(down.clock)) s.now = down.clock;
    }
    if (down.delivery === 'unsupported' && ['ios', 'host-ios'].includes(host)) {
      // iOS holds no contact across requests: a drag on a scroller is what the scroll view's pan ends with, its content
      // moved with the finger (LLP 1116.003); any other drag is refused, never answered `unsupported` as a success.
      if (during.length) throw new Error('drag: during needs the finger held down, which only --touch platform does on iOS (LLP 1080.000 §11)');
      const r = await carrier.ask({ op: 'tap', id: node.id, scrollDrag: [dx, dy], ...(from ? { from: start } : {}) });
      if (r.error) throw new Error(r.error);
      return noted(await s.tagged({ ...r, target, ...(scrolled ? { scrolled } : {}), drag: said, carrier: host, mode: timing }), await reorderNotes(s, mark));
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
  return noted(await s.tagged({ tapped: node.id, target, ...(scrolled ? { scrolled } : {}), at: down.at, drag: said, lifted: up.at, ...(done.length ? { during: done } : {}), delivery: down.delivery, carrier: host, mode: timing }), [...new Set(phaseNotes), ...await reorderNotes(s, mark)]);
}

/** How much longer than its holds a played contact with steps inside stays down: `HELD_LEAD` to see the touch begin
 *  in the dispatch log and to end the clock's real run, `REACH_MS` for the longest wait for it to reach its view (the
 *  touch runner's timer is already running), and `HELD_READ` for each read (a simulator's screenshot takes about
 *  100 ms). */
export const HELD_LEAD = 250, HELD_READ = 150;

/**
 * `tap <target> down` … `tap up` under `--touch platform`: one real touch, so a native view's own recognizers (a
 * `UILongPressGestureRecognizer`) see it held. The runner holds no finger across requests (LLP 1080.000 P3), so the
 * contact is played whole at its lift: `down` books the point (`at`, else the target's middle, a viewport point kept
 * from then on), the holds, reads and `clock +<ms>` steps between are kept in order (`heldRead`, a hold through the
 * session's `pointer` too), `up` presses there, and `cancel` drops it with nothing pressed. Once the touch has reached
 * its view the steps run in order, a hold or a `clock +<ms>` moving the app's clock on real time (a module's
 * `context.after` and UIKit's recognizers keep their order, as on a device), and the finger lifts after them. Under
 * either timing the clock is then the agent's (LLP 1080.000 §12: any landed input takes it). A move is a drag's
 * (`tap <target> drag`).
 */
export async function heldDown(s, node, at) {
  const b = (await s.layout()).nodes.find((n) => n.id === node.id);
  if (!b) throw new Error(`view ${node.id} has no box on screen`);
  const point = at ? [at.x, at.y] : [b.x + b.w / 2, b.y + b.h / 2];
  s.contact = { x: point[0], y: point[1], whole: { id: node.id, hold: 0, queue: [] } };
  return { contact: node.id, phase: 'down', at: point, delivery: 'platform', note: 'under --touch platform a held contact is one real touch, played at tap up: the holds, reads and clock +<ms> steps before then run inside it, pressed, and answer at tap up' };
}

const heldTime = (whole, ms) => {
  if (whole.hold + ms > DRAG_BOUNDS.hold) throw new MalformedOperation(`a held real touch lasts at most ${DRAG_BOUNDS.hold} ms`);
  whole.hold += ms;
};

/** An op line written between `tap <target> down` and `tap up` of a played contact (`heldDown`): a `tap hold`, a read
 *  or a `clock +<ms>` is kept, in order, to run inside the touch at its lift, and its reply is given then, in the
 *  order written (`s.heldReads`). `run` performs the line as the drive would and answers `[op, reply]`; inside the
 *  touch a screenshot does not settle (`s.held`). A hold and a `clock +<ms>` count toward the press; input, and a
 *  `clock` that waits (`settle`, `data`) or seeks, is refused by name. False for a move, up or cancel, or with no
 *  played contact down. */
export function heldRead(s, line, run) {
  const whole = s.contact?.whole, text = line.trim(), w = text.split(/\s+/);
  if (!whole || (w[0] === 'tap' && ['move', 'up', 'cancel'].includes(w[1]))) return false;
  if (w[0] === 'tap' && w[1] === 'hold') {
    const [, { ms }] = tapWords(w.slice(1), true);
    pointerArguments('hold', { ms });
    heldTime(whole, ms);
    whole.queue.push({ hold: ms, line: text });
    return true;
  }
  const clock = w[0] === 'clock' && /^\+\d+(\.\d+)?$/.test(w[1] ?? '') && (w.length === 2 || (w.length === 3 && w[2] === 'real'));
  if (!duringAllowed(text) || (w[0] === 'clock' && !clock)) throw new Error(`under --touch platform tap <target> down … tap up is one real touch, played at tap up, and only tap hold, reads and clock +<ms> run inside it (tree, layout, state, logs, screenshot, perf): tap up or tap cancel before \`${text}\``);
  if (clock) heldTime(whole, Number(w[1].slice(1)));
  whole.queue.push({ ...(clock ? { clock: Number(w[1].slice(1)) } : {}), line: text, run });
  return true;
}

export async function heldPhase(s, carrier, phase, opts) {
  const whole = s.contact.whole, at = [s.contact.x, s.contact.y];
  if (phase === 'move') throw new Error('tap move: under --touch platform a finger that moves is one real gesture, tap <target> drag <dx> <dy> [press <ms>] [over <ms>] [hold <ms>]; tap up or tap cancel ends this contact');
  if (phase === 'hold') {
    heldTime(whole, opts.ms);
    whole.queue.push({ hold: opts.ms });
    return { phase, at, delivery: 'platform', held: whole.hold };
  }
  s.contact = null;
  const reads = s.heldReads = [], queue = whole.queue;
  let held = 0;
  // `held`: how long the finger has been held by the steps so far, a hold's reply says.
  const holdReply = async (q, extra = {}) => { held += q.hold; if (q.line) reads.push(['tap', await s.tagged({ phase: 'hold', at, delivery: 'platform', held, ...extra })]); };
  if (phase === 'cancel') {
    // Nothing was pressed: each line still answers, in order, a read as it reads after the cancel.
    for (const q of queue) if (q.hold !== undefined) await holdReply(q, { note: 'nothing was pressed' }); else { held += q.clock ?? 0; reads.push(await q.run()); }
    return { phase, at, delivery: 'platform', note: `nothing was pressed${queue.some((q) => q.run) ? '; the steps written before it ran after it, unpressed' : ''}` };
  }
  // The point the contact went down at, still on its target: a target that moved since is refused, never followed.
  const b = (await s.layout()).nodes.find((n) => n.id === whole.id);
  if (!b || at[0] < b.x || at[1] < b.y || at[0] > b.x + b.w || at[1] > b.y + b.h) throw new Error(`tap up: view ${whole.id} moved off (${at}), where the contact went down, so nothing was pressed: tap it down again`);
  // A step that fails once the touch is delivered is the reply's landError, the receipt kept: a retry would press twice.
  let landError = null;
  const inside = queue.map((q) => async () => {
    s.held = `tap #${whole.id} down`;
    try {
      if (q.hold !== undefined) {
        const r = q.hold ? await s.clock(`+${q.hold} real`) : null;
        await holdReply(q);
        return r;
      }
      if (q.clock !== undefined) {
        held += q.clock;
        const r = await s.clock(`+${q.clock} real`);
        reads.push(['clock', r]);
        return r;
      }
      const out = await q.run();
      reads.push(out);
      return out[1];
    } catch (e) {
      landError ??= `the touch was delivered; ${q.line ? `\`${q.line}\`` : `moving the clock ${q.hold} ms`} while it was down failed: ${e.message ?? e}`;
      if (q.line) reads.push(['failed', { op: q.line, error: e.message ?? String(e) }]);
      return null;
    } finally { s.held = null; }
  });
  const readCount = queue.filter((q) => q.run && q.clock === undefined).length;
  const press = inside.length ? whole.hold + HELD_LEAD + REACH_MS + HELD_READ * readCount : 0;
  let r;
  try { r = await carrier.input(whole.id, 'drag', { at, drag: { dx: 0, dy: 0, press: 0, over: 0, hold: press, during: inside, ...(readCount ? { reach: true } : {}) } }); }
  catch (error) { error.message = error.message.replace(/lengthen hold$/, `the press was delivered; the reads inside it took longer than the ${HELD_READ} ms each it allows them: read less in one press`); throw error; }
  const { during, drag, ...reply } = r;
  return { ...reply, phase, at, hold: whole.hold, ...(landError ? { landError } : {}) };
}

/** The most strokes a drag past the viewport is split into, and how long each but the last rests before it lifts. */
export const STROKES = { most: 8, rest: 150 };

/** A scroller's drag past the viewport (`dragTap`) on a carrier other than iOS, as strokes that stay in it: each moves (dx, dy) scaled down to
 * end inside it (a point short of the edge), over the drag's `over` scaled alike. Each but the last rests
 * `STROKES.rest` ms before it lifts, so it lifts at rest and flings nothing; the last keeps the drag's own hold. Past
 * `STROKES.most` it is refused before any input. A stroke that fails, lands with an error or is not delivered stops
 * it, saying which; the reply is the last stroke's, with the whole drag, the strokes, every stroke's note, and the
 * scrolling summed. */
async function strokes(s, target, opts, [x, y], vp) {
  const { dx, dy, over = 250 } = opts;
  const room = Math.min(dx > 0 ? (vp.w - 1 - x) / dx : dx < 0 ? (x - 1) / -dx : 1, dy > 0 ? (vp.h - 1 - y) / dy : dy < 0 ? (y - 1) / -dy : 1);
  if (!(room > 0)) throw new Error(`drag: from (${x},${y}) there is no room to drag toward (${dx}, ${dy}) inside the viewport (${vp.w} × ${vp.h})`);
  const count = Math.ceil(1 / room - 1e-9);
  if (count > STROKES.most) throw new Error(`drag: (${dx}, ${dy}) from (${x},${y}) is ${count} strokes inside the viewport (${vp.w} × ${vp.h}), past ${STROKES.most}: drag in parts, or from nearer its middle`);
  const parts = [], notes = [], scrolled = [0, 0];
  for (let i = 0; i < count; i++) {
    const part = Math.min(room, 1 - room * i), last = i === count - 1;
    const r = await s.tap(target, { drag: { ...opts, dx: dx * part, dy: dy * part, over: Math.max(1, over * part), press: i ? 0 : opts.press, hold: last ? opts.hold : STROKES.rest } });
    const failed = r?.error ?? r?.landError ?? (r?.delivery === 'unsupported' ? r.reason ?? 'not delivered' : null);
    if (failed) throw Object.assign(new Error(`drag: stroke ${i + 1} of ${count}: ${failed}`), { reply: r });
    if (r.note) notes.push(r.note);
    if (Array.isArray(r.scrolledBy)) { scrolled[0] += r.scrolledBy[0]; scrolled[1] += r.scrolledBy[1]; }
    parts.push(r);
  }
  const r = parts.at(-1);
  return { ...r, drag: { ...r.drag, dx, dy }, strokes: count, ...(r.scrolledBy ? { scrolledBy: scrolled } : {}), ...(notes.length ? { note: [...new Set(notes)].join('; ') } : {}) };
}

/** `drag to`'s delta: from the start (`from`, else the middle) to B's middle, or `at` from B's top left. */
async function toward(s, node, { to, at, from }) {
  const end = await s.find(to, false);
  if (!end) throw new Error(`drag to: ${to} is not mounted`);
  const layout = await s.layout(), a = layout.nodes.find((n) => n.id === node.id), b = layout.nodes.find((n) => n.id === end.id), vp = layout.viewport;
  if (!a) throw new Error(`view ${node.id} has no box on screen`);
  if (!b || (vp && (b.x + b.w <= 0 || b.y + b.h <= 0 || b.x >= vp.w || b.y >= vp.h))) throw new Error(`drag to: ${to} is off screen; drag dx dy hold ms scrolls a list or a board toward it`);
  const start = from ? [a.x + from[0], a.y + from[1]] : [a.x + a.w / 2, a.y + a.h / 2];
  const point = at ? [b.x + at[0], b.y + at[1]] : [b.x + b.w / 2, b.y + b.h / 2];
  return { dx: point[0] - start[0], dy: point[1] - start[1] };
}
