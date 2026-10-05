import { test, expect } from 'bun:test';
import { filterIconNames, iconLabel, firstEmoji, monogramOf, monogramWidth, projectIdentity, decodeIcon, encodeIcon, iconPicker, runIconOp, iconFields, ICON_COLORS, POPULAR_ICONS } from './settings-b-icons';
import { projectIdentity as presentationIdentity } from './presentation';
import { LUCIDE_PATHS } from './settings-b-lucide';
import type { Obj } from './domain';
import type { Native, Files } from './protocol';
import type { T3Client } from './client';

test('the icon grid is the popular set, else every Lucide name containing the query (60 at most)', () => {
  expect(filterIconNames('')).toEqual(POPULAR_ICONS);
  expect(POPULAR_ICONS.every(name => LUCIDE_PATHS[name])).toBe(true);
  expect(Object.keys(LUCIDE_PATHS).length).toBeGreaterThan(1500);
  expect(filterIconNames('  Git Branch ')).toEqual(expect.arrayContaining(['git-branch', 'git-branch-plus']));
  expect(filterIconNames('a').length).toBe(60);
  expect(filterIconNames('zzzz-none')).toEqual([]);
  expect(iconLabel('flask-conical')).toBe('Flask Conical');
  expect(iconLabel('code-2')).toBe('Code 2');
});

test('pasted emoji keep their first grapheme; text is not an emoji', () => {
  expect(firstEmoji('  🚀 launch')).toBe('🚀');
  expect(firstEmoji('🛠️tools')).toBe('🛠️');
  expect(firstEmoji('👩🏽‍💻')).toBe('👩🏽‍💻');
  expect(firstEmoji('🇰🇷')).toBe('🇰🇷');
  expect(firstEmoji('1️⃣')).toBe('1️⃣');
  expect(firstEmoji('abc')).toBe('');
  expect(firstEmoji('   ')).toBe('');
});

test('monograms follow ProjectMonogramText and project identity matches the sidebar', () => {
  expect(monogramOf(' ap ')).toEqual({ text: 'AP', valid: true });
  expect(monogramOf('!!').valid).toBe(false);
  expect(monogramOf('').valid).toBe(false);
  expect(monogramOf('ｗ').text).toBe('W');
  expect(monogramWidth('A')).toBe(6);
  expect(monogramWidth('AB')).toBe(12);
  for (const name of ['actions-project', 'Parity fixture', 'exact2', 'T3 Code', '']) {
    const identity = projectIdentity(name), shown = presentationIdentity(name);
    expect(identity.monogram).toBe(shown.projectMark);
    expect(ICON_COLORS.find(color => color.value === identity.color)!.ink).toBe(shown.projectInk);
  }
});

test('overrides decode a lucide monogram and encode monograms for older peers', () => {
  expect(decodeIcon({ kind: 'lucide', name: 'folder-code', color: 'pink', monogramText: 'AB' })).toEqual({ kind: 'monogram', name: '', color: 'pink', text: 'AB', emoji: '' });
  expect(decodeIcon({ kind: 'lucide', name: 'rocket', color: 'nope' })).toEqual({ kind: 'lucide', name: 'rocket', color: '', text: '', emoji: '' });
  expect(decodeIcon(null).kind).toBe('');
  expect(encodeIcon({ kind: 'monogram', name: '', color: 'teal', text: 'Q', emoji: '' })).toEqual({ kind: 'lucide', name: 'folder-code', color: 'teal', monogramText: 'Q' });
  expect(encodeIcon({ kind: 'emoji', name: '', color: '', text: '', emoji: '🚀' })).toEqual({ kind: 'emoji', emoji: '🚀' });
});

function fakeClient(projects: Obj[], replies: Record<string, (payload: Obj) => Obj>) {
  const writes: Obj[] = [], rpcs: [string, Obj][] = [];
  let shell = { projects, threads: [] as Obj[] };
  const client = {
    connection: 'connected', environmentId: 'env', origin: 'http://127.0.0.1:14809', generation: 3,
    shell: { projects, threads: [], sequence: 0 },
    rpc: async (_native: Native, method: string, payload: Obj) => { rpcs.push([method, payload]); const reply = replies[method]; if (!reply) throw new Error(`no ${method}`); return reply(payload); },
    restAccess: () => ({
      http: async () => ({ snapshotSequence: 1, projects: shell.projects, threads: [] }),
      ids: async () => [`cmd-${writes.length}`],
      write: async (_storage: Files, pending: Obj) => {
        writes.push(pending);
        const payload = pending.payload as Obj;
        shell = { ...shell, projects: shell.projects.map(project => project.id === payload.projectId ? { ...project, faviconPath: payload.faviconPath, projectIcon: payload.projectIcon } : project) };
        return {};
      },
    }),
  };
  return { client: client as unknown as T3Client, writes, rpcs };
}
const native = { available: true, watch() {}, later: async () => ({}) } as unknown as Native;
const storage = {} as Files;

