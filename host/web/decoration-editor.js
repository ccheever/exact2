// App-computed styling over a plain source string (LLP 1114.007).
// Editing rules stay in the app. Only source mapping, atoms and undo live here.
import { reconcileDecorations } from './markup-editor.js';
import { inertAncestor } from './navigation.js';

const EDITOR_CSS = `.exact-decoration-editor{white-space:pre-wrap;overflow-wrap:anywhere;overflow:auto;outline:none;cursor:text}
.exact-decoration-editor:empty::before{content:attr(placeholder);color:var(--exact-placeholder-color,GrayText);pointer-events:none}
.exact-decoration-line{position:relative;min-height:1lh}
.exact-decoration-hidden{display:none}
.exact-decoration-revealed{opacity:.45}
.exact-decoration-atom{display:inline-block;white-space:nowrap;cursor:default;border-radius:.3em;padding:0 .2em;background:Highlight;color:HighlightText}
.exact-decoration-marker{position:absolute;left:0;width:var(--exact-decoration-gutter,1.5em);box-sizing:border-box;text-align:right;padding-right:.35em;user-select:none;cursor:default}
.exact-decoration-rule{position:absolute;left:0;right:0;top:50%;border-top:1px solid currentColor;pointer-events:none}
`;

/** Read native DOM edits, retaining hidden source but excluding marker paint. */
export function readDecoratedSource(root) {
  const points = new Map(); let value = '';
  const block = n => n?.nodeType === 1 && /^(DIV|P)$/.test(n.nodeName);
  function walk(node) {
    if (node.nodeType === 3) { points.set(node, value.length); value += node.data; return; }
    if (node.dataset?.decorationPaint !== undefined) { points.set(node, [value.length]); return; }
    const offsets = [value.length], kids = [...node.childNodes];
    for (let i = 0; i < kids.length; i++) {
      const child = kids[i];
      if (i && (block(child) || block(kids[i - 1]))) value += '\n';
      offsets[i] = value.length;
      if (child.nodeName === 'BR') { points.set(child, [value.length]); if (i < kids.length - 1) value += '\n'; }
      else walk(child);
      offsets[i + 1] = value.length;
    }
    points.set(node, offsets);
  }
  walk(root);
  const offset = (node, at) => { const point = points.get(node); return typeof point === 'number' ? point + at : point?.[at] ?? value.length; };
  return { value, offset };
}

