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
