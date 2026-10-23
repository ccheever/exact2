import { expect, test } from 'bun:test';
import { mkdtempSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { checkOp } from '../../scripts/agent-check.mjs';

// Build first with `bun host/web/build.mjs fieldnotes`.
test('Fieldnotes text remains readable in both schemes', async () => {
  const cache = mkdtempSync(resolve(import.meta.dir, '.contrast-'));
  let session;
  try {
    session = await open({
      host: 'web', app: 'fieldnotes', size: [1100, 900], storage: 'contrast',
      env: { XDG_CACHE_HOME: cache, EXACT_AGENT_STORAGE_FRESH: '1', EXACT_AGENT_EPHEMERAL_PORT: '1' },
    });
    const failures = [];
    const check = async (screen, target) => {
      await session.clock('settle');
      await session.tree(target);
      const result = await checkOp(session, ['both']);
      expect(result.check.map(report => report.scheme)).toEqual(['light', 'dark']);
      for (const report of result.check) {
        failures.push(...report.findings.filter(f => f.kind === 'contrast')
          .map(f => ({ screen, scheme: report.scheme, ...f })));
      }
    };
    await check('empty notebook', 'empty-notebook');
    await session.type('note-title', 'A walk in the woods');
    await session.type('note-body', 'Ferns beside the path.');
    await session.tap('pin-note');
    await session.tap('save-note');
    await session.clock('settle');
    const saved = (await session.state()).derives.editingId;
    expect(saved).not.toBe('');
    await session.tap('new-note');
    await session.tap(`note-${saved}`);
    await session.clock('settle');
    await session.type('note-body', 'Ferns beside the path. Remember the clearing.');
    await check('editor with draft', 'unsaved');
    await session.tap('find-notes');
    await session.type('search', 'woods');
    await check('search', `note-${saved}`);
    await session.tap('discard');
    await session.tap('close-search');
    await session.tap('delete-note');
    await check('delete confirmation', 'confirm-delete');
    await session.tap('cancel-delete');
    await session.tap('backups');
    await check('backups', 'backup-panel');
    await session.tap('save-backup');
    await check('saved backup', 'backup-text');
    await session.tap('restore-backup');
    await check('restore confirmation', 'confirm-restore');
    await session.tap('cancel-restore');
    await session.type('import-text', 'not a backup');
    await session.tap('restore-backup');
    await session.tap('confirm-restore');
    await session.clock('settle');
    expect((await session.state()).slots.status.failed).toBe(true);
    await check('invalid backup', 'status-message');
    expect(failures).toEqual([]);
  } finally {
    await session?.close();
    rmSync(cache, { recursive: true, force: true });
  }
}, 30000);
