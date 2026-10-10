import { queuedEditState, type MobileQueuedEditSession } from './queued-edit-state';
import { describe, expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { arr, type Obj } from './shared/domain';
import { chatLocal, transcriptRows } from './shared/timeline-presentation';
import type { Native } from './shared/protocol';
import { mobileThread, mobileThreadRows, mobileThreadComposer, mobileThreadBlocks } from './thread';
import { mobileCodeTokens } from './thread-highlight';
import { mobileModelSelectionReady, mobileModelSelectionUnavailable } from './model-availability';

const now = Date.parse('2026-10-07T12:00:00Z');
const at = (seconds: number) => new Date(now + seconds * 1000).toISOString();
const item = (id: string, type: string, extra: Obj = {}): Obj => ({ id, type, threadId: 't1', runId: 'r1', nodeId: null,
  ordinal: 1, status: 'completed', startedAt: at(0), updatedAt: at(2), ...extra });
const user = (text: string) => item('user', 'user_message', { text, messageId: 'mu', inputIntent: 'turn_start', attachments: [], createdBy: 'user' });
const answer = (text: string) => item('answer', 'assistant_message', { text, messageId: 'ma', streaming: false });
function fixture(items: Obj[] = [], projection: Obj = {}): T3Client {
  const client = new T3Client();
  client.environmentId = 'env'; client.threadId = 't1'; client.projectId = 'p1';
  client.connection = 'connected'; client.configLive = true; client.shellLive = true; client.threadLive = true;
  client.scopes = ['orchestration:operate']; client.providerId = 'provider'; client.modelId = 'model';
  client.config = { environment: { capabilities: { serverResolvedCommandContext: true } },
    providers: [{ instanceId: 'provider', driver: 'codex', enabled: true, installed: true, models: [{ slug: 'model', name: 'Model' }] }] };
  client.shell.projects = [{ id: 'p1', workspaceRoot: '/repo' }];
  client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: 1, projection: {
    thread: { id: 't1', projectId: 'p1', title: 'Actual title' }, runs: [{ id: 'r1', status: 'completed', startedAt: at(0), completedAt: at(10) }],
    attempts: [], nodes: [], checkpoints: [], turnItems: items, runtimeRequests: [], ...projection,
    visibleTurnItems: items.map((value, position) => ({ position, visibility: 'local', sourceThreadId: 't1', sourceItemId: value.id, item: value })) } };
  return client;
}

