// A component's own actions and derives read from its `.contract` file and run with Contract's action semantics
// (docs/contract-for-agents.md: a read sees the value from before the action, a `let` is visible after its line, the
// last write to a slot wins), for tests of a drag's state (not a test file). Main's `analyze-read-after-assign`
// (LLP 1116.003 D6) finds an action reading a slot its start branch has just reset; sidebar-sweep.test.ts and
// queued-drag.test.ts run the bodies that had it. Statements: `let`, a slot write, `if` / `else if` / `else`.
import { readFileSync } from 'node:fs';

export type Env = Record<string, unknown>;
type Node = { line: string; body: Node[] };

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

export function value(expr: string, env: Env): unknown {
  const names = Object.keys(env);
  return new Function(...names, `return (${js(expr)[0]});`)(...names.map(name => env[name]));
}

/** Runs statements: reads see `env` (the state from before the action, the props, the locals so far); writes go to `writes`. */
function run(nodes: Node[], outer: Env, slots: Set<string>, writes: Env, owner: string) {
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
      if (taken) run(taken.body, env, slots, writes, owner);
    } else if ((m = /^let (\w+) = (.+)$/.exec(line))) {
      if (m[1]! in env) throw new Error(`a local cannot shadow ${m[1]}`);
      env[m[1]!] = value(m[2]!, env);
    } else if ((m = /^(\w+) = (.+)$/.exec(line))) {
      if (!slots.has(m[1]!)) throw new Error(`${m[1]} is not ${owner} state`);
      writes[m[1]!] = value(m[2]!, env);
    } else {
      throw new Error(`unsupported statement: ${line}`);
    }
  }
}

/** `component name` of `file` (next to this module): its state, its actions and its derives. */
export function contractComponent(file: string, name: string) {
  const source = readFileSync(`${import.meta.dir}/${file}`, 'utf8').split('\n');
  const from = source.indexOf(`component ${name}`);
  if (from < 0) throw new Error(`${file}: no component ${name}`);
  const to = source.findIndex((line, i) => i > from && line.startsWith('component '));
  const lines = source.slice(from, to < 0 ? undefined : to);

  /** The statements of `action name` as a tree by indentation; comments and blank lines dropped. */
  const action = (actionName: string): { params: string[]; body: Node[] } => {
    const start = lines.findIndex(line => new RegExp(`^  action ${actionName}(\\(|$)`).test(line));
    if (start < 0) throw new Error(`${file}: no action ${actionName} in ${name}`);
    const params = [...(/\((.*)\)/.exec(lines[start]!)?.[1] ?? '').matchAll(/(\w+):/g)].map(m => m[1]!);
    const body: [number, string][] = [];
    for (let i = start + 1; i < lines.length; i++) {
      const line = lines[i]!;
      if (line.trim() === '' || line.trim().startsWith('//')) continue;
      const indent = line.length - line.trimStart().length;
      if (indent <= 2) break;
      body.push([indent, line.trim()]);
    }
    const tree = (at: number, indent: number): [Node[], number] => {
      const nodes: Node[] = [];
      while (at < body.length && body[at]![0] === indent) {
        const [children, next] = tree(at + 1, indent + 2);
        nodes.push({ line: body[at]![1], body: children });
        at = next;
      }
      return [nodes, at];
    };
    return { params, body: tree(0, 4)[0] };
  };

  return {
    /** The slots at mount: each `state x = …` evaluated alone. */
    state(): Env {
      const state: Env = {};
      for (const line of lines) {
        const m = /^  state (\w+) = (.+)$/.exec(line);
        if (m) state[m[1]!] = value(m[2]!, {});
      }
      return state;
    },
    /** Runs `action` over `state` with `scope` (props, helpers) and `args`; its writes land on `state` as it ends. */
    call(actionName: string, state: Env, scope: Env, args: unknown[]) {
      const { params, body } = action(actionName);
      const writes: Env = {};
      run(body, { ...scope, ...state, ...Object.fromEntries(params.map((p, i) => [p, args[i]])) }, new Set(Object.keys(state)), writes, name);
      Object.assign(state, writes);
    },
    /** Every `derive x = …` in declaration order, each seeing the state, `scope` and the derives before it. */
    derives(state: Env, scope: Env): Env {
      const env: Env = { ...scope, ...state }, out: Env = {};
      for (const line of lines) {
        const m = /^  derive (\w+) = (.+)$/.exec(line);
        if (m) env[m[1]!] = out[m[1]!] = value(m[2]!, env);
      }
      return out;
    },
  };
}
