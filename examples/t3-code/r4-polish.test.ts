// Lane r4-polish: relaunch persistence through the preference file, the
// palette's project order, the sidebar width fixed at load, the welcome gate's
// hydration, and the SnapShot shortcut-conflict label.
import { afterEach, beforeEach, describe, test, expect } from 'bun:test';
import { noPrimary, resetPrimary } from './local-primary-fixture';
// These cases are the hosted rules (resolveHostedFirstRunDecision): no embedded server runs on this Mac.
beforeEach(noPrimary);
afterEach(resetPrimary);
import { T3Client } from './client';
import type { Obj } from './domain';
import type { Files, Native } from './protocol';
import { commandView } from './palette';
import { pickerProjects } from './r4-polish-palette-projects';
import { initialSidebarWidth, sidebarLaunchWidth, sidebarWidthStored, sidebarMaximumWidth } from './r4-polish-sidebar-width';
import { sidebarLocal } from './sidebar-commands';
import { welcomeView } from './pages-welcome';
import { pagesPrefs } from './pages-prefs';
import { shellView } from './shell';
import { toasts } from './toast';
import { usagePrefs, usageLocal } from './pages-usage';
import { commandLabel } from './snapshot-shortcut';
import { NIGHTLY_NOTICE } from './shell-nightly';
import { commandShortcut } from './shell';

const NOW = Date.parse('2026-10-04T12:00:00.000Z');
const iso = (offset: number) => new Date(NOW + offset).toISOString();

/** A preference file kept in memory, as the native adapter keeps app:/data/t3-code.json. */
function disk(initial = '') {
  const box = { text: initial, writes: 0 };
  const files: Files = { fs: {
    async mkdir() {},
    async readFile() { if (!box.text) throw new Error('missing'); return new TextEncoder().encode(box.text).buffer; },
    async atomicWriteFile(_path: string, bytes: Uint8Array) { box.text = new TextDecoder().decode(bytes); box.writes++; },
  } } as Files;
  return { box, files };
}
const load = (client: T3Client, files: Files) => (client as unknown as { load(storage: Files): Promise<void> }).load(files);

describe('relaunch persistence (the preference file the reference keeps in localStorage)', () => {
  test('the Nightly notice dismissed by its × stays dismissed after a relaunch', async () => {
    const { files, box } = disk();
    const first = new T3Client();
    await load(first, files);
    await shellView(first, null, files, NOW, '', false);
    const notice = toasts(first).find(toast => toast.title === NIGHTLY_NOTICE.title)!;
    expect(notice).toBeDefined();
    await shellView(first, null, files, NOW + 1000, `x${notice.id}`, false);
    expect(JSON.parse(box.text).shell.nightlyNoticeDismissed).toBe(true);
    const relaunched = new T3Client();
    await load(relaunched, files);
    await shellView(relaunched, null, files, NOW + 2000, '', false);
    expect(toasts(relaunched).some(toast => toast.title === NIGHTLY_NOTICE.title)).toBe(false);
  });
  test('the Usage tab and period, a draft and the sidebar survive a relaunch', async () => {
    const { files } = disk();
    const first = new T3Client();
    await load(first, files);
    await usageLocal(first, 'metric', 'tokens');
    await usageLocal(first, 'window', '7');
    Object.assign(first, { environmentId: 'env1', projectId: 'p1', threadId: 't1' });
    first.local.drafts[first.draftKey] = 'kept draft';
    await first.savePreferences(files);
    const relaunched = new T3Client();
    await load(relaunched, files);
    expect(usagePrefs(relaunched)).toEqual({ metric: 'tokens', windowDays: 7 });
    expect(relaunched.local.selections.env1).toEqual({ projectId: 'p1', threadId: 't1' });
    expect(relaunched.local.drafts['env1:t1']).toBe('kept draft');
  });
});

