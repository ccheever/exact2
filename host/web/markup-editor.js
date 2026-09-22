// @ref LLP 1045 D1/D5/D6 — source text is the document; decorations never
// serialize it. Rolldown bundles this optional host module, loaded after paint.
import { Compartment, EditorSelection, EditorState, StateEffect, StateField, Transaction } from '@codemirror/state';
import { Decoration, EditorView, keymap, placeholder, WidgetType } from '@codemirror/view';
import { defaultKeymap, history, historyKeymap, isolateHistory } from '@codemirror/commands';

const refresh = StateEffect.define();
const external = Transaction.addToHistory.of(false);
const theme = EditorView.theme({
  '&': { height: '100%', font: 'inherit', color: 'inherit', background: 'transparent' },
  '&.cm-focused': { outline: 'none' },
  '.cm-scroller': { overflow: 'auto', font: 'inherit', lineHeight: 'inherit' },
  '.cm-content': { padding: '0', minHeight: '100%', caretColor: 'currentColor' },
  '.cm-line': { padding: '0', overflowWrap: 'anywhere' },
  '.cm-cursor': { borderLeftColor: 'currentColor' },
  '.cm-placeholder': { color: 'light-dark(#3c3c4380, #ebebf580)' },
  '.exact-md-marker': { opacity: '.38' },
  '.exact-md-bold': { fontWeight: '700' },
  '.exact-md-italic': { fontStyle: 'italic' },
  '.exact-md-strike': { textDecoration: 'line-through' },
  '.exact-md-link': { color: 'light-dark(#245aba, #83b3ff)', textDecoration: 'underline' },
  '.exact-md-code': { fontFamily: 'ui-monospace, monospace', background: 'light-dark(#00000009, #ffffff12)', borderRadius: '3px' },
  '.exact-md-code-line': { fontFamily: 'ui-monospace, monospace', background: 'light-dark(#00000009, #ffffff12)' },
  '.exact-md-quote': { borderLeft: '3px solid light-dark(#d1d5db, #596170)', color: 'light-dark(#556074, #b4becc)' },
  '.exact-md-footnote': { fontSize: '.75em', verticalAlign: 'super', lineHeight: '0' },
  '.exact-md-rule': { display: 'inline-block', width: '100%', borderTop: '1px solid currentColor', opacity: '.25', verticalAlign: 'middle' },
  '.exact-md-token-keyword': { color: 'light-dark(#9c36b5, #da9bff)' },
  '.exact-md-token-string': { color: 'light-dark(#267038, #a4d792)' },
  '.exact-md-token-comment': { color: 'light-dark(#687382, #8c98a8)', fontStyle: 'italic' },
  '.exact-md-token-number': { color: 'light-dark(#b45716, #efb176)' },
  '.exact-md-token-type': { color: 'light-dark(#086b83, #79d5e8)' },
});

class Marker extends WidgetType {
  constructor(kind, text) { super(); this.kind = kind; this.text = text; }
  eq(other) { return other.kind === this.kind && other.text === this.text; }
  toDOM() {
    const el = document.createElement(this.kind === 4 ? 'sup' : 'span');
    el.textContent = ['• ', '☐ ', '☑ ', '', this.text][this.kind] ?? this.text;
    if (this.kind === 3) { el.className = 'exact-md-rule'; el.setAttribute('role', 'separator'); }
    if (this.kind === 4) el.className = 'exact-md-footnote';
    return el;
  }
  ignoreEvent() { return false; }
}

