// scripts/design-tool-harness.mjs's port: these runs' own (EXACT_TOOL_PORT, else this checkout's), even when a file
// run before in the same process loaded the port module with none, as tamber-server.test.mjs does (7740): the
// harness then waited for a tool line on 7740 that its child printed on its own port. In a process of its own, so
// neither this file's environment nor its cached modules reach another's.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';

const ROOT = resolve(import.meta.dir, '..');

test("the harness's tool origin is these runs' port, whatever port the port module was first loaded with", () => {
  const env = { ...process.env };
  delete env.EXACT_TOOL_PORT;
  const script = `
    const first = await import(${JSON.stringify(resolve(ROOT, 'scripts/tamber-tool-port.mjs'))});
    const harness = await import(${JSON.stringify(resolve(ROOT, 'scripts/design-tool-harness.mjs'))});
    console.log(JSON.stringify({ first: first.TOOL_PORT, port: harness.TOOL_PORT, origin: harness.TOOL_ORIGIN, dist: harness.TOOL_DIST, env: process.env.EXACT_TOOL_PORT }));`;
  const run = spawnSync(process.execPath, ['-e', script], { cwd: ROOT, env, encoding: 'utf8' });
  expect(run.status, run.stderr).toBe(0);
  // The run's own port (claimed from this checkout's on: that one unless another run holds it), never 7740.
  const got = JSON.parse(run.stdout.trim().split('\n').at(-1)), port = got.port;
  expect(port).toBeGreaterThanOrEqual(17741);
  expect(port).toBeLessThanOrEqual(17940);
  expect(got).toEqual({ first: 7740, port, origin: `http://127.0.0.1:${port}`, dist: resolve(ROOT, `target/design-tool-web-${port}`), env: String(port) });
}, 60_000);
