// Settings › Projects pins and archives a project (T3 Code 2359643cff, MIT, see LICENSE-T3:
// components/settings/ProjectSettingsPanel.tsx Pin and Archive rows, SettingsScopeNotice.tsx and
// SettingsScopeSentence.tsx ProjectScopeMenu's Archived mark, hooks/useProjectGroupUpdate.ts).
import { afterEach, describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { obj, type Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import type { OpOut } from '../../src/client/client-ops';
import { fleet } from '../../src/connections/settings-b-fleet';
import { sidebarWrites } from '../../src/client/client-ops-sidebar';
import { projectsPage } from '../../src/settings/projects-view';
import { settingsGroups } from '../../src/settings/projects-fleet';
import { resolveScope } from '../../src/settings/settings-core';
import { scopeNoticeGroups } from '../../src/settings/settings-scope-notice';
import { scopeEnvironments, scopeProjectGroups } from '../../src/settings/settings-scope-sources';
import { toasts } from '../../src/shell/toast';

afterEach(() => fleet.entries.clear());
const at = '2026-11-01T00:00:00.000Z';

function rig(caps: Obj = { projectOrganization: true }, fail = '') {
  const writes: Obj[] = [];
  const client = new T3Client();
  Object.assign(client, { connection: 'connected', environmentId: 'env-a', loaded: true, configLive: true, shellLive: true, presentation: {} });
  client.config = { environment: { label: 'This Mac', capabilities: caps }, settings: {}, providers: [] };
  client.scopes = ['orchestration:read', 'orchestration:operate'];
  client.shell = { projects: [{ id: 'alpha', title: 'alpha', workspaceRoot: '/alpha', createdAt: at, updatedAt: at },
    { id: 'beta', title: 'beta', workspaceRoot: '/beta', createdAt: at, updatedAt: at, pinnedAt: at, archivedAt: at }], threads: [], sequence: 1 };
  Object.assign(client, { restAccess: () => ({ ids: async (count: number) => Array.from({ length: count }, (_, index) => `c${index}`),
    request: async (method: string, payload: Obj) => { if (fail) throw new Error(fail); writes.push({ method, ...payload }); return {}; } }) });
  const native = { available: true, watch() {}, later: async (request: unknown) => ({ ok: true, generation: Number(obj(request).generation) || 2, value: {} }) } as unknown as Native;
  const key = (title: string) => settingsGroups(client).find(group => group.name === title)!.key;
  const run = (id: string, value: string) => sidebarWrites.call(client, 'organize-group', id, value, 0, native, {} as Files, {} as OpOut);
  return { client, native, writes, key, run };
}

describe('Settings › Projects: Pin and Archive', () => {
  test('the rows read the whole group, and only where its server keeps them', () => {
    const { client, key } = rig();
    expect(projectsPage(client, key('alpha'), '', true)).toMatchObject({ organize: true, pinned: false, archived: false });
    expect(projectsPage(client, key('beta'), '', true)).toMatchObject({ organize: true, pinned: true, archived: true });
    const old = rig({});
    expect(projectsPage(old.client, old.key('alpha'), '', true).organize).toBe(false);
  });
  test('the switch and the button write project.update on every member', async () => {
    const { writes, key, run } = rig();
    expect(await run(key('alpha'), 'pinned:true')).toBe(true);
    expect(await run(key('beta'), 'archived:false')).toBe(true);
    expect(writes).toEqual([{ method: 'projects.mutate', type: 'project.update', commandId: 'c0', projectId: 'alpha', pinned: true },
      { method: 'projects.mutate', type: 'project.update', commandId: 'c0', projectId: 'beta', archived: false }]);
  });
  test("a refusal is a toast with the reference's title", async () => {
    for (const [value, title] of [['pinned:true', 'Failed to pin project'], ['pinned:false', 'Failed to unpin project'], ['archived:true', 'Failed to archive project'], ['archived:false', 'Failed to unarchive project']]) {
      const { client, key, run } = rig({ projectOrganization: true }, 'Server refused');
      await run(key('alpha'), value!);
      expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title, description: 'Server refused' });
    }
  });
  test('an archived project reads "Archived" in the scope menu and the project notice', () => {
    const { client } = rig();
    expect(resolveScope(client, '', '', '').projectChoices.map(choice => [choice.label, choice.secondary])).toEqual([['All projects', ''], ['alpha', ''], ['beta', 'Archived']]);
    const notice = scopeNoticeGroups('project', scopeProjectGroups(client), scopeEnvironments(client));
    expect(notice[0]!.choices.map(choice => [choice.label, choice.secondary])).toEqual([['alpha', '/alpha'], ['beta', 'Archived · /beta']]);
  });
});
