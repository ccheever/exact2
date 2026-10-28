// The driver's keys and typed input: a chord as CDP's key event, the browser's and the
// native carriers' key paths, a held key, a held device request's target, and the CLI's
// `type` and `tap` forms. Split from agent.mjs, which re-exports each.
import { randomBytes } from 'node:crypto';
import { resolve } from 'node:path';
import { dragArguments } from './agent-drag.mjs';

export function refusalMessage(reply) {
  const r = reply?.refused;
  return r && ['host','subsystem','message'].every(key => typeof r[key] === 'string')
    ? `host ${r.host} refused (${r.subsystem}): ${r.message}` : null;
}
export function hostReply(reply) {
  const message = refusalMessage(reply);
  if (message) {
    const error = Object.assign(new Error(message), {reply});
    if (!process.env.EXACT_AGENT_DEBUG && !process.argv.includes('--json')) error.stack = message;
    throw error;
  }
  return reply;
}

/** Independent of the app and its state. A host sets `malformed: true` on its JSON reply; `reply()` raises this error from that flag. */
export class MalformedOperation extends Error {
  malformed = true;
  static reply(reply) {
    hostReply(reply);
    if (reply?.malformed === true && reply.error != null) throw Object.assign(new MalformedOperation(reply.error), { reply });
    return reply;
  }
}

export async function operationReply(carrier, req) {
  let reply;
  try { reply = MalformedOperation.reply(await carrier.ask(req)); }
  catch (error) {
    if (error instanceof MalformedOperation) error.message = `${req.op}: ${error.message}`;
    throw error;
  }
  if (reply.error) throw Object.assign(new Error(`${req.op}: ${reply.error}`), { reply });
  return reply;
}

export function pinchArguments(scale) {
  if (!(scale > 0 && Number.isFinite(scale))) throw new MalformedOperation('pinch: expected a positive finite scale');
}

export function intoArguments({ block = 'start', inline = 'nearest' }) {
  for (const alignment of [block, inline]) {
    if (!['start', 'center', 'end', 'nearest'].includes(alignment)) throw new MalformedOperation(`no alignment ${typeof alignment === "string" ? alignment : JSON.stringify(alignment)}: start, center, end or nearest`);
  }
}

export function inputJSON(text) {
  try { return JSON.parse(text); } catch (e) { throw new MalformedOperation(e.message); }
}

export function keyArguments(opts) {
  if (opts.phase != null && !['down', 'up'].includes(opts.phase)) throw new MalformedOperation(`key: not a phase: ${opts.phase}`);
  if (opts.for !== undefined && (opts.key == null || opts.phase != null || !Number.isFinite(opts.for) || opts.for < 0)) throw new MalformedOperation('type for: expected a key and a nonnegative finite duration, without phase');
}

export function pointerArguments(phase, opts) {
  if (!['move', 'hold', 'up', 'cancel'].includes(phase)) throw new MalformedOperation(`pointer: not a phase: ${phase} (move, hold, up, cancel)`);
  if (['x', 'y', 'dx', 'dy'].some(k => opts[k] !== undefined && !Number.isFinite(opts[k]))) throw new MalformedOperation('pointer: expected finite coordinates');
  if (opts.ms !== undefined && (!Number.isFinite(opts.ms) || opts.ms < 0)) throw new MalformedOperation('pointer: expected a nonnegative finite duration');
}

/** `resize <w>x<h>` or `resize <w> <h>`: the words after the op. */
export function resizeArguments(words) {
  const m = /^(\d+)(?:x|\s+)(\d+)$/.exec([words].flat().join(' ').trim());
  if (!m) throw new MalformedOperation('resize <w>x<h> or resize <w> <h>, as resize 800x600');
  return [Number(m[1]), Number(m[2])];
}

const modifierBits = { Alt: 1, Control: 2, Meta: 4, Shift: 8 };
const modifierOf = code => /^(Alt|Control|Meta|Shift)(Left|Right)$/.exec(code)?.[1];

/** CDP's flags describe the state after this edge. A second physical side
 * keeps its modifier held; explicit chord flags last only for their own event.
 * The caller records accepted events, not this derived snapshot, for reset. */
export function withHeldModifiers(method, event, heldKeys) {
  const keyboard = method === 'Input.dispatchKeyEvent';
  if (!keyboard && !['Input.dispatchMouseEvent', 'Input.dispatchTouchEvent'].includes(method)) return event;
  let modifiers = event.modifiers ?? 0;
  const own = keyboard ? modifierBits[modifierOf(event.code)] ?? 0 : 0;
  if (own) modifiers = event.type === 'keyUp' ? modifiers & ~own : modifiers | own;
  for (const code of heldKeys.keys()) {
    if (keyboard && event.type === 'keyUp' && code === event.code) continue;
    modifiers |= modifierBits[modifierOf(code)] ?? 0;
  }
  const result = { ...event, modifiers };
  // Chrome runs ContextMenu's default only for a raw keydown (as Playwright
  // delivers it); keyDown without text names the key but opens no menu.
  if (keyboard && result.code === 'ContextMenu' && result.type === 'keyDown') result.type = 'rawKeyDown';
  if (keyboard && modifiers & 8 && /^[a-z]$/.test(result.key)) {
    result.key = result.key.toUpperCase();
    if (result.text) result.text = result.text.toUpperCase();
  }
  if (keyboard && modifiers & 6) delete result.text;
  return result;
}

/**
 * A driver's key as CDP's key event: a `key` name (`Enter`, `a`, `Space`),
 * a code (`KeyA`, `Digit7`, `ControlRight`), or either after any of `Shift+`, `Control+`,
 * `Alt+`, `Meta+` — Playwright's chord syntax (`Shift+Enter`, `Meta+s`,
 * `+`). `text` is what the key types: nothing with Control or Meta held, a
 * shortcut's (chat F3, kanban F27 in the x2apps diaries).
 */
