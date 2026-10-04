// The driver's keys and typed input: a chord as CDP's key event, the browser's and the
// native carriers' key paths, a held key, a held device request's target, and the CLI's
// `type` forms. Split from agent.mjs, which re-exports each.
import { randomBytes } from 'node:crypto';

/**
 * A driver's key as CDP's key event: a `key` name (`Enter`, `a`, `Space`),
 * a code (`KeyA`, `Digit7`), or either after any of `Shift+`, `Control+`,
 * `Alt+`, `Meta+` — Playwright's chord syntax (`Shift+Enter`, `Meta+s`,
 * `+`). `text` is what the key types: nothing with Control or Meta held, a
 * shortcut's (chat F3, kanban F27 in the x2apps diaries).
 */
export function cdpKey(chord) {
  const held = new Set();
  let code = chord, key, vk;
  for (let m; (m = /^(Shift|Control|Alt|Meta)\+(.+)$/.exec(code)); code = m[2]) held.add(m[1]);
  if (/^Key[A-Z]$/.test(code)) { key = code.slice(3).toLowerCase(); vk = code.charCodeAt(3); }
  else if (/^Digit[0-9]$/.test(code)) { key = code.slice(5); vk = code.charCodeAt(5); }
  else if (/^F(?:[1-9]|1[0-9]|2[0-4])$/.test(code)) { key = code; vk = 111 + Number(code.slice(1)); }
  // A key by its `key` name works as on every other target (pomodoro F5): a
  // letter or digit on its US key, punctuation on its own, a named key.
  else if (/^[a-zA-Z0-9]$/.test(code)) { key = code; vk = code.toUpperCase().charCodeAt(0); code = /\d/.test(code) ? `Digit${code}` : `Key${code.toUpperCase()}`; }
  else if (code.length === 1 && code !== ' ') { key = code; vk = 0; code = { '-': 'Minus', '=': 'Equal', '[': 'BracketLeft', ']': 'BracketRight', '\\': 'Backslash', ';': 'Semicolon', "'": 'Quote', '`': 'Backquote', ',': 'Comma', '.': 'Period', '/': 'Slash' }[code] ?? ''; }
  else {
    const special = { ArrowUp: ['ArrowUp', 38], ArrowDown: ['ArrowDown', 40], ArrowLeft: ['ArrowLeft', 37], ArrowRight: ['ArrowRight', 39], Space: [' ', 32], ' ': [' ', 32], Enter: ['Enter', 13], Escape: ['Escape', 27], Tab: ['Tab', 9], Backspace: ['Backspace', 8], Delete: ['Delete', 46], Home: ['Home', 36], End: ['End', 35], PageUp: ['PageUp', 33], PageDown: ['PageDown', 34], Shift: ['Shift', 16], ShiftLeft: ['Shift', 16], ShiftRight: ['Shift', 16], Control: ['Control', 17], Alt: ['Alt', 18], Meta: ['Meta', 91], ...Object.fromEntries(Array.from({ length: 12 }, (_, i) => [`F${i + 1}`, [`F${i + 1}`, 112 + i]])) }[code];
    if (!special) throw new Error(`key: unsupported key ${code}`);
    [key, vk] = special;
    if (['Shift', 'Control', 'Alt', 'Meta'].includes(code)) { held.add(code); code += 'Left'; }
    if (code === ' ') code = 'Space';
  }
  if (held.has('Shift') && /^[a-z]$/.test(key)) key = key.toUpperCase();
  const modifiers = (held.has('Alt') ? 1 : 0) | (held.has('Control') ? 2 : 0) | (held.has('Meta') ? 4 : 0) | (held.has('Shift') ? 8 : 0);
  const text = held.has('Control') || held.has('Meta') ? undefined : key === 'Enter' ? '\r' : key.length === 1 ? key : undefined;
  return { code, key, vk, modifiers, text };
}

/** Browser-owned key release carries device identity, never a canvas lookup. */
export async function browserKey({id, opts, evaluate, ask, call, frame}) {
  if (opts.phase != null && !['down', 'up'].includes(opts.phase)) throw new Error(`key: not a phase: ${opts.phase}`);
  const isWorld = await evaluate(`exact.gpu?.wantsInput(${id}) ?? false`);
  const f = isWorld ? await ask({ op: 'focus', id, world: true }) : await evaluate(`(() => { const el = exact.views.get(${id}); el?.focus(); return {ok:document.activeElement === el}; })()`);
  if (f.error || !f.ok) throw new Error(f.error ?? `view ${id} could not take focus`);
  const { code, key, vk, modifiers, text } = cdpKey(opts.key);
  const reply = phase => ({ typed: id, key: opts.key, ...(phase != null ? { phase } : {}), delivery: 'platform' });
  const release = async () => {
    await call('Input.dispatchKeyEvent', { type: 'keyUp', code, key, windowsVirtualKeyCode: vk, modifiers });
    await frame();
    return reply('up');
  };
  try {
    for (const phase of opts.phase == null ? ['down', 'up'] : [opts.phase]) await call('Input.dispatchKeyEvent', { type: phase === 'down' ? 'keyDown' : 'keyUp', code, key, windowsVirtualKeyCode: vk, modifiers, ...(phase === 'down' && text === '\r' ? { text } : {}) });
    await frame();
  } catch (error) { if (opts.phase === 'down') error.release = release; throw error; }
  return { ...reply(opts.phase), ...(opts.phase === 'down' ? { release } : {}) };
}

