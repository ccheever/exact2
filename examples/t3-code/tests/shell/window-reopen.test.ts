// The reopen after ⌘W. With keepRunning the window is only hidden; the reference's `activate` makes a new window
// (DesktopLifecycle.ts, DesktopWindow.activate) that loads `/`, whose IndexDraftLanding opens the most recent
// project's draft. The page hears the reopen as a `pageshow` with `persisted` (exactPage().restores); a minimize
// changes only its visibility.
import { expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { shellLocal } from '../../src/shell/shell-commands';
import { appPath, appText } from '../support/app-files';
import { contractComponent, type Env } from '../support/contract-action-fixture';

test('the reopen opens the most recent project\'s draft, and the closed thread keeps its draft', async () => {
  const client = new T3Client();
  client.environmentId = 'local';
  client.shell = { sequence: 1, projects: [{ id: 'a', title: 'Alpha', updatedAt: '2026-10-01T00:00:00Z' }, { id: 'b', title: 'Beta' }],
    threads: [{ id: 't1', projectId: 'a', latestUserMessageAt: '2026-09-01T00:00:00Z' }, { id: 't2', projectId: 'b', latestUserMessageAt: '2026-10-02T00:00:00Z' }] };
  client.projectId = 'a'; client.threadId = 't1';
  client.local.drafts['local:t1'] = 'half a message';
  client.local.drafts['local:new:b'] = 'project draft';
  client.connection = 'connected';
  client.subscriptions.thread = 'subscription-t1';
  const calls: unknown[] = [];
  const native = { available: true, watch() {}, later: async (request: unknown) => {
    calls.push(request);
    return { ok: true, generation: client.generation, value: {} };
  } } as unknown as Native;
  expect(await shellLocal(client, native, 'reopen', '', '')).toBe('sidebar:new-thread');
  expect(client).toMatchObject({ projectId: 'b', threadId: '', thread: null });
  expect(client.subscriptions.thread).toBeUndefined();
  expect(client.draft).toBe('project draft');
  expect(client.local.drafts).toEqual({ 'local:t1': 'half a message', 'local:new:b': 'project draft' });
  expect(calls).toEqual([{ op: 'unsubscribe', key: 'thread', generation: client.generation }]);
});

test('reopening leaves Settings, utility pages and the palette, then loads and focuses the draft composer', () => {
  const root = contractComponent('app.contract', 'T3Code');
  const state: Env = { settingsOpen: true, utilityPage: 'pull-requests', paletteOpen: true, draftOwner: 'local:t1', jumpRequest: 2 };
  const commands: unknown[][] = [], focused: unknown[] = [];
  root.call('windowReopened', state, { command: (...args: unknown[]) => commands.push(args) }, []);
  expect(state).toMatchObject({ settingsOpen: false, utilityPage: '', paletteOpen: false });
  expect(commands).toEqual([['shelllocal:reopen', '', '', 0]]);
  root.call('fragmentOpened', state, {
    fragmentChanged: { message: 'sidebar:new-thread' },
    focus: (target: unknown) => focused.push(target),
  }, []);
  expect(state).toMatchObject({ draftOwner: '__unset__', jumpRequest: 3 });
  expect(focused).toEqual(['composer']);
});

test('the reopen task keys each restored page, independently of visibility and focus', async () => {
  const app = await appText('app.contract');
  const shapes = await Bun.file(appPath('app-shapes.contract')).text();
  expect(shapes).toMatch(/shape Page\n(  \w+: \w+\n)*  restores: number/);
  expect(app).toMatch(/task windowReopen when page\.restores > 0 key=page\.restores\n    after\(1, windowReopened\)/);
});
