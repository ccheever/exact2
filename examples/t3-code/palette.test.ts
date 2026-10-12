import { describe, test, expect } from 'bun:test';
import { T3Client } from './client';
import { obj, str, type Obj } from './domain';
import type { Files, Native } from './protocol';
import { commandView, filterGroups, flatten, row, relativeLabel, shortcutText, commandShortcut, contentParts, type Group } from './palette';
import { addProjectView, browseParent, cloneDirectoryName, cloneDestination, normalizeCloneUrl, readiness, resolvePath, appendSegment, browseDirectory, flowOf } from './palette-add';
import { fileRows, lineParts, contentRows, matchIndices, filePickerView, contentSearchView, parseFlags } from './palette-files';
import { paletteCommand } from './palette-commands';
import { paletteView } from './palette-view';
import { keyboardDispatch, visit, favoriteEditor } from './keyboard-dispatch';
import { toasts } from './toast';

const NOW = Date.parse('2026-10-03T12:00:00.000Z');
const binding = (key: string, command: string, extra: Obj = {}): Obj => ({ command, shortcut: { key, modKey: true, metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...extra } });
function client(): T3Client {
  const c = new T3Client();
  Object.assign(c, { connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 1, environmentId: 'env1', origin: 'http://127.0.0.1:14805', scopes: ['orchestration:read', 'orchestration:operate'] });
  c.config = { environment: { label: 'Local', capabilities: { serverResolvedCommandContext: true, threadPullRequests: true } }, newProjectsRoot: '/home/me/projects', availableEditors: ['zed', 'cursor'],
    keybindings: [binding('k', 'commandPalette.toggle'), binding('p', 'filePicker.toggle'), binding('f', 'projectSearch.toggle', { shiftKey: true }), binding('n', 'chat.new'), binding('o', 'chat.new', { shiftKey: true }),
      binding('a', 'theme.select', { altKey: true }), binding('a', 'appearance.cycle', { altKey: true, shiftKey: true }), binding('t', 'themeEditor.toggle', { altKey: true, shiftKey: true }),
      binding('u', 'usage.open'), binding('c', 'thread.copyReference', { shiftKey: true }), binding('[', 'navigation.back'), binding(']', 'navigation.forward'), binding('s', 'thread.settle', { shiftKey: true }),
      binding('1', 'thread.jump.1'), binding('2', 'thread.jump.2')], settings: {} };
  c.shell = { sequence: 1, projects: [{ id: 'p1', title: 'Parity fixture', workspaceRoot: '/repo' }, { id: 'p2', title: 'Second', workspaceRoot: '/second' }],
    threads: [{ id: 't1', projectId: 'p1', title: 'fixture complete', updatedAt: '2026-10-03T01:00:00.000Z', modelSelection: { instanceId: 'codex' } },
      { id: 't2', projectId: 'p1', title: 'fixture complete fork', updatedAt: '2026-10-03T02:00:00.000Z', branch: 'feature/x' },
      { id: 't3', projectId: 'p2', title: 'archived', updatedAt: '2026-10-03T03:00:00.000Z', archivedAt: '2026-10-03T04:00:00.000Z' }] };
  c.projectId = 'p1'; c.threadId = 't2';
  return c;
}
class Fake implements Native {
  available = true; calls: Obj[] = []; replies: Record<string, unknown> = {};
  watch() {}
  async later(input: unknown): Promise<unknown> {
    const request = obj(input); this.calls.push(request);
    const key = request.op === 'request' ? str(request.method) : str(request.op);
    if (key === 'ids') return { ok: true, generation: 1, value: Array.from({ length: Number(request.count) }, (_, i) => `id-${i}`) };
    if (key in this.replies) { const value = this.replies[key]; return value instanceof Error ? { ok: false, generation: 1, error: { kind: 'x', message: value.message, uncertain: false } } : { ok: true, generation: 1, value }; }
    return { ok: true, generation: 1, value: {} };
  }
}
const files = (): Files => ({ fs: { async mkdir() {}, async readFile() { throw new Error('none'); }, async atomicWriteFile() {} } });

