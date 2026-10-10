import { describe, test, expect } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';
import { terminalMetadataEvent } from './shared/terminal-drawer-view';
import { terminalDraftRecords } from './shared/terminal-integrations';
import { mobileTerminalPrepare, mobileTerminalAction, mobileTerminalCapture, mobileTerminalAttachOutput, mobileTerminalEvent, mobileTerminalColors } from './terminal-mobile';
function fixture() {
  const client = new T3Client(); client.origin = 'https://terminal.test'; client.environmentId = 'env'; client.threadId = 'thread'; client.projectId = 'p';
  client.connection = 'connected'; client.configLive = true; client.shellLive = true; client.threadLive = true;
  client.config = { environment: { platform: { os: 'darwin' } } };
  client.shell.projects = [{ id: 'p', title: 'Repo', workspaceRoot: '/repo' }];
  client.shell.threads = [{ id: 'thread', projectId: 'p', title: 'Thread' }];
  const calls: Obj[] = [], auth = { authenticated: true, permissions: ['terminal:read', 'terminal:operate'] };
  const files: Files = { fs: { async mkdir() {}, async readFile() { return new ArrayBuffer(0); }, async atomicWriteFile() {} } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    return { ok: true, generation: client.generation, value: request.op === 'http' ? auth : request.op === 'ids' ? ['4f138455-8a22-42fb-85ad-9a7ae2d22766'] : request.op === 'subscribe' ? { id: 'sub-1' } : {} };
  } };
  return { client, native, files, calls, auth };
}
const prepare = (f: ReturnType<typeof fixture>, id = '') => mobileTerminalPrepare(id, 'dark', 't3-code', 11, 1000, f.native, f.client);
const act = (f: ReturnType<typeof fixture>, op: string, id = 'term-1', value = '') => mobileTerminalAction(op, id, value, f.native, f.files, f.client);
function metadata(f: ReturnType<typeof fixture>) {
  terminalMetadataEvent(f.client, { subscriptionId: 'sub-1', value: { type: 'snapshot', terminals: [
    { threadId: 'thread', terminalId: 'term-2', cwd: '/actual', status: 'running', label: 'Tests', hasRunningSubprocess: true, updatedAt: '2026-01-01T00:00:00Z' }] } });
}
describe('mobile terminal actual ownership and permissions', () => {
  test('uses actual descriptor/workspace/font and authorizes before native attachment', async () => {
    const f = fixture(), view = await prepare(f);
    expect(view).toMatchObject({ ready: true, readOnly: false, title: 'Terminal', subtitle: 'Repo', hostPlatform: 'mac' });
    expect(JSON.parse(view.sourceJSON)).toMatchObject({ cwd: '/repo', threadId: 'thread', terminalId: 'term-1', fontSize: 11, readOnly: false });
    expect(f.calls.some(c => c.op === 'mobileTerminalPermissions' && c.operate === true)).toBe(true);
    expect(f.calls.some(c => c.method === 'terminal.open')).toBe(false);
  });
  test('read-only chooses existing sessions and never creates or clears a shell', async () => {
    const f = fixture(); f.auth.permissions = ['terminal:read']; await prepare(f); metadata(f);
    const view = await prepare(f); expect(view).toMatchObject({ ready: true, terminalId: 'term-2', readOnly: true });
    expect(JSON.parse(view.sourceJSON).cwd).toBe('/actual');
    expect((await act(f, 'new')).message).toContain('permission'); expect((await act(f, 'clear', 'term-2')).message).toContain('permission');
    expect(f.calls.some(c => c.method === 'terminal.open' || c.method === 'terminal.clear')).toBe(false);
  });
  test('permission downgrade propagates natively and refuses subsequent operations', async () => {
    const f = fixture(); await prepare(f); f.auth.permissions = [];
    expect((await prepare(f)).ready).toBe(false);
    expect(f.calls.filter(c => c.op === 'mobileTerminalPermissions').at(-1)).toMatchObject({ read: false, operate: false });
    expect((await act(f, 'input', 'term-1', 'danger')).message).toContain('permission');
  });
  test('late authorization cannot attach or write another selected thread', async () => {
    const f = fixture(), original = f.native.later;
    f.native.later = async input => { const reply = await original(input); if (obj(input).op === 'http') f.client.threadId = 'other'; return reply; };
    expect((await prepare(f)).error).toContain('changed');
    expect(f.calls.some(c => c.op === 'mobileTerminalPermissions')).toBe(false);
    expect((await act(f, 'new')).message).toContain('permissions');
  });
  test('new and close await actual RPC success, including real workspace and deleteHistory', async () => {
    const f = fixture(); await prepare(f); const result = await act(f, 'new'); expect(result.message).toBe('');
    expect(f.calls.find(c => c.method === 'terminal.open')?.payload).toMatchObject({ threadId: 'thread', cwd: '/repo', cols: 80, rows: 24 });
    expect((await act(f, 'close', result.terminalId)).closed).toBe(true);
    expect(f.calls.find(c => c.method === 'terminal.close')?.payload).toEqual({ threadId: 'thread', terminalId: result.terminalId, deleteHistory: true });
  });
  test('server refusal does not claim terminal creation succeeded', async () => {
    const f = fixture(); await prepare(f); const original = f.native.later;
    f.native.later = async input => obj(input).method === 'terminal.open' ? { ok: false, generation: f.client.generation, error: { kind: 'Terminal', message: 'workspace removed' } } : original(input);
    expect((await act(f, 'new')).message).toBe('workspace removed');
  });
  test('native capture rejects other session events; range selection is frozen and bounded', () => {
    expect(mobileTerminalEvent(JSON.stringify({ key: 'old', type: 'capture', text: 'old output' }), 'current').type).toBe('');
    expect(mobileTerminalCapture('first\nsecond\nthird\n', 1, 2)).toMatchObject({ text: 'second\nthird', start: 1, end: 2, canAttach: true });
    expect(mobileTerminalCapture('x'.repeat(64001), 0, -1)).toMatchObject({ tooLarge: true, canAttach: false });
  });
  test('only an actual ended stream leaves the current session, preferring a remaining live session', async () => {
    const f = fixture(); await prepare(f); metadata(f);
    const key = JSON.stringify(['env', 'thread', 'term-1']);
    expect(mobileTerminalEvent(JSON.stringify({ key, type: 'status', status: 'closed', version: 0 }), key).action).toBe('');
    expect(mobileTerminalEvent(JSON.stringify({ key, type: 'status', status: 'exited', version: 2 }), key).action).toBe('session-ended');
    expect(await act(f, 'session-ended', 'term-1')).toMatchObject({ terminalId: 'term-2', closed: false, message: '' });
    expect(await act(f, 'session-ended', 'term-2')).toMatchObject({ terminalId: '', closed: true, message: '' });
  });
  test('capture inserts a real shared context record and refuses a project switch during ID allocation', async () => {
    const f = fixture(), key = JSON.stringify(['env', 'thread', 'term-1']);
    expect((await mobileTerminalAttachOutput(key, 'first\nsecond\n', 1, 1, 1000, f.native, f.files, f.client)).message).toBe('');
    expect(terminalDraftRecords(f.client, f.client.draft)[0]).toMatchObject({ kind: 'terminal', terminalId: 'term-1', text: 'second', lineStart: 2, lineEnd: 2 });
    const prior = f.client.draft; const original = f.native.later;
    f.native.later = async input => { const response = await original(input); if (obj(input).op === 'ids') f.client.threadId = 'other'; return response; };
    expect((await mobileTerminalAttachOutput(key, 'late', 0, 0, 1000, f.native, f.files, f.client)).message).toContain('changed');
    expect(f.client.draft).toBe(''); expect(f.client.local.drafts['env:thread']).toBe(prior);
  });
});


