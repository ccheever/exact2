// The design tool's own local origin (LLP 1122 T1): 127.0.0.1:7740, the one
// its grants name, or another port for tests and gate runs (`EXACT_TOOL_PORT`),
// for which the tool is built with that origin.
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, resolve } from 'node:path';

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
 * test file come too, every 7740 in the tests that port, so the stub binds
 * it. `dist` is where that stage builds; when the staged sources differ
 * from the last staging's (one written, or one the tool no longer has gone
 * from the stage), its build's marker goes, so the next build is made again
 * whoever staged first (a build's own check, `webChanges`, finds a file
 * changed but not one gone). What the build writes there
 * (app.contract.d.ts) stays. */
export function stageTool({ tests = false } = {}) {
  const from = resolve(ROOT, 'apps/design-tool'), to = resolve(ROOT, `target/design-tool-${tests ? 'test' : 'src'}-${TOOL_PORT}`);
  const dist = tests ? resolve(ROOT, `target/design-tool-test-web-${TOOL_PORT}`) : TOOL_DIST;
  mkdirSync(to, { recursive: true });
  const source = f => f === 'stub' || /\.(ts|contract|json)$/.test(f) && !f.endsWith('.d.ts');
  const take = f => source(f) && (tests || f !== 'stub' && f !== 'app.test.contract');
  const prune = (dir, keep) => { for (const f of readdirSync(resolve(to, dir))) if (source(f) && !keep(f)) rmSync(resolve(to, dir, f), { recursive: true, force: true }); };
  const sum = createHash('sha256');
  prune('.', f => take(f) && existsSync(resolve(from, f)));
  const put = (rel, staged) => { sum.update(`${rel}\0${staged}\0`); if (!existsSync(resolve(to, rel)) || readFileSync(resolve(to, rel), 'utf8') !== staged) { mkdirSync(dirname(resolve(to, rel)), { recursive: true }); writeFileSync(resolve(to, rel), staged); } };
  for (const f of readdirSync(from).filter(take).sort()) {
    if (f === 'stub') {
      for (const g of readdirSync(resolve(from, f)).sort()) put(`stub/${g}`, readFileSync(resolve(from, f, g), 'utf8'));
      prune('stub', g => existsSync(resolve(from, f, g)));
      continue;
    }
    let staged = readFileSync(resolve(from, f), 'utf8');
    if (f === 'app.ts') {
      const literal = "'http://127.0.0.1:7740'";
      if (staged.split(literal).length !== 2) throw new Error(`apps/design-tool/app.ts writes ${literal} ${staged.split(literal).length - 1} times: stageTool replaces exactly one, the API origin`);
      staged = staged.replace(literal, `'${TOOL_ORIGIN}'`);
    }
    if (f === 'app.test.contract') staged = staged.replaceAll('7740', String(TOOL_PORT));
    if (f === 'app.json' && !tests) { const manifest = JSON.parse(staged); delete manifest.commands; staged = `${JSON.stringify(manifest, null, 2)}\n`; }
    put(f, staged);
  }
  const mark = resolve(to, '.exact-stage'), signature = sum.digest('hex');
  if (!existsSync(mark) || readFileSync(mark, 'utf8') !== signature) { rmSync(resolve(dist, '.exact-build.json'), { force: true }); writeFileSync(mark, signature); }
  return { dir: to, dist };
}
