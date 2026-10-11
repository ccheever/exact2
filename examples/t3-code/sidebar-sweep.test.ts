// adopt-main-fixes-r10: the sidebar's row-action sweep (r11-upstream, 1826fb55cc) starts fresh each time, as the
// reference's does: Sidebar.tsx `startActionSweep` (T3 Code 1e2ecbd975, MIT, see LICENSE-T3) makes a new closure per
// press (`targetKey = null`, `sweptKeys = []`, the pressed row and action) and each move resolves the armed rows from
// the pointer's current position (`sweepTo(sidebarThreadKeyAtY(list, y))`).
//
// This runs ThreadSection's own `sweep` and `cancelSweep` actions from sidebar.contract with Contract's action
// semantics (docs/contract-for-agents.md: a read sees the value from before the action, a `let` is visible after its
// line, the last write to a slot wins). Main's `analyze-read-after-assign` (LLP 1116.003 D6) found the old body reading
// what its start branch had just reset: a sweep after an earlier one kept that sweep's offset, origin, verb, armed rows
// and cancel to its release, and every move armed the rows of the move before.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

type Node = { line: string; body: Node[] };
type Env = Record<string, unknown>;

const source = readFileSync(`${import.meta.dir}/sidebar.contract`, 'utf8').split('\n');
const component = (() => {
  const from = source.indexOf('component ThreadSection');
  const to = source.findIndex((line, i) => i > from && line.startsWith('component '));
  return source.slice(from, to < 0 ? undefined : to);
})();

/** The statements of `action name` as a tree by indentation; comments and blank lines dropped. */
function action(name: string): { params: string[]; body: Node[] } {
  const start = component.findIndex(line => new RegExp(`^  action ${name}(\\(|$)`).test(line));
  if (start < 0) throw new Error(`sidebar.contract: no action ${name} in ThreadSection`);
  const params = [...(/\((.*)\)/.exec(component[start]!)?.[1] ?? '').matchAll(/(\w+):/g)].map(m => m[1]!);
  const lines: [number, string][] = [];
  for (let i = start + 1; i < component.length; i++) {
    const line = component[i]!;
    if (line.trim() === '' || line.trim().startsWith('//')) continue;
    const indent = line.length - line.trimStart().length;
    if (indent <= 2) break;
    lines.push([indent, line.trim()]);
  }
  const tree = (at: number, indent: number): [Node[], number] => {
    const nodes: Node[] = [];
    while (at < lines.length && lines[at]![0] === indent) {
      const [body, next] = tree(at + 1, indent + 2);
      nodes.push({ line: lines[at]![1], body });
      at = next;
    }
    return [nodes, at];
  };
  return { params, body: tree(0, 4)[0] };
}

/** Contract's expression syntax as JavaScript: `and`, `or`, `not`, `==`, `!=` outside string literals. */
function js(s: string, i = 0, untilBrace = false): [string, number] {
  let out = '', code = '', depth = 0;
  const flush = () => {
    out += code.replace(/\band\b/g, '&&').replace(/\bor\b/g, '||').replace(/\bnot\b/g, '!').replace(/([!=])=/g, '$1==');
    code = '';
  };
  while (i < s.length) {
    const c = s[i]!;
    if (c === '"') {
      const end = s.indexOf('"', i + 1);
      flush(); out += s.slice(i, end + 1); i = end + 1;
    } else if (c === '`') {
      flush(); out += '`'; i++;
      while (s[i] !== '`') {
        if (s.startsWith('${', i)) { const [inner, end] = js(s, i + 2, true); out += `\${${inner}}`; i = end + 1; }
        else out += s[i++];
      }
      out += '`'; i++;
    } else {
      if (untilBrace && c === '{') depth++;
      if (untilBrace && c === '}' && depth-- === 0) { flush(); return [out, i]; }
      code += c; i++;
    }
  }
  flush();
  return [out, i];
}

function value(expr: string, env: Env): unknown {
  const names = Object.keys(env);
  return new Function(...names, `return (${js(expr)[0]});`)(...names.map(name => env[name]));
}

/** Runs statements: reads see `env` (the state from before the action, the props, the locals so far); writes go to `writes`. */
function run(nodes: Node[], outer: Env, slots: Set<string>, writes: Env) {
  const env = { ...outer };
  for (let k = 0; k < nodes.length; k++) {
    const { line, body } = nodes[k]!;
    let m: RegExpExecArray | null;
    if (line.startsWith('if ')) {
      const arms: { cond: string | null; body: Node[] }[] = [{ cond: line.slice(3), body }];
      while (nodes[k + 1]?.line === 'else' || nodes[k + 1]?.line.startsWith('else if ')) {
        const next = nodes[++k]!;
        arms.push({ cond: next.line === 'else' ? null : next.line.slice(8), body: next.body });
        if (next.line === 'else') break;
      }
      const taken = arms.find(arm => arm.cond === null || Boolean(value(arm.cond, env)));
      if (taken) run(taken.body, env, slots, writes);
    } else if ((m = /^let (\w+) = (.+)$/.exec(line))) {
      if (m[1]! in env) throw new Error(`a local cannot shadow ${m[1]}`);
      env[m[1]!] = value(m[2]!, env);
    } else if ((m = /^(\w+) = (.+)$/.exec(line))) {
      if (!slots.has(m[1]!)) throw new Error(`${m[1]} is not ThreadSection state`);
      writes[m[1]!] = value(m[2]!, env);
    } else {
      throw new Error(`unsupported statement: ${line}`);
    }
  }
}

