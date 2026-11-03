// A managed policy's permission ceiling in the composer (ChatComposer, ComposerFooterControls at 5d3d78ab93).
import { describe, expect, test } from 'bun:test';
import { snapshot } from '../../src/client/presentation';
import { opened } from '../support/composer-controls-fixture';

const policy = { organization: 'Example Corp', lockedSettings: [], limits: { maxRuntimeMode: 'auto-accept-edits' } };

describe('managed runtime mode ceiling', () => {
  test('modes above the ceiling stay listed, disabled with the reason', async () => {
    const { client } = await opened();
    client.config.managedPolicy = policy;
    const runtimes = snapshot(client).composer.runtimes as { mode: string; disabledReason: string }[];
    expect(runtimes.map(option => [option.mode, option.disabledReason])).toEqual([
      ['approval-required', ''], ['auto-accept-edits', ''],
      ['auto', 'Example Corp limits threads to Auto-accept edits.'], ['full-access', 'Example Corp limits threads to Auto-accept edits.'],
    ]);
  });

  test('a thread above the ceiling cannot send, and a warning offers the broadest mode left', async () => {
    const { client, native, command } = await opened();
    client.config.managedPolicy = policy;
    expect(client.runtimeMode).toBe('full-access');
    const view = snapshot(client);
    expect(view.canSend).toBe(false);
    expect(view.composer.sendStatus).toBe('Example Corp limits threads to Auto-accept edits.');
    const notice = (view.composer.notices as { id: string; variant: string; icon: string; title: string; action: string; actionValue: string; actionLabel: string }[])
      .find(item => item.id === 'managed-runtime-mode');
    expect(notice).toMatchObject({ variant: 'warning', icon: 'building', title: 'Example Corp limits threads to Auto-accept edits.',
      action: 'runtime', actionValue: 'auto-accept-edits', actionLabel: 'Switch to Auto-accept edits' });
    const before = native.committed.length;
    await command('send', '', 'Hello');
    expect(client.error).toBe('Example Corp limits threads to Auto-accept edits.');
    expect(native.committed.length).toBe(before);
    await command(notice!.action, '', notice!.actionValue);
    expect(client.runtimeMode).toBe('auto-accept-edits');
    const after = snapshot(client);
    expect(after.composer.sendStatus).toBe('');
    expect((after.composer.notices as { id: string }[]).some(item => item.id === 'managed-runtime-mode')).toBe(false);
  });

  test('an unmanaged environment allows every mode', async () => {
    const { client } = await opened();
    const view = snapshot(client);
    expect((view.composer.runtimes as { disabledReason: string }[]).every(option => option.disabledReason === '')).toBe(true);
    expect(view.canSend).toBe(true);
  });
});
