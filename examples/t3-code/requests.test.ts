import { describe, test, expect } from 'bun:test';
import { T3Client } from './client';
import { snapshot, transcriptPresentation } from './presentation';
import { pendingRequests, requestPresentation, requestActivity, resolveAnswer } from './requests';
import { obj, str, type Obj } from './domain';
import type { Native, Files } from './protocol';

// A connected, writable client over a fake bridge that records dispatched commands.
function harness(projection: Obj) {
  const committed: Obj[] = [];
  let serial = 0, saved = '';
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input);
    if (request.op === 'ids') return { ok: true, generation: 1, value: Array.from({ length: Number(request.count) }, () => `id-${++serial}`) };
    if (request.op === 'request') { committed.push(obj(request.payload)); return { ok: true, generation: 1, value: {} }; }
    return { ok: true, generation: 1, value: {} };
  } };
  const storage: Files = { fs: { async mkdir() {}, async readFile() { if (!saved) throw new Error('missing'); return new TextEncoder().encode(saved).buffer; },
    async atomicWriteFile(_path, bytes) { saved = new TextDecoder().decode(bytes); } } };
  const client = new T3Client();
  Object.assign(client, { available: true, generation: 1, connection: 'connected', environmentId: 'env', projectId: 'p1', threadId: 't1',
    configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:read', 'orchestration:operate'],
    config: { environment: { capabilities: { serverResolvedCommandContext: true } } } });
  client.thread = { projection: { thread: { id: 't1' }, runtimeRequests: [], turnItems: [], ...projection }, sequence: 0, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null };
  const command = (op: string, id = '', value = '', n = 0) => client.command(op, id, value, n, native, storage);
  const projectionOf = () => client.thread!.projection;
  return { client, command, committed, projectionOf };
}
const question = (id: string, extra: Obj = {}): Obj => ({ id, header: id === 'scope' ? 'Scope' : 'Format', question: `Which ${id}?`,
  options: [{ label: 'Workspace', description: 'Use the temporary workspace.' }, { label: 'Session', description: 'Use this session only.' }], ...extra });
const input = (questions: Obj[], capability = 'live'): Obj => ({
  runtimeRequests: [{ id: 'r1', kind: 'user_input', status: 'pending', createdAt: '2026-10-03T00:00:00Z', responseCapability: { type: capability } }],
  turnItems: [{ id: 'i1', type: 'user_input_request', requestId: 'r1', questions }] });

describe('approvals', () => {
  test('shows the first pending approval with reference labels, default decisions and count', () => {
    const { client } = harness({
      runtimeRequests: [
        { id: 'b', kind: 'command', status: 'pending', createdAt: '2026-10-03T00:00:02Z', responseCapability: { type: 'live' } },
        { id: 'a', kind: 'file-change', status: 'pending', createdAt: '2026-10-03T00:00:01Z', responseCapability: { type: 'live' } }],
      turnItems: [{ id: 'x', type: 'approval_request', requestId: 'b', prompt: 'run ls' }] });
    const view = requestPresentation(client);
    expect(view.requestMode).toBe('approval');
    expect(view.approvals).toHaveLength(1);
    expect(view.approvals[0]).toMatchObject({ id: 'a', label: 'File change approval', detail: 'File change approval', count: '1/2', mono: true, canRespond: true });
    expect(view.approvals[0]!.options.map(option => option.label)).toEqual(['Decline', 'Approve']);
    expect(view.approvals[0]!.more.map(option => option.label)).toEqual(['Cancel', 'Always allow this session']);
    expect(view.questions).toHaveLength(0);
  });
  test('labels every request kind and keeps provider order, warnings and app names', () => {
    const kinds = ['command', 'file-read', 'file-change', 'permission', 'mcp-elicitation'];
    const labels = kinds.map(kind => requestPresentation(harness({
      runtimeRequests: [{ id: 'a', kind, status: 'pending', responseCapability: { type: 'live' } }],
      turnItems: [{ id: 'x', type: 'approval_request', requestId: 'a', appName: 'Figma', options: [{ decision: 'acceptAlways', label: 'Always', warning: 'Risky' }, { decision: 'accept', label: 'Yes' }, { decision: 'decline', label: 'No' }] }] }).client).approvals[0]!);
    expect(labels.map(label => label.label)).toEqual(['Command approval', 'File read approval', 'File change approval', 'App permission approval', 'App access approval']);
    expect(labels[4]).toMatchObject({ appName: 'Figma', mono: false });
    expect(labels[0]!.options.map(option => option.id)).toEqual(['accept', 'decline']);
    expect(labels[0]!.more).toEqual([{ id: 'acceptAlways', label: 'Always', warning: 'Risky', primary: false }]);
  });
  test('a provider process that is gone replaces the detail and disables decisions', async () => {
    const { client, command, committed } = harness({
      runtimeRequests: [{ id: 'a', kind: 'command', status: 'pending', responseCapability: { type: 'not_resumable' } }],
      turnItems: [{ id: 'x', type: 'approval_request', requestId: 'a', prompt: 'run ls' }] });
    expect(requestPresentation(client).approvals[0]).toMatchObject({ canRespond: false, detail: 'Provider process is gone — interrupt or restart the run to respond.' });
    expect((await command('approval', 'a', 'accept')).message).toBe('This approval is no longer live.');
    expect(committed).toHaveLength(0);
  });
  test('approve dispatches only advertised decisions and blocks composer sends', async () => {
    const { command, committed } = harness({
      runtimeRequests: [{ id: 'a', kind: 'command', status: 'pending', responseCapability: { type: 'live' } }],
      turnItems: [{ id: 'x', type: 'approval_request', requestId: 'a', prompt: 'run ls' }] });
    expect((await command('send', '', 'hello')).message).toBe('Resolve this approval request to continue.');
    expect((await command('approval', 'a', 'acceptAlways')).message).toContain('approval options offered');
    await command('approval', 'a', 'acceptForSession');
    expect(committed).toEqual([{ type: 'runtime-request.respond', threadId: 't1', requestId: 'a', decision: 'acceptForSession', commandId: 'id-1' }]);
  });
  test('a live request stays pressable while reconnecting and the press names the reconnect', async () => {
    const { client, command, committed } = harness({
      runtimeRequests: [{ id: 'a', kind: 'command', status: 'pending', responseCapability: { type: 'live' } }],
      turnItems: [{ id: 'x', type: 'approval_request', requestId: 'a', prompt: 'run ls' }] });
    client.connection = 'reconnecting';
    expect(requestPresentation(client).approvals[0]).toMatchObject({ live: true, canRespond: false });
    expect((await command('approval', 'a', 'accept')).message).toBe('Reconnect before making changes.');
    expect(committed).toHaveLength(0);
    const gone = harness(input([question('scope')], 'not_resumable')).client;
    expect(requestPresentation(gone).questions[0]).toMatchObject({ live: false, canRespond: false });
  });
});

