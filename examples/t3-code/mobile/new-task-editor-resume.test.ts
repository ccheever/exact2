import { expect, test } from 'bun:test';
import { mobileNewTaskEditorAction as action, mobileNewTaskEditorView as view } from './new-task-editor';
import { mobileNewTaskDraftChoicesUpdate, mobileNewTaskDraftStore } from './mobile-new-task-drafts';
import { mobileNewTaskContextRead } from './mobile-new-task-context';
import { mobileDraftAttachmentIds } from './draft-attachment-order';
import { draftFiles, setDraftFiles, type DraftFile } from './shared/composer-editor-files';
import { contextLink } from './shared/composer-editor-menu';
import { obj, str, type Obj } from './shared/domain';
import { fleet } from './shared/settings-b-fleet';
import type { Files, Native } from './shared/protocol';
import type { PickerIntakeFile } from './composer-picker-intake-io';
import { fixture, deferred, event, config, continuation, heldReply, intakeReply, intakeCalls, pickedFile, pickedImage,
  pickedId, oldId, savedRecord } from './new-task-editor-test-support';

const id = (n: number) => `${String(n).padStart(8, '0')}-aaaa-4aaa-8aaa-aaaaaaaaaaaa`;
const range = (value: string) => ({ start: value.length, end: value.length });
const resumes = (f: Awaited<ReturnType<typeof fixture>>) => f.calls.filter(call => call.action === 'resume');

// Native transitions are mocked here. Saves serialize the real MobileDraftClient into the fixture's disk bytes.
async function cycleFixture(source = '🙂 replace here', selection = { start: 3, end: 10 }, initial: readonly PickerIntakeFile[] = [pickedFile]) {
  const f = await fixture(); continuation(f, initial);
  let count = 1, value = source, caret = selection, sequence = 0, files = initial;
  let frozen: Obj | null = null;
  const proofs = new Map<string, { request: string; event: Obj }>();
  f.idsHook(async () => [id(++sequence)]);
  f.handoffHook(async request => {
    frozen = { receiptId: id(100 + sequence), identity: f.identity, retired: false,
      event: { ...event(f.identity, value, ++count, 'text'), selection: caret, kind: 'frozen',
        operationId: request.operationId, operationRevision: request.operationRevision } };
    f.receipts([frozen]);
    return { ok: true, generation: request.generation, value: { receipt: frozen } };
  });
  const nativeResume = async (request: Obj) => {
    let proof = proofs.get(str(request.operationId));
    if (proof) expect(JSON.stringify(request)).toBe(proof.request);
    else {
      value = str(obj(request.document).value); caret = obj(request.document).selection as typeof caret;
      proof = { request: JSON.stringify(request), event: { ...event(f.identity, value, ++count, 'text'), selection: { ...caret },
        kind: 'resumed', operationId: request.operationId, operationRevision: request.operationRevision, receiptId: request.receiptId } };
      proofs.set(str(request.operationId), proof); f.receipts([]);
    }
    return { ok: true, generation: request.generation, value: { event: structuredClone(proof.event) } };
  };
  f.resumeHook(nativeResume);
  f.intakeHook(async request => intakeReply(request, request.action === 'finish' ? 'finished'
    : request.action === 'cancel' ? 'cancelled' : 'staged', files));
  const pick = (kind: 'files' | 'photos' = 'files') => action(f.client, f.route, { kind: 'pick', source: kind }, f.native, f.storage);
  const change = (text: string, selected = range(text)) => {
    value = text; caret = selected;
    return { ...event(f.identity, text, ++count, 'text'), selection: { ...selected } };
  };
  const observe = (payload: unknown, native: Native = f.native, storage: Files = f.storage) =>
    action(f.client, f.route, { kind: 'observe', payload }, native, storage);
  const type = (text: string, selected = range(text)) => observe(change(text, selected));
  const retire = (text = value) => {
    const receipt = { receiptId: id(1000 + count), identity: f.identity, retired: true,
      event: { ...change(text), kind: 'retired' } };
    f.receipts([receipt]); return receipt;
  };
  return { ...f, pick, nativeResume, observe, type, change, retire, frozen: () => structuredClone(frozen),
    proof: () => structuredClone([...proofs.values()].at(-1)!.event), files: (next: readonly PickerIntakeFile[]) => { files = next; } };
}