const PUNCTUATION = { '-': 'Minus', '_': 'Minus', '=': 'Equal', '+': 'Equal', '[': 'BracketLeft', '{': 'BracketLeft', ']': 'BracketRight', '}': 'BracketRight',
  '\\': 'Backslash', '|': 'Backslash', ';': 'Semicolon', ':': 'Semicolon', "'": 'Quote', '"': 'Quote', '`': 'Backquote', '~': 'Backquote',
  ',': 'Comma', '<': 'Comma', '.': 'Period', '>': 'Period', '/': 'Slash', '?': 'Slash',
  '!': 'Digit1', '@': 'Digit2', '#': 'Digit3', '$': 'Digit4', '%': 'Digit5', '^': 'Digit6', '&': 'Digit7', '*': 'Digit8', '(': 'Digit9', ')': 'Digit0' };
export function cdpKey(chord) {
  if (typeof chord !== 'string') throw new MalformedOperation(`key: expected a key name, not ${chord}`);
  const held = new Set();
  let code = chord, key, vk, location = 0;
  for (let m; (m = /^(Shift|Control|Alt|Meta)\+(.+)$/.exec(code)); code = m[2]) held.add(m[1]);
  const modifier = /^(Shift|Control|Alt|Meta)(Left|Right)?$/.exec(code);
  if (modifier) {
    key = modifier[1]; location = modifier[2] === 'Right' ? 2 : 1;
    code = key + (location === 2 ? 'Right' : 'Left'); held.add(key);
    vk = { Shift: 16, Control: 17, Alt: 18, Meta: location === 2 ? 92 : 91 }[key];
  }
  else if (/^Key[A-Z]$/.test(code)) { key = code.slice(3).toLowerCase(); vk = code.charCodeAt(3); }
  else if (/^Digit[0-9]$/.test(code)) { key = code.slice(5); vk = code.charCodeAt(5); }
  else if (/^F(?:[1-9]|1[0-9]|2[0-4])$/.test(code)) { key = code; vk = 111 + Number(code.slice(1)); }
  // A key by its `key` name works as on every other target (pomodoro F5): a
  // letter or digit on its US key, punctuation on its own, a named key.
  else if (/^[a-zA-Z0-9]$/.test(code)) { key = code; vk = code.toUpperCase().charCodeAt(0); code = /\d/.test(code) ? `Digit${code}` : `Key${code.toUpperCase()}`; }
  // Shifted punctuation is the physical key under it, as Linux's driver_key
  // and Apple's KeyCodes.codeName name it (b6 review C2): `!` is Digit1.
  else if (code.length === 1 && code !== ' ') { key = code; vk = 0; code = PUNCTUATION[code] ?? ''; }
  else {
    const special = { ArrowUp: ['ArrowUp', 38], ArrowDown: ['ArrowDown', 40], ArrowLeft: ['ArrowLeft', 37], ArrowRight: ['ArrowRight', 39], Space: [' ', 32], ' ': [' ', 32], Enter: ['Enter', 13], Escape: ['Escape', 27], ContextMenu: ['ContextMenu', 93], Tab: ['Tab', 9], Backspace: ['Backspace', 8], Delete: ['Delete', 46], Home: ['Home', 36], End: ['End', 35], PageUp: ['PageUp', 33], PageDown: ['PageDown', 34] }[code];
    if (!Array.isArray(special)) throw new MalformedOperation(`key: unsupported key ${code}`);
    [key, vk] = special;
    if (code === ' ') code = 'Space';
  }
  if (held.has('Shift') && /^[a-z]$/.test(key)) key = key.toUpperCase();
  const modifiers = (held.has('Alt') ? 1 : 0) | (held.has('Control') ? 2 : 0) | (held.has('Meta') ? 4 : 0) | (held.has('Shift') ? 8 : 0);
  const text = held.has('Control') || held.has('Meta') ? undefined : key === 'Enter' ? '\r' : key.length === 1 ? key : undefined;
  return { code, key, vk, modifiers, text, location };
}

/** Validate before resolving a target, using the selected carrier's key vocabulary. */
export async function validateKey(key, { host, browser }) {
  const chord = String(key);
  // A native host parses the name itself and labels an unknown one in its reply; its vocabulary is its own.
  if (host !== 'web') return;
  if (!browser || browser === 'chrome') cdpKey(chord);
  else {
    const { chordKeys } = await import('./agent-playwright.mjs');
    try { chordKeys(chord); }
    catch (error) { if (error.malformed === true) throw error; } // Known keys unavailable to this carrier are refused later, when the key is sent.
  }
}

/** A chord's modifiers in the order it names them (`Shift+Control+x` is
 * Shift, then Control); a modifier pressed alone is the key, not a prefix. */
export function chordModifiers(chord) {
  const held = [];
  for (let m, rest = String(chord); (m = /^(Shift|Control|Alt|Meta)\+(.+)$/.exec(rest)); rest = m[2]) held.push(m[1]);
  return held;
}
const MODIFIER_KEYS = { Shift: ['ShiftLeft', 16], Control: ['ControlLeft', 17], Alt: ['AltLeft', 18], Meta: ['MetaLeft', 91] };
/** A keyboard's modifier edges around a key or a click (Charlie, 2026-10-07):
 * each modifier's own keydown in order before, its keyup in reverse after, as
 * a person's keyboard sends them and Playwright's `press` does, so a `key`
 * handler hears `Shift` before `Enter` on every engine. CDP's flags follow
 * from the keys held (`withHeldModifiers`). */