describe('questions', () => {
  test('one active question with shortcuts, position and composer placeholder state', () => {
    const { client } = harness(input([question('scope'), question('format')]));
    const view = snapshot(client);
    expect(view.requestMode).toBe('question');
    expect(view.questions).toHaveLength(1);
    expect(view.questions[0]).toMatchObject({ id: 'r1::scope', header: 'Scope', position: '1/2', last: false, action: 'Next', canAdvance: false, complete: false });
    expect(view.questions[0]!.options.map(option => [option.id, option.shortcut, option.selected])).toEqual([['Workspace', '1', false], ['Session', '2', false]]);
    expect(view.requestKey).toBe('r1::scope');
  });
  test('single choice advances; the last single choice submits every answer', async () => {
    const { client, command, committed } = harness(input([question('scope'), question('format')]));
    await command('pick-option', 'r1::scope', 'Session');
    expect(committed).toHaveLength(0);
    expect(requestPresentation(client).questions[0]).toMatchObject({ id: 'r1::format', position: '2/2', index: 1, action: 'Submit' });
    await command('previous-question');
    expect(requestPresentation(client).questions[0]).toMatchObject({ id: 'r1::scope', index: 0 });
    expect(requestPresentation(client).questions[0]!.options[1]).toMatchObject({ selected: true });
    await command('advance-question', 'r1');
    await command('pick-option', 'r1::format', 'Workspace');
    expect(committed[0]).toMatchObject({ type: 'runtime-request.respond', requestId: 'r1', answers: { scope: 'Session', format: 'Workspace' } });
  });
  test('the composer types the custom answer, which outranks a selection and is submitted on Return', async () => {
    const { client, command, committed } = harness(input([question('scope')]));
    await command('choice', 'r1::scope', 'Workspace');
    await command('draft', '', 'Use YAML');
    const view = snapshot(client);
    expect(view.draft).toBe('Use YAML');
    expect(view.questions[0]!.options.every(option => !option.selected)).toBe(true);
    expect(client.local.drafts['env:t1']).toBeUndefined();
    await command('send', '', 'Use YAML');
    expect(committed[0]).toMatchObject({ answers: { scope: 'Use YAML' } });
  });
  test('choosing an option returns displaced custom text to the thread draft', async () => {
    const { client, command } = harness(input([question('scope')]));
    client.local.drafts['env:t1'] = 'Earlier';
    await command('draft', '', 'typed');
    await command('choice', 'r1::scope', 'Session');
    expect(client.local.drafts['env:t1']).toBe('Earlier\n\ntyped');
    expect(resolveAnswer({ id: 'scope', header: '', question: '', multiSelect: false, allowCustom: true, options: [{ label: 'Session', value: 'Session', description: '' }] }, client.answers['r1::scope'])).toBe('Session');
  });
  test('validation names the unanswered question and multiple selection toggles exact values', async () => {
    const { command, committed } = harness(input([question('scope', { multiSelect: true, allowCustomAnswer: false, options: [{ label: 'One', value: ' exact ' }, { label: 'Two', value: 'two' }] })]));
    expect((await command('advance-question', 'r1')).message).toBe('Answer “Scope” first.');
    expect((await command('draft', '', 'free text')).message).toBe('Choose one of the offered options.');
    await command('choice', 'r1::scope', ' exact ');
    await command('choice', 'r1::scope', 'two');
    await command('choice', 'r1::scope', 'two');
    await command('choice', 'r1::scope', 'two');
    await command('advance-question', 'r1');
    expect(committed[0]).toMatchObject({ answers: { scope: [' exact ', 'two'] } });
  });
  test('dismissible questions dismiss through the server; required ones refuse', async () => {
    const message = harness(input([question('scope')], 'message'));
    expect(requestPresentation(message.client).questions[0]!.dismissible).toBe(true);
    await message.command('dismiss-question', 'r1');
    expect(message.committed[0]).toEqual({ type: 'thread.user-input.dismiss', threadId: 't1', requestId: 'r1', commandId: 'id-1' });
    const live = harness(input([question('scope')]));
    expect((await live.command('dismiss-question', 'r1')).message).toBe('This question needs an answer.');
  });
  test('approvals take precedence over questions, which return when approvals resolve', () => {
    const { client, projectionOf } = harness(input([question('scope')]));
    projectionOf().runtimeRequests = [{ id: 'a', kind: 'command', status: 'pending', createdAt: '2026-10-03T00:00:05Z', responseCapability: { type: 'live' } }, ...(projectionOf().runtimeRequests as Obj[])];
    expect(requestPresentation(client).requestMode).toBe('approval');
    expect(pendingRequests(projectionOf()).inputs).toHaveLength(1);
  });
});