describe('command palette root', () => {
  test('actions follow the reference order with shortcut labels, then recent threads', () => {
    const view = commandView(client(), { page: '', query: '', now: NOW, scheme: 'dark', matches: new Map(), matchQuery: '', searching: false });
    const titles = view.rows.map(entry => entry.title);
    expect(titles.slice(0, 8)).toEqual(['New thread in Parity fixture', 'New thread in...', 'Copy thread ID', 'Link pull request to thread', 'Show linked pull requests', 'Restart agent session', 'Go to file', 'Search project contents']);
    expect(titles).toContain('Open settings');
    expect(view.rows[0]!.header).toBe('Actions');
    expect(view.rows[0]!.shortcut).toBe('⇧⌘O');
    expect(view.rows[0]!.bold).toBe('Parity fixture');
    expect(view.rows.find(entry => entry.title === 'Show linked pull requests')!.kind).toBe('disabled');
    expect(view.rows.find(entry => entry.title === 'Change theme')!.shortcut).toBe('⌥⌘A');
    expect(view.rows.find(entry => entry.title === 'Change appearance')!.shortcut).toBe('⌥⇧⌘A');
    const recent = view.rows.filter(entry => entry.op === 'thread');
    expect(recent.map(entry => entry.title)).toEqual(['fixture complete fork', 'fixture complete']);
    expect(recent[0]!.header).toBe('Recent Threads');
    expect(recent[0]!.current).toBe(true);
    expect(recent[0]!.timestamp).toBe('10h ago');
    expect(recent[0]!.branch).toBe('feature/x');
    expect(view.placeholder).toBe('Search commands, projects, and threads...');
    expect(view.enterLabel).toBe('Select');
  });
  test('rows carry their painted geometry for keyboard scrolling', () => {
    const view = commandView(client(), { page: '', query: '', now: NOW, scheme: 'dark', matches: new Map(), matchQuery: '', searching: false });
    // The served f90b77d reference (list top 133): label 141, rows at 169, 201, 233 once laid out
    // (its very first open draws them 1px tighter until the first keypress re-measures).
    expect(view.rows[0]!.top).toBe(8 + 28);
    expect(view.rows[0]!.height).toBe(32);
    expect(view.rows[1]!.top).toBe(8 + 28 + 32);
    const copy = view.rows.find(entry => entry.title === 'Copy thread ID')!;
    expect(copy.height).toBe(48);
    expect(copy.top).toBe(8 + 28 + 64);
    const recent = view.rows.find(entry => entry.op === 'thread')!;
    expect(recent.height).toBe(50);
  });
  test('a query searches actions, projects, settings and threads with message matches', () => {
    const c = client();
    const view = commandView(c, { page: '', query: 'fixture', now: NOW, scheme: 'dark', matches: new Map([['t1', { source: 'user', snippet: 'please run fixture complete' }]]), matchQuery: 'fixture', searching: false });
    const headers = view.rows.filter(entry => entry.header).map(entry => entry.header);
    expect(headers).toContain('Projects');
    expect(headers).toContain('Threads');
    const project = view.rows.find(entry => entry.op === 'open-project')!;
    expect(project.title).toBe('Parity fixture');
    expect(project.description).toBe('/repo');
    const thread = view.rows.find(entry => entry.arg === 't1' && entry.op === 'thread')!;
    expect(thread.matchLabel).toBe('You:');
    expect(thread.matchParts.filter(part => part.hit).map(part => part.text)).toEqual(['fixture']);
    const none = commandView(c, { page: '', query: 'zzzz', now: NOW, scheme: 'dark', matches: new Map(), matchQuery: '', searching: false });
    expect(none.rows).toEqual([]);
    expect(none.empty).toBe('No matching commands, projects, or threads.');
    const settings = commandView(c, { page: '', query: 'worktree branch naming', now: NOW, scheme: 'dark', matches: new Map(), matchQuery: '', searching: false });
    expect(settings.rows.some(entry => entry.description.startsWith('Settings · '))).toBe(true);
  });
  test('submenus: New thread in..., Change theme and Change appearance', () => {
    const c = client();
    const projects = commandView(c, { page: 'new-thread-in', query: '', now: NOW, scheme: 'dark', matches: new Map(), matchQuery: '', searching: false });
    expect(projects.rows.map(entry => entry.title)).toEqual(['Parity fixture', 'Second']);
    expect(projects.rows[0]!.header).toBe('Projects');
    expect(projects.rows.map(entry => entry.shortcut)).toEqual(['⌘1', '⌘2']); // thread.jump.N holds isDesktop: the desktop reference numbers them (SH-3).
    expect(projects.back).toBe(true);
    expect(projects.placeholder).toBe('Search...');
    const themes = commandView(c, { page: 'theme', query: '', now: NOW, scheme: 'dark', matches: new Map(), matchQuery: '', searching: false });
    expect(themes.rows.map(entry => entry.title)).toEqual(['T3 Code', 'T3 Chat', 'Grove', 'Ocean', 'Ember', 'Iris']);
    expect(themes.rows[0]!.trailing).toBe('Current');
    expect(themes.rows[0]!.orbs.length).toBe(2);
    const modes = commandView(c, { page: 'appearance', query: '', now: NOW, scheme: 'dark', matches: new Map(), matchQuery: '', searching: false });
    expect(modes.rows.map(entry => [entry.title, entry.trailing])).toEqual([['System', 'Current'], ['Light', ''], ['Dark', '']]);
  });
  test('ranking keeps exact and prefix matches first and secondary items last', () => {
    const group: Group = { value: 'actions', label: 'Actions', items: [
      { terms: ['open usage'], row: row({ key: 'a', title: 'Open usage' }) },
      { terms: ['usage'], row: row({ key: 'b', title: 'Usage' }) },
      { terms: ['usage x'], row: row({ key: 'c', title: 'Usage x' }), secondary: true }] };
    expect(flatten(filterGroups([group], 'usage', false, { projects: [], settings: [], threads: [] })).map(entry => entry.title)).toEqual(['Usage', 'Open usage', 'Usage x']);
    expect(filterGroups([group], '>usage', false, { projects: [], settings: [], threads: [] })[0]!.items.length).toBe(3);
  });
  test('labels', () => {
    expect(shortcutText({ key: 'k', modKey: true })).toBe('⌘K');
    expect(shortcutText({ key: 'a', modKey: true, altKey: true, shiftKey: true })).toBe('⌥⇧⌘A');
    expect(relativeLabel('2026-10-03T11:59:30.000Z', NOW)).toBe('just now');
    expect(relativeLabel('2026-10-01T11:00:00.000Z', NOW)).toBe('2d ago');
    expect(commandShortcut(client(), 'usage.open')).toBe('⌘U');
    expect(contentParts('A fixture and Fixture', 'fixture').filter(part => part.hit).length).toBe(2);
  });
});

