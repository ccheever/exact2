// The server's half of inspect (LLP 1122 D5; T2b step 5): a build's
// `selected` report joined to source.
//
// - **Currency.** A report whose plan, design or pin revision is not the
//   build's latest status is from before a batch or a pin: refused, and the
//   build is asked again (`inspect {refresh: true}`).
// - **The join.** The site is looked up in the source map of the plan the
//   report names (`app.plan.map.json`, verified by that digest): its file,
//   line, column and the component call sites above it, and each row's
//   origin. Each file the join reads is checked against the digest the map
//   says it was compiled from: one that moved since makes the selection
//   read-only, with its reason (D6's fail-closed rule).
// - **What is editable.** D6's rungs: a row that reads one component param,
//   the param rung (LLP 1122 T3, the param named in full, never by the IR
//   token the plan names it by); else a row its author wrote, where the map
//   places it, the template rung (on the node) or the style rung (in its
//   class's `style`), edited by `set-row` (T5), the token its whole value is
//   offered beside; a style's state part, the state rung (T4); a row reached
//   by inheritance is a link to its token, never this node's edit; anything
//   else is read-only, with its reason.
// - **Library** (*1.15*, LLP 1139 D5). A node of an exported library
//   component's own view names its declaring identity and part
//   (`source.library`, and each row with a map origin, a held state's rows
//   the held node's), from the map only: one the host's report carries is
//   dropped on every path.
// - **Outline.** Each build that can inspect is told the sites of its own
//   plan at the selection's source location (`outline {sites}`), every copy
//   of the node.
import { createHash } from 'node:crypto';
import { request } from 'node:http';
import { readFileSync, realpathSync } from 'node:fs';
import { pathName } from './tamber-paths.mjs';

const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');

/** The source map a resident dev loop said it made (LLP 1122 T2c): its
 * parent read `card` (`{plan, sha256, bytes, url}`) from the pipe it gave
 * the child for that alone, and fetches the bytes from the child's loopback
 * `port`, naming the `host` the child's gate answers. Only the child's map
 * path, no redirect; kept only when the bytes are the card's and the map's
 * own digest is the plan's. `{digest, bytes}`, or null. */
export function pullMap(port, card, host) {
  if (!card || !/^[0-9a-f]{64}$/.test(card.plan ?? '') || !/^[0-9a-f]{64}$/.test(card.sha256 ?? '') || !Number.isSafeInteger(card.bytes) || card.url !== `/__dev/map/${card.sha256}`) return Promise.resolve(null);
  return new Promise((done) => {
    const req = request({ host: '127.0.0.1', port, path: card.url, headers: { host } }, (res) => {
      if (res.statusCode !== 200) { res.resume(); done(null); return; }
      const parts = [];
      let size = 0;
      res.on('data', (d) => { size += d.length; if (size > card.bytes) { req.destroy(); done(null); } else parts.push(d); });
      res.on('end', () => {
        const bytes = Buffer.concat(parts);
        try {
          done(bytes.length === card.bytes && createHash('sha256').update(bytes).digest('hex') === card.sha256 && JSON.parse(bytes.toString('utf8')).digest === card.plan ? { digest: card.plan, bytes } : null);
        } catch { done(null); }
      });
    });
    req.on('error', () => done(null));
    req.end();
  });
}

/** A card line from a resident loop's map pipe (`{plan, map}`), or null. */
export const mapCard = (line) => {
  try {
    const c = JSON.parse(line);
    return /^[0-9a-f]{64}$/.test(c?.plan ?? '') && (c.map === null || (typeof c.map === 'object' && typeof c.map.sha256 === 'string')) ? { plan: c.plan, map: c.map } : null;
  } catch { return null; }
};

/** The map `bytes` hold, when its digest is `digest`; else null. */
export function mapFor(bytes, digest) {
  if (!bytes || !digest) return null;
  try {
    const map = JSON.parse(Buffer.from(bytes).toString('utf8'));
    return map?.digest === digest && Array.isArray(map.nodes) ? map : null;
  } catch { return null; }
}

/** Why a file the map names has moved since it was compiled, or ''. */
function moved(map, file, read, app = null) {
  // Named as an answer names a file (`tamber-paths.mjs`), never this machine's path.
  const shown = app ? pathName(file, app) : file;
  const known = (map.sources ?? []).find(s => s.file === file);
  if (!known) return `${shown} is not in the build's source map`;
  // A compiled-in module is nowhere on disk, and cannot move.
  if (!file.startsWith('/')) return '';
  let now;
  try { now = sha256(read(file)); } catch { return `${shown} is gone since this build`; }
  return now === known.sha256 ? '' : `${shown} changed since this build: select it again once it has reloaded`;
}

/** Why the design a token rung would edit is not the one the build runs,
 * or '': the app's design file (`file`, as the server names it) as the map
 * compiled it, or, changed since, as it was when the dev loop took the
 * design it serves now (`served {digest, source}`, a value edit the build
 * took live), when the build reports running that. Fails closed: a design
 * the map does not name, or one that cannot be read, is a reason. */
function designMoved(map, build, served, file, read, app = null) {
  // Gone from where the server looks (a deleted file): the map's own entry,
  // whose read then fails.
  file ??= (map.sources ?? []).find(s => /\.tamber$/.test(s.file))?.file ?? null;
  if (!file) return '';
  // Named as an answer names a file (`tamber-paths.mjs`), never this machine's path.
  const shown = app ? pathName(file, app) : file;
  const real = f => { try { return realpathSync(f); } catch { return f; } };
  const known = (map.sources ?? []).find(s => s.file === file || real(s.file) === real(file));
  if (!known) return `the app's design ${shown} is not in the build's source map`;
  let now;
  try { now = sha256(read(file)); } catch { return `${shown} is gone since this build`; }
  if (now === known.sha256) return '';
  if (served?.digest && served.source === now && build.design === served.digest) return '';
  return `${shown} changed since this build, which has not taken it: select again once it has`;
}

