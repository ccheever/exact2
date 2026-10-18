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
// - **What is editable.** D6's token and param rungs (the rest come with
//   T5): offered for a row the node itself sets (`authored`, its own
//   attribute) to one token, or to one component param (LLP 1122 T3, the
//   param named in full, never by the IR token the plan names it by); a row
//   reached by inheritance is a link to its token, never this node's edit;
//   a style's or the tag's row comes with template editing (T5); anything
//   else is read-only, with its reason.
// - **Outline.** Each build that can inspect is told the sites of its own
//   plan at the selection's source location (`outline {sites}`), every copy
//   of the node.
import { createHash } from 'node:crypto';
import { request } from 'node:http';
import { readFileSync, realpathSync } from 'node:fs';

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
function moved(map, file, read) {
  const known = (map.sources ?? []).find(s => s.file === file);
  if (!known) return `${file} is not in the build's source map`;
  // A compiled-in module is nowhere on disk, and cannot move.
  if (!file.startsWith('/')) return '';
  let now;
  try { now = sha256(read(file)); } catch { return `${file} is gone since this build`; }
  return now === known.sha256 ? '' : `${file} changed since this build: select it again once it has reloaded`;
}

/** Why the design a token rung would edit is not the one the build runs,
 * or '': the app's design file (`file`, as the server names it) as the map
 * compiled it, or, changed since, as it was when the dev loop took the
 * design it serves now (`served {digest, source}`, a value edit the build
 * took live), when the build reports running that. Fails closed: a design
 * the map does not name, or one that cannot be read, is a reason. */
function designMoved(map, build, served, file, read) {
  // Gone from where the server looks (a deleted file): the map's own entry,
  // whose read then fails.
  file ??= (map.sources ?? []).find(s => /\.tamber$/.test(s.file))?.file ?? null;
  if (!file) return '';
  const real = f => { try { return realpathSync(f); } catch { return f; } };
  const known = (map.sources ?? []).find(s => s.file === file || real(s.file) === real(file));
  if (!known) return `the app's design ${file} is not in the build's source map`;
  let now;
  try { now = sha256(read(file)); } catch { return `${file} is gone since this build`; }
  if (now === known.sha256) return '';
  if (served?.digest && served.source === now && build.design === served.digest) return '';
  return `${file} changed since this build, which has not taken it: select again once it has`;
}

// A declared token's name (a dotted word, a role's spelling); a synthetic
// token is named by its whole expression, and is no token to edit.
const declared = name => /^-?[A-Za-z_][\w.-]*$/.test(name);

// The IR's name for a component param (LLP 1121 D15: `param.f<hash>.Card.x`),
// which no author or tool writes; anywhere in a name, since a synthetic
// token is named by its whole expression.
const IR_PARAM = /param\.f[0-9a-f]+\.[A-Za-z_][\w-]*(?:\.[A-Za-z_][\w-]*)*/g;

/** Each IR param token's param, from `describe`'s `components` (tamber/1.10):
 * its full and short names, and the token it aliases directly (LLP 1122 D6:
 * its value as the app has it, the line's or the default, when that names
 * one declared token, a role override by its role's spelling too). A param
 * with `when` lines aliases none: which branch wins is the copy's. */
export function paramsOf(ir) {
  const tokens = new Map();
  for (const t of ir?.tokens ?? []) { tokens.set(t.name, t.name); if (t.role) tokens.set(t.role, t.name); }
  const out = new Map();
  for (const c of ir?.components ?? []) for (const p of c.params ?? []) {
    if (typeof p.token !== 'string') continue;
    const written = String(p.set?.value ?? p.default ?? '').trim();
    const token = (p.set?.whens ?? 0) === 0 ? tokens.get(written) ?? null : null;
    out.set(p.token, { param: p.name, short: p.short ?? null, token });
  }
  return out;
}

/** `text` with each IR param token in it named by its param's full name,
 * or "a component param" where `params` has none (not read yet); and
 * whether one was. */
function rename(text, params) {
  let unread = false;
  const out = String(text).replace(IR_PARAM, n => { const p = params.get(n); if (!p) unread = true; return p ? p.param : 'a component param'; });
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
  if (!selected?.rows) return selected;
  return { ...selected, rows: selected.rows.map(r => (r.token ? { ...r, token: named(r.token, params).token } : r)) };
}

/** Each row's editability: the token or param rung, or why not (D5, D6). Fails
 * closed: the rung only for a row the node itself sets (`authored`, its own
 * attribute in the map) whose whole value is one token; every other row
 * says why, and links the tokens it reads. */
