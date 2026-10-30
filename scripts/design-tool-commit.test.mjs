// The design tool's commit against a real design server (split from design-tool.test.mjs, so a lane runs it beside
// the others): the session's writes committed, history and revert, and the refusals that say why.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { drive, edit, proxied, repo, toolTurns } from './design-tool-harness.mjs';

toolTurns();

/** An app with the fixture design in a git repository of its own: `app`,
 * and git in it. */
test('commit takes the session\'s writes, leaves other staging and never pushes; history reads it as edits, and revert holds a past commit\'s inverse', async () => {
  const { app, git } = repo('commit');
  // A past design commit to revert, other staging to leave alone, and a
  // remote that must not move.
  writeFileSync(join(app, 'app.tamber'), readFileSync(join(app, 'app.tamber'), 'utf8').replace('surface.inset = light-dark(#f8f9fa, #101214)', 'surface.inset = light-dark(#f8f9fb, #101214)'));
  git('commit', '-q', '-am', 'a warmer inset');
  const past = git('rev-parse', 'HEAD').stdout.trim();
  writeFileSync(join(app, 'notes.txt'), 'one\n'); git('add', 'notes.txt'); git('commit', '-q', '-m', 'notes');
  writeFileSync(join(app, 'notes.txt'), 'two\n'); git('add', 'notes.txt');
  const staged = git('ls-files', '-s', 'notes.txt').stdout;
  const remote = mkdtempSync(join(tmpdir(), 'exact-design-tool-remote-'));
  spawnSync('git', ['init', '-q', '--bare', remote]);
  git('remote', 'add', 'origin', remote); git('push', '-q', 'origin', 'HEAD:main');
  const pushed = spawnSync('git', ['ls-remote', remote], { encoding: 'utf8' }).stdout;
  // A hand edit, just before history is read: its row says what moved.
  const tool = await proxied(app, request => {
    if (request.method === 'history') writeFileSync(join(app, 'app.tamber'), readFileSync(join(app, 'app.tamber'), 'utf8').replace('tone.warm = oklch(0.7 0.1 warmth)', 'tone.warm = oklch(0.71 0.1 warmth)'));
  });
  try {
    const replies = await drive(tool.url, `design-tool-commit-${process.pid}`, [
      'type name Ada', 'tap save-name', 'clock data',
      'type dial-hue-stepper 210', 'clock data', 'tap apply', 'clock data',
      'clock +1', 'clock data', 'clock +1', 'clock data', 'state',
      'tap commit', 'clock data', 'state',
      // (the hand edit lands as history is read; the changes' long poll brings its row when the server's watcher sees
      // it, in real time, which the virtual clock doesn't wait on: waited for by that row, not any reply)
      'tap toggle-history', 'clock data', 'clock until state changes.recent.0.who == "changed on disk" within 20000', 'clock data', 'state',
      `tap revert-${past.slice(0, 12)}`, 'clock data', 'clock +1', 'clock data', 'clock +1', 'clock data', 'state',
    ]);
    const states = replies.filter(r => r.resources);
    const entry = states[0].resources.changeset.entries[0];
    expect(entry).toMatchObject({ who: 'design-tool:Ada', mine: true, committed: false });
    expect(entry.text).toMatch(/^app\.tamber: tone\.base oklch\(.+\) → oklch\(.+\) · design-tool:Ada$/);
    expect(entry.diff).toContain('+  hue: number = 210');
    const made = git('log', '-1', '--format=%H%n%B').stdout;
    expect(made).toContain('design: 1 change by design-tool:Ada');
    expect(made).toContain(entry.text);
    expect(git('show', '--name-only', '--format=', 'HEAD').stdout.trim()).toBe('app.tamber');
    expect(git('ls-files', '-s', 'notes.txt').stdout).toBe(staged);
    expect(spawnSync('git', ['ls-remote', remote], { encoding: 'utf8' }).stdout).toBe(pushed);
    expect(states[1].slots.committed).toEqual({ ok: true, error: '', commit: made.slice(0, 12), files: ['app.tamber'], refused: [], edits: [{ n: 0, text: entry.text }] });
    expect(states[1].resources.changeset.entries[0].committed).toBe(true);
    // Nothing left to commit: Commit is off.
    expect(states[1].resources.scope).toEqual({ open: 0, refused: false, unavailable: false });
    const history = states[2].resources.history;
    expect(history.commits[0]).toMatchObject({ subject: 'design: 1 change by design-tool:Ada', files: [{ file: 'app.tamber', refused: '', edits: ['hue 200 → 210'] }] });
    expect(history.commits.find(c => c.commit === past).files[0].edits).toEqual(['surface.inset light-dark(#f8f9fa, #101214) → light-dark(#f8f9fb, #101214)']);
    expect(states[3].slots.reverted).toMatchObject({ ok: true, commit: past });
    // Held, not written: in the inbox, the file as it was.
    expect(states[3].resources.changes.held.map(h => h.proposal)).toContain(states[3].slots.reverted.proposal);
    expect(readFileSync(join(app, 'app.tamber'), 'utf8')).toContain('surface.inset = light-dark(#f8f9fb, #101214)');
    const hand = states[3].resources.changeset.entries.find(e => e.who === 'changed on disk');
    expect(hand.text).toBe('app.tamber: tone.warm oklch(0.7 0.1 20) → oklch(0.71 0.1 20) · changed on disk');
  } finally {
    await tool.close();
    rmSync(app, { recursive: true, force: true });
    rmSync(remote, { recursive: true, force: true });
  }
}, 900_000);