describe('add project', () => {
  test('path helpers mirror client-runtime projects.ts', () => {
    expect(browseParent('/Users/me/projects/')).toBe('/Users/me/');
    expect(browseParent('/Users/')).toBe('/');
    expect(browseParent('/')).toBe(null);
    expect(browseDirectory('~/pro')).toBe('~/');
    expect(appendSegment('~/pro', 'projects')).toBe('~/projects/');
    expect(resolvePath('./sub/../app', '/repo')).toBe('/repo/app');
    expect(cloneDirectoryName('git@github.com:owner/repo.git')).toBe('repo');
    expect(cloneDirectoryName('https://host')).toBe('');
    expect(cloneDestination('~/', 'repo')).toBe('~/repo');
    expect(normalizeCloneUrl('owner/repo')).toBe('https://github.com/owner/repo.git');
  });
  test('a failed source-control discovery is asked again, not kept as "Setup Required"', async () => {
    const c = client(), native = new Fake();
    native.replies['server.discoverSourceControl'] = new Error('the answer was let go');
    const failed = await addProjectView(c, native, { page: 'add-project', query: '', highlighted: false });
    expect(failed.rows.find(entry => entry.title === 'GitHub repository')!.badge).toBe('Setup Required');
    native.replies['server.discoverSourceControl'] = { sourceControlProviders: [{ kind: 'github', label: 'GitHub', status: 'available', auth: { status: 'authenticated' } }] };
    // Bounded: a read right after the failure does not ask again; one after the client's state moves does.
    const soon = await addProjectView(c, native, { page: 'add-project', query: '', highlighted: false });
    expect(soon.rows.find(entry => entry.title === 'GitHub repository')!.badge).toBe('Setup Required');
    c.revision++;
    const view = await addProjectView(c, native, { page: 'add-project', query: '', highlighted: false });
    expect(view.rows[3]!.title).toBe('GitHub repository');
    expect(view.rows[3]!.badge).toBe('');
  });
  test('sources: new project, local folder, Git URL, then ready providers before Setup Required', async () => {
    const c = client(), native = new Fake();
    native.replies['server.discoverSourceControl'] = { sourceControlProviders: [{ kind: 'github', label: 'GitHub', status: 'available', auth: { status: 'authenticated' } }, { kind: 'gitlab', label: 'GitLab', status: 'missing', installHint: 'Install glab' }] };
    const view = await addProjectView(c, native, { page: 'add-project', query: '', highlighted: false });
    expect(view.rows.map(entry => entry.title)).toEqual(['New project', 'Local folder', 'Git URL', 'GitHub repository', 'Azure DevOps repository', 'Bitbucket repository', 'Forgejo / Gitea repository', 'GitLab repository']);
    expect(view.rows[0]!.header).toBe('Sources');
    expect(view.rows[1]!.description).toBe('Browse a folder on disk');
    expect(view.rows[4]!.badge).toBe('Setup Required');
    expect(view.rows[4]!.index).toBe(-1);
    expect(view.count).toBe(4);
    expect(view.backHint).toBe(true);
    expect(readiness(null).url!.ready).toBe(true);
  });
  test('browsing lists directories and submits the resolved path', async () => {
    const c = client(), native = new Fake();
    native.replies['filesystem.browse'] = { parentPath: '/home/me/', entries: [{ name: 'projects', fullPath: '/home/me/projects' }, { name: '.hidden', fullPath: '/home/me/.hidden' }, { name: 'photos', fullPath: '/home/me/photos' }] };
    const view = await addProjectView(c, native, { page: '', query: '~/pro', highlighted: false });
    expect(view.rows.map(entry => entry.title)).toEqual(['projects']);
    expect(view.rows[0]!.header).toBe('Directories');
    expect(view.rows[0]!.arg).toBe('~/projects/');
    expect(view.accessory).toBe('Create & Add');
    expect(view.enterOp).toBe('flow');
    expect(view.enterArg).toBe('add-path');
    expect(view.placeholder).toBe('Enter project path (e.g. ~/projects/my-app)');
    const exact = await addProjectView(c, native, { page: 'add-project/browse', query: '~/projects', highlighted: false });
    expect(exact.accessory).toBe('Add');
    expect(exact.enterArg2).toBe('/home/me/projects');
    expect(exact.popOnEmpty).toBe(true);
  });
  test('the Git URL step continues to a destination', async () => {
    const c = client(), native = new Fake();
    const step = await addProjectView(c, native, { page: 'add-project/clone:url', query: '', highlighted: false });
    expect(step.placeholder).toBe('Enter Git clone URL');
    expect(step.accessory).toBe('Continue');
    expect(step.accessoryEnabled).toBe(false);
    expect(step.empty).toBe('Enter a Git clone URL and press Enter to continue.');
    const result = await paletteCommand(c, native, files(), 'clone-repository:url', 'https://example.com/acme/tool.git', 'add-project/clone:url');
    expect(result.page).toBe('add-project/clone:url/confirm');
    expect(result.query).toBe('~/tool');
    expect(flowOf(c).clone!.remoteUrl).toBe('https://example.com/acme/tool.git');
    native.replies['filesystem.browse'] = { parentPath: '/home/me/', entries: [] };
    const confirm = await addProjectView(c, native, { page: 'add-project/clone:url/confirm', query: '~/tool', highlighted: false });
    expect(confirm.contextLabel).toBe('Repository');
    expect(confirm.accessory).toBe('Create & Clone');
  });
  test('the New project step previews the folder and offers GitHub only when ready', async () => {
    const c = client(), native = new Fake();
    const view = await addProjectView(c, native, { page: 'new-project', query: 'My App', highlighted: false });
    expect(view.placeholder).toBe('Project name');
    expect(view.contextDescription).toBe('Creates /home/me/projects/my-app');
    expect(view.rows.map(entry => entry.title)).toEqual(['Add existing project']);
    expect(view.enterArg).toBe('new-project');
    expect(view.enterLabel).toBe('Create');
  });
  test('adding a path creates the project and opens a new thread in it', async () => {
    const c = client(), native = new Fake();
    native.replies['http'] = { snapshotSequence: 2, projects: [...c.shell.projects, { id: 'id-1', title: 'app', workspaceRoot: '/home/me/app' }], threads: [] };
    const result = await paletteCommand(c, native, files(), 'add-path', '/home/me/app', '');
    expect(toasts(c)).toEqual([]);
    expect(result.close).toBe(true);
    expect(result.project).toBe('id-1');
    const write = native.calls.find(call => call.method === 'projects.mutate')!;
    expect(obj(write.payload)).toMatchObject({ type: 'project.create', workspaceRoot: '/home/me/app', title: 'app', createWorkspaceRootIfMissing: true });
  });
  test('an existing project opens its latest thread', async () => {
    const c = client();
    const result = await paletteCommand(c, new Fake(), files(), 'add-path', '/repo/', '');
    expect(result.thread).toBe('t2');
  });
  test('failures toast with the reference titles', async () => {
    const c = client(), native = new Fake();
    native.replies['projects.mutate'] = new Error('Folder is not readable');
    const result = await paletteCommand(c, native, files(), 'add-path', '/nope', '');
    expect(result.close).toBe(false);
    expect(toasts(c).slice(-1)[0]).toMatchObject({ kind: 'error', title: 'Failed to add project', description: 'Folder is not readable' });
    native.replies['sourceControl.lookupRepository'] = new Error('Not found');
    await paletteCommand(c, native, files(), 'clone-repository:github', 'acme/none', 'add-project/clone:github');
    expect(toasts(c).slice(-1)[0]!.title).toBe('Repository lookup failed');
  });
  test('restart agent session detaches provider sessions and refreshes providers', async () => {
    const c = client(), native = new Fake();
    (c as unknown as { thread: unknown }).thread = { projection: { providerSessions: [{ id: 's1' }] } };
    const result = await paletteCommand(c, native, files(), 'restart-session', 't2', '');
    expect(result.close).toBe(true);
    const detach = native.calls.find(call => call.method === 'orchestration.dispatchCommand')!;
    expect(obj(detach.payload)).toMatchObject({ type: 'provider-session.detach', providerSessionId: 's1', threadId: 't2', reason: 'client-requested' });
    expect(native.calls.find(call => call.method === 'server.refreshProviders')!.payload).toMatchObject({ cwd: '/repo', fresh: true });
    expect(toasts(c).slice(-1)[0]).toMatchObject({ kind: 'success', title: 'Agent session will restart', description: 'Your next message starts a fresh session.' });
  });
});

