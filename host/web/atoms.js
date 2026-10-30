// Inline atoms on the web (LLP 1045.000): the pill the Markdown reader and
// editor draw, the reader's presses, the agent's `state.atoms` and a reader's
// `rangeFrame`. Fetched only by a page with atoms: the editor imports it, and
// the reader's `renderMarkup` (navigation.js) loads it when a piece is an
// atom. It registers itself as `exact.atoms` for the glue and the agent.

/** An inline atom's pill (D4, D8): modeled on the macOS token, one line at
 * the line's text height, its label cut with an ellipsis at the line's width;
 * its kind's `color` and `background-color` (`css`) win, and its face's
 * (`a.color`, `a.backgroundColor`) over those. It lays out its icon and then
 * its label in its line's direction, which it inherits, or in the one its
 * face pins (`a.direction`, `ltr` or `rtl`). A button, or a link as
 * declared, named by its face's `a.ariaLabel` or else its label, and
 * described by its title; its icon is not read. `editing` makes it the
 * editor's island: no tab stop, no source of its own. Its `data-atom-*`
 * attributes are what a press reports. */
export function atomBox(a, css = '', editing = false) {
  const box = document.createElement('span');
  const set = (name, value) => value && box.setAttribute(name, value);
  box.className = 'exact-atom';
  Object.assign(box.dataset, { exactAtom: a.kind, atomSource: a.source, atomHref: a.href ?? '', atomStart: a.start, atomEnd: a.end });
  box.setAttribute('role', a.role === 'link' ? 'link' : 'button');
  set('aria-label', a.ariaLabel || a.label);
  set('aria-description', a.title);
  set('title', a.title);
  if (editing) box.contentEditable = 'false';
  else box.tabIndex = 0;
  if (a.direction === 'ltr' || a.direction === 'rtl') box.dir = a.direction;
  box.style.cssText = 'display:inline-block;box-sizing:border-box;max-width:100%;vertical-align:bottom;overflow:hidden;text-overflow:ellipsis;'
    + 'white-space:nowrap;text-indent:0;padding:0 .3em;border-radius:.3em;background-color:light-dark(#e1e7f3,#3b4252);color:inherit;'
    + 'cursor:default;user-select:none;-webkit-user-select:none;' + css;
  // One value each, so a colour the browser does not take leaves the kind's.
  if (a.color) box.style.setProperty('color', a.color);
  if (a.backgroundColor) box.style.setProperty('background-color', a.backgroundColor);
  if (a.icon && !a.icon.startsWith('symbol:')) {
    const img = box.appendChild(document.createElement('img'));
    img.src = a.icon; img.alt = ''; img.setAttribute('aria-hidden', 'true');
    // Inline before the label, over the page's `img { display: block }` (index.html).
    img.style.cssText = 'display:inline-block;width:.75em;height:.75em;margin-inline-end:.25em;vertical-align:baseline;object-fit:contain';
  }
  box.append(a.label);
  return box;
}

/** A reader piece's pill: its detail `[kind, role, icon, title, href,
 * source, start, end, color, backgroundColor, direction, ariaLabel]`, its
 * label `text`, in the piece's scale, weight and flags (italic 1, strike 4),
 * its kind's paint from the text's `data-atom-styles` and its face's over
 * it. */
export function readerAtom(el, text, detail, scale = 1, weight = 0, flags = 0) {
  const [kind, role, icon, title, href, source, start, end, color = '', backgroundColor = '', direction = '', ariaLabel = ''] = detail;
  let styles = {};
  try { styles = JSON.parse(el.getAttribute('data-atom-styles') || '{}'); } catch {}
  presses(el);
  const box = atomBox({ kind, role, label: text, icon, title, href, source, start, end, color, backgroundColor, direction, ariaLabel }, styles[kind] ?? '');
  if (scale !== 1) box.style.fontSize = `${scale}em`;
  if (weight) box.style.fontWeight = weight;
  if (flags & 1) box.style.fontStyle = 'italic';
  if (flags & 4) box.style.textDecoration = 'line-through';
  return box;
}