const modifierEdge = (call, name, type) => {
  const [code, vk] = MODIFIER_KEYS[name];
  return call('Input.dispatchKeyEvent', { type, key: name, code, windowsVirtualKeyCode: vk, location: 1, modifiers: 0 });
};
/** Each modifier's keyup in reverse, every one attempted; the first failure is thrown after. */
async function releaseModifiers(call, pressed) {
  let failed;
  for (const name of [...pressed].reverse()) await modifierEdge(call, name, 'keyUp').catch(error => { failed ??= error; });
  if (failed) throw failed;
}
/** Each modifier's keydown in order; one that fails lets the earlier ones go and throws. */
async function pressModifiers(call, names) {
  const pressed = [];
  for (const name of names) {
    try { await modifierEdge(call, name, 'keyDown'); } catch (error) { await releaseModifiers(call, pressed).catch(() => {}); throw error; }
    pressed.push(name);
  }
  return pressed;
}
export async function modifierEdges(call, names, type) {
  if (type === 'keyUp') return releaseModifiers(call, names);
  await pressModifiers(call, names);
}

/** `act` with a chord's modifiers held around it, released when it throws. */
export async function withChordModifiers(call, mods, act) {
  const pressed = await pressModifiers(call, mods);
  try { return await act(); } finally { await releaseModifiers(call, pressed); }
}
/** A click, a run of clicks or a wheel with modifiers named holds them as a
 * keyboard does, as Playwright's carrier holds them (`withHeldKeys`). */
export function heldForClick(call, kind, opts, act) {
  const clicks = ['press', 'contextmenu', 'dblclick', 'auxclick', 'clicks', 'mouse', 'wheel'].includes(kind) && opts?.modifiers;
  return clicks ? withChordModifiers(call, String(opts.modifiers).split('+').filter(Boolean), act) : act();
}

/** The chord a real paste sends: ⌘V on macOS, Ctrl+V elsewhere. */
export function pasteChord() {
  return process.platform === 'darwin' ? 'Meta+v' : 'Control+v';
}

/** `type <id> copy|cut|paste`. Paste sends [`pasteChord`] first and lands
 * the clipboard event only when that keydown was not prevented (drums: skipping
 * the chord hid a `key` handler whose preventDefault blocked a real paste).
 * Copy and cut stay the clipboard event. `keyDown`/`keyUp` default to CDP. */
export async function deliverClipboard({ id, opts, evaluate, ask, call, keyDown, keyUp, insertText }) {
  const f = await ask({ op: 'focus', id, select: false });
  hostReply(f); if (f.error) throw new Error(f.error);
  // Firefox builds a synthetic paste's clipboardData from its own `dataType`
  // and `data` and ignores `clipboardData`, so its paste carried no text.
  const send = async () => {
    // A paste's files stand in for the browser's (`exact.agentPaste`, documents-glue.js `pasted`): Firefox hands a synthetic paste no files.
    const heard = await evaluate(`(() => { ${opts.pasted ? `exact.agentPaste = ${JSON.stringify(opts.pasted)};` : ''} const el = document.activeElement?.closest?.('[data-view]') ? document.activeElement : exact.views.get(${id}), dt = new DataTransfer(); if (${JSON.stringify(opts.clipboard)} === 'paste') dt.setData('text/plain', ${JSON.stringify(opts.text ?? '')}); const ev = new ClipboardEvent(${JSON.stringify(opts.clipboard)}, { clipboardData: dt, ...(${JSON.stringify(opts.clipboard)} === 'paste' ? { dataType: 'text/plain', data: ${JSON.stringify(opts.text ?? '')} } : {}), bubbles: true, cancelable: true }); el.dispatchEvent(ev); return { editable: el.matches('input, textarea, [contenteditable]'), prevented: ev.defaultPrevented }; })()`);
    if (opts.clipboard === 'paste' && heard.editable && !heard.prevented) await (insertText ?? (text => call('Input.insertText', { text })))(opts.text ?? '');
  };
  if (opts.clipboard !== 'paste') { await send(); return; }
  const chord = cdpKey(pasteChord()), mods = chordModifiers(pasteChord());
  // A key that fails after its modifier went down lets the modifier go again.
  const down = keyDown ?? (async () => { await modifierEdges(call, mods, 'keyDown'); try { await call('Input.dispatchKeyEvent', { type: 'keyDown', key: chord.key, code: chord.code, windowsVirtualKeyCode: chord.vk, modifiers: chord.modifiers, location: chord.location }); } catch (error) { await modifierEdges(call, mods, 'keyUp').catch(() => {}); throw error; } });
  const up = keyUp ?? (async () => { try { await call('Input.dispatchKeyEvent', { type: 'keyUp', key: chord.key, code: chord.code, windowsVirtualKeyCode: chord.vk, modifiers: chord.modifiers, location: chord.location }); } finally { await modifierEdges(call, mods, 'keyUp'); } });
  // The keydown itself is kept and read once its dispatch is over, so every
  // listener's preventDefault counts: the target's key handler (the contract's
  // stopPropagation marks the event, host/web/glue.js, host/web-js/rt.js) and
  // a button's `aria-keyshortcuts`, whose capture listener stops the event
  // before any later listener would see it (input-glue.js).
  // The chord is also the browser's own paste where it is one (Ctrl+V in Chrome
  // off the Mac, WebKit's paste: command, Firefox): a trusted paste of the real
  // clipboard, which the app would hear beside the driver's (WebKit heard two,
  // Firefox only the empty one). It is stopped before any listener; the driver's
  // paste carries the text.
  await evaluate(`(() => { window.__exactPasteKey = null; addEventListener('keydown', window.__exactPasteHear = (e) => { window.__exactPasteKey = e; }, true); addEventListener('paste', window.__exactPasteMute = (e) => { if (e.isTrusted) { e.preventDefault(); e.stopImmediatePropagation(); } }, true); })()`);
  let held = false;
  try {
    await down();
    held = true;
    if (await evaluate('window.__exactPasteKey?.defaultPrevented !== true')) await send();
  } finally {
    // A throw from the paste event must still release the chord.
    if (held) await up().catch(() => {});
    await evaluate(`(() => { removeEventListener('keydown', window.__exactPasteHear, true); removeEventListener('paste', window.__exactPasteMute, true); window.__exactPasteHear = window.__exactPasteMute = window.__exactPasteKey = null; })()`).catch(() => {});
  }
}

