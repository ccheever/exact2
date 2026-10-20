// The reopen after ⌘W. With keepRunning the window is only hidden; the reference's `activate` makes a new window
// (DesktopLifecycle.ts, DesktopWindow.activate) that loads `/`, whose IndexDraftLanding opens the most recent
// project's draft. The page hears the reopen as a `pageshow` with `persisted` (exactPage().restores); a minimize
// changes only its visibility.
import { expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { shellLocal } from '../../src/shell/shell-commands';
import { appPath } from '../support/app-files';

test('the reopen opens the most recent project\'s draft, and the closed thread keeps its draft', async () => {
  const client = new T3Client();
  client.environmentId = 'local';
  client.shell = { sequence: 1, projects: [{ id: 'a', title: 'Alpha', updatedAt: '2026-10-01T00:00:00Z' }, { id: 'b', title: 'Beta' }],
    threads: [{ id: 't1', projectId: 'a', latestUserMessageAt: '2026-09-01T00:00:00Z' }, { id: 't2', projectId: 'b', latestUserMessageAt: '2026-10-02T00:00:00Z' }] };
  client.projectId = 'a'; client.threadId = 't1';
  client.local.drafts['local:t1'] = 'half a message';
  const native = { available: true, watch() {}, later: async () => ({ ok: true, generation: 0, value: {} }) } as unknown as Native;
  expect(await shellLocal(client, native, 'reopen', '', '')).toBe('sidebar:new-thread');
  expect(client).toMatchObject({ projectId: 'b', threadId: '' });
  expect(client.local.drafts['local:t1']).toBe('half a message');
});

test('the root runs it once for each return (exactPage().restores), leaving Settings, the utility pages and the palette', async () => {
  const app = await Bun.file(appPath('app.contract')).text();
  const shapes = await Bun.file(appPath('app-shapes.contract')).text();
  expect(shapes).toMatch(/shape Page\n(  \w+: \w+\n)*  restores: number/);
  const task = app.slice(app.indexOf('task windowReopen'), app.indexOf('action toastHovering'));
  expect(task).toContain('task windowReopen when page.restores > 0 key=page.restores');
  for (const line of ['settingsOpen = false', 'utilityPage = ""', 'paletteOpen = false', 'send fragmentChanged = command("shelllocal:reopen", "", "", 0)']) expect(task).toContain(line);
});
