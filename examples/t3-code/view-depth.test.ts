// view-depth-under-test-stack: main's source sweep (contract/cli/tests/it/button_migration.rs) compiles this app on a
// 2 MiB test thread with the unoptimized compiler, whose lowering recursion costs 19,904 bytes per nested view site
// (contract_lower::Lowerer::nodes + node) and 12,944 bytes per nested expression level (expr::compile), added together
// at each node: a component's props and derives are substituted into its view, while a `fn` call binds its arguments
// to locals. The base of this task (1b848a8be) needed 2,303 KiB and aborted the whole test binary (issue X67, GitHub
// #320). This test is the guard the debug compile cannot be (it takes about 215 s): it rebuilds the compiler's site
// tree from the sources and predicts that peak, so a new surface or a long lookup chain on a deep route fails here
// instead of at main-adoption time.
//
// Calibration (the task record): the walk below gives the compiled plans' site trees exactly (base and after: 109,849
// nodes, 41,379 and 43,947 regions, the same site-depth histograms), and against an instrumented copy of contract-lower
// that sampled the stack at every node and expression level it predicts base 2,303 KiB measured / 2,303 predicted
// (icons.contract:11, 80 sites, 59 levels) and after 1,535 / 1,536 KiB (timeline-files.contract:39, 67 sites, 17
// levels); on all 52 leaves the instrumented runs listed it is never under and at most two levels over. Re-measure the
// three constants when an adoption round brings a new contract-lower; the ground truth stays
// `(ulimit -s 2048; target/debug/contract build examples/t3-code/app.contract)`.
// Re-measured in adopt-main-fixes-r7 on main bc357d03c's contract-lower (LLP 1115's heading styles grew the site frames):
// lldb at a forced overflow gives Lowerer::nodes 19,392 + node 512 and expr::compile 12,944 bytes (were 18,816 + 512 and
// 12,720); the instrumented peak is 1,573 KiB at timeline-files.contract:39 (67 sites, 17 levels; 1,535 KiB on the old
// compiler), which the old constants under-predicted by 37 KiB. With these, the model gives that peak exactly and is
// 0 to 13 KiB over on the twelve deepest leaves. Two shallower leaves with handlers it does not count are under:
// markdown.contract:692 by 36 KiB and shell-tip.contract:83 by 9 KiB, both at 1,320 KiB (record adopt-main-fixes-r7).
import { describe, expect, test } from 'bun:test';
import { readdirSync, readFileSync } from 'node:fs';

const PER_SITE = 19_904;
const PER_LEVEL = 12_944;
const BELOW_LOWERING = 58_000; // the compile's frames under the first view site
const BUDGET = 1_792 * 1024; // 7/8 of the 2 MiB thread: room for a compiler change between measurements

type E =
  | { k: 'lit' }
  | { k: 'id'; name: string }
  | { k: 'one'; x: E }
  | { k: 'many'; xs: E[] }
  | { k: 'call'; name: string; args: E[] }
  | { k: 'bind'; names: string[]; x: E }
  | { k: 'named'; name: string; value: E };