/** Browser-owned key release carries device identity, never a canvas lookup. */
export async function browserKey({id, opts, evaluate, ask, call, frame}) {
  keyArguments(opts);
  const { code, key, vk, modifiers, text, location } = cdpKey(opts.key), mods = chordModifiers(opts.key);
  const isWorld = await evaluate(`exact.gpu?.wantsInput(${id}) ?? false`);
  const f = isWorld ? await ask({ op: 'focus', id, world: true }) : await evaluate(`(() => { const el = exact.views.get(${id}); el?.focus(); return {ok:document.activeElement === el}; })()`);
  if (f.error || !f.ok) throw new Error(f.error ?? `view ${id} could not take focus`);
  const reply = phase => ({ typed: id, key: opts.key, ...(phase != null ? { phase } : {}), delivery: 'platform' });
  // A key's text goes with its down, as a keyboard's does: a held printable key types, and each
  // auto-repeat types again (#140); a Control or Meta chord has none (`cdpKey`).
  // The chord's modifiers go down before the key and up after it (`modifierEdges`); a held
  // key's auto-repeat (`opts.repeat`, #140) finds them down already. A key that fails to go
  // down lets its modifiers go again, held or not.
  // Whether the modifiers are down, so each comes up once: a lone `up` finds them held.
  const pressing = phase => phase === 'down' && !opts.repeat;
  let modsDown = opts.phase === 'up' || !!opts.repeat;
  const letGo = async () => { if (modsDown) { modsDown = false; await modifierEdges(call, mods, 'keyUp'); } };
  const edge = async phase => {
    if (pressing(phase)) { await modifierEdges(call, mods, 'keyDown'); modsDown = true; }
    try { await call('Input.dispatchKeyEvent', { type: phase === 'down' ? 'keyDown' : 'keyUp', code, key, windowsVirtualKeyCode: vk, modifiers, location, ...(phase === 'down' && text ? { text } : {}), ...(phase === 'down' && opts.repeat ? { autoRepeat: true } : {}) }); }
    catch (error) { if (pressing(phase)) await letGo().catch(() => {}); throw error; }
    finally { if (phase === 'up') await letGo(); }
  };
  const release = async () => {
    await edge('up');
    await frame();
    return reply('up');
  };
  try {
    // A held key's later down is the platform's auto-repeat (`KeyboardEvent.repeat`, #140).
    for (const phase of opts.phase == null ? ['down', 'up'] : [opts.phase]) await edge(phase);
    await frame();
  } catch (error) { if (opts.phase === 'down') error.release = release; throw error; }
  return { ...reply(opts.phase), ...(opts.phase === 'down' ? { release } : {}) };
}

/** Native key carrier shared by stdio, phone and simulator. */
export async function nativeKey({id, opts, ask}) {
  keyArguments(opts);
  const releaseKey = opts.phase === 'down' && opts.ownedRelease ? randomBytes(16).toString('hex') : undefined;
  const {ownedRelease, ...input} = opts;
  const release = async () => {
    const r = await ask({op:'type', releaseKey, phase:'up'});
    hostReply(r); if (r.error) throw new Error(r.error);
    return r;
  };
  try {
    const r = await ask({op:'type', id, ...input, ...(releaseKey ? {releaseKey} : {})});
    hostReply(r); if (r.error) throw new Error(r.error);
    return {...r, ...(releaseKey ? {release} : {})};
  } catch (error) { if (releaseKey) error.release = release; throw error; }
}

/** The auto-repeat a held key makes (#140), at macOS's default rate (`NSEvent.keyRepeatDelay` and
 * `keyRepeatInterval` with nothing set): the first repeat 500 ms after the down, then one every 83 ms. A
 * modifier alone does not repeat, as AppKit's flags change does not. */
export const KEY_REPEAT = { delay: 500, interval: 83 };
const repeats = key => !/^(Shift|Control|Alt|Meta)(Left|Right)?$/.test(String(key).replace(/^(?:(?:Shift|Control|Alt|Meta)\+(?=.))+/, ''));

/** Held-key form: one resolved carrier, including release after a failed clock. While held, the key
 * repeats on the virtual clock ([`KEY_REPEAT`]): each repeat a keydown whose `repeat` is true. */