for (const kind of ['files', 'photos'] as const) test(`${kind} hands back one mount with the saved UTF16 document and real chip tokens`, async () => {
  const f = await cycleFixture(undefined, undefined, [kind === 'files' ? pickedFile : pickedImage]);
  const before = view(f.client, f.route); await f.pick(kind);
  const request = resumes(f)[0], document = obj(request.document), after = obj(obj(intakeCalls(f, 'finish')[0].publication).after);
  expect(request).toMatchObject({ generation: 3, identity: f.identity, operationId: id(1), operationRevision: 1, receiptId: id(101) });
  expect(document.value).toBe(after.text); expect(document.value).toBe(obj(f.disk().drafts)[f.key]);
  const tokens = JSON.parse(str(document.tokensJson)); expect(tokens).toHaveLength(1);
  expect(tokens[0]).toMatchObject({ start: 3, source: String(document.value).slice(tokens[0].start, tokens[0].end) });
  expect(tokens[0].label).not.toContain('unavailable');
  expect(document.selection).toEqual({ start: tokens[0].end, end: tokens[0].end });
  expect(f.order.indexOf('composerEditorFinalizations:acknowledge')).toBeLessThan(f.order.indexOf('composerEditorHandoff:resume'));
  expect(f.order.indexOf('composerPickerIntake:finish')).toBeLessThan(f.order.indexOf('composerEditorHandoff:resume'));
  const afterView = view(f.client, f.route);
  expect(afterView).toMatchObject({ kind: 'editing', editable: true, node: { key: before.node?.key } });
  expect(config(afterView)).toMatchObject({ acknowledgedEventCount: 3, document: { ...document, isNativeEcho: true } });
  expect(config(afterView).focusIntent).toEqual({ serial: '', attempt: 0, operation: 'none' });
  await f.type(`${document.value}🙂 typing`);
  expect(obj(f.disk().drafts)[f.key]).toBe(`${document.value}🙂 typing`);
});

test('shelf-only image insertion resumes the frozen empty caret', async () => {
  const f = await cycleFixture('', { start: 0, end: 0 }, [pickedImage]); await f.pick('photos');
  expect(obj(resumes(f)[0].document)).toEqual({ value: '', selection: { start: 0, end: 0 }, tokensJson: '[]', isNativeEcho: false });
  expect(view(f.client, f.route).editable).toBe(true); expect(obj(f.disk().snapshotDrafts)[f.key]).toHaveLength(1);
});

for (const outcome of ['cancelled', 'empty', 'rejected'] as const) test(`${outcome} hands back frozen source and selection after confirmed cancellation`, async () => {
  const text = outcome === 'rejected' ? 'x'.repeat(1_000_000) : '🙂 keep this';
  const f = await cycleFixture(text, { start: 3, end: 3 }, outcome === 'empty' ? [] : [pickedFile]);
  if (outcome === 'cancelled') f.intakeHook(async request => intakeReply(request, 'cancelled'));
  await f.pick();
  expect(view(f.client, f.route).editable).toBe(true);
  expect(obj(resumes(f)[0].document)).toMatchObject({ value: text, selection: { start: 3, end: 3 }, tokensJson: '[]' });
  expect(obj(f.disk().drafts)[f.key]).toBe(text); expect(intakeCalls(f, 'finish')).toHaveLength(0);
  if (outcome === 'rejected') {
    expect(f.order.indexOf('composerPickerIntake:cancel')).toBeLessThan(f.order.indexOf('composerFileHold:release'));
    expect(f.calls.filter(call => call.action === 'release')).toHaveLength(1);
  }
});

