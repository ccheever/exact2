// A component's own actions and derives read from its `.contract` file and run with Contract's action semantics
// (docs/contract-for-agents.md: a read sees the value from before the action, a `let` is visible after its line, the
// last write to a slot wins), for tests of a drag's state (not a test file). Main's `analyze-read-after-assign`
// (LLP 1116.003 D6) finds an action reading a slot its start branch has just reset; sidebar-sweep.test.ts and
// queued-drag.test.ts run the bodies that had it. Statements: `let`, a slot write, `if` / `else if` / `else`,
// `refresh <resource>` (the scope's `refresh(name)`), and a call (an action prop such as ThreadSection's `hoverCard`, an
// injected action, a host command such as `preventDefault()`, a mutation's `send`, whose command the scope records and
// whose mutation a `sent(mutation, command)` in the scope is told), made with the reads as they stand at its line.
// `drawn` says whether a view line is drawn: the `if` / `else if` / `else` arms around it hold (legacy-sidebar-full.test.ts
// runs the legacy rows' hover and focus through it, as the host delivers them).
import { readFileSync } from 'node:fs';
import { appPath, appText } from './app-files';

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
  return new Function(...names, `return (${namedCalls(js(expr)[0])});`)(...names.map(name => env[name]));
}

/** Named call arguments become a trailing object: showPopover(id, { source }), KmItem({ id, label }). */
function namedCalls(source: string): string {
  const quotedEnd = (start: number): number => {
    const quote = source[start];
    for (let i = start + 1; i < source.length; i++) {
      if (source[i] === '\\') { i++; continue; }
      if (source[i] === quote) return i + 1;
    }
    throw new Error('unterminated string in Contract expression');
  };
  const group = (start: number): { text: string; end: number } => {
    const args: string[] = [];
    let part = '', depth = 0;
    for (let i = start; i < source.length; i++) {
      const c = source[i]!;
      if (c === '"' || c === "'" || c === '`') {
        const end = quotedEnd(i); part += source.slice(i, end); i = end - 1;
      } else if (c === '(') {
        const inner = group(i + 1); part += `(${inner.text})`; i = inner.end;
      } else if (c === ')' && depth === 0) {
        args.push(part);
        const positional: string[] = [], named: string[] = [];
        for (const arg of args) {
          const field = /^\s*(\w+)\s*=(?!=|>)([\s\S]*)$/.exec(arg);
          if (field) named.push(`${field[1]}: ${field[2]}`);
          else {
            if (named.length) throw new Error('positional argument after named Contract argument');
            positional.push(arg);
          }
        }
        return { text: named.length ? [...positional, `{ ${named.join(', ')} }`].join(', ') : args.join(','), end: i };
      } else if (c === ',' && depth === 0) {
        args.push(part); part = '';
      } else {
        if (c === '[' || c === '{') depth++;
        if (c === ']' || c === '}') depth--;
        part += c;
      }
    }
    throw new Error('unterminated call in Contract expression');
  };
  let out = '';
  for (let i = 0; i < source.length; i++) {
    const c = source[i]!;
    if (c === '"' || c === "'" || c === '`') {
      const end = quotedEnd(i); out += source.slice(i, end); i = end - 1;
    } else if (c === '(') {
      const inner = group(i + 1); out += `(${inner.text})`; i = inner.end;
    } else out += c;
  }
  return out;
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
    } else if (line.startsWith('match ')) {
      // Option values use a value for some and null/undefined for none in the supplied scope.
      const result = value(line.slice(6), env);
      const present = result !== null && result !== undefined;
      for (const arm of body) {
        const some = /^case some\((\w+)\)$/.exec(arm.line);
        if (!some && arm.line !== 'case none') throw new Error(`unsupported match arm: ${arm.line}`);
        if (present && some) {
          run(arm.body, { ...env, [some[1]!]: result }, slots, writes, owner);
          break;
        }
        if (!present && arm.line === 'case none') { run(arm.body, env, slots, writes, owner); break; }
      }
    } else if ((m = /^let (\w+) = (.+)$/.exec(line))) {
      if (m[1]! in env) throw new Error(`a local cannot shadow ${m[1]}`);
      env[m[1]!] = value(m[2]!, env);
    } else if ((m = /^send (\w+) = (.+)$/.exec(line))) {
      // A mutation's send: its command, a call the scope records, and the mutation it goes on, told to the scope's `sent`.
      const command = value(m[2]!, env);
      if (typeof env.sent === 'function') env.sent(m[1]!, command);
    } else if ((m = /^refresh (\w+)$/.exec(line))) {
      if (typeof env.refresh !== 'function') throw new Error(`refresh ${m[1]} needs a refresh(name) in the scope`);
      env.refresh(m[1]!);
    } else if ((m = /^(\w+) = (.+)$/.exec(line))) {
      if (!slots.has(m[1]!)) throw new Error(`${m[1]} is not ${owner} state`);
      writes[m[1]!] = value(m[2]!, env);
    } else if (/^\w+\(.*\)$/.test(line)) {
      // A call: an action prop (`hoverCard`, the window's hover), an
      // injected action or a host command (legacy-sidebar-full.test.ts).
      value(line, env);
    } else {
      throw new Error(`unsupported statement: ${line}`);
    }
  }
}

