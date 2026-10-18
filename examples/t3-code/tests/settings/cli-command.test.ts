import { expect, test } from 'bun:test';
import { obj } from '../../src/client/domain';
import { T3Client } from '../../src/client/client';
import { cliCommandRow, readCliCommand } from '../../src/settings/cli-command';
import { applyCoreSetting } from '../../src/settings/settings-core';
import type { Native } from '../../src/client/protocol';
import { toasts } from '../../src/shell/toast';

test('the desktop command row follows installation, PATH and shadowing state', () => {
  expect(cliCommandRow({ supported: false, installedPath: null, onPath: false })).toBeNull();
  expect(cliCommandRow({ supported: true, installedPath: null, onPath: false })).toMatchObject({ label: 'Install', value: 'install', description: "Run T3 Code's CLI as `t3` from any terminal." });
  expect(cliCommandRow({ supported: true, installedPath: '/bin/t3', onPath: true })).toMatchObject({ label: 'Remove', value: 'uninstall', description: 'Installed at /bin/t3. Open a new terminal to use it.' });
  expect(cliCommandRow({ supported: true, installedPath: '/bin/t3', onPath: false })?.description).toContain('not on your PATH yet');
  expect(cliCommandRow({ supported: true, installedPath: null, onPath: false, shadowedBy: '/other/t3' })?.description).toBe("Another t3 at /other/t3 runs first in a new terminal. Remove it to use T3 Code's.");
});

test('the General command uses the local desktop bridge and toasts install/remove failures', async () => {
  const client = new T3Client(), calls: string[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const op = String(obj(input).op); calls.push(op);
    return op === 'cliCommandState' ? { ok: true, generation: Number(obj(input).generation), value: { supported: true, installedPath: null, onPath: false } }
      : { ok: false, generation: Number(obj(input).generation), error: { kind: 'CliCommand', message: 'The existing command was kept.', uncertain: false } };
  } };
  expect(await readCliCommand(client, native)).toMatchObject({ id: 'cli-command', label: 'Install' });
  await applyCoreSetting(client, native, 'cli-command:|||', 'install');
  expect(toasts(client).at(-1)).toMatchObject({ title: 'Could not install t3', description: 'The existing command was kept.' });
  await applyCoreSetting(client, native, 'cli-command:|||', 'uninstall');
  expect(toasts(client).at(-1)?.title).toBe('Could not remove t3');
  expect(calls).toEqual(['cliCommandState', 'cliCommandInstall', 'cliCommandUninstall']);
  expect(await readCliCommand(client, null)).toBeNull();
});
