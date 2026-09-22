// @ref LLP 1045 D1/D5/D6 — a contentEditable whose text is the Markdown source:
// syntax is hidden (display:none), never removed, so plain typing, composition
// and spelling run natively and are read back. Every editing rule — caret
// placement, deletion, pending formats, list Return, undo — is the shared
// exact-markdown-editor crate, in markup-editor.wasm, fetched with this module
// only when a Markdown textarea mounts.
const NONE = 0xffffffff, HANDLED = 1, SOURCE = 2, PLACE = 4;
// beforeinput kinds, as `mde_before_input` numbers them; 0 runs natively.
const KINDS = { historyUndo: 1, historyRedo: 2, formatBold: 3, formatItalic: 4, formatStrikeThrough: 5, insertParagraph: 7, insertLineBreak: 7,
  insertText: 8, insertReplacementText: 9, insertFromPaste: 10, insertFromDrop: 10, insertFromYank: 10, deleteContentBackward: 11, deleteContentForward: 12 };
const kindOf = t => KINDS[t] ?? (t.startsWith('format') ? 6 : !t.startsWith('delete') ? 0 : /Backward|^deleteByCut$|^deleteContent$/.test(t) ? 13 : 14);
const STYLES = [[1, 'md-b'], [2, 'md-i'], [4, 'md-c'], [8, 'md-s'], [16, 'md-a'], [64, 'md-m']];
const CSS = `
.exact-markdown-editor { overflow: auto; white-space: pre-wrap; overflow-wrap: anywhere; outline: none; caret-color: currentColor; cursor: text; }
.exact-markdown-editor.md-empty::before { content: attr(placeholder); position: absolute; pointer-events: none; color: light-dark(#3c3c4380, #ebebf580); }
.md-hide, .md-collapsed { display: none; }
.md-b { font-weight: 700; } .md-i { font-style: italic; } .md-s { text-decoration: line-through; }
.md-c { font-family: ui-monospace, "SF Mono", Menlo, monospace; font-size: .88em; background: light-dark(#0000000f, #ffffff14); border-radius: 4px; padding: 1px 3px; }
.md-a { color: light-dark(#245aba, #83b3ff); text-decoration: underline; text-underline-offset: 2px; }
.md-m { opacity: .45; }
.md-h1 { font-size: 1.7em; font-weight: 750; line-height: 1.25; margin: .2em 0 .1em; }
.md-h2 { font-size: 1.4em; font-weight: 700; line-height: 1.3; margin: .2em 0 .1em; }
.md-h3 { font-size: 1.2em; font-weight: 700; } .md-h4, .md-h5, .md-h6 { font-weight: 700; }
.md-code { font-family: ui-monospace, "SF Mono", Menlo, monospace; font-size: .88em; background: light-dark(#0000000f, #ffffff14); padding: 0 12px; }
.md-code-first { border-top-left-radius: 8px; border-top-right-radius: 8px; padding-top: 8px; }
.md-code-last { border-bottom-left-radius: 8px; border-bottom-right-radius: 8px; padding-bottom: 8px; }
.md-quote { --q: 1; color: light-dark(#6b6b70, #a1a1aa); padding-left: calc(var(--q) * 18px); background: repeating-linear-gradient(to right, light-dark(#d4d4d8, #52525b) 0 3px, transparent 3px 18px) no-repeat; background-size: calc(var(--q) * 18px) 100%; }
.md-quote[data-quote="2"] { --q: 2; } .md-quote[data-quote="3"] { --q: 3; }
.md-list { --d: 0; padding-left: calc(28px + var(--d) * 24px); }
.md-list[data-depth="1"] { --d: 1; } .md-list[data-depth="2"] { --d: 2; } .md-list[data-depth="3"] { --d: 3; }
.md-quote.md-list { padding-left: calc(var(--q) * 18px + 28px + var(--d) * 24px); }
.md-bullet::before, .md-number::before, .md-task::before { position: absolute; left: calc(var(--d, 0) * 24px + var(--q, 0) * 18px); width: 22px; text-align: center; }
.md-bullet::before { content: "•"; font-weight: 700; } .md-list[data-depth="1"].md-bullet::before { content: "◦"; }
.md-number::before { content: attr(data-num); opacity: .6; text-align: right; font-variant-numeric: tabular-nums; }
.md-task::before { content: ""; top: .3em; margin-left: 3px; width: 13px; height: 13px; border: 1.5px solid currentColor; border-radius: 4px; cursor: pointer; }
.md-task[data-done="1"] { opacity: .6; }
.md-task[data-done="1"]::before { content: "✓"; background: CanvasText; color: Canvas; border-color: CanvasText; font: 700 11px/13px system-ui, sans-serif; }
.md-rule { min-height: 1.5em; } .md-rule::after { content: ""; position: absolute; left: 0; right: 0; top: 50%; border-top: 1px solid light-dark(#e4e4e7, #333338); }
`;