for (const lost of [false, true]) test(`${lost ? 'lost' : 'delayed'} resume reply cannot regress the highest buffered typing`, async () => {
  const f = await cycleFixture(), entered = deferred<void>(), gate = deferred<void>();
  f.resumeHook(async request => { const result = await f.nativeResume(request); entered.resolve(); await gate.promise;
    if (lost) throw Error('reply lost'); return result; });
  const picking = f.pick(); await entered.promise;
  const original = str(obj(resumes(f)[0].document).value), first = f.change(`${original}🙂 one`), second = f.change(`${original}🙂 two`);
  expect(await f.observe(second)).toMatchObject({ accepted: true });
  expect(await f.observe(first)).toMatchObject({ accepted: false });
  expect(await f.observe({ ...second, value: 'contradiction', selection: { start: 0, end: 0 } })).toMatchObject({ accepted: false });
  expect(obj(f.disk().drafts)[f.key]).toBe(original);
  gate.resolve(); await picking;
  if (lost) { f.resumeHook(f.nativeResume); await f.reread(); expect(resumes(f)[1]).toEqual(resumes(f)[0]); }
  expect(view(f.client, f.route).editable).toBe(true);
  expect(obj(f.disk().drafts)[f.key]).toBe(second.value);
  await f.type(`${second.value} newest`); const saved = f.writes.length;
  expect(await f.observe(f.proof())).toMatchObject({ accepted: true });
  expect(f.writes).toHaveLength(saved); expect(obj(f.disk().drafts)[f.key]).toBe(`${second.value} newest`);
  expect(config(view(f.client, f.route)).document).toMatchObject({ value: `${second.value} newest` });
});

for (const wrong of ['operationId', 'operationRevision', 'receiptId', 'mountId', 'owner', 'value', 'selection', 'eventCount', 'generation'] as const)
  test(`resume refuses mismatched ${wrong} without replacing its immutable request`, async () => {
    const f = await cycleFixture();
    f.resumeHook(async request => {
      const result = await f.nativeResume(request);
      if (wrong === 'generation') return { ...result, generation: 4 };
      return { ...result, value: { event: { ...result.value.event, [wrong]: wrong === 'eventCount' ? 2
        : wrong === 'operationRevision' ? 2 : wrong === 'selection' ? { start: 0, end: 0 } : wrong === 'value' ? 'wrong' : id(999) } } };
    });
    await f.pick(); expect(view(f.client, f.route).kind).toBe('frozen');
    f.resumeHook(f.nativeResume); await f.reread();
    expect(view(f.client, f.route).editable).toBe(true); expect(resumes(f)[1]).toEqual(resumes(f)[0]);
  });

test('two picks retain earlier issued holds, and Undo/Redo publishes inventory through actual persistence', async () => {
  const f = await cycleFixture(); await f.pick();
  const firstText = str(obj(resumes(f)[0].document).value), firstProof = f.proof(), oldReceipt = f.frozen();
  const second: PickerIntakeFile = { ...pickedFile, id: id(20), name: 'second.txt' };
  await f.type(firstText, { start: 0, end: firstText.length }); f.files([second]); await f.pick();
  const secondText = str(obj(resumes(f)[1].document).value);
  expect(mobileDraftAttachmentIds(f.client, f.key)).toEqual([second.id]);
  expect(resumes(f)[1]).toMatchObject({ operationRevision: 2, operationId: id(2), receiptId: id(102) });
  expect(f.calls.filter(call => call.action === 'release')).toHaveLength(0);
  await f.type(firstText);
  expect(draftFiles(f.client.local)).toMatchObject([{ id: pickedId, attachmentId: '', status: 'staged' }]);
  expect(f.disk().composerFiles).toMatchObject([{ id: pickedId, attachmentId: '', status: 'staged' }]);
  expect(savedRecord(f).context).toMatchObject({ records: [{ attachmentId: pickedId }] });
  await f.type(secondText);
  expect(mobileDraftAttachmentIds(f.client, f.key)).toEqual([second.id]);
  expect(f.disk().composerFiles).toMatchObject([{ id: second.id }]);
  expect(await f.observe(firstProof)).toMatchObject({ accepted: false });
  f.receipts([oldReceipt]); await f.reread();
  expect(view(f.client, f.route).editable).toBe(true); expect(f.disk().composerFiles).toMatchObject([{ id: second.id }]);
  expect(f.calls.filter(call => call.action === 'release')).toHaveLength(0);
});

