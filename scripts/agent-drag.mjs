// `tap <target> drag …` (LLP 1080.000 §11), the driver's half: validation,
// the viewport, and the route to the touch runner (`host/apple/touches.mjs`)
// or to the carrier's own contact phases. `agent.mjs` calls it from `tap`.
import { DRAG_BOUNDS } from '../host/apple/touches.mjs';

/**
 * `tap <target> drag …` (LLP 1080.000 §11): one whole gesture from `from`
 * (an offset from the target's box, its middle by default): press `press`
 * ms, one straight drag by (dx, dy) over `over` ms, hold `hold` ms, lift;
 * `during` thunks run while the finger is down, when no input is accepted
 * (`s.held`). The start and the end must be in the viewport. A real touch
 * under `--touch platform`; elsewhere the carrier's own contact phases,
 * refused where the carrier refuses them.
 */
export async function dragTap({ s, carrier, node, target, host, timing, tapRefusal, scrolled }, opts) {
  const { dx, dy, from, press = 0, over = 250, hold = 0, during = [] } = opts, drag = { dx, dy, press, over, hold, during }, said = { dx, dy, press, over, hold };
  if (![dx, dy, press, over, hold].every(Number.isFinite) || [press, over, hold].some((v) => v < 0)) throw new Error('drag: expected finite dx, dy and non-negative press, over and hold (ms)');
  if (from !== undefined && !(Array.isArray(from) && from.length === 2 && from.every(Number.isFinite))) throw new Error('drag: from takes two finite numbers, an offset from the target\'s box');
  const moves = dx !== 0 || dy !== 0;
  if (moves && over <= 0) throw new Error('drag: a drag that moves needs over > 0');
  for (const k of ['press', 'over', 'hold']) if (drag[k] > DRAG_BOUNDS[k]) throw new Error(`drag: ${k} ${drag[k]} ms is past its bound, ${DRAG_BOUNDS[k]} ms`);
  if (press + hold + (moves ? over : 0) > DRAG_BOUNDS.total) throw new Error(`drag: the gesture lasts past ${DRAG_BOUNDS.total} ms`);
  if (s.contact) throw new Error('a contact is already down; use `tap up` or `tap cancel` first');
  const layout = await s.layout(), b = layout.nodes.find((n) => n.id === node.id), vp = layout.viewport;
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
    return s.tagged({ ...r, target, ...(scrolled ? { scrolled } : {}), carrier: host, mode: timing });
  }
  // The carrier's phases, each reply checked: an error or a refusal releases the contact and throws.
  let down, done = [], up;
  const phase = async (name, opts) => {
    const r = await s.pointer(name, opts);
    if (r.error || r.delivery === 'unsupported') throw new Error(`drag: ${name}: ${r.error ?? r.reason ?? 'unsupported'}`);
    return r;
  };
  try {
    down = await s.tap(target, { down: true, at: from });
    if (down.error) throw new Error(`drag: down: ${down.error}`);
    if (down.delivery === 'unsupported') {
      const { phase: _, ...refused } = down;
      return s.tagged({ ...refused, drag: said, reason: `${down.reason ?? 'no held contact'}; a real drag is --touch platform's (LLP 1080.000 §11)` });
    }
    // Here the ops come before the press, and each is bounded by the gesture's own bound.
    for (const op of during) {
      let timer;
      const late = new Promise((_, no) => { timer = setTimeout(() => no(new Error(`drag: an op during it outlasted ${DRAG_BOUNDS.total} ms`)), DRAG_BOUNDS.total); });
      const running = held(op)();
      running.catch(() => {});
      try { done.push(await Promise.race([running, late])); } finally { clearTimeout(timer); }
    }
    if (press) await phase('hold', { ms: press });
    if (moves) await phase('move', { dx, dy, ms: over });
    if (hold) await phase('hold', { ms: hold });
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
  return s.tagged({ tapped: node.id, target, ...(scrolled ? { scrolled } : {}), at: down.at, drag: said, lifted: up.at, ...(done.length ? { during: done } : {}), delivery: down.delivery, carrier: host, mode: timing });
}
