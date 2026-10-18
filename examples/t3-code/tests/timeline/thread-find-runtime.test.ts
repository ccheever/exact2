import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import type { Obj } from '../../src/client/domain';
import { threadFindAction, threadFindEvent, threadFindState, threadFindTick, threadFindView } from '../../src/timeline/thread-find-runtime';
const native = { available: true } as Native;
const result = { snapshotSequence: 1, totalMatches: 1, activeIndex: 0, match: { entryId: 'old', runId: 'r', occurrence: 0 } };
function fixture(progressive = false) {
  const calls: Obj[] = [], rows: Obj[] = [];
  const client = { generation: 1, environmentId: 'e', threadId: 't', origin: 'http://fixture', config: { threadFind: true, threadFindProgressive: progressive },
    thread: { sequence: 1, hasMore: true, projection: { visibleTurnItems: rows } }, projection: { visibleTurnItems: rows },
    rpc: async (_native: Native, method: string, payload: Obj) => { calls.push({ method, payload }); return result; },
    restAccess: () => ({ call: async (request: Obj) => { calls.push(request); return { id: '1-1' }; } }),
    history: async (_native: Native, id: string) => { calls.push({ op: 'history', throughEntryId: id }); rows.push({ sourceThreadId: 't', sourceItemId: id, item: { id } }); },
  } as unknown as T3Client;
  return { client, calls, rows };
}
async function search(client: T3Client) {
  await threadFindTick(client, native, 1000, [{ name: 'tool', displayName: 'Tool' }], '["t","visible"]');
  threadFindAction(client, 'open', ''); threadFindAction(client, 'query', 'needle');
  return threadFindTick(client, native, 1100, [{ name: 'tool', displayName: 'Tool' }]);
}
describe('thread find transport and paging', () => {
  test('a read-only RPC searches the server and pages through the missing match', async () => {
    const { client, calls } = fixture(); const view = await search(client);
    expect(calls[0]).toEqual({ method: 'orchestration.searchThread', payload: { threadId: 't', query: 'needle', skills: [{ name: 'tool', displayName: 'Tool' }] } });
    expect(calls[1]).toEqual({ op: 'history', throughEntryId: 'old' }); expect(view.entry).toBe('["t","old"]');
  });
  test('message search ids resolve to the projected item key after paging, and viewport starts use message ids', async () => {
    const { client, calls, rows } = fixture();
    rows.push({ sourceThreadId: 't', sourceItemId: 'turn-item:message:visible', item: { id: 'turn-item:message:visible', messageId: 'visible' } });
    client.history = async (_native: Native, id: string) => {
      calls.push({ op: 'history', throughEntryId: id });
      rows.push({ sourceThreadId: 't', sourceItemId: `turn-item:message:${id}`, item: { id: `turn-item:message:${id}`, messageId: id } });
    };
    await threadFindTick(client, native, 1000, [], '["t","turn-item:message:visible"]');
    threadFindAction(client, 'open', ''); threadFindAction(client, 'query', 'needle');
    const view = await threadFindTick(client, native, 1100);
    expect(view.entry).toBe('["t","turn-item:message:old"]');
    await threadFindTick(client, native, 1200, [], '["t","turn-item:message:visible"]');
    threadFindAction(client, 'open', ''); await threadFindTick(client, native, 1200);
    expect((calls.at(-1)!.payload as Obj).start).toEqual({ entryId: 'visible', occurrence: 0 });
    expect(calls.filter(call => call.op === 'history')).toHaveLength(1);
  });
  test('progressive frames before the subscribe reply are accepted, stale stream keys are ignored', async () => {
    const { client, calls } = fixture(true); await search(client);
    const key = String(calls[0]!.key); expect(calls[0]!.method).toBe('orchestration.searchThreadStream');
    threadFindEvent(client, { key, generation: 1, value: { ...result, complete: false } });
    expect(threadFindView(client).count).toBe('1/…');
    threadFindAction(client, 'query', 'second'); await threadFindTick(client, native, 1200);
    threadFindEvent(client, { key, generation: 1, value: { ...result, totalMatches: 99 } }); expect(threadFindView(client).count).not.toBe('1/99');
    const next = threadFindState(client).key; threadFindEvent(client, { key: next, generation: 1, value: result });
    await threadFindTick(client, native, 1200); expect(threadFindView(client).entry).toBe('["t","old"]');
  });
  test('closing and switching threads unsubscribe and invalidate late frames', async () => {
    const { client, calls } = fixture(true); await search(client); const key = String(calls[0]!.key);
    threadFindAction(client, 'close', ''); await threadFindTick(client, native, 1200);
    expect(calls.some(call => call.op === 'unsubscribe' && call.key === key)).toBeTrue();
    client.threadId = 'other'; threadFindEvent(client, { key, generation: 1, value: result }); expect(threadFindView(client).open).toBeFalse();
  });
  test('a failed read has an explicit retry and never loads a stale thread', async () => {
    const { client, calls } = fixture(); client.rpc = async () => { throw new Error('Offline'); };
    expect((await search(client)).error).toBe('Offline'); expect(calls).toHaveLength(0);
    threadFindAction(client, 'retry', ''); client.rpc = async () => { client.threadId = 'other'; return result; };
    await threadFindTick(client, native, 1200); expect(calls).toHaveLength(0); expect(threadFindView(client).open).toBeFalse();
  });
});