describe('file picker and content search', () => {
  test('fuzzy highlights per matched letter', () => {
    expect(matchIndices('fixture-result.md', 'fxr')).toEqual([0, 2, 5]);
    const [entry] = fileRows([{ path: 'docs/fixture-result.md', kind: 'file' }, { path: 'docs', kind: 'directory' }], 'fix');
    expect(entry!.title).toBe('fixture-result.md');
    expect(entry!.titleParts.filter(part => part.hit).map(part => part.text).join('')).toBe('fix');
    expect(entry!.description).toBe('docs/fixture-result.md');
  });
  test('the picker searches the active workspace', async () => {
    const c = client(), native = new Fake();
    native.replies['projects.searchEntries'] = { entries: [{ path: 'fixture-result.md', kind: 'file' }], truncated: false };
    const view = await filePickerView(c, native, '');
    expect(view.rows[0]!.header).toBe('Parity fixture');
    expect(view.enterLabel).toBe('Open file');
    expect(view.escapeLabel).toBe('Back');
    expect(native.calls.find(call => call.method === 'projects.searchEntries')!.payload).toMatchObject({ cwd: '/repo', query: '', limit: 200 });
  });
  test('content results group by file with line numbers and marks', async () => {
    expect(lineParts('# Exact T3 fixture', [{ start: 11, end: 18 }]).map(part => [part.text, part.hit])).toEqual([['# Exact T3 ', false], ['fixture', true]]);
    expect(lineParts('# Exact T3 fixture', [{ start: 11, end: 18 }], 'fixture-result.md').map(part => [part.text, part.hit, part.cls])).toEqual([['# Exact T3 ', false, 'tag'], ['fixture', true, 'tag']]); // lane r12-render: Shiki's heading ink
    expect(fileRows([{ kind: 'file', path: 'docs/a.md' }, { kind: 'file', path: 'src/b.ts' }], '').map(entry => entry.icon)).toEqual(['file-md', 'file']);
    const rows = contentRows([{ path: 'fixture-result.md', lineNumber: 1, lineContent: '# Exact T3 fixture', matchRanges: [{ start: 11, end: 18 }] }]);
    expect(rows.map(entry => entry.kind)).toEqual(['file-header', 'line']);
    expect(rows[0]!.badge).toBe('1');
    const c = client(), native = new Fake();
    native.replies['projects.searchContents'] = { matches: [{ path: 'fixture-result.md', lineNumber: 1, lineContent: '# Exact T3 fixture', matchRanges: [{ start: 11, end: 18 }] }], truncated: false };
    const empty = await contentSearchView(c, native, '', '');
    expect(empty.empty).toBe('Type to search across your project.');
    expect(empty.placeholder).toBe('Search in Parity fixture');
    const view = await contentSearchView(c, native, 'fixture', 'case regex');
    expect(view.summary).toBe('1 results in 1 files');
    expect(native.calls.find(call => call.method === 'projects.searchContents')!.payload).toMatchObject({ caseSensitive: true, wholeWord: false, useRegex: true, limit: 500 });
    expect(parseFlags('word').wholeWord).toBe(true);
  });
  test('the resource routes modes and closes', async () => {
    const c = client(), native = new Fake();
    expect((await paletteView(c, native, [false])).open).toBe(false);
    expect((await paletteView(c, native, [true, 'files', '', '', '', false, NOW, 'dark'])).label).toBe('File picker');
    expect((await paletteView(c, native, [true, 'command', 'add-project', '', '', false, NOW, 'dark'])).rows[0]!.header).toBe('Sources');
  });
});

