import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { setTurnItemOpen, turnItemIsOpen } from './shared/timeline-item-fetch';
import { mobileAnswerFile, mobileAnswerFilesRequest, mobilePrepareAnswerFiles, mobileOpenAnswerFile } from './thread-answer-files';
import { mobileThread, mobileThreadAnswerFilesPrepare } from './thread';
import { chatLocal } from './shared/timeline-presentation';
const now = 1_800_000_000_000;
function fixture(count = 1) {
  const client = new T3Client(); Object.assign(client, { origin: 'https://answer.test', environmentId: 'env', threadId: 'selected', generation: 7,
    connection: 'connected', configLive: true, shellLive: true, threadLive: true });
  const files = Array.from({ length: count }, (_, n) => ({ id: `file-${n}`, name: `${n}.png`, type: 'image', mimeType: 'image/png' }));
  const row: Obj = { sourceThreadId: 'inherited', sourceItemId: 'question-item', item: { id: 'question-item', threadId: 'inherited', type: 'user_input_request',
    updatedAt: '2026-10-07T12:00:00Z', questionAnswer: { requestId: 'request', questionTextById: { q: 'Which file?' }, answers: {}, attachmentsByQuestionId: { q: files } } } };
  client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: 1, projection: {
    thread: { id: 'selected' }, visibleTurnItems: [row], turnItems: [obj(row.item)], runs: [], attempts: [], nodes: [], checkpoints: [], runtimeRequests: [], subagents: [] } };
  setTurnItemOpen(client, JSON.stringify(['inherited', 'question-item']), true);
  const requests: Obj[] = [], opens: Obj[] = [];
  client.rpc = async (_native, method, payload) => { requests.push({ method, ...payload }); return { relativeUrl: `/assets/${obj(payload.resource).attachmentId}`, expiresAt: now + 3_600_000 }; };
  const native: Native = { available: true, watch() {}, async later(input) { opens.push(obj(input)); return { ok: true, generation: 7, value: { opened: true } }; } };
  const view = (at = now, file = files[0]!) => mobileAnswerFile(client, row, 'q', file, at);
  return { client, row, files, requests, opens, native, view };
}
async function prepare(f: ReturnType<typeof fixture>, at: number) {
  const visible = turnItemIsOpen(f.client, JSON.stringify(['inherited', 'question-item'])) ? f.files.map(file => f.view(at, file).id) : [];
  const request = mobileAnswerFilesRequest(f.client, visible, at);
  return mobilePrepareAnswerFiles(f.client, f.native, at, request.owner, request.request, visible);
}
test('nested inherited answer files sign exact source attachment resources and open the actual URL', async () => {
  const f = fixture(); expect(f.view().url).toBe('');
  await prepare(f, now);
  expect(f.requests).toEqual([{ method: 'assets.createUrl', resource: { _tag: 'attachment', attachmentId: 'file-0' } }]);
  expect(f.view()).toMatchObject({ name: '0.png', image: true, url: 'https://answer.test/assets/file-0' });
  expect(JSON.parse(f.view().id).slice(1, 5)).toEqual(['inherited', 'question-item', 'q', 'file-0']);
  expect(await mobileOpenAnswerFile(f.client, f.view().id, f.native, now)).toMatchObject({ message: '' });
  expect(f.opens).toEqual([{ generation: 7, op: 'mobileOpenURL', url: 'https://answer.test/assets/file-0' }]);
  expect(f.requests).toHaveLength(1);
});
test('collapsed answer rows do not sign files', async () => {
  const f = fixture(); setTurnItemOpen(f.client, JSON.stringify(['inherited', 'question-item']), false);
  await prepare(f, now); expect(f.requests).toHaveLength(0);
});
for (const change of ['remove', 'replace', 'generation', 'selection', 'source']) test(`late answer URL cannot survive ${change}`, async () => {
  const f = fixture(); let release!: (value: Obj) => void;
  f.client.rpc = async () => new Promise(resolve => release = resolve);
  const pending = prepare(f, now), id = f.view().id;
  if (change === 'remove') obj(obj(f.row.item).questionAnswer).attachmentsByQuestionId = { q: [] };
  if (change === 'replace') f.files[0]!.name = 'replaced.png';
  if (change === 'generation') f.client.generation++;
  if (change === 'selection') f.client.threadId = 'other';
  if (change === 'source') f.row.sourceThreadId = 'different-inherited';
  release({ relativeUrl: '/late', expiresAt: now + 3_600_000 });
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' }); expect(f.view().url).toBe('');
  await expect(mobileOpenAnswerFile(f.client, id, f.native, now)).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.opens).toHaveLength(0);
});
test('pending reads survive ordinary projection and repeated preparation without replacement', async () => {
  const f = fixture(), releases: ((value: Obj) => void)[] = [];
  f.client.rpc = async () => new Promise(resolve => releases.push(resolve));
  const first = prepare(f, now), before = mobileAnswerFilesRequest(f.client, [f.view().id], now);
  expect(before.request).toBe('');
  for (let n = 0; n < 20; n++) expect(mobileAnswerFilesRequest(f.client, [f.view().id], now)).toEqual(before);
  await prepare(f, now); expect(releases).toHaveLength(1);
  releases[0]!({ relativeUrl: '/kept', expiresAt: now + 3_600_000 }); await first;
  expect(f.view().url).toBe('https://answer.test/kept');
  expect(mobileAnswerFilesRequest(f.client, [f.view().id], now).request).toBe('');
});
test('stale refresh retains a valid displayed URL, expiry and dead connections hide it', async () => {
  const f = fixture(); await prepare(f, now);
  let release!: (value: Obj) => void; f.client.rpc = async () => new Promise(resolve => release = resolve);
  const later = now + 300_001, pending = prepare(f, later);
  expect(f.view(later).url).toBe('https://answer.test/assets/file-0');
  f.client.connection = 'reconnecting'; expect(f.view(later).url).toBe(''); f.client.connection = 'connected';
  release({ relativeUrl: '/renewed', expiresAt: now + 3_600_000 }); await pending;
  expect(f.view(later).url).toBe('https://answer.test/renewed'); expect(f.view(now + 3_600_000).url).toBe('');
});
test('failure is disabled without a hot retry loop; stale failure can be prepared again', async () => {
  const f = fixture(); let calls = 0; f.client.rpc = async () => { calls++; throw new Error('asset unavailable'); };
  await prepare(f, now); await prepare(f, now + 1);
  expect(calls).toBe(1); expect(f.view().url).toBe('');
  expect((await mobileOpenAnswerFile(f.client, f.view().id, f.native, now)).message).toContain('could not be opened');
  expect(f.opens).toHaveLength(0);
  await prepare(f, now + 300_001); expect(calls).toBe(2);
});
test('invalid or expired URL never enables a link and OS refusal is not successful', async () => {
  for (const reply of [{ relativeUrl: 'javascript:alert(1)', expiresAt: now + 3_600_000 },
    { relativeUrl: 'https://user:secret@answer.test/a', expiresAt: now + 3_600_000 }, { relativeUrl: '/expired', expiresAt: now }]) {
    const f = fixture(); f.client.rpc = async () => reply;
    await prepare(f, now); expect(f.view().url).toBe('');
  }
  const f = fixture(); await prepare(f, now);
  const refusal: Native = { ...f.native, async later() { return { ok: true, generation: 7, value: { opened: false } }; } };
  expect((await mobileOpenAnswerFile(f.client, f.view().id, refusal, now)).message).toContain('could not be opened');
});
test('more than256 active attachments remain enabled without repeated signing', async () => {
  const f = fixture(257); await prepare(f, now);
  await prepare(f, now + 1);
  expect(f.requests).toHaveLength(257); expect(f.files.every(file => f.view(now + 1, file).url !== '')).toBe(true);
  setTurnItemOpen(f.client, JSON.stringify(['inherited', 'question-item']), false);
  await prepare(f, now + 2);
  expect(f.files.filter(file => f.view(now + 2, file).url !== '')).toHaveLength(256);
});

