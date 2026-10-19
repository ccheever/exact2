// The design tool on a port of its own (EXACT_TOOL_PORT, for tests and gate
// runs): its origin, the dev loops' frame-ancestors and its build follow the
// port exactly, and 7740 stays a person's.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
const facts = (env, tests = false) => JSON.parse(spawnSync(process.execPath, ['-e', `const m = await import(${JSON.stringify(resolve(ROOT, 'scripts/tamber-server.mjs'))}); const s = m.TOOL_PORT === 7740 ? null : m.stageTool({ tests: ${tests} }); console.log(JSON.stringify({ origin: m.TOOL_ORIGIN, frame: m.FRAME_ANCESTORS, dist: m.TOOL_DIST, staged: s?.dir, built: s?.dist }))`], { env: { ...process.env, ...env }, encoding: 'utf8' }).stdout.trim().split('\n').at(-1));

test('another port moves the tool\'s origin, the frame-ancestors (that origin alone) and its build; the default is 7740', () => {
  const mine = facts({ EXACT_TOOL_PORT: '17999' });
  expect(mine.origin).toBe('http://127.0.0.1:17999');
  // A page framed from 7740 (or any other port) is refused: only this origin is named.
  expect(mine.frame).toBe("frame-ancestors 'self' http://127.0.0.1:17999");
  expect(mine.dist).toBe(resolve(ROOT, 'target/design-tool-web-17999'));
  // Its staged sources name that origin in the tool's API (and so its grants).
  expect(readFileSync(resolve(mine.staged, 'app.ts'), 'utf8')).toContain("export const API = 'http://127.0.0.1:17999';");
  // A file the tool no longer has goes from the stage when it is staged again.
  writeFileSync(resolve(mine.staged, 'gone.ts'), 'export {};\n');
  facts({ EXACT_TOOL_PORT: '17999' });
  expect(existsSync(resolve(mine.staged, 'gone.ts'))).toBe(false);
  // The staged app names no server of its own (a drive of it starts no stub on 7740).
  expect(JSON.parse(readFileSync(resolve(mine.staged, 'app.json'), 'utf8')).commands).toBe(undefined);
  // Its build is made again when the staged sources change, whoever staged them first: the
  // marker goes then (a source gone, as a file changed), and stays when nothing changed; the
  // build's own declarations stay in the stage.
  expect(mine.built).toBe(mine.dist);
  mkdirSync(mine.dist, { recursive: true });
  const marker = resolve(mine.dist, '.exact-build.json'), declarations = resolve(mine.staged, 'app.contract.d.ts');
  writeFileSync(marker, '{}'); writeFileSync(declarations, 'export {};\n');
  facts({ EXACT_TOOL_PORT: '17999' });
  expect(existsSync(marker) && existsSync(declarations)).toBe(true);
  writeFileSync(resolve(mine.staged, '.exact-stage'), 'another staging');
  facts({ EXACT_TOOL_PORT: '17999' });
  expect(existsSync(marker)).toBe(false);
  expect(existsSync(declarations)).toBe(true);
  // The contract tests' stage: the stub and the tests come too, every 7740 in them the port, so
  // the stub binds it; the app names its server; it builds apart from the tool.
  const tested = facts({ EXACT_TOOL_PORT: '17999' }, true), tests = readFileSync(resolve(tested.staged, 'app.test.contract'), 'utf8');
  expect(tests).not.toContain('7740');
  expect(tests).toContain('fail fetch "http://127.0.0.1:17999"');
  expect(readFileSync(resolve(tested.staged, 'app.ts'), 'utf8')).toContain("export const API = 'http://127.0.0.1:17999';");
  expect(existsSync(resolve(tested.staged, 'stub/tamber-stub.ts'))).toBe(true);
  expect(JSON.parse(readFileSync(resolve(tested.staged, 'app.json'), 'utf8')).commands.server).toEqual(['bun', 'stub/tamber-stub.ts']);
  expect(tested.built).toBe(resolve(ROOT, 'target/design-tool-test-web-17999'));
  // A port setting that is no port is refused, never 7740 by default.
  for (const bad of ['', '0', 'x', '80', '70000']) expect(spawnSync(process.execPath, ['-e', `await import(${JSON.stringify(resolve(ROOT, 'scripts/tamber-tool-port.mjs'))})`], { env: { ...process.env, EXACT_TOOL_PORT: bad }, encoding: 'utf8' }).stderr).toContain('EXACT_TOOL_PORT is');
  const env = { ...process.env }; delete env.EXACT_TOOL_PORT;
  const plain = JSON.parse(spawnSync(process.execPath, ['-e', `const m = await import(${JSON.stringify(resolve(ROOT, 'scripts/tamber-server.mjs'))}); console.log(JSON.stringify({ origin: m.TOOL_ORIGIN, frame: m.FRAME_ANCESTORS, dist: m.TOOL_DIST }))`], { env, encoding: 'utf8' }).stdout.trim().split('\n').at(-1));
  expect(plain).toEqual({ origin: 'http://127.0.0.1:7740', frame: "frame-ancestors 'self' http://127.0.0.1:7740", dist: resolve(ROOT, 'target/design-tool-web') });
});

