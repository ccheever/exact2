// A plain textarea's decorations (LLP 1114.007): the app's TextDecorations
// record painted with the CSS Custom Highlight API over the field's own
// value ranges (`createValueRange()`, OpaqueRange), one `::highlight()` rule
// per span. Paint only: the textarea stays the browser's, its value and
// undo history are never touched, and nothing is repainted while text is
// being composed. A browser without `createValueRange` paints nothing.

const painted = typeof HTMLTextAreaElement === 'function' && typeof HTMLTextAreaElement.prototype.createValueRange === 'function'
  && typeof Highlight === 'function' && !!globalThis.CSS?.highlights;
const high = unit => unit >= 0xd800 && unit <= 0xdbff, low = unit => unit >= 0xdc00 && unit <= 0xdfff;

/** The one replacement between two strings' common prefix and suffix, in
 * UTF-16 units, never splitting a surrogate pair: kernel/src/text/decorations.rs `Edit::between`. */
export function edit(before, after) {
  let start = 0;
  while (start < before.length && start < after.length && before.charCodeAt(start) === after.charCodeAt(start)) start++;
  if (start > 0 && start < Math.max(before.length, after.length) && high(before.charCodeAt(start - 1))) start--;
  let suffix = 0;
  const room = Math.min(before.length, after.length) - start;
  while (suffix < room && before.charCodeAt(before.length - 1 - suffix) === after.charCodeAt(after.length - 1 - suffix)) suffix++;
  if (suffix > 0 && low(before.charCodeAt(before.length - suffix))) suffix--;
  return { start, oldEnd: before.length - suffix, newEnd: after.length - suffix };
}

/** The record's spans on `current`: as given for its own text, otherwise each
 * boundary moved across the replacement as a live DOM `Range`'s is, and
 * provisional until the app's next record: `Decorations::resolve`. */
export function resolve(record, current) {
  const e = edit(record.value, current), old = record.value.length;
  const boundary = at => at <= e.start ? at : at >= e.oldEnd ? at + e.newEnd - e.oldEnd : e.start;
  // Offsets as the runner's codec reads them: a non-finite one refuses the
  // record; the rest are truncated toward zero, never below it.
  const offset = n => typeof n === 'number' && Number.isFinite(n) ? Math.max(0, Math.trunc(n)) : NaN;
  const spans = [];
  for (const span of record.spans ?? []) {
    let start = Math.min(offset(span.start), old), end = Math.min(offset(span.end), old);
    if (Number.isNaN(start) || Number.isNaN(end)) return { provisional: record.value !== current, spans: [] };
    if (!(start < end)) continue;
    start = Math.min(boundary(start), current.length); end = Math.min(boundary(end), current.length);
    if (start < end) spans.push({ name: span.name, start, end });
  }
  return { provisional: record.value !== current, spans };
}

let fields = 0;
/** Paint `textarea`'s `data-decorations` with its `data-decoration-styles`
 * (CSS text by name) and keep painting it as either changes or the person
 * types. `log` hears a name with no declaration, once. */
export function install(textarea, log = () => {}) {
  if (textarea.exactDecorations) return textarea.exactDecorations;
  const id = ++fields, names = [], warned = new Set(), stop = new AbortController();
  // A composition already under way when this loads is the runtime's to say.
  let sheet = null, composing = !!textarea.exactComposing, gone = false;
  const clear = () => { for (const name of names) CSS.highlights.delete(name); names.length = 0; if (sheet) sheet.textContent = ''; };
  const handle = textarea.exactDecorations = { painted, applied: null, sync, destroy };
  function sync() {
    if (gone || composing) return;
    let record = null, table = {};
    try { record = JSON.parse(textarea.getAttribute('data-decorations') ?? 'null'); table = JSON.parse(textarea.getAttribute('data-decoration-styles') ?? '{}'); }
    catch (error) { log(`decorations: ${error.message}`); }
    const resolved = record && typeof record.value === 'string' ? resolve(record, textarea.value) : null;
    handle.applied = resolved && { ...resolved, value: textarea.value };
    if (!painted) return;
    clear();
    if (!resolved) return;
    const rules = [];
    resolved.spans.forEach((span, index) => {
      if (!Object.hasOwn(table, span.name)) {
        if (!warned.has(span.name)) { warned.add(span.name); log(`decorations: no decoration ${JSON.stringify(span.name)}`); }
        return;
      }
      // One highlight per span, registered in order: at equal priority the
      // later registration paints over the earlier (CSS Highlight API §4).
      const name = `exact-decoration-${id}-${index}`;
      CSS.highlights.set(name, new Highlight(textarea.createValueRange(span.start, span.end)));
      names.push(name); rules.push(`::highlight(${name}){${table[span.name]}}`);
    });
    if (rules.length && !sheet) { sheet = document.createElement('style'); document.head.append(sheet); }
    if (sheet) sheet.textContent = rules.join('\n');
  }
  function destroy() {
    if (gone) return;
    gone = true; stop.abort(); observer.disconnect();
    if (painted) clear();
    sheet?.remove();
    delete textarea.exactDecorations;
  }
  const options = { signal: stop.signal };
  textarea.addEventListener('input', () => { if (!composing) sync(); }, options);
  textarea.addEventListener('compositionstart', () => { composing = true; }, options);
  textarea.addEventListener('compositionend', () => { composing = false; sync(); }, options);
  const observer = new MutationObserver(sync);
  observer.observe(textarea, { attributes: true, attributeFilter: ['data-decorations', 'data-decoration-styles'] });
  sync();
  return handle;
}

if (globalThis.exact) globalThis.exact.installDecorations = install;
