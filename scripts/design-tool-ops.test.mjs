// The design tool's token operations on a real design server, as a person drives them (split from
// design-tool.test.mjs, its heaviest drive, so a lane runs it beside the others).
import { expect, test } from 'bun:test';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { ROOT, STUB_APP, drive, edit, proxied, toolTurns } from './design-tool-harness.mjs';

toolTurns();

test('expressions are kept unless pinned, "only this mode" adds a when, redo writes it again, and tokens are added, renamed and removed; undo restores an unformatted line', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-ops-'));
  // Not in fmt's layout: undo must give these bytes back, not fmt's.
  const primary = '  text.primary   =   light-dark(#0f1419, #e7e9ea)  pair=surface.page';
  const original = readFileSync(resolve(ROOT, 'apps/design-tool/stub/dials.tamber'), 'utf8').replace('  text.primary = light-dark(#0f1419, #e7e9ea)  pair=surface.page', primary);
  expect(original).toContain(primary);
  writeFileSync(join(app, 'app.tamber'), original);
  writeFileSync(join(app, 'app.contract'), STUB_APP);
  const tool = await proxied(app);
  try {
    const replies = await drive(tool.url, `design-tool-ops-${process.pid}`, [
      'type name Ada', 'tap save-name', 'clock data',
      // accent.soft follows accent and lift: its inputs, and a pin to write
      // a value, which the checker refuses (lift would move no colour),
      // saying why.
      'tap cell-accent.soft-0-light', 'clock data', 'state', 'tap pin', 'state',
      'type branch-value #123456', 'type branch-value key Enter', 'clock data', 'state', 'tap cancel-edit',
      // accent.fill follows accent, which accent.soft still follows: its pin
      // is taken, one line, and undone to the bytes.
      'tap cell-accent.fill-0-light', 'clock data', 'tap pin', 'type branch-value #2070f0', 'type branch-value key Enter', 'clock data', 'tap apply', 'clock data',
      'clock +1', 'clock data', 'tap undo', 'clock data',
      // A light cell passes over the under-dark `when`s, and has no "only
      // this mode" to offer.
      'tap cell-surface.page-0-light', 'clock data', 'state',
      // The compact dark cell of surface.card has no `when` of its own.
      'tap cell-surface.card-1-dark', 'clock data', 'tap only-this-mode', 'clock data', 'tap apply', 'clock data',
      'clock +1', 'clock data', 'tap undo', 'clock data', 'tap redo', 'clock data', 'tap undo', 'clock data',
      'clock +1', 'clock data', 'type new-name surface.raised', 'type new-value light-dark(#ffffff, #202020)', 'tap add-token', 'clock data', 'tap apply', 'clock data',
      'clock +1', 'clock data', 'tap cell-surface.raised-0-light', 'clock data', 'type rename-to surface.lifted', 'tap rename', 'clock data', 'tap apply', 'clock data',
      'clock +1', 'clock data', 'tap cell-surface.lifted-0-light', 'clock data',
      // Settled before the dialog opens: a settle runs the tool's follow task (a commit later), whose read of a file
      // the rename wrote reloads the branch panel, and with it the dialog, were it open.
      'clock settle', 'clock data', 'tap remove', 'tap confirm-remove-yes', 'clock settle', 'clock data', 'tap apply', 'clock data',
      // The unformatted line itself, edited and undone.
      'clock +1', 'clock data', 'tap cell-text.primary-0-light', 'clock data', 'type branch-value light-dark(#000000, #e7e9ea)', 'type branch-value key Enter', 'clock data', 'tap apply', 'clock data',
      'clock +1', 'clock data', 'tap undo', 'clock data', 'state',
    ]);
    const states = replies.filter(r => r.resources);
    expect(states[0].resources.branch).toMatchObject({ token: 'accent.soft', expression: true, inputs: [], dials: [{ name: 'accent', group: 'Colour', more: false }, { name: 'lift', group: 'Colour', more: false }] });
    expect(states[1].slots.pinning).toBe(true);
    expect(states[2].resources.proposal.ok).toBe(false);
    expect(states[2].resources.proposal.error).toContain('tamber-dial-inert');
    expect(states[3].resources.branch).toMatchObject({ token: 'surface.page', when: '', own: '', exact: true });
    const w = tool.writes;
    const pin = w.splice(0, 2);
    expect(pin.map(x => [x.method, x.ok])).toEqual([['transaction', true], ['undo', true]]);
    const fill = '  accent.fill = accent';
    expect(edit(original, pin[0].after)).toEqual({ at: original.split('\n').indexOf(fill), removed: [fill], added: ['  accent.fill = #2070f0'] });
    expect(pin[1].after).toBe(original);
    expect(w.map(x => [x.method, x.ok])).toEqual([
      ['transaction', true], ['undo', true], ['transaction', true], ['undo', true],
      ['transaction', true], ['transaction', true], ['transaction', true],
      ['transaction', true], ['undo', true],
    ]);
    const lines = original.split('\n');
    const card = '    when dark-style=dim: light-dark(#f1f3f5, #1e2732)';
    const mode = { at: lines.indexOf(card) + 1, removed: [], added: ['    when dark-style=black density=compact: light-dark(#f1f3f5, #16181c)'] };
    expect(edit(original, w[0].after)).toEqual(mode);
    expect(w[1].after).toBe(original);
    expect(w[2].after).toBe(w[0].after);
    expect(w[3].after).toBe(original);
    expect(edit(original, w[4].after)).toMatchObject({ removed: [], added: ['  surface.raised = light-dark(#ffffff, #202020)'] });
    expect(edit(w[4].after, w[5].after)).toMatchObject({ removed: ['  surface.raised = light-dark(#ffffff, #202020)'], added: ['  surface.lifted = light-dark(#ffffff, #202020)'] });
    expect(w[6].after).toBe(original);
    const at = lines.indexOf(primary);
    const changedPrimary = edit(original, w[7].after);
    expect(changedPrimary.at).toBe(at);
    expect(changedPrimary.removed).toEqual([primary]);
    expect(changedPrimary.added).toHaveLength(1);
    expect(changedPrimary.added[0]).toContain('#000000');
    expect(w[8].after).toBe(original);
  } finally {
    await tool.close();
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);
