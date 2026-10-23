import { expect, test } from 'bun:test';
import { connected } from '../support/client-fixture';
import { arr, obj } from '../../src/client/domain';
import { updateEverythingMachines, updateEverythingState } from '../../src/sidebar/update-everything';
import { toasts } from '../../src/shell/toast';
import { sessionPermissions } from '../../src/connections/remote-scopes';

test('provider installs retain their transport after confirmation and later UI commands', async () => {
  const { client, native, command } = await connected();
  const advisory = { status: 'behind_latest', canUpdate: true, updateCommand: 'fixture-update', latestVersion: '2.0.0' };
  client.config.providers = ['codex', 'claude'].map(driver => ({ ...arr(client.config.providers)[0],
    driver, instanceId: driver, versionAdvisory: advisory }));
  const calls: string[] = [];
  let release!: () => void, finished!: () => void;
  const barrier = new Promise<void>(resolve => { release = resolve; });
  const done = new Promise<void>(resolve => { finished = resolve; });
  const original = native.later.bind(native);
  native.later = async input => {
    const request = obj(input);
    if (request.op === 'sidebarNotify' && calls.length === 2 && updateEverythingState(client).run === null) finished();
    if (request.method !== 'server.updateProvider') return original(input);
    const driver = String(obj(request.payload).provider); calls.push(driver);
    await barrier;
    return native.good({ providers: [{ instanceId: driver, driver, updateState: { status: 'succeeded' } }] });
  };
  await command('sidebar:ue:open');
  expect(updateEverythingState(client).dialog?.providers).toHaveLength(1);
  await command('sidebar:ue:confirm');
  expect(calls).toHaveLength(2);
  await command('select-thread', 't1');
  release();
  await done;
  await Promise.resolve(); // The empty server phase settles after its queued Promise.all.
  expect(calls.sort()).toEqual(['claude', 'codex']);
  expect(toasts(client).at(-1)?.kind).toBe('success');
});

test('server maintenance uses effective grants, including authoritative empty permissions', async () => {
  const { client } = await connected();
  for (const [session, permitted] of [
    [{ authenticated: true, scopes: ['orchestration:operate'], permissions: [] }, false],
    [{ authenticated: true, permissions: ['orchestration:operate'] }, false],
    [{ authenticated: true, permissions: ['environment:maintain'] }, true],
    [{ authenticated: true, scopes: ['orchestration:operate'] }, true],
  ] as const) {
    client.scopes = sessionPermissions(session);
    expect(updateEverythingMachines(client).find(machine => machine.environmentId === client.environmentId)?.canMaintain).toBe(permitted);
  }
});
