// scripts/design-tool-harness.mjs's port: these runs' own (EXACT_TOOL_PORT, else this checkout's), even when a file
// run before in the same process loaded the port module with none, as tamber-server.test.mjs does (7740): the
// harness then waited for a tool line on 7740 that its child printed on its own port.
import { expect, test } from 'bun:test';
import { resolve } from 'node:path';
import { checkoutPort } from './port-lock.mjs';

test("the harness's tool origin is these runs' port, whatever port the port module was first loaded with", async () => {
  const said = process.env.EXACT_TOOL_PORT;
  delete process.env.EXACT_TOOL_PORT;
  try {
    const first = await import('./tamber-tool-port.mjs');
    const harness = await import('./design-tool-harness.mjs');
    const port = said ?? String(checkoutPort(resolve(import.meta.dir, '..')));
    expect(String(harness.TOOL_PORT)).toBe(port);
    expect(harness.TOOL_ORIGIN).toBe(`http://127.0.0.1:${port}`);
    expect(harness.TOOL_DIST).toEndWith(port === '7740' ? 'target/design-tool-web' : `target/design-tool-web-${port}`);
    // The module loaded first keeps the port it read (what the harness no longer trusts blindly).
    if (said === undefined) expect(first.TOOL_PORT).toBe(7740);
  } finally { if (said === undefined) delete process.env.EXACT_TOOL_PORT; else process.env.EXACT_TOOL_PORT = said; }
});
