// Pin favorite projects from the draft hero's project menu (T3 Code 2359643cff, MIT, see LICENSE-T3:
// chat/DraftHeroHeadline.tsx, ProjectPinButton.tsx, hooks/useProjectGroupUpdate.ts and hooks/useOrderWhileOpen.ts).
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { pagesHome } from '../../src/pages/pages-home';
import { pagesLocal } from '../../src/pages/pages-commands';
import { contractComponent, contractFns } from '../support/contract-action-fixture';

const at = '2026-11-01T00:00:00.000Z';

function rig(caps: Obj = { projectOrganization: true }, scopes = ['orchestration:read', 'orchestration:operate']) {
  const writes: Obj[] = [];
  const client = new T3Client();
  Object.assign(client, { connection: 'connected', environmentId: 'env-a', loaded: true, configLive: true, shellLive: true, presentation: {}, projectId: 'alpha' });
  client.config = { environment: { label: 'This Mac', capabilities: caps }, settings: {}, providers: [], keybindings: [{ command: 'thread.pin', shortcut: { key: 'p', modKey: true, shiftKey: true } }] };
  client.scopes = scopes;
  client.shell = { projects: [{ id: 'alpha', title: 'alpha', workspaceRoot: '/alpha', createdAt: at, updatedAt: at }, { id: 'beta', title: 'beta', workspaceRoot: '/beta', createdAt: at, updatedAt: at, pinnedAt: at }], threads: [], sequence: 1 };
  Object.assign(client, { restAccess: () => ({ ids: async (count: number) => Array.from({ length: count }, (_, index) => `c${index}`),
    request: async (method: string, payload: Obj) => { writes.push({ method, ...payload }); return {}; } }) });
  const native = { available: true, watch() {} } as unknown as Native;
  return { client, native, writes };
}

describe('the draft hero\'s project menu offers a pin', () => {
  test('pinned projects come first and carry their pin; the title names the shortcut', async () => {
    const { client } = rig();
    const home = await pagesHome(client, null, false);
    expect(home.projects.map(project => [project.name, project.canPin, project.pinned, project.pinTitle])).toEqual([
      ['beta', true, true, 'Unpin beta (⇧⌘P)'], ['alpha', true, false, 'Pin alpha (⇧⌘P)']]);
    expect(home.pinChord).toBe('Meta+Shift+p');
  });
  test('not where the server drops pins or this session may not operate', async () => {
    expect((await pagesHome(rig({}).client, null, false)).projects.some(project => project.canPin)).toBe(false);
    expect((await pagesHome(rig(undefined, ['orchestration:read']).client, null, false)).projects.some(project => project.canPin)).toBe(false);
  });
  test('a row\'s pin writes project.update and toggles', async () => {
    const { client, native, writes } = rig();
    await pagesLocal(client, native, {} as Files, 'hero-pin', 'alpha', '');
    await pagesLocal(client, native, {} as Files, 'hero-pin', 'beta', '');
    expect(writes).toEqual([
      { method: 'projects.mutate', type: 'project.update', commandId: 'c0', projectId: 'alpha', pinned: true },
      { method: 'projects.mutate', type: 'project.update', commandId: 'c0', projectId: 'beta', pinned: false }]);
    const refused = rig({});
    await pagesLocal(refused.client, refused.native, {} as Files, 'hero-pin', 'alpha', '');
    expect(refused.writes).toEqual([]);
  });
});

describe('the menu (HeroPicker)', () => {
  const picker = contractComponent('pages-hero.contract', 'HeroPicker');
  const row = (id: string, pinned: boolean, canPin = true) => ({ id, name: id, selected: id === 'b', canPin, pinned, pinTitle: '' });
  const home = (projects: Obj[]) => ({ projects, pinChord: 'Meta+Shift+p' });
  // DraftHeroHeadline's ComboboxSearchInput onKeyDown: the search field, which holds the focus, takes the shortcut.
  test('the thread pin shortcut pins the highlighted row, and nothing else does', () => {
    const press = (cursor: string, key: string, e: Obj, canPin = true) => {
      const calls: unknown[][] = [];
      picker.call('searchKey', { open: true, query: '', cursor, held: ['a', 'b'], pinOver: '', over: false },
        { ...contractFns('menu-keys.contract'), home: home([row('a', true, canPin), row('b', false, canPin)]), command: (...args: unknown[]) => calls.push(['command', ...args]),
          preventDefault: () => calls.push(['preventDefault']), stopPropagation: () => calls.push(['stopPropagation']) },
        [key, { metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, code: '', ...e }]);
      return calls;
    };
    expect(press('b', 'P', { metaKey: true, shiftKey: true })).toEqual([['preventDefault'], ['stopPropagation'], ['command', 'pageslocal:hero-pin', 'b', '', 0]]);
    expect(press('b', 'ㅔ', { metaKey: true, shiftKey: true, code: 'KeyP' })).toEqual([['preventDefault'], ['stopPropagation'], ['command', 'pageslocal:hero-pin', 'b', '', 0]]);
    expect(press('b', 'p', { metaKey: true })).toEqual([]);
    expect(press('-', 'P', { metaKey: true, shiftKey: true })).toEqual([]);
    expect(press('add-project', 'P', { metaKey: true, shiftKey: true })).toEqual([]);
    expect(press('b', 'P', { metaKey: true, shiftKey: true }, false)).toEqual([]);
  });
  test('while open the rows keep the order they opened in; new rows join at the end; closed, the server\'s order', () => {
    const state = { open: false, query: '', cursor: '', held: [] as string[], pinOver: '', over: false };
    const scope = { home: home([row('a', true), row('b', false)]) };
    picker.call('toggled', state, scope, [{ newState: 'open' }]);
    expect(state.held).toEqual(['a', 'b']);
    // Base UI highlights the chosen project as the popup opens, so its pin shows and the shortcut pins it.
    expect([state.cursor, picker.derives(state, { size: 30, ...scope }).highlight]).toEqual(['b', 'b']);
    // b is pinned: the server now lists it first, and a new project arrives.
    const after = { size: 30, home: home([row('b', true), row('a', true), row('c', false)]) };
    expect((picker.derives(state, after).matches as Obj[]).map(project => project.id)).toEqual(['a', 'b', 'c']);
    // The arrived row joins the held order, so a later pin does not move it either (useOrderWhileOpen withArrivedKeys).
    expect(picker.derives(state, after).arrived).toEqual(['c']);
    picker.call('hoverRow', state, after, ['c', true]);
    expect(state.held).toEqual(['a', 'b', 'c']);
    const pinnedC = { size: 30, home: home([row('c', true), row('b', true), row('a', true)]) };
    expect((picker.derives(state, pinnedC).matches as Obj[]).map(project => project.id)).toEqual(['a', 'b', 'c']);
    state.pinOver = 'c';
    picker.call('toggled', state, after, [{ newState: 'closed' }]);
    expect(state.pinOver).toBe('');
    expect((picker.derives(state, after).matches as Obj[]).map(project => project.id)).toEqual(['b', 'a', 'c']);
  });
});