export async function typeFor({node, target, options, carrier, clock, tagged, delivery, host, timing}) {
  keyArguments(options);
  const {for: duration, ...held} = options, key = String(held.key), steps = [];
  let release;
  const send = async (phase, repeat = false) => {
    const args = [target, {...held, phase, ...(repeat ? {repeat} : {})}];
    try {
      const result = phase === 'up' && release ? await release() : await carrier.input(node.id, 'key', {...held, key, phase, ...(repeat ? {repeat} : {ownedRelease:true})});
      const {release: ownedRelease, ...r} = result;
      if (phase === 'down' && !repeat) release = ownedRelease;
      const reply = await tagged({...r, typed:node.id, target, delivery:r.delivery ?? delivery, carrier:host, mode:timing});
      steps.push({op:'type', args, reply});
    } catch (error) { release ??= error.release; steps.push({op:'type', args, error:error.message}); throw error; }
  };
  const advance = async ms => {
    const args = [`+${ms}`];
    try { steps.push({op:'clock', args, reply:await clock(args[0])}); }
    catch (error) { steps.push({op:'clock', args, error:error.message}); throw error; }
  };
  let failure;
  try {
    await send('down');
    let at = 0;
    for (let next = KEY_REPEAT.delay; repeats(key) && next < duration; next += KEY_REPEAT.interval) {
      await advance(next - at);
      at = next;
      await send('down', true);
    }
    await advance(duration - at);
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

/** The CLI `tap` line's words after `tap` (LLP 1012 §1): `[target, opts]` for `s.tap`, or `[null, opts]` for a
 * held contact's phase when `held` says one is down (`move [by] <x> <y> [over <ms>]`, `hold [<ms>]`, `up`, `cancel`).
 * Strict: a word no form uses, a word twice, or a number that is none is refused by name, never dropped (#107:
 * `tap probe auxclick` was sent as a left click). `drag` and a held request's `tap @N` are the caller's. */
export function tapWords(args, held = false) {
  const [first, form, ...rest] = args;
  const refuse = (why) => { throw new MalformedOperation(`tap${args.length ? ' ' + args.join(' ') : ''}: ${why}`); };
  if (first == null) refuse('needs a target');
  const number = (word, what) => {
    const n = Number(word);
    if (word === undefined || word === '' || !Number.isFinite(n)) refuse(`${what} takes a number, not ${word === undefined ? 'nothing' : JSON.stringify(word)}`);
    return n;
  };
  // The options after a form, in any order: `at <x> <y>`, `modifiers <Shift+Meta>`, `over <ms>`, or a flag.
  const options = (words, allowed) => {
    const opts = {}, seen = new Set();
    for (let i = 0; i < words.length;) {
      const w = words[i];
      if (!allowed.includes(w)) refuse(`unknown word ${JSON.stringify(w)}; ${allowed.length ? `this form takes ${allowed.map((a) => ({ at: 'at <x> <y>', modifiers: 'modifiers <Shift+Control+Alt+Meta>', over: 'over <ms>' }[a] ?? a)).join(', ')}` : 'this form takes nothing more'}`);
      if (seen.has(w)) refuse(`${w} twice`);
      seen.add(w);
      if (w === 'at') { opts.at = [number(words[i + 1], 'at'), number(words[i + 2], 'at')]; i += 3; }
      else if (w === 'over') { opts.ms = number(words[i + 1], 'over'); i += 2; }
      else if (w === 'modifiers') {
        if (!words[i + 1] || allowed.includes(words[i + 1])) refuse('modifiers takes Shift, Control, Alt or Meta, joined by +');
        opts.modifiers = words[i + 1]; i += 2;
      } else { opts[w] = true; i += 1; }
    }
    return opts;
  };
  // A MIDI input's message (LLP 1125 §4): `tap midi "<input>" 90 3c 64`, each byte two hex digits; or its clock (D9):
  // `tap midi "<input>" clock <bpm> <ms>`. The input may be quoted, as a test step quotes it.
  const input = /^".*"$/.test(form ?? '') ? inputJSON(form) : form;
  if (first === 'midi' && form !== undefined && rest.length && rest.every((w) => /^[0-9a-f]{2}$/i.test(w))) return [null, { midi: { input, data: rest.map((w) => parseInt(w, 16)) } }];
  if (first === 'midi' && form !== undefined && rest[0] === 'clock') {
    const [bpm, ms] = [number(rest[1], 'clock'), number(rest[2], 'clock')];
    if (rest.length !== 3 || !(bpm > 0 && ms >= 0)) refuse('tap midi "<input>" clock <bpm> <ms> takes a tempo above 0 and a length');
    return [null, { midi: { input, clock: { bpm, ms } } }];
  }
  if (held && first === 'move') {
    const by = form === 'by', [a, b, ...more] = by ? rest : [form, ...rest];
    const point = by ? { dx: number(a, 'move by'), dy: number(b, 'move by') } : { x: number(a, 'move'), y: number(b, 'move') };
    return [null, { phase: 'move', ...point, ms: 0, ...options(more, ['over', 'modifiers']) }];
  }
  if (held && first === 'hold') {
    if (rest.length) refuse('hold takes one duration');
    return [null, { phase: 'hold', ms: form === undefined ? 0 : number(form, 'hold') }];
  }
  if (held && first === 'up') return [null, { phase: 'up', ...options(args.slice(1), ['modifiers']) }];
  if (held && first === 'cancel') return [null, { phase: 'cancel', ...options(args.slice(1), []) }];
  if (form === undefined) return [first, {}];
  // A held contact's phases name no target: the contact is the one `tap <target> down` began.
  if (['up', 'cancel', 'hold', 'move'].includes(form)) refuse(held ? `a held contact's phases take no target: \`tap ${[form, ...rest].join(' ')}\`` : `${form} is a phase of a held contact, which none is: \`tap <target> down\`, then \`tap ${form}${form === 'move' ? ' <x> <y>' : form === 'hold' ? ' <ms>' : ''}\``);
  if (form.startsWith('{')) { try { return [first, JSON.parse(args.slice(1).join(' '))]; } catch (e) { refuse(e.message); } }
  switch (form) {
    // A click with modifiers held (gallery F20): `tap <target> modifiers Shift+Meta`.
    case 'modifiers': return [first, options(args.slice(1), ['modifiers'])];
    // A press at a point in the target, from its top left: whatever a finger there reaches (LLP 1012 §1), never aimed.
    case 'at': return [first, options(args.slice(1), ['at', 'modifiers'])];
    case 'down': return [first, { down: true, ...options(rest, ['at', 'modifiers']) }];
    case 'hover': return [first, { hover: true, ...options(rest, []) }];
    // A held state (LLP 1122 D8): `tap <target> hold-state active|hover|focus-visible|none`, as the host's own.
    case 'hold-state':
      if (rest.length !== 1 || !['active', 'hover', 'focus-visible', 'none'].includes(rest[0])) refuse('hold-state takes active, hover, focus-visible or none');
      return [first, { holdState: rest[0] === 'none' ? null : rest[0] }];
    case 'history': if (rest.length !== 1) refuse('history takes one step, as history -1'); return [first, { history: number(rest[0], 'history') }];
    case 'mediasession':
      if (!rest[0] || rest.length > 2) refuse('mediasession <action> [seconds]');
      return [first, { mediaSession: rest[0], ...(rest[1] !== undefined ? { seconds: number(rest[1], 'mediasession') } : {}) }];
    case 'pinch': return [first, { pinch: number(rest[0], 'pinch'), ...options(rest.slice(1), ['at']) }];
    // Files dragged in from outside, each a path (studio diary R19).
    case 'drop': if (!rest.length) refuse('drop takes one or more paths'); return [first, { drop: rest.map((path) => resolve(path)) }];
    // A drag of those files from outside comes over the target and stays there (`dragenter`), until a `drop` there
    // or a `dragleave`, which takes it away undropped.
    case 'dragenter': if (!rest.length) refuse('dragenter takes the dragged paths'); return [first, { dragenter: rest.map((path) => resolve(path)) }];
    case 'dragleave': if (rest.length) refuse('dragleave takes no more words'); return [first, { dragleave: true }];
    case 'into': {
      // tap <list> into <key> [block <v>] [inline <v>]
      if (rest[0] === undefined) refuse('into takes a key');
      const into = { key: String(rest[0]) };
      for (let i = 1; i < rest.length; i += 2) {
        if (!['block', 'inline'].includes(rest[i]) || rest[i + 1] === undefined || rest[i] in into) refuse(`into: unknown option ${rest[i]}; block <start|center|end|nearest> and inline <…>, each once`);
        into[rest[i]] = rest[i + 1];
      }
      try { intoArguments(into); } catch (e) { refuse(e.message); }
      return [first, { into }];
    }
    // A wheel at the target's middle or `at` in it, the modifiers held (a pinch is a Control-held wheel; studio diary R3).
    // `fling`, `hold` and `release` are a trackpad's, through the window server (macOS, LLP 1142 D3).
    case 'wheel': {
      const opts = { wheel: [number(rest[0], 'wheel'), number(rest[1], 'wheel')], ...options(rest.slice(2), ['gesture', 'fling', 'hold', 'release', 'at', 'modifiers']) };
      const forms = ['gesture', 'fling', 'hold', 'release'].filter((f) => opts[f]);
      if (forms.length > 1) refuse(`wheel takes one of gesture, fling, hold and release, not ${forms.join(' and ')}`);
      if ((opts.fling || opts.hold || opts.release) && opts.modifiers) refuse(`wheel ${forms[0]} holds no modifiers`);
      if (opts.release && (opts.wheel[0] !== 0 || opts.wheel[1] !== 0 || opts.at)) refuse('wheel release is `wheel 0 0 release`: it lifts the held gesture where it is');
      return [first, opts];
    }
    // The mouse's buttons (#107): the left, a double click, the right (`contextmenu`), the middle (`auxclick`), n clicks.
    case 'mouse': case 'dblclick': case 'contextmenu': case 'auxclick': return [first, { [form]: true, ...options(rest, ['at', 'modifiers']) }];
    case 'clicks': return [first, { clicks: number(rest[0], 'clicks'), ...options(rest.slice(1), ['at', 'modifiers']) }];
    // A pad's stick on a world canvas (LLP 1127.003 D3): `tap world stick left 0.7 -0.2 [for <ms>]`.
    case 'stick':
      if (!['left', 'right'].includes(rest[0]) || !(rest.length === 3 || (rest.length === 5 && rest[3] === 'for'))) refuse('stick takes left or right, x and y in [-1, 1], and optionally for <ms>');
      return [first, { stick: rest[0], value: [number(rest[1], 'stick'), number(rest[2], 'stick')], ...(rest[3] === 'for' ? { for: number(rest[4], 'for') } : {}) }];
    // Most often a target of several words left unquoted (sb4 farm wave 4, 053: `tap Why are my…`).
    default: refuse(`unknown word ${JSON.stringify(form)}; a target of several words is quoted inside the op (\`tap "${args.join(' ').replace(/["\\]/g, '\\$&')}"\`), and after a target come only: tap <target> [at <x> <y> [modifiers <M>] | modifiers <M> | down [at <x> <y>] [modifiers <M>] | wheel <dx> <dy> [gesture | fling | hold] [at <x> <y>] [modifiers <M>] | wheel 0 0 release | mouse | dblclick | contextmenu | auxclick | clicks <1-3> (each [at <x> <y>] [modifiers <M>]) | hover | drop <path…> | dragenter <path…> | dragleave | into <key> … | pinch <scale> [at <x> <y>] | history <n> | mediasession <action> [seconds] | drag …]`);
  }
}

/** `tap midi "<input>" clock <bpm> <ms>` (LLP 1125 D9) as an arm delivers it: Timing Clock at `k · 60000 / (24 · bpm)` ms
 * from now, k ≥ 1, through `ms`, in slices at most 50 ms long. Each slice is `{ to, ticks }`, its end and its ticks as ms
 * from the start; the driver moves the clock to its end, then delivers its ticks, one runner call. A tempo whose tick is a
 * whole number of ms (125, 100, 250) keeps every stamp exact. */
export function clockSlices({ bpm, ms }) {
  const interval = 60000 / (24 * bpm), out = [];
  for (let k = 1, from = 0; from < ms;) {
    const to = Math.min(from + 50, ms), ticks = [];
    for (; k * interval <= to; k++) ticks.push(k * interval);
    out.push({ to, ticks });
    from = to;
  }
  return out;
}

/** A `tap`'s options checked before any host is asked: malformed ones are refused by name. Returns the mouse's button forms given. */
export function tapArguments(opts) {
  if (opts.wheel !== undefined && ![opts.wheel?.[0], opts.wheel?.[1]].every(Number.isFinite)) throw new MalformedOperation('wheel: expected two finite deltas');
  if (opts.drag) dragArguments(opts.drag);
  if (opts.mouse && ['contextmenu','dblclick','auxclick','clicks','down','drag','wheel','hover','pinch','history','into','gesture'].some(key => opts[key] !== undefined)) throw new MalformedOperation('mouse cannot be combined with another input mode');
  if (opts.pinch !== undefined) pinchArguments(opts.pinch);
  if (opts.into) intoArguments(opts.into);
  if (opts.seconds != null && !Number.isFinite(opts.seconds)) throw new MalformedOperation('mediasession: expected finite seconds');
  if (opts.mediaSession === 'seekto' && !(Number.isFinite(opts.seconds) && opts.seconds >= 0)) throw new MalformedOperation('mediasession: seekto needs finite nonnegative seconds');
  // The mouse's forms (#107): one button form at a time, each at its middle or `at`, the modifiers held through it.
  const button = ['mouse', 'contextmenu', 'auxclick', 'dblclick', 'clicks'].filter((k) => opts[k] !== undefined && opts[k] !== false);
  if (button.length > 1) throw new MalformedOperation(`tap: one of mouse, dblclick, contextmenu, auxclick and clicks at a time, not ${button.join(' and ')}`);
  if (opts.clicks !== undefined && ![1, 2, 3].includes(opts.clicks)) throw new MalformedOperation('tap … clicks: 1, 2 or 3');
  if (opts.at !== undefined && (opts.hover || opts.history !== undefined || opts.drop || opts.dragenter || opts.dragleave || opts.into)) throw new MalformedOperation('at is a point on a press, a click (mouse, dblclick, contextmenu, auxclick, clicks), a wheel, a contact, a pinch or a drag');
  if (opts.at !== undefined && (!Array.isArray(opts.at) || opts.at.length !== 2 || !opts.at.every(Number.isFinite))) throw new MalformedOperation('tap at needs two finite numbers');
  const modifiers = opts.modifiers ?? opts.drag?.modifiers;
  if (modifiers !== undefined && !String(modifiers).split('+').filter(Boolean).every(k => ['Shift', 'Control', 'Alt', 'Meta'].includes(k))) throw new MalformedOperation(`tap … modifiers: ${modifiers} is not Shift, Control, Alt and Meta joined by +`);
  if (modifiers !== undefined && (opts.hover || opts.history !== undefined || opts.into || opts.drop || opts.pinch !== undefined)) throw new MalformedOperation('modifiers are held through a press, a click, a wheel, a contact or a drag');
  return button;
}

/** What of a `tap` this carrier cannot deliver as a hand's (#107), or null. AppKit and Chrome carry every form;
 * elsewhere a middle click, a run of clicks, a wheel or a double click at a point, and modifiers held through a
 * button other than a plain press's or through a contact answer `unsupported`, never another input. */
export function pointerGap(host, browser, opts) {
  if (['macos', 'mac', 'host'].includes(host) || (host === 'web' && (browser ?? 'chrome') === 'chrome')) return null;
  const held = opts.modifiers ?? opts.drag?.modifiers;
  const gaps = [
    opts.auxclick && 'a middle-button click (auxclick)',
    opts.clicks !== undefined && host !== 'web' && `clicks ${opts.clicks}`, // the Playwright carrier clicks at a point (agent-playwright.mjs)
    opts.wheel && opts.at !== undefined && 'a wheel at a point',
    opts.dblclick && opts.at !== undefined && !['ios', 'host-ios'].includes(host) && 'a double click at a point', // iOS: the double tap that selects a word there
    host === 'web' && opts.mouse && 'a mouse click', // the Playwright carrier has none
    held && (opts.contextmenu || opts.dblclick || opts.mouse || opts.down || opts.drag || opts.phase) && 'modifiers held through a context menu, a double click, a mouse click, a contact or a drag',
  ].filter(Boolean);
  return gaps.length ? `${host === 'web' ? browser : host} does not carry ${gaps.join('; ')} (LLP 1012 §1)` : null;
}

/** The CLI `type` line. Key, copy and cut stay whitespace-split.
 * Typed text and a paste's text keep their tail: real newlines stay, and `\\n` `\\t` `\\r`
 * `\\\\` decode, as does a JSON string around the tail; single quotes around it are taken off. `hello for 100`
 * stays text (notes: a newline was flattened to a space). */
export function typeCommand(line) {
  const body = String(line).trim().replace(/^type(?:\s+|$)/, '');
  const matched = /^(?:"((?:[^"\\]|\\.)*)"|(\S+))(?:\s+([\s\S]*))?$/.exec(body);
  if (!matched) throw new MalformedOperation('type needs a target');
  const target = matched[1] != null ? inputJSON(`"${matched[1]}"`) : matched[2];
  const rest = matched[3] ?? '';
  const head = /^\S+/.exec(rest)?.[0];
  // A paste of files (`clipboardData.files`): paths, or one image's data with no file.
  const files = /^paste\s+--(file|image)\s+([\s\S]*\S)\s*$/.exec(rest);
  if (files) return typeArguments([target, 'paste', `--${files[1]}`, ...files[2].split(/\s+/)]);
  // A paste's text is a tail as typed text is: its newlines are the clipboard's.
  const pasted = /^paste\s+([\s\S]*\S[\s\S]*)$/.exec(rest);
  if (pasted) return [target, { clipboard: 'paste', text: decodeTypeText(pasted[1]) }];
  // A world surface's message (LLP 1139.004 D1), what `post` is short for: the rest of the line.
  const message = /^message\s+([\s\S]*\S[\s\S]*)$/.exec(rest);
  if (message) return [target, { message: decodeTypeText(message[1]) }];
  if (head === 'key' || head === 'copy' || head === 'cut' || head === 'paste') return typeArguments([target, ...rest.trim().split(/\s+/)]);
  return [target, decodeTypeText(rest)];
}
function decodeTypeText(text) {
  if (text.length >= 2 && text.startsWith('"') && text.endsWith('"')) {
    try { return JSON.parse(text); } catch { /* a quote in the text, not a JSON string */ }
  }
  // Single quotes around the whole text are a shell's, and are taken off as a shell would (app farm wave 2:
  // `"type room-draft 'Test Den'"` typed the quotes). Text that means them is written as JSON: `"'Den'"`.
  if (text.length >= 2 && text.startsWith("'") && text.endsWith("'")) return text.slice(1, -1);
  return text.replace(/\\([ntr\\])/g, (_, c) => ({ n: '\n', t: '\t', r: '\r', '\\': '\\' }[c]));
}