test('Cmd+F follows the focused Diff/PR Code scope and yields to the composer or a changed surface', async () => {
  const { T3Client } = await import('../../src/client/client');
  const { keyboardDispatch } = await import('../../src/shell/keyboard-dispatch');
  const { focusDiffFind } = await import('../../src/diff/diff-find');
  const client = new T3Client(); client.threadId = 't'; client.config = { keybindings: [{ command: 'chat.find', shortcut: { key: 'f', modKey: true } }] };
  const context = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: true };
  const find = (extra = {}) => keyboardDispatch(client, [], '', '', { ...context, ...extra }).find(row => row.command === 'chat.find');
  expect(find()).toMatchObject({ kind: 'command', target: 'chatlocal:find-open' });
  focusDiffFind(client, 'panel'); expect(find({ findPanelKind: 'diff' })).toMatchObject({ kind: 'command', target: 'chatlocal:diff-find-open' });
  expect(find({ composerFocus: true })?.target).toBe('chatlocal:find-open');
  expect(find({ findPanelKind: 'files' })?.target).toBe('chatlocal:find-open');
  focusDiffFind(client, 'pr', 'ref'); expect(find({ findPanelKind: 'pull-request', prCodeOpen: true })).toMatchObject({ kind: 'command', target: 'chatlocal:pr-code-find-open', extra: 'ref' });
  expect(find({ findPanelKind: 'pull-request', prCodeOpen: false })?.target).toBe('chatlocal:find-open');
});

test('find shortcuts have a renderer branch and its command arguments open each owning model', async () => {
  const { opened } = await import('../support/client-fixture');
  const { appPath } = await import('../support/app-files');
  const { keyboardDispatch } = await import('../../src/shell/keyboard-dispatch');
  const { focusDiffFind } = await import('../../src/diff/diff-find');
  const { peekCode } = await import('../../src/pull-requests/pages-pr-code');
  const { client, command } = await opened();
  client.config = { ...client.config, threadFind: true, keybindings: [{ command: 'chat.find', shortcut: { key: 'f', modKey: true } }] };
  const renderer = await Bun.file(appPath('src/settings/settings-shortcuts.contract')).text();
  const kinds = new Set([...renderer.matchAll(/if item\.kind == "([^"]+)"/g)].map(match => match[1]));
  const context = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: true };
  const reference = JSON.stringify({ projectId: 'p1', repository: 'fixture/repo', number: 42 });
  for (const slot of ['thread', 'panel', 'pr'] as const) {
    focusDiffFind(client, slot === 'thread' ? null : slot, slot === 'pr' ? reference : '');
    const row = keyboardDispatch(client, [], '', '', { ...context, findPanelKind: slot === 'pr' ? 'pull-request' : 'diff', prCodeOpen: true }).find(row => row.command === 'chat.find')!;
    expect(kinds.has(row.kind)).toBeTrue();
    expect(row.kind).toBe('command');
    expect(renderer).toContain('ShortcutButton(item=item, press=command(item.target, item.extra, "", 0))');
    expect((await command(row.target, row.extra, '', 0)).message).toBe('');
    if (slot === 'thread') expect(threadFindView(client).open).toBeTrue();
    if (slot === 'panel') expect(client.diffState.find.open).toBeTrue();
    if (slot === 'pr') expect(peekCode(client)?.find.open).toBeTrue();
  }
});

test('file, citation and skill faces paint the displayed label while preserving a different raw run text', async () => {
  const { contractComponent } = await import('../support/contract-action-fixture');
  const run = { text: 'src/needle.ts:7', findParts: [{ id: '0', text: 'needle', find: 3 }, { id: '1', text: '.ts · L7', find: 0 }] };
  const label = 'needle.ts · L7';
  for (const name of ['FileChipFace', 'CitationContextChip', 'ChipFace']) {
    const face = contractComponent('markdown.contract', name);
    const scope = { run, label, chip: { kind: 'skill', label }, urls: [] };
    expect(face.drawn('FindParts(parts=run.findParts)', scope)).toBeTrue();
    expect(face.drawn('FindParts(parts=run.findParts)', { ...scope, label: 'Different', chip: { kind: 'skill', label: 'Different' } })).toBeFalse();
  }
});
