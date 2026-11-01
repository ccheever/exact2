// An empty server thread docks its composer
// over "Send a message to start the conversation.", the right panel's open state is the thread's, ⌘↩ belongs to a
// focused Files preview comment draft, a provider link to another target closes "Add custom model", and Escape in the
// command palette over Settings closes only the palette. Reference: T3 Code 1e2ecbd975 (ChatView.tsx isDraftHeroState,
// MessagesTimeline.tsx, rightPanelStore.ts, FilePreviewPanel.tsx with DiffCommentAnnotation, ProviderSettingsPanel.tsx's
// keyed content, useEscapeToGoBack). The keys and the layout are checked by driving the app on macOS.
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { snapshot } from '../../src/client/presentation';
import { obj, type Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { panelState, panelView, surfaceLocal } from '../../src/panel/panel-surfaces';
import { appText } from '../support/app-files';

import { adoptHostLocale, currentTimestampLocale } from '../../src/shared/timestamp-format';

const source = async (file: string) => appText(file);

const appTs = 'import { answer } from "./answer";\n\nexport function hello(name: string): string {\n  return `Hello, ${name}`;\n}\n';
function harness() {
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    const payload = obj(request.payload);
    if (request.op === 'request' && request.method === 'projects.listEntries') return { ok: true, generation: 1, value: { entries: [] } };
    if (request.op === 'request' && request.method === 'projects.readFile') return { ok: true, generation: 1, value: { contents: payload.relativePath === 'README.md' ? '# Fixture\n' : appTs, byteLength: 120, truncated: false } };
    if (request.op === 'editorInsert' || request.op === 'editorEdit') return { ok: true, generation: 1, value: { applied: true } };
    return { ok: true, generation: 1, value: {} };
  } };
  const storage: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('missing'); }, async atomicWriteFile() {} } };
  const client = new T3Client();
  Object.assign(client, { available: true, generation: 1, connection: 'connected', environmentId: 'env', projectId: 'p1', threadId: 't1',
    configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:read', 'orchestration:operate', 'filesystem:read', 'terminal:read', 'terminal:operate', 'preview:operate'],
    config: { environment: { capabilities: { serverResolvedCommandContext: true } } } });
  client.shell.projects = [{ id: 'p1', title: 'Fixture', workspaceRoot: '/repo' }];
  client.shell.threads = [{ id: 't1', projectId: 'p1' }, { id: 't2', projectId: 'p1' }];
  client.thread = { projection: { thread: { id: 't1' }, runtimeRequests: [], turnItems: [], runs: [], checkpoints: [] }, sequence: 0, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null };
  const surface = (op: string, id = '', value = '') => surfaceLocal(client, native, op, id, value);
  return { client, native, storage, calls, surface };
}

describe('timestamps follow exactTime().formatLocale', () => {
  test('the snapshot request\'s format locale is the timestamps\' locale; none leaves the runtime default', () => {
    const { client } = harness();
    snapshot(client, 0, '', 'en-KR');
    expect(currentTimestampLocale()).toBe('en-KR');
    snapshot(client, 0, '', '');
    expect(currentTimestampLocale()).toBe('en-US');
    adoptHostLocale(null);
  });
});

describe('a server thread with no messages docks the composer over the timeline\'s placeholder', () => {
  test('threadLoading is true from a server thread\'s selection until its detail arrives, never on a draft', () => {
    const { client } = harness();
    expect(snapshot(client)).toMatchObject({ threadId: 't1', threadLoading: false, messages: [] });
    client.thread = null;
    expect(snapshot(client).threadLoading).toBe(true);
    client.threadId = '';
    expect(snapshot(client).threadLoading).toBe(false);
  });
});

describe('the right panel\'s open state is the thread\'s (rightPanelStore byThreadKey)', () => {
  test('the launcher opened on a thread stays that thread\'s: a fresh draft and another thread open closed, and it comes back', async () => {
    const { client, native, surface } = harness();
    const view = () => panelView(client, native, 0);
    expect(await view()).toMatchObject({ key: 'env:t1', launcher: false, open: false });
    // rightPanelStore.show: isOpen with no surface is the launcher (RightPanelEmptyState).
    await surface('show');
    expect(await view()).toMatchObject({ key: 'env:t1', launcher: true, open: false });
    // ⌘N: the project's draft has a panel of its own, closed.
    client.threadId = '';
    expect(await view()).toMatchObject({ key: 'env:new:p1', launcher: false, open: false });
    // Another thread too.
    client.threadId = 't2';
    expect(await view()).toMatchObject({ key: 'env:t2', launcher: false, open: false });
    client.threadId = 't1';
    expect(await view()).toMatchObject({ key: 'env:t1', launcher: true });
    // A surface replaces the launcher; hiding closes the thread's panel, and showing it again brings the surface back.
    await surface('open', '', 'files');
    expect(await view()).toMatchObject({ launcher: false, open: true, kind: 'files' });
    await surface('hide');
    expect(await view()).toMatchObject({ launcher: false, open: false });
    await surface('show');
    expect(await view()).toMatchObject({ launcher: false, open: true, kind: 'files' });
    await surface('close-all');
    expect(await view()).toMatchObject({ launcher: false, open: false, count: 0 });
  });
});