test('the picker seeds from the current override and searches image files', async () => {
  const projects = [{ id: 'p1', title: 'actions-project', workspaceRoot: '/w/actions', projectIcon: { kind: 'lucide', name: 'folder-code', color: 'teal', monogramText: 'XY' } }];
  const { client, rpcs } = fakeClient(projects, { 'projects.searchEntries': payload => ({ entries: payload.query === 'zzz' ? [] : [{ path: 'public/logo.svg', kind: 'file' }, { path: 'public', kind: 'directory' }, { path: 'public/favicon.png', kind: 'file' }] }) });
  const icons = await iconPicker(client, native, 'project-icon', '', 'XY', '🚀 hi', '/w/actions', 'actions-project');
  expect(icons.seeds).toEqual([{ key: 'seed:p1', mode: 'monogram', iconName: 'folder-code', color: 'teal', emoji: '💻', letters: 'XY' }]);
  expect(icons.icons.length).toBe(24);
  expect([icons.monogram, icons.monogramValid, icons.pasted]).toEqual(['XY', true, '🚀']);
  const files = await iconPicker(client, native, 'project-favicon', '', '', '', '/w/actions', 'actions-project');
  expect(files.files.map(file => [file.index, file.name, file.path, file.token])).toEqual([[0, 'logo.svg', 'public/logo.svg', 'svg'], [1, 'favicon.png', 'public/favicon.png', 'image']]);
  expect(rpcs[0]).toEqual(['projects.searchEntries', { cwd: '/w/actions', query: '', limit: 200, imageOnly: true }]);
  expect((await iconPicker(client, native, 'project-favicon', 'zzz', '', '', '/w/actions', 'actions-project')).fileEmpty).toBe('No matching image files.');
  expect((await iconPicker(client, native, 'project-icon', '', '!!', '', '/w/actions', 'x')).monogramValid).toBe(false);
});

test('saving fans the icon out to every member and validates first', async () => {
  const projects = [{ id: 'p1', title: 'a', workspaceRoot: '/w/a' }, { id: 'p2', title: 'a', workspaceRoot: '/w/b' }];
  const { client, writes } = fakeClient(projects, {});
  await runIconOp(client, native, storage, 'project-icon-set', 'p1,p2', 'kind=monogram&text=q&color=teal');
  expect(writes.map(write => (write.payload as Obj).projectId)).toEqual(['p1', 'p2']);
  expect(writes[0]!.payload).toEqual({ type: 'project.update', commandId: 'cmd-0', projectId: 'p1', faviconPath: null, projectIcon: { kind: 'lucide', name: 'folder-code', color: 'teal', monogramText: 'Q' } });
  await runIconOp(client, native, storage, 'project-favicon-set', 'p1', 'path=public%2Ffavicon.png');
  expect(writes[2]!.payload).toEqual({ type: 'project.update', commandId: 'cmd-2', projectId: 'p1', faviconPath: 'public/favicon.png', projectIcon: null });
  await expect(runIconOp(client, native, storage, 'project-icon-set', 'p1', 'kind=lucide&name=not-an-icon&color=teal')).rejects.toThrow('Choose an icon');
  await expect(runIconOp(client, native, storage, 'project-favicon-set', 'p1', 'path=notes.txt')).rejects.toThrow('Choose an image file');
  await expect(runIconOp(client, native, storage, 'project-icon-set', 'p1', 'kind=monogram&text=%21%21&color=teal')).rejects.toThrow('One or two letters');
  await expect(runIconOp(client, native, storage, 'project-icon-set', 'gone', 'kind=emoji&emoji=%F0%9F%9A%80')).rejects.toThrow('membership changed');
  expect(writes.length).toBe(3);
});

test('the settings row shows the override, else the server favicon, else the automatic monogram', async () => {
  const signed = { 'assets.createUrl': () => ({ relativeUrl: '/api/assets/abc/favicon.png', expiresAt: 0 }) };
  const plain = [{ id: 'p1', title: 'actions-project', workspaceRoot: '/w/actions' }];
  let fields = await iconFields(fakeClient(plain, signed).client, native, 'actions-project', plain);
  expect([fields.iconKind, fields.faviconSrc, fields.iconText, fields.iconScope]).toEqual(['', 'http://127.0.0.1:14809/api/assets/abc/favicon.png', 'AP', 'p1']);
  const missing = [{ id: 'p9', title: 'zeta', workspaceRoot: '/w/zeta' }];
  fields = await iconFields(fakeClient(missing, { 'assets.createUrl': () => ({ relativeUrl: '/api/assets/x/project-favicon-missing' }) }).client, native, 'zeta', missing);
  expect(fields.faviconSrc).toBe('');
  const lucide = [{ id: 'p3', title: 'r', workspaceRoot: '/w/r', projectIcon: { kind: 'lucide', name: 'rocket', color: 'rose' } }];
  fields = await iconFields(fakeClient(lucide, {}).client, native, 'r', lucide);
  expect([fields.iconKind, fields.iconD === LUCIDE_PATHS.rocket, fields.iconInk, fields.faviconSrc]).toEqual(['lucide', true, 'light-dark(#ec003f, #ff637e)', '']);
});