// One module load at `port` (TOOL_PORT is read once), running `body` with the module as `m`; its JSON line, or its error.
const at = (port, body) => {
  const r = spawnSync(process.execPath, ['-e', `const m = await import(${JSON.stringify(resolve(ROOT, 'scripts/tamber-tool-port.mjs'))}); console.log(JSON.stringify(await (async () => { ${body} })()))`], { cwd: ROOT, env: { ...process.env, EXACT_TOOL_PORT: String(port) }, encoding: 'utf8' });
  return r.status === 0 ? JSON.parse(r.stdout.trim().split('\n').at(-1)) : { error: r.stderr };
};

test('the contract tests\' stage: a file or glob named in the tool\'s sources is the stage\'s, its own dist, a port the tests use for a page refused, the stub pruned', () => {
  const src = resolve(ROOT, 'apps/design-tool');
  const r = at(17998, `return m.stageToolTests(${JSON.stringify(src)}, ['web', 'apps/design-tool/app.test.contract', ${JSON.stringify(resolve(src, '*.test.contract'))}, '--size', '390x844', 'tests/elsewhere.test.contract'])`);
  const stage = resolve(ROOT, 'target/design-tool-test-17998');
  expect(r.dir).toBe(stage);
  expect(r.args).toEqual(['web', resolve(stage, 'app.test.contract'), resolve(stage, '*.test.contract'), '--size', '390x844', 'tests/elsewhere.test.contract']);
  expect(at(17998, 'm.stageToolTests(' + JSON.stringify(src) + ', []); return process.env.EXACT_WEB_DIST')).toBe(resolve(ROOT, 'target/design-tool-test-web-17998'));
  // Another app, or 7740 itself: run as it is.
  expect(at(17998, `return m.stageToolTests(${JSON.stringify(resolve(ROOT, 'apps/theme-fixture'))}, ['web'])`)).toBe(null);
  expect(at(7740, `return m.stageToolTests(${JSON.stringify(src)}, ['web'])`)).toBe(null);
  // The tests' own dev-loop page is on 8765: that port for the tool would make it the tool's own.
  expect(at(8765, 'return m.stageTool({ tests: true })').error).toContain('names port 8765 for a page of its own');
  // A file gone from the stub goes from the stage's, whatever its kind.
  writeFileSync(resolve(stage, 'stub/gone.txt'), 'x');
  at(17998, 'return m.stageTool({ tests: true }).dir');
  expect(existsSync(resolve(stage, 'stub/gone.txt'))).toBe(false);
});

test('a build is current only for the stage it was made from: one staged since, by any process, differs', () => {
  const r = at(17997, 'const a = m.stageTool(), b = m.stageTool(); m.builtFrom(a.dist, a.signature); return { same: a.signature === b.signature, built: m.builtFrom(a.dist), sig: a.signature }');
  expect(r.same).toBe(true);
  expect(r.built).toBe(r.sig);
  // (What tamber-server.mjs buildTool compares before it accepts the build.)
  expect(at(17997, `m.builtFrom(m.TOOL_DIST, 'a build made from another stage'); return m.builtFrom(m.TOOL_DIST) === m.stageTool().signature`)).toBe(false);
});