test('canceling a second pick never releases the first accepted file hold', async () => {
  const f = await cycleFixture(); await f.pick();
  f.intakeHook(async request => intakeReply(request, 'cancelled')); await f.pick();
  expect(view(f.client, f.route).editable).toBe(true);
  expect(f.calls.filter(call => call.action === 'release')).toHaveLength(0);
  await f.type(''); expect(draftFiles(f.client.local)).toEqual([]);
  await f.type(str(obj(resumes(f)[0].document).value)); expect(f.disk().composerFiles).toMatchObject([{ id: pickedId }]);
});

for (const failed of ['source-save', 'source-ack', 'publication-save', 'finish'] as const)
  test(`${failed} cannot dispatch handback until its captured boundary is confirmed`, async () => {
    const f = await cycleFixture(); let writes = 0;
    if (failed.endsWith('save')) f.writeHook(async () => {
      if (++writes === (failed === 'source-save' ? 1 : 2)) throw Error('disk unavailable');
    });
    if (failed === 'source-ack') f.ackHook(async () => { throw Error('ack unavailable'); });
    if (failed === 'finish') f.intakeHook(async request => {
      if (request.action === 'finish') throw Error('finish unavailable'); return intakeReply(request, 'staged', [pickedFile]);
    });
    await f.pick(); expect(resumes(f)).toHaveLength(0);
    f.writeHook(null); f.ackHook(null);
    f.intakeHook(async request => intakeReply(request, request.action === 'finish' ? 'finished' : 'staged', [pickedFile]));
    await f.reread(); expect(resumes(f)).toHaveLength(1); expect(view(f.client, f.route).editable).toBe(true);
    expect(obj(f.disk().drafts)[f.key]).toBe(obj(resumes(f)[0].document).value);
  });

test('failed save of buffered typing retries without repeating resume or overwriting later source', async () => {
  const f = await cycleFixture(), entered = deferred<void>(), gate = deferred<void>();
  f.resumeHook(async request => { const result = await f.nativeResume(request); entered.resolve(); await gate.promise; return result; });
  const picking = f.pick(); await entered.promise;
  await f.type('🙂 latest unsaved'); f.writeHook(async () => { throw Error('disk full'); });
  gate.resolve(); await picking;
  expect(f.client.local.drafts[f.key]).toBe('🙂 latest unsaved'); expect(obj(f.disk().drafts)[f.key]).not.toBe('🙂 latest unsaved');
  expect(view(f.client, f.route).editable).toBe(true);
  f.writeHook(null); await f.reread();
  expect(obj(f.disk().drafts)[f.key]).toBe('🙂 latest unsaved'); expect(resumes(f)).toHaveLength(1);
  expect(f.disk().composerFiles).toEqual([]);
});

for (const change of ['route', 'generation', 'catalog', 'incarnation', 'source', 'context', 'inventory', 'epoch', 'branch'] as const)
  test(`${change} change before first resume prevents new native editing authority`, async () => {
    const f = await cycleFixture(), previousFleet = fleet.saved;
    f.intakeHook(async request => {
      if (request.action === 'finish') {
        if (change === 'route') f.detach();
        if (change === 'generation') f.client.generation++;
        if (change === 'epoch') f.client.threadEpoch++;
        if (change === 'catalog') fleet.saved = [{ environmentId: 'env', name: 'Changed', origin: 'https://changed.test' }] as typeof fleet.saved;
        const draft = mobileNewTaskDraftStore(f.client).records[f.key];
        if (change === 'incarnation') draft.createdAt = 'changed';
        if (change === 'source') f.client.local.drafts[f.key] = 'external';
        if (change === 'context') draft.context = { version: 1, records: [] };
        if (change === 'inventory') setDraftFiles(f.client.local, []);
        if (change === 'branch') draft.branchChoice = { kind: 'explicit', envMode: 'local', branch: 'changed', worktreePath: '' };
      }
      return intakeReply(request, request.action === 'finish' ? 'finished' : 'staged', [pickedFile]);
    });
    try { await f.pick(); expect(resumes(f)).toHaveLength(0); expect(intakeCalls(f, 'finish')).toHaveLength(1); }
    finally { fleet.saved = previousFleet; }
  });

