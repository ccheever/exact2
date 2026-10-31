// A rendered page's own stylesheet (LLP 1048.007 D4): of the shell's sheet,
// the rules that can match the page's document before its runtime runs.
//
// The build analyzes the sheet once (`analyze`): each rule with what its
// selector needs of a document, as alternatives of tokens (a tag `t:a`, a
// class `c:c7`, an id `i:x`, an attribute name `a:role`), any one of which,
// whole, lets it match. Being needed is a necessary condition, not a
// sufficient one: structure, attribute values and states are ignored, so a
// rule left out matches no element of the page. A renderer keeps the rules
// whose needs the page's tokens meet (`select`, and page.rs `page_style`),
// in the sheet's order, so what a kept rule wins it still wins.

// Present in every document whatever its markup: the parser makes them.
const ALWAYS = new Set(['t:html', 't:head', 't:body', 't:tbody']);
// Set by the browser itself, without the runtime: `open` (a details or a
// dialog an invoker opens), so it needs nothing.
const NATIVE = new Set(['open']);

// The selector list `s` split at top-level commas.
function split(s, by = ',') {
  const out = []; let depth = 0, at = 0, quote = null;
  for (let i = 0; i < s.length; i++) {
    const ch = s[i];
    if (quote) { if (ch === '\\') i++; else if (ch === quote) quote = null; continue; }
    if (ch === '"' || ch === "'") quote = ch;
    else if (ch === '\\') i++;
    else if (ch === '(' || ch === '[') depth++;
    else if (ch === ')' || ch === ']') depth--;
    else if (ch === by && depth === 0) { out.push(s.slice(at, i)); at = i + 1; }
  }
  out.push(s.slice(at));
  return out;
}

// The alternatives a selector list needs: [[token…]…]; [[]] for none.
export function needs(list) {
  const alts = [];
  for (const complex of split(list)) {
    let options = [[]];
    for (const need of compounds(complex)) options = options.flatMap(o => need.map(n => [...o, ...n]));
    alts.push(...options);
    if (alts.length > 64) return [[]]; // past this, keep the rule
  }
  return alts.map(a => [...new Set(a)].filter(t => !ALWAYS.has(t)));
}

