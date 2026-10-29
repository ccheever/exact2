// @ref LLP 1045 D1/D5/D6 — a contentEditable whose text is the Markdown source:
// syntax is hidden (display:none), never removed, so plain typing, composition
// and spelling run natively and are read back. Every editing rule — caret
// placement, deletion, pending formats, list Return, undo — is the shared
// exact-markdown-editor crate, in markup-editor.wasm, fetched with this module
// only when a Markdown textarea mounts.
import { inertAncestor } from './navigation.js';
import { atomBox } from './atoms.js';

const NONE = 0xffffffff, HANDLED = 1, SOURCE = 2, PLACE = 4;
// beforeinput kinds, as `mde_before_input` numbers them; 0 runs natively.
const KINDS = { historyUndo: 1, historyRedo: 2, formatBold: 3, formatItalic: 4, formatStrikeThrough: 5, insertParagraph: 7, insertLineBreak: 7,
  insertText: 8, insertReplacementText: 9, insertFromPaste: 10, insertFromDrop: 10, insertFromYank: 10, deleteContentBackward: 11, deleteContentForward: 12,
  deleteWordBackward: 15, deleteWordForward: 16 };
const HISTORY = { undo: 'historyUndo', redo: 'historyRedo' };
const kindOf = t => KINDS[t] ?? (t.startsWith('format') ? 6 : !t.startsWith('delete') ? 0 : /Backward|^deleteByCut$|^deleteContent$/.test(t) ? 13 : 14);
const STYLES = [[1, 'md-b'], [2, 'md-i'], [4, 'md-c'], [8, 'md-s'], [16, 'md-a'], [64, 'md-m']];
const CSS = `
.exact-markdown-editor { overflow: auto; white-space: pre-wrap; overflow-wrap: anywhere; outline: none; caret-color: currentColor; cursor: text; }
.exact-markdown-editor, .exact-markdown-editor * { position: relative; }
.exact-markdown-editor.md-empty > .md-line:first-child::before { content: attr(data-placeholder); position: absolute; pointer-events: none; color: var(--exact-placeholder-color, light-dark(#3c3c4380, #ebebf580)); }
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
.md-quote { --q: 1; --bar: light-dark(#d4d4d8, #52525b); color: light-dark(#6b6b70, #a1a1aa); border-inline-start: calc(var(--q) * 18px) solid transparent;
  border-image: linear-gradient(to right, var(--bar) 0 3px, transparent 3px 18px, var(--bar) 18px 21px, transparent 21px 36px, var(--bar) 36px 39px, transparent 39px calc(100% - 39px),
    var(--bar) calc(100% - 39px) calc(100% - 36px), transparent calc(100% - 36px) calc(100% - 21px), var(--bar) calc(100% - 21px) calc(100% - 18px), transparent calc(100% - 18px) calc(100% - 3px), var(--bar) calc(100% - 3px)) 0 18; }
.md-quote[data-quote="2"] { --q: 2; border-image-slice: 0 36; } .md-quote[data-quote="3"] { --q: 3; border-image-slice: 0 54; }
.md-list { --d: 0; padding-inline-start: calc(28px + var(--d) * 24px); }
.md-list[data-depth="1"] { --d: 1; } .md-list[data-depth="2"] { --d: 2; } .md-list[data-depth="3"] { --d: 3; }
.md-bullet::before, .md-number::before, .md-task::before { position: absolute; inset-inline-start: calc(var(--d, 0) * 24px); width: 22px; text-align: center; }
.md-bullet::before { content: "•"; font-weight: 700; } .md-list[data-depth="1"].md-bullet::before { content: "◦"; }
.md-number::before { content: attr(data-num); opacity: .6; text-align: end; font-variant-numeric: tabular-nums; }
.md-task::before { content: ""; top: .3em; margin-inline-start: 3px; width: 13px; height: 13px; border: 1.5px solid currentColor; border-radius: 4px; cursor: pointer; }
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

/** A line's class, attributes and segments, from its `mde_view` record and
 * its atoms' faces (`atoms`, in line order: each segment of an atom's hidden
 * source is drawn after its island, LLP 1045.000 §4). */
function describe(v, at, line, atoms = []) {
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
    const style = v[o + 2], hidden = style & 128, atom = atoms.find(a => a.at === v[o]);
    if (!hidden || atom) empty = false;
    segs.push([line.slice(v[o], v[o + 1]), hidden ? 'md-hide' : STYLES.filter(([bit]) => style & bit).map(([, c]) => c).join(' '), atom]);
  }
  // A line that ends with an atom keeps a break after it, where a caret can stand.
  return { cls: cls.join(' '), attrs, segs, empty: empty || segs.at(-1)?.[2] !== undefined };
}
/** A line's atoms from its `mde_view` record: where each starts in the line, and its face. */
function lineAtoms(v, at, faces, styles) {
  const count = v[at + 9], o = at + 10 + count * 3, n = v[o], out = [];
  for (let k = 0; k < n; k++) {
    const [s, e, , style] = v.subarray(o + 1 + k * 4, o + 5 + k * 4), face = faces.shift() ?? {};
    out.push({ ...face, at: s, len: e - s, style, css: styles[face.kind] ?? '', key: JSON.stringify([face, style, styles[face.kind] ?? '']) });
  }
  return out;
}
/** An island: the atom's pill, in the formats around it, contributing no source. */
function island(a) {
  const box = atomBox(a, a.css, true);
  box.dataset.atomAt = a.at; box.dataset.atomKey = a.key;
  if (a.style & 1) box.style.fontWeight = '700'; if (a.style & 2) box.style.fontStyle = 'italic'; if (a.style & 8) box.style.textDecoration = 'line-through';
  return box;
}
const isIsland = n => n?.nodeType === 1 && n.classList.contains('exact-atom');
/** An atom's hidden source whose island an edit removed: its atom goes with it. */
const orphanSource = n => n?.nodeType === 1 && n.exactIsland !== undefined && n.previousSibling !== n.exactIsland;
function build(d) {
  const line = document.createElement('div');
  line.className = d.cls;
  for (const [k, value] of d.attrs) line.setAttribute(k, value);
  for (const [t, c, atom] of d.segs) {
    if (!c) { line.append(t); continue; }
    const span = document.createElement('span'); span.className = c; span.textContent = t;
    if (atom) { const box = island(atom); line.append(box); span.exactIsland = box; }
    line.append(span);
  }
  if (d.empty) line.append(document.createElement('br'));
  return line;
}
/** Whether a line's DOM already is `d` — as it is after most native keystrokes.
 * Its `dir` and placeholder are set in place (`direct`, `showPlaceholder`), so
 * they are no part of `d`. */
function matches(line, d) {
  const own = 1 + line.hasAttribute('dir') + line.hasAttribute('data-placeholder');
  if (line.className !== d.cls || line.attributes.length !== own + d.attrs.length || d.attrs.some(([k, value]) => line.getAttribute(k) !== value)) return false;
  const kids = line.childNodes, boxes = d.segs.filter(s => s[2]).length;
  if (kids.length !== d.segs.length + boxes + (d.empty ? 1 : 0)) return false;
  for (let i = 0, j = 0; i < d.segs.length; i++, j++) {
    const [t, c, atom] = d.segs[i];
    if (atom && !(isIsland(kids[j]) && kids[j].dataset.atomKey === atom.key && kids[j + 1]?.exactIsland === kids[j++])) return false;
    const k = kids[j];
    if (c ? !(k.nodeName === 'SPAN' && k.className === c && k.attributes.length === 1 && k.childNodes.length === 1 && k.firstChild.nodeType === 3 && k.firstChild.data === t)
      : !(k.nodeType === 3 && k.data === t)) return false;
  }
  return !d.empty || kids[kids.length - 1].nodeName === 'BR';
}

/** One state's words as the editor keeps them with an undo step: the line
 * count and the most common word, then each line that holds another, `-`
 * for none (`3 ltr 1=rtl`). */
function packWords(ws) {
  const tally = new Map();
  let common, most = 0;
  for (const w of ws) { const n = (tally.get(w) ?? 0) + 1; tally.set(w, n); if (n > most) { common = w; most = n; } }
  let out = `${ws.length} ${common ?? '-'}`;
  ws.forEach((w, i) => { if (w !== common) out += ` ${i}=${w ?? '-'}`; });
  return out;
}
/** `packWords`' words for `count` lines; null when they were for another count. */
function unpackWords(s, count) {
  const [n, common, ...others] = s.split(' ');
  if (Number(n) !== count) return null;
  const word = w => w === '-' ? undefined : w, ws = new Array(count).fill(word(common));
  for (const o of others) { const at = o.indexOf('='); ws[Number(o.slice(0, at))] = word(o.slice(at + 1)); }
  return ws;
}
/** A line's inline-start border and padding, where its text starts. */
const gutter = css => (parseFloat(css.borderInlineStartWidth) || 0) + (parseFloat(css.paddingInlineStart) || 0);

// Replacing the textarea happens only once this module and its wasm are
// ready; until then it remains an ordinary, usable source editor.
function installMarkupEditor(textarea, host) {
  const el = document.createElement('div');
  for (const attr of textarea.attributes) el.setAttribute(attr.name, attr.value);
  el.classList.add('exact-markdown-editor');
  const h = wasm.mde_new(), defaults = new Set(), ours = ['contenteditable', 'role', 'aria-multiline', 'aria-readonly', 'aria-disabled', 'aria-placeholder'];
  let source = '', lines = [], records = [], starts = [0], index = new WeakMap(), payload = '', placed = null;
  let destroyed = false, composing = false, notifying = false, pointer = false, pendingValue, pendingSync = false;
  let nativeHidden = null, direction = 'none', told = null, reported = '', moving = false, reread = false, atomStyles = {}, atomed = null;
  // The app's `paragraphDirs` words; whether they still wait to be placed by
  // line number; each line's text and word as drawn; and the drawn words as
  // the editor last kept them for its undo steps (LLP 1045 D11).
  let words = [], wordsSaid = null, placeWords = true, drawnTexts = [], drawnWords = [], heldWords = '';
  // Each line's direction under `auto` and what it was decided for (`mde_auto`).
  let autoFor = null, autoDirs = [];
  let tabIndex = el.getAttribute('tabindex');
  const writable = () => !el.hasAttribute('disabled') && !el.hasAttribute('readonly') && !inertAncestor(el);
  const now = () => performance.now();
  const sel = () => [wasm.mde_sel(h, 0), wasm.mde_sel(h, 1)];

  // Observe native edits, including IME and spelling; never infer their result
  // from beforeinput data. Renderer mutations are drained after each render.
  let mutations = [];
  const observer = new MutationObserver(batch => mutations.push(...batch));
  observer.observe(el, { subtree: true, childList: true, characterData: true });
  function render() {
    // Native replacement/composition can detach, merge or nest our lines.
    // A record is reusable only while its node occupies the recorded place.
    if (el.childNodes.length !== lines.length || lines.some((line, i) => el.childNodes[i] !== line)) {
      records = []; lines = []; el.replaceChildren();
    }
    const n = wasm.mde_view(h), v = new Uint32Array(wasm.memory.buffer, wasm.mde_out(), n).slice();
    const next = [], found = [], faces = atomFaces();
    starts = [];
    for (let i = 0, at = 1; i < v[0]; i++) {
      const line = source.slice(v[at], v[at + 1]), segments = at + 10 + v[at + 9] * 3, end = segments + 1 + v[segments] * 4;
      const atoms = v[segments] ? lineAtoms(v, at, faces, atomStyles) : [];
      next.push({ line, view: v.subarray(at + 2, end), atoms: atoms.map(a => a.key).join('\n') });
      found.push([at, line, atoms]); starts.push(v[at]); at = end;
    }
    const same = (a, b) => a.line === b.line && a.atoms === b.atoms && a.view.length === b.view.length && a.view.every((value, i) => value === b.view[i]);
    let head = 0, tail = 0;
    while (head < records.length && head < next.length && same(records[head], next[head])) head++;
    while (tail < records.length - head && tail < next.length - head && same(records[records.length - 1 - tail], next[next.length - 1 - tail])) tail++;
    const removed = lines.splice(head, records.length - head - tail);
    const fresh = found.slice(head, next.length - tail).map(([at, line, atoms]) => describe(v, at, line, atoms));
    // A native keystroke usually leaves the DOM exactly as it would be drawn:
    // keep those nodes, so the input method's autocorrect state survives.
    const kept = removed.length === fresh.length && removed.every((line, i) => matches(line, fresh[i]));
    const added = kept ? removed.splice(0) : fresh.map(build);
    const before = lines[head] ?? null;
    for (const line of removed) line.remove();
    for (const line of added) if (!line.isConnected) el.insertBefore(line, before);
    lines.splice(head, 0, ...added);
    records = next;
    mutations = []; observer.takeRecords();
    carry(next.map(r => r.line));
    lines.forEach((line, i) => { index.set(line, i); direct(line, i); });
    el.classList.toggle('md-empty', source === '');
    showPlaceholder();
  }
  function read() { source = utf16.decode(new Uint16Array(wasm.memory.buffer, wasm.mde_source(h), wasm.mde_source_len(h))); }
  /** Each atom's kind, label, icon, title, role, destination, source and its face's paint, in source order (`mde_atom_faces`). */
  function atomFaces() {
    const out = [], lines = text(wasm.mde_atom_faces(h)).split('\n');
    for (let k = 0; k + 9 < lines.length; k += 10) { const [kind, label, icon, title, role, href, src, color, backgroundColor] = lines.slice(k, k + 9); out.push({ kind, label, icon, title, role, href, source: src, color, backgroundColor }); }
    return out;
  }
  /** The field's `atoms` and `atomFaces` into the editor (LLP 1045.000 D2), and its kinds' paint; a change redraws. */
  function atoms() {
    const kinds = el.getAttribute('data-atoms') ?? '', faces = el.getAttribute('data-atom-faces') ?? '', styles = el.getAttribute('data-atom-styles') ?? '';
    if (atomed?.[0] === kinds && atomed[1] === faces && atomed[2] === styles) return 0;
    atomed = [kinds, faces, styles];
    try { atomStyles = JSON.parse(styles || '{}'); } catch { atomStyles = {}; }
    put(kinds + faces);
    return wasm.mde_atoms(h, kinds.length, faces.length) | SOURCE;
  }
  /** The field's `paragraphDirs` words (LLP 1045 D11); whether they changed. */
  function directions() {
    const said = el.getAttribute('data-paragraph-dirs') ?? '';
    if (said === wordsSaid) return false;
    wordsSaid = said; words = said.split(/[\t\n\f\r ]+/).filter(Boolean); placeWords = true;
    return true;
  }
  /** Each line's word as drawn: new words by line number, a line past the
   * last word taking the last; then each word stays with its line through
   * edits, and a line an edit adds takes the word of the line above it (at
   * the top, below it) until the app's next words. Each undo step keeps the
   * words of the state it returns to, which undo and redo give back
   * (`mde_words_back`); the app's new words forget them. */
  function carry(texts) {
    // An undo or redo gave back the step's words (their length plus one; one
    // for a step whose words the app's new words forgot, which carries).
    const back = !placeWords && words.length ? wasm.mde_words_back(h) : 0;
    const restored = back > 1 ? unpackWords(text(back - 1), texts.length) : null;
    if (placeWords) {
      placeWords = false;
      drawnWords = texts.map((_, i) => words[Math.min(i, words.length - 1)]);
      wasm.mde_forget_words(h); heldWords = '';
    } else if (restored) {
      drawnWords = restored;
    } else {
      const old = drawnTexts, was = drawnWords;
      let head = 0, tail = 0;
      while (head < old.length && head < texts.length && old[head] === texts[head]) head++;
      while (tail < old.length - head && tail < texts.length - head && old[old.length - 1 - tail] === texts[texts.length - 1 - tail]) tail++;
      const kept = old.length - head - tail, next = was.slice(0, head);
      for (let k = 0; k < texts.length - head - tail; k++) next.push(k < kept ? was[head + k] : next[head + k - 1] ?? was[head + kept]);
      drawnWords = next.concat(was.slice(was.length - tail));
    }
    drawnTexts = texts;
    // The words of this state, which the next undo step keeps; after an undo
    // or redo the editor holds the step's, so they are set again.
    const packed = words.length ? packWords(drawnWords) : '';
    if (packed !== heldWords || back) { wasm.mde_set_words(h, put(packed)); heldWords = packed; }
  }
  /** New words from the app, by line number, once the batch that brought them
   * has drawn its value too (a render in between places them itself). */
  function placeNewWords() {
    if (destroyed || !placeWords) return;
    carry(drawnTexts);
    lines.forEach((line, i) => direct(line, i));
  }
  /** Line `i`'s `dir` from its word: `ltr` or `rtl`, or for `auto` its first
   * strong letter outside syntax and atoms, as the editor crate decides it for
   * Apple too (`mde_auto`). With none it inherits the field's. */
  function direct(line, i) {
    const word = drawnWords[i];
    let dir = word === 'ltr' || word === 'rtl' ? word : null;
    if (word === 'auto') {
      if (autoFor?.[0] !== source || autoFor[1] !== atomed) {
        const n = wasm.mde_auto(h);
        autoDirs = new Uint32Array(wasm.memory.buffer, wasm.mde_out(), n).slice();
        autoFor = [source, atomed];
      }
      dir = autoDirs[i] === 2 ? 'rtl' : autoDirs[i] === 1 ? 'ltr' : null;
    }
    if (dir) { if (line.getAttribute('dir') !== dir) line.setAttribute('dir', dir); } else if (line.hasAttribute('dir')) line.removeAttribute('dir');
  }
  /** An empty editor's placeholder, on its line, so it takes that line's
   * direction, as UIKit draws it in the first line's paragraph style. */
  function showPlaceholder() {
    const line = lines[0], text = source === '' ? el.getAttribute('placeholder') ?? '' : '';
    if (!line) return;
    if (text) { if (line.getAttribute('data-placeholder') !== text) line.setAttribute('data-placeholder', text); }
    else line.removeAttribute('data-placeholder');
  }

  /** The source offset of a DOM point inside the editor. An island adds no
   * source: the point before it is its atom's start; one after it, or in its
   * hidden source, the atom's end (LLP 1045.000 §4). */
  function toSource(node, offset) {
    if (node === el) return offset >= lines.length ? source.length : starts[offset] ?? 0;
    const line = (node.nodeType === 3 ? node.parentElement : node)?.closest('.md-line');
    if (!line || !index.has(line)) return sel()[0];
    let at = 0;
    for (const [i, kid] of [...line.childNodes].entries()) {
      const len = isIsland(kid) || kid.nodeName === 'BR' ? 0 : kid.textContent.length;
      if (node === line && offset === i) return starts[index.get(line)] + at + (isIsland(line.childNodes[i - 1]) ? len : 0);
      if (kid === node || kid.contains(node)) {
        if (isIsland(kid)) return starts[index.get(line)] + at + (kid.nextSibling?.textContent.length ?? 0);
        if (kid.exactIsland) return starts[index.get(line)] + at + len;
        const r = document.createRange(); r.setStart(kid, 0); r.setEnd(node, offset);
        return starts[index.get(line)] + at + r.toString().length;
      }
      at += len;
    }
    return starts[index.get(line)] + at;
  }
  /** A DOM point for a source offset, on visible text left or right of hidden syntax. */
  function toDom(p, right) {
    let lo = 0, hi = starts.length - 1;
    while (lo < hi) { const mid = (lo + hi + 1) >> 1; if (starts[mid] <= p) lo = mid; else hi = mid - 1; }
    const line = lines[lo];
    if (!line) return { node: el, offset: 0 };
    const local = p - starts[lo], found = [], edges = [];
    let acc = 0;
    for (const [i, kid] of [...line.childNodes].entries()) {
      if (isIsland(kid)) { if (acc === local) edges.push({ node: line, offset: i }); continue; }
      const t = kid.nodeType === 3 ? kid : kid.firstChild?.nodeType === 3 ? kid.firstChild : null, len = kid.nodeName === 'BR' ? 0 : kid.textContent.length;
      if (kid.exactIsland && acc + len === local) edges.push({ node: line, offset: i + 1 });
      else if (t && !kid.classList?.contains('md-hide') && acc <= local && local <= acc + len) found.push({ node: t, offset: local - acc, start: local === acc, end: local === acc + len });
      acc += len;
    }
    if (found.length) return right ? found.find(c => !c.end) ?? found.at(-1) : found.find(c => !c.start) ?? found[0];
    if (edges.length) return edges[0];
    const br = line.querySelector(':scope > br');
    return { node: line, offset: br ? [...line.childNodes].indexOf(br) : line.childNodes.length };
  }
  function domSelection() {
    const s = document.getSelection();
    if (!s?.rangeCount || !el.contains(s.anchorNode) || !el.contains(s.focusNode)) return null;
    const a = toSource(s.anchorNode, s.anchorOffset), f = toSource(s.focusNode, s.focusOffset);
    const from = Math.min(a, f), to = Math.max(a, f), way = f < a ? 'backward' : 'forward';
    // The browser's Edit menu and mobile Select All may select only visible
    // text. A selection spanning that whole document owns hidden syntax too.
    if (from !== to) {
      const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
      const visible = t => t.data.length && !t.parentElement.closest('.md-hide, .md-collapsed, .exact-atom');
      let first = walker.nextNode();
      while (first && !visible(first)) first = walker.nextNode();
      walker.currentNode = el;
      let last = walker.lastChild();
      while (last && !visible(last)) last = walker.previousNode();
      if (first && from <= toSource(first, 0) && to >= toSource(last, last.data.length)) return [0, source.length, way];
    }
    return [from, to, way];
  }
  function place(reveal) {
    if (document.activeElement !== el) return;
    const [from, to] = sel(), all = from === 0 && to === source.length && from !== to;
    const a = all ? { node: el, offset: 0 } : toDom(from, wasm.mde_draws_after(h, from) === 1);
    const b = all ? { node: el, offset: el.childNodes.length } : from === to ? a : toDom(to, false);
    placed = [from, to];
    if (direction === 'backward' && from !== to) document.getSelection().setBaseAndExtent(b.node, b.offset, a.node, a.offset);
    else document.getSelection().setBaseAndExtent(a.node, a.offset, b.node, b.offset);
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
  /** The app's `input`: the edit's own type and data, the source and selection it left (`edit`: an InputEvent's init). */
  function notify(edit) {
    notifying = true;
    try { el.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: edit.inputType ?? '', data: edit.data ?? null, isComposing: !!edit.isComposing })); }
    finally { notifying = false; }
    if (pendingValue !== undefined || pendingSync) queueMicrotask(flush);
    moved();
  }
  /** Show what an editing call did: redraw, place the caret, tell the app. */
  function after(bits, edit = {}) {
    if (destroyed) return bits;
    if (bits & SOURCE) { read(); render(); }
    if (bits & PLACE) { direction = 'none'; place(true); }
    if (bits & SOURCE) notify(edit);
    emit(); moved();
    return bits;
  }
  function run(command, argument = '') {
    if (destroyed || !writable() || composing) return false;
    const n = put(command + argument);
    after(wasm.mde_command(h, command.length, n - command.length, now()), { inputType: HISTORY[command] });
    return true;
  }
  /** HTML's `selectionchange` at the field, once per task, when its source selection moved. */
  function moved() {
    if (moving || destroyed) return;
    moving = true;
    queueMicrotask(() => {
      moving = false;
      const now = [el.selectionStart, el.selectionEnd, el.selectionDirection].join();
      if (destroyed || now === reported || !host.live(el)) return;
      reported = now; el.dispatchEvent(new Event('selectionchange'));
    });
  }
  /** The text and selection in the DOM, hidden syntax included, as a native edit left them; `skip` leaves nodes out. */
  function readDom(skip) {
    const points = new Map();
    let value = '';
    function walk(node) {
      if (node.nodeType === 3) { points.set(node, value.length); value += node.data; return; }
      const offsets = [value.length], kids = [...node.childNodes].filter(child => !skip?.(child) && !isIsland(child) && !orphanSource(child));
      for (let i = 0; i < kids.length; i++) {
        const child = kids[i], block = n => n?.nodeType === 1 && /^(DIV|P)$/.test(n.nodeName);
        if (i && (block(child) || block(kids[i - 1]))) value += '\n';
        offsets[i] = value.length;
        if (child.nodeName === 'BR') {
          points.set(child, [value.length]);
          if (i < kids.length - 1) value += '\n'; // a final BR is the caret placeholder
        } else walk(child);
        offsets[i + 1] = value.length;
      }
      points.set(node, offsets);
    }
    walk(el);
    const s = document.getSelection(), offset = (node, at) => {
      const p = points.get(node);
      return typeof p === 'number' ? p + at : p?.[at] ?? NONE;
    };
    return { value, a: offset(s?.anchorNode, s?.anchorOffset), b: offset(s?.focusNode, s?.focusOffset) };
  }
  /** `selectionStart`, `selectionEnd`, `selectionDirection` in source offsets; while an input method composes, the DOM's. */
  function selection() {
    if (composing) {
      const { a, b } = readDom();
      if (a !== NONE && b !== NONE) return [Math.min(a, b), Math.max(a, b), a === b ? 'none' : b < a ? 'backward' : 'forward'];
    }
    const [a, b] = sel();
    return [a, b, a === b ? 'none' : direction];
  }
  function reconcile(edit) {
    if (destroyed || composing) return 0;
    const s = domSelection(), changes = mutations.concat(observer.takeRecords());
    mutations = [];
    const hidden = nativeHidden; nativeHidden = null;
    let first = lines.length, last = -1, structural = false;
    for (const change of changes) {
      const node = change.target.nodeType === 3 ? change.target.parentElement : change.target;
      const line = node?.closest('.md-line'), i = index.get(line);
      if (i === undefined || line.parentNode !== el || line.querySelector('div, p, br:not(:last-child)') || node.closest?.('.exact-atom')) { structural = true; break; }
      first = Math.min(first, i); last = Math.max(last, i);
    }
    if (structural || last < 0 || hidden || reread) {
      reread = false;
      // Read what survived, including hidden syntax, not detached cached nodes.
      // Native editors may create nested blocks and BRs during composition.
      let { value, a, b } = readDom();
      // The native editor may retain display:none syntax inside a selection
      // it replaced. Drop only unchanged, selected hidden nodes from before
      // this edit; newly typed syntax and a cancelled composition survive.
      if (hidden && changes.length && value !== source) ({ value, a, b } = readDom(child => hidden.get(child) === child.textContent));
      const n = put(value), bits = wasm.mde_reconcile(h, n, Math.min(a, b), Math.max(a, b), now());
      // Even an unchanged source may have a new native DOM shape.
      records = []; lines = []; el.replaceChildren();
      if (!(bits & SOURCE)) { render(); place(false); }
      return after(bits, edit);
    }
    const from = starts[first], to = last + 1 < starts.length ? starts[last + 1] - 1 : source.length;
    // A line's text is its source: no island's label, and no hidden source an island left with.
    const lineText = line => [...line.childNodes].map(k => isIsland(k) || orphanSource(k) || k.nodeName === 'BR' ? '' : k.textContent).join('');
    const n = put(lines.slice(first, last + 1).map(lineText).join('\n'));
    return after(wasm.mde_reconcile_range(h, from, to, n, s ? s[0] : NONE, s ? s[1] : NONE, now()), edit);
  }
  function rememberNativeSelection() {
    const [from, to] = sel();
    nativeHidden = from === 0 && to === source.length && from !== to
      ? new Map([...el.querySelectorAll('.md-hide, .md-collapsed')].map(node => [node, node.textContent])) : null;
  }
  function selectionChanged() {
    if (destroyed || composing || !host.live(el)) return;
    const s = domSelection();
    if (!s) return;
    const echo = placed; placed = null;
    const answer = wasm.mde_select(h, s[0], s[1], echo ? echo[0] : NONE, echo ? echo[1] : NONE);
    if (answer) direction = s[2];
    // Mid-drag the platform owns the selection; snap it when the drag ends.
    if (answer === 2 && !pointer) place(false);
    if (answer) emit();
    moved();
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
  // ⌘⇧C: the copy range's plain text, each atom the editor recognized in the whole source its label.
  const copyPlain = () => {
    if (!wasm.mde_copy(h)) return;
    const [a, b] = new Uint32Array(wasm.memory.buffer, wasm.mde_out(), 2);
    navigator.clipboard?.writeText(text(wasm.mde_plain(h, a, b))).catch(error => console.warn('exact: plain copy failed', error));
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
    if (el.getAttribute('markup') === 'markdown') { configure(); showPlaceholder(); if (directions()) queueMicrotask(placeNewWords); const bits = atoms(); if (bits & SOURCE) { render(); if (bits & PLACE) place(false); emit(); } return; }
    // Markdown off: the same source in a plain textarea, the explicit source mode.
    const native = document.createElement('textarea'), [from, to] = sel(), focused = document.activeElement === el;
    for (const attr of el.attributes) if (!ours.includes(attr.name) && !defaults.has(attr.name)) native.setAttribute(attr.name, attr.value);
    if (tabIndex !== null) native.setAttribute('tabindex', tabIndex);
    native.classList.remove('exact-markdown-editor', 'md-empty');
    if (!native.classList.length) native.removeAttribute('class');
    native.value = source; native.exactSourceValue = source;
    native.setSelectionRange(from, to, direction);
    host.replace(native); destroy(); el.replaceWith(native);
    if (focused && !native.disabled) native.focus();
  }
  function destroy() {
    if (destroyed) return;
    destroyed = true;
    document.removeEventListener('selectionchange', selectionChanged);
    window.removeEventListener('pointerup', release); window.removeEventListener('pointercancel', release);
    observer.disconnect();
    wasm.mde_drop(h);
  }

  // selectionchange is asynchronous; a select-all immediately followed by
  // typing must reach the shared editor, and the app's `beforeinput` the
  // selection it applies to, before either decides (capture runs first).
  el.addEventListener('beforeinput', e => { if (!destroyed && !composing && !e.isComposing) selectionChanged(); }, true);
  /** The editor's own handling, after the app's `beforeinput` (installed after `host.replace`): one it prevented changes nothing. */
  function beforeInput(e) {
    if (destroyed || composing || e.defaultPrevented || e.isComposing || e.inputType === 'insertCompositionText') return;
    if (!writable()) { e.preventDefault(); return; }
    const kind = kindOf(e.inputType);
    if (!kind) { rememberNativeSelection(); return; }
    let a = NONE, b = NONE;
    const target = (kind === 9 || kind >= 11) && e.getTargetRanges?.()[0];
    if (target) { const x = toSource(target.startContainer, target.startOffset), y = toSource(target.endContainer, target.endOffset); a = Math.min(x, y); b = Math.max(x, y); }
    const dt = e.dataTransfer, data = e.data ?? (dt && (dt.getData('text/markdown') || dt.getData('text/plain'))) ?? '';
    const bits = wasm.mde_before_input(h, kind, put(data), a, b, now());
    if (!(bits & HANDLED)) { rememberNativeSelection(); return; } // native typing, read back on input
    e.preventDefault();
    after(bits, { inputType: e.inputType, data: e.data });
  }
  // The app hears this editor's own input events only, carrying the source;
  // while an input method composes, the DOM's text and selection (`selection`).
  el.addEventListener('input', e => {
    if (notifying) return;
    e.stopImmediatePropagation();
    const edit = { inputType: e.inputType, data: e.data };
    if (!e.isComposing) reconcile(edit);
    else if (composing) { told = el.value; notify({ ...edit, isComposing: true }); }
  });
  // Over a range that holds an atom, the range goes first, its own undo step; the platform composes over it (LLP 1045.000 D5).
  el.addEventListener('compositionstart', () => { selectionChanged(); rememberNativeSelection(); if (wasm.mde_compose(h, now()) & SOURCE) { read(); reread = true; } composing = true; });
  // A composition the app heard and that left the source as it was (cancelled) is heard too: its text went.
  el.addEventListener('compositionend', e => {
    composing = false;
    const heard = told, edit = { inputType: 'insertCompositionText', data: e.data };
    told = null;
    queueMicrotask(() => { if (!(reconcile(edit) & SOURCE) && heard !== null && heard !== source && !destroyed) notify(edit); flush(); });
  });
  el.addEventListener('keydown', e => {
    if (e.isComposing || destroyed || e.defaultPrevented || e.exactStopped || e.$stopped) return;
    const mod = e.metaKey || e.ctrlKey, key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
    let command = null;
    if (mod && !e.altKey) {
      if (key === 'a' && !e.shiftKey) {
        if (el.hasAttribute('disabled')) return;
        e.preventDefault(); wasm.mde_select_all(h); place(false); emit(); return;
      }
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
    // Undo and Redo are the platform's `beforeinput` first, the app's to cancel; the editor applies it (`beforeInput`).
    if (HISTORY[command]) el.dispatchEvent(new InputEvent('beforeinput', { bubbles: true, cancelable: true, inputType: HISTORY[command] }));
    else run(command);
  });
  const clip = cut => e => {
    const markdown = copied();
    if (markdown === null) return;
    e.preventDefault();
    // Copy keeps the literal source (LLP 1045 D1); Mod-Shift-C copies plain text.
    e.clipboardData.setData('text/plain', markdown);
    e.clipboardData.setData('text/markdown', markdown);
    if (cut && writable()) after(wasm.mde_cut(h, now()), { inputType: 'deleteByCut' });
  };
  el.addEventListener('copy', clip(false));
  el.addEventListener('cut', clip(true));
  /** Whether `x` is in a line's marker gutter: its inline-start border (a
   * quote's bars) and padding, the right in a right-to-left line. */
  const inGutter = (line, x) => {
    const box = line.getBoundingClientRect(), css = getComputedStyle(line), start = gutter(css);
    return (css.direction === 'rtl' ? box.right - x : x - box.left) <= start;
  };
  // A press on an island selects its atom, then is the field's `atompress` (LLP 1045.000 D5, D7).
  const pressed = e => { const box = e.target.closest?.('.exact-atom'); return box && el.contains(box) && index.has(box.parentNode) ? islands().find(i => i.box === box) : null; };
  // A disabled or inert field presses no atom; a read-only one presses but selects nothing.
  const pressable = () => !el.hasAttribute('disabled') && !inertAncestor(el);
  el.addEventListener('pointerdown', e => {
    const atom = e.button === 0 && pressed(e);
    if (atom) { e.preventDefault(); if (!pressable() || !writable()) return; if (document.activeElement !== el) HTMLElement.prototype.focus.call(el, { preventScroll: true }); direction = 'none'; after(wasm.mde_set_selection(h, atom.s, atom.e)); return; }
    const line = e.target.closest?.('.md-task');
    if (line && el.contains(line) && writable() && inGutter(line, e.clientX)) {
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
  el.addEventListener('click', e => { const atom = pressed(e); if (!atom || destroyed || !pressable()) return; const d = atom.box.dataset; el.dispatchEvent(new CustomEvent('atompress', { detail: [d.exactAtom, d.atomSource, d.atomHref ?? '', atom.s, atom.e, e.shiftKey, e.ctrlKey, e.altKey, e.metaKey] })); });
  el.addEventListener('dblclick', e => { if (pressed(e)) e.preventDefault(); });
  document.addEventListener('selectionchange', selectionChanged);

  const raw = textarea.exactSourceValue;
  wasm.mde_load(h, put(raw?.replace(/\r\n?/g, '\n') === textarea.value ? raw : textarea.value));
  atoms(); directions(); read(); render();
  wasm.mde_select(h, textarea.selectionStart, textarea.selectionEnd, NONE, NONE);
  Object.defineProperties(el, {
    value: { get: () => composing ? readDom().value : source, set: setValue },
    selectionStart: { get: () => selection()[0] },
    selectionEnd: { get: () => selection()[1] },
    selectionDirection: { get: () => selection()[2] },
  });
  reported = selection().join();
  // HTML's range methods over source offsets (LLP 1045 D6): offsets are
  // WebIDL `unsigned long`s, clamped to the source; a caret in hidden syntax
  // is drawn at its visible edge, so it reads back there.
  const offset = v => Math.min(source.length, Number(v) >>> 0);
  el.setSelectionRange = (start, end, way) => {
    if (destroyed) return;
    if (composing) throw new DOMException('text is being composed', 'InvalidStateError');
    const b = offset(end), a = Math.min(offset(start), b);
    after(wasm.mde_set_selection(h, a, b));
    direction = way === 'forward' || way === 'backward' ? way : 'none';
    if (direction === 'backward') place(false);
    moved();
  };
  // One undo step for each call, as a format command is (LLP 1045 D1), and
  // the one `insertReplacementText` input a plain field's call makes.
  el.setRangeText = (text, ...range) => {
    if (destroyed) return;
    if (composing) throw new DOMException('text is being composed', 'InvalidStateError');
    text = String(text);
    let [a, b] = sel();
    if (range.length) {
      if (range.length < 2) throw new TypeError('setRangeText takes a start and an end');
      [a, b] = [Number(range[0]) >>> 0, Number(range[1]) >>> 0];
      if (a > b) throw new DOMException('start exceeds end', 'IndexSizeError');
    }
    const mode = ['preserve', 'select', 'start', 'end'].indexOf(range[2] === undefined ? 'preserve' : String(range[2]));
    if (mode < 0) throw new TypeError(`setRangeText: ${range[2]} is not a selectionMode`);
    direction = 'none';
    after(wasm.mde_replace(h, a, b, put(text), mode, now()) | SOURCE, { inputType: 'insertReplacementText', data: text });
  };
  el.focus = options => { if (el.hasAttribute('disabled')) return; HTMLElement.prototype.focus.call(el, options); place(false); };
  el.select = () => { if (el.hasAttribute('disabled')) return; wasm.mde_select_all(h); place(false); emit(); };
  // `rangeFrame` (LLP 1114.007) over source offsets. Collapsed, the caret's
  // box at the visible text nearest the offset (an offset in hidden syntax
  // sits at its edge). Wider, the union of the visible text it covers and of
  // the caret either side of each line break in it; hidden syntax adds none.
  const visible = t => t.data.length && !t.parentElement.closest('.md-hide, .md-collapsed, .exact-atom');
  /** Each island with its atom's source range. */
  const islands = () => [...el.querySelectorAll('.md-line > .exact-atom')].map(box => { const s = starts[index.get(box.parentNode)] + Number(box.dataset.atomAt); return { box, s, e: s + (box.nextSibling?.textContent.length ?? 0) }; });
  function caret(p) {
    let lo = 0, hi = starts.length - 1;
    while (lo < hi) { const mid = (lo + hi + 1) >> 1; if (starts[mid] <= p) lo = mid; else hi = mid - 1; }
    const line = lines[lo];
    // A line not drawn (a collapsed fence's) has no caret.
    if (!line || !line.getClientRects().length) return null;
    // At an atom's edge, its island's (LLP 1045.000 D7).
    // Its start is the line's start side: the right in a right-to-left line.
    for (const { box, s, e } of islands()) if (box.parentNode === line && (p === s || p === e)) { const b = box.getBoundingClientRect(), rtl = getComputedStyle(line).direction === 'rtl'; return new DOMRect((p === s) !== rtl ? b.left : b.right, b.top, 0, b.height); }
    const walker = document.createTreeWalker(line, NodeFilter.SHOW_TEXT), r = document.createRange();
    // The visible text holding `p`, else the nearer of the text either side.
    let at = null, before = null, after = null;
    for (let t = walker.nextNode(); t && !at; t = walker.nextNode()) {
      if (!visible(t)) continue;
      const s = toSource(t, 0), e = s + t.data.length;
      if (s <= p && p <= e) at = [t, p - s];
      else if (e < p) before = [t, t.data.length, e];
      else after ??= [t, 0, s];
    }
    at ??= before && after ? (p - before[2] <= after[2] - p ? before : after) : before ?? after;
    if (at) { r.setStart(at[0], at[1]); const box = r.getBoundingClientRect(); if (box.height) return box; }
    // A caret on an empty line has no text to measure: the line's start.
    const l = line.getBoundingClientRect(), css = getComputedStyle(line);
    return new DOMRect(css.direction === 'rtl' ? l.right - gutter(css) : l.x + gutter(css), l.y, 0, l.height);
  }
  function rangeFrame(from, to) {
    if (destroyed || composing) return null;
    if (from === to) return caret(from);
    let left = Infinity, top = Infinity, right = -Infinity, bottom = -Infinity;
    const add = b => { if (b) { left = Math.min(left, b.left); top = Math.min(top, b.top); right = Math.max(right, b.right); bottom = Math.max(bottom, b.bottom); } };
    const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
    for (let t = walker.nextNode(); t; t = walker.nextNode()) {
      if (!visible(t)) continue;
      const s = toSource(t, 0);
      if (s >= to || s + t.data.length <= from) continue;
      const r = document.createRange();
      r.setStart(t, Math.max(0, from - s)); r.setEnd(t, Math.min(t.data.length, to - s));
      for (const b of r.getClientRects()) add(b);
    }
    for (const at of starts) if (at > from && at <= to) { add(caret(at - 1)); add(caret(at)); }
    for (const { box, s, e } of islands()) if (s < to && from < e) add(box.getBoundingClientRect());
    return left === Infinity ? null : new DOMRect(left, top, right - left, bottom - top);
  }
  /** The atoms as drawn, for the agent's `state.atoms` (LLP 1045.000 D9). */
  const atomList = () => destroyed ? [] : islands().map(({ box, s, e }) => ({ kind: box.dataset.exactAtom, source: box.dataset.atomSource, start: s, end: e, label: box.getAttribute('aria-label') ?? '', box }));
  el.exactMarkup = { sync, format(command, argument = '') { if (destroyed || !writable()) return false; el.focus(); return run(command, String(argument)); }, setValue, destroy, rangeFrame, atoms: atomList };
  configure();
  const initialFocus = document.activeElement === textarea;
  textarea.replaceWith(el);
  host.replace(el);
  el.addEventListener('beforeinput', beforeInput);
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