test('settings changes during finish and pending resume survive both saved publications', async () => {
  const f = await cycleFixture(), entered = deferred<void>(), gate = deferred<void>();
  const first = { providerId: 'codex', modelId: 'first', modelOptions: [] }, latest = { ...first, modelId: 'latest' };
  f.intakeHook(async request => {
    if (request.action === 'finish') mobileNewTaskDraftChoicesUpdate(f.client, f.key, first);
    return intakeReply(request, request.action === 'finish' ? 'finished' : 'staged', [pickedFile]);
  });
  f.resumeHook(async request => { const result = await f.nativeResume(request); entered.resolve(); await gate.promise; return result; });
  const picking = f.pick(); await entered.promise;
  mobileNewTaskDraftChoicesUpdate(f.client, f.key, latest); await f.type('🙂 edited after settings');
  gate.resolve(); await picking;
  expect(savedRecord(f).choices).toEqual(latest); expect(obj(f.disk().drafts)[f.key]).toBe('🙂 edited after settings');
});

test('offscreen dispatched completion uses fresh IO and never enables a replacement mount', async () => {
  const f = await cycleFixture(), entered = deferred<void>(), gate = deferred<void>();
  f.resumeHook(async request => { const result = await f.nativeResume(request); entered.resolve(); await gate.promise; throw Error('lost old reply'); });
  const picking = f.pick(); await entered.promise;
  await f.type('🙂 offscreen source'); f.detach(); gate.resolve(); await picking;
  const route = await f.open('new-route'), replacement = view(f.client, route);
  f.resumeHook(f.nativeResume);
  const calls: Obj[] = [], native: Native = { available: true, watch() {}, later(input) { calls.push(obj(input)); return f.native.later(input); } };
  await action(f.client, route, { kind: 'retry' }, native, f.storage);
  expect(resumes(f)[1]).toEqual(resumes(f)[0]); expect(calls.filter(call => call.action === 'resume')).toHaveLength(1);
  expect(obj(f.disk().drafts)[f.key]).toBe('🙂 offscreen source');
  expect(view(f.client, route).node?.key).toBe(replacement.node?.key);
  expect(config(view(f.client, route)).focusIntent).toEqual({ serial: '', attempt: 0, operation: 'none' });
});

for (const proof of ['reply', 'lost'] as const) test(`${proof} handback permits a distinct final Undo receipt and retains it through save failure`, async () => {
  const f = await cycleFixture();
  if (proof === 'lost') f.resumeHook(async request => { await f.nativeResume(request); throw Error('lost'); });
  await f.pick();
  const text = str(obj(resumes(f)[0].document).value);
  if (proof === 'reply') await f.type('');
  else {
    await f.type('');
    // Confirm native handback through its event, then start a second uncertain handback.
    await f.observe(f.proof()); f.files([]); await f.pick();
  }
  const final = f.retire(text), acks = f.calls.filter(call => call.action === 'acknowledge').length;
  f.detach(); f.writeHook(async () => { throw Error('final save failed'); }); await f.reread();
  expect(f.client.local.drafts[f.key]).toBe(text); expect(draftFiles(f.client.local)).toMatchObject([{ id: pickedId }]);
  expect(f.calls.filter(call => call.action === 'acknowledge')).toHaveLength(acks);
  expect(f.calls.filter(call => call.action === 'release')).toHaveLength(0);
  f.writeHook(null); await f.reread();
  expect(obj(f.disk().drafts)[f.key]).toBe(text); expect(f.disk().composerFiles).toMatchObject([{ id: pickedId, status: 'staged', attachmentId: '' }]);
  expect(f.calls.filter(call => call.action === 'acknowledge').at(-1)?.receiptId).toBe(final.receiptId);
  expect(view(f.client, await f.resume('retired-return')).kind).toBe('mounting');
});

test('distinct retired source is handled before old slot disappearance reconciliation', async () => {
  const f = await cycleFixture(); f.resumeHook(async request => { await f.nativeResume(request); throw Error('lost'); });
  await f.pick(); const retired = f.retire('🙂 final native typing');
  f.detach(); await f.reread();
  expect(obj(f.disk().drafts)[f.key]).toBe(retired.event.value); expect(f.disk().composerFiles).toEqual([]);
  expect(f.calls.filter(call => call.action === 'acknowledge').at(-1)?.receiptId).toBe(retired.receiptId);
  const read = view(f.client, await f.resume('final-return')); expect(read.kind).toBe('mounting');
});