export function typeArguments(args) {
  // The clipboard's events at the target (spreadsheet F6): `copy`, `cut`, `paste <text…>`.
  if (['copy', 'cut'].includes(args[1]) && args.length === 2) return [args[0], {clipboard:args[1]}];
  if (args[1] === 'paste' && args[2] === '--file') {
    if (args.length < 4) throw new MalformedOperation('paste --file takes one or more paths');
    return [args[0], {clipboard:'paste', text:'', files:args.slice(3).map((path) => resolve(path))}];
  }
  if (args[1] === 'paste' && args[2] === '--image') {
    if (args.length !== 4) throw new MalformedOperation('paste --image takes one image file, pasted as its data');
    return [args[0], {clipboard:'paste', text:'', image:resolve(args[3])}];
  }
  if (args[1] === 'paste' && args.length > 2) return [args[0], {clipboard:'paste', text:args.slice(2).join(' ')}];
  if (args[1] !== 'key' || !args[2]) return [args[0], args.slice(1).join(' ')];
  if (args[3] === 'for') {
    if (args.length !== 5) throw new MalformedOperation('type key for: expected one duration');
    return [args[0], {key:args[2], for:Number(args[4])}];
  }
  return [args[0], {key:args[2], ...(args[3] != null ? {phase:args[3]} : {})}];
}

