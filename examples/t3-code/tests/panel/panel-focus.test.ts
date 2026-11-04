// startWorkspaceFocus (T3 Code lib/workspaceFocus.ts at 5d3d78ab93, MIT, see LICENSE-T3): a person's action that opens
// the right panel or changes the surface it shows asks for the panel's focus, so opening Files focuses "Search files".
// An opening that is not the person's asks nothing, nor does moving to another thread.
import { expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import { panelState, panelView, type Surface } from '../../src/panel/panel-surfaces';

const surface = (id: string): Surface => ({ id, kind: id as Surface['kind'], path: '', line: 0, reveal: 0 });

test('a person opening the panel or another tab asks for its focus; anything else does not', async () => {
  const client = {
    environmentId: 'env', threadId: 't1', projectId: 'p1', ready: true, revision: 0, generation: 1, diffOpen: false, scopes: [],
    get draftKey() { return `env:${this.threadId}`; }, config: { environment: { capabilities: {} } }, local: { clientSettings: {} },
    shell: { projects: [{ id: 'p1', title: 'p', workspaceRoot: '/repo' }], threads: [{ id: 't1', projectId: 'p1' }, { id: 't2', projectId: 'p1' }] },
  } as unknown as T3Client;
  const asks = async () => (await panelView(client, null, 0)).focusRequest;
  const state = panelState(client);
  expect(await asks()).toBe(0);
  Object.assign(state, { surfaces: [surface('files')], active: 'files', visible: true, userRevision: 1 }); // the person opens Files
  expect(await asks()).toBe(1);
  expect(await asks()).toBe(1);
  Object.assign(state, { surfaces: [surface('files'), surface('pull-requests')], active: 'pull-requests' }); // a turn opens a tab
  expect(await asks()).toBe(1);
  Object.assign(state, { active: 'files', userRevision: 2 }); // the person picks the Files tab again
  expect(await asks()).toBe(2);
  Object.assign(state, { visible: false, userRevision: 3 }); // the person hides the panel
  expect(await asks()).toBe(2);
  client.threadId = 't2'; // another thread's panel, then back
  expect(await asks()).toBe(2);
  client.threadId = 't1';
  expect(await asks()).toBe(2);
  Object.assign(state, { visible: true, userRevision: 4 }); // the person shows it again
  expect(await asks()).toBe(3);
});