test('actual transcript descriptor excludes a child whose enclosing run fold is closed', async () => {
  const f = fixture(); Object.assign(obj(f.row.item), { runId: 'run', status: 'completed', startedAt: '2026-10-07T12:00:00Z' });
  f.client.projection.runs = [{ id: 'run', status: 'completed', startedAt: '2026-10-07T12:00:00Z', completedAt: '2026-10-07T12:01:00Z' }];
  let shown = mobileThread(now, false, f.client);
  expect(shown.rows.some(row => row.toggleOp === 'chatlocal:fold' && !row.expanded)).toBe(true);
  expect(shown.answerFilesRequest).toBe('');
  await mobileThreadAnswerFilesPrepare(shown.answerFilesOwner, shown.answerFilesRequest, now, f.native, f.client);
  expect(f.requests).toHaveLength(0);
  await chatLocal(f.client, f.native, 'fold', 'run', '');
  shown = mobileThread(now, false, f.client); expect(shown.answerFilesRequest).not.toBe('');
  await mobileThreadAnswerFilesPrepare(shown.answerFilesOwner, shown.answerFilesRequest, now, f.native, f.client);
  expect(f.requests).toHaveLength(1); expect(f.view().url).not.toBe('');
});
test('explicit preparation bounds concurrency to four and drains all owned reads', async () => {
  const f = fixture(9); let active = 0, peak = 0, calls = 0;
  f.client.rpc = async () => { active++; calls++; peak = Math.max(peak, active); await Promise.resolve(); active--; return { relativeUrl: '/asset', expiresAt: now + 3_600_000 }; };
  await prepare(f, now); expect(calls).toBe(9); expect(peak).toBe(4); expect(active).toBe(0);
});


test('Open-owned signing does not schedule empty preparation mutations', async () => {
  const f = fixture(); await prepare(f, now);
  let release!: (value: Obj) => void, calls = 0;
  f.client.rpc = async () => { calls++; return new Promise(resolve => release = resolve); };
  const stale = now + 300_001, opening = mobileOpenAnswerFile(f.client, f.view(stale).id, f.native, stale);
  for (let n = 0; n < 3; n++) {
    expect(mobileAnswerFilesRequest(f.client, [f.view(stale).id], stale).request).toBe('');
    await prepare(f, stale);
  }
  expect(calls).toBe(1);
  release({ relativeUrl: '/renewed-open', expiresAt: now + 3_600_000 });
  expect((await opening).message).toBe('');
  expect(f.view(stale).url).toBe('https://answer.test/renewed-open');
});
