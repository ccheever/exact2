// The driver's keys and typed input: a chord as CDP's key event, the browser's and the
// native carriers' key paths, a held key, a held device request's target, and the CLI's
// `type` and `tap` forms. Split from agent.mjs, which re-exports each.
import { randomBytes } from 'node:crypto';
import { resolve } from 'node:path';

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
    if (!special) throw new Error(`key: unsupported key ${code}`);
    [key, vk] = special;
    if (code === ' ') code = 'Space';
  }
  if (held.has('Shift') && /^[a-z]$/.test(key)) key = key.toUpperCase();
  const modifiers = (held.has('Alt') ? 1 : 0) | (held.has('Control') ? 2 : 0) | (held.has('Meta') ? 4 : 0) | (held.has('Shift') ? 8 : 0);
  const text = held.has('Control') || held.has('Meta') ? undefined : key === 'Enter' ? '\r' : key.length === 1 ? key : undefined;
  return { code, key, vk, modifiers, text, location };
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
  if (f.error) throw new Error(f.error);
  // Firefox builds a synthetic paste's clipboardData from its own `dataType`
  // and `data` and ignores `clipboardData`, so its paste carried no text.
  const send = async () => {
    const heard = await evaluate(`(() => { const el = document.activeElement?.closest?.('[data-view]') ? document.activeElement : exact.views.get(${id}), dt = new DataTransfer(); if (${JSON.stringify(opts.clipboard)} === 'paste') dt.setData('text/plain', ${JSON.stringify(opts.text ?? '')}); const ev = new ClipboardEvent(${JSON.stringify(opts.clipboard)}, { clipboardData: dt, ...(${JSON.stringify(opts.clipboard)} === 'paste' ? { dataType: 'text/plain', data: ${JSON.stringify(opts.text ?? '')} } : {}), bubbles: true, cancelable: true }); el.dispatchEvent(ev); return { editable: el.matches('input, textarea, [contenteditable]'), prevented: ev.defaultPrevented }; })()`);
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
  if (opts.phase != null && !['down', 'up'].includes(opts.phase)) throw new Error(`key: not a phase: ${opts.phase}`);
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

/** The auto-repeat a held key makes (#140), at macOS's default rate (`NSEvent.keyRepeatDelay` and
 * `keyRepeatInterval` with nothing set): the first repeat 500 ms after the down, then one every 83 ms. A
 * modifier alone does not repeat, as AppKit's flags change does not. */
export const KEY_REPEAT = { delay: 500, interval: 83 };
const repeats = key => !/^(Shift|Control|Alt|Meta)(Left|Right)?$/.test(String(key).replace(/^(?:(?:Shift|Control|Alt|Meta)\+(?=.))+/, ''));

/** Held-key form: one resolved carrier, including release after a failed clock. While held, the key
 * repeats on the virtual clock ([`KEY_REPEAT`]): each repeat a keydown whose `repeat` is true. */
export async function typeFor({node, target, options, carrier, clock, tagged, delivery, host, timing}) {
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
  const refuse = (why) => { throw new Error(`tap ${args.join(' ')}: ${why}`); };
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
  if (form.startsWith('{')) return [first, JSON.parse(args.slice(1).join(' '))];
  switch (form) {
    // A click with modifiers held (gallery F20): `tap <target> modifiers Shift+Meta`.
    case 'modifiers': return [first, options(args.slice(1), ['modifiers'])];
    case 'down': return [first, { down: true, ...options(rest, ['at', 'modifiers']) }];
    case 'hover': return [first, { hover: true, ...options(rest, []) }];
    case 'history': if (rest.length !== 1) refuse('history takes one step, as history -1'); return [first, { history: number(rest[0], 'history') }];
    case 'mediasession':
      if (!rest[0] || rest.length > 2) refuse('mediasession <action> [seconds]');
      return [first, { mediaSession: rest[0], ...(rest[1] !== undefined ? { seconds: number(rest[1], 'mediasession') } : {}) }];
    case 'pinch': return [first, { pinch: number(rest[0], 'pinch'), ...options(rest.slice(1), ['at']) }];
    // Files dragged in from outside, each a path (studio diary R19).
    case 'drop': if (!rest.length) refuse('drop takes one or more paths'); return [first, { drop: rest.map((path) => resolve(path)) }];
    case 'into': {
      // tap <list> into <key> [block <v>] [inline <v>]
      if (rest[0] === undefined) refuse('into takes a key');
      const into = { key: String(rest[0]) };
      for (let i = 1; i < rest.length; i += 2) {
        if (!['block', 'inline'].includes(rest[i]) || rest[i + 1] === undefined || rest[i] in into) refuse(`into: unknown option ${rest[i]}; block <start|center|end|nearest> and inline <…>, each once`);
        into[rest[i]] = rest[i + 1];
      }
      return [first, { into }];
    }
    // A wheel at the target's middle or `at` in it, the modifiers held (a pinch is a Control-held wheel; studio diary R3).
    case 'wheel': return [first, { wheel: [number(rest[0], 'wheel'), number(rest[1], 'wheel')], ...options(rest.slice(2), ['gesture', 'at', 'modifiers']) }];
    // The mouse's buttons (#107): the left, a double click, the right (`contextmenu`), the middle (`auxclick`), n clicks.
    case 'mouse': case 'dblclick': case 'contextmenu': case 'auxclick': return [first, { [form]: true, ...options(rest, ['at', 'modifiers']) }];
    case 'clicks': return [first, { clicks: number(rest[0], 'clicks'), ...options(rest.slice(1), ['at', 'modifiers']) }];
    default: refuse(`unknown word ${JSON.stringify(form)}; tap <target> [modifiers <M> | down [at <x> <y>] [modifiers <M>] | wheel <dx> <dy> [gesture] [at <x> <y>] [modifiers <M>] | mouse | dblclick | contextmenu | auxclick | clicks <1-3> (each [at <x> <y>] [modifiers <M>]) | hover | drop <path…> | into <key> … | pinch <scale> [at <x> <y>] | history <n> | mediasession <action> [seconds] | drag …]`);
  }
}

/** What of a `tap` this carrier cannot deliver as a hand's (#107), or null. AppKit and Chrome carry every form;
 * elsewhere a middle click, a run of clicks, a wheel or a double click at a point, and modifiers held through a
 * button other than a plain press's or through a contact answer `unsupported`, never another input. */
export function pointerGap(host, browser, opts) {
  if (['macos', 'mac', 'host'].includes(host) || (host === 'web' && (browser ?? 'chrome') === 'chrome')) return null;
  const held = opts.modifiers ?? opts.drag?.modifiers;
  const gaps = [
    opts.auxclick && 'a middle-button click (auxclick)',
    opts.clicks !== undefined && `clicks ${opts.clicks}`,
    opts.wheel && opts.at !== undefined && 'a wheel at a point',
    opts.dblclick && opts.at !== undefined && 'a double click at a point',
    host === 'web' && opts.mouse && 'a mouse click', // the Playwright carrier has none
    held && (opts.contextmenu || opts.dblclick || opts.mouse || opts.down || opts.drag || opts.phase) && 'modifiers held through a context menu, a double click, a mouse click, a contact or a drag',
  ].filter(Boolean);
  return gaps.length ? `${host === 'web' ? browser : host} does not carry ${gaps.join('; ')} (LLP 1012 §1)` : null;
}

/** The CLI `type` line. Key, copy and cut stay whitespace-split.
 * Typed text and a paste's text keep their tail: real newlines stay, and `\\n` `\\t` `\\r`
 * `\\\\` decode, as does a JSON string around the tail. `hello for 100`
 * stays text (notes: a newline was flattened to a space). */
export function typeCommand(line) {
  const body = String(line).trim().replace(/^type(?:\s+|$)/, '');
  const matched = /^(?:"((?:[^"\\]|\\.)*)"|(\S+))(?:\s+([\s\S]*))?$/.exec(body);
  if (!matched) throw new Error('type needs a target');
  const target = matched[1] != null ? JSON.parse(`"${matched[1]}"`) : matched[2];
  const rest = matched[3] ?? '';
  const head = /^\S+/.exec(rest)?.[0];
  // A paste's text is a tail as typed text is: its newlines are the clipboard's.
  const pasted = /^paste\s+([\s\S]*\S[\s\S]*)$/.exec(rest);
  if (pasted) return [target, { clipboard: 'paste', text: decodeTypeText(pasted[1]) }];
  if (head === 'key' || head === 'copy' || head === 'cut' || head === 'paste') return typeArguments([target, ...rest.trim().split(/\s+/)]);
  return [target, decodeTypeText(rest)];
}
function decodeTypeText(text) {
  if (text.length >= 2 && text.startsWith('"') && text.endsWith('"')) {
    try { return JSON.parse(text); } catch { /* a quote in the text, not a JSON string */ }
  }
  return text.replace(/\\([ntr\\])/g, (_, c) => ({ n: '\n', t: '\t', r: '\r', '\\': '\\' }[c]));
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
