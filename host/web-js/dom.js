// The DOM the JS runtime needs, and no more, for rendering a page under
// Bun (`render.mjs`): elements, text, comments and fragments in a tree,
// attributes in insertion order, an inline style map, and HTML serialization
// by the rules `host/web/src/document.rs` writes (escaping, void elements,
// boolean attributes). Region anchors (comments) are not written: adoption
// inserts its own.
const VOID = new Set(['img', 'input', 'br', 'hr', 'meta', 'link', 'source', 'area', 'col', 'embed', 'wbr']);
const esc = (s, attr) => String(s).replace(attr ? /[&"\r]/g : /[&<>\r]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', '\r': '&#13;' })[c]);

class Node {
  constructor(type) { this.nodeType = type; this.parentNode = null; this.childNodes = []; }
  get parentElement() { return this.parentNode?.nodeType === 1 ? this.parentNode : null; }
  get firstChild() { return this.childNodes[0] ?? null; }
  get nextSibling() { const s = this.parentNode?.childNodes; return s ? s[s.indexOf(this) + 1] ?? null : null; }
  // rt.js `each` empties a region that is all its parent holds by these: without them every sibling went too.
  get previousSibling() { const s = this.parentNode?.childNodes; return s ? s[s.indexOf(this) - 1] ?? null : null; }
  get lastChild() { return this.childNodes.at(-1) ?? null; }
  get isConnected() { let n = this; while (n.parentNode) n = n.parentNode; return n.nodeType === 9; }
  append(...nodes) { for (const n of nodes) this.insertBefore(typeof n === 'string' ? new Text(n) : n, null); }
  // The nodes as one fragment, then before the first child (rt.js `each`'s row fragments).
  prepend(...nodes) { const f = new Fragment(); f.append(...nodes); this.insertBefore(f, this.firstChild); }
  insertBefore(n, ref) {
    const list = n.nodeType === 11 ? n.childNodes.splice(0) : [n];
    for (const x of list) { x.parentNode?.removeChild(x); x.parentNode = this; }
    const at = ref ? this.childNodes.indexOf(ref) : -1;
    this.childNodes.splice(at < 0 ? this.childNodes.length : at, 0, ...list);
    return n;
  }
  removeChild(n) { const i = this.childNodes.indexOf(n); if (i >= 0) this.childNodes.splice(i, 1); n.parentNode = null; return n; }
  remove() { this.parentNode?.removeChild(this); }
  appendChild(n) { this.insertBefore(n, null); return n; }
  replaceChildren(...nodes) { for (const c of this.childNodes) c.parentNode = null; this.childNodes = []; this.append(...nodes); }
  before(...nodes) { for (const n of nodes) this.parentNode.insertBefore(n, this); }
  after(...nodes) { const next = this.nextSibling; for (const n of nodes) this.parentNode.insertBefore(n, next); }
  get textContent() { return this.childNodes.map(c => c.textContent).join(''); }
  set textContent(v) { for (const c of this.childNodes) c.parentNode = null; this.childNodes = v === '' ? [] : [new Text(v)]; }
  addEventListener() {} removeEventListener() {} dispatchEvent() {}
}
class Text extends Node { constructor(t) { super(3); this.data = String(t); } get textContent() { return this.data; } html() { return esc(this.data); } }
class Comment extends Node { constructor() { super(8); } get textContent() { return ''; } html() { return ''; } }
class Fragment extends Node { constructor() { super(11); } }
class Style {
  constructor() { this.map = new Map(); }
  setProperty(k, v) { if (v !== '' && v != null) this.map.set(k, String(v)); }
  removeProperty(k) { this.map.delete(k); }
  getPropertyValue(k) { return this.map.get(k) ?? ''; }
  set fontSize(v) { this.setProperty('font-size', v); } set fontWeight(v) { this.setProperty('font-weight', v); }
  set fontStyle(v) { this.setProperty('font-style', v); } set fontFamily(v) { this.setProperty('font-family', v); }
  set textDecoration(v) { this.setProperty('text-decoration', v); } set opacity(v) { this.setProperty('opacity', v); }
  get isolation() { return this.getPropertyValue("isolation"); }
  set isolation(v) { this.setProperty("isolation", v); }
  get cssText() { return [...this.map].map(([k, v]) => `${k}:${v};`).join(''); }
  set cssText(t) { this.map.clear(); for (const d of t.split(';')) { const i = d.indexOf(':'); if (i > 0) this.map.set(d.slice(0, i).trim(), d.slice(i + 1).trim()); } }
}
class Element extends Node {
  constructor(tag, fonts) { super(1); this.fonts = fonts; this.localName = tag; this.attrs = new Map(); this.style = new Style(); const data = k => 'data-' + k.replace(/[A-Z]/g, c => '-' + c.toLowerCase());
    // Read as written: rt.js reads `dataset.exactOn` and the like back.
    this.dataset = new Proxy({}, { set: (_, k, v) => (this.setAttribute(data(k), v), true), get: (_, k) => typeof k === 'string' ? this.getAttribute(data(k)) ?? undefined : undefined,
      has: (_, k) => typeof k === 'string' && this.hasAttribute(data(k)), deleteProperty: (_, k) => (this.removeAttribute(data(k)), true) }); }
  get id() { return this.getAttribute("id") ?? ""; }
  get attributes() { return [...this.attrs].map(([name, value]) => ({ name, value })); }
  getAttributeNames() { return [...this.attrs.keys()]; }
  get children() { return this.childNodes.filter(c => c.nodeType === 1); }
  get nextElementSibling() { let n = this.nextSibling; while (n && n.nodeType !== 1) n = n.nextSibling; return n; }
  get tagName() { return this.localName.toUpperCase(); }
  setAttribute(k, v) { this.attrs.set(k, String(v)); }
  getAttribute(k) { return this.attrs.get(k) ?? null; }
  hasAttribute(k) { return this.attrs.has(k); }
  removeAttribute(k) { this.attrs.delete(k); }
  toggleAttribute(k, on) { if (on) this.attrs.set(k, ''); else this.attrs.delete(k); }
  get childElementCount() { return this.childNodes.filter(c => c.nodeType === 1).length; }
  get firstElementChild() { return this.childNodes.find(c => c.nodeType === 1) ?? null; }
  getElementsByTagName(tag) { return this.childNodes.flatMap(c => c.nodeType === 1 ? [...(tag === '*' || c.localName === tag ? [c] : []), ...c.getElementsByTagName(tag)] : []); }
  querySelectorAll() { return []; } querySelector() { return null; } contains() { return false; }
  // Simple selectors only (`tag`, `tag[attr]`, `[attr]`, comma-separated): rt.js `on` asks whether a press is native.
  matches(sel) { return sel.split(',').some(p => { const m = /^([\w-]*)(?:\[([\w-]+)\])?$/.exec(p.trim()); return !!m && (!m[1] || m[1] === this.localName) && (!m[2] || this.hasAttribute(m[2])); }); }
  get value() { return this.localName === 'textarea' ? this.textContent : this.getAttribute('value') ?? ''; }
  get options() { return this.childNodes.filter(c => c.localName === 'option'); }
  // A select's value is the option that carries it, `selected` (document.rs).
  set value(v) { if (this.localName === 'select') for (const o of this.childNodes) o.toggleAttribute?.('selected', o.getAttribute('value') === v); else if (this.localName === 'textarea') this.textContent = v; else if (v === '') this.removeAttribute('value'); else this.setAttribute('value', v); }
  set checked(v) { this.toggleAttribute('checked', !!v); }
  get checked() { return this.hasAttribute('checked'); }
  set muted(v) {} pause() {} play() { return Promise.resolve(); }
  set className(v) { this.setAttribute('class', v); }
  // Read only (symbols.js `tinted`): the class attribute's names.
  get classList() { const names = (this.getAttribute('class') ?? '').split(/\s+/).filter(Boolean); return Object.assign(names, { contains: c => names.includes(c) }); }
  set href(v) { this.setAttribute('href', v); }
  html(inheritedFont = 16) {
    // Symbol images need a real natural size before adoption: the element's
    // font size, from its static class or live inline row, else inherited.
    let font = inheritedFont;
    for (const cls of (this.getAttribute('class') ?? '').split(/\s+/)) if (this.fonts?.has(cls)) font = fontPx(this.fonts.get(cls), inheritedFont);
    const ownFont = this.style.getPropertyValue('font-size');
    if (ownFont !== '') font = fontPx(ownFont, inheritedFont);
    if (this.localName === 'img' && this.hasAttribute('data-symbol-source')) this.setAttribute('src', `data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='${font}' height='${font}'/%3E`);
    // A head is the page's <head>, never an element in the root (as document.rs).
    if (this.localName === 'template') return '';
    let out = `<${this.localName}`;
    const style = this.style.cssText;
    for (const [k, v] of this.attrs) if (k !== 'style') out += v === '' ? ` ${k}` : ` ${k}="${esc(v, true)}"`;
    if (style) out += ` style="${esc(style, true)}"`;
    out += '>';
    if (VOID.has(this.localName)) return out;
    return out + this.childNodes.map(c => c.html(font)).join('') + `</${this.localName}>`;
  }
}

// A `font-size` in px, `em` (of the inherited size) or `rem` (of CSS's
// initial 16: a render has no reader's setting), as the browser resolves it.
const fontPx = (value, inherited) => {
  const m = /^\s*([\d.]+)(px|em|rem)?\s*$/i.exec(value);
  if (!m) return inherited;
  const unit = (m[2] ?? 'px').toLowerCase();
  return Number(m[1]) * (unit === 'em' ? inherited : unit === 'rem' ? 16 : 1);
};

/** A document for one render: `#exact-root` in a body, a head for metas. */
export function createDocument(shell = '') {
  const fonts = new Map();
  for (const rule of shell.matchAll(/\.([\w-]+)\{([^}]+)\}/g)) {
    const size = /(?:^|;)font-size:([^;]+)(?:;|$)/.exec(rule[2]);
    if (size) fonts.set(rule[1], size[1]);
  }
  const doc = new Node(9);
  const root = new Element('div'); root.setAttribute('id', 'exact-root');
  const head = new Element('head'); const body = new Element('body');
  doc.append(head, body); body.append(root);
  Object.assign(doc, {
    title: '', head, body, documentElement: body, styleSheets: [], // symbols.js reads a tint from the stylesheets: a render has none
    createElement: t => new Element(t, fonts), createElementNS: (_, t) => new Element(t, fonts),
    createTextNode: t => new Text(t), createComment: () => new Comment(), createDocumentFragment: () => new Fragment(),
    getElementById: id => (id === 'exact-root' ? root : null),
    querySelector: sel => (/meta\[name="description"\]/.test(sel) ? head.childNodes.find(m => m.getAttribute?.('name') === 'description') ?? null : null),
    querySelectorAll: () => [],
    root, rootHTML: () => root.childNodes.map(c => c.html()).join(''),
  });
  return doc;
}