/** A statement without its trailing `// comment` (a `//` inside a string literal stays). */
function uncommented(line: string): string {
  let quote = '';
  for (let i = 0; i < line.length; i++) {
    const c = line[i]!;
    if (quote) { if (c === '\\') i++; else if (c === quote) quote = ''; }
    else if (c === '"' || c === '`') quote = c;
    else if (c === '/' && line[i + 1] === '/') return line.slice(0, i).trimEnd();
  }
  return line;
}

/** `component name` of `file` (next to this module): its state, its actions and its derives. */
export function contractComponent(file: string, name: string) {
  const source = appText(file).split('\n');
  // A component, or a members file of one (`members T3Code`), whose members read as the component's.
  const from = source.includes(`component ${name}`) ? source.indexOf(`component ${name}`) : source.indexOf(`members ${name}`);
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
      body.push([indent, uncommented(line.trim())]);
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

  /** The first line of the view containing `marker`. */
  const viewLine = (marker: string): string => {
    const line = lines.slice(lines.indexOf('  view') + 1).find(entry => entry.includes(marker));
    if (!line) throw new Error(`${file}: no view line of ${name} has ${marker}`);
    return line;
  };

  const derives = (state: Env, scope: Env): Env => {
    const env: Env = { ...scope, ...state }, out: Env = {};
    for (const line of lines) {
      const m = /^  derive (\w+) = (.+)$/.exec(line);
      if (m) env[m[1]!] = out[m[1]!] = value(m[2]!, env);
    }
    return out;
  };

  return {
    /** The slots at mount: each untyped `state x = …` evaluated alone (`scope`: a shape's constructor it calls). */
    state(scope: Env = {}): Env {
      const state: Env = {};
      for (const line of lines) {
        const m = /^  state (\w+) = (.+)$/.exec(line);
        if (m) state[m[1]!] = value(m[2]!, scope);
      }
      return state;
    },
    /**
     * Runs `action` over `state` with `scope` (props, helpers) and `args`; its writes land on `state` as it ends. It reads
     * the derives as the starting state gives them. An action of the component it calls runs in the same commit: it reads
     * the starting state too, and its writes land with these. A function of the same name in `scope` stands in for it.
     */
    call(actionName: string, state: Env, scope: Env, args: unknown[]) {
      const writes: Env = {}, slots = new Set(Object.keys(state));
      const own: Env = {};
      // Each derive the scope can give (a test's scope may leave out what an unrelated derive reads).
      const derived: Env = {}, env: Env = { ...scope, ...state };
      for (const line of lines) {
        const m = /^  derive (\w+) = (.+)$/.exec(line);
        if (!m) continue;
        try { env[m[1]!] = derived[m[1]!] = value(m[2]!, env); } catch { /* left unread */ }
      }
      const invoke = (target: string, values: unknown[]) => {
        const { params, body } = action(target);
        run(body, { ...own, ...derived, ...scope, ...state, ...Object.fromEntries(params.map((p, i) => [p, values[i]])) }, slots, writes, name);
      };
      for (const line of lines) { const m = /^  action (\w+)/.exec(line); if (m && !(m[1]! in scope)) own[m[1]!] = (...values: unknown[]) => invoke(m[1]!, values); }
      invoke(actionName, args);
      Object.assign(state, writes);
    },
    /**
     * Whether the first view line containing `marker` is drawn: each `if`, `else if` and `else` arm around it holds,
     * read over `state`, `scope` and the derives (a `for` around it counts as drawn).
     */
    drawn(marker: string, state: Env, scope: Env): boolean {
      const env: Env = { ...scope, ...state, ...derives(state, scope) };
      const from = lines.findIndex(line => line === '  view');
      if (from < 0) throw new Error(`${file}: ${name} has no view`);
      const open: { indent: number; cond: string | null }[] = [], arms = new Map<number, string[]>();
      for (const line of lines.slice(from + 1)) {
        const text = line.trim();
        if (!text || text.startsWith('//')) continue;
        const indent = line.length - line.trimStart().length;
        while (open.length && open[open.length - 1]!.indent >= indent) open.pop();
        const prior = () => (arms.get(indent) ?? []).map(arm => `!(${js(arm)[0]})`);
        let cond: string | null = null, m: RegExpExecArray | null;
        if ((m = /^if (.+)$/.exec(text))) { arms.set(indent, [m[1]!]); cond = js(m[1]!)[0]; }
        else if ((m = /^else if (.+)$/.exec(text))) { cond = [...prior(), `(${js(m[1]!)[0]})`].join(' && '); arms.get(indent)!.push(m[1]!); }
        else if (text === 'else') cond = prior().join(' && ');
        open.push({ indent, cond });
        if (text.includes(marker)) {
          const names = Object.keys(env);
          return open.every(({ cond: arm }) => arm === null || Boolean(new Function(...names, `return (${arm});`)(...names.map(n => env[n]))));
        }
      }
      throw new Error(`${file}: no view line of ${name} has ${marker}`);
    },
    /** Every `derive x = …` in declaration order, each seeing the state, `scope` and the derives before it. */
    derives,
    /**
     * The value of attribute `attr` on the first view line containing `marker` (`attr=(…)`, a call `attr=name(…)`, a
     * quoted or backquoted string, or one token), read over `state`, `scope` and the derives.
     */
    attribute(marker: string, attr: string, state: Env, scope: Env): unknown {
      const line = viewLine(marker), at = line.indexOf(` ${attr}=`);
      if (at < 0) throw new Error(`${file}: the line of ${name} with ${marker} has no ${attr}`);
      const start = at + attr.length + 2, call = /^\w+\(/.exec(line.slice(start))?.[0].length ?? 0, open = call ? '(' : line[start];
      let end = start + Math.max(0, call - 1);
      if (open === '(') {
        for (let depth = 0; end < line.length; end++) {
          const c = line[end]!;
          if (c === '"' || c === '`') { end = line.indexOf(c, end + 1); continue; }
          if (c === '(') depth++;
          if (c === ')' && --depth === 0) break;
        }
        end++;
      } else if (open === '"' || open === '`') end = line.indexOf(open, start + 1) + 1;
      else end = start + /^\S+/.exec(line.slice(start))![0].length;
      return value(line.slice(start, end), { ...scope, ...state, ...derives(state, scope) });
    },
    /** The named arguments of the component call that opens the first view line containing `marker` (`Tip(label=…)`). */
    callArguments(marker: string, state: Env, scope: Env): Env {
      const line = viewLine(marker).trim(), callee = /^(\w+)\(/.exec(line)?.[1];
      if (!callee) throw new Error(`${file}: the line of ${name} with ${marker} is no component call`);
      return value(line, { ...scope, ...state, ...derives(state, scope), [callee]: (args: Env) => args }) as Env;
    },
  };
}

/** `file`'s top-level `fn` lines as functions of their parameters (each sees the ones before it and `scope`). */
export function contractFns(file: string, scope: Env = {}): Env {
  const fns: Env = {};
  for (const line of readFileSync(appPath(file), 'utf8').split('\n')) {
    const m = /^fn (\w+)\(([^)]*)\): \w+(?:\[\])? = (.+)$/.exec(line);
    if (!m) continue;
    const params = [...m[2]!.matchAll(/(\w+):/g)].map(p => p[1]!);
    fns[m[1]!] = (...args: unknown[]) => value(m[3]!, { ...scope, ...fns, ...Object.fromEntries(params.map((p, i) => [p, args[i]])) });
  }
  return fns;
}
