// Back to the default right after Set on a library component's param, against a real design server (the second
// pass's #9; split from design-tool-ops.test.mjs, so a lane runs it beside the others).
import { expect, test } from 'bun:test';
import { cpSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { ROOT, drive, proxied, timed, toolTurns } from './design-tool-harness.mjs';

toolTurns();

test('Back to the default right after Set for this app on a package param is accepted the first time (the second pass\'s #9)', async () => {
  // Inside the checkout, where `@exact/cards` resolves as it does for apps/library-gate.
  const app = mkdtempSync(join(ROOT, 'target', 'design-tool-library-'));
  cpSync(resolve(ROOT, 'apps/library-gate'), app, { recursive: true });
  const original = readFileSync(join(app, 'app.tamber'), 'utf8');
  const unsets = [];
  // (The server's wait 3 s, not 25 s: `clock held` below returns on the write's push or, landed already, the next
  // heartbeat, inside its 20 s bound either way; tamber-timing.mjs.)
  const tool = await proxied(app, request => { if (request.method === 'propose' && request.ops?.some(o => o.op === 'unset-param')) unsets.push(request.ops); }, { env: timed({ POLL: 3_000 }) });
  try {
    await drive(tool.url, `design-tool-library-${process.pid}`, [
      'type name Ada', 'tap save-name', 'clock data',
      // Set for this app: written by the full name, as an exported library component's param is.
      'type dial-@exact/cards/post-card.contract#PostCard.surface-field #ff3366', 'type dial-@exact/cards/post-card.contract#PostCard.surface-field key Enter', 'clock data', 'tap apply', 'clock data',
      // The write's push read again (a design read follows it), then Back to the default at once.
      'clock +1', 'clock held', 'tap param-PostCard.surface-reset', 'clock data', 'tap apply', 'clock data',
    ]);
    expect(tool.writes.map(w => [w.method, w.ok, w.error ?? null])).toEqual([['transaction', true, null], ['transaction', true, null]]);
    expect(tool.writes[0].after).toContain('@exact/cards/post-card.contract#PostCard.surface = #ff3366');
    // Proposed once: a refusal (the pass saw a 400) would have the tool propose it again.
    expect(unsets.length).toBe(1);
    expect(tool.writes[1].after).toBe(original);
  } finally {
    await tool.close();
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);