describe('keyboard dispatch', () => {
  const context = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false };
  test('palette, settings and thread commands', () => {
    const c = client();
    const items = keyboardDispatch(c, [], '', '', context);
    const find = (command: string) => items.find(item => item.command === command);
    expect(find('commandPalette.toggle')).toMatchObject({ kind: 'palette', target: 'command', chord: 'Meta+k' });
    expect(find('filePicker.toggle')!.target).toBe('files');
    expect(find('projectSearch.toggle')!.chord).toBe('Meta+Shift+f');
    expect(find('theme.select')).toMatchObject({ kind: 'palette', target: 'command', extra: 'theme' });
    expect(find('usage.open')).toMatchObject({ kind: 'palette-run', target: 'usage', chord: 'Meta+u' });
    expect(find('themeEditor.toggle')).toMatchObject({ kind: 'palette-run', target: 'theme-editor', chord: 'Meta+Alt+Shift+t' });
    expect(find('settings.open')).toMatchObject({ chord: 'Meta+,', kind: 'settings' });
    expect(find('thread.settle')).toMatchObject({ kind: 'command', target: 'chat:settle', extra: 't2' });
    const open = keyboardDispatch(c, [], '', '', { ...context, modalOpen: true, paletteOpen: true });
    expect(open.map(item => item.command)).toEqual(['commandPalette.toggle', 'filePicker.toggle', 'projectSearch.toggle']);
  });
  test('back and forward walk the visited threads', () => {
    const c = client();
    c.threadId = 't1'; visit(c);
    c.threadId = 't2';
    let items = keyboardDispatch(c, [], '', '', context);
    expect(items.find(item => item.command === 'navigation.back')).toMatchObject({ kind: 'thread', target: 't1' });
    c.threadId = 't1';
    items = keyboardDispatch(c, [], '', '', context);
    expect(items.find(item => item.command === 'navigation.forward')).toMatchObject({ kind: 'thread', target: 't2' });
    expect(items.find(item => item.command === 'navigation.back')).toBeUndefined();
  });
  test('the favourite editor follows the reference EDITORS order', () => {
    expect(favoriteEditor({ availableEditors: ['zed', 'cursor'] })).toBe('cursor');
    expect(favoriteEditor({})).toBe('');
  });
});
