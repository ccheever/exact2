// audit-wave-followups (FU-1..FU-5, found by the 2026-10-10 audit fix wave): an empty server thread docks its composer
// over "Send a message to start the conversation.", the right panel's open state is the thread's, ⌘↩ belongs to a
// focused Files preview comment draft, a provider link to another target closes "Add custom model", and Escape in the
// command palette over Settings closes only the palette. Reference: T3 Code 1e2ecbd975 (ChatView.tsx isDraftHeroState,
// MessagesTimeline.tsx, rightPanelStore.ts, FilePreviewPanel.tsx with DiffCommentAnnotation, ProviderSettingsPanel.tsx's
// keyed content, useEscapeToGoBack). The keys and the layout are driven on macOS (the task record's acceptance).
import { describe, expect, test } from 'bun:test';
import { T3Client } from './client';
import { snapshot } from './presentation';
import { obj, type Obj } from './domain';
import type { Files, Native } from './protocol';
import { panelState, panelView, surfaceLocal } from './r4-surfaces-panel';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
async function component(file: string, name: string): Promise<string> {
  const text = await source(file);
  const start = text.indexOf(`\ncomponent ${name}\n`);
  expect(start).toBeGreaterThanOrEqual(0);
  const rest = text.slice(start + 1);
  const end = rest.slice(1).search(/\n(component|shape|style|fn|use) /);
  return end < 0 ? rest : rest.slice(0, end + 1);
}
const line = (text: string, needle: string) => text.split('\n').find(candidate => candidate.includes(needle)) ?? '';

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
    configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:read', 'orchestration:operate'],
    config: { environment: { capabilities: { serverResolvedCommandContext: true } } } });
  client.shell.projects = [{ id: 'p1', title: 'Fixture', workspaceRoot: '/repo' }];
  client.shell.threads = [{ id: 't1', projectId: 'p1' }, { id: 't2', projectId: 'p1' }];
  client.thread = { projection: { thread: { id: 't1' }, runtimeRequests: [], turnItems: [], runs: [], checkpoints: [] }, sequence: 0, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null };
  const surface = (op: string, id = '', value = '') => surfaceLocal(client, native, op, id, value);
  return { client, native, storage, calls, surface };
}

describe('FU-1: a server thread with no messages docks the composer over the timeline\'s placeholder', () => {
  test('only a draft with an empty timeline is the hero (isDraftHeroState); the placeholder waits for the thread\'s detail', async () => {
    const column = await component('app-main.contract', 'ChatColumn');
    expect(column).toContain('  derive hero = length(data.messages) == 0 and data.threadId == ""\n  derive overlaid = not hero\n');
    expect(column).toContain('      when hero\n        column flex=1');
    // MessagesTimeline's empty state: text-sm text-muted-foreground/30, centred, not while the detail loads or a run works.
    expect(column).toContain('      when length(data.messages) == 0 and data.threadId != ""\n');
    expect(column).toContain('          when not data.threadLoading and not data.running\n            text "Send a message to start the conversation." font-size="0.875rem" line-height="1.25rem" color="light-dark(#71717b4d, #8181814d)"');
    // The draft hero's spacer under the composer is the hero's alone, and the canvas measures the docked overlay.
    expect(column).toContain('      when hero and data.connected and data.projectId != ""\n');
    expect(line(await source('app.contract'), 'resource canvas = chatCanvas(')).toContain('length(data.messages) > 0 or data.threadId != ""');
  });

  test('threadLoading is true from a server thread\'s selection until its detail arrives, never on a draft', () => {
    const { client } = harness();
    expect(snapshot(client)).toMatchObject({ threadId: 't1', threadLoading: false, messages: [] });
    client.thread = null;
    expect(snapshot(client).threadLoading).toBe(true);
    client.threadId = '';
    expect(snapshot(client).threadLoading).toBe(false);
  });
});