function paletteClient(order = ''): T3Client {
  const c = new T3Client();
  Object.assign(c, { connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 1, environmentId: 'env1' });
  c.config = { environment: { label: 'Local', capabilities: {} }, settings: {}, keybindings: [] };
  c.shell = { sequence: 1, projects: [
    { id: 'p1', title: 'Alpha', workspaceRoot: '/a', createdAt: iso(-9_000_000), updatedAt: iso(-9_000_000) },
    { id: 'p2', title: 'Beta', workspaceRoot: '/b', createdAt: iso(-1_000_000), updatedAt: iso(-1_000_000) },
    { id: 'p3', title: 'Gamma', workspaceRoot: '/c', createdAt: iso(-5_000_000), updatedAt: iso(-100) }],
  threads: [{ id: 'a', projectId: 'p1', title: 'alpha thread', createdAt: iso(-8_000_000), latestUserMessageAt: iso(-10_000), updatedAt: iso(-10_000) },
    { id: 'b', projectId: 'p2', title: 'beta thread', createdAt: iso(-900_000), latestUserMessageAt: iso(-500_000), updatedAt: iso(-500_000) }] };
  if (order) (c.local.clientSettings as unknown as Record<string, unknown>).sidebarProjectSortOrder = order;
  c.projectId = 'p2'; c.threadId = 'b';
  return c;
}
// 'local' matches every project on the same field (its environment), so search ranking keeps the list order.
const projectRows = (c: T3Client, page = '') => commandView(c, { page, query: page ? '' : 'local', now: NOW, scheme: 'dark', matches: new Map(), matchQuery: '', searching: false })
  .rows.filter(row => row.key.startsWith(page ? 'projects:new-thread-in:' : 'projects-search:project:')).map(row => row.title);

describe('palette projects follow Project order (CommandPalette pickerProjects)', () => {
  test('Projects search: the current project first, then the sidebar order for each setting', () => {
    expect(projectRows(paletteClient())).toEqual(['Beta', 'Gamma', 'Alpha']);
    expect(projectRows(paletteClient('created_at'))).toEqual(['Beta', 'Gamma', 'Alpha']);
    expect(projectRows(paletteClient('manual'))).toEqual(['Beta', 'Alpha', 'Gamma']);
    const draft = paletteClient(); draft.projectId = 'p1'; draft.threadId = '';
    expect(projectRows(draft)).toEqual(['Alpha', 'Gamma', 'Beta']);
  });
  test('New thread in... reads the same list', () => {
    const c = paletteClient('created_at'); c.projectId = 'p1'; c.threadId = 'a';
    expect(projectRows(c, 'new-thread-in')).toEqual(['Alpha', 'Beta', 'Gamma']);
    expect(pickerProjects(paletteClient('manual')).map(project => project.id)).toEqual(['p2', 'p1', 'p3']);
  });
  test('a grouped repository is one entry titled by the group, targeting the current member', () => {
    const c = paletteClient();
    c.shell = { ...c.shell, projects: c.shell.projects.map(project => project.id === 'p3' ? project : { ...project, repositoryIdentity: { canonicalKey: 'repo', displayName: 'Repo', rootPath: str(project.workspaceRoot) } }) };
    const list = pickerProjects(c);
    expect(list.map(project => [project.id, project.title])).toEqual([['p2', 'Repo'], ['p3', 'Gamma']]);
  });
});
const str = (value: unknown) => typeof value === 'string' ? value : '';