let wasm;
const utf16 = new TextDecoder('utf-16le');
/** Write a string into the editor's input buffer; returns its length. */
function put(s) {
  const at = wasm.mde_input(s.length), m = new Uint16Array(wasm.memory.buffer, at, s.length);
  for (let i = 0; i < s.length; i++) m[i] = s.charCodeAt(i);
  return s.length;
}
const text = n => utf16.decode(new Uint16Array(wasm.memory.buffer, wasm.mde_text(), n));

/** A line's class, attributes and segments, from its `mde_view` record. */
function describe(v, at, line) {
  const [heading, quote, depth, flags, deco, da, db, count] = v.subarray(at + 2, at + 10);
  const cls = ['md-line'], attrs = [], segs = [];
  if (heading) cls.push('md-h' + heading);
  if (flags & 1) cls.push('md-code');
  if (flags & 4) cls.push('md-code-first');
  if (flags & 8) cls.push('md-code-last');
  if (flags & 2) cls.push('md-collapsed');
  if (quote) { cls.push('md-quote'); attrs.push(['data-quote', String(Math.min(quote, 3))]); }
  if (flags & 16) { cls.push('md-list'); attrs.push(['data-depth', String(Math.min(depth, 3))]); }
  if (deco) cls.push(['', 'md-bullet', 'md-number', 'md-task', 'md-rule'][deco]);
  if (deco === 2) attrs.push(['data-num', line.slice(da, db)]);
  if (deco === 3) attrs.push(['data-done', String(da)]);
  let empty = true;
  for (let k = 0, o = at + 10; k < count; k++, o += 3) {
    const style = v[o + 2], hidden = style & 128;
    if (!hidden) empty = false;
    segs.push([line.slice(v[o], v[o + 1]), hidden ? 'md-hide' : STYLES.filter(([bit]) => style & bit).map(([, c]) => c).join(' ')]);
  }
  return { cls: cls.join(' '), attrs, segs, empty };
}
function build(d) {
  const line = document.createElement('div');
  line.className = d.cls;
  for (const [k, value] of d.attrs) line.setAttribute(k, value);
  for (const [t, c] of d.segs) {
    if (!c) { line.append(t); continue; }
    const span = document.createElement('span'); span.className = c; span.textContent = t; line.append(span);
  }
  if (d.empty) line.append(document.createElement('br'));
  return line;
}
/** Whether a line's DOM already is `d` — as it is after most native keystrokes. */
function matches(line, d) {
  if (line.className !== d.cls || line.attributes.length !== 1 + d.attrs.length || d.attrs.some(([k, value]) => line.getAttribute(k) !== value)) return false;
  const kids = line.childNodes;
  if (kids.length !== d.segs.length + (d.empty ? 1 : 0)) return false;
  for (let i = 0; i < d.segs.length; i++) {
    const [t, c] = d.segs[i], k = kids[i];
    if (c ? !(k.nodeName === 'SPAN' && k.className === c && k.attributes.length === 1 && k.childNodes.length === 1 && k.firstChild.nodeType === 3 && k.firstChild.data === t)
      : !(k.nodeType === 3 && k.data === t)) return false;
  }
  return !d.empty || kids[kids.length - 1].nodeName === 'BR';
}