function decorations(state, call, enabled) {
  if (!enabled()) return { drawn: Decoration.none, atomic: Decoration.none };
  const source = state.doc.toString(), selection = state.selection.main;
  const styled = call('style', source, selection.from, selection.to);
  const all = [], atomic = [], replaced = [];
  const valid = (a, b) => Number.isInteger(a) && Number.isInteger(b) && a >= 0 && b > a && b <= source.length;
  // Only inline replacements: never remove a line break or overlap the
  // selection. CodeMirror owns line layout, bidi, cursor mapping and IME.
  const safe = (a, b) => valid(a, b) && !source.slice(a, b).includes('\n')
    && !(selection.from <= b && a <= selection.to);
  for (const [a, b, kind, text] of styled.r ?? []) if (safe(a, b)) {
    const range = Decoration.replace({ widget: new Marker(kind, text), inclusive: false }).range(a, b);
    all.push(range); atomic.push(range); replaced.push([a, b]);
  }
  for (const [a, b] of styled.h ?? []) if (safe(a, b) && !replaced.some(([x, y]) => a < y && x < b)) {
    const range = Decoration.replace({ inclusive: false }).range(a, b);
    all.push(range); atomic.push(range);
  }
  for (const [a, b, flags, href, token] of styled.s ?? []) if (valid(a, b)) {
    const classes = [[1, 'bold'], [2, 'italic'], [4, 'code'], [8, 'strike'], [16, 'link'], [64, 'marker']]
      .filter(([bit]) => flags & bit).map(([, name]) => `exact-md-${name}`);
    if (token) classes.push(`exact-md-token-${token}`);
    const attributes = href ? { title: href } : {};
    all.push(Decoration.mark({ class: classes.join(' '), attributes }).range(a, b));
  }
  for (const [a, b, kind, level, depth, quote] of styled.p ?? []) {
    if (a > source.length || b > source.length) continue;
    const classes = [], style = [];
    if (kind === 1) { style.push(`font-size:${[1, 1.8, 1.5, 1.25, 1.1, 1, 1][level]}em`, 'font-weight:700', 'line-height:1.3', 'padding-top:.25em', 'padding-bottom:.12em'); }
    if (kind === 7 || kind === 6 || kind === 9) classes.push('exact-md-code-line');
    if (quote) { classes.push('exact-md-quote'); style.push(`padding-left:${quote * 14}px`); }
    if (kind >= 2 && kind <= 4) style.push(`padding-left:${depth * 20}px`);
    for (let line = state.doc.lineAt(a); line.from <= b;) {
      all.push(Decoration.line({ class: classes.join(' '), attributes: { style: style.join(';') } }).range(line.from));
      if (line.to >= b || line.number === state.doc.lines) break;
      line = state.doc.line(line.number + 1);
    }
  }
  return { drawn: Decoration.set(all, true), atomic: Decoration.set(atomic, true) };
}