function installContentEditable(textarea, host) {
  const el = document.createElement('div');
  for (const attribute of textarea.attributes) el.setAttribute(attribute.name, attribute.value);
  el.classList.add('exact-decoration-editor');
  el.setAttribute('role', 'textbox'); el.setAttribute('aria-multiline', 'true');
  let source = textarea.value, start = textarea.selectionStart ?? 0, end = textarea.selectionEnd ?? 0, direction = 'none';
  let composing = false, destroyed = false, rendering = false, lastChange = source, pendingValue, placed;
  let resolved = { provisional: false, spans: [], blocks: [], hidden: [], atoms: [] }, styles = {}, points = [];
  const undo = [], redo = [], warned = new Set();
  const writable = () => !el.hasAttribute('disabled') && !el.hasAttribute('readonly') && !inertAncestor(el);
  const snapshot = () => ({ value: source, start, end, direction });
  const same = (a, b) => a.value === b.value && a.start === b.start && a.end === b.end;
  const remember = () => { const value = snapshot(); if (!undo.length || !same(undo.at(-1), value)) undo.push(value); if (undo.length > 500) undo.shift(); redo.length = 0; };
  const emit = (type = 'insertReplacementText', data = null) => el.dispatchEvent(new InputEvent('input', { inputType: type, data, bubbles: true }));
  function selectFromDOM() {
    const s = getSelection();
    if (!s || !el.contains(s.anchorNode) || !el.contains(s.focusNode)) return false;
    if (placed && placed[0] === s.anchorNode && placed[1] === s.anchorOffset && placed[2] === s.focusNode && placed[3] === s.focusOffset) return true;
    const read = readDecoratedSource(el), a = read.offset(s.anchorNode, s.anchorOffset), b = read.offset(s.focusNode, s.focusOffset);
    start = Math.min(a, b); end = Math.max(a, b); direction = a > b ? 'backward' : 'forward';
    return true;
  }
  function domPoint(offset, after = false) {
    const atomic = points.find(p => p.atom && (offset === p.start || offset === p.end));
    if (atomic) return [atomic.element.parentNode, [...atomic.element.parentNode.childNodes].indexOf(atomic.element) + (offset === atomic.end ? 1 : 0)];
    const hits = points.filter(p => p.start <= offset && offset <= p.end && p.node.isConnected && !p.hidden);
    const at = after ? hits.find(p => offset < p.end) ?? hits.at(-1) : hits.find(p => offset > p.start) ?? hits[0];
    if (at) return [at.node, Math.min(at.node.data.length, Math.max(0, offset - at.start))];
    const nearest = points.filter(p => !p.hidden && p.node.isConnected).sort((a, b) => Math.abs(a.start - offset) - Math.abs(b.start - offset))[0];
    return nearest ? [nearest.node, offset < nearest.start ? 0 : nearest.node.data.length] : [el.firstChild ?? el, 0];
  }
  function place() {
    if (document.activeElement !== el || composing) return;
    const a = domPoint(start, true), b = start === end ? a : domPoint(end);
    getSelection().setBaseAndExtent(...(direction === 'backward' ? [...b, ...a] : [...a, ...b]));
    const s = getSelection(); placed = [s.anchorNode, s.anchorOffset, s.focusNode, s.focusOffset];
  }
  function css(name) {
    if (Object.hasOwn(styles, name)) return styles[name];
    if (!warned.has(name)) { warned.add(name); host.log?.(`decorations: unknown declaration ${JSON.stringify(name)}`); }
    return '';
  }
  function configure() {
    el.contentEditable = String(writable());
    el.setAttribute('aria-readonly', String(el.hasAttribute('readonly')));
    el.setAttribute('aria-disabled', String(el.hasAttribute('disabled')));
    if (el.hasAttribute('disabled')) el.tabIndex = -1;
    else el.tabIndex = Number(textarea.getAttribute('tabindex') ?? 0);
  }
  function render() {
    if (destroyed || composing) return;
    rendering = true;
    try {
      styles = JSON.parse(el.getAttribute('data-decoration-styles') ?? '{}');
      const record = JSON.parse(el.getAttribute('data-decorations') ?? 'null');
      if (record && typeof record.value === 'string') resolved = reconcileDecorations(record, source);
      else resolved = { provisional: false, spans: [], blocks: [], hidden: [], atoms: [] };
    } catch (error) { host.log?.(`decorations: ${error.message}`); resolved = { provisional: false, spans: [], blocks: [], hidden: [], atoms: [] }; }
    points = []; const lines = source.split('\n'), fragment = document.createDocumentFragment(); let base = 0;
    for (const text of lines) {
      const line = document.createElement('div'); line.className = 'exact-decoration-line';
      const blocks = resolved.blocks.filter(b => b.start <= base && base < b.end);
      for (const block of blocks) line.style.cssText += css(block.name);
      const cuts = new Set([0, text.length]);
      for (const range of [...resolved.spans, ...resolved.hidden, ...resolved.atoms]) {
        if (range.end > base && range.start < base + text.length) { cuts.add(Math.max(0, range.start - base)); cuts.add(Math.min(text.length, range.end - base)); }
      }
      // Inline style boundaries cannot split a chip into editable source pieces.
      for (const cut of cuts) if (resolved.atoms.some(atom => atom.start < base + cut && base + cut < atom.end)) cuts.delete(cut);
      const sorted = [...cuts].sort((a, b) => a - b);
      for (let i = 0; i < sorted.length - 1; i++) {
        const from = base + sorted[i], to = base + sorted[i + 1]; if (from === to) continue;
        const span = document.createElement('span'), node = document.createTextNode(source.slice(from, to));
        for (const range of resolved.spans.filter(r => r.start <= from && to <= r.end)) span.style.cssText += css(range.name);
        const hidden = resolved.hidden.find(r => r.start <= from && to <= r.end);
        const reveal = hidden?.reveal && start <= hidden.reveal.end && end >= hidden.reveal.start;
        if (hidden) { span.classList.add(reveal ? 'exact-decoration-revealed' : 'exact-decoration-hidden'); if (!reveal) span.setAttribute('aria-hidden', 'true'); }
        const atom = resolved.atoms.find(r => r.start === from && r.end === to);
        if (atom) {
          span.classList.add('exact-decoration-atom'); span.contentEditable = 'false'; span.style.cssText += css(atom.name);
          span.setAttribute('aria-label', atom.label);
          const raw = document.createElement('span'); raw.className = 'exact-decoration-hidden'; raw.setAttribute('aria-hidden', 'true'); raw.append(node); span.append(raw);
          const label = document.createElement('span'); label.dataset.decorationPaint = ''; label.textContent = atom.label; span.append(label);
        } else span.append(node);
        line.append(span); points.push({ node, element: span, atom: !!atom, start: from, end: to, hidden: !!hidden && !reveal || !!atom });
      }
      if (!text) line.append(document.createElement('br'));
      for (const block of blocks.filter(b => b.marker)) {
        if (block.marker !== 'rule' && !line.style.paddingLeft) line.style.paddingLeft = '1.5em';
        line.style.setProperty('--exact-decoration-gutter', line.style.paddingLeft || '1.5em');
        const marker = document.createElement('span'); marker.dataset.decorationPaint = ''; marker.contentEditable = 'false'; marker.className = block.marker === 'rule' ? 'exact-decoration-rule' : 'exact-decoration-marker';
        if (block.marker.startsWith('checkbox:')) {
          const checked = block.marker === 'checkbox:on'; marker.setAttribute('role', 'checkbox'); marker.setAttribute('aria-checked', String(checked)); marker.setAttribute('aria-label', 'Task'); marker.textContent = checked ? '☑' : '☐';
          marker.addEventListener('pointerdown', e => { e.preventDefault(); if (writable()) host.toggle?.(el, checked, block.start, block.end); });
        } else { marker.setAttribute('aria-hidden', 'true'); marker.textContent = block.marker === 'bullet' ? '•' : block.marker === 'source' ? source.slice(block.start, block.end).match(/^\s*\d+[.)]/)?.[0].trim() ?? '' : ''; }
        line.prepend(marker);
      }
      fragment.append(line); base += text.length + 1;
    }
    el.replaceChildren(fragment); configure(); place(); rendering = false;
    el.exactDecorations.applied = { ...resolved, value: source };
  }
  function restore(value, type) {
    ({ value: source, start, end, direction } = value); render(); emit(type);
  }
  function replace(text, from = start, to = end, mode = 'preserve', undoable = false) {
    if (!writable() || composing) return false;
    const size = source.length; from = Math.max(0, Math.min(size, Number(from) >>> 0)); to = Math.max(0, Math.min(size, Number(to) >>> 0));
    if (from > to || !['preserve', 'select', 'start', 'end'].includes(mode)) throw new DOMException('Invalid range', 'IndexSizeError');
    for (const atom of resolved.atoms) if (from < atom.end && to > atom.start || from === to && atom.start < from && from < atom.end) { from = Math.min(from, atom.start); to = Math.max(to, atom.end); }
    if (undoable) remember(); else { undo.length = 0; redo.length = 0; }
    const oldStart = start, oldEnd = end, delta = text.length - (to - from);
    source = source.slice(0, from) + text + source.slice(to);
    const carry = (p, finish) => p <= from ? p : p >= to ? p + delta : finish ? from + text.length : from;
    [start, end] = mode === 'select' ? [from, from + text.length] : mode === 'start' ? [from, from] : mode === 'end' ? [from + text.length, from + text.length] : [carry(oldStart, false), carry(oldEnd, true)];
    render(); emit('insertReplacementText', text); return true;
  }
  function history(back) {
    const from = back ? undo : redo, to = back ? redo : undo;
    if (!from.length) return; to.push(snapshot()); restore(from.pop(), back ? 'historyUndo' : 'historyRedo');
  }
  const selectionChanged = () => {
    const previous = `${start},${end},${direction}`;
    if (rendering || composing || destroyed || !selectFromDOM() || previous === `${start},${end},${direction}`) return;
    render(); el.dispatchEvent(new Event('selectionchange')); el.dispatchEvent(new Event('select'));
  };
  el.addEventListener('beforeinput', e => {
    if (e.defaultPrevented || !writable() || composing || e.isComposing) return;
    selectFromDOM();
    if (e.inputType === 'historyUndo' || e.inputType === 'historyRedo') { e.preventDefault(); history(e.inputType === 'historyUndo'); return; }
    const atom = start === end && resolved.atoms.find(a => e.inputType === 'deleteContentBackward' && start === a.end || e.inputType === 'deleteContentForward' && start === a.start);
    if (atom) { e.preventDefault(); replace('', atom.start, atom.end, 'start', true); return; }
    remember();
  });
  el.addEventListener('input', e => {
    if (e.inputType === 'insertReplacementText' && el.textContent !== source && rendering) return;
    if (composing || e.isComposing) return;
    const read = readDecoratedSource(el); if (read.value === source) return;
    const selection = getSelection(); start = read.offset(selection?.anchorNode, selection?.anchorOffset); end = read.offset(selection?.focusNode, selection?.focusOffset);
    if (start > end) [start, end] = [end, start]; source = read.value; render();
  });
  el.addEventListener('compositionstart', () => { selectFromDOM(); remember(); composing = true; });
  el.addEventListener('compositionend', () => {
    composing = false; const read = readDecoratedSource(el); source = read.value; selectFromDOM();
    if (pendingValue !== undefined) { if (source !== pendingValue) { undo.length = 0; redo.length = 0; } source = pendingValue; pendingValue = undefined; }
    render();
  });
  el.addEventListener('keydown', e => {
    if (e.defaultPrevented || e.isComposing || !writable()) return;
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'z') { e.preventDefault(); history(!e.shiftKey); }
  });
  for (const kind of ['copy', 'cut']) el.addEventListener(kind, e => {
    if (e.defaultPrevented || !e.clipboardData || !selectFromDOM() || start === end) return;
    e.clipboardData.setData('text/plain', source.slice(start, end)); e.preventDefault();
    if (kind === 'cut' && writable()) replace('', start, end, 'start', true);
  });
  el.addEventListener('blur', () => { if (lastChange !== source) { lastChange = source; el.dispatchEvent(new Event('change', { bubbles: true })); } });
  document.addEventListener('selectionchange', selectionChanged);
  Object.defineProperties(el, {
    value: { get: () => source, set(value) { value = String(value); if (composing) { pendingValue = value; return; } if (source === value) return; undo.length = 0; redo.length = 0; source = value; start = Math.min(start, source.length); end = Math.min(end, source.length); render(); } },
    selectionStart: { get: () => start }, selectionEnd: { get: () => end }, selectionDirection: { get: () => direction },
  });
  el.setSelectionRange = (a, b, dir = 'none') => { end = Math.min(source.length, Number(b) >>> 0); start = Math.min(end, Number(a) >>> 0); direction = dir; render(); };
  el.setRangeText = (text, ...range) => replace(String(text), ...range);
  el.select = () => el.setSelectionRange(0, source.length);
  el.exactDecorations = { applied: null, sync: render, replace, rangeFrame(a, b) {
    if (composing || a > b || a < 0 || b > source.length) return null;
    const range = document.createRange(); range.setStart(...domPoint(a, true)); range.setEnd(...domPoint(b)); return range.getBoundingClientRect();
  }, destroy() { destroyed = true; document.removeEventListener('selectionchange', selectionChanged); } };
  const focused = document.activeElement === textarea; textarea.replaceWith(el); host.replace(el); render(); if (focused) { el.focus(); place(); }
  return el;
}