for (const absence of ['reserved', 'released', 'error'] as const) test(`${absence} observation alone never proves a dispatched resume`, async () => {
  const f = await cycleFixture(); f.resumeHook(async () => { throw Error('no positive handback'); }); await f.pick();
  f.receipts([]);
  if (absence === 'released') f.reserved([]);
  if (absence === 'error') f.readHook(async () => ({ ok: false, generation: 3, error: { kind: 'test', message: 'no read' } }));
  await f.reread();
  expect(view(f.client, f.route).editable).toBe(false);
  expect(await f.pick()).toMatchObject({ accepted: false });
});

test('uncertain immutable finish stays unresolved when status would be staged and settings changed', async () => {
  const f = await cycleFixture(); let first: Obj | undefined;
  f.intakeHook(async request => {
    if (request.action === 'finish') { first ??= structuredClone(request); expect(request).toEqual(first); throw Error('uncertain finish'); }
    return intakeReply(request, 'staged', [pickedFile]);
  });
  await f.pick(); const choices = { providerId: 'codex', modelId: 'new-settings', modelOptions: [] };
  mobileNewTaskDraftChoicesUpdate(f.client, f.key, choices); await f.client.persist(f.storage);
  await f.reread(); await f.reread();
  expect(resumes(f)).toHaveLength(0); expect(intakeCalls(f, 'status')).toHaveLength(0);
  expect(savedRecord(f).choices).toEqual(choices); expect(intakeCalls(f, 'finish')).toHaveLength(3);
});

function oldFile(key: string): DraftFile {
  return { id: oldId, contextId: oldId, draftKey: key, environmentId: 'env', name: 'old.txt', mimeType: 'text/plain',
    sizeBytes: 10, source: 'attached', status: 'ready', attachmentId: 'uploaded-old' };
}
function seedOldFile(client: Awaited<ReturnType<typeof fixture>>['client'], key: string) {
  setDraftFiles(client.local, [oldFile(key)]);
  client.local.drafts[key] = contextLink('file', oldId, 'old.txt');
  mobileNewTaskDraftStore(client).records[key].context = { version: 1, records: [{ version: 1, kind: 'file', label: 'old.txt', contextId: oldId,
    attachmentId: oldId, name: 'old.txt', mimeType: 'text/plain', sizeBytes: 10 }] };
}

test('saved files acquire exact holds before editing; deletion and Undo restore staged inventory without upload identity', async () => {
  const f = await fixture(seedOldFile), text = f.client.local.drafts[f.key]!;
  expect(view(f.client, f.route)).toMatchObject({ editable: false, kind: 'mounting' });
  const entered = deferred<void>(), gate = deferred<void>();
  f.holdHook(async request => { entered.resolve(); await gate.promise; return heldReply(request); });
  const preparing = action(f.client, f.route, { kind: 'observe', payload: event(f.identity, text) }, f.native, f.storage);
  await entered.promise; expect(view(f.client, f.route).editable).toBe(false);
  expect(await action(f.client, f.route, { kind: 'observe', payload: event(f.identity, '', 2, 'text') }, f.native, f.storage))
    .toMatchObject({ accepted: false });
  gate.resolve(); await preparing; expect(view(f.client, f.route).editable).toBe(true);
  expect(f.calls.find(call => call.op === 'composerFileHold')).toMatchObject({ generation: 3, identity: f.identity, file: oldFile(f.key) });
  await action(f.client, f.route, { kind: 'observe', payload: event(f.identity, '', 2, 'text') }, f.native, f.storage);
  expect(f.disk().composerFiles).toEqual([]);
  await action(f.client, f.route, { kind: 'observe', payload: event(f.identity, text, 3, 'text') }, f.native, f.storage);
  expect(f.disk().composerFiles).toEqual([{ ...oldFile(f.key), status: 'staged', attachmentId: '' }]);
  expect(obj(f.disk().mobileAttachmentOrder)[f.key]).toEqual([oldId]);
  expect(view(f.client, f.route).editable).toBe(true);
  expect(f.calls.filter(call => call.action === 'acquire')).toHaveLength(1);
});