// Replacing the textarea happens only once this module and its wasm are
// ready; until then it remains an ordinary, usable source editor.
function installMarkupEditor(textarea, host) {
  const el = document.createElement('div');
  for (const attr of textarea.attributes) el.setAttribute(attr.name, attr.value);
  el.classList.add('exact-markdown-editor');
  const h = wasm.mde_new(), defaults = new Set(), ours = ['contenteditable', 'role', 'aria-multiline', 'aria-readonly', 'aria-disabled', 'aria-placeholder'];
  let source = '', lines = [], keys = [], starts = [0], index = new WeakMap(), payload = '', placed = null;
  let destroyed = false, composing = false, notifying = false, pointer = false, pendingValue, pendingSync = false;
  let tabIndex = el.getAttribute('tabindex');
  const writable = () => !el.hasAttribute('disabled') && !el.hasAttribute('readonly') && !el.closest('[inert]');
  const now = () => performance.now();
  const sel = () => [wasm.mde_sel(h, 0), wasm.mde_sel(h, 1)];

  function render() {
    const n = wasm.mde_view(h), v = new Uint32Array(wasm.memory.buffer, wasm.mde_out(), n).slice();
    const next = [], found = [];
    starts = [];
    for (let i = 0, at = 1; i < v[0]; i++) {
      const line = source.slice(v[at], v[at + 1]), end = at + 10 + v[at + 9] * 3;
      next.push(v.subarray(at + 2, end).join(',') + '\u0000' + line);
      found.push([at, line]); starts.push(v[at]); at = end;
    }
    let head = 0, tail = 0;
    while (head < keys.length && head < next.length && keys[head] === next[head]) head++;
    while (tail < keys.length - head && tail < next.length - head && keys[keys.length - 1 - tail] === next[next.length - 1 - tail]) tail++;
    const removed = lines.splice(head, keys.length - head - tail);
    const fresh = found.slice(head, next.length - tail).map(([at, line]) => describe(v, at, line));
    // A native keystroke usually leaves the DOM exactly as it would be drawn:
    // keep those nodes, so the input method's autocorrect state survives.
    const kept = removed.length === fresh.length && removed.every((line, i) => matches(line, fresh[i]));
    const added = kept ? removed.splice(0) : fresh.map(build);
    const before = lines[head] ?? null;
    for (const line of removed) line.remove();
    for (const line of added) if (!line.isConnected) el.insertBefore(line, before);
    lines.splice(head, 0, ...added);
    keys = next;
    lines.forEach((line, i) => index.set(line, i));
    el.classList.toggle('md-empty', source === '');
  }
  function read() { source = utf16.decode(new Uint16Array(wasm.memory.buffer, wasm.mde_source(h), wasm.mde_source_len(h))); }

  /** The source offset of a DOM point inside the editor. */
  function toSource(node, offset) {
    if (node === el) return offset >= lines.length ? source.length : starts[offset] ?? 0;
    const line = (node.nodeType === 3 ? node.parentElement : node)?.closest('.md-line');
    if (!line || !index.has(line)) return sel()[0];
    const r = document.createRange();
    r.setStart(line, 0); r.setEnd(node, offset);
    return starts[index.get(line)] + r.toString().length;
  }
  /** A DOM point for a source offset, on visible text left or right of hidden syntax. */
  function toDom(p, right) {
    let lo = 0, hi = starts.length - 1;
    while (lo < hi) { const mid = (lo + hi + 1) >> 1; if (starts[mid] <= p) lo = mid; else hi = mid - 1; }
    const line = lines[lo];
    if (!line) return { node: el, offset: 0 };
    const local = p - starts[lo], walker = document.createTreeWalker(line, NodeFilter.SHOW_TEXT), found = [];
    for (let t = walker.nextNode(), acc = 0; t; acc += t.data.length, t = walker.nextNode()) {
      if (!t.parentElement.classList.contains('md-hide') && acc <= local && local <= acc + t.data.length) found.push({ node: t, offset: local - acc, start: local === acc, end: local === acc + t.data.length });
    }
    if (found.length) return right ? found.find(c => !c.end) ?? found.at(-1) : found.find(c => !c.start) ?? found[0];
    const br = line.querySelector(':scope > br');
    return { node: line, offset: br ? [...line.childNodes].indexOf(br) : line.childNodes.length };
  }
  function domSelection() {
    const s = document.getSelection();
    if (!s?.rangeCount || !el.contains(s.anchorNode) || !el.contains(s.focusNode)) return null;
    const a = toSource(s.anchorNode, s.anchorOffset), f = toSource(s.focusNode, s.focusOffset);
    return [Math.min(a, f), Math.max(a, f)];
  }
  function place(reveal) {
    if (document.activeElement !== el) return;
    const [from, to] = sel(), a = toDom(from, wasm.mde_draws_after(h, from) === 1), b = from === to ? a : toDom(to, false);
    placed = [from, to];
    document.getSelection().setBaseAndExtent(a.node, a.offset, b.node, b.offset);
    if (!reveal) return;
    const s = document.getSelection(), focus = s.focusNode?.nodeType === 3 ? s.focusNode.parentElement : s.focusNode;
    let r = s.rangeCount ? s.getRangeAt(0).getBoundingClientRect() : null;
    if (!r?.height && focus) r = focus.getBoundingClientRect();
    const box = el.getBoundingClientRect();
    if (r && r.bottom > box.bottom) el.scrollTop += r.bottom - box.bottom;
    else if (r && r.top < box.top) el.scrollTop -= box.top - r.top;
  }
  function emit() {
    if (destroyed || !host.live(el)) return;
    const next = text(wasm.mde_facts(h));
    if (next !== payload) { payload = next; host.select(el, payload); }
  }
  function notify() {
    notifying = true;
    try { el.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText' })); }
    finally { notifying = false; }
    if (pendingValue !== undefined || pendingSync) queueMicrotask(flush);
  }
  /** Show what an editing call did: redraw, place the caret, tell the app. */
  function after(bits) {
    if (destroyed) return;
    if (bits & SOURCE) { read(); render(); }
    if (bits & PLACE) place(true);
    if (bits & SOURCE) notify();
    emit();
  }
  function run(command, argument = '') {
    if (destroyed || !writable() || composing) return false;
    const n = put(command + argument);
    after(wasm.mde_command(h, command.length, n - command.length, now()));
    return true;
  }
  function reconcile() {
    if (destroyed || composing) return;
    const s = domSelection(), n = put(lines.map(line => line.textContent).join('\n'));
    after(wasm.mde_reconcile(h, n, s ? s[0] : NONE, s ? s[1] : NONE, now()));
  }
  function selectionChanged() {
    if (destroyed || composing || !host.live(el)) return;
    const s = domSelection();
    if (!s) return;
    const echo = placed; placed = null;
    const answer = wasm.mde_select(h, s[0], s[1], echo ? echo[0] : NONE, echo ? echo[1] : NONE);
    // Mid-drag the platform owns the selection; snap it when the drag ends.
    if (answer === 2 && !pointer) place(false);
    if (answer) emit();
  }
  function setValue(value) {
    value = String(value);
    if (destroyed) return;
    if (value === source) { pendingValue = undefined; return; }
    if (composing || notifying) { pendingValue = value; return; }
    pendingValue = undefined;
    const bits = wasm.mde_set_value(h, put(value));
    if (bits & SOURCE) { read(); render(); place(false); emit(); }
  }
  function flush() {
    if (destroyed || composing) return;
    if (pendingValue !== undefined) setValue(pendingValue);
    if (pendingSync) { pendingSync = false; sync(); }
  }
  const link = () => {
    if (!writable()) return;
    const current = text(wasm.mde_facts(h)).split('\n')[3];
    const url = window.prompt('Link URL', current || 'https://');
    if (url !== null) run('link', url);
  };
  function copied() {
    if (!wasm.mde_copy(h)) return null;
    const o = new Uint32Array(wasm.memory.buffer, wasm.mde_out(), 2);
    return source.slice(o[0], o[1]);
  }
  const copyPlain = () => {
    const markdown = copied();
    if (markdown === null) return;
    navigator.clipboard?.writeText(text(wasm.mde_plain(put(markdown)))).catch(error => console.warn('exact: plain copy failed', error));
  };
  function configure() {
    const disabled = el.hasAttribute('disabled'), readonly = el.hasAttribute('readonly');
    if (el.hasAttribute('tabindex') && !disabled) tabIndex = el.getAttribute('tabindex');
    el.contentEditable = disabled || readonly ? 'false' : 'true';
    if (disabled) el.removeAttribute('tabindex');
    else if (tabIndex !== null || readonly) el.setAttribute('tabindex', tabIndex ?? '0');
    el.setAttribute('role', 'textbox'); el.setAttribute('aria-multiline', 'true');
    el.setAttribute('aria-readonly', String(readonly)); el.setAttribute('aria-disabled', String(disabled));
    if (el.hasAttribute('placeholder')) el.setAttribute('aria-placeholder', el.getAttribute('placeholder')); else el.removeAttribute('aria-placeholder');
    for (const [name, value] of [['spellcheck', String(textarea.spellcheck)], ['autocorrect', 'on'], ['autocapitalize', textarea.autocapitalize || 'sentences'], ['writingsuggestions', 'true']]) {
      if (!el.hasAttribute(name)) { el.setAttribute(name, value); defaults.add(name); }
    }
    if (disabled && document.activeElement === el) el.blur();
  }
  function sync() {
    if (destroyed) return;
    if (composing || notifying) { pendingSync = true; return; }
    if (el.getAttribute('markup') === 'markdown') { configure(); return; }
    // Markdown off: the same source in a plain textarea, the explicit source mode.
    const native = document.createElement('textarea'), [from, to] = sel(), focused = document.activeElement === el;
    for (const attr of el.attributes) if (!ours.includes(attr.name) && !defaults.has(attr.name)) native.setAttribute(attr.name, attr.value);
    if (tabIndex !== null) native.setAttribute('tabindex', tabIndex);
    native.classList.remove('exact-markdown-editor', 'md-empty');
    if (!native.classList.length) native.removeAttribute('class');
    native.value = source; native.exactSourceValue = source;
    native.setSelectionRange(from, to);
    host.replace(native); destroy(); el.replaceWith(native);
    if (focused && !native.disabled) native.focus();
  }
  function destroy() {
    if (destroyed) return;
    destroyed = true;
    document.removeEventListener('selectionchange', selectionChanged);
    window.removeEventListener('pointerup', release); window.removeEventListener('pointercancel', release);
    wasm.mde_drop(h);
  }

  el.addEventListener('beforeinput', e => {
    if (destroyed || e.isComposing || e.inputType === 'insertCompositionText') return;
    if (!writable()) { e.preventDefault(); return; }
    const kind = kindOf(e.inputType);
    if (!kind) return;
    let a = NONE, b = NONE;
    const target = (kind === 9 || kind >= 11) && e.getTargetRanges?.()[0];
    if (target) { const x = toSource(target.startContainer, target.startOffset), y = toSource(target.endContainer, target.endOffset); a = Math.min(x, y); b = Math.max(x, y); }
    const dt = e.dataTransfer, data = e.data ?? (dt && (dt.getData('text/markdown') || dt.getData('text/plain'))) ?? '';
    const bits = wasm.mde_before_input(h, kind, put(data), a, b, now());
    if (!(bits & HANDLED)) return; // plain typing: the platform's, read back on input
    e.preventDefault();
    after(bits);
  });
  // The app hears this editor's own input events only, carrying the source.
  el.addEventListener('input', e => {
    if (notifying) return;
    e.stopImmediatePropagation();
    if (!e.isComposing) reconcile();
  });
  el.addEventListener('compositionstart', () => { composing = true; });
  el.addEventListener('compositionend', () => { composing = false; queueMicrotask(() => { reconcile(); flush(); }); });
  el.addEventListener('keydown', e => {
    if (e.isComposing || destroyed) return;
    const mod = e.metaKey || e.ctrlKey, key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
    let command = null;
    if (mod && !e.altKey) {
      if (key === 'z') command = e.shiftKey ? 'redo' : 'undo';
      else if (key === 'y' && !e.shiftKey) command = 'redo';
      else if (key === 'b' && !e.shiftKey) command = 'bold';
      else if (key === 'i' && !e.shiftKey) command = 'italic';
      else if (key === ']') command = 'indent';
      else if (key === '[') command = 'outdent';
      else if (key === 'k' && !e.shiftKey) { e.preventDefault(); link(); return; }
      else if (key === 'c' && e.shiftKey) { e.preventDefault(); copyPlain(); return; }
    } else if (key === 'Tab' && !e.altKey && / (bullet|ordered|task) /.test(` ${payload.split('\n')[0]} `)) command = e.shiftKey ? 'outdent' : 'indent';
    if (!command || !writable()) return;
    e.preventDefault();
    run(command);
  });
  const clip = cut => e => {
    const markdown = copied();
    if (markdown === null) return;
    e.preventDefault();
    // Copy keeps the literal source (LLP 1045 D1); Mod-Shift-C copies plain text.
    e.clipboardData.setData('text/plain', markdown);
    e.clipboardData.setData('text/markdown', markdown);
    if (cut && writable()) after(wasm.mde_cut(h, now()));
  };
  el.addEventListener('copy', clip(false));
  el.addEventListener('cut', clip(true));
  el.addEventListener('pointerdown', e => {
    const line = e.target.closest?.('.md-task');
    if (line && el.contains(line) && writable() && e.clientX - line.getBoundingClientRect().left <= parseFloat(getComputedStyle(line).paddingLeft)) {
      e.preventDefault();
      after(wasm.mde_toggle_task(h, starts[index.get(line)], now()));
      return;
    }
    pointer = true;
  });
  const release = () => { if (pointer) { pointer = false; queueMicrotask(selectionChanged); } };
  window.addEventListener('pointerup', release);
  window.addEventListener('pointercancel', release);
  el.addEventListener('focus', () => queueMicrotask(emit));
  document.addEventListener('selectionchange', selectionChanged);

  const raw = textarea.exactSourceValue;
  wasm.mde_load(h, put(raw?.replace(/\r\n?/g, '\n') === textarea.value ? raw : textarea.value));
  read(); render();
  wasm.mde_select(h, textarea.selectionStart, textarea.selectionEnd, NONE, NONE);
  Object.defineProperty(el, 'value', { get: () => source, set: setValue });
  el.focus = options => { if (el.hasAttribute('disabled')) return; HTMLElement.prototype.focus.call(el, options); place(false); };
  el.select = () => { if (el.hasAttribute('disabled')) return; wasm.mde_select_all(h); place(false); emit(); };
  el.exactMarkup = { sync, format(command, argument = '') { if (destroyed || !writable()) return false; el.focus(); return run(command, String(argument)); }, setValue, destroy };
  configure();
  const initialFocus = document.activeElement === textarea;
  textarea.replaceWith(el);
  host.replace(el);
  if (initialFocus) el.focus();
  queueMicrotask(emit);
  return el;
}

const style = document.createElement('style');
style.textContent = CSS;
document.head.append(style);
// glue awaits this: the install function once the editor's wasm is ready.
globalThis.exact.installMarkupEditor = fetch(new URL('./markup-editor.wasm', import.meta.url))
  .then(r => { if (!r.ok) throw new Error(`markup-editor.wasm: ${r.status}`); return r.arrayBuffer(); })
  .then(bytes => WebAssembly.instantiate(bytes, {}))
  .then(({ instance }) => { wasm = instance.exports; return installMarkupEditor; });