test('terminal capture rejects an edit ending during native ID allocation instead of using ordinary content', async () => {
  const { queuedEditState, queuedEditThreadKey, queuedEditEndMemory } = await import('./queued-edit-state');
  const f = fixture(); f.client.local.drafts[f.client.draftKey] = 'ordinary untouched';
  const edit = { owner: 'terminal-edit', session: 'terminal-session', draftKey: 'env:thread~queued-edit~r', origin: f.client.origin,
    environmentId: 'env', threadId: 'thread', projectId: 'p', generation: f.client.generation, revision: 1, runId: 'r', messageId: 'm', text: 'queued',
    attachments: [], existingAttachments: [], saving: false };
  const state = queuedEditState(f.client); state.sessions.set(edit.owner, edit); state.active.set(queuedEditThreadKey('env', 'thread'), edit.owner);
  const original = f.native.later;
  f.native.later = async input => { const reply = await original(input); if (obj(input).op === 'ids') queuedEditEndMemory(edit.owner, f.client); return reply; };
  const result = await mobileTerminalAttachOutput(JSON.stringify(['env', 'thread', 'term-1']), 'output', 0, 0, 1000, f.native, f.files, f.client);
  expect(result.message).toContain('changed'); expect(f.client.draft).toBe('ordinary untouched');
  expect(terminalDraftRecords(f.client, f.client.draft)).toEqual([]);
});

// Native menus consume the captured transport identity and real session metadata.
test('terminal menu configuration keeps source session cwd, status and selection', async () => {
  const f = fixture(); await prepare(f); metadata(f);
  const view = await prepare(f, 'term-2'), source = JSON.parse(view.sourceJSON);
  expect(source).toMatchObject({ key: view.sessionKey, generation: f.client.generation, workspaceRoot: '/actual',
    tabs: [{ id: 'term-2', label: 'Tests', cwd: '/actual', status: 'running', running: true, selected: true }] });
  expect(mobileTerminalEvent(JSON.stringify({ type: 'menu', key: view.sessionKey, action: 'select', text: 'term-2' }), view.sessionKey))
    .toMatchObject({ type: 'menu', action: 'select', text: 'term-2' });
  expect(mobileTerminalEvent(JSON.stringify({ type: 'menu', key: 'previous', action: 'new' }), view.sessionKey).type).toBe('');
});

test('terminal capture uses sheet roles for every bundled appearance', async () => {
  const { mobileTheme } = await import('./design');
  for (const palette of ['t3-code', 't3-chat', 'grove', 'ocean', 'ember', 'iris']) {
    for (const scheme of ['light', 'dark']) {
      const theme = mobileTheme(scheme, palette), colors = mobileTerminalColors(scheme, palette);
      expect(colors.sheetBackground).toBe(theme.sheet!);
      expect(colors.sheetForeground).toBe(theme.foreground!);
      expect(colors.sheetSubtle).toBe(theme.colors.subtle!);
    }
  }
});
