// ⌘K's New thread without a project starts on the primary environment (T3 Code v0.1.5 CommandPalette.tsx
// scratchTargetEnvironmentId, #26301): the rows show while any machine offers a folder for threads without a project,
// not only the one on screen.
import { afterEach, describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { newThreadInItems, rootActions } from '../../src/palette/palette';
import { primary, type PrimaryTarget } from '../../src/connections/local-primary';
import { fleet, type FleetEntry } from '../../src/connections/settings-b-fleet';

function client(scratch = ''): T3Client {
  const c = new T3Client();
  Object.assign(c, { connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 1, environmentId: 'env-remote' });
  c.config = { environment: { label: 'Remote', capabilities: {} }, scratchWorkspaceRoot: scratch, settings: {}, keybindings: [] };
  c.shell = { sequence: 1, projects: [{ id: 'p1', title: 'Parity fixture', workspaceRoot: '/repo', updatedAt: '2026-10-01T00:00:00.000Z' }], threads: [] };
  c.projectId = 'p1';
  return c;
}
const offers = (c: T3Client) => [rootActions(c).some(item => item.row.key === 'new-thread-without-project'), newThreadInItems(c).some(item => item.row.key === 'new-thread-in:no-project')];

afterEach(() => { fleet.entries.delete('primary-key'); primary.target = null; });

describe('New thread without a project', () => {
  test('shows on a machine without the folder when the primary offers one, and hides when none does', () => {
    expect(offers(client())).toEqual([false, false]);
    primary.target = { environmentId: 'env-primary' } as PrimaryTarget;
    fleet.entries.set('primary-key', { key: 'primary-key', environmentId: 'env-primary', phase: 'connected', generation: 2, synchronized: 2,
      config: { scratchWorkspaceRoot: '/Users/me/.t3/scratch' }, shell: { projects: [], threads: [] } } as unknown as FleetEntry);
    expect(offers(client())).toEqual([true, true]);
    // A primary still synchronizing offers nothing yet.
    (fleet.entries.get('primary-key') as unknown as { synchronized: number }).synchronized = 1;
    expect(offers(client())).toEqual([false, false]);
    expect(offers(client('/remote/scratch'))).toEqual([true, true]);
  });
});