let nextPaintId = 0;
const paintProperty = /^(?:color|background-color|text-decoration(?:-(?:line|color|style|thickness))?|text-shadow)\s*:/;
function paintOnly(el) {
  if (typeof el.createValueRange !== 'function' || typeof Highlight !== 'function' || !globalThis.CSS?.highlights) return false;
  try {
    const record = JSON.parse(el.getAttribute('data-decorations') ?? 'null'), table = JSON.parse(el.getAttribute('data-decoration-styles') ?? '{}');
    return !!record && !['blocks', 'hidden', 'atoms'].some(kind => record[kind]?.length)
      && (record.spans ?? []).every(span => (table[span.name] ?? '').split(';').filter(part => part.trim()).every(part => paintProperty.test(part.trim())));
  } catch { return false; }
}

/** Keep the browser's textarea and native editing for paint-only records. */
function installPaint(textarea, host) {
  const id = ++nextPaintId, sheet = document.createElement('style'), registrations = [], abort = new AbortController();
  document.head.append(sheet); let composing = false, disposed = false;
  const clear = () => { for (const [name, range] of registrations) { CSS.highlights.delete(name); range.disconnect(); } registrations.length = 0; };
  const sync = () => {
    if (disposed || composing) return;
    if (!paintOnly(textarea)) { destroy(); delete textarea.exactDecorations; installContentEditable(textarea, host); return; }
    clear();
    const record = JSON.parse(textarea.getAttribute('data-decorations')), table = JSON.parse(textarea.getAttribute('data-decoration-styles') ?? '{}');
    const resolved = reconcileDecorations(record, textarea.value), rules = [];
    resolved.spans.forEach((span, index) => {
      if (!Object.hasOwn(table, span.name)) return;
      const name = `exact-decoration-${id}-${index}`, range = textarea.createValueRange(span.start, span.end);
      CSS.highlights.set(name, new Highlight(range)); registrations.push([name, range]);
      rules.push(`::highlight(${name}){${table[span.name]}}`);
    });
    sheet.textContent = rules.join('\n'); textarea.exactDecorations.applied = { ...resolved, value: textarea.value };
  };
  const destroy = () => { disposed = true; clear(); sheet.remove(); abort.abort(); };
  textarea.exactDecorations = { painted: true, sync, destroy, applied: null, rangeFrame(a, b) {
    if (composing || a < 0 || a > b || b > textarea.value.length) return null;
    const range = textarea.createValueRange(a, b), box = range.getBoundingClientRect(); range.disconnect(); return box;
  } };
  textarea.addEventListener('input', sync, { capture: true, signal: abort.signal });
  textarea.addEventListener('compositionstart', () => { composing = true; }, { signal: abort.signal });
  textarea.addEventListener('compositionend', () => { composing = false; sync(); }, { signal: abort.signal });
  sync(); return textarea;
}