describe('⌘↩ in a Files preview comment draft (as in the Diff)', () => {
  test('while the draft\'s textarea holds the focus the composer\'s Send declares no chord', async () => {
    const { client, native, surface, calls } = harness();
    const chords = () => snapshot(client).composer.sendChords;
    await surface('file', 'src/app.ts', '');
    expect(chords()).toContain('Meta+Enter');
    // The gutter's "+" on line 3: the draft mounts with the focus (autofocus).
    await surface('files-comment-begin', 'src/app.ts', '3');
    expect(chords()).toBe('');
    await surface('files-comment-blur', 'src/app.ts', '');
    expect(chords()).toContain('Meta+Enter');
    await surface('files-comment-focus', 'src/app.ts', '');
    expect(chords()).toBe('');
    // Hiding the panel gives the chord back while the draft stays; showing it holds it again.
    await surface('hide');
    expect(chords()).toContain('Meta+Enter');
    await surface('show');
    expect(chords()).toBe('');
    // ⌘↩ reaches DiffDraftCard's `keys`, which saves: the comment becomes the composer's "app.ts L3" chip.
    await surface('files-comment-save', 'src/app.ts', 'audit note');
    expect(String(calls.filter(call => call.op === 'editorInsert').at(-1)?.text)).toStartWith('[app.ts L3](t3-context://v1/review-comment/');
    expect(chords()).toContain('Meta+Enter');
    // A focus report with no draft open never holds the chord.
    await surface('files-comment-focus', 'src/app.ts', '');
    expect(chords()).toContain('Meta+Enter');
  });

  test('a draft that left the tree without a blur holds nothing: another file, another thread, the editor, a cancel', async () => {
    const { client, native, surface } = harness();
    const chords = () => snapshot(client).composer.sendChords;
    await surface('file', 'src/app.ts', '');
    await surface('files-comment-begin', 'src/app.ts', '2');
    expect(chords()).toBe('');
    // Another file tab: the draft's card is not drawn (no blur comes; LLP 1008).
    await surface('file', 'README.md', '');
    expect(chords()).toContain('Meta+Enter');
    await surface('activate', 'file:src/app.ts', '');
    expect(chords()).toBe('');
    // Another thread's panel: the focus was this thread's.
    client.threadId = 't2';
    await panelView(client, native, 0);
    expect(chords()).toContain('Meta+Enter');
    client.threadId = 't1';
    expect(chords()).toBe('');
    // The editor hides the cards.
    await surface('files-begin-edit', 'src/app.ts', '');
    expect(chords()).toContain('Meta+Enter');
    await surface('files-end-edit', 'src/app.ts', '');
    expect(chords()).toBe('');
    await surface('files-comment-cancel', 'src/app.ts', '');
    expect(chords()).toContain('Meta+Enter');
    expect(panelState(client).visible).toBe(true);
  });
});