for (const change of ['failure', 'route', 'generation', 'catalog', 'inventory', 'incarnation'] as const)
  test(`saved-file preparation cannot enable editing after ${change}`, async () => {
    const f = await fixture(seedOldFile), savedFleet = fleet.saved;
    f.holdHook(async request => {
      if (change === 'failure') throw Error('hold unavailable');
      if (change === 'route') f.detach();
      if (change === 'generation') f.client.generation++;
      if (change === 'catalog') fleet.saved = [{ environmentId: 'env', name: 'Changed', origin: 'https://changed.test' }] as typeof fleet.saved;
      if (change === 'inventory') setDraftFiles(f.client.local, [{ ...oldFile(f.key), sizeBytes: 11 }]);
      if (change === 'incarnation') mobileNewTaskDraftStore(f.client).records[f.key].createdAt = 'different';
      return heldReply(request);
    });
    try {
      await action(f.client, f.route, { kind: 'observe', payload: event(f.identity, f.client.local.drafts[f.key]!) }, f.native, f.storage);
      expect(view(f.client, f.route).editable).toBe(false); expect(resumes(f)).toHaveLength(0);
    } finally { fleet.saved = savedFleet; }
  });

test('invalid raw context remains intact during native text and final-source publication', async () => {
  const raw = { future: true, records: 'opaque' };
  const f = await fixture((client, key) => { Object.assign(mobileNewTaskDraftStore(client).records[key], { context: raw }); });
  expect(await action(f.client, f.route, { kind: 'observe', payload: event(f.identity, '🙂 text only', 2, 'text') }, f.native, f.storage))
    .toMatchObject({ accepted: true });
  expect(savedRecord(f).context).toEqual(raw);
  f.receipts([{ receiptId: id(500), identity: f.identity, retired: true,
    event: { ...event(f.identity, '🙂 last text', 3, 'text'), kind: 'retired' } }]);
  f.detach(); await f.reread();
  expect(savedRecord(f).context).toEqual(raw); expect(obj(f.disk().drafts)[f.key]).toBe('🙂 last text');
});

test('token projection uses workspace skills, actual file context, bundled icons and confirmed trailing tokens', async () => {
  const f = await cycleFixture('@"src/main.ts" $review ', { start: 0, end: 0 }, []);
  f.client.providerId = 'codex';
  f.client.config.providers = [{ instanceId: 'codex', driver: 'codex', skills: [{ name: 'review', displayName: 'Review Code' }] }];
  await f.pick();
  const tokens = JSON.parse(str(obj(resumes(f)[0].document).tokensJson));
  expect(tokens).toEqual(expect.arrayContaining([expect.objectContaining({ type: 'mention', iconUri: 't3-bundled-icon:typescript' }),
    expect.objectContaining({ type: 'skill', label: 'Review Code' })]));
  await f.type('@"src/main.ts" $review');
  expect(JSON.parse(str(obj(config(view(f.client, f.route)).document).tokensJson)))
    .toEqual(expect.arrayContaining([expect.objectContaining({ type: 'skill', label: 'Review Code' })]));
});

test('an older pending resume reply cannot change the next picker cycle', async () => {
  const f = await cycleFixture(), entered = deferred<void>(), gate = deferred<void>();
  let calls = 0;
  f.resumeHook(async request => {
    const result = await f.nativeResume(request);
    if (++calls === 1) { entered.resolve(); await gate.promise; }
    return result;
  });
  const oldPick = f.pick(); await entered.promise;
  await f.reread(); expect(view(f.client, f.route).editable).toBe(true);
  f.files([{ ...pickedFile, id: id(40), name: 'later.md' }]);
  await f.pick(); await f.type('🙂 after the second pick');
  const current = config(view(f.client, f.route)), disk = f.disk();
  gate.resolve(); await oldPick;
  expect(config(view(f.client, f.route))).toEqual(current); expect(f.disk()).toEqual(disk);
  expect(f.calls.filter(call => call.action === 'release')).toHaveLength(0);
  expect(intakeCalls(f, 'finish')).toHaveLength(2);
});