describe('request history rows', () => {
  test('approval rows lead with their title and answered questions with their questions and answers', () => {
    expect(requestActivity({ type: 'approval_request', title: 'Approval', prompt: 'Allow the\nfixture?' }, {})).toEqual({ label: 'Allow the fixture?', body: 'Allow the\nfixture?' });
    expect(requestActivity({ type: 'approval_request', requestKind: 'command' }, {}).label).toBe('Approval requested');
    expect(requestActivity({ type: 'user_input_request', requestId: 'r1', questions: [{ id: 'scope', question: 'Which scope?' }] }, {})).toMatchObject({ label: 'Input requested' });
    const projection = { runtimeRequests: [{ id: 'r1', status: 'resolved', answers: { scope: { answers: ['Session'] }, format: 'Use YAML' } }] };
    const row = requestActivity({ type: 'user_input_request', requestId: 'r1', questions: [{ id: 'scope', question: 'Which scope?' }, { id: 'format', question: 'Which format?' }] }, projection);
    expect(row.label).toBe('Which scope? · Which format?  Session · Use YAML');
    expect(str(row.body)).toBe('Which scope?\n  Session\n\nWhich format?\n  Use YAML');
  });
});

describe('connection and thread errors', () => {
  // The offline banner moved to the composer's notice stack: server-update-notices.test.ts.
  test('a failed root run or provider session error shows until dismissed for that message', async () => {
    const { client, command } = harness({ thread: { id: 't1', providerInstanceId: 'codex' },
      runs: [{ id: 'run1', ordinal: 1, status: 'failed', rootNodeId: 'n1' }],
      turnItems: [{ id: 'e1', type: 'error', status: 'failed', runId: 'run1', nodeId: 'n1', updatedAt: '2026-10-03T00:00:00Z', failure: { message: 'Intentional fixture failure.' } }] });
    expect(snapshot(client)).toMatchObject({ error: 'Intentional fixture failure.', clientError: '' });
    await command('dismiss-error');
    expect(snapshot(client).error).toBe('');
    client.thread!.projection.providerSessions = [{ providerInstanceId: 'codex', lastError: 'provider process exited' }];
    expect(snapshot(client).error).toBe('provider process exited');
    client.error = 'Reconnect before making changes.';
    expect(snapshot(client)).toMatchObject({ error: 'Reconnect before making changes.', clientError: 'Reconnect before making changes.' });
  });
});

describe('request rows in the work log', () => {
  test("a pending approval is the running turn's live activity, after its earlier tools", () => {
    const row = (id: string, item: Obj) => ({ sourceThreadId: 't1', sourceItemId: id, ordinal: 0, item: { id, runId: 'r1', status: 'completed', startedAt: '2026-10-03T00:00:00Z', ...item } });
    const { client } = harness({ runs: [{ id: 'r1', status: 'running', startedAt: '2026-10-03T00:00:00Z' }],
      runtimeRequests: [{ id: 'a', kind: 'command', status: 'pending', responseCapability: { type: 'live' } }] });
    const items = [row('u', { type: 'user_message', text: 'fixture approval' }), row('c', { type: 'command_execution', input: 'ls', output: '' }),
      row('q', { type: 'approval_request', requestId: 'a', prompt: 'Allow the fixture?' })];
    Object.assign(client.thread!.projection, { turnItems: items.map(entry => entry.item), visibleTurnItems: items });
    const live = transcriptPresentation(client).filter(message => message.kind === 'live');
    expect(live).toHaveLength(1);
    expect(live[0]).toMatchObject({ title: 'Allow the fixture?', icon: 'message-circle', live: true });
  });
});
