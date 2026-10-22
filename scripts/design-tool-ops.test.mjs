// The design tool's token operations on a real design server, as a person drives them, and an agent's
// held proposals reviewed there (split from design-tool.test.mjs, two of its heaviest drives, so a lane
// runs them beside the others).
import { expect, test } from 'bun:test';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { spawnOwned } from './test-children.mjs';
import { ROOT, STUB_APP, drive, edit, proxied, repo, toolTurns } from './design-tool-harness.mjs';

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
      // The tool's follow task (a commit later) runs on a moving clock, and its read of the file the rename wrote
      // reloads the branch panel, and with it the dialog, were it open: `clock +1` runs it if it is due, and no op
      // moves the clock between opening the dialog and confirming, so a changes answer that comes later arms it
      // only to run after. (Not `clock settle`: follow re-arms on each answer of the changes poll, so a settle
      // spends its 16 rounds on it, about 25 s each.)
      'clock +1', 'clock data', 'tap remove', 'tap confirm-remove-yes', 'clock +1', 'clock data', 'tap apply', 'clock data',
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

test('an agent\'s write arrives attributed within one poll, and held proposals are accepted whole, in part and recomputed when stale', async () => {
  const { app } = repo('agents');
  const read = () => readFileSync(join(app, 'app.tamber'), 'utf8');
  let tool, accepts = 0;
  const agentWrite = async () => {
    const digest = createHash('sha256').update(readFileSync(join(app, 'app.tamber'))).digest('hex');
    const op = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'op', tool.origin, JSON.stringify({ method: 'transaction', client: 'agent:codex', files: { 'app.tamber': digest }, ops: [{ op: 'set-token', name: 'surface.card', value: 'light-dark(#f1f3f5, #17191d)' }] })], { cwd: ROOT, stdio: 'ignore' });
    expect(await new Promise(ok => op.once('close', ok))).toBe(0);
  };
  let explains = 0;
  tool = await proxied(app, async request => {
    // The agent writes just before the tool's first explain; a hand edit
    // lands just before the third accept, so that proposal is stale.
    if (request.method === 'explain' && ++explains === 1) await agentWrite();
    if (request.method === 'accept' && ++accepts === 3) writeFileSync(join(app, 'app.tamber'), read().replace('accent.fill = accent', 'accent.fill = #3070f0'));
  });
  try {
    const { token } = await (await fetch(`${tool.origin}/__tamber/token`)).json();
    const ask = async request => (await fetch(`${tool.origin}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify(request) })).json();
    const p1 = (await ask({ method: 'propose', hold: true, client: 'agent:codex', ops: [{ op: 'set-token', name: 'text.primary', value: 'light-dark(#101010, #e7e9ea)' }, { op: 'set-token', name: 'ink.on-brand', value: '#fefefe' }] })).ok.proposal;
    const p2 = (await ask({ method: 'propose', hold: true, client: 'agent:claude', ops: [{ op: 'set-token', name: 'accent.fill', value: '#2070f0' }] })).ok.proposal;
    const id = n => p1.replace(/-1$/, `-${n}`);
    expect(p2).toBe(id(2));
    const replies = await drive(tool.url, `design-tool-agents-${process.pid}`, [
      'type name Ada', 'tap save-name', 'clock data',
      'tap cell-surface.page-0-light', 'clock data', 'clock +1', 'clock data', 'clock +1', 'clock data', 'state',
      // Part of p1: the server recomputes it as a new proposal, reviewed afresh.
      `tap pick-${p1}-0`, `tap accept-picked-${p1}`, 'clock data', 'clock +1', 'clock data', 'state',
      `tap accept-${id(3)}`, 'clock data', 'clock +1', 'clock data', 'state',
      // p2, with the file moved since: recomputed, then accepted.
      `tap accept-${p2}`, 'clock data', 'clock +1', 'clock data', 'state',
      `tap accept-${id(4)}`, 'clock data', 'clock +1', 'clock data', 'state',
      // An accepted proposal is one step: undone, and redone.
      'tap undo', 'clock data', 'clock +1', 'clock data', 'tap redo', 'clock data', 'clock +1', 'clock data', 'state',
      // And highlights fade, oldest first, one each 3 s.
      ...Array.from({ length: 8 }, () => ['clock +3000', 'clock data']).flat(), 'state',
    ]);
    const states = replies.filter(r => r.resources);
    const [first, part, whole, stale, fresh, redone, faded] = states;
    expect(first.resources.changeset.entries[0]).toMatchObject({ who: 'agent:codex', mine: false });
    expect(first.resources.changeset.entries[0].text).toBe('app.tamber: surface.card light-dark(#f1f3f5ff, #16181cff) → light-dark(#f1f3f5ff, #17191dff) · agent:codex');
    expect(first.resources.lit).toMatchObject({ names: ['surface.card'], whos: ['agent:codex'] });
    // Only the cells that moved: the dark side, where the default applies.
    expect(first.resources.lit.cells).toEqual(['surface.card|0-dark', 'surface.card|1-dark']);
    // Each with the tokens it changes (a role that follows one among them, the params that alias one, in full).
    expect(first.resources.changes.held.map(h => [h.proposal, h.client, [...h.tokens].sort(), h.items.map(i => i.text)])).toEqual([
      [p1, 'agent:codex', ['-exact-label', 'ink.on-brand', 'text.primary'], ['text.primary = light-dark(#101010, #e7e9ea) (app.tamber)', 'ink.on-brand = #fefefe (app.tamber)']],
      [p2, 'agent:claude', ['accent.fill', 'app.contract#Card.fill', 'app.tamber#MapCard.pin'], ['accent.fill = #2070f0 (app.tamber)']],
    ]);
    expect(part.slots.written.note).toContain(`as ${id(3)}`);
    const recomputed = part.resources.changes.held.find(h => h.proposal === id(3));
    expect(recomputed).toMatchObject({ replaces: p1 });
    expect([...recomputed.tokens].sort()).toEqual(['-exact-label', 'text.primary']);
    expect(recomputed.diff).toContain('#101010');
    expect(recomputed.diff).not.toContain('#fefefe');
    expect(whole.slots.written.note).toBe(`Accepted ${id(3)}.`);
    expect(stale.slots.written.note).toContain(`as ${id(4)}`);
    const restaled = stale.resources.changes.held.find(h => h.proposal === id(4));
    expect(restaled).toMatchObject({ replaces: p2, client: 'agent:claude' });
    // Its diff is the new one, from the hand edit's value.
    expect(restaled.diff).toContain('-  accent.fill = #3070f0');
    expect(fresh.slots.written.note).toBe(`Accepted ${id(4)}.`);
    expect(fresh.resources.changes.held).toEqual([]);
    expect(tool.writes.map(w => [w.method, w.ok])).toEqual([['accept', true], ['accept', true], ['accept', true], ['accept', true], ['undo', true], ['transaction', true]]);
    expect(tool.writes[4].after).toBe(tool.writes[2].after);
    expect(tool.writes[5].after).toBe(tool.writes[3].after);
    expect(redone.slots.written).toMatchObject({ ok: true, last: 'redo' });
    // Redone as its proposer's.
    expect(redone.resources.changeset.entries[0].who).toBe('agent:claude');
    expect(redone.resources.changeset.entries[0].text).toContain('redone by design-tool:Ada');
    expect(faded.resources.lit.names).toEqual([]);
    const now = read();
    expect(now).toContain('text.primary = light-dark(#101010, #e7e9ea)');
    expect(now).not.toContain('#fefefe');
    expect(now).toContain('accent.fill = #2070f0');
    expect(now).not.toContain('accent.fill = #3070f0');
  } finally {
    await tool.close();
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);