test('commit refuses a file dirty or staged at its baseline, or edited by hand between two writes, saying why', async () => {
  const cases = [
    ['dirty', ({ app }) => writeFileSync(join(app, 'app.tamber'), readFileSync(join(app, 'app.tamber'), 'utf8').replace('tone.warm = oklch(0.7 0.1 warmth)', 'tone.warm = oklch(0.71 0.1 warmth)')), 'it differed from HEAD when the session first wrote it'],
    ['staged', ({ app, git }) => { writeFileSync(join(app, 'app.tamber'), readFileSync(join(app, 'app.tamber'), 'utf8').replace('tone.warm = oklch(0.7 0.1 warmth)', 'tone.warm = oklch(0.71 0.1 warmth)')); git('add', 'app.tamber'); }, 'it was staged when the session first wrote it'],
    ['hand', () => {}, 'it was changed by someone other than this server'],
  ];
  for (const [name, before, why] of cases) {
    const r = repo(name);
    before(r);
    // Between the two writes, a hand edit: just before the tool proposes
    // the second.
    let proposals = 0;
    const tool = await proxied(r.app, request => {
      if (name === 'hand' && request.method === 'propose' && request.ops?.[0]?.op === 'set-token' && ++proposals === 1) writeFileSync(join(r.app, 'app.tamber'), readFileSync(join(r.app, 'app.tamber'), 'utf8').replace('tone.warm = oklch(0.7 0.1 warmth)', 'tone.warm = oklch(0.71 0.1 warmth)'));
    });
    try {
      const head = r.git('rev-parse', 'HEAD').stdout;
      const replies = await drive(tool.url, `design-tool-refuse-${name}-${process.pid}`, [
        'type name Ada', 'tap save-name', 'clock data',
        'type dial-hue-stepper 210', 'clock data', 'tap apply', 'clock data', 'clock +1', 'clock data',
        'tap cell-surface.page-2-dark', 'clock data', 'type branch-value light-dark(#ffffff, #101820)', 'type branch-value key Enter', 'clock data', 'tap apply', 'clock data', 'clock +1', 'clock data',
        'tap commit', 'clock data', 'tree commit-refused', 'state',
      ]);
      expect(tool.writes.map(w => [w.method, w.ok]), name).toEqual([['transaction', true], ['transaction', true]]);
      expect(replies.at(-1).slots.committed, name).toMatchObject({ ok: false, refused: [{ file: 'app.tamber', why }] });
      expect(JSON.stringify(replies.at(-2)), name).toContain(`app.tamber: ${why}`);
      expect(r.git('rev-parse', 'HEAD').stdout, name).toBe(head);
    } finally {
      await tool.close();
      rmSync(r.app, { recursive: true, force: true });
    }
  }
}, 900_000);
