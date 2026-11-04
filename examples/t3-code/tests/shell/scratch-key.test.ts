// chat.newWithoutProject (T3 Code chatRouteCommands.ts at 5d3d78ab93, MIT, see LICENSE-T3) starts the thread on the
// machine useScratchProject's scratchEnvironmentId picks, the primary first, so ⌥⌘N works while the machine on screen
// offers no folder for threads without a project.
import { afterEach, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { fleet, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { keyboardDispatch } from '../../src/shell/keyboard-dispatch';

afterEach(() => fleet.entries.clear());
const KEY = 'http://127.0.0.1:16231\nenv-b';
const context = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false };

function screen(): T3Client {
  const client = new T3Client();
  Object.assign(client, { connection: 'connected', environmentId: 'env-a', loaded: true, configLive: true, shellLive: true, presentation: {} });
  client.config = { environment: { label: 'Lane Second' }, settings: {}, providers: [],
    keybindings: [{ command: 'chat.newWithoutProject', shortcut: { key: 'n', modKey: true, altKey: true } }] };
  client.shell = { projects: [], threads: [], sequence: 1 };
  return client;
}
const other = (scratchWorkspaceRoot: string): FleetEntry => ({ key: KEY, origin: 'http://127.0.0.1:16231', environmentId: 'env-b', phase: 'connected', message: '',
  traceId: '', generation: 2, synchronized: 2, lastEvent: 0, subscriptions: {}, config: { environment: { label: 'This Mac' }, scratchWorkspaceRoot },
  shell: { projects: [], threads: [] }, scopes: ['orchestration:operate'], error: '', requested: true } as unknown as FleetEntry);
const scratchKeys = (client: T3Client) => keyboardDispatch(client, [], '', '', context).filter(item => item.command === 'chat.newWithoutProject');

test('⌥⌘N starts a thread without a project on another machine when the one on screen offers no folder', () => {
  const client = screen();
  expect(scratchKeys(client)).toEqual([]);
  fleet.entries.set(KEY, other('/tmp/scratch'));
  expect(scratchKeys(client).map(item => [item.chord, item.kind, item.target])).toEqual([['Meta+Alt+n', 'flow', 'scratch']]);
});