describe('mobile V2 transcript presentation', () => {
  const commentary = (id: string, text: string, seconds: number, extra: Obj = {}) => item(id, 'assistant_message',
    { text, messageId: `message-${id}`, streaming: false, startedAt: at(seconds), updatedAt: at(seconds), ...extra });
  const command = (id: string, seconds: number, extra: Obj = {}) => item(id, 'command_execution',
    { input: id, output: '', exitCode: 0, startedAt: at(seconds), updatedAt: at(seconds), ...extra });

  test('keeps initial commentary before Worked while intermediate prose and tools share the existing fold', async () => {
    const client = fixture([user('Inspect'), commentary('first', 'I will inspect it.', 1), command('pwd', 2),
      commentary('middle', 'Checking status next.', 3), command('status', 4), answer('Done')]);
    const before = structuredClone(client.projection), key = (id: string) => JSON.stringify(['t1', id]);
    const collapsed = mobileThreadRows(client, now);
    expect(collapsed.map(row => row.id)).toEqual([key('user'), key('first'), 'turn-fold:r1', key('answer')]);
    expect(collapsed[1]).toMatchObject({ body: 'I will inspect it.', showMeta: false });
    expect(collapsed[2]).toMatchObject({ kind: 'work', expanded: false, toggleId: 'r1' });
    expect(collapsed[3]).toMatchObject({ body: 'Done', showMeta: true });
    expect(transcriptRows(client).some(row => row.id === key('first'))).toBe(false);
    await chatLocal(client, { available: true } as Native, 'fold', 'r1', '');
    const expanded = mobileThreadRows(client, now);
    expect(expanded.map(row => row.kind)).toEqual(['user', 'assistant', 'work', 'entry', 'assistant', 'entry', 'assistant']);
    expect(expanded.filter(row => row.id === key('first'))).toHaveLength(1);
    expect(expanded.find(row => row.id === key('middle'))?.body).toBe('Checking status next.');
    expect(expanded[2]).toMatchObject({ toggleId: 'r1', expanded: true });
    await chatLocal(client, { available: true } as Native, 'fold', 'r1', '');
    expect(mobileThreadRows(client, now)).toEqual(collapsed);
    expect(client.projection).toEqual(before);
  });

  test('two assistant messages alone remain visible without a spurious Worked fold', () => {
    const client = fixture([user('Inspect'), commentary('first', 'Starting.', 1), answer('Done')]);
    expect(mobileThreadRows(client, now).map(row => row.body)).toEqual(['Inspect', 'Starting.', 'Done']);
    expect(mobileThreadRows(client, now).some(row => row.kind === 'work')).toBe(false);
  });

  test('fold stays at the first hidden tool when work precedes the initial assistant message', () => {
    const client = fixture([user('Inspect'), command('pwd', 1), commentary('first', 'Checked the directory.', 2), answer('Done')]);
    expect(mobileThreadRows(client, now).map(row => [row.kind, row.body])).toEqual([
      ['user', 'Inspect'], ['work', ''], ['assistant', 'Checked the directory.'], ['assistant', 'Done'],
    ]);
    expect(transcriptRows(client, true).find(row => row.kind === 'work')?.createdAt).toBe(at(1));
  });

  test('middle assistant prose can fold even when a turn has no tools', () => {
    const client = fixture([user('Explain'), commentary('first', 'Starting.', 1), commentary('middle', 'More detail.', 2), answer('Done')]);
    expect(mobileThreadRows(client, now).map(row => [row.kind, row.body])).toEqual([
      ['user', 'Explain'], ['assistant', 'Starting.'], ['work', ''], ['assistant', 'Done'],
    ]);
  });

  test('runless prompts retain their own initial commentary and independent fold ownership', async () => {
    const prompt = (id: string, seconds: number) => item(id, 'user_message',
      { text: id, runId: null, messageId: id, inputIntent: 'turn_start', attachments: [], startedAt: at(seconds), updatedAt: at(seconds) });
    const items = [prompt('one', 0), commentary('first-one', 'First turn.', 1, { runId: null }), command('pwd', 2, { runId: null }),
      commentary('final-one', 'Done one.', 3, { runId: null }), prompt('two', 4), commentary('first-two', 'Second turn.', 5, { runId: null }),
      command('status', 6, { runId: null }), commentary('final-two', 'Done two.', 7, { runId: null })];
    const client = fixture(items, { runs: [] }), collapsed = mobileThreadRows(client, now);
    expect(collapsed.map(row => row.kind)).toEqual(['user', 'assistant', 'work', 'assistant', 'user', 'assistant', 'work', 'assistant']);
    const folds = collapsed.filter(row => row.kind === 'work');
    expect(folds[0]!.toggleId).not.toBe(folds[1]!.toggleId);
    await chatLocal(client, { available: true } as Native, 'fold', folds[0]!.toggleId, '');
    expect(mobileThreadRows(client, now).filter(row => row.kind === 'work').map(row => row.expanded)).toEqual([true, false]);
    expect(mobileThreadRows(client, now).filter(row => row.body.endsWith('turn.'))).toHaveLength(2);
  });

  for (const state of ['running', 'streaming', 'failed', 'interrupted']) {
    test(`initial commentary remains visible without a completed-work fold during ${state}`, () => {
      const rows = [user('Inspect'), commentary('first', 'Starting.', 1), command('pwd', 2),
        commentary('final', 'Done', 3, { streaming: state === 'streaming' })];
      if (state === 'interrupted') rows.push(item('interrupt', 'run_interrupt_result', { interrupted: true }));
      const client = fixture(rows, { runs: [{ id: 'r1', status: state === 'running' ? 'running' : state === 'failed' ? 'failed' : 'completed',
        startedAt: at(0), completedAt: state === 'running' ? null : at(4) }] });
      const shown = mobileThreadRows(client, now);
      expect(shown.find(row => row.body === 'Starting.')).toBeDefined();
      expect(shown.some(row => row.kind === 'work')).toBe(false);
    });
  }

  for (const files of [[], [{ path: 'src/app.ts', additions: 2, deletions: 1 }]]) {
    test(`omits ${files.length ? 'populated' : 'empty'} checkpoints before row boundaries and retains reconnect projection`, () => {
      const checkpoint = (id: string) => item(id, 'checkpoint', { checkpointId: id, scopeId: 'workspace', files });
      const client = fixture([checkpoint('before'), user('Hello'), answer('Done'), checkpoint('after')]);
      const before = structuredClone(client.projection);
      const shared = transcriptRows(client);
      expect(shared.filter(row => row.kind === 'checkpoint')).toHaveLength(2);
      const expected = [
        { id: JSON.stringify(['t1', 'user']), kind: 'user', first: true, last: false, showMeta: true },
        { id: JSON.stringify(['t1', 'answer']), kind: 'assistant', first: false, last: true, showMeta: true },
      ];
      expect(mobileThreadRows(client, now)).toMatchObject(expected);
      client.connection = 'reconnecting';
      expect(mobileThread(now, false, client)).toMatchObject({ loaded: true, loading: false, rows: expected });
      expect(client.projection).toEqual(before);
      expect(transcriptRows(client)).toEqual(shared);
    });
  }

  test('retains genuine file-change work and disclosure while omitting its checkpoint', async () => {
    const files = [{ path: 'src/app.ts', additions: 2, deletions: 1 }];
    const items = [user('Update the app'), item('edit', 'file_change', {
      fileName: '/repo/src/app.ts', changes: [{ operation: 'update', path: '/repo/src/app.ts' }],
    }), answer('Updated')];
    const client = fixture([...items, item('checkpoint', 'checkpoint', { checkpointId: 'cp1', scopeId: 'workspace', files })]);
    const withoutCheckpoint = fixture(items);
    expect(mobileThreadRows(client, now)).toEqual(mobileThreadRows(withoutCheckpoint, now));
    expect(mobileThreadRows(client, now).map(row => row.kind)).toEqual(['user', 'work', 'assistant']);
    expect(mobileThreadRows(client, now)[1]).toMatchObject({ toggleOp: 'chatlocal:fold', toggleId: 'r1', expanded: false });
    await chatLocal(client, { available: true } as Native, 'fold', 'r1', '');
    await chatLocal(withoutCheckpoint, { available: true } as Native, 'fold', 'r1', '');
    const rows = mobileThreadRows(client, now);
    expect(rows).toEqual(mobileThreadRows(withoutCheckpoint, now));
    const edit = rows.flatMap(row => row.activities).find(activity => activity.id === JSON.stringify(['t1', 'edit']));
    expect(edit?.body).toContain('src/app.ts');
    expect(rows.some(row => row.kind === 'checkpoint')).toBe(false);
    expect(rows.at(-1)).toMatchObject({ kind: 'assistant', last: true, showMeta: true });
  });

  test('keeps scoped shared ids, hides mobile blank answers, and retains reconnect cache', () => {
    const client = fixture([user('Hello'), answer('')]);
    expect(mobileThreadRows(client, now).map(row => row.id)).toEqual([JSON.stringify(['t1', 'user'])]);
    client.connection = 'reconnecting';
    expect(mobileThread(now, false, client)).toMatchObject({ loaded: true, loading: false, title: 'Actual title', rows: [{ first: true, last: true }] });
    client.thread = null; client.connection = 'disconnected';
    expect(mobileThread(now, false, client)).toMatchObject({ loaded: false, emptyTitle: 'Messages not cached' });
  });
  test('disclosure mutates shared timeline state and projects real tool details', async () => {
    const client = fixture([user('Run tests'), item('command', 'command_execution', { input: 'bun test', output: 'passed', exitCode: 0 }), answer('Done')]);
    const folded = mobileThreadRows(client, now);
    expect(folded.map(row => row.kind)).toEqual(['user', 'work', 'assistant']);
    expect(folded[1]).toMatchObject({ toggleOp: 'chatlocal:fold', toggleId: 'r1', expanded: false });
    await chatLocal(client, { available: true } as Native, 'fold', 'r1', '');
    expect(mobileThreadRows(client, now).some(row => row.activities.some(activity => activity.body === 'bun test'))).toBe(true);
  });
  test('duration labels use supplied clock and streaming answers have no metadata', () => {
    const client = fixture([user('Go'), answer('Partial')], { runs: [{ id: 'r1', status: 'running', startedAt: at(0) }] });
    arr(client.projection.visibleTurnItems)[1]!.item = { ...answer('Partial'), streaming: true, status: 'running' };
    const rows = mobileThreadRows(client, now + 65_000);
    expect(rows.find(row => row.kind === 'working')?.title).toBe('Working for 1m 5s');
    expect(rows.find(row => row.kind === 'assistant')?.showMeta).toBe(false);
  });
  test('mobile approval defaults and explicit server order retain shared permission refusal', () => {
    const approval = item('approval', 'approval_request', { requestId: 'request', requestKind: 'command', prompt: 'Run command?' });
    const client = fixture([approval], { runtimeRequests: [{ id: 'request', status: 'pending', kind: 'command', createdAt: at(0), responseCapability: { type: 'live' } }] });
    expect(mobileThread(now, false, client).approvals[0]).toMatchObject({ title: 'command', disabled: false,
      options: [{ id: 'accept', label: 'Allow once' }, { id: 'acceptForSession', label: 'Allow session' }, { id: 'decline', label: 'Decline' }] });
    approval.options = [{ decision: 'decline', label: 'No' }, { decision: 'accept', label: 'Yes', warning: 'Runs a command' }];
    client.scopes = [];
    expect(mobileThread(now, false, client).approvals[0]).toMatchObject({ disabled: true, options: [{ id: 'decline', label: 'No' }, { id: 'accept', label: 'Yes' }] });
  });
  test('composer admission follows actual synchronization, model and request permissions', () => {
    const client = fixture(); client.local.drafts[client.draftKey] = 'Inspect source';
    expect(mobileThreadComposer(client)).toMatchObject({ canSend: true, sendLabel: 'Send', modelUnavailable: false });
    client.connection = 'reconnecting';
    expect(mobileThreadComposer(client)).toMatchObject({ canSend: false, showReadOnlyNotice: false, modelUnavailable: false });
    client.connection = 'connected'; client.scopes = [];
    expect(mobileThreadComposer(client)).toMatchObject({ canSend: false, showReadOnlyNotice: true });
    client.scopes = ['orchestration:operate']; client.modelId = 'missing';
    arr(client.config.providers)[0]!.driver = 'antigravity';
    expect(mobileThreadComposer(client)).toMatchObject({ canSend: false, modelUnavailable: true });
  });
  test('fenced blocks retain literal content and unknown languages remain readable', () => {
    const blocks = mobileThreadBlocks('Before\n\n```typescript\nconst a = 1;\n```\nAfter', false);
    expect(blocks.map(block => block.kind)).toEqual(['markdown', 'code', 'markdown']);
    expect(blocks[1]?.tokens.map(token => token.text).join('')).toBe('const a = 1;');
    expect(mobileCodeTokens('<unknown>', 'unregistered-language', true).map(token => token.text).join('')).toBe('<unknown>');
  });
});

