// Test helper: a Contract component's state, derives and actions run as JavaScript, so a test sends the events a
// host would (a hover, a focus, a press) and reads what the component then shows, rather than its text
// (legacy-sidebar-full.test.ts). It takes the forms the legacy sidebar's rows use: `state` and `derive` lines,
// `action` bodies of assignments, calls and `if` / `else if` / `else`, and `and` / `or` / `not`; the props and the
// host's functions (`performanceNow`, `preventDefault`, the action props) come from the test.

const js = (text: string) => text.replace(/\band\b/g, '&&').replace(/\bor\b/g, '||').replace(/\bnot\b/g, '!');

/** An action body (its lines, indented) as JavaScript statements. */
function statements(lines: string[]): string {
  const out: string[] = [], open: number[] = [];
  for (const line of lines) {
    const text = line.trim();
    if (!text || text.startsWith('//')) continue;
    const indent = line.length - line.trimStart().length;
    while (open.length && indent <= open[open.length - 1]!) { out.push('}'); open.pop(); }
    const branch = /^(if|else if) (.+)$/.exec(text);
    if (branch) { out.push(`${branch[1]} (${js(branch[2]!)}) {`); open.push(indent); }
    else if (text === 'else') { out.push('else {'); open.push(indent); }
    else out.push(`${js(text)};`);
  }
  while (open.length) { out.push('}'); open.pop(); }
  return out.join('\n');
}

export interface ComponentRun {
  /** The component's state, derives (read when asked), props and actions. */
  readonly scope: Record<string, unknown>;
  /** Runs one of its actions, as the host's event would. */
  act(name: string, ...args: unknown[]): void;
}

/** `component <name>` of a Contract source with these props and host functions. */
export function contractComponent(source: string, name: string, props: Record<string, unknown>, host: Record<string, unknown> = {}): ComponentRun {
  const start = source.indexOf(`\ncomponent ${name}\n`);
  if (start < 0) throw new Error(`no component ${name}`);
  const lines = source.slice(start + 1, source.indexOf('\n  view\n', start)).split('\n').slice(1);
  const scope: Record<string, unknown> = Object.create(null);
  Object.assign(scope, props, host);
  const evaluate = (expr: string) => new Function('scope', `with (scope) { return (${js(expr)}); }`)(scope);
  for (let index = 0; index < lines.length; index++) {
    const text = lines[index]!;
    if (!/^ {2}\S/.test(text)) continue;
    const state = /^ {2}state (\w+) = (.+)$/.exec(text), derive = /^ {2}derive (\w+) = (.+)$/.exec(text), action = /^ {2}action (\w+)(?:\(([^)]*)\))?$/.exec(text);
    if (state) scope[state[1]!] = evaluate(state[2]!);
    else if (derive) { const expr = derive[2]!; Object.defineProperty(scope, derive[1]!, { get: () => evaluate(expr), enumerable: true }); }
    else if (action) {
      const params = (action[2] ?? '').split(',').map(param => param.split(':')[0]!.trim()).filter(Boolean);
      const body: string[] = [];
      while (index + 1 < lines.length && (/^ {4}/.test(lines[index + 1]!) || !lines[index + 1]!.trim())) body.push(lines[++index]!);
      const run = new Function('scope', ...params, `with (scope) {\n${statements(body)}\n}`);
      scope[action[1]!] = (...args: unknown[]) => run(scope, ...args);
    }
  }
  return { scope, act: (action, ...args) => (scope[action] as (...values: unknown[]) => void)(...args) };
}