/** A pad on a world canvas (LLP 1127.003 D3): `type <canvas> key Gamepad…` presses a pad button, `tap <canvas> stick
 * left|right <x> <y>` moves a stick in the Gamepad API's axes (positive y is down). Each is the agent's pad 0, reported to
 * the world as a host reports a real one (`delivery: synthesized`) and held across requests; `for <ms>` holds it that long
 * on the clock, then lets go. */
export async function padOp({ op, s, target, opts, host, timing }) {
  if (op === 'tap' && (!['left', 'right'].includes(opts.stick) || !opts.value?.every?.((v) => Number.isFinite(v) && Math.abs(v) <= 1))) throw new MalformedOperation('tap <canvas> stick left|right <x> <y>: x and y in [-1, 1]');
  if (opts.for !== undefined && !(Number.isFinite(opts.for) && opts.for >= 0 && opts.phase == null)) throw new MalformedOperation(`${op} for: expected a nonnegative finite duration, without phase`);
  const node = await s.target(target), word = op === 'type' ? 'typed' : 'tapped';
  const form = op === 'type' ? { key: String(opts.key), ...(opts.phase ? { phase: opts.phase } : {}) } : { stick: opts.stick, value: opts.value };
  const send = (extra = {}) => s.op({ op, id: node.id, world: true, ...form, ...extra });
  const tag = (r) => ({ ...r, [word]: node.id, target, carrier: host, mode: timing });
  if (opts.for === undefined) return s.landed(tag(await send()));
  const steps = [await send(op === 'type' ? { phase: 'down' } : {})];
  try { steps.push(await s.clock(`+${opts.for}`)); }
  finally { steps.push(await send(op === 'type' ? { phase: 'up' } : { value: [0, 0] })); }
  return s.landed(tag({ ...steps[0], for: opts.for, steps }));
}