// Contract's expression grammar, as far as depth goes: each node is one expr::compile frame; parentheses are none.
class Parser {
  p = 0;
  constructor(readonly s: string) {}
  ws() {
    while (this.p < this.s.length && /\s/.test(this.s[this.p]!)) this.p++;
    if (this.s.startsWith('//', this.p)) this.p = this.s.length; // a trailing comment
  }
  peek(re: RegExp) { this.ws(); re.lastIndex = this.p; const m = re.exec(this.s); return m && m.index === this.p ? m[0] : null; }
  take(re: RegExp) { const m = this.peek(re); if (m !== null) this.p += m.length; return m; }
  word(w: string) { return this.take(new RegExp(`${w}(?!\\w)`, 'y')); }
  at(ch: string) { this.ws(); return this.s[this.p] === ch; }
  expect(t: string) {
    this.ws();
    if (!this.s.startsWith(t, this.p)) throw new Error(`expected ${t} at ${JSON.stringify(this.s.slice(this.p, this.p + 30))}`);
    this.p += t.length;
  }
  done() { this.ws(); return this.p >= this.s.length; }
  expr(): E {
    const lambda = this.take(/[A-Za-z_]\w*\s*=>|\(\s*[A-Za-z_]\w*(\s*,\s*[A-Za-z_]\w*)*\s*\)\s*=>/y);
    if (lambda) return { k: 'one', x: { k: 'bind', names: lambda.match(/[A-Za-z_]\w*/g)!, x: this.expr() } };
    const c = this.binary(0);
    if (!this.take(/\?(?!\?)/y)) return c;
    const t = this.expr(); this.expect(':');
    return { k: 'many', xs: [c, t, this.expr()] };
  }
  static OPS = [/or(?!\w)/y, /and(?!\w)/y, /==|!=|<=|>=|<|>/y, /\+|-/y, /\*|\/(?!\/)|%/y];
  binary(level: number): E {
    if (level === Parser.OPS.length) return this.unary();
    if (level === 2 && this.word('not')) return { k: 'one', x: this.binary(2) };
    let l = this.binary(level + 1);
    while (this.take(Parser.OPS[level]!)) l = { k: 'many', xs: [l, this.binary(level + 1)] };
    return l;
  }
  unary(): E { return this.take(/-/y) ? { k: 'one', x: this.unary() } : this.postfix(); }
  postfix(): E {
    let x = this.primary();
    for (;;) {
      if (this.s[this.p] === '.' && /[A-Za-z_]/.test(this.s[this.p + 1] ?? '')) { this.p++; this.take(/\w+/y); x = { k: 'one', x }; }
      else if (this.s[this.p] === '(') { this.p++; const args = this.list(')'); x = x.k === 'id' ? { k: 'call', name: x.name, args } : { k: 'many', xs: [x, ...args] }; }
      else if (this.s[this.p] === '[') { this.p++; const i = this.expr(); this.expect(']'); x = { k: 'many', xs: [x, i] }; }
      else return x;
    }
  }
  list(close: string): E[] {
    const out: E[] = [];
    if (this.at(close)) { this.p++; return out; }
    for (;;) {
      const named = this.take(/[A-Za-z_][\w-]*\s*=(?![=>])/y);
      out.push(named ? { k: 'named', name: named.replace(/\s*=$/, ''), value: this.expr() } : this.expr());
      if (this.at(',')) { this.p++; continue; }
      this.expect(close);
      return out;
    }
  }
  primary(): E {
    this.ws();
    const ch = this.s[this.p];
    if (ch === '"') { this.p++; while (this.p < this.s.length && this.s[this.p] !== '"') this.p += this.s[this.p] === '\\' ? 2 : 1; this.p++; return { k: 'lit' }; }
    if (ch === '`') {
      this.p++;
      const parts: E[] = [];
      while (this.p < this.s.length && this.s[this.p] !== '`') {
        if (this.s[this.p] === '\\') this.p += 2;
        else if (this.s.startsWith('${', this.p)) { this.p += 2; parts.push(this.expr()); this.expect('}'); }
        else this.p++;
      }
      this.p++;
      return parts.length ? { k: 'many', xs: parts } : { k: 'lit' };
    }
    if (ch === '(') { this.p++; const x = this.expr(); this.expect(')'); return x; }
    if (ch === '[') { this.p++; return { k: 'many', xs: this.list(']') }; }
    if (this.take(/[0-9]+(\.[0-9]+)?([eE][-+]?[0-9]+)?/y)) return { k: 'lit' };
    if (this.word('match')) {
      const arms: E[] = [this.expr()];
      this.expect('{');
      while (!this.at('}')) {
        this.word('case');
        const pattern = this.take(/[^=]*?(?==>)/y) ?? '';
        this.expect('=>');
        arms.push({ k: 'bind', names: [...pattern.matchAll(/\(\s*(\w+)\s*\)/g)].map(m => m[1]!), x: this.expr() });
        if (this.at(',')) this.p++;
      }
      this.p++;
      return { k: 'many', xs: arms };
    }
    if (this.word('true') || this.word('false') || this.word('none')) return { k: 'lit' };
    const id = this.take(/[A-Za-z_]\w*/y);
    if (id) return { k: 'id', name: id };
    throw new Error(`unexpected ${JSON.stringify(this.s.slice(this.p, this.p + 30))}`);
  }
}

type Line = { indent: number; text: string; at: string };
type Component = { file: string; derives: [string, E][]; view: Line[] };
type Env = Map<string, number>;
type Fill = { lines: Line[]; file: string; env: Env; fill: Fill } | null;
type Peak = { bytes: number; sites: number; levels: number; at: string; route: string[] };

