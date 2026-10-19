// The design tool's own local origin (LLP 1122 T1): 127.0.0.1:7740, the one
// its grants name, or another port for tests and gate runs (`EXACT_TOOL_PORT`),
// for which the tool is built with that origin.
import { existsSync, mkdirSync, readdirSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, relative, resolve } from 'node:path';

const ROOT = resolve(new URL('..', import.meta.url).pathname);

/** The design tool's own local port: 7740, or `EXACT_TOOL_PORT` (a test's
 * own, so no run holds a person's), which must be a port from 1024 to 65535:
 * anything else is refused rather than falling back to 7740. */
export const TOOL_PORT = (() => {
  const said = process.env.EXACT_TOOL_PORT;
  if (said === undefined) return 7740;
  const port = /^\d+$/.test(said) ? Number(said) : NaN;
  if (!(port >= 1024 && port <= 65535)) throw new Error(`EXACT_TOOL_PORT is ${JSON.stringify(said)}: a port from 1024 to 65535, or unset for 7740`);
  return port;
})();
/** Where `bun exact.mjs tamber tool` builds the design tool, and the wire
 * serves it from: a build of its own for another port (its origin is in its
 * grants). */
export const TOOL_DIST = resolve(ROOT, TOOL_PORT === 7740 ? 'target/design-tool-web' : `target/design-tool-web-${TOOL_PORT}`);
/** The design tool's own local URL: its grants name this one origin
 * (apps/design-tool/app.ts). */
export const TOOL_ORIGIN = `http://127.0.0.1:${TOOL_PORT}`;

/** For another port: the tool's sources staged beside its build, its API
 * origin (which its grants name) that port's, the one literal app.ts writes
 * (refused unless there is exactly one). For the build (`tests` false) the
 * stub stays behind and the staged app names no server of its own, so a
 * drive of it starts none on 7740; for its contract tests the stub and the
 * test files come too, every 7740 in the tests that port (refused when they
 * name that port already, for a page of their own), so the stub binds it,
 * built into `dist`. A file staging writes is newer than the last build,
 * and one gone from the stage was an input of it: either makes the build's
 * own check (agent-launch.mjs `webChanges`) build again, whoever staged.
 * What the build writes there (app.contract.d.ts) stays. */
export function stageTool({ tests = false } = {}) {
  const from = resolve(ROOT, 'apps/design-tool'), to = resolve(ROOT, `target/design-tool-${tests ? 'test' : 'src'}-${TOOL_PORT}`);
  const dist = tests ? resolve(ROOT, `target/design-tool-test-web-${TOOL_PORT}`) : TOOL_DIST;
  mkdirSync(to, { recursive: true });
  const source = f => f === 'stub' || /\.(ts|contract|json)$/.test(f) && !f.endsWith('.d.ts');
  const take = f => source(f) && (tests || f !== 'stub' && !f.endsWith('.test.contract'));
  const prune = (dir, keep, only = source) => { for (const f of readdirSync(resolve(to, dir))) if (only(f) && !keep(f)) rmSync(resolve(to, dir, f), { recursive: true, force: true }); };
  prune('.', f => take(f) && existsSync(resolve(from, f)));
  const put = (rel, staged) => { if (!existsSync(resolve(to, rel)) || readFileSync(resolve(to, rel), 'utf8') !== staged) { mkdirSync(dirname(resolve(to, rel)), { recursive: true }); writeFileSync(resolve(to, rel), staged); } };
  for (const f of readdirSync(from).filter(take).sort()) {
    if (f === 'stub') {
      for (const g of readdirSync(resolve(from, f)).sort()) put(`stub/${g}`, readFileSync(resolve(from, f, g), 'utf8'));
      prune('stub', g => existsSync(resolve(from, f, g)), () => true);
      continue;
    }
    let staged = readFileSync(resolve(from, f), 'utf8');
    if (f === 'app.ts') {
      const literal = "'http://127.0.0.1:7740'";
      if (staged.split(literal).length !== 2) throw new Error(`apps/design-tool/app.ts writes ${literal} ${staged.split(literal).length - 1} times: stageTool replaces exactly one, the API origin`);
      staged = staged.replace(literal, `'${TOOL_ORIGIN}'`);
    }
    if (f.endsWith('.test.contract')) {
      if (new RegExp(`:${TOOL_PORT}(?!\\d)`).test(staged)) throw new Error(`apps/design-tool/${f} names port ${TOOL_PORT} for a page of its own: choose another EXACT_TOOL_PORT`);
      staged = staged.replaceAll('7740', String(TOOL_PORT));
    }
    if (f === 'app.json' && !tests) { const manifest = JSON.parse(staged); delete manifest.commands; staged = `${JSON.stringify(manifest, null, 2)}\n`; }
    put(f, staged);
  }
  return { dir: to, dist };
}

/** A run of the design tool's tests in `dir` (an app-cli verb), if it is
 * this checkout's tool: on the checkout's own port (`checkoutPort`, unless
 * EXACT_TOOL_PORT names one), from a stage with its stub and tests, built
 * into its own dist; the stage's dir, and `args` with each test file or glob
 * in the tool's sources, by its real path, the stage's. `tested(i)` says
 * which arguments are test files (app-cli knows each one's role: a test
 * verb's operand, `--test`'s value); nothing else maps (a URL, an op, a
 * flag's value). Null to run as it is (another app, or EXACT_TOOL_PORT=7740). */
export function stageToolTests(dir, args, tested = () => false) {
  const from = resolve(ROOT, 'apps/design-tool');
  if (!existsSync(from) || realpathSync(dir) !== realpathSync(from) || TOOL_PORT === 7740) return null;
  const staged = stageTool({ tests: true }), real = realpathSync(from);
  process.env.EXACT_WEB_DIST = staged.dist;
  // (A glob's real path is its directory's, up to its first wildcard.)
  const realOf = a => { const at = resolve(a), glob = at.search(/[*?[]/); if (glob < 0) return existsSync(at) ? realpathSync(at) : at; const head = dirname(at.slice(0, glob + 1)); return existsSync(head) ? realpathSync(head) + at.slice(head.length) : at; };
  const into = a => { const at = realOf(a); return at === real || at.startsWith(`${real}/`) ? resolve(staged.dir, relative(real, at)) : a; };
  return { dir: staged.dir, args: args.map((a, i) => (!tested(i) ? a : a.startsWith('--test=') ? `--test=${into(a.slice(7))}` : into(a))) };
}

/** Which of a verb's arguments (after the verb) are test files: a test verb's
 * operands after its host, and `--test`'s value (`--test=<file>` itself); `valued` is help.mjs
 * `knownFlags(verb)` (each flag, and whether it takes a value). */
export function testFiles(verb, rest, valued) {
  const tested = new Set();
  for (let i = 0, operands = 0; i < rest.length; i++) {
    const flag = rest[i].startsWith('-') ? rest[i].split('=')[0] : null;
    if (flag) { if (flag === '--test' && rest[i].includes('=')) tested.add(i); else if (valued.get(flag) && !rest[i].includes('=')) { if (flag === '--test') tested.add(i + 1); i++; } continue; }
    if (verb === 'test' && operands++ > 0) tested.add(i);
  }
  return tested;
}