describe('a provider link to another target remounts the Providers page (ProviderSettingsPanelContent key)', () => {
  // The fns are read from app-settings.contract and run as JavaScript, as diff-panel-parity.test.ts runs dbKeys.
  const fns = async () => {
    const text = await source('app-settings.contract');
    const names = ['providerLinkVisit', 'modelAddingOn'];
    const decls = names.map(name => {
      const found = new RegExp(`^fn ${name}\\(([^)]*)\\): [^=]+ = (.+)$`, 'm').exec(text);
      if (!found) throw new Error(`app-settings.contract: no fn ${name}`);
      const params = found[1]!.split(',').map(param => param.split(':')[0]!.trim());
      const body = found[2]!.replace(/\band\b/g, '&&').replace(/\bor\b/g, '||').replace(/\bnot\b/g, '!').replace(/ == /g, ' === ');
      return `function ${name}(${params.join(', ')}) { return ${body}; }`;
    });
    return new Function(`${decls.join('\n')}\nreturn { ${names.join(', ')} };`)() as {
      providerLinkVisit: (visit: number, onPage: boolean, machine: string, target: string, nextMachine: string, nextTarget: string) => number;
      modelAddingOn: (adding: string, openedOn: number, visit: number) => string;
    };
  };

  test('A → B → A closes the field A opened; a link to the page\'s own target keeps it', async () => {
    const { providerLinkVisit, modelAddingOn } = await fns();
    // Settings open on Providers by a link to A; "Add custom model" opened on A's Models block (visit 1).
    let visit = providerLinkVisit(0, false, '', '', 'env', 'target:codex');
    let page = { machine: 'env', selected: 'target:codex' };
    const openedOn = visit, adding = 'codex:models:0';
    expect(modelAddingOn(adding, openedOn, visit)).toBe(adding);
    const link = (machine: string, selected: string) => { visit = providerLinkVisit(visit, true, page.machine, page.selected, machine, selected); page = { machine, selected }; };
    // The same target again: no remount, the field stays (same search, same key).
    link('env', 'target:codex');
    expect(modelAddingOn(adding, openedOn, visit)).toBe(adding);
    // A → B → A: each link changes the key, so the page remounts twice and the field is gone on A.
    link('env', 'target:claudeAgent');
    link('env', 'target:codex');
    expect(page.selected).toBe('target:codex');
    expect(modelAddingOn(adding, openedOn, visit)).toBe('');
    // Another environment's same instance is another key too; a link from another route always remounts.
    expect(providerLinkVisit(5, true, 'env', 'target:codex', 'env2', 'target:codex')).toBe(6);
    expect(providerLinkVisit(5, false, 'env', 'target:codex', 'env', 'target:codex')).toBe(6);
  });

  // Review of #372: the link compared with `providerSelected`, which a provider chosen on the page overwrites with its
  // raw row id (providerUi "select"; providers.ts takes both forms). The reference keys the content on the route's
  // environment and instance only (ProviderSettingsPanel.tsx:288); the page's pick is local useState (line 630).
  test('the route\'s target, not the page\'s pick: link A, pick A, Add, link A keeps the field; a link after the nav closes it', async () => {
    const { providerLinkVisit, modelAddingOn } = await fns();
    const app = await source('app.contract');
    // The root state openProviderSettings compares the link with, read from the source, so the model runs what the root does.
    const compared = /providerVisit = providerLinkVisit\(providerVisit, settingsOpen and settingsRoute == "providers", settingsMachine, (\w+), /.exec(app)?.[1];
    expect(compared === 'providerSelected' || compared === 'providerLinkTarget').toBe(true);
    const root = { settingsOpen: false, settingsRoute: 'general', settingsMachine: '', providerSelected: '', providerLinkTarget: '', providerVisit: 0 };
    const link = (id: string) => {
      root.providerVisit = providerLinkVisit(root.providerVisit, root.settingsOpen && root.settingsRoute === 'providers', root.settingsMachine, root[compared as 'providerSelected'], 'env', `target:${id}`);
      Object.assign(root, { settingsOpen: true, settingsRoute: 'providers', settingsMachine: 'env', providerSelected: `target:${id}`, providerLinkTarget: `target:${id}` });
    };
    const pick = (id: string) => { root.providerSelected = id; };
    const navigate = (route: string) => { root.settingsRoute = route; root.providerLinkTarget = ''; };
    link('codex');
    pick('codex');
    let openedOn = root.providerVisit;
    const adding = 'codex:models:0';
    link('codex');
    expect(modelAddingOn(adding, openedOn, root.providerVisit)).toBe(adding);
    // Away by the nav and back: the route names no instance, so A's link is another key and the field closes.
    navigate('general');
    navigate('providers');
    openedOn = root.providerVisit;
    link('codex');
    expect(modelAddingOn(adding, openedOn, root.providerVisit)).toBe('');
    // The source writes what the model does: the link sets the target, the page's pick leaves it, every other way onto
    // a route (the nav, a search result or its Enter, the scope, a toast's or the palette's Settings link) drops it.
    expect(app).toContain('      providerSelected = id == "" ? "" : `target:${id}`\n      providerLinkTarget = id == "" ? "" : `target:${id}`\n');
    const providerUi = app.slice(app.indexOf('  action providerUi(what: string, value: string)'));
    expect(providerUi.slice(0, providerUi.indexOf('\n  action ', 1))).not.toContain('providerLinkTarget');
    for (const head of ['  action coreNavigate(route: string)\n    settingsRoute = route\n',
      '  action corePick(route: string, target: string, scope: string, id: string)\n    settingsRoute = route\n',
      '    if k == "Enter" and settingsNavigation.firstRoute != ""\n      settingsRoute = settingsNavigation.firstRoute\n',
      '    if op == "ui:settings"\n      settingsRoute = target\n',
      '    if op == "settings" or op == "project-settings"\n      paletteOpen = false\n']) {
      expect(app).toContain(head);
      expect(app.slice(app.indexOf(head) + head.length).split('\n')[0]!.trim()).toStartWith('providerLinkTarget = ""');
    }
    expect(app).toContain('    providerSelected = "" // routes/settings.tsx keys the page on its scope: Providers starts over on the new environment\'s first row\n    providerLinkTarget = ""\n');
  });
});