// A declared token's name (a dotted word, a role's spelling); a synthetic
// token is named by its whole expression, and is no token to edit.
const declared = name => /^-?[A-Za-z_][\w.-]*$/.test(name);

// The IR's name for a component param (contract/lower/src/params.rs:
// `param.f<8 hex>.<Component>.<param>`), which no author or tool writes;
// anywhere in a name, since a synthetic token is named by its whole
// expression, but only as a whole word. The JS target's pages write it as
// a custom property (`var(--x-param-f<8 hex>-Card-fill)`, modes.rs).
const IR_PARAM = /(?<![\w.-])param\.f[0-9a-f]{8}\.[A-Za-z_][\w-]*\.[A-Za-z_][\w-]*(?![\w.-])/g;
const CSS_PARAM = /var\(--x-param-f[0-9a-f]{8}-[\w-]+\)/g;
// A role's spelling as the checker reads it: any case, `-exact-` or not.
const roleKey = name => String(name).toLowerCase().replace(/^-exact-/, '');

/** Each IR param token's param, from `describe`'s `components` (tamber/1.10):
 * its full and short names, and the token it aliases directly (LLP 1122 D6:
 * its value as the app has it, the line's or the default, when that names
 * one declared token, a role override by any spelling of its role). A param
 * with `when` lines aliases none: which branch wins is the copy's. */
export function paramsOf(ir) {
  const names = new Set((ir?.tokens ?? []).map(t => t.name));
  const roles = new Map((ir?.tokens ?? []).filter(t => t.role).map(t => [roleKey(t.role), t.name]));
  const tokenOf = v => (names.has(v) ? v : roles.get(roleKey(v)) ?? null);
  const out = new Map();
  for (const c of ir?.components ?? []) for (const p of c.params ?? []) {
    if (typeof p.token !== 'string') continue;
    const written = String(p.set?.value ?? p.default ?? '').trim();
    const token = (p.set?.whens ?? 0) === 0 && written ? tokenOf(written) : null;
    out.set(p.token, { param: p.name, short: p.short ?? null, token, css: `var(--x-${p.token.replace(/[^A-Za-z0-9_-]/g, '-')})` });
  }
  // Two params whose property would be one (the page suffixes the second,
  // modes.rs) are named by neither: a page's text then says "a component
  // param" rather than another param's name.
  const seen = new Map();
  for (const p of out.values()) seen.set(p.css, (seen.get(p.css) ?? 0) + 1);
  for (const p of out.values()) if (seen.get(p.css) > 1) p.css = null;
  return out;
}

/** `text` with each IR param token in it, or the custom property a page
 * writes for one, named by its param's full name, or "a component param"
 * where `params` has none (not read yet); and whether one was. */
function rename(text, params) {
  let unread = false;
  const css = new Map([...params.values()].filter(p => p.css).map(p => [p.css, p]));
  const out = String(text)
    .replace(IR_PARAM, n => { const p = params.get(n); if (!p) unread = true; return p ? p.param : 'a component param'; })
    .replace(CSS_PARAM, v => { const p = css.get(v); if (!p) unread = true; return p ? p.param : 'a component param'; });
  return { out, unread };
}

/** A row's token report with every IR param token named by its param's full
 * name, in `names` and `text`; a name that holds one the server has not read
 * is left out of `names`, so no client sees it, and the row is `unread`. */
export function named(token, params) {
  if (!token?.names) return { token, unread: false };
  let unread = false;
  const names = [];
  for (const n of token.names) {
    const r = rename(n, params);
    if (r.unread) unread = true;
    else names.push(r.out);
  }
  const text = token.text == null ? token.text : rename(token.text, params).out;
  return { token: { ...token, names, ...(text == null ? {} : { text }) }, unread };
}

/** A `selected` report as the server publishes it: its rows' token names
 * scrubbed of IR param tokens as the join's are. */
export function published(selected, params) {
  if (!selected || typeof selected !== 'object') return selected;
  return { ...selected, ...(selected.rows ? { rows: scrubbed(selected.rows, params) } : {}), ...(selected.held ? { held: heldOf(selected.held, params) } : {}) };
}
// (a `library` is the map's to say, never the host report's: dropped from what the host sent, on every path, a
// row's own and its source's)
const bare = ({ library: _host, ...r }) => (r.source && typeof r.source === 'object' ? { ...r, source: (({ library: _said, ...at }) => at)(r.source) } : r);
// (`fromSite`, a page's word to the server, never leaves: the join names it as `fromAt`.)
const scrubbed = (rows, params) => (rows ?? []).map(bare).map(({ fromSite: _site, ...r }) => (r.token ? { ...r, token: named(r.token, params).token } : r));
// A held state's rows (LLP 1121 stage 8 E), scrubbed as the selection's.
const heldOf = ({ library: _host, ...held }, params) => ({ ...held, rows: scrubbed(held.rows, params) });