// Golden tokens captured from pinned @shikijs/{core,engine-javascript,langs,themes}@4.2.0.
// Mobile uses github-light-default/github-dark-default; expected color case is normalized.
// Captured under Node24.13.1; Bun1.2.22 produces invalid comment tokens and is not this repo's pinned runtime.
const shikiFixtures = [{"language":"typescript","code":"const answer: number = 42; // hello","dark":false,"expected":[{"text":"const","color":"#cf222e","weight":400,"slant":"normal"},{"text":" ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"answer","color":"#0550ae","weight":400,"slant":"normal"},{"text":":","color":"#cf222e","weight":400,"slant":"normal"},{"text":" ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"number","color":"#0550ae","weight":400,"slant":"normal"},{"text":" ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"=","color":"#cf222e","weight":400,"slant":"normal"},{"text":" ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"42","color":"#0550ae","weight":400,"slant":"normal"},{"text":"; ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"// hello","color":"#6e7781","weight":400,"slant":"normal"}]},{"language":"typescript","code":"const answer: number = 42; // hello","dark":true,"expected":[{"text":"const","color":"#ff7b72","weight":400,"slant":"normal"},{"text":" ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"answer","color":"#79c0ff","weight":400,"slant":"normal"},{"text":":","color":"#ff7b72","weight":400,"slant":"normal"},{"text":" ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"number","color":"#79c0ff","weight":400,"slant":"normal"},{"text":" ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"=","color":"#ff7b72","weight":400,"slant":"normal"},{"text":" ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"42","color":"#79c0ff","weight":400,"slant":"normal"},{"text":"; ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"// hello","color":"#8b949e","weight":400,"slant":"normal"}]},{"language":"json","code":"{\"enabled\": true, \"name\": \"T3\"}","dark":false,"expected":[{"text":"{","color":"#1f2328","weight":400,"slant":"normal"},{"text":"\"enabled\"","color":"#116329","weight":400,"slant":"normal"},{"text":": ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"true","color":"#0550ae","weight":400,"slant":"normal"},{"text":", ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"\"name\"","color":"#116329","weight":400,"slant":"normal"},{"text":": ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"\"T3\"","color":"#0a3069","weight":400,"slant":"normal"},{"text":"}","color":"#1f2328","weight":400,"slant":"normal"}]},{"language":"json","code":"{\"enabled\": true, \"name\": \"T3\"}","dark":true,"expected":[{"text":"{","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"\"enabled\"","color":"#7ee787","weight":400,"slant":"normal"},{"text":": ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"true","color":"#79c0ff","weight":400,"slant":"normal"},{"text":", ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"\"name\"","color":"#7ee787","weight":400,"slant":"normal"},{"text":": ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"\"T3\"","color":"#a5d6ff","weight":400,"slant":"normal"},{"text":"}","color":"#e6edf3","weight":400,"slant":"normal"}]},{"language":"bash","code":"echo \"$HOME\" # hello","dark":false,"expected":[{"text":"echo","color":"#0550ae","weight":400,"slant":"normal"},{"text":" ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"\"","color":"#0a3069","weight":400,"slant":"normal"},{"text":"$HOME","color":"#1f2328","weight":400,"slant":"normal"},{"text":"\"","color":"#0a3069","weight":400,"slant":"normal"},{"text":" ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"# hello","color":"#6e7781","weight":400,"slant":"normal"}]},{"language":"bash","code":"echo \"$HOME\" # hello","dark":true,"expected":[{"text":"echo","color":"#79c0ff","weight":400,"slant":"normal"},{"text":" ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"\"","color":"#a5d6ff","weight":400,"slant":"normal"},{"text":"$HOME","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"\"","color":"#a5d6ff","weight":400,"slant":"normal"},{"text":" ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"# hello","color":"#8b949e","weight":400,"slant":"normal"}]},{"language":"yaml","code":"name: mobile\nenabled: true","dark":false,"expected":[{"text":"name","color":"#116329","weight":400,"slant":"normal"},{"text":": ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"mobile","color":"#0a3069","weight":400,"slant":"normal"},{"text":"\n","color":"#1f2328","weight":400,"slant":"normal"},{"text":"enabled","color":"#116329","weight":400,"slant":"normal"},{"text":": ","color":"#1f2328","weight":400,"slant":"normal"},{"text":"true","color":"#0550ae","weight":400,"slant":"normal"}]},{"language":"yaml","code":"name: mobile\nenabled: true","dark":true,"expected":[{"text":"name","color":"#7ee787","weight":400,"slant":"normal"},{"text":": ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"mobile","color":"#a5d6ff","weight":400,"slant":"normal"},{"text":"\n","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"enabled","color":"#7ee787","weight":400,"slant":"normal"},{"text":": ","color":"#e6edf3","weight":400,"slant":"normal"},{"text":"true","color":"#79c0ff","weight":400,"slant":"normal"}]}];
for (const fixture of shikiFixtures) test(`mobile Shiki ${fixture.language} ${fixture.dark ? 'dark' : 'light'} oracle`, () => {
  const tokens: { text: string; color: string; weight: number; slant: string }[] = [];
  for (const token of mobileCodeTokens(fixture.code, fixture.language, fixture.dark)) {
    const style = { text: token.text, color: token.color.toLowerCase(), weight: token.weight, slant: token.slant }, last = tokens.at(-1);
    if (last && last.color === style.color && last.weight === style.weight && last.slant === style.slant) last.text += style.text;
    else tokens.push(style);
  }
  expect(tokens).toEqual(fixture.expected);
});

test('queued composer projects dedicated text and never offers ordinary Stop', () => {
  const client = fixture([], { runs: [{ id: 'active', status: 'running' }] });
  client.local.drafts[client.draftKey] = 'ordinary draft';
  const edit: MobileQueuedEditSession = { owner: 'edit', session: 'unique', draftKey: 'env:t1~queued-edit~queued',
    origin: client.origin, environmentId: 'env', threadId: 't1', projectId: 'p1', generation: client.generation,
    revision: 1, runId: 'queued', messageId: 'queued-message', text: 'replacement', attachments: [], existingAttachments: [], saving: false };
  queuedEditState(client).sessions.set(edit.owner, edit); queuedEditState(client).active.set('env:t1', edit.owner);
  expect(mobileThreadComposer(client)).toMatchObject({ editing: true, draft: 'replacement', showStop: false, canSend: true, sendSymbol: 'checkmark', canCancel: true });
  queuedEditState(client).sessions.set(edit.owner, { ...edit, saving: true });
  expect(mobileThreadComposer(client)).toMatchObject({ saving: true, canSend: false, canCancel: false, blockedReason: 'Saving…' });
  queuedEditState(client).sessions.set(edit.owner, edit); client.modelId = 'unavailable';
  arr(client.config.providers)[0]!.driver = 'antigravity';
  expect(mobileThreadComposer(client)).toMatchObject({ canSend: false, modelUnavailable: true });
  expect(client.draft).toBe('ordinary draft');
});


test('mobile unavailable notice follows the pinned Antigravity driver, including configured instances absent from catalog', () => {
  const selection = { instanceId: 'custom-instance', model: 'retained-model' };
  const provider = { instanceId: 'custom-instance', driver: 'antigravity', enabled: true, installed: true, auth: { status: 'authenticated' }, models: [{ slug: 'retained-model' }] };
  expect(mobileModelSelectionUnavailable(null, selection)).toBe(false);
  expect(mobileModelSelectionUnavailable({}, null)).toBe(false);
  expect(mobileModelSelectionUnavailable({ providers: [provider] }, selection)).toBe(false);
  expect(mobileModelSelectionReady({ providers: [provider] }, selection)).toBe(true);
  for (const patch of [{ enabled: false }, { installed: false }, { auth: { status: 'unauthenticated' } }, { availability: 'unavailable' }, { models: [] }]) {
    expect(mobileModelSelectionUnavailable({ providers: [{ ...provider, ...patch }] }, selection)).toBe(true);
    expect(mobileModelSelectionReady({ providers: [{ ...provider, ...patch }] }, selection)).toBe(false);
  }
  expect(mobileModelSelectionUnavailable({ providers: [], settings: { providerInstances: { 'custom-instance': { driver: 'antigravity' } } } }, selection)).toBe(true);
  expect(mobileModelSelectionUnavailable({ providers: [], settings: { providerInstances: { 'custom-instance': { driver: 'codex' } } } }, selection)).toBe(false);
  expect(mobileModelSelectionReady({ providers: [provider] }, { ...selection, model: '' })).toBe(false);
  // Source's notice does not test status:error. The transport still refuses it.
  expect(mobileModelSelectionUnavailable({ providers: [{ ...provider, status: 'error' }] }, selection)).toBe(false);
  expect(mobileModelSelectionReady({ providers: [{ ...provider, status: 'error' }] }, selection)).toBe(false);
});
