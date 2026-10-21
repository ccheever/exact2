import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { obj, type Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { opened } from '../support/composer-controls-fixture';
import { addAttachedFiles } from '../../src/composer/composer-controls-attach';
import { reservedAttachments } from '../../src/composer/composer-editor-attach';
import { advanceQuestion, requestPresentation, resolveAnswer } from '../../src/composer/requests';
import { questionDraftKey, releaseQuestionDrafts, snapshotAttachmentKey } from '../../src/composer/question-attachments';
import { draftRows } from '../../src/sidebar/sidebar-view';

const uuid = (n: number) => `aaaaaaaa-aaaa-4aaa-8aaa-${String(n).padStart(12, '0')}`;
const asked = (questions: Obj[] = [{ id: 'first', header: 'First', question: 'Which window?', allowCustomAnswer: true, options: [] }]) => ({
  runtimeRequests: [{ id: 'questions', kind: 'user_input', status: 'pending', responseCapability: { type: 'live' } }],
  turnItems: [{ id: 'request-item', type: 'user_input_request', requestId: 'questions', questions }] });
const image = (n: number, extra: Obj = {}): Obj => ({ id: uuid(n), name: `shot-${n}.png`, mimeType: 'image/png', sizeBytes: 10, ...extra });

describe('question attachment drafts (questionAttachments.ts)', () => {
  test('each question has its own draft key, apart from the thread draft and the sidebar draft rows', () => {
    const key = questionDraftKey('env', 't1', 'request', 'question');
    expect(key).toBe('env:question:t1:["request","question"]');
    expect(key).not.toBe(questionDraftKey('env', 't1', 'request', 'other'));
    expect(key.startsWith('env:new:')).toBe(false);
  });
  test('a resolved request releases its question drafts and their images; pending ones stay', () => {
    const drafts: Record<string, Obj[]> = { 'env:t1': [image(1)], [questionDraftKey('env', 't1', 'old', 'q')]: [image(2)],
      [questionDraftKey('env', 't1', 'live', 'q')]: [image(3)], [questionDraftKey('env', 't2', 'old', 'q')]: [image(4)] };
    const releases: string[] = [];
    expect(releaseQuestionDrafts(drafts, releases, 'env:question:t1:', new Set([questionDraftKey('env', 't1', 'live', 'q')]))).toBe(true);
    expect(Object.keys(drafts).sort()).toEqual(['env:question:t1:["live","q"]', 'env:question:t2:["old","q"]', 'env:t1']);
    expect(releases).toEqual([uuid(2)]);
  });
  test('resolveSnapShotAttachmentTarget: a capture keeps the question open when it was first seen', () => {
    const pins = new Map<string, string | null>();
    expect(snapshotAttachmentKey(pins, 'a', 'thread', 'question-1')).toBe('question-1');
    // The question closed, or another one opened: the capture goes to the thread's draft.
    expect(snapshotAttachmentKey(pins, 'a', 'thread', null)).toBe('thread');
    expect(snapshotAttachmentKey(pins, 'a', 'thread', 'question-2')).toBe('thread');
    // Absence is pinned too: a later question never claims an earlier capture.
    expect(snapshotAttachmentKey(pins, 'b', 'thread', null)).toBe('thread');
    expect(snapshotAttachmentKey(pins, 'b', 'thread', 'question-1')).toBe('thread');
  });
  test('attachments alone answer a question that takes a custom answer, unless the server cannot take them', () => {
    const question = { id: 'q', header: '', question: '', multiSelect: false, allowCustom: true, options: [] };
    expect(resolveAnswer(question, undefined)).toBeNull();
    expect(resolveAnswer(question, undefined, { count: 1, blocked: false })).toBe('');
    expect(resolveAnswer({ ...question, multiSelect: true }, { selected: [], custom: '  ', index: 0 }, { count: 2, blocked: false })).toBe('');
    expect(resolveAnswer({ ...question, allowCustom: false }, undefined, { count: 1, blocked: false })).toBeNull();
    expect(resolveAnswer(question, { selected: [], custom: 'typed', index: 0 }, { count: 1, blocked: true })).toBeNull();
  });
});

async function questionClient(capabilities: Obj = { questionAttachments: true }) {
  const context = await opened(), { client, native } = context;
  client.config = { ...client.config, environment: { ...obj(client.config.environment), capabilities: { ...obj(obj(client.config.environment).capabilities),
    attachmentUploads: true, fileAttachments: { maxUploadBytes: 26_214_400 }, ...capabilities } } };
  Object.assign(client.thread!.projection, asked([{ id: 'first', header: 'First', question: 'Which window?', allowCustomAnswer: true, options: [] },
    { id: 'second', header: 'Second', question: 'Anything else?', allowCustomAnswer: true, options: [] }]));
  const calls: Obj[] = [], later = native.later.bind(native);
  native.later = async input => {
    const request = obj(input); calls.push(request);
    if (request.op === 'editorInsert') return native.ok({ applied: true });
    if (request.op === 'snapshotDraftRead') return native.ok({ base64: 'png' });
    if (request.op === 'snapshotDraftRemove' || request.op === 'uploadAttachment') return native.ok({});
    if (request.method === 'attachments.createUploadUrl') return native.ok({ attachmentId: `uploaded-${obj(request.payload).name}`, relativeUrl: '/api/attachments/upload/test' });
    return later(input);
  };
  const first = questionDraftKey(client.environmentId, client.threadId, 'questions', 'first');
  const second = questionDraftKey(client.environmentId, client.threadId, 'questions', 'second');
  return { ...context, calls, first, second };
}

describe('a question takes images beside its answer', () => {
  test('a drop, a paste or Attach files puts images on the active question’s shelf, without an image chip', async () => {
    const { client, native, calls, first } = await questionClient();
    client.local.drafts[client.draftKey] = 'The thread draft keeps its text';
    expect(client.attachmentKey).toBe(first);
    await addAttachedFiles(client, native, [{ kind: 'image', ...image(1) }]);
    expect(client.local.snapshotDrafts[first]).toMatchObject([{ id: uuid(1), name: 'shot-1.png' }]);
    expect(client.local.snapshotDrafts[client.draftKey] ?? []).toEqual([]);
    expect(client.snapshotDrafts.map(entry => entry.id)).toEqual([uuid(1)]);
    expect(calls.filter(call => call.op === 'editorInsert')).toEqual([]);
    expect(client.local.drafts[client.draftKey]).toBe('The thread draft keeps its text');
    // The request's questions share one answer's attachment limit.
    client.answers['questions::first'] = { selected: [], custom: '', index: 0 };
    client.local.snapshotDrafts[questionDraftKey(client.environmentId, client.threadId, 'questions', 'second')] = [image(2)];
    expect(reservedAttachments(client)).toBe(2);
    // The sidebar lists new-thread drafts only; a question's shelf is not one.
    expect(draftRows(client, () => ({ projectMark: '', projectInk: '', projectSurface: '' }) as never)).toEqual([]);
  });
  test('the answer uploads each question’s images and sends them as that question’s attachments', async () => {
    const { client, native, command, first, second } = await questionClient();
    client.local.snapshotDrafts[first] = [image(1, { source: { kind: 'snap-shot', appName: 'Fixture' } })];
    client.local.snapshotDrafts[second] = [image(2), image(3)];
    client.answers.questions = { selected: [], custom: '', index: 1 };
    client.answers['questions::second'] = { selected: [], custom: 'Both of these', index: 0 };
    // The first question has no text: its image answers it.
    expect(requestPresentation(client).questions[0]).toMatchObject({ canAdvance: true, complete: true });
    await command('submit-answers', 'questions');
    expect(client.error).toBe('');
    expect(native.committed.at(-1)).toMatchObject({ type: 'runtime-request.respond', requestId: 'questions', answers: { first: '', second: 'Both of these' },
      attachmentsByQuestionId: {
        first: [{ type: 'image', id: 'uploaded-shot-1.png', name: 'shot-1.png', mimeType: 'image/png', source: { kind: 'snap-shot', appName: 'Fixture' } }],
        second: [{ type: 'image', id: 'uploaded-shot-2.png' }, { type: 'image', id: 'uploaded-shot-3.png' }],
      } });
    // The request resolves: its drafts go and their local copies are released.
    client.thread!.projection.runtimeRequests = [{ id: 'questions', kind: 'user_input', status: 'resolved', responseCapability: { type: 'live' } }];
    requestPresentation(client);
    expect(client.local.snapshotDrafts[first]).toBeUndefined();
    expect(client.local.snapshotDrafts[second]).toBeUndefined();
    expect(client.local.snapshotReleases).toEqual(expect.arrayContaining([uuid(1), uuid(2), uuid(3)]));
    expect(client.attachmentKey).toBe(client.draftKey);
  });
  test('images a server cannot take with an answer keep the question unanswered', async () => {
    const { client, first } = await questionClient({ questionAttachments: false });
    client.local.snapshotDrafts[first] = [image(1)];
    client.answers['questions::first'] = { selected: [], custom: 'Typed', index: 0 };
    expect(() => advanceQuestion(client, 'questions')).toThrow('Answer “First” first.');
  });
});

// SnapShotCoordinator: a capture taken while a question is open attaches to it, pinned when the capture starts.
function captureFixture(capabilities: Obj = { questionAttachments: true }) {
  const client = new T3Client(), id = uuid(9);
  Object.assign(client, { generation: 1, environmentId: 'env', projectId: 'p', threadId: 't', connection: 'connected', configLive: true, shellLive: true, threadLive: true,
    providerId: 'fixture', modelId: 'fixture-model', scopes: ['orchestration:read', 'orchestration:operate'] });
  client.config = { environment: { capabilities: { serverResolvedCommandContext: true, ...capabilities } }, providers: [] };
  client.shell = { sequence: 1, projects: [{ id: 'p' }], threads: [{ id: 't', projectId: 'p' }] };
  client.local.deviceSettings.snapShotEnabled = true;
  const state: Obj = { captures: [], capturing: [] }, calls: Obj[] = [];
  const files: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode('').buffer; }, async atomicWriteFile() {} } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    let value: unknown = {};
    if (request.op === 'snapshotState') value = state;
    if (request.op === 'http') value = { projects: client.shell.projects, threads: client.shell.threads };
    if (request.op === 'snapshotRead') value = { id, owner: request.owner, name: 'Capture.png', mimeType: 'image/png', sizeBytes: 68, source: { kind: 'snap-shot' } };
    if (request.op === 'snapshotDraftSave') value = { id, sizeBytes: 68 };
    return { ok: true, generation: 1, value };
  } };
  const question = (open: boolean) => { (client as unknown as { thread: Obj }).thread = { projection: { thread: { id: 't' }, ...(open ? asked() : {}) } }; };
  const key = questionDraftKey('env', 't', 'questions', 'first');
  const captured = () => { state.capturing = []; state.captures = [{ id, owner: client.snapshotOwner }]; };
  return { client, native, files, calls, state, id, key, question, captured };
}