// Four settleable rows 28 pt tall, 30 pt apart, in a 200-pt list at the top; each row's action button is 20 pt tall,
// 4 pt below the row's top, so its centre (where a sweep starts) is the row's top + 14.
const rows = ['a', 'b', 'c', 'd'].map(id => ({ id, canSettle: true, canUnsettle: false, canWake: false, hoverKey: `|${id}|` }));
const frames: Record<string, { y: number; height: number }> = { 'thread-list': { y: 0, height: 200 } };
rows.forEach(({ id }, i) => { frames[`row-${id}`] = { y: 30 * i, height: 28 }; frames[`sweep-${id}`] = { y: 30 * i + 4, height: 20 }; });

function section() {
  const state: Env = {};
  for (const line of component) {
    const m = /^  state (\w+) = (.+)$/.exec(line);
    if (m) state[m[1]!] = value(m[2]!, {});
  }
  const slots = new Set(Object.keys(state));
  const data = { sidebar: { sweepEpoch: 0 } };
  const helpers: Env = {
    min: Math.min, max: Math.max,
    join: (list: string[], sep: string) => list.join(sep),
    map: <T, U>(list: T[], f: (t: T) => U) => list.map(f),
    filter: <T>(list: T[], f: (t: T) => boolean) => list.filter(f),
    frame: (id: string) => { const f = frames[id]; if (!f) throw new Error(`no frame ${id}`); return f; },
  };
  const call = (name: string, args: unknown[]) => {
    const { params, body } = action(name);
    const writes: Env = {};
    run(body, { ...helpers, ...state, rows, data, scrollY: 0, ...Object.fromEntries(params.map((p, i) => [p, args[i]])) }, slots, writes);
    Object.assign(state, writes);
  };
  return {
    state,
    move: (id: string, dy: number, verb = 'settle', dx = 0) => call('sweep', [id, verb, dx, dy]),
    cancel: () => call('cancelSweep', []),
    // The release is a `sweep|…` drop; the client bumps sweepEpoch when it lands, which ends the sweep.
    release: () => { data.sidebar.sweepEpoch++; },
    get at() { return [state.sweepDy, state.sweepY, state.sweepAt, state.sweepKeys]; },
  };
}

describe('a sidebar row-action sweep starts fresh (sidebar.contract ThreadSection.sweep)', () => {
  test('each move arms the rows between the pressed row and the pointer as it is now', () => {
    const s = section();
    s.move('a', 2);
    expect(s.at).toEqual([2, 14, 16, '']); // under the 6-pt distance: still the button's click
    s.move('a', 8);
    expect(s.at).toEqual([10, 14, 24, '|a|']);
    s.move('a', 10);
    s.move('a', 10);
    expect(s.at).toEqual([30, 14, 44, '|a|b|']);
  });

  test('a sweep on another row after a release takes nothing from the last one', () => {
    const s = section();
    for (let i = 0; i < 3; i++) s.move('a', 10);
    s.release();
    s.move('c', 3);
    // The old body gave [33, 74, 44, '|a|b|']: the last sweep's offset and point, and its rows armed at once.
    expect(s.at).toEqual([3, 74, 77, '']);
    expect([s.state.sweepId, s.state.sweepVerb, s.state.sweepTop]).toEqual(['c', 'settle', 60]);
    s.move('c', 3);
    expect(s.at).toEqual([6, 74, 80, '|c|']);
  });

  test('a sweep on the same row after a release, and one after an Escape, start from the press', () => {
    const s = section();
    s.move('b', 10, 'unsettle');
    s.move('b', 10, 'unsettle');
    s.release();
    s.move('b', 10);
    expect([s.state.sweepVerb, ...s.at]).toEqual(['settle', 10, 44, 54, '|b|']);
    s.cancel();
    expect([s.state.sweepCancelled, s.state.sweepKeys]).toEqual([true, '']);
    s.move('b', 10);
    expect(s.state.sweepKeys).toBe(''); // a cancelled sweep stays cancelled to its release
    s.release();
    s.move('c', -10);
    expect([s.state.sweepCancelled, ...s.at]).toEqual([false, -10, 74, 64, '|c|']);
    s.move('c', -25);
    expect(s.at).toEqual([-35, 74, 39, '|b|c|']);
  });
});