test('resume evidence received offscreen can settle buffered source without a native retry', async () => {
  const f = await cycleFixture(); f.resumeHook(async request => { await f.nativeResume(request); throw Error('reply lost'); });
  await f.pick(); const proof = f.proof(); f.detach();
  expect(await f.observe(f.change('🙂 buffered offscreen'))).toMatchObject({ accepted: true });
  const calls: Obj[] = [], fresh: Native = { available: true, watch() {}, later(request) { calls.push(obj(request)); return f.native.later(request); } };
  expect(await f.observe(proof, fresh)).toMatchObject({ accepted: true });
  expect(obj(f.disk().drafts)[f.key]).toBe('🙂 buffered offscreen');
  expect(calls.filter(call => call.action === 'resume')).toHaveLength(0);
  expect(view(f.client, f.route).editable).toBe(false);
});

test('same-mount final receipts after two completed cycles cannot be mistaken for the first saved source', async () => {
  const f = await cycleFixture(); await f.pick(); const old = f.frozen();
  f.files([]); await f.pick();
  const final = f.retire('🙂 last source after two picks'); f.detach(); await f.reread();
  expect(obj(f.disk().drafts)[f.key]).toBe(final.event.value);
  f.receipts([old]); await f.reread();
  expect(obj(f.disk().drafts)[f.key]).toBe(final.event.value);
  expect(f.calls.filter(call => call.action === 'acknowledge').map(call => call.receiptId)).toEqual([id(101), id(102), final.receiptId]);
});

test('more than 200 picker cycles follow pinned context eviction without releasing retained Undo holds', async () => {
  const f = await cycleFixture('', { start: 0, end: 0 });
  let first = '', latest = '';
  for (let n = 0; n < 201; n++) {
    if (n) await f.type(latest, { start: 0, end: latest.length });
    f.files([{ ...pickedFile, id: id(2000 + n), name: `note-${n}.md` }]);
    expect(await f.pick()).toMatchObject({ accepted: true });
    expect(view(f.client, f.route).editable).toBe(true);
    latest = str(obj(resumes(f).at(-1)!.document).value); first ||= latest;
  }
  expect(intakeCalls(f, 'hold')).toHaveLength(201);
  expect(f.calls.filter(call => call.op === 'composerFileHold' && call.action === 'acquire')).toHaveLength(0);
  expect(f.calls.filter(call => call.action === 'release')).toHaveLength(0);
  await f.type(first);
  expect(f.disk().composerFiles).toEqual([]);
  expect(mobileNewTaskContextRead(f.client, f.key)).toMatchObject({ ok: true, context: undefined });
  const tokens = JSON.parse(str(obj(config(view(f.client, f.route)).document).tokensJson));
  expect(tokens[0].label).toContain('unavailable');
});

test('lost hydration during resume prevents publication of buffered native source', async () => {
  const f = await cycleFixture(), entered = deferred<void>(), gate = deferred<void>();
  f.resumeHook(async request => { const result = await f.nativeResume(request); entered.resolve(); await gate.promise; return result; });
  const picking = f.pick(); await entered.promise;
  await f.type('🙂 buffered'); const disk = f.disk();
  f.client.local = { ...f.client.local }; gate.resolve(); await picking;
  expect(f.disk()).toEqual(disk);
  expect(await f.observe(f.proof())).toMatchObject({ accepted: false });
  expect(view(f.client, f.route).editable).toBe(false);
});

test('a let-go resume replays its exact request using fresh invocation IO', async () => {
  const f = await cycleFixture();
  f.resumeHook(async request => { await f.nativeResume(request); throw { name: 'FetchError', kind: 'Aborted' }; });
  await expect(f.pick()).rejects.toMatchObject({ kind: 'Aborted' });
  f.resumeHook(f.nativeResume);
  const calls: Obj[] = [], native: Native = { available: true, watch() {}, later(request) { calls.push(obj(request)); return f.native.later(request); } };
  await action(f.client, f.route, { kind: 'retry' }, native, f.storage);
  expect(resumes(f)[1]).toEqual(resumes(f)[0]); expect(calls.filter(call => call.action === 'resume')).toHaveLength(1);
  expect(view(f.client, f.route).editable).toBe(true); expect(intakeCalls(f, 'finish')).toHaveLength(1);
});
