import { describe, expect, test } from 'bun:test';
import { opened } from '../support/composer-controls-fixture';
import { obj, type Obj } from '../../src/client/domain';
import { draftFiles, fileChipLink, stageTextAttachment } from '../../src/composer/composer-editor-files';

type Gate = 'read' | 'mint' | 'upload' | 'ids' | 'persist';
type Switch = 'thread' | 'environment' | 'origin' | 'generation' | 'request';
async function questionSubmission(gate?: Gate) {
  const fixture = await opened(), { client, native, disk } = fixture;
  client.projection.runtimeRequests = [{ id: 'questions', kind: 'user_input', status: 'pending', responseCapability: { type: 'live' } }];
  client.projection.turnItems = [{ id: 'request-item', type: 'user_input_request', requestId: 'questions', questions:
    [{ id: 'first', question: 'First scope', allowCustomAnswer: true, options: [] }, { id: 'second', question: 'Second scope', allowCustomAnswer: true, options: [] }] }];
  const first = stageTextAttachment(client, 'first context', 'first.txt', 'text/plain', 'first-file');
  const second = stageTextAttachment(client, 'second context', 'second.txt', 'text/plain', 'second-file');
  if (gate === 'read') first.source = 'attached';
  client.answers.questions = { selected: [], custom: '', index: 1 };
  client.answers['questions::first'] = { selected: [], custom: fileChipLink(first), index: 0 };
  client.answers['questions::second'] = { selected: [], custom: fileChipLink(second), index: 0 };
  const answers = JSON.stringify(client.answers), draftKey = client.draftKey;
  client.local.drafts[draftKey] = 'Keep the original draft';
  let changed = false, reached!: () => void, release!: () => void, waiting = true;
  const entered = new Promise<void>(resolve => { reached = resolve; });
  const held = new Promise<void>(resolve => { release = resolve; });
  const calls: { request: Obj; afterSwitch: boolean }[] = [];
  const original = native.later.bind(native), write = disk.fs.atomicWriteFile.bind(disk.fs);
  native.later = async input => {
    const request = obj(input); calls.push({ request, afterSwitch: changed });
    if (waiting && ((gate === 'read' && request.op === 'composerAttachRead') || (gate === 'mint' && request.method === 'attachments.createUploadUrl')
      || (gate === 'upload' && request.op === 'uploadAttachment') || (gate === 'ids' && request.op === 'ids'))) {
      waiting = false; reached(); await held;
    }
    if (request.op === 'composerAttachRead') return native.ok({ base64: btoa('first context') });
    if (request.method === 'attachments.createUploadUrl') return native.ok({ attachmentId: `uploaded-${obj(request.payload).name}`, relativeUrl: '/api/attachments/upload/test' });
    if (request.op === 'uploadAttachment' || request.op === 'composerAttachRemove') return native.ok({});
    return original(input);
  };
  disk.fs.atomicWriteFile = async (path, bytes) => {
    if (gate === 'persist' && waiting && new TextDecoder().decode(bytes).includes('runtime-request.respond')) { waiting = false; reached(); await held; }
    return write(path, bytes);
  };
  return { ...fixture, calls, entered, release, answers, draftKey, markChanged: () => { changed = true; } };
}

describe('pending question submission ownership', () => {
  for (const gate of ['read', 'mint', 'upload', 'ids', 'persist'] as const) for (const replacement of ['thread', 'environment'] as const) {
    test(`${replacement} replacement during ${gate} cannot receive the previous request or later question uploads`, async () => {
      const fixture = await questionSubmission(gate), { client, native, command, calls, entered, release, draftKey, answers } = fixture;
      const submitting = command('submit-answers', 'questions');
      await Promise.race([entered, submitting.then(() => { throw Error('Submission completed before the deferred boundary'); })]);
      fixture.markChanged();
      if (replacement === 'thread') client.threadId = 't2'; else client.environmentId = 'other-environment';
      client.local.drafts[client.draftKey] = 'Keep the replacement draft';
      stageTextAttachment(client, 'replacement context', 'replacement.txt', 'text/plain', 'second-file');
      release(); await submitting;
      expect(native.committed.filter(request => request.type === 'runtime-request.respond')).toEqual([]);
      expect(calls.filter(call => call.afterSwitch && ['composerAttachRead', 'uploadAttachment'].includes(String(call.request.op)))).toEqual([]);
      expect(calls.filter(call => call.afterSwitch && ['attachments.createUploadUrl', 'orchestration.dispatchCommand'].includes(String(call.request.method)))).toEqual([]);
      expect(client.local.pending).toEqual({});
      expect(client.local.drafts[draftKey]).toBe('Keep the original draft');
      expect(client.local.drafts[client.draftKey]).toBe('Keep the replacement draft');
      expect(JSON.stringify(client.answers)).toBe(answers);
      expect(draftFiles(client.local).filter(file => file.draftKey === draftKey)).toHaveLength(2);
    });
  }
  for (const replacement of ['origin', 'generation', 'request'] as const) test(`${replacement} replacement during upload retires the submission`, async () => {
    const fixture = await questionSubmission('upload'), { client, native, command, entered, release, answers, calls } = fixture;
    const submitting = command('submit-answers', 'questions');
    await Promise.race([entered, submitting.then(() => { throw Error('Submission completed before upload'); })]);
    fixture.markChanged();
    if (replacement === 'origin') client.origin = 'https://replacement.example';
    else if (replacement === 'generation') client.generation++;
    else {
      client.projection.runtimeRequests = [{ id: 'replacement', kind: 'user_input', status: 'pending', responseCapability: { type: 'live' } }];
      client.projection.turnItems = [{ id: 'replacement-item', type: 'user_input_request', requestId: 'replacement', questions:
        [{ id: 'replacement-question', question: 'A new request', initialAnswer: 'A new initial answer', allowCustomAnswer: true, options: [] }] }];
    }
    release(); await submitting;
    expect(native.committed.filter(request => request.type === 'runtime-request.respond')).toEqual([]);
    expect(calls.filter(call => call.afterSwitch && ['attachments.createUploadUrl', 'orchestration.dispatchCommand'].includes(String(call.request.method)))).toEqual([]);
    expect(JSON.stringify(client.answers)).toBe(answers);
    expect(client.local.pending).toEqual({});
  });
  test('the same owner submits both question attachments to the captured request', async () => {
    const { command, native, client } = await questionSubmission();
    await command('submit-answers', 'questions');
    expect(client.error).toBe('');
    expect(native.committed.at(-1)).toMatchObject({ type: 'runtime-request.respond', threadId: 't1', requestId: 'questions', attachmentsByQuestionId: {
      first: [{ id: 'uploaded-first.txt', type: 'file' }], second: [{ id: 'uploaded-second.txt', type: 'file' }],
    } });
  });
});