function measure(dir: string) {
  const errors: string[] = [];
  const parse = <T>(at: string, make: () => T, fallback: T): T => {
    try { return make(); } catch (error) { errors.push(`${at}: ${(error as Error).message}`); return fallback; }
  };
  const whole = (s: string) => { const p = new Parser(s); const x = p.expr(); if (!p.done()) throw new Error(`left over ${JSON.stringify(s.slice(p.p, p.p + 30))}`); return x; };
  const files = new Map<string, { components: Map<string, Component>; uses: Map<string, string> }>();
  const fns = new Map<string, { params: string[]; body: E }>();
  for (const file of readdirSync(dir).filter(name => name.endsWith('.contract') && !name.endsWith('.test.contract'))) {
    const components = new Map<string, Component>();
    const uses = new Map<string, string>();
    let component: Component | null = null;
    let section = '';
    let sectionIndent = 0;
    readFileSync(`${dir}/${file}`, 'utf8').split('\n').forEach((raw, index) => {
      const indent = raw.length - raw.trimStart().length;
      const text = raw.trim();
      const at = `${file}:${index + 1}`;
      if (text === '' || text.startsWith('//')) return;
      if (indent === 0) {
        component = null;
        const use = /^use (.+) from "\.\/(.+?)"/.exec(text);
        if (use) for (const name of use[1]!.split(',')) uses.set(name.trim(), use[2]!);
        const declared = /^component (\w+)/.exec(text);
        if (declared) components.set(declared[1]!, component = { file, derives: [], view: [] });
        const fn = /^fn (\w+)\((.*?)\)\s*:[^=]+=(.*)$/.exec(text);
        if (fn) {
          if (fns.has(fn[1]!)) errors.push(`${at}: a second fn ${fn[1]}`);
          fns.set(fn[1]!, { params: fn[2]!.split(',').map(param => param.split(':')[0]!.trim()).filter(Boolean), body: parse(at, () => whole(fn[3]!), { k: 'lit' }) });
        }
        section = '';
        return;
      }
      const into: Component | null = component;
      if (!into) return;
      if (section && indent <= sectionIndent) section = '';
      if (section === 'view') into.view.push({ indent, text: text.replace(/\s+\/\/ .*$/, ''), at });
      if (section) return;
      const derive = /^derive (\w+)\s*=(.*)$/.exec(text);
      if (derive) into.derives.push([derive[1]!, parse(at, () => whole(derive[2]!), { k: 'lit' })]);
      else if (/^(view|props|state|inject|slot|action|resource|mutation)\b/.test(text)) { section = text === 'view' ? 'view' : 'other'; sectionIndent = indent; }
    });
    files.set(file, { components, uses });
  }
  const resolve = (file: string, name: string): Component | null => {
    const scope = files.get(file);
    const from = scope?.uses.get(name);
    return scope?.components.get(name) ?? (from ? resolve(from, name) : null);
  };

  const fnLevels = new Map<string, number>();
  const levels = (x: E, env: Env): number => {
    switch (x.k) {
      case 'lit': return 1;
      case 'id': return env.get(x.name) ?? 1;
      case 'one': return 1 + levels(x.x, env);
      case 'many': return 1 + Math.max(0, ...x.xs.map(item => levels(item, env)));
      case 'named': return levels(x.value, env);
      case 'bind': { const inner = new Map(env); for (const name of x.names) inner.set(name, 1); return levels(x.x, inner); }
      case 'call': {
        const args = Math.max(0, ...x.args.map(arg => levels(arg, env)));
        const fn = env.has(x.name) ? undefined : fns.get(x.name);
        if (!fn) return 1 + args;
        // The fn's body is compiled once per call over its parameters as locals (contract-lower expr.rs).
        let body = fnLevels.get(x.name);
        if (body === undefined) fnLevels.set(x.name, body = levels(fn.body, new Map(fn.params.map(param => [param, 1]))));
        return 1 + Math.max(args, body);
      }
    }
  };
  const lineExprs = (text: string): E[] => {
    const p = new Parser(text);
    p.take(/[A-Za-z][\w-]*/y);
    const out: E[] = [];
    while (!p.done()) { p.take(/-?[A-Za-z_][\w-]*(:[\w-]+)?=(?!=)/y); out.push(p.unary()); }
    return out;
  };

  const cache = new Map<string, E[]>();
  const exprs = (at: string, make: () => E[]) => { let xs = cache.get(at); if (!xs) cache.set(at, xs = parse(at, make, [])); return xs; };
  let peak: Peak = { bytes: 0, sites: 0, levels: 0, at: '', route: [] };
  let deepest = 0;
  const route: string[] = [];
  const site = (sites: number, xs: E[], env: Env, at: string) => {
    deepest = Math.max(deepest, sites);
    const nested = Math.max(0, ...xs.map(x => levels(x, env)));
    const bytes = BELOW_LOWERING + sites * PER_SITE + nested * PER_LEVEL;
    if (bytes > peak.bytes) peak = { bytes, sites, levels: nested, at, route: [...route] };
  };
  // One parent's children; `depth` is the parent's site depth. A `when` or `each` is a region site; each `else when`
  // nests one more region in the previous link's else arm; a component use inlines its view with no site of its own.
  const block = (lines: Line[], file: string, depth: number, env: Env, fill: Fill) => {
    let chain = depth;
    for (let i = 0; i < lines.length;) {
      const head = lines[i]!;
      let end = i + 1;
      while (end < lines.length && lines[end]!.indent > head.indent) end++;
      const body = lines.slice(i + 1, end);
      const text = head.text;
      const when = /^(else )?when (.*)$/.exec(text);
      const each = /^each ([\w, ]+?) in (.*)$/.exec(text);
      if (when) {
        const region = when[1] ? chain + 1 : depth + 1;
        site(region, exprs(head.at, () => [whole(when[2]!)]), env, head.at);
        route.push(head.at); block(body, file, region, env, fill); route.pop();
        chain = region;
      } else if (text === 'else') {
        block(body, file, chain, env, fill);
      } else if (each) {
        const inner = new Map(env);
        for (const name of each[1]!.split(',')) inner.set(name.trim(), 1);
        site(depth + 1, exprs(head.at, () => {
          const p = new Parser(each[2]!);
          const xs = [p.expr()];
          if (p.take(/key=/y)) xs.push(p.unary());
          if (!p.done()) throw new Error('left over after the key');
          return xs;
        }), inner, head.at);
        route.push(head.at); block(body, file, depth + 1, inner, fill); route.pop();
        chain = depth + 1;
      } else if (text === 'children') {
        if (fill) block(fill.lines, fill.file, depth, fill.env, fill.fill);
      } else if (/^[A-Z]/.test(text)) {
        const name = /^\w+/.exec(text)![0];
        const used = resolve(file, name);
        if (!used) { errors.push(`${head.at}: no component ${name}`); i = end; continue; }
        const inner: Env = new Map();
        for (const arg of exprs(head.at, () => { const call = whole(text); return call.k === 'call' ? call.args : []; }))
          if (arg.k === 'named') inner.set(arg.name, levels(arg.value, env));
        for (const [derived, x] of used.derives) inner.set(derived, levels(x, inner));
        route.push(`${head.at} ${name}`);
        block(used.view, used.file, depth, inner, body.length ? { lines: body, file, env, fill } : null);
        route.pop();
      } else {
        site(depth + 1, exprs(head.at, () => lineExprs(text)), env, head.at);
        route.push(head.at); block(body, file, depth + 1, env, fill); route.pop();
      }
      i = end;
    }
  };
  const root = files.get('app.contract')?.components.get('T3Code');
  if (!root) throw new Error('app.contract has no component T3Code');
  block(root.view, 'app.contract', 0, new Map(), null);
  return { peak, deepest, errors };
}

describe('view depth under main\'s 2 MiB test thread', () => {
  const { peak, deepest, errors } = measure(import.meta.dir);
  const kib = (bytes: number) => `${Math.round(bytes / 1024)} KiB`;

  test('the model reads every view line, derive and fn, and resolves every component use', () => {
    expect(errors).toEqual([]);
  });

  test('the debug compiler\'s predicted lowering stack stays within 7/8 of 2 MiB', () => {
    const report = `${kib(peak.bytes)} at ${peak.at} (${peak.sites} sites, ${peak.levels} expression levels; deepest site ${deepest})\n`
      + `route: ${peak.route.filter(step => / [A-Z]/.test(step)).join(' > ')}\n`
      + 'Flatten as the task record view-depth-under-test-stack did: group lookup chains with includes(), move a lookup '
      + 'into a fn, turn else-when chains into sibling whens; then confirm with the ulimit -s 2048 debug compile.';
    if (peak.bytes > BUDGET) throw new Error(`over ${kib(BUDGET)}: ${report}`);
    expect(peak.bytes).toBeLessThanOrEqual(BUDGET);
    expect(peak.sites).toBeGreaterThan(40); // the model still walks into the deep routes
  });
});