describe('FU-2: the right panel\'s open state is the thread\'s (rightPanelStore byThreadKey)', () => {
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

  test('the window holds its open state for the panel key it was opened on, and adds the thread\'s launcher', async () => {
    const app = await source('app.contract');
    expect(app).toContain('  state rightPanelAt = ""\n  derive rightPanel = (rightPanelAt != "" and rightPanelAt == shell.panel.key) or shell.panel.launcher\n');
    expect(app).not.toMatch(/\bstate rightPanel = /);
    expect(app).not.toMatch(/^\s+rightPanel = /m);
    expect(line(app, '    rightPanelAt = closingTerminal ?')).toEndWith('? shell.panel.key : ""');
    const shapes = await source('r4-surfaces-shapes.contract');
    expect(shapes).toContain('shape R4Panel\n  key: string');
    expect(shapes).toContain('  launcher: bool');
  });
});

describe('FU-3: ⌘↩ in a Files preview comment draft (as PA-12 in the Diff)', () => {
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

  test('the Files draft card reports its focus as the Diff\'s does, on a queued send its save shares with nothing else', async () => {
    const preview = await component('r4-surfaces-files.contract', 'R4FilePreview');
    expect(line(preview, 'DiffDraftCard(')).toContain('focusIn=local("surface-files-comment-focus", files.path, ""), focusOut=local("surface-files-comment-blur", files.path, "")');
    // The live drive's first try: ⌘↩ saved the chip, but the composer's `draft` write (localChanged) replaced the save
    // before it closed the draft. The comment ops now have their own queued mutation.
    const app = await source('app.contract');
    expect(app).toContain('  mutation fileCommentChanged as shape Change queue refreshes data');
    expect(app).toContain('    if startsWith(op, "surface-files-comment-")\n      send fileCommentChanged = command(`chatlocal:${op}`, id, value, 0)\n    else\n      send localChanged = command(`chatlocal:${op}`');
    expect(line(app, '  action write(value: string)')).toBe('  action write(value: string)');
    expect(app).toContain('  action write(value: string)\n    draft = value\n    draftOwner = `${composerOwner}${data.requestKey}`\n    send localChanged = command("draft", "", value, 0)');
  });
});

describe('FU-4: a provider link to another target remounts the Providers page (ProviderSettingsPanelContent key)', () => {
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
      providerLinkVisit: (visit: number, onPage: boolean, machine: string, selected: string, nextMachine: string, nextSelected: string) => number;
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

  test('the root counts the links, and Settings reads the field through the count', async () => {
    const app = await source('app.contract');
    expect(app).toContain('      providerVisit = providerLinkVisit(providerVisit, settingsOpen and settingsRoute == "providers", settingsMachine, providerSelected, id == "" ? "" : machine, id == "" ? "" : `target:${id}`)\n      openSettings()');
    const settings = await component('app-settings.contract', 'SettingsWindow');
    expect(settings).toContain('  derive modelAddingLive = modelAddingOn(modelAdding, modelAddingVisit, providerVisit)\n');
    expect(settings).toContain('      modelAdding = value\n      modelAddingVisit = providerVisit\n');
    expect(line(settings, 'derive modelAddingShown')).toContain('modelAddingLive != ""');
    expect(line(settings, 'ProvidersPanel(')).toContain('modelAdding=modelAddingLive');
  });
});

describe('FU-5: Escape in the command palette over Settings closes only the palette', () => {
  test('the open palette owns Escape, so Settings\' Back gives up its shortcut', async () => {
    const settings = await component('app-settings.contract', 'SettingsWindow');
    expect(line(settings, 'derive escapeOwned =')).toContain(' or paletteOpen or ');
    expect(line(settings, 'SettingsNav(')).toContain('menuOpen=escapeOwned,');
    expect(line(await source('app-window.contract'), 'SettingsWindow(')).toContain('paletteOpen=paletteOpen');
    // The palette's own Escape stays (its hidden close button), in both of its views.
    const palette = await source('palette.contract');
    expect(palette.split('\n').filter(text => text.includes('testId="palette-escape"') && text.includes('aria-keyshortcuts="Escape"'))).toHaveLength(2);
  });
});
