// Test helper: a one-line Contract `fn` of this app evaluated as JavaScript, so a test checks what the function
// returns rather than its text (usage-pr-pages.test.ts, browser-capture.test.ts; contractFns: cite-selection-anchor.test.ts,
// timeline-anchor.test.ts).
import { readFileSync } from 'node:fs';
import { appPath } from './app-files';

const source = (file: string) => Bun.file(appPath(file)).text();

/** A one-line Contract `fn`'s body as JavaScript, with its non-method dependencies supplied by the caller. */
export const contractFn = async (file: string, name: string, params: string[], extra: Record<string, unknown> = {}) => {
  const body = new RegExp(`^fn ${name}\\([^)]*\\): [^=]+ = (.+)$`, 'm').exec(await source(file))?.[1];
  if (!body) throw new Error(`${file}: no fn ${name}`);
  const js = body.replace(/\band\b/g, '&&').replace(/\bor\b/g, '||').replace(/\bnot\b/g, '!');
  return new Function(...Object.keys(extra), ...params, `return ${js};`).bind(null, ...Object.values(extra)) as (...args: unknown[]) => unknown;
};

/** The index of the bracket closing the one at `open`, over (), [], {} and template literals' `${…}`. */
function closing(s: string, open: number): number {
  let depth = 0;
  for (let i = open; i < s.length; i++) {
    const c = s[i]!;
    if (c === '"') { i = s.indexOf('"', i + 1); continue; }
    if ('([{'.includes(c)) depth++;
    else if (')]}'.includes(c) && --depth === 0) return i;
  }
  throw new Error(`unclosed bracket in ${s}`);
}

/**
 * A one-line Contract expression as JavaScript, for the few forms these functions use beyond contract-fn-fixture's: `match x
 * { case some(r) => a, case none => b }` over an option (`at` gives `undefined` for none), a shape's constructor with named
 * arguments (`CitePart(block=key, …)` is the record), `and`, `or`, `not`.
 */
export function toJs(expr: string, shapes: string[]): string {
  let s = expr;
  for (let at = s.indexOf('match '); at >= 0; at = s.indexOf('match ')) {
    const brace = s.indexOf(' { case some(', at), end = closing(s, brace + 1);
    const subject = s.slice(at + 'match '.length, brace);
    const name = s.slice(brace + ' { case some('.length, s.indexOf(')', brace));
    const arms = s.slice(s.indexOf(') => ', brace) + ') => '.length, end);
    const split = arms.lastIndexOf(', case none => ');
    s = `${s.slice(0, at)}((${name}) => ${name} === undefined ? (${arms.slice(split + ', case none => '.length).trim()}) : (${arms.slice(0, split)}))(${subject})${s.slice(end + 1)}`;
  }
  for (const shape of shapes) {
    for (let at = s.indexOf(`${shape}(`); at >= 0; at = s.indexOf(`${shape}(`)) {
      const open = at + shape.length, end = closing(s, open);
      s = `${s.slice(0, at)}({ ${s.slice(open + 1, end).replace(/\b([a-z]\w*)=(?![=>])/g, '$1: ')} })${s.slice(end + 1)}`;
    }
  }
  return s.replace(/\band\b/g, '&&').replace(/\bor\b/g, '||').replace(/\bnot\b/g, '!');
}

const std = { min: Math.min, max: Math.max };

/** Every one-line `fn` of a `.contract` file as JavaScript, each seeing the others; `shapes` are its record constructors. */
export function contractFns(file: string, shapes: string[] = [], extra: Record<string, unknown> = {}): Record<string, (...args: never[]) => unknown> {
  const fns: Record<string, (...args: never[]) => unknown> = {};
  const scope: Record<string, unknown> = { ...std, ...extra };
  for (const m of readFileSync(appPath(file), 'utf8').matchAll(/^fn (\w+)\(([^)]*)\): [^=]+ = (.+)$/gm)) {
    const params = [...m[2]!.matchAll(/(\w+):/g)].map(p => p[1]!);
    const body = toJs(m[3]!, shapes);
    fns[m[1]!] = (...args: never[]) => new Function(...Object.keys(scope), ...params, `return (${body});`)(...Object.values(scope), ...args);
    scope[m[1]!] = fns[m[1]!];
  }
  return fns;
}