// Each compound of a complex selector, as the alternatives it needs.
function compounds(complex) {
  const out = []; let i = 0; const s = complex;
  const name = () => { const m = /^(?:\\.|[-\w -￿])+/.exec(s.slice(i)); i += m ? m[0].length : 0; return m ? m[0].replace(/\\(.)/g, '$1') : ''; };
  let current = [[]];
  const and = token => { current = current.map(o => [...o, token]); };
  const flush = () => { out.push(current); current = [[]]; };
  while (i < s.length) {
    const ch = s[i];
    if (/[\s>+~]/.test(ch)) { flush(); i++; continue; }
    if (ch === '&') { i++; continue; }
    if (ch === '*') { i++; continue; }
    if (ch === '.') { i++; and(`c:${name()}`); continue; }
    if (ch === '#') { i++; and(`i:${name()}`); continue; }
    if (ch === '[') {
      const end = closing(s, i, '[', ']');
      const attr = /^\[\s*([-\w:|]+)/.exec(s.slice(i))?.[1]?.replace(/^.*\|/, '').toLowerCase();
      if (attr && !NATIVE.has(attr)) and(`a:${attr}`);
      i = end + 1; continue;
    }
    if (ch === ':') {
      const pseudo = /^::?([-\w]+)/.exec(s.slice(i));
      i += pseudo[0].length;
      if (s[i] === '(') {
        const end = closing(s, i, '(', ')'), inner = s.slice(i + 1, end);
        i = end + 1;
        // One of the list's selectors must match the element (or, for
        // `:has`, a relative): its alternatives join this compound's.
        if (['is', 'where', 'matches', 'has'].includes(pseudo[1].toLowerCase())) {
          const alts = needs(inner.replace(/^\s*[>+~]/, ''));
          current = current.flatMap(o => alts.map(a => [...o, ...a]));
        }
      }
      continue;
    }
    if (/[-\w]/.test(ch)) { and(`t:${name().toLowerCase()}`); continue; }
    i++; // anything else needs nothing
  }
  flush();
  return out;
}

function closing(s, at, open, close) {
  let depth = 0, quote = null;
  for (let i = at; i < s.length; i++) {
    const ch = s[i];
    if (quote) { if (ch === '\\') i++; else if (ch === quote) quote = null; continue; }
    if (ch === '"' || ch === "'") quote = ch;
    else if (ch === '\\') i++;
    else if (ch === open) depth++;
    else if (ch === close && --depth === 0) return i;
  }
  return s.length;
}

// The sheet's top-level blocks, each `prelude{body}`.
function blocks(css) {
  const out = []; let depth = 0, at = 0, quote = null, open = -1;
  for (let i = 0; i < css.length; i++) {
    const ch = css[i];
    if (quote) { if (ch === '\\') i++; else if (ch === quote) quote = null; continue; }
    if (ch === '"' || ch === "'") quote = ch;
    else if (ch === '\\') i++;
    else if (ch === '{') { if (depth++ === 0) open = i; }
    else if (ch === '}' && --depth === 0) { out.push([css.slice(at, open).trim(), css.slice(open + 1, i)]); at = i + 1; }
    else if (ch === ';' && depth === 0) { out.push([css.slice(at, i + 1).trim(), null]); at = i + 1; }
  }
  return out;
}

const GROUPING = /^@(media|supports|container|layer|scope|document)\b/i;

/** The sheet as rules in order: `[wrap, rule, needs]`, `wrap` the grouping
 * at-rules around it (`@media …{`, each opened), `needs` as `needs` gives. */
export function analyze(css, wrap = '') {
  const out = [];
  for (const [prelude, body] of blocks(css)) {
    if (body === null) out.push([wrap, prelude, [[]]]);
    else if (GROUPING.test(prelude) && !/^@layer\s+[^{]*$/.test(prelude) && blocks(body).length) out.push(...analyze(body, `${wrap}${prelude}{`));
    else if (prelude.startsWith('@')) out.push([wrap, `${prelude}{${body}}`, [[]]]);
    // The app's classes, written inside one `#exact-root#exact-root{…}`: each its own rule, nested.
    else if (prelude === '#exact-root#exact-root' && /^[^{]*\{/.test(body)) out.push(...analyze(body, `${wrap}${prelude}{`));
    else out.push([wrap, `${prelude}{${body}}`, needs(prelude)]);
  }
  return out;
}

/** The tokens a document's markup offers (as `needs` names them). */
export function tokens(html) {
  const out = new Set();
  for (let i = html.indexOf('<'); i >= 0; i = html.indexOf('<', i + 1)) {
    const tag = /^<([a-zA-Z][-\w]*)/.exec(html.slice(i, i + 64));
    if (!tag) continue;
    out.add(`t:${tag[1].toLowerCase()}`);
    // Its attributes, to the `>` outside a quoted value.
    i += tag[0].length;
    while (i < html.length && html[i] !== '>') {
      const name = /^[\s\/]*([^\s"'=<>\/]+)/.exec(html.slice(i, i + 256));
      if (!name) { i++; continue; }
      i += name[0].length;
      const attr = name[1].toLowerCase();
      let value = '';
      const eq = /^\s*=\s*/.exec(html.slice(i, i + 16));
      if (eq) {
        i += eq[0].length;
        const q = html[i];
        if (q === '"' || q === "'") { const end = html.indexOf(q, i + 1); value = html.slice(i + 1, end < 0 ? html.length : end); i = end < 0 ? html.length : end + 1; }
        else { const m = /^[^\s>]*/.exec(html.slice(i)); value = m[0]; i += value.length; }
      }
      out.add(`a:${attr}`);
      if (attr === 'class') for (const c of value.split(/\s+/).filter(Boolean)) out.add(`c:${c}`);
      if (attr === 'id') out.add(`i:${value}`);
    }
  }
  return out;
}

/** The rules of `rules` (`analyze`'s) a document offering `have` can need, as one sheet. */
export function select(rules, have) {
  let out = '', open = '';
  for (const [wrap, rule, alts] of rules) {
    if (!alts.some(a => a.every(t => have.has(t)))) continue;
    if (wrap !== open) { out += '}'.repeat(open.split('{').length - 1) + wrap; open = wrap; }
    out += rule;
  }
  return out + '}'.repeat(open.split('{').length - 1);
}
