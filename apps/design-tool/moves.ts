// Moving a row between D6's rungs (LLP 1122 T5): promote a row its author
// wrote to a new param or token, or detach a param's row to its template,
// each one transaction the server checks and undoes in one step.
import type { ParamInfo } from './rescope';

/** A value the server takes as a number (else a Contract string). */
export const NUMBER = /^-?(\d+\.?\d*|\.\d+)(e[+-]?\d+)?$/i;

// A param named in full, as `describe` gives a default that reads one: `card.contract#Card.fill`; and a
// param's own full name, whole (its file's path may hold any character, a space too).
const FULL = /([\w./-]+\.(?:contract|tamber))#([A-Za-z_][\w-]*)\.([A-Za-z_][\w-]*)/;
const PARAM = /^(.+\.(?:contract|tamber))#([A-Za-z_][\w-]*)\.([A-Za-z_][\w-]*)$/;

/** A param's value as a literal a row can take: its default where the design sets none; through a
 * default that is one other param, that one's (a chain `b → a`, whose closure the detach removes);
 * null where the design sets it or the default is an expression over another param, said by the
 * proposal. */
const literalOf = (param: string, params: Map<string, ParamInfo>, depth = 0): string | null => {
  const p = params.get(param), [, file, component] = PARAM.exec(param) ?? [];
  if (!p || p.set !== null || depth > 4) return null;
  const whole = new RegExp(`^${FULL.source}$`).exec(p.default.trim()), short = /^param\.([A-Za-z_][\w-]*)$/.exec(p.default.trim());
  if (whole || short) return literalOf(whole ? whole[0] : `${file}#${component}.${short![1]}`, params, depth + 1);
  // (A default written over another param, by its full name or as `param.x`, is no literal a row can take.)
  return FULL.test(p.default) || /(^|[^\w.])param\.[A-Za-z_]/.test(p.default) ? null : p.default;
};

/** A row moved between rungs (LLP 1122 D6's enumerated set, T5), one transaction: promoted to a
 * new param (`add-param` in the template's file, its kind the row's, and the row reading
 * `param.<name>`) or a new token (`add-token` and the row naming it), each with the value as written
 * as its default; or a param's row detached to its template (the param's value written at the row,
 * and `remove-param`, in the param's own file, for it unless another row still reads it, `keep`,
 * and for each param the server says it alone left read, `extra`). None, said by the proposal,
 * where a detach can't: the design sets the param, or its default is an expression over another. */
export function moveOps(file: string, kind: string, t: any, name: string, params: Map<string, ParamInfo>, kinds: Map<string, string>): Record<string, unknown>[] {
  const as = (v: string) => (NUMBER.test(v) ? Number(v) : v);
  const k = kindOf(t.addr?.row ?? '', t.text ?? '', kinds), value = k === 'length' && NUMBER.test(t.text ?? '') ? `${t.text}px` : t.text;
  // (`component` is the name its file declares it with, which the join read above the row.)
  if (kind === 'promote-param') return [{ op: 'add-param', component: t.component, name, kind: k, default: value, file: t.addr.file }, { op: 'set-row', ...t.addr, value: `param.${name}` }];
  if (kind === 'promote-token') return [{ op: 'add-token', name, kind: k, value, ...(file ? { file } : {}) }, { op: 'set-row', ...t.addr, value: name }];
  const [, from, component, short] = PARAM.exec(t.param) ?? [], literal = literalOf(t.param, params);
  if (literal == null || !component) return [];
  const gone = [...(t.keep ? [] : [short]), ...(t.extra ?? [])];
  return [{ op: 'set-row', ...t.addr, value: as(literal) }, ...gone.map(n => ({ op: 'remove-param', component, name: n, file: from }))];
}

/** What a detach's refusal says of its params (diagnostics naming them in full or by component):
 * the ones left unread (`tamber-param-unused`), of the detached param's own component and file, not
 * yet removed; and whether the detached param is still read by another row (`tamber-param-unknown`
 * for it once removed), so it is kept. */
export const unread = (error: string, target: string, ops: Record<string, unknown>[]) => {
  const [, file, component, short] = PARAM.exec(JSON.parse(target).param) ?? [];
  const gone = new Set(ops.filter(o => o.op === 'remove-param').map(o => String(o.name)));
  const named = (line: string) => [...line.matchAll(/`(?:([^`#]+)#)?([A-Za-z_][\w-]*)\.([A-Za-z_][\w-]*)`/g)].filter(m => m[2] === component && (!m[1] || m[1] === file)).map(m => m[3]);
  const lines = error.split('\n');
  return {
    more: [...new Set(lines.filter(l => l.includes('tamber-param-unused')).flatMap(named))].filter(n => !gone.has(n)),
    keep: gone.has(short) && lines.some(l => l.includes('tamber-param-unknown') && (named(l).includes(short) || l.includes(`param.${short}`))),
  };
};

/** A new param's or token's kind: the row's (LLP 1121 D15's kinds); for a row the names don't say, a
 * token value's own, or a colour's where its text is one. */
const kindOf = (row: string, text: string, kinds: Map<string, string>) =>
  /(^|-)color$|^(background|fill|stroke)$/.test(row) ? 'color'
  : row === 'box-shadow' ? 'box-shadow' : row === 'text-shadow' ? 'text-shadow' : row === 'font-family' ? 'family'
  // (A line height takes a length too, where its text is one.)
  : /^(opacity|line-height|flex-grow|font-weight|-exact-press-scale|-exact-press-opacity)$/.test(row) ? (row === 'line-height' && !NUMBER.test(text) ? 'length' : 'number')
  : /duration$/.test(row) ? 'duration' : /timing-function$|easing$/.test(row) ? 'easing'
  : kinds.get(text) ?? (/^(#|rgba?\(|hsla?\(|oklch\(|oklab\(|lab\(|lch\(|color\(|color-mix\(|light-dark\()/i.test(text) ? 'color' : 'length');

/** Where a `set-row` writes, in words: a style's row, or a row at its file and line. */
const rowAt = (a: any) => (a.style ? `${a.style} ${a.row} (${a.file})` : `${a.row} at ${a.file}:${a.at?.line}`);
/** A `set-row` edit or a move, as Undo names it; null for any other kind. */
export const moveLabel = (kind: string, target: string, value: string) =>
  kind === 'set-row' ? `${rowAt(JSON.parse(target))} = ${value}`
  : kind === 'promote-param' ? `${rowAt(JSON.parse(target).addr)} as the param ${value}`
  : kind === 'promote-token' ? `${rowAt(JSON.parse(target).addr)} as the token ${value}`
  : kind === 'detach-param' ? `${JSON.parse(target).param} detached to ${rowAt(JSON.parse(target).addr)}` : null;
