import { describe, test, expect } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { snapshot, transcriptPresentation } from '../../src/client/presentation';
import { noteComposerTyping, releaseHeldRequests } from '../../src/composer/composer-typing-guard';
import { pendingRequests, requestPresentation, requestActivity, resolveAnswer } from '../../src/composer/requests';
import { timelinePendingApprovals } from '../../src/timeline/timeline-approvals';
import { arr, obj, str, type Obj } from '../../src/client/domain';
import type { Native, Files } from '../../src/client/protocol';
import { readFileSync } from 'node:fs';
import { appPath } from '../support/app-files';
import { contractComponent, value } from '../support/contract-action-fixture';

/** The `disabled=(…)` of the requests.contract view line holding `marker`, read over `env`. */
function viewDisabled(marker: string, env: Record<string, unknown>): boolean {
  const line = readFileSync(appPath('src/composer/requests.contract'), 'utf8').split('\n').find(line => line.includes(marker))!;
  const start = line.indexOf('disabled=(') + 'disabled='.length;
  let depth = 0, end = start;
  do { if (line[end] === '(') depth++; else if (line[end] === ')') depth--; end++; } while (depth > 0);
  return Boolean(value(line.slice(start, end), env));
}

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

// Approvals answer on their timeline row (T3 Code v0.1.5, #26255): the composer has no approval banner; while one
// waits, the composer is the draft a send redirects with.
describe('approvals', () => {
  test('the composer shows no approval; the timeline row has every pending one', () => {
    const { client } = harness({
      runtimeRequests: [
        { id: 'b', kind: 'command', status: 'pending', createdAt: '2026-10-03T00:00:02Z', responseCapability: { type: 'live' } },
        { id: 'a', kind: 'file-change', status: 'pending', createdAt: '2026-10-03T00:00:01Z', responseCapability: { type: 'live' } }],
      turnItems: [{ id: 'x', type: 'approval_request', requestId: 'b', prompt: 'run ls' }] });
    const view = requestPresentation(client);
    expect(view.requestMode).toBe('approval');
    expect('approvals' in view).toBe(false);
    expect(view.questions).toHaveLength(0);
    expect('approvals' in snapshot(client)).toBe(false);
    expect([...timelinePendingApprovals(client).keys()]).toEqual(['a', 'b']);
  });
  test('the placeholder asks for feedback when the agent reads it, else offers the redirect; the composer carries no reason', () => {
    const { client } = harness({
      runtimeRequests: [{ id: 'a', kind: 'command', status: 'pending', responseCapability: { type: 'live', supportsFeedback: true } }],
      turnItems: [{ id: 'x', type: 'approval_request', requestId: 'a', prompt: 'rm -rf build', reason: 'Clean the build folder', fullAccessOverride: 'ask_rule' }] });
    const composer = snapshot(client).composer;
    expect(composer.placeholder).toBe('Send feedback to decline this request');
    expect('approvalReason' in composer || 'approvalWhy' in composer).toBe(false);
    expect(timelinePendingApprovals(client).get('a')).toMatchObject({ reason: 'Clean the build folder', whyAsked: expect.stringContaining('an ask rule in the agent') });
    arr(client.projection.runtimeRequests)[0]!.responseCapability = { type: 'live' };
    expect(snapshot(client).composer.placeholder).toBe('Send a message to decline and redirect the agent');
  });
  test('a provider process that is gone takes no decision', async () => {
    const { command, committed } = harness({
      runtimeRequests: [{ id: 'a', kind: 'command', status: 'pending', responseCapability: { type: 'not_resumable' } }],
      turnItems: [{ id: 'x', type: 'approval_request', requestId: 'a', prompt: 'run ls' }] });
    expect((await command('approval', 'a', 'accept')).message).toBe('This approval is no longer live.');
    expect(committed).toHaveLength(0);
  });
  test('approve dispatches only advertised decisions', async () => {
    const { command, committed } = harness({
      runtimeRequests: [{ id: 'a', kind: 'command', status: 'pending', responseCapability: { type: 'live' } }],
      turnItems: [{ id: 'x', type: 'approval_request', requestId: 'a', prompt: 'run ls' }] });
    expect((await command('approval', 'a', 'acceptAlways')).message).toContain('approval options offered');
    await command('approval', 'a', 'acceptForSession');
    expect(committed).toEqual([{ type: 'runtime-request.respond', threadId: 't1', requestId: 'a', decision: 'acceptForSession', commandId: 'id-1' }]);
  });
  test('a live request\'s press while reconnecting names the reconnect', async () => {
    const { client, command, committed } = harness({
      runtimeRequests: [{ id: 'a', kind: 'command', status: 'pending', responseCapability: { type: 'live' } }],
      turnItems: [{ id: 'x', type: 'approval_request', requestId: 'a', prompt: 'run ls' }] });
    client.connection = 'reconnecting';
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
  test('a connection without orchestration:operate shows the question with its answers disabled (canOperateThread)', async () => {
    const { client, command, committed } = harness(input([question('scope'), question('format')]));
    const shown = () => requestPresentation(client).questions[0]!;
    const env = (q: Obj) => ({ q, busy: false });
    expect(shown()).toMatchObject({ operable: true, canRespond: true });
    expect(viewDisabled('QuestionChoice(option=option', env(shown()))).toBe(false);
    client.scopes = ['orchestration:read', 'terminal:read'];
    expect(shown()).toMatchObject({ operable: false, canRespond: false, live: true, responseReason: 'This connection does not have permission to respond.' });
    // The options (and so their digit shortcuts) and Submit/Next are disabled; Previous and Dismiss stay as they were.
    expect(viewDisabled('QuestionChoice(option=option', env(shown()))).toBe(true);
    expect(viewDisabled('testId=`submit-answers-', env({ ...shown(), canAdvance: true }))).toBe(true);
    // handleOptionSelection: a pick on the disabled card neither selects nor advances.
    await command('pick-option', 'r1::scope', 'Session');
    await command('choice', 'r1::scope', 'Workspace');
    expect(shown()).toMatchObject({ id: 'r1::scope', index: 0 });
    expect(shown().options.every(option => !option.selected)).toBe(true);
    expect(committed).toHaveLength(0);
    client.scopes = ['orchestration:read', 'orchestration:operate'];
    expect(viewDisabled('testId=`submit-answers-', env({ ...shown(), canAdvance: true }))).toBe(false);
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
  // ChatComposer at v0.1.5: the question card shows while an approval waits on its timeline row.
  test('a question shows while an approval waits, and the approval keeps its row', () => {
    const { client, projectionOf } = harness(input([question('scope')]));
    projectionOf().runtimeRequests = [{ id: 'a', kind: 'command', status: 'pending', createdAt: '2026-10-03T00:00:05Z', responseCapability: { type: 'live' } }, ...(projectionOf().runtimeRequests as Obj[])];
    const view = requestPresentation(client);
    expect(view.requestMode).toBe('question');
    expect(view.questions).toMatchObject([{ id: 'r1::scope', question: 'Which scope?' }]);
    expect([...timelinePendingApprovals(client).keys()]).toEqual(['a']);
    // The placeholder stays the approval's (isComposerApprovalState comes first).
    expect(snapshot(client).composer.placeholder).toBe('Send a message to decline and redirect the agent');
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
  // TimelinePendingApprovalRow (#26255): the approval answers on its own row; the work before it stops reading as live.
  test('a pending approval is its own row after the running turn\'s earlier tools', () => {
    const row = (id: string, item: Obj) => ({ sourceThreadId: 't1', sourceItemId: id, ordinal: 0, item: { id, runId: 'r1', status: 'completed', startedAt: '2026-10-03T00:00:00Z', ...item } });
    const { client } = harness({ runs: [{ id: 'r1', status: 'running', startedAt: '2026-10-03T00:00:00Z' }],
      runtimeRequests: [{ id: 'a', kind: 'command', status: 'pending', responseCapability: { type: 'live' } }] });
    const items = [row('u', { type: 'user_message', text: 'fixture approval' }), row('c', { type: 'command_execution', input: 'ls', output: '' }),
      row('q', { type: 'approval_request', requestId: 'a', prompt: 'Allow the fixture?' })];
    Object.assign(client.thread!.projection, { turnItems: items.map(entry => entry.item), visibleTurnItems: items });
    const rows = transcriptPresentation(client);
    expect(rows.flatMap(message => message.approval ?? [])).toMatchObject([{ id: 'a', label: 'Command approval', detail: 'Allow the fixture?', live: true }]);
    expect(rows.findIndex(message => (message.approval ?? []).length > 0)).toBeGreaterThan(rows.findIndex(message => message.title === 'Ran ls'));
    expect(rows.some(message => message.live && (message.kind === 'live' || message.kind === 'thinking'))).toBe(false);
  });
});


describe('requests arriving while typing', () => {
  test('the draft stays editable until idle or blur releases a newly arriving question', async () => {
    const { client, command, projectionOf } = harness({});
    requestPresentation(client);
    noteComposerTyping(client, true);
    Object.assign(projectionOf(), input([question('scope')]));
    expect(requestPresentation(client).requestMode).toBe('');
    await command('draft', '', 'Finish this thought');
    expect(client.local.drafts[client.draftKey]).toBe('Finish this thought');
    expect(client.answers['r1::scope']).toBeUndefined();
    noteComposerTyping(client, false);
    expect(requestPresentation(client).requestMode).toBe('question');
    expect(client.local.drafts[client.draftKey]).toBe('Finish this thought');
  });
  test('approvals also wait, a send releases them, and existing requests do not get hidden', () => {
    const { client, projectionOf } = harness({});
    requestPresentation(client);
    noteComposerTyping(client, true);
    projectionOf().runtimeRequests = [{ id: 'a', kind: 'command', status: 'pending', responseCapability: { type: 'live' } }];
    expect(requestPresentation(client).requestMode).toBe('');
    releaseHeldRequests(client);
    expect(requestPresentation(client).requestMode).toBe('approval');
    noteComposerTyping(client, true);
    expect(requestPresentation(client).requestMode).toBe('approval');
    client.threadId = 't2';
    expect(requestPresentation(client).requestMode).toBe('approval');
  });
  test('a stale single-choice press cannot advance a different question or a newly opened request', async () => {
    const { client, command, committed, projectionOf } = harness(input([question('scope'), question('format')]));
    await command('pick-option', 'r1::scope', 'Session');
    expect(requestPresentation(client).questions[0]?.id).toBe('r1::format');
    expect((await command('pick-option', 'r1::scope', 'Workspace')).message).toBe('That question is no longer active.');
    expect(requestPresentation(client).questions[0]?.id).toBe('r1::format');
    Object.assign(projectionOf(), input([question('scope')]));
    (projectionOf().runtimeRequests as Obj[])[0]!.id = 'r2';
    (projectionOf().turnItems as Obj[])[0]!.requestId = 'r2';
    await command('pick-option', 'r1::format', 'Session');
    expect(requestPresentation(client).questions[0]?.id).toBe('r2::scope');
    expect(committed).toEqual([]);
  });
});

describe('Pi editor initial answers', () => {
  test('seeds once, keeps literal whitespace, and accepts a deliberately cleared editor', async () => {
    const initial = '  @src/file.ts\n\n  literal [file](path)  \n';
    const f = harness(input([question('edit', { initialAnswer: initial })]));
    expect(requestPresentation(f.client).draft).toBe(initial);
    await f.command('answer', '', '   \n');
    expect(requestPresentation(f.client)).toMatchObject({ draft: '   \n', questions: [{ complete: true, canAdvance: true }] });
    await f.command('answer', '', '');
    expect(requestPresentation(f.client).draft).toBe('');
    expect(requestPresentation(f.client).questions[0]?.complete).toBe(true);
    expect(requestPresentation(f.client).draft).toBe('');
  });
  test('does not seed a question that forbids custom answers or replace an existing option draft', () => {
    const f = harness(input([question('scope', { initialAnswer: 'prefilled', allowCustomAnswer: false })]));
    expect(requestPresentation(f.client).draft).toBe('');
    const g = harness(input([question('scope', { initialAnswer: 'prefilled' })]));
    g.client.answers['r1::scope'] = { selected: ['Workspace'], custom: '', index: 0 };
    expect(requestPresentation(g.client).draft).toBe('');
    expect(g.client.answers['r1::scope']?.selected).toEqual(['Workspace']);
  });
});

// ComposerPendingUserInputPanel at 7c3b3b3605ef (d414cb5103), as requests.contract draws it.
describe('the question card', () => {
  const file = 'src/composer/requests.contract';
  const banner = contractComponent(file, 'QuestionBanner'), choice = contractComponent(file, 'QuestionChoice');
  const q = { id: 'q', requestId: 'r', header: 'Window', question: 'Which window shows the problem?', position: '1/2', dismissible: true, responding: false,
    live: true, operable: true, multiSelect: false, options: [], md: {} };
  /** The `name=` value of the first view line holding `marker`, read over `env` (a quoted value as it stands). */
  const attribute = (marker: string, name: string, env: Record<string, unknown>) => {
    const line = readFileSync(appPath(file), 'utf8').split('\n').find(text => text.includes(marker))!;
    const at = line.indexOf(` ${name}=`) + name.length + 2;
    if (line[at] === '"') return line.slice(at + 1, line.indexOf('"', at + 1));
    let depth = 0, end = at;
    do { if (line[end] === '(') depth++; else if (line[end] === ')') depth--; end++; } while (depth > 0);
    return value(line.slice(at, end), env);
  };
  test('the header row starts with the header, no icon before it; collapsed, it adds "·" and the question', () => {
    const lines = readFileSync(appPath(file), 'utf8').split('\n');
    const row = lines.findIndex(line => line.includes('row flex=1 min-width=0 gap="0.25rem" align-items="center"'));
    expect(lines[row + 1]!.trim().startsWith('text q.header ')).toBe(true);
    expect(attribute('row width="100%" min-height="1.5rem"', 'padding-left', {})).toBe('1rem');
    for (const collapsed of ['', 'q']) {
      expect(banner.drawn('text "·"', { collapsed }, { q })).toBe(collapsed === 'q');
      expect(banner.drawn('text q.question', { collapsed }, { q })).toBe(collapsed === 'q');
      expect(banner.drawn('testId=`question-body-', { collapsed }, { q })).toBe(collapsed === '');
    }
  });
  test('an option has a radio, or a checkbox when several can be chosen; unselected labels at 70%; its key cap in a 1.5rem column', () => {
    const option = { id: 'o', label: 'Main', description: '', shortcut: '1', selected: false };
    for (const multi of [false, true]) {
      const scope = { option, questionId: 'q', multi, disabled: false };
      expect(choice.drawn('testId=`choice-radio-', { hovering: false }, scope)).toBe(!multi);
      expect(choice.drawn('testId=`choice-checkbox-', { hovering: false }, scope)).toBe(multi);
    }
    const color = (selected: boolean) => attribute('testId=`choice-label-', 'color', { option: { ...option, selected } });
    expect(color(false)).toBe('light-dark(#18181bb3, #f5f5f5b3)');
    expect(color(true)).toBe('light-dark(#18181b, #f5f5f5)');
    const scope = { option, questionId: 'q', multi: false, disabled: false };
    expect(choice.drawn('testId=`choice-key-', { hovering: false }, scope)).toBe(true);
    expect(attribute('testId=`choice-key-', 'width', {})).toBe('1.5rem');
    expect(choice.drawn('testId=`choice-key-', { hovering: false }, { ...scope, option: { ...option, shortcut: '' } })).toBe(false);
    // A description that repeats the label is not drawn twice.
    expect(choice.drawn('text option.description', { hovering: false }, { ...scope, option: { ...option, description: 'Main' } })).toBe(false);
    expect(choice.drawn('text option.description', { hovering: false }, { ...scope, option: { ...option, description: 'The main window' } })).toBe(true);
  });
});