function install(textarea, host) { return paintOnly(textarea) ? installPaint(textarea, host) : installContentEditable(textarea, host); }

if (!document.querySelector('style[data-exact-decorations]')) { const style = document.createElement('style'); style.dataset.exactDecorations = ''; style.textContent = EDITOR_CSS; document.head.append(style); }
globalThis.exact.installDecorationEditor = globalThis.exact.installMarkupEditor.then(() => install);

/** Keep compiled reactive bindings attached while the native field becomes an editor. */
export function bindRuntime(e, {install, id, Ids, Views, log}) {
  let current = e;
  install(e, {log, replace(n) {
    current = n; Ids.set(n, id); Views.set(id, n);
    for (const name of ['value', 'selectionStart', 'selectionEnd', 'selectionDirection']) Object.defineProperty(e, name, {configurable:true, get:()=>n[name], ...(name === 'value' ? {set:v=>{n.value=v;}} : {})});
    e.setSelectionRange = (...args) => n.setSelectionRange(...args); e.setRangeText = (...args) => n.setRangeText(...args);
    for (const type of ['input','beforeinput','change','selectionchange','select','focus','blur','keydown','pointerenter','pointerleave','click','toggle','copy','cut','paste']) n.addEventListener(type, ev => {
      const forwarded = new ev.constructor(type, ev);
      for (const key of ['oldState','newState','start','end']) if (key in ev) Object.defineProperty(forwarded, key, {value:ev[key]});
      if (ev.defaultPrevented) forwarded.preventDefault(); if (!e.dispatchEvent(forwarded)) ev.preventDefault();
    }, ['keydown','beforeinput','copy','cut','paste'].includes(type));
  }, toggle(n, checked, start, end) {
    const event = new Event('toggle'); Object.assign(event, {oldState:checked?'checked':'unchecked', newState:checked?'unchecked':'checked', start, end}); n.dispatchEvent(event);
  }});
  const observer = new MutationObserver(ms => {
    if (current !== e) for (const m of ms) { const v = e.getAttribute(m.attributeName); v == null ? current.removeAttribute(m.attributeName) : current.setAttribute(m.attributeName, v); }
    current.exactDecorations.sync();
  }); observer.observe(e, {attributes:true});
  return () => {observer.disconnect(); current.exactDecorations?.destroy();};
}