describe('SnapShots during a question', () => {
  test('a capture taken while a question is open joins that question', async () => {
    const f = captureFixture();
    f.question(true); f.captured();
    await f.client.adoptSnapshots(f.native, f.files);
    expect(f.client.local.snapshotDrafts[f.key]).toMatchObject([{ id: f.id }]);
    expect(f.client.local.snapshotDrafts['env:t'] ?? []).toEqual([]);
    expect(f.calls.find(call => call.op === 'snapshotAcknowledge')).toMatchObject({ id: f.id, focus: true });
  });
  test('a capture started before the question opened stays with the thread', async () => {
    const f = captureFixture();
    f.question(false); f.state.capturing = [f.id];
    await f.client.adoptSnapshots(f.native, f.files);
    f.question(true); f.captured();
    await f.client.adoptSnapshots(f.native, f.files);
    expect(f.client.local.snapshotDrafts['env:t']).toMatchObject([{ id: f.id }]);
    expect(f.client.local.snapshotDrafts[f.key]).toBeUndefined();
  });
  test('a capture whose question closed before it landed goes to the thread', async () => {
    const f = captureFixture();
    f.question(true); f.state.capturing = [f.id];
    await f.client.adoptSnapshots(f.native, f.files);
    f.question(false); f.captured();
    await f.client.adoptSnapshots(f.native, f.files);
    expect(f.client.local.snapshotDrafts['env:t']).toMatchObject([{ id: f.id }]);
  });
  test('a server that takes no question attachments keeps captures on the thread', async () => {
    const f = captureFixture({ questionAttachments: false });
    f.question(true); f.captured();
    await f.client.adoptSnapshots(f.native, f.files);
    expect(f.client.local.snapshotDrafts['env:t']).toMatchObject([{ id: f.id }]);
    expect(f.client.local.snapshotDrafts[f.key]).toBeUndefined();
  });
});
