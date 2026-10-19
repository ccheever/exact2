// Moving a row between D6's rungs (LLP 1122 T5): promote a row its author
// wrote to a new param or token, or detach a param's row to its template,
// each one transaction the server checks and undoes in one step.
import type { ParamInfo } from './rescope';

/** A value the server takes as a number (else a Contract string). */
export const NUMBER = /^-?(\d+\.?\d*|\.\d+)(e[+-]?\d+)?$/i;

/** A row moved between rungs (LLP 1122 D6's enumerated set, T5), one transaction: promoted to a
 * new param (`add-param` and the row reading `param.<name>`) or a new token (`add-token` and the row
 * naming it), each with the value as written as its default and the kind it is of; or a param's
 * row detached to its template (the param's default written at the row, `remove-param` for it and
 * for any the server says it alone left read, `extra`). None, said by the proposal, where a detach
 * can't: the design sets the param, or its default reads another param. */
export function moveOps(file: string, kind: string, t: any, name: string, params: Map<string, ParamInfo>, kinds: Map<string, string>): Record<string, unknown>[] {
  const as = (v: string) => (NUMBER.test(v) ? Number(v) : v);
  if (kind === 'promote-param') return [{ op: 'add-param', component: t.component, name, kind: kindOf(t.text, kinds), default: t.text }, { op: 'set-row', ...t.addr, value: `param.${name}` }];
  if (kind === 'promote-token') return [{ op: 'add-token', name, kind: kindOf(t.text, kinds), value: t.text, ...(file ? { file } : {}) }, { op: 'set-row', ...t.addr, value: name }];
  const p = params.get(t.param), [, component, short] = /#([^.]+)\.(.+)$/.exec(t.param) ?? [];
  if (!p || p.set !== null || p.default.includes('#') || !component) return [];
  return [{ op: 'set-row', ...t.addr, value: as(p.default) }, ...[short, ...(t.extra ?? [])].map(n => ({ op: 'remove-param', component, name: n }))];
}
/** The params a refusal says are left unread (`tamber-param-unused`), of the detached param's
 * component, not yet removed by `ops`. */
export const unread = (error: string, target: string, ops: Record<string, unknown>[]) => {
  const component = /#([^.]+)\./.exec(JSON.parse(target).param)?.[1] ?? '', gone = new Set(ops.filter(o => o.op === 'remove-param').map(o => String(o.name)));
  return [...new Set(error.split('\n').filter(l => l.includes('tamber-param-unused') && l.includes(component)).flatMap(l => [...l.matchAll(/`(?:[^`#]*#[^.`]+\.)?([A-Za-z_][\w-]*)`/g)].map(m => m[1])))].filter(n => n !== component && !gone.has(n));
};
/** A value's kind for a new param or token: a token's own, else as its text reads (a colour, a length, a number). */
const kindOf = (text: string, kinds: Map<string, string>) => kinds.get(text) ?? (/^(#|rgba?\(|hsla?\(|oklch\(|oklab\(|lab\(|lch\(|color\(|color-mix\(|light-dark\()/i.test(text) ? 'color' : /^-?(\d+\.?\d*|\.\d+)$/.test(text) ? 'number' : 'length');

/** Where a `set-row` writes, in words: a style's row, or a row at its file and line. */
const rowAt = (a: any) => (a.style ? `${a.style} ${a.row} (${a.file})` : `${a.row} at ${a.file}:${a.at?.line}`);
/** A `set-row` edit or a move, as Undo names it; null for any other kind. */
export const moveLabel = (kind: string, target: string, value: string) =>
  kind === 'set-row' ? `${rowAt(JSON.parse(target))} = ${value}`
  : kind === 'promote-param' ? `${rowAt(JSON.parse(target).addr)} as the param ${value}`
  : kind === 'promote-token' ? `${rowAt(JSON.parse(target).addr)} as the token ${value}`
  : kind === 'detach-param' ? `${JSON.parse(target).param} detached to this template` : null;