/** Native key carrier shared by stdio, phone and simulator. */
export async function nativeKey({id, opts, ask}) {
  const releaseKey = opts.phase === 'down' && opts.ownedRelease ? randomBytes(16).toString('hex') : undefined;
  const {ownedRelease, ...input} = opts;
  const release = async () => {
    const r = await ask({op:'type', releaseKey, phase:'up'});
    if (r.error) throw new Error(r.error);
    return r;
  };
  try {
    const r = await ask({op:'type', id, ...input, ...(releaseKey ? {releaseKey} : {})});
    if (r.error) throw new Error(r.error);
    return {...r, ...(releaseKey ? {release} : {})};
  } catch (error) { if (releaseKey) error.release = release; throw error; }
}

/** Held-key form: one resolved carrier, including release after a failed clock. */
export async function typeFor({node, target, options, carrier, clock, tagged, delivery, host, timing}) {
  const {for: duration, ...held} = options, key = String(held.key), steps = [];
  let release;
  const send = async phase => {
    const args = [target, {...held, phase}];
    try {
      const result = phase === 'up' && release ? await release() : await carrier.input(node.id, 'key', {...held, key, phase, ownedRelease:true});
      const {release: ownedRelease, ...r} = result;
      if (phase === 'down') release = ownedRelease;
      const reply = await tagged({...r, typed:node.id, target, delivery:r.delivery ?? delivery, carrier:host, mode:timing});
      steps.push({op:'type', args, reply});
    } catch (error) { release ??= error.release; steps.push({op:'type', args, error:error.message}); throw error; }
  };
  let failure;
  try {
    await send('down');
    const args = [`+${duration}`];
    try { steps.push({op:'clock', args, reply:await clock(args[0])}); }
    catch (error) { steps.push({op:'clock', args, error:error.message}); throw error; }
  } catch (error) { failure = error; }
  finally { try { await send('up'); } catch (error) { failure ??= error; } }
  if (failure) { failure.steps = steps; throw failure; }
  return tagged({typed:node.id, target, key, for:duration, delivery:steps[0].reply.delivery, steps});
}
/** Parse the CLI type form without treating an ordinary text suffix as a key. */
/** A held device request's target, `@N` (LLP 1069.007 D4): its ticket, or null. */
export const ticketOf = (target) => /^@[1-9]\d*$/.test(String(target)) ? Number(String(target).slice(1)) : null;
/** Whether a target names a held device request: `@N`, or `@<id>` (files F11). */
export const holdOf = (target) => /^@\S+$/.test(String(target));
/** The ticket of the one hold `@<name>` names in `pending` (`state`'s): by the node its answer arrives at (a picker's `id`,
 * the hold's `name`) or by its capability (`open-directory`, `pick`, `export`, `share`, …). Refused by name when none or several do. */
export function heldTicket(pending, target) {
  const name = String(target).slice(1), holds = pending.filter((p) => p.device);
  const named = holds.filter((p) => p.name === name || p.device.args?.id === name);
  const found = named.length ? named : holds.filter((p) => p.device.capability === name);
  const listed = holds.map((p) => `@${p.ticket} ${p.device.capability} at "${p.name}"`).join(', ') || 'none';
  if (found.length !== 1) throw new Error(`${target}: ${found.length ? `${found.length} holds match; name one by its ticket` : 'no held device request answers at that node or has that capability'} (held: ${listed})`);
  return found[0].ticket;
}

/** The paths a `pick` answer names: one per line when it has a line break (an authored test's `pick`, whose quoted
 * path may hold a space), else the CLI's whitespace-separated `type @id a.png b.png`. */
export const pickedPaths = (value) => { const text = String(value); return text.split(text.includes('\n') ? '\n' : /\s+/).map((p) => p.trim()).filter(Boolean); };

/** A native carrier's held contact: one that went down with `mouse` (a drag's, review A1) says `mouse` on each phase
 * until it lifts, is cancelled, or its down fails, an error reply or a thrown request alike. `send(button)` asks. */
export function mouseContact() {
  let held = false;
  return {
    get held() { return held; },
    async ask(kind, opts, send) {
      if (kind === 'down') held = !!opts.mouse;
      let r;
      try { r = await send(held ? { mouse: true } : {}); }
      catch (error) { if (kind === 'down') held = false; throw error; }
      if (kind === 'up' || kind === 'cancel' || (kind === 'down' && r?.error)) held = false;
      return r;
    },
  };
}

export function typeArguments(args) {
  // The clipboard's events at the target (spreadsheet F6): `copy`, `cut`, `paste <text…>`.
  if (['copy', 'cut'].includes(args[1]) && args.length === 2) return [args[0], {clipboard:args[1]}];
  if (args[1] === 'paste' && args.length > 2) return [args[0], {clipboard:'paste', text:args.slice(2).join(' ')}];
  if (args[1] !== 'key' || !args[2]) return [args[0], args.slice(1).join(' ')];
  if (args[3] === 'for') {
    if (args.length !== 5) throw new Error('type key for: expected one duration');
    return [args[0], {key:args[2], for:Number(args[4])}];
  }
  return [args[0], {key:args[2], ...(args[3] != null ? {phase:args[3]} : {})}];
}