const record = (box, e) => {
  const d = box.dataset;
  return [d.exactAtom, d.atomSource, d.atomHref ?? '', Number(d.atomStart), Number(d.atomEnd), e.shiftKey, e.ctrlKey, e.altKey, e.metaKey];
};

/** A reader's presses (D6, D7): a click, Return, or Space on a button atom
 * (on key up, as a button activates) is the text's `atompress`, its detail
 * the `AtomEvent` record; a link atom takes Return only, as a link does. A
 * disabled or inert text presses nothing. */
export function presses(el) {
  if (el.exactAtomPresses) return;
  el.exactAtomPresses = true;
  const hit = e => {
    const box = e.target?.closest?.('[data-exact-atom]');
    return box && el.contains(box) && box.contentEditable !== 'false' && !el.closest('[disabled], [inert]') ? box : null;
  };
  const fire = (box, e) => el.dispatchEvent(new CustomEvent('atompress', { detail: record(box, e) }));
  el.addEventListener('click', e => { const box = hit(e); if (box) fire(box, e); });
  el.addEventListener('keydown', e => {
    const box = e.target === hit(e) && hit(e);
    if (!box || e.repeat) return;
    if (e.key === 'Enter') { e.preventDefault(); fire(box, e); }
    else if (e.key === ' ' && box.getAttribute('role') === 'button') e.preventDefault(); // no scroll; it activates on key up
  });
  el.addEventListener('keyup', e => {
    const box = e.key === ' ' && e.target === hit(e) && hit(e);
    if (box && box.getAttribute('role') === 'button') { e.preventDefault(); fire(box, e); }
  });
}

/** Each inline atom on the page, as the agent's `state.atoms` (D9): its
 * node's id, kind, source and range, label, and its pill's frame from the
 * node's top left (where `tap <node> at x y` presses it). */
export function state(idOf) {
  const out = [];
  const add = (host, a, box) => {
    const h = host.getBoundingClientRect(), r = box.getBoundingClientRect();
    out.push({ id: idOf(host), ...a, frame: [r.x - h.x, r.y - h.y, r.width, r.height].map(n => Math.round(n * 100) / 100) });
  };
  for (const el of document.querySelectorAll('.exact-markdown-editor')) {
    for (const { box, ...a } of el.exactMarkup?.atoms?.() ?? []) add(el, a, box);
  }
  for (const box of document.querySelectorAll('[data-exact-atom]:not([contenteditable="false"])')) {
    const d = box.dataset, host = box.closest('[data-exact-text]') ?? box.parentElement;
    add(host, { kind: d.exactAtom, source: d.atomSource, start: Number(d.atomStart), end: Number(d.atomEnd), label: box.textContent }, box);
  }
  return out;
}

/** A reader's `rangeFrame` (D7): the union of the pills whose source ranges
 * meet `start..end`; collapsed at a pill's edge, its edge, on the side its
 * line's direction puts it. Null where no pill is. */
export function rangeFrame(el, start, end) {
  let left = Infinity, top = Infinity, right = -Infinity, bottom = -Infinity;
  for (const box of el.querySelectorAll('[data-exact-atom]')) {
    const s = Number(box.dataset.atomStart), e = Number(box.dataset.atomEnd), r = box.getBoundingClientRect();
    if (start === end) {
      if (start !== s && start !== e) continue;
      // The line's direction, not the pill's own, which its face may pin.
      const rtl = getComputedStyle(box.parentElement).direction === 'rtl', x = (start === s) !== rtl ? r.left : r.right;
      return new DOMRect(x, r.top, 0, r.height);
    }
    if (!(s < end && start < e)) continue;
    left = Math.min(left, r.left); top = Math.min(top, r.top); right = Math.max(right, r.right); bottom = Math.max(bottom, r.bottom);
  }
  return left === Infinity ? null : new DOMRect(left, top, right - left, bottom - top);
}

(globalThis.exact ??= {}).atoms = { readerAtom, state, rangeFrame };
