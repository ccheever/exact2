import { queuedEditState, type MobileQueuedEditSession } from './queued-edit-state';
import { describe, expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { arr, type Obj } from './shared/domain';
import { chatLocal } from './shared/timeline-presentation';
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