export function editable(row, origin, params = new Map(), unread = false) {
  const names = row.token?.names ?? [];
  // (A param, by its full name, is a link too: the tool opens the param.)
  const isParam = n => [...params.values()].some(p => p.param === n);
  const link = names.find(n => declared(n) || isParam(n)) ?? null;
  if (unread) return { rung: null, reason: 'it reads a component param this server has not read yet: inspect it again', link };
  if (row.source === 'inherited') return { rung: null, reason: `inherited from #${row.from}: the token is that node's to edit`, link };
  if (row.source === 'dynamic') return { rung: null, reason: 'a dynamic expression: read-only here', link };
  if (row.source === 'initial') return { rung: null, reason: 'its initial value: nothing sets it' };
  if (row.source !== 'authored') return { rung: null, reason: `a row whose source is ${row.source ?? 'not known'}: read-only here`, link };
  if (row.token?.implied) return { rung: null, reason: 'a mode boundary sets it: read-only here', link };
  if (origin == null) return { rung: null, reason: 'the source map does not say where this row is set: read-only', link };
  if (origin.startsWith('class:') && origin.includes('/')) return { rung: null, reason: `a conditional class (${origin.slice(6)}): refused here`, link };
  if (origin.startsWith('class:')) return { rung: null, reason: `a style's row (${origin.slice(6)}): comes with template editing (T5)`, link };
  if (origin !== 'own') return { rung: null, reason: 'the tag\'s own row: comes with template editing (T5)', link };
  // @ref LLP 1121 D8 — an `:active` row lowered to the press paint: its part's to edit.
  if (!names.length && row.state) return { rung: null, reason: `a \`:${row.state}\` part's row: \`set-state\` edits it` };
  if (!names.length) return { rung: null, reason: 'a value, not a token: setting a row comes with T5' };
  // Its whole value is the token: its name, or the role spelling the runner
  // resolved to it (`-exact-secondary-label` for `secondary-label`).
  const text = String(row.token.text ?? '').trim();
  // A component's param, its whole value (exact1's default rung, D6).
  const param = names.length === 1 && text === names[0] ? [...params.values()].find(p => p.param === names[0]) : null;
  if (param) return { rung: 'param', param: param.param, short: param.short, ...(param.token ? { token: param.token } : {}), modes: row.token.modes ?? '' };
  if (names.length > 1 || !declared(names[0]) || (text !== names[0] && text.replace(/^-exact-/, '') !== names[0])) return { rung: null, reason: `an expression over tokens (${names.join(', ')}): edit a token itself`, link };
  return { rung: 'token', token: names[0], modes: row.token.modes ?? '' };
}

/** A build's selection joined to source, for the `selection` event, from
 * `map` (null when the build's map is not here). */
export function join(build, selected, map, { read = readFileSync, served = null, design = null, params = new Map() } = {}) {
  const base = { build: build.id, node: selected.node, site: selected.site, instance: selected.instance, tuple: selected.tuple, trimmed: !!selected.trimmed, ...(selected.held ? { held: selected.held } : {}) };
  const rows = (selected.rows ?? []).map(r => { const n = named(r.token, params); return { ...r, ...(r.token ? { token: n.token } : {}), unread: n.unread }; });
  if (!map) return { ...base, source: null, rows: rows.map(({ unread, ...r }) => ({ ...r, editable: { rung: null, reason: 'no source map for this build\'s plan' } })), readOnly: 'no source map for this build\'s plan' };
  const node = map.nodes[selected.site];
  if (!node) return { ...base, source: null, rows: rows.map(({ unread, ...r }) => ({ ...r, editable: { rung: null, reason: 'the site is not in the source map' } })), readOnly: 'the site is not in the source map' };
  const bound = new Map((node.bindings ?? []).map(b => [b.row, b]));
  // An `:active` row is written in its part (LLP 1121 D8): its file counts too.
  const files = [...new Set([node.file, ...(node.chain ?? []).map(c => c.file), ...(node.bindings ?? []).flatMap(b => b.at ? [b.at.file] : [])])];
  const readOnly = files.map(f => moved(map, f, read)).find(Boolean) || designMoved(map, build, served, design, read) || null;
  const source = { file: node.file, line: node.line, col: node.col, end_col: node.end_col, component: node.component, chain: node.chain ?? [] };
  // What is held (LLP 1122 D8): each row where it is written: an `:active` row lowered to the press paint at its
  // part's place (the map's `at`), a state look's at its node, with the state.
  if (base.held) base.held = { ...base.held, rows: (base.held.rows ?? []).map(r => {
    const b = bound.get(r.name);
    return { ...r, state: base.held.state, source: b?.state ? b.at ?? null : source };
  }) };
  return {
    ...base, source, readOnly,
    rows: rows.map(r => {
      const b = bound.get(r.name);
      const origin = b?.origin ?? null;
      // A state part's row, where it is written (the map's `state` and `at`).
      const part = b?.state ? { state: b.state, at: b.at ?? null } : {};
      const { unread, ...row } = r;
      const offered = editable({ ...row, ...part }, origin, params, unread);
      return { ...row, origin, ...part, editable: readOnly ? { rung: null, reason: readOnly, link: offered.link ?? offered.param ?? offered.token ?? null } : offered };
    }),
  };
}

/** The sites of `map` at `source`'s location: every copy of that node. */
export function sitesAt(map, source) {
  if (!map || !source) return [];
  const here = x => x && x.file === source.file && x.line === source.line && x.col === source.col;
  // A node, or a state part's row it lowered (`uses` names that row's own place).
  return map.nodes.flatMap((n, site) => here(n) || (n.bindings ?? []).some(b => here(b.at)) ? [site] : []);
}

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
export function staleMap(map, read = readFileSync) {
  for (const { file, sha256: digest } of map.sources ?? []) {
    // A design edit moves no span (and a live one keeps the plan).
    if (!file.startsWith('/') || /\.tamber$/.test(file)) continue;
    let now;
    try { now = sha256(read(file)); } catch { return `${file} is gone since the running plan`; }
    if (now !== digest) return `${file} changed since the running plan: outline once it has reloaded`;
  }
  return '';
}