describe('sidebar width (threadSidebarWidth.ts)', () => {
  test('the initial width: 16rem, at least 13rem, leaving 40rem of main content', () => {
    expect([initialSidebarWidth(null, 1280), initialSidebarWidth(null, 840), initialSidebarWidth(null, 880)]).toEqual([256, 208, 240]);
    expect([initialSidebarWidth(300, 1280), initialSidebarWidth(300, 900), initialSidebarWidth(100, 1280)]).toEqual([300, 260, 208]);
    expect(sidebarMaximumWidth(600)).toBe(208);
  });
  test('with no stored width the size is fixed at load; a stored width leaves the clamp to the window', async () => {
    const { files } = disk();
    const client = new T3Client();
    expect(sidebarLaunchWidth(client, 840, true)).toBe(0); // preferences not read yet: the window clamps
    await load(client, files);
    expect(sidebarLaunchWidth(client, 840, false)).toBe(0); // the baked answer
    expect(sidebarLaunchWidth(client, 840, true)).toBe(208);
    expect(sidebarLaunchWidth(client, 1280, true)).toBe(208); // a later resize keeps it
    client.local.sidebarWidth = 300; (client.local as unknown as { sidebarWidthStored: boolean }).sidebarWidthStored = true;
    expect(sidebarLaunchWidth(client, 1280, true)).toBe(0);
    await sidebarLocal(client, null as unknown as Native, 'width-reset', '', '');
    expect([client.local.sidebarWidth, sidebarWidthStored(client)]).toEqual([256, false]);
    expect(sidebarLaunchWidth(client, 1280, true)).toBe(256); // the reset sizes it again from the window
    expect(sidebarLaunchWidth(client, 840, true)).toBe(208); // r12-sidebar: f870c41 clamps the fixed size to the live window (refresize.mjs: 256 → 208 at 840)
  });
  test('only a finished drag stores the width; toggles send 0; older files count a non-default width as stored', async () => {
    const legacy = disk(JSON.stringify({ version: 1, sidebarWidth: 256, sidebarOpen: false }));
    const a = new T3Client(); await load(a, legacy.files);
    expect(sidebarWidthStored(a)).toBe(false);
    const dragged = disk(JSON.stringify({ version: 1, sidebarWidth: 312 }));
    const b = new T3Client(); await load(b, dragged.files);
    expect([b.local.sidebarWidth, sidebarWidthStored(b)]).toEqual([312, true]);
    const flagged = disk(JSON.stringify({ version: 1, sidebarWidth: 312, sidebarWidthStored: false }));
    const c = new T3Client(); await load(c, flagged.files);
    expect(sidebarWidthStored(c)).toBe(false);
  });
});

describe('the first-run gate waits for the saved preferences (FirstRunGate hydration)', () => {
  const native = { available: true, watch: () => {}, later: async (request: Obj) => request.op === 'environments' ? { ok: true, value: { saved: [] }, generation: 1 } : { ok: true, value: {}, generation: 1 } } as unknown as Native;
  test('a relaunch with setup done and no saved environment opens the app, not the wizard', async () => {
    const { files } = disk(JSON.stringify({ version: 1, pages: { onboardingCompletedAt: '2026-10-04T00:00:00.000Z' } }));
    const client = new T3Client();
    expect((await welcomeView(client, native, { step: 'connect', now: NOW })).show).toBe(false); // pending until read
    await load(client, files);
    expect(pagesPrefs(client).onboardingCompletedAt).toBe('2026-10-04T00:00:00.000Z');
    expect((await welcomeView(client, native, { step: 'connect', now: NOW })).show).toBe(false);
  });
  test('a first launch still shows the wizard once the empty preferences are read', async () => {
    const { files } = disk();
    const client = new T3Client();
    await load(client, files);
    expect((await welcomeView(client, native, { step: 'connect', now: NOW })).show).toBe(true);
  });
});

test('the SnapShot shortcut conflict names Send and Start New Thread as the reference does', () => {
  expect(commandLabel('composer.sendAndNewThread')).toBe('Composer: Send and Start New Thread');
});

test('header shortcut labels: the newest binding that wins its chord in the default context (findEffectiveShortcutForCommand)', () => {
  const notTerminal = { type: 'not', node: { type: 'identifier', name: 'terminalFocus' } }, terminal = { type: 'identifier', name: 'terminalFocus' };
  const config = { keybindings: [
    { command: 'terminal.new', shortcut: { key: 'n', modKey: true }, whenAst: terminal },
    { command: 'chat.new', shortcut: { key: 'n', modKey: true }, whenAst: notTerminal },
    { command: 'chat.new', shortcut: { key: 'o', modKey: true, shiftKey: true }, whenAst: notTerminal },
    { command: 'chat.newLocal', shortcut: { key: 'n', modKey: true, shiftKey: true }, whenAst: notTerminal },
    { command: 'terminal.close', shortcut: { key: 'w', modKey: true }, whenAst: terminal },
    { command: 'rightPanel.close', shortcut: { key: 'w', modKey: true }, whenAst: notTerminal },
    { command: 'sidebar.toggle', shortcut: { key: 'b', modKey: true } }] };
  expect(['chat.new', 'chat.newLocal', 'terminal.new', 'terminal.close', 'rightPanel.close', 'sidebar.toggle'].map(command => commandShortcut(config, command)))
    .toEqual(['⇧⌘O', '⇧⌘N', '', '', '⌘W', '⌘B']);
});
