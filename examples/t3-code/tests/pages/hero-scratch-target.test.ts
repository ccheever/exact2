// DraftHeroHeadline at 5d3d78ab93 (MIT, see LICENSE-T3): "or start without a project" follows scratchTargetEnvironmentId,
// the primary's folder first, so a project on a machine without a scratch folder still offers it while this Mac has one.
import { afterEach, expect, test } from 'bun:test';
import { pagesHome } from '../../src/pages/pages-home';
import { fleet, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { opened } from '../support/composer-controls-fixture';
import { primaryAt, resetPrimary } from '../support/local-primary-fixture';

afterEach(() => { fleet.entries.clear(); resetPrimary(); });

const thisMac = (scratchWorkspaceRoot: string | undefined) => ({ key: 'local', origin: 'http://127.0.0.1:3773', environmentId: 'env-local', phase: 'connected',
  generation: 1, synchronized: 1, shell: { projects: [], threads: [] }, config: scratchWorkspaceRoot ? { scratchWorkspaceRoot } : {} }) as unknown as FleetEntry;

test('a remote project offers the line while this Mac has a scratch folder, and not when no machine has one', async () => {
  const { client } = await opened();
  client.environmentId = 'env-remote';
  client.config = { ...client.config, scratchWorkspaceRoot: undefined };
  primaryAt('http://127.0.0.1:3773', 'env-local');
  fleet.entries.set('local', thisMac('/Users/me/.t3/scratch'));
  expect((await pagesHome(client, null, false)).scratchLine).toBe(true);
  fleet.entries.set('local', thisMac(undefined));
  expect((await pagesHome(client, null, false)).scratchLine).toBe(false);
});