// A row by the name its author wrote: an `:active` part's as the press paint carries it (AUTHORED, below).
const authored = name => AUTHORED[name] ?? WRITTEN[name] ?? String(name).replace(/_/g, '-');
// Rows a part may fold whose attribute is named otherwise (contract/lower/src/states.rs RUNNER_ROWS).
const WRITTEN = { text_color: 'color', tint_color: '-exact-tint-color' };
// A state part's row as its author wrote it at its place (`at`, in `lines`): a look's border side is the `border-color`
// written there, its four sides whole, or the side's own longhand (`border-top-color`), as the source says; null for
// another, which no edit of this row may write. Else its name (`authored`).
const writtenAs = (lines, at, name) => {
  const side = /^border_color_(top|right|bottom|left)$/.exec(name)?.[1];
  if (!side) return authored(name);
  // (At the map's UTF-8 byte column, as `plainAt` reads it.)
  const said = /^[a-z-]+/.exec(Buffer.from(lines[at.line - 1] ?? '', 'utf8').subarray(Math.max(0, at.col - 1)).toString('utf8'))?.[0];
  return said === 'border-color' || said === `border-${side}-color` ? said : null;
};

/** A state part's row as `set-state` addresses it (LLP 1122 T4, tamber/1.12):
 * the style whose part writes it, by the name its file declares it with
 * (the compiler may rename a second `style Card` in another file), the
 * state, the row as written, the style's file relative to the app, and the
 * row's value as written there (`text`, what an editor starts from). Null
 * for a row a node writes itself (no style) or under a conditional class;
 * a reason for a style outside the app's own files (a package's, vendored
 * or linked ones included: `own`, the server's own Contract sources by
 * app-relative path, when it is given), which no operation writes. */
export function stateRung(row, b, app, read = readFileSync, own = null) {
  if (!b?.state || !b.at?.file) return null;
  if (!/^class:[A-Za-z_][\w-]*$/.test(b.origin ?? '')) return null;
  const rel = app && b.at.file.startsWith(`${app}/`) ? b.at.file.slice(app.length + 1) : null;
  if (galleryFile(b.at.file)) return { rung: null, reason: GALLERY };
  if (!rel || b.at.file.includes('/node_modules/') || (own && !own.has(rel))) return { rung: null, reason: `a package's style (${b.origin.slice(6)}): read-only here` };
  let lines;
  try { lines = String(read(b.at.file)).split('\n'); } catch { return null; }
  // Its `style` line: the nearest above the part's row, at the line's start.
  const style = styleAbove(lines, b.at.line);
  if (!style) return null;
  const written = writtenAs(lines, b.at, row.name);
  if (!written) return null;
  // The row's value as written, where it is one plain value (`plainAt`); else none, and an editor starts empty.
  const text = plainAt(lines[b.at.line - 1] ?? '', b.at.col, written);
  return { rung: 'state', style, state: b.state, row: written, file: rel, ...(text != null ? { text } : {}) };
}

/** The style a row on `line` (1-based) is written in: the nearest `style Name` above it at the
 * line's start, through indented lines, blank ones and comments; null where another top-level
 * declaration comes first (the row is no style's). */