// Replacing the textarea happens only once the optional module is available;
// until then it remains an ordinary, usable source editor with the same value.
function installMarkupEditor(textarea, host) {
  const el = document.createElement('div');
  for (const attr of textarea.attributes) el.setAttribute(attr.name, attr.value);
  el.classList.add('exact-markdown-editor');
  const initialFocus = document.activeElement === textarea;
  let destroyed = false, composing = false, dragging = false, pendingValue, pendingSync = false, notifying = false;
  let lastSelection = '', view;
  let tabIndex = textarea.getAttribute('tabindex') ?? '0';
  const inputDefaults = { spellcheck: String(textarea.spellcheck), autocorrect: 'on', autocapitalize: textarea.autocapitalize || 'sentences', writingsuggestions: 'true' };
  const enabled = () => el.getAttribute('markup') === 'markdown';
  const frozen = () => composing || dragging || view?.compositionStarted;
  const writable = () => !el.hasAttribute('disabled') && !el.hasAttribute('readonly') && !el.closest('[inert]');
  const config = new Compartment();
  const styled = StateField.define({
    create: state => decorations(state, host.call, enabled),
    update(value, tr) {
      if (frozen()) return tr.docChanged ? { drawn: value.drawn.map(tr.changes), atomic: value.atomic.map(tr.changes) } : value;
      return tr.docChanged || tr.selection || tr.effects.some(e => e.is(refresh)) ? decorations(tr.state, host.call, enabled) : value;
    },
    provide: field => [EditorView.decorations.from(field, value => value.drawn), EditorView.atomicRanges.of(view => view.state.field(field).atomic)],
  });
  const emitSelection = () => {
    if (destroyed || frozen() || !view || !host.live(el)) return;
    const sel = view.state.selection.main;
    const result = host.call('selection', view.state.doc.toString(), sel.from, sel.to);
    const payload = [result.formats, result.mixed ? '1' : '0', result.unavailable, result.link].join('\n');
    if (payload !== lastSelection) { lastSelection = payload; host.select(el, payload); }
  };
  const emit = update => {
    if (destroyed || !host.live(el)) return;
    // The CM transaction is complete before app write-back re-enters here.
    if (update.docChanged && update.transactions.some(tr => tr.annotation(Transaction.userEvent))) {
      notifying = true;
      try { el.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText' })); }
      finally { notifying = false; }
    }
    if (update.docChanged || update.selectionSet || update.focusChanged) queueMicrotask(emitSelection);
    if ((pendingValue !== undefined || pendingSync) && !frozen()) queueMicrotask(flush);
  };
  function setValue(value) {
    value = String(value);
    if (destroyed) return;
    if (value === view.state.doc.toString()) { pendingValue = undefined; return; }
    if (frozen() || notifying) { pendingValue = value; return; }
    pendingValue = undefined;
    const old = view.state.doc.toString();
    let a = 0, z = 0;
    while (a < old.length && a < value.length && old[a] === value[a]) a++;
    while (z < old.length - a && z < value.length - a && old[old.length - 1 - z] === value[value.length - 1 - z]) z++;
    // Keep a surrogate pair in one replacement, even when only its low unit differs.
    if (a && /[\uD800-\uDBFF]/.test(old[a - 1])) a--;
    if (z && /[\uDC00-\uDFFF]/.test(old[old.length - z])) z--;
    view.dispatch({ changes: { from: a, to: old.length - z, insert: value.slice(a, value.length - z) }, annotations: [external, isolateHistory.of('full')] });
  }
  function flush() {
    if (destroyed || frozen()) return;
    if (pendingValue !== undefined) setValue(pendingValue);
    if (pendingSync) { pendingSync = false; sync(); }
    else view.dispatch({ effects: refresh.of(null), annotations: external });
    emitSelection();
  }
  function format(command, argument = '') {
    if (destroyed || !writable() || frozen() || !enabled()) return false;
    const selection = view.state.selection.main;
    const edit = host.call('edit', view.state.doc.toString(), selection.from, selection.to, command, String(argument));
    if (edit.error) { console.warn('exact: format refused:', edit.error); return false; }
    if (!edit.replacements?.length) return false;
    view.dispatch({ changes: edit.replacements.map(([from, to, insert]) => ({ from, to, insert })),
      selection: EditorSelection.single(...edit.selection), scrollIntoView: true,
      annotations: [Transaction.userEvent.of(command === 'newline' ? 'input' : 'input.format'), isolateHistory.of('full')] });
    view.focus();
    return true;
  }
  const link = () => {
    if (!writable() || frozen()) return false;
    const selection = view.state.selection.main;
    const state = host.call('selection', view.state.doc.toString(), selection.from, selection.to);
    const url = window.prompt('Link URL', state.link || 'https://');
    if (url !== null) format('link', url);
    return true;
  };
  const copyPlain = () => {
    const sel = view.state.selection.main;
    const text = host.call('plain', view.state.sliceDoc(sel.from, sel.to));
    navigator.clipboard?.writeText(text).catch(error => console.warn('exact: plain copy failed', error));
    return true;
  };
  function configuration() {
    const disabled = el.hasAttribute('disabled'), readonly = el.hasAttribute('readonly');
    if (el.hasAttribute('tabindex')) { tabIndex = el.getAttribute('tabindex'); el.removeAttribute('tabindex'); }
    const attrs = { ...inputDefaults, role: 'textbox', 'aria-multiline': 'true', 'aria-readonly': String(readonly), 'aria-disabled': String(disabled), tabindex: disabled ? '-1' : tabIndex };
    for (const attr of el.attributes) if (/^(aria-|autocapitalize$|autocorrect$|spellcheck$|inputmode$|enterkeyhint$|lang$|dir$)/.test(attr.name)) attrs[attr.name] = attr.value;
    // Disabled/readonly are the committed control props, not authored ARIA hints.
    attrs['aria-disabled'] = String(disabled); attrs['aria-readonly'] = String(readonly);
    return [EditorState.readOnly.of(disabled || readonly), EditorView.editable.of(!disabled && !readonly),
      EditorView.contentAttributes.of(attrs), placeholder(el.getAttribute('placeholder') ?? '')];
  }
  function sync() {
    if (destroyed) return;
    if (frozen() || notifying) { pendingSync = true; return; }
    if (!enabled()) {
      const native = document.createElement('textarea'), source = view.state.doc.toString(), sel = view.state.selection.main, focused = view.hasFocus;
      for (const attr of el.attributes) native.setAttribute(attr.name, attr.value);
      native.setAttribute('tabindex', tabIndex);
      native.classList.remove('exact-markdown-editor'); native.value = source; native.exactSourceValue = source;
      const offset = p => source.slice(0, p).replace(/\r\n?/g, '\n').length;
      native.setSelectionRange(offset(sel.from), offset(sel.to), sel.anchor > sel.head ? 'backward' : 'forward');
      host.replace(native); el.exactMarkup.destroy(); el.replaceWith(native);
      if (focused && !native.disabled) native.focus();
      return;
    }
    view.dispatch({ effects: [config.reconfigure(configuration()), refresh.of(null)], annotations: external });
    if (el.hasAttribute('disabled') && view.hasFocus) view.contentDOM.blur();
  }
  const stop = event => { event.stopPropagation(); return false; };
  const handlers = EditorView.domEventHandlers({
    input: stop,
    compositionstart(event) { composing = true; return stop(event); },
    compositionend(event) { composing = false; setTimeout(flush, 0); return stop(event); },
    pointerdown() { dragging = true; return false; },
    focus() { el.dispatchEvent(new FocusEvent('focus')); queueMicrotask(emitSelection); return false; },
    blur() { el.dispatchEvent(new FocusEvent('blur')); return false; },
  });
  const release = () => { if (dragging) { dragging = false; queueMicrotask(flush); } };
  window.addEventListener('pointerup', release);
  window.addEventListener('pointercancel', release);
  const raw = textarea.exactSourceValue;
  const initialValue = raw?.replace(/\r\n?/g, '\n') === textarea.value ? raw : textarea.value;
  const sourceOffset = offset => {
    let at = 0;
    for (let n = 0; n < offset && at < initialValue.length; n++, at++) if (initialValue[at] === '\r' && initialValue[at + 1] === '\n') at++;
    return at;
  };
  const initialSelection = EditorSelection.single(sourceOffset(textarea.selectionDirection === 'backward' ? textarea.selectionEnd : textarea.selectionStart),
    sourceOffset(textarea.selectionDirection === 'backward' ? textarea.selectionStart : textarea.selectionEnd));
  view = new EditorView({ parent: el, state: EditorState.create({ doc: initialValue, selection: initialSelection,
    extensions: [EditorState.lineSeparator.of('\n'), theme, EditorView.lineWrapping, history(), styled, config.of(configuration()), handlers,
      EditorView.updateListener.of(emit), keymap.of([
        { key: 'Mod-b', run: () => format('bold') }, { key: 'Mod-i', run: () => format('italic') },
        { key: 'Mod-k', run: link }, { key: 'Mod-Shift-c', run: copyPlain },
        { key: 'Mod-]', run: () => format('indent') }, { key: 'Mod-[', run: () => format('outdent') },
        { key: 'Enter', run: () => format('newline') }, ...historyKeymap, ...defaultKeymap,
      ])],
  }) });
  Object.defineProperty(el, 'value', { get: () => view.state.doc.toString(), set: setValue });
  el.focus = () => { if (!el.hasAttribute('disabled')) view.focus(); };
  el.select = () => { if (!el.hasAttribute('disabled')) view.dispatch({ selection: { anchor: 0, head: view.state.doc.length } }); };
  el.exactMarkup = { sync, format, setValue, view, destroy() {
    if (destroyed) return;
    destroyed = true; window.removeEventListener('pointerup', release); window.removeEventListener('pointercancel', release); view.destroy();
  } };
  // The outer node owns authored layout and identity; only CM's content takes focus.
  el.removeAttribute('tabindex'); el.removeAttribute('role');
  textarea.replaceWith(el);
  host.replace(el);
  if (initialFocus) view.focus();
  queueMicrotask(emitSelection);
  return el;
}

globalThis.exact.installMarkupEditor = installMarkupEditor;
export { installMarkupEditor };
