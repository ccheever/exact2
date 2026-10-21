// A missing t3.json is not a failed read (T3 Code 74541bab17, MIT, see LICENSE-T3: projectFilesQueryState.ts
// getProjectFileQueryAtom, SettingsScopeContext.tsx, t3ProjectFileDefaults.ts): t3.json reads send `allowMissing`,
// and an answer with `exists: false` reads as no file, with "'t3.json' does not exist." in the Files preview.
import { expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { ensureFile, filesState } from '../../src/panel/files-surface';
import { scopeMemberFiles } from '../../src/settings/settings-scope-sources';

const native = { available: true } as Native;
const missing = { relativePath: 't3.json', contents: '', byteLength: 0, truncated: false, exists: false };

test('the Files preview reads a missing t3.json as absent, and other files without allowMissing', async () => {
  const calls: Obj[] = [];
  const client = {
    environmentId: 'env', threadId: 't1', projectId: 'p1', ready: true, connection: 'connected', config: {}, local: { clientSettings: {} },
    shell: { projects: [{ id: 'p1', title: 'demo', workspaceRoot: '/repo' }], threads: [{ id: 't1', projectId: 'p1' }] },
    restAccess: () => ({ request: async (method: string, payload: Obj) => {
      calls.push({ method, ...payload });
      if (method === 'projects.readFile') return payload.relativePath === 't3.json' ? missing : { contents: 'a', byteLength: 1, truncated: false };
      return { entries: [] };
    } }),
  } as unknown as T3Client;
  await ensureFile(client, native, 't3.json', false);
  await ensureFile(client, native, 'a.ts', false);
  const reads = calls.filter(call => call.method === 'projects.readFile');
  expect(reads).toEqual([{ method: 'projects.readFile', cwd: '/repo', relativePath: 't3.json', allowMissing: true }, { method: 'projects.readFile', cwd: '/repo', relativePath: 'a.ts' }]);
  expect(filesState(client).reads.get('t3.json')).toMatchObject({ contents: '', error: "'t3.json' does not exist.", notFile: false });
  expect(filesState(client).reads.get('a.ts')).toMatchObject({ contents: 'a', error: '' });
});

test('settings read a missing t3.json as no project file', async () => {
  const sent: Obj[] = [];
  const client = { async settingsCoreRequest(_native: Native, method: string, payload: Obj) { sent.push({ method, ...payload }); return missing; } };
  const files = await scopeMemberFiles(client as never, native, [{ id: 'p', environmentId: 'env', workspaceRoot: '/repo', physicalProjectKey: 'key' }] as never,
    [{ environmentId: 'env', connection: { phase: 'connected' } }] as never, () => ({ parsed: true }));
  expect(sent).toEqual([{ method: 'projects.readFile', cwd: '/repo', relativePath: 't3.json', allowMissing: true }]);
  expect(files.get('key')).toBeNull();
});