const styleAbove = (lines, line) => {
  for (let i = line - 2; i >= 0; i--) {
    const l = lines[i] ?? '', m = /^style\s+([A-Za-z_][\w-]*)/.exec(l);
    if (m) return m[1];
    if (/^\S/.test(l) && !/^\/\//.test(l)) return null;
  }
  return null;
};

// The variant gallery's generated source (`contract gallery`, the JS dev loop's build cache: under a node_modules's
// .cache/exact-gallery or the repo's target/dev-gallery), which no operation writes: its tokens and params are the app's.
const galleryFile = (file) => /\/(?:node_modules\/\.cache\/exact-gallery|target\/dev-gallery)\//.test(file ?? '');
const GALLERY = "the variant gallery's generated source: read-only (its tokens and params are the app's)";

// A file of the app's own Contract sources, relative to the app (`own`, the
// server's list, when given), or null for a package's.
const ownFile = (file, app, own) => {
  const rel = app && file?.startsWith(`${app}/`) ? file.slice(app.length + 1) : null;
  return !rel || file.includes('/node_modules/') || (own && !own.has(rel)) ? null : rel;
};

/** A row its author wrote, as `set-row` addresses it (LLP 1122 T5, D6,
 * tamber/1.12): at the node's place, the template rung (origin `own`: every
 * copy of this source node, `instances` of them), or on its style (origin a
 * single `class:<Name>`, by the name its file declares it with: every node
 * using it). `row` is the attribute as written (a shorthand's for a longhand
 * of it: `set-row` edits the shorthand whole), `text` its value. Only a
 * plain value, one string or number as `set-row` replaces, is offered; an
 * expression or a name in scope is the source's, said so. Null where the
 * map gives the row no place (D1's address), or a style's state part
 * (`set-state`'s, T4). */
export function rowRung(row, b, map, app, instances = 1, read = readFileSync, own = null) {
  if (!b?.at?.file) return null;
  const style = /^class:([A-Za-z_][\w-]*)$/.exec(b.origin ?? '')?.[1] ?? null;
  if (b.origin !== 'own' && !style) return null;
  if (style && b.state) return null;
  // (The attribute as written, below: the map's `attr`, or, for a state row under the node (which has none), its
  // authored name at its place: `background-color` for the press paint's `press_background_color`.)
  if (!b.attr && !b.state) return null;
  // A layout row (D14, the map's `layout`): the author's, as `set-row` says; a token or param it reads is offered.
  if (b.layout) return { rung: null, layout: true, reason: `\`${b.attr}\` is a layout row: layout is the author's (LLP 1122 D14); edit it in the source, or at a param's or token's rung where it reads one` };
  const rel = ownFile(b.at.file, app, own);
  if (galleryFile(b.at.file)) return { rung: null, reason: GALLERY };
  if (!rel) return { rung: null, reason: style ? `a package's style (${style}): read-only here` : 'a package\'s component: read-only here' };
  let lines;
  try { lines = String(read(b.at.file)).split('\n'); } catch { return null; }
  const attr = b.attr ?? writtenAs(lines, b.at, row.name);
  if (!attr) return null;
  const text = plainAt(lines[b.at.line - 1] ?? '', b.at.col, attr);
  if (text == null) return { rung: null, reason: `\`${attr}\` is written as an expression or a name in scope, or over more than one line: edit it in the source` };
  if (style) {
    const declared = styleAbove(lines, b.at.line);
    return declared ? { rung: 'style', style: declared, row: attr, file: rel, text } : null;
  }
  const sha256 = (map?.sources ?? []).find(x => x.file === b.at.file)?.sha256;
  if (!sha256) return null;
  // (The component by the name its file declares it with, above the row: a promotion's `add-param` names it.)
  let component = null;
  for (let i = b.at.line - 2; i >= 0 && !component; i--) component = /^component\s+([A-Za-z_][\w-]*)/.exec(lines[i] ?? '')?.[1] ?? null;
  return { rung: 'template', file: rel, at: { line: b.at.line, col: b.at.col }, sha256, row: attr, ...(b.state ? { state: b.state } : {}), instances, text, ...(component ? { component } : {}) };
}

/** A Contract string's text, its escapes decoded one by one as the lexer does (lexer.rs `escape`):
 * `\n`, `\t`, `\"`, `\\`, `` \` ``, `\$`, and `\u{…}` (one to six hex digits) or `\u` four bare; null
 * for another (which the lexer refuses). */
const unescaped = (body) => {
  let out = '';
  for (let i = 0; i < body.length; i++) {
    if (body[i] !== '\\') { out += body[i]; continue; }
    const c = body[++i];
    const simple = { n: '\n', t: '\t', '"': '"', '\\': '\\', '`': '`', $: '$' }[c];
    if (simple != null) { out += simple; continue; }
    if (c !== 'u') return null;
    const m = /^\{([0-9a-fA-F]{1,6})\}|^([0-9a-fA-F]{4})/.exec(body.slice(i + 1));
    const code = m ? parseInt(m[1] ?? m[2], 16) : NaN;
    if (!m || code > 0x10ffff || (code >= 0xd800 && code <= 0xdfff)) return null;
    out += String.fromCodePoint(code); i += m[0].length;
  }
  return out;
};

/** The value of attribute `attr` written at UTF-8 byte column `col` of `line` (the map's columns are
 * bytes), when it is one plain value as `set-row` replaces (tamber_rows.rs `plain`): a string (its
 * escapes decoded) or a bare `#…` colour, which the lexer reads as a string; a number, an exponent
 * and a minus included; inside any parentheses; then the line's end, a comment, another attribute or
 * a positional argument (a string, a template or one in parentheses). Null otherwise (an expression, a
 * name in scope, an unclosed value). */
export function plainAt(line, col, attr) {
  const at = Buffer.from(line, 'utf8').subarray(Math.max(0, col - 1)).toString('utf8');
  const head = new RegExp(`^${attr.replace(/[-]/g, '\\-')}\\s*=\\s*`).exec(at);
  if (!head) return null;
  let rest = at.slice(head[0].length), minus = '', open = 0;
  for (;;) {
    const m = /^(-?)\s*\(\s*/.exec(rest);
    if (!m) break;
    if (m[1]) { if (minus) return null; minus = '-'; }
    open++; rest = rest.slice(m[0].length);
  }
  const lead = /^-\s*/.exec(rest);
  if (lead) { if (minus) return null; minus = '-'; rest = rest.slice(lead[0].length); }
  let text = null, m;
  if ((m = /^"((?:[^"\\]|\\.)*)"/.exec(rest)) && !minus) {
    if ((text = unescaped(m[1])) == null) return null;
  } else if ((m = /^#[0-9A-Za-z]+/.exec(rest)) && !minus) text = m[0];
  else if ((m = /^(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?(?![\w.])/.exec(rest))) text = `${minus}${m[0]}`;
  else return null;
  rest = rest.slice(m[0].length);
  for (; open > 0; open--) { const c = /^\s*\)/.exec(rest); if (!c) return null; rest = rest.slice(c[0].length); }
  // (A positional after: a string, a template, one in parentheses, or a name, not an operator word.)
  return /^(\s*$|\s*\/\/|\s+[\w-]+\s*=|\s*["`(]|\s+(?!(and|or|not|in|is)\b)[A-Za-z_][\w.]*\s*($|\/\/|["`(]|[\w-]+\s*=|[A-Za-z_]))/.test(rest) ? text : null;
}

/** Each row's editability: the token or param rung, or why not (D5, D6). Fails
 * closed: the rung only for a row the node itself sets (`authored`, its own
 * attribute in the map) whose whole value is one token; every other row
 * says why, and links the tokens it reads. */
export function editable(row, origin, params = new Map(), unread = false, state = null, written = null) {
  const names = row.token?.names ?? [];
  // (A param, by its full name, is a link too: the tool opens the param.)
  const isParam = n => [...params.values()].some(p => p.param === n);
  const link = names.find(n => declared(n) || isParam(n)) ?? null;
  if (unread) return { rung: null, reason: 'it reads a component param this server has not read yet: inspect it again', link };
  if (row.source === 'inherited') return { rung: null, reason: `inherited from ${row.fromAt ? `${row.fromAt.component ?? 'a node'} (${String(row.fromAt.file).split('/').pop()}:${row.fromAt.line})` : `#${row.from}`}: the token is that node's to edit`, link };
  if (row.source === 'dynamic') return { rung: null, reason: 'a dynamic expression: read-only here', link };
  if (row.source === 'initial') return { rung: null, reason: 'its initial value: nothing sets it' };
  if (row.source !== 'authored') return { rung: null, reason: `a row whose source is ${row.source ?? 'not known'}: read-only here`, link };
  if (row.token?.implied) return { rung: null, reason: 'a mode boundary sets it: read-only here', link };
  if (origin == null) return { rung: null, reason: 'the source map does not say where this row is set: read-only', link };
  // A style's state part row (T4): the state rung, the default (D6); the
  // token its whole value is, offered beside it, never the default.
  if (state) {
    if (!state.rung) return { ...state, link };
    const text = String(row.token?.text ?? '').trim(), one = names.length === 1 && declared(names[0]) && (text === names[0] || text.replace(/^-exact-/, '') === names[0]);
    return one ? { ...state, token: names[0] } : state;
  }
  if (origin.startsWith('class:') && origin.includes('/')) return { rung: null, reason: `a conditional class (${origin.slice(6)}): refused here`, link };
  // Its whole value is the token: its name, or the role spelling the runner
  // resolved to it (`-exact-secondary-label` for `secondary-label`).
  const text = String(row.token?.text ?? '').trim();
  const one = names.length === 1 && declared(names[0]) && (text === names[0] || text.replace(/^-exact-/, '') === names[0]);
  // A component's param, its whole value (exact1's default rung, D6).
  const param = names.length === 1 && text === names[0] ? [...params.values()].find(p => p.param === names[0]) : null;
  // (Its template's address too, where the map places the row: a detach writes there, T5.)
  if (param) return { rung: 'param', param: param.param, short: param.short, ...(param.token ? { token: param.token } : {}), modes: row.token.modes ?? '', ...(written?.rung === 'template' ? { template: written } : {}) };
  // Else the template or style rung, where the map places the row (T5): the
  // default (D6), the token its whole value is offered beside it.
  if (written?.rung) return one ? { ...written, token: names[0], modes: row.token.modes ?? '' } : written;
  // (A row set-row can't take, a layout row or one written otherwise, still offers the token it reads, whole.)
  if (written && one) return { rung: 'token', token: names[0], modes: row.token.modes ?? '' };
  if (written) { const { layout, ...why } = written; return { ...why, link }; }
  if (origin.startsWith('class:')) return { rung: null, reason: `a style's row (${origin.slice(6)}) the map gives no place for: the source's to edit`, link };
  if (origin !== 'own') return { rung: null, reason: 'the tag\'s own row: the source\'s to edit', link };
  // @ref LLP 1121 D8 — an `:active` row lowered to the press paint: its part's to edit.
  if (!names.length && row.state) return { rung: null, reason: `an inline \`:${row.state}\` row on the node the map gives no place for: the source's to edit` };
  if (!names.length) return { rung: null, reason: 'a value the map gives no place for: the source\'s to edit' };
  if (!one) return { rung: null, reason: `an expression over tokens (${names.join(', ')}): edit a token itself`, link };
  return { rung: 'token', token: names[0], modes: row.token.modes ?? '' };
}

// The press rows `:active` lowers to (LLP 1121 D8 as amended; contract/lower/src/states.rs ACTIVE_ROWS), each by
// the row its author wrote under `:active`.
const AUTHORED = { press_background_color: 'background-color', press_color: 'color', press_opacity_to: 'opacity', press_scale: '-exact-press-scale' };
// A map binding as the rungs read it (theme2's 02:46 and 03:18 rulings): one row's base value, its `origin`, `at`, `attr`
// and `layout`, absent where no base row writes it; and `states`, each state part writing it. `asPart` gives a part's
// as `{row, origin, state, at}`, `asBase` the base's: never one for the other.
const asPart = (b, state) => { const p = b?.states?.find(s => s.state === state); return p ? { row: b.row, origin: p.origin, state: p.state, at: p.at } : null; };
const asBase = b => (b?.origin ? b : null);

/** A build's selection joined to source, for the `selection` event, from
 * `map` (null when the build's map is not here). */
export function join(build, selected, map, { read = readFileSync, served = null, design = null, params = new Map(), app = null, own = null } = {}) {
  const base = { build: build.id, node: selected.node, site: selected.site, instance: selected.instance, tuple: selected.tuple, trimmed: !!selected.trimmed, ...(selected.held ? { held: heldOf(selected.held, params) } : {}) };
  // (a `library` is the map's to say, never the host report's: dropped from what the host sent)
  const rows = (selected.rows ?? []).map(bare).map(r => { const n = named(r.token, params); return { ...r, ...(r.token ? { token: n.token } : {}), unread: n.unread }; });
  if (!map) return { ...base, source: null, rows: rows.map(({ unread, fromSite: _site, ...r }) => ({ ...r, editable: { rung: null, reason: 'no source map for this build\'s plan' } })), readOnly: 'no source map for this build\'s plan' };
  const node = map.nodes[selected.site];
  if (!node) return { ...base, source: null, rows: rows.map(({ unread, fromSite: _site, ...r }) => ({ ...r, editable: { rung: null, reason: 'the site is not in the source map' } })), readOnly: 'the site is not in the source map' };
  const bound = new Map((node.bindings ?? []).map(b => [b.row, b]));
  // The node a state is held on, named on this one inside it (stage 9 E): its rows' places are its own.
  const owner = base.held && Number.isSafeInteger(base.held.site) ? map.nodes[base.held.site] ?? null : node;
  // An `:active` row is written in its part (LLP 1121 D8): its file counts too, and the held node's.
  const placed = n => n ? [n.file, ...(n.chain ?? []).map(c => c.file), ...(n.bindings ?? []).flatMap(b => [b.at, ...(b.states ?? []).map(p => p.at)].flatMap(at => at ? [at.file] : []))] : [];
  const files = [...new Set([...placed(node), ...(owner !== node ? placed(owner) : [])])];
  const readOnly = files.map(f => moved(map, f, read, app)).find(Boolean) || designMoved(map, build, served, design, read, app) || null;
  // The exported library component whose own view the node is of, and its part (LLP 1139 D5, *1.15*): from the map
  // (`part=` never reaches a host), beside `component`, the map's bare declared name (two packages can share one; an
  // alias the app imports it by is in neither); absent outside one (a slot's fill is the caller's).
  const libraryOf = n => (typeof n?.library === 'string' ? { identity: n.library, part: typeof n.part === 'string' ? n.part : null } : null);
  const library = libraryOf(node);
  const source = { file: node.file, line: node.line, col: node.col, end_col: node.end_col, component: node.component, chain: node.chain ?? [], ...(library ? { library } : {}) };
  // A source node's copies (a template edit is every one's): one a call-site chain, as a hold's replay
  // counts them (a `button "…"`'s label shares its span and chain, and is no second copy).
  const copiesOf = n => Math.max(1, new Set(sitesAt(map, n).filter(i => same(map.nodes[i], n)).map(i => JSON.stringify((map.nodes[i].chain ?? []).map(c => [c.file, c.line, c.col])))).size);
  const copies = copiesOf(node);
  // What is held (LLP 1122 D8): each row where it is written, its part's place (the map's `states`, keyed by row and
  // state): an `:active` row lowered to the press paint, a `:hover` or `:focus-visible` look's row (queue item (1)). Its node is the one it is held on
  // (the press it resolved to, named on a node inside it: stage 9 E), its site's map node.
  if (base.held) {
    const ownerBound = owner === node ? bound : new Map((owner?.bindings ?? []).map(b => [b.row, b]));
    // (the held node's own library and part, where it is of a library component's view)
    const ownerLibrary = owner === node ? library : libraryOf(owner);
    const ownerSource = owner === node ? source : owner && { file: owner.file, line: owner.line, col: owner.col, end_col: owner.end_col, component: owner.component, chain: owner.chain ?? [], ...(ownerLibrary ? { library: ownerLibrary } : {}) };
    // Edited where it is held (T4): only where the build names that node (its held site); else shown, not offered.
    const heldHere = Number.isSafeInteger(base.held.site) && !!owner;
    base.held = { ...base.held, rows: (base.held.rows ?? []).map((r, i) => {
      const b = asPart(ownerBound.get(r.name), base.held.state) ?? asBase(ownerBound.get(r.name));
      // An `:active` row is named as its author wrote it, under its part (theme2's 16:13 (2)), the press row it
      // lowered to beside it (`row`).
      const authored = b?.state ? AUTHORED[r.name] ?? null : null;
      // (A held look's row is its part's, never its base row's: a row the map gives no part in the held state is
      // the base's, which the look is not written at.)
      // (Whether it reads a param not yet read: from the host's own report, before its names were scrubbed.)
      const unread = named(selected.held.rows?.[i]?.token, params).unread;
      const offered = !heldHere || readOnly ? null : editable({ ...r, source: 'authored', state: b?.state }, b?.origin ?? null, params, unread, stateRung(r, b, app, read, own), b?.state ? rowRung(r, b, map, app, copiesOf(owner), read, own) : null);
      return { ...r, ...(authored ? { name: authored, row: r.name } : {}), state: base.held.state, source: b?.state && b.at ? b.at : ownerSource ?? null, ...(b?.origin && ownerLibrary ? { library: ownerLibrary } : {}), ...(offered ? { editable: offered } : {}) };
    }) };
  }
  return {
    ...base, source, readOnly,
    rows: rows.map(r => {
      const b = asBase(bound.get(r.name)) ?? asPart(bound.get(r.name), 'active');
      const origin = b?.origin ?? null;
      // A state part's row, where it is written (the map's `state` and `at`); an
      // own or class row, the attribute it came from and that file's digest as
      // compiled: `set-row`'s address (LLP 1122 D6, tamber/1.12).
      const sha = f => (map.sources ?? []).find(s => s.file === f)?.sha256;
      const written = b?.at && !b.state ? { at: { ...b.at, ...(sha(b.at.file) ? { sha256: sha(b.at.file) } : {}) }, attr: b.attr, ...(b.layout ? { layout: true } : {}) } : {};
      const part = b?.state ? { state: b.state, ...(b.at ? { at: b.at } : {}) } : written;
      // An inherited row's ancestor, named by its source (tamber/1.16 `fromAt`) where the page gave its plan site.
      const { unread, fromSite, ...given } = r;
      const fromNode = given.source === 'inherited' && Number.isSafeInteger(fromSite) ? map.nodes?.[fromSite] : null;
      const row = fromNode?.file ? { ...given, fromAt: { file: fromNode.file, line: fromNode.line, component: fromNode.component ?? null } } : given;
      // A folded `:disabled` or `:checked` row (queue item (2)) is the runtime's choice over its base and its parts,
      // so `dynamic`: where the map says both, it is read as its base, its rung the base's, each part beside it in
      // `states` with its own (theme2's and review2's 04:24). One the map can't explain stays dynamic, read-only.
      const folds = asBase(bound.get(r.name)) && row.source === 'dynamic' ? (bound.get(r.name).states ?? []).filter(p => p.state === 'disabled' || p.state === 'checked') : [];
      const rung = rowRung(row, b, map, app, copies, read, own);
      // (Its base's token, from the base as written, whichever branch the runtime shows now.)
      const base = folds.length ? writtenToken(row, writtenAt(b, b.attr, read, map), params) : null;
      const shown = base ? { ...row, source: 'authored', token: base.token } : row;
      const offered = editable({ ...shown, ...part }, origin, params, unread || !!base?.unread, stateRung(row, b, app, read, own), rung);
      const shut = o => (readOnly ? { rung: null, reason: readOnly, link: o.link ?? o.param ?? o.token ?? null } : o);
      const states = folds.map(p => {
        const v = { row: r.name, origin: p.origin, state: p.state, at: p.at }, w = rowRung(row, v, map, app, copies, read, own), st = stateRung(row, v, app, read, own);
        // A part's edit is that part's alone: never a param its value names, which the base and every other use share
        // (nor refused for one the row the host shows reads).
        const e = editable({ ...row, source: 'authored', token: writtenToken(row, writtenAt(v, null, read), params, true).token, state: p.state }, p.origin, params, false, st, w);
        // (Its place; a part with none, a conditional class's, its node's.)
        const at = p.at ?? node;
        return { state: p.state, source: { file: at.file, line: at.line, col: at.col, end_col: at.end_col }, editable: shut(e) };
      });
      return { ...row, origin, ...part, ...(origin && library ? { library } : {}), editable: shut(offered), ...(folds.length ? { states } : {}) };
    }),
  };
}

/** A row's value as written at its place (`b.at`), its base attribute's (`attr`) or, for a part, by the name written
 * there (`writtenAs`), the component it is written in (the top-level `component` above it, none under another
 * declaration), and that component's identity (`<home>#<component>`, its file's home as `map` says it); null where
 * the file can't be read or the place says no such name. Read whether or not the file is the app's own: a param a
 * package's component reads is overridden in the app's design. */
function writtenAt(b, attr, read, map = null) {
  if (!b?.at?.file) return null;
  let lines;
  try { lines = String(read(b.at.file)).split('\n'); } catch { return null; }
  const name = attr ?? writtenAs(lines, b.at, b.row);
  if (!name) return null;
  let component = null;
  for (let i = b.at.line - 2; i >= 0; i--) {
    const l = lines[i] ?? '';
    if (/^\S/.test(l) && !/^\/\//.test(l)) { component = /^component\s+([A-Za-z_][\w-]*)/.exec(l)?.[1] ?? null; break; }
  }
  const home = (map?.sources ?? []).find(x => x.file === b.at.file)?.home;
  return { text: plainAt(lines[b.at.line - 1] ?? '', b.at.col, name), component, identity: home && component ? `${home}#${component}` : null };
}

/** The token a folded row's base or part reads, from its value as written (`w`, `writtenAt`'s), whichever branch the
 * runtime shows now (queue item (2)): a param, one `param.<name>`, by its full name, its component's identity and
 * the name (`<home>#<component>.<name>`, as the compiler names it), among the params read; never for a part
 * (`part`), whose edit is its own; a token by its name or its role's spelling (any case, `-exact-`) where the row's
 * names hold it; else none. `unread` where that param is not read (yet, or the map names no home for its file). */
function writtenToken(row, w, params, part = false) {
  const names = row.token?.names ?? [], modes = row.token?.modes ?? '';
  const text = typeof w?.text === 'string' ? w.text.trim() : null;
  const none = { token: { names: [], text: text ?? '', modes } };
  if (text == null) return none;
  const name = /^param\.([A-Za-z_][\w-]*)$/.exec(text)?.[1];
  if (name) {
    if (part) return none;
    const full = w.identity ? `${w.identity}.${name}` : null;
    const param = full && [...params.values()].find(p => p.param === full);
    return param ? { token: { names: [param.param], text: param.param, modes } } : { ...none, unread: true };
  }
  // (A role's `-exact-` spelling in any case, as the kernel parses it; any other name exactly: a custom token's case
  // is its own, so `Brand.Fill` is never `brand.fill`.)
  const low = text.toLowerCase(), token = names.find(n => n === text || (low.startsWith('-exact-') && low.slice(7) === n));
  return token ? { token: { names: [token], text: token, modes } } : none;
}

/** The sites of `map` at `source`'s location: every copy of that node. */
export function sitesAt(map, source) {
  if (!map || !source) return [];
  const here = x => x && x.file === source.file && x.line === source.line && x.col === source.col;
  // A node, or a state part's row on it (`uses` names that row's own place).
  return map.nodes.flatMap((n, site) => here(n) || (n.bindings ?? []).some(b => (b.states ?? []).some(p => here(p.at))) ? [site] : []);
}

/** The one site of plan map `to` that is site `site` of plan map `from`
 * (a held node across a rebuild, LLP 1122 D8), by the node's copies: the
 * sites at its source location, one copy a call-site chain (a `button
 * "…"` and its label, or a slot at two outlets, are several sites of one
 * copy, told apart by their order). The same calls in both plans: the copy
 * with this one's chain. As many copies, their calls moved (a line written
 * above them): the copy of the same rank in source order, where each pair
 * is of one shape (its calls' files and components), each file's calls
 * moved by one count of lines (so none moved past another) and the copies
 * share calls alike (two under one call in one plan are so in the other);
 * else the copies whose calls did not move are themselves, and the rest go
 * by that rule among themselves (one call moved: the one left over). With
 * copies added or removed, the copy whose calls did not move. Null when
 * none, or no single one, corresponds. */
export function correspondingSite(from, to, site) {
  const node = from?.nodes?.[site];
  if (!node || !to?.nodes) return null;
  const at = x => [x.file, x.line, x.col].join(':');
  const key = n => (n.chain ?? []).map(at).join('>');
  // Each plan's copies (the sites of each chain), in source order.
  const copies = map => {
    const by = new Map();
    map.nodes.forEach((n, i) => { if (same(n, node)) by.set(key(n), [...(by.get(key(n)) ?? []), i]); });
    return [...by.values()].sort((a, b) => cmp(map.nodes[a[0]].chain ?? [], map.nodes[b[0]].chain ?? []));
  };
  const was = copies(from), now = copies(to);
  const g = was.findIndex(c => c.includes(site));
  const chainOf = (map, c) => map.nodes[c[0]].chain ?? [];
  const wasKeys = was.map(c => key(from.nodes[c[0]])), nowKeys = now.map(c => key(to.nodes[c[0]]));
  // This site among its copy's sites: the same place in a copy of as many.
  const pick = c => (c && c.length === was[g].length ? c[was[g].indexOf(site)] : null);
  if (was.length === now.length && wasKeys.every(k => nowKeys.includes(k))) return pick(now[nowKeys.indexOf(wasKeys[g])]);
  // A pairing of old copies with new ones holds where each pair is of one
  // shape, the pairs share calls alike (two under one call in one plan are
  // so in the other), and each file's calls moved as one edit there moves
  // them: those above it by none, those below by one count of lines (a call
  // moved past another, or a pairing that crosses, moves them otherwise).
  const shape = ch => ch.map(c => `${c.file}#${c.component ?? ''}`).join('>');
  const place = x => (x ? at(x) : '');
  const oneEdit = moves => {
    const files = new Map();
    for (const [x, y] of moves) files.set(x.file, [...(files.get(x.file) ?? []), [x.line, y.line - x.line]]);
    return [...files.values()].every(list => {
      const d = list.sort((a, b) => a[0] - b[0]).map(([, k]) => k), last = d.at(-1);
      return d.every((k, i) => k === last || (k === 0 && d.slice(0, i).every(z => z === 0)));
    });
  };
  const holds = pairs => {
    const o = ([c]) => chainOf(from, was[c]), m = ([, n]) => chainOf(to, now[n]);
    return pairs.every(p => shape(o(p)) === shape(m(p)))
      && oneEdit(pairs.flatMap(p => o(p).map((x, j) => [x, m(p)[j]])))
      && pairs.every(p => pairs.every(q => o(p).every((x, j) => (at(x) === place(o(q)[j])) === (place(m(p)[j]) === place(m(q)[j])))));
  };
  const all = list => list.map((_, i) => i);
  // As many copies: all by rank; else the unmoved ones themselves and the
  // rest by rank among themselves (one call moved: the one left over).
  if (was.length === now.length) {
    const byRank = all(was).map(i => [i, i]);
    if (holds(byRank)) return pick(now[g]);
    const w = all(was).filter(i => !nowKeys.includes(wasKeys[i])), n = all(now).filter(i => !wasKeys.includes(nowKeys[i]));
    const moved = w.map((c, i) => [c, n[i]]), kept = all(was).filter(i => !w.includes(i)).map(i => [i, nowKeys.indexOf(wasKeys[i])]);
    if (w.includes(g)) return holds([...kept, ...moved]) ? pick(now[n[w.indexOf(g)]]) : null;
  }
  return pick(now[nowKeys.indexOf(wasKeys[g])]);
}
// The same node, by its place (a lowered state row's binding is not it).
const same = (a, b) => a && a.file === b.file && a.line === b.line && a.col === b.col;
// Two call-site chains in source order: by each call's file, line, then column.
const cmp = (a, b) => { for (let k = 0; k < Math.min(a.length, b.length); k++) { const d = (a[k].file < b[k].file ? -1 : a[k].file > b[k].file ? 1 : 0) || (a[k].line - b[k].line) || (a[k].col - b[k].col); if (d) return d; } return a.length - b.length; };

/** Whether a report is the build's latest: its plan, design and pin
 * revision those of its latest status. */
export function current(build, report) {
  // (And its plan digest, when both say one: T2c.)
  const digest = report.selected?.planDigest ?? null;
  return (report.plan ?? null) === (build.plan ?? null) && (report.design ?? null) === (build.design ?? null)
    && (report.revision ?? null) === (build.status?.revision ?? null) && (!digest || !build.planDigest || digest === build.planDigest);
}

/** Why `map` is not the sources as they are now (a file it compiled has
 * changed or gone since), or '': a span from a fresh compile (`uses`) is
 * looked up in it only while it is. */
export function staleMap(map, read = readFileSync, app = null) {
  for (const { file, sha256: digest } of map.sources ?? []) {
    // A design edit moves no span (and a live one keeps the plan).
    if (!file.startsWith('/') || /\.tamber$/.test(file)) continue;
    const shown = app ? pathName(file, app) : file;
    let now;
    try { now = sha256(read(file)); } catch { return `${shown} is gone since the running plan`; }
    if (now !== digest) return `${shown} changed since the running plan: outline once it has reloaded`;
  }
  return '';
}

/** The plan sites a move `n` call sites up from `site` may land on (tamber/1.16 `inspect` `select`), as levels, nearest
 * first: the nodes of each enclosing copy from the `n`th call out to the app's own, whose call-site chain is the
 * site's with its innermost calls taken off, each level that owns a node. A page selects in the first level that has a
 * node above its selection, by its own tree: a component that only renders another, or one whose nodes are beside the
 * selection, is passed over, and past the outermost call it lands on the root. */
export function levelsUp(map, site, n) {
  const chain = map.nodes?.[site]?.chain;
  if (!Array.isArray(chain)) return [];
  const key = c => `${c.file}:${c.line}:${c.col}`, chainOf = node => (node?.chain ?? []).map(key).join('>');
  const levels = [];
  for (let k = Math.min(n, chain.length); k <= chain.length; k++) {
    const want = chain.slice(k).map(key).join('>'), found = map.nodes.flatMap((node, i) => (chainOf(node) === want ? [i] : []));
    if (found.length) levels.push(found);
  }
  return levels;
}