/** The CLI `post [<surface>] <text>` (LLP 1139.004 D1): `type <surface> message <text>` with the surface
 * optional, so the text is one word or one quoted string ("…" as JSON, '…' as a shell's). One argument is
 * the text for the page's one world; two are the surface and the text. */
export function postCommand(line) {
  const body = String(line).trim().replace(/^post(?:\s+|$)/, ''), words = [];
  for (const m of body.matchAll(/"((?:[^"\\]|\\.)*)"|'([^']*)'|(\S+)/g)) words.push(m[1] != null ? inputJSON(`"${m[1]}"`) : m[2] ?? m[3]);
  if (words.length === 1) return [null, { message: words[0] }];
  if (words.length === 2) return [words[0], { message: words[1] }];
  throw new MalformedOperation(words.length ? `post [<surface>] <text>: quote text of several words, as post world "spawn gulper"; type <surface> message <text…> takes the rest of the line` : 'post [<surface>] <text>: the text a world reads from Input::messages, as post spawn-gulper');
}

/** Post `message` into a world surface, as the page's `postMessage(text, surface)` does (LLP 1139.004 D1):
 * `target` is a canvas the tree finds (a testId, a label) or a surface's name; none, the page's one world.
 * The host queues it as `InputEvent::Message`, the next tick reads it, and a surface with no live canvas
 * holds it, as it holds the page's posts. */
export async function postInto(s, target, message, { host, timing }) {
  if (typeof message !== 'string' || !message) throw new MalformedOperation('post: expected the text to post');
  if (s.held) throw new Error(`a drag's finger is down (${s.held}): only reads and the clock until it lifts`);
  const node = target == null ? null : await s.find(target, false);
  const where = target == null ? {} : node?.world ? { id: node.id } : { surface: String(target) };
  const r = await s.op({ op: 'type', message, ...where });
  return s.landed({ ...r, ...(target == null ? {} : { target }), delivery: r.delivery ?? 'posted', carrier: host, mode: timing });
}
