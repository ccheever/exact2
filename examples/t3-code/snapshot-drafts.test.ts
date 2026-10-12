import { test, expect } from 'bun:test';
import { T3Client } from './client';
import { obj, type Obj } from './domain';
import type { Native, Files } from './protocol';
const id = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa';
function fixture() {
  const client = new T3Client();
  Object.assign(client, { generation: 1, environmentId: 'env', projectId: 'p', threadId: 't', connection: 'connected', configLive: true, shellLive: true, threadLive: true, providerId: 'fixture', modelId: 'fixture-model', scopes: ['orchestration:operate'] });
  client.config = { environment: { capabilities: { serverResolvedCommandContext: true } }, providers: [{ instanceId: 'fixture', enabled: true, installed: true, availability: 'available', auth: { status: 'authenticated' }, models: [{ slug: 'fixture-model' }] }] };
  client.shell = { sequence: 1, projects: [{ id: 'p' }], threads: [{ id: 't', projectId: 'p' }] };
  const calls: Obj[] = [], events: string[] = [];
  let failSave = false, persisted = '';
  const files: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(persisted).buffer; }, async atomicWriteFile(_path, bytes) { events.push('persist'); if (failSave) throw new Error('disk full'); persisted = new TextDecoder().decode(bytes); } } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request); events.push(String(request.op));
    let value: unknown = {};
    if (request.op === 'snapshotState') value = { captures: [{ id, owner: client.snapshotOwner }], pending: [id] };
    if (request.op === 'http' && request.path === '/api/orchestration/shell') value = { projects: client.shell.projects, threads: client.shell.threads };
    if (request.op === 'snapshotRead') value = { id, owner: request.owner, name: 'Capture.png', mimeType: 'image/png', sizeBytes: 68, source: { kind: 'snap-shot', appName: 'Fixture app', windowTitle: 'Fixture', capturedAt: '2026-10-03T00:00:00Z' } };
    if (request.op === 'snapshotDraftRead') value = { base64: 'fixture-png' };
    if (request.op === 'ids') value = ['command', 'message', 'new-thread'];
    if (request.method === 'attachments.createUploadUrl') value = { attachmentId: 'uploaded-fixture.png', relativeUrl: '/api/attachments/upload/fixture.signed', expiresAt: 999999 };
    return { ok: true, generation: 1, value };
  } };
  return { client, native, files, calls, events, failSave: () => { failSave = true; }, persisted: () => obj(JSON.parse(persisted || '{}')) };
}
test('capture is durably associated with original draft before native acknowledgement; reread deduplicates', async () => {
  const f = fixture(); await f.client.adoptSnapshots(f.native, f.files);
  expect(f.client.snapshotDrafts).toHaveLength(1);
  expect(f.events.indexOf('persist')).toBeLessThan(f.events.indexOf('snapshotAcknowledge'));
  expect(obj(f.persisted().snapshotDrafts)['env:t']).toHaveLength(1);
  await f.client.adoptSnapshots(f.native, f.files);
  expect(f.calls.filter(call => call.op === 'snapshotDraftSave')).toHaveLength(1);
});
test('failed persistence retains native pending capture without an adopted attachment', async () => {
  const f = fixture(); f.failSave(); await f.client.adoptSnapshots(f.native, f.files);
  expect(f.client.error).toBe(`Snapshot failed. Capture ${id}: disk full`);
  expect(f.client.snapshotDrafts).toEqual([]); expect(f.calls.some(call => call.op === 'snapshotAcknowledge')).toBe(false);
  // The flight is dismissed at once; the pending capture remains for a later retry.
  expect(f.calls.find(call => call.op === 'snapshotDismiss')).toMatchObject({ id });
});
test('selection change during capture read delivers only to the original existing draft', async () => {
  const f = fixture(), later = f.native.later;
  f.native.later = async request => { const value = await later(request); if (obj(request).op === 'snapshotRead') f.client.threadId = 'other'; return value; };
  await f.client.adoptSnapshots(f.native, f.files);
  expect(f.client.snapshotDrafts).toEqual([]); expect(f.client.local.snapshotDrafts['env:t']).toHaveLength(1); expect(f.calls.some(call => call.op === 'snapshotAcknowledge')).toBe(true);
});
test.each([false, true])('deferred IDs cannot send to newly selected draft (cached image=%s)', async cached => {
  const f = fixture();
  // Load before creating the local draft so first-load normalization cannot replace it.
  await f.client.command('draft', '', 'original text', 0, f.native, f.files);
  if (cached) f.client.local.snapshotDrafts[f.client.draftKey] = [{ id, name: 'Capture.png', mimeType: 'image/png', sizeBytes: 68, source: {}, uploadId: 'already-uploaded.png' }];
  const later = f.native.later;
  f.native.later = async request => { const result = await later(request); if (obj(request).op === 'ids') f.client.threadId = 'other'; return result; };
  const result = await f.client.command('send', '', 'original text', 0, f.native, f.files);
  expect(result.message).toContain('draft or model changed');
  expect(f.calls.some(call => call.method === 'orchestration.dispatchCommand' || call.method === 'orchestration.launchThread')).toBe(false);
  expect(f.client.local.drafts['env:t']).toBe('original text');
  if (cached) expect(f.client.local.snapshotDrafts['env:t']).toHaveLength(1);
});
test('stale image read performs no server write', async () => {
  const f = fixture(); await f.client.command('draft', '', 'original text', 0, f.native, f.files);
  f.client.local.snapshotDrafts[f.client.draftKey] = [{ id, name: 'Capture.png', mimeType: 'image/png', sizeBytes: 68, source: {} }];
  const later = f.native.later;
  f.native.later = async request => { const result = await later(request); if (obj(request).op === 'snapshotDraftRead') f.client.threadId = 'other'; return result; };
  const result = await f.client.command('send', '', 'original text', 0, f.native, f.files);
  expect(result.message).toContain('draft changed'); expect(f.calls.some(call => call.method === 'attachments.createUploadUrl')).toBe(false);
  expect(f.client.local.drafts['env:t']).toBe('original text');
});
test('deferred upload releases only newly minted server upload and preserves original draft', async () => {
  const f = fixture(); await f.client.command('draft', '', 'original text', 0, f.native, f.files);
  f.client.local.snapshotDrafts[f.client.draftKey] = [{ id, name: 'Capture.png', mimeType: 'image/png', sizeBytes: 68, source: {} }];
  const later = f.native.later;
  f.native.later = async request => { const result = await later(request); if (obj(request).op === 'uploadAttachment') f.client.threadId = 'other'; return result; };
  const result = await f.client.command('send', '', 'original text', 0, f.native, f.files);
  expect(result.message).toContain('draft changed'); expect(f.calls.filter(call => call.method === 'attachments.delete').map(call => obj(call.payload).attachmentId)).toEqual(['uploaded-fixture.png']);
  expect(f.calls.some(call => call.method === 'orchestration.dispatchCommand')).toBe(false);
  expect(f.client.local.snapshotDrafts['env:t']).toHaveLength(1); expect(f.client.local.drafts['env:t']).toBe('original text');
});

test('successful send releases stored image only after committed removal; failed commit retains retry file', async () => {
  for (const failCommit of [false, true]) {
    const f = fixture(); await f.client.command('draft', '', 'original text', 0, f.native, f.files);
    f.client.local.snapshotDrafts[f.client.draftKey] = [{ id, name: 'Capture.png', mimeType: 'image/png', sizeBytes: 68, source: {}, uploadId: 'ready.png' }];
    const later = f.native.later;
    f.native.later = async request => { const response = await later(request); if (obj(request).method === 'orchestration.dispatchCommand' && failCommit) f.failSave(); return response; };
    await f.client.command('send', '', 'original text', 0, f.native, f.files);
    expect(f.calls.some(call => call.op === 'snapshotDraftRemove')).toBe(!failCommit);
    if (!failCommit) expect(obj(f.persisted().snapshotDrafts)['env:t']).toEqual([]);
    else expect(f.client.local.snapshotReleases).toContain(id);
  }
});
test('saved draft association survives client recreation without capture permission', async () => {
  const f = fixture(); await f.client.adoptSnapshots(f.native, f.files);
  const reopened = new T3Client(); reopened.generation = 1; reopened.environmentId = 'env'; reopened.projectId = 'p'; reopened.threadId = 't';
  await reopened.command('draft', '', 'restored text', 0, f.native, f.files);
  expect(reopened.snapshotDrafts.map(image => image.id)).toEqual([id]);
  expect(reopened.draft).toBe('restored text');
});
test('selection changed during pending-operation persistence prevents server dispatch', async () => {
  const f = fixture(); await f.client.command('draft', '', 'original text', 0, f.native, f.files);
  const write = f.files.fs.atomicWriteFile;
  f.files.fs.atomicWriteFile = async (path, bytes) => { await write(path, bytes); if (obj(obj(JSON.parse(new TextDecoder().decode(bytes))).pending).env) f.client.threadId = 'other'; };
  const result = await f.client.command('send', '', 'original text', 0, f.native, f.files);
  expect(result.message).toContain('draft or model changed'); expect(f.calls.some(call => call.method === 'orchestration.dispatchCommand')).toBe(false);
  expect(f.client.local.drafts['env:t']).toBe('original text');
});
test('upload failure preserves image and text and never submits a provider turn', async () => {
  const f = fixture(); await f.client.command('draft', '', 'original text', 0, f.native, f.files);
  f.client.local.snapshotDrafts[f.client.draftKey] = [{ id, name: 'Capture.png', mimeType: 'image/png', sizeBytes: 68, source: {} }];
  const later = f.native.later;
  f.native.later = async request => obj(request).op === 'uploadAttachment' ? { ok: false, generation: 1, error: { message: 'Upload rejected', kind: 'HTTP', uncertain: false } } : later(request);
  const result = await f.client.command('send', '', 'original text', 0, f.native, f.files);
  expect(result.message).toContain('Upload rejected'); expect(f.client.snapshotDrafts).toHaveLength(1); expect(f.client.draft).toBe('original text');
  expect(f.calls.some(call => call.method === 'orchestration.dispatchCommand')).toBe(false);
});

test('delete succeeded then final preference commit failed: recreated client retries absent file idempotently', async () => {
  const f = fixture(); await f.client.command('draft', '', 'original text', 0, f.native, f.files);
  f.client.local.snapshotDrafts[f.client.draftKey] = [{ id, name: 'Capture.png', mimeType: 'image/png', sizeBytes: 68, source: {}, uploadId: 'ready.png' }];
  let deleted = false, failed = false;
  const write = f.files.fs.atomicWriteFile;
  f.files.fs.atomicWriteFile = async (path, bytes) => {
    if (deleted && !failed) { failed = true; throw new Error('crash before cleanup commit'); }
    if (failed) return; // Simulate termination: no later original-client write lands.
    await write(path, bytes);
  };
  const later = f.native.later;
  f.native.later = async request => { const result = await later(request); if (obj(request).op === 'snapshotDraftRemove') deleted = true; return result; };
  await f.client.command('send', '', 'original text', 0, f.native, f.files);
  expect(deleted).toBe(true); expect(f.persisted().snapshotReleases).toContain(id);
  f.files.fs.atomicWriteFile = write;
  const reopened = new T3Client(); reopened.generation = 1; reopened.environmentId = 'env'; reopened.projectId = 'p'; reopened.threadId = 't';
  await reopened.command('draft', '', 'next draft', 0, f.native, f.files);
  expect(f.calls.filter(call => call.op === 'snapshotDraftRemove')).toHaveLength(2);
  expect(f.persisted().snapshotReleases).toEqual([]); expect(reopened.snapshotDrafts).toEqual([]);
});
test('uncertain send retains files until matching server message is reconciled and committed', async () => {
  const f = fixture(); await f.client.command('draft', '', 'original text', 0, f.native, f.files);
  f.client.local.snapshotDrafts[f.client.draftKey] = [{ id, name: 'Capture.png', mimeType: 'image/png', sizeBytes: 68, source: {}, uploadId: 'ready.png' }];
  const later = f.native.later;
  f.native.later = async request => obj(request).method === 'orchestration.dispatchCommand' ? { ok: false, generation: 1, error: { kind: 'Network', message: 'ack lost', uncertain: true } } : later(request);
  await f.client.command('send', '', 'original text', 0, f.native, f.files);
  expect(f.client.pending?.uncertain).toBe(true); expect(f.calls.some(call => call.op === 'snapshotDraftRemove')).toBe(false);
  f.client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null, projection: { thread: { id: 't', projectId: 'p' }, messages: [{ id: 'message' }] } };
  await f.client.command('retry', '', '', 0, f.native, f.files);
  expect(f.client.pending).toBeUndefined(); expect(f.calls.some(call => call.op === 'snapshotDraftRemove')).toBe(true);
  expect(obj(f.persisted().snapshotDrafts)['env:t']).toEqual([]);
});


test('revoked capture grants do not prevent durable feedback choices and sound selection reenables playback', async () => {
  const f = fixture(); await f.client.command('draft', '', 'retained text', 0, f.native, f.files);
  f.client.local.deviceSettings.snapShotEnabled = true;
  const later = f.native.later;
  f.native.later = async request => obj(request).op === 'snapshotConfigure'
    ? { ok: false, generation: 1, error: { kind: 'SnapShot', message: 'Screen Recording access is required.' } }
    : later(request);
  for (const [key, value] of [['snapShotPlaySound', 'false'], ['snapShotSound', 'soft-pop'], ['snapShotSound', 'camera-shutter'], ['snapShotFlash', 'false'], ['snapShotAnimations', 'false']]) {
    const result = await f.client.command('setting-snapshot', key, value, 0, f.native, f.files);
    expect(result.message).toBe('');
    expect(obj(f.persisted().deviceSettings)[key]).toBe(key === 'snapShotSound' ? value : value === 'true');
  }
  expect(obj(f.persisted().deviceSettings)).toMatchObject({ snapShotEnabled: true, snapShotPlaySound: true, snapShotSound: 'camera-shutter', snapShotFlash: false, snapShotAnimations: false });
  expect(f.client.local.drafts['env:t']).toBe('retained text');
  expect(f.calls.some(call => call.method)).toBe(false);
  expect((await f.client.command('setting-snapshot', 'snapShotSound', 'invalid', 0, f.native, f.files)).message).toBe('Unsupported snapshot setting.');
});

test('feedback changes do not swallow nonpermission failures; preview requires actual native playback acknowledgment', async () => {
  const f = fixture(); await f.client.command('draft', '', '', 0, f.native, f.files);
  const later = f.native.later; let played = true;
  f.native.later = async request => {
    if (obj(request).op === 'snapshotConfigure') return { ok: false, generation: 1, error: { kind: 'SnapShot', message: 'Monitor installation failed.' } };
    if (obj(request).op === 'snapshotPlaySound') return { ok: true, generation: 1, value: { played } };
    return later(request);
  };
  expect((await f.client.command('setting-snapshot', 'snapShotFlash', 'false', 0, f.native, f.files)).message).toContain('Monitor installation failed');
  expect(f.client.local.deviceSettings.snapShotFlash).toBe(true);
  expect((await f.client.command('snapshot-preview-sound', '', 'soft-pop', 0, f.native, f.files)).message).toBe('Played Whoosh');
  expect((await f.client.command('snapshot-preview-sound', '', 'camera-shutter', 0, f.native, f.files)).message).toBe('Played Click');
  played = false;
  expect((await f.client.command('snapshot-preview-sound', '', 'soft-pop', 0, f.native, f.files)).message).toBe('The snapshot sound did not play.');
  const count = f.calls.length;
  expect((await f.client.command('snapshot-preview-sound', '', 'invalid', 0, f.native, f.files)).message).toBe('Choose a supported sound.');
  expect(f.calls.slice(count).some(call => call.op === 'snapshotPlaySound')).toBe(false);
});

test('snapshot shortcut aliases and literal plus match every app binding, independently of when', async () => {
  const { snapshotShortcut, snapshotConflict } = await import('./snapshot-shortcut');
  expect(snapshotShortcut('Command+SHIFT+A')).toBe('meta+shift+a');
  expect(snapshotShortcut('CTRL+ALT++')).toBe('ctrl+alt+plus');
  expect(snapshotShortcut('meta+')).toBe('');
  expect(snapshotShortcut('mod+meta+a')).toBe('');
  expect(snapshotShortcut('shift+shift')).toBe('shift+shift');
  expect(snapshotConflict({ keybindings: [{ command: 'thread.new', shortcut: { key: 'a', modKey: true, shiftKey: true }, whenAst: { type: 'identifier', name: 'inactive' } }] }, 'META+SHIFT+A')).toBe('thread.new');
});

test('snapshot shortcut cancellation, OS conflict and reset preserve saved preferences without backend writes', async () => {
  const f = fixture(); await f.client.command('draft', '', '', 0, f.native, f.files);
  const later = f.native.later; let available = true;
  f.native.later = async request => obj(request).op === 'snapshotCheckShortcut' ? { ok: true, generation: 1, value: { available, message: 'OS conflict' } } : obj(request).method === 'server.getConfig' ? { ok: true, generation: 1, value: f.client.config } : later(request);
  await f.client.command('snapshot-shortcut-record', '', 'true', 0, f.native, f.files);
  await f.client.command('snapshot-shortcut-record', '', 'false', 0, f.native, f.files);
  expect(f.client.local.deviceSettings.snapShotShortcut).toBe('shift+shift');
  expect((await f.client.command('snapshot-shortcut-save', '', 'Command+ALT+2', 0, f.native, f.files)).message).toBe('');
  expect(obj(f.persisted().deviceSettings).snapShotShortcut).toBe('meta+alt+2');
  available = false;
  expect((await f.client.command('snapshot-shortcut-save', '', 'ctrl+alt+3', 0, f.native, f.files)).message).toBe('OS conflict');
  expect(obj(f.persisted().deviceSettings).snapShotShortcut).toBe('meta+alt+2');
  available = true;
  expect((await f.client.command('snapshot-shortcut-save', '', 'shift+shift', 0, f.native, f.files)).message).toBe('');
  expect(obj(f.persisted().deviceSettings).snapShotShortcut).toBe('shift+shift');
  expect(f.calls.some(call => call.method && call.method !== 'server.getConfig')).toBe(false);
});

test('shortcut save revalidates fresh config and scope before any native replacement', async () => {
  const f = fixture(); await f.client.command('draft', '', '', 0, f.native, f.files);
  const later = f.native.later;
  f.native.later = async request => obj(request).method === 'server.getConfig' ? { ok: true, generation: 1, value: { keybindings: [{ command: 'new-binding', shortcut: { metaKey: true, key: 'a' } }] } } : later(request);
  // Reference commandLabel names the command as the keybindings list does.
  expect((await f.client.command('snapshot-shortcut-save', '', 'meta+a', 0, f.native, f.files)).message).toBe('T3 Code already uses this for "New Binding".');
  expect(f.calls.some(call => call.op === 'snapshotConfigure')).toBe(false);
  f.native.later = async request => {
    if (obj(request).method === 'server.getConfig') { f.client.projectId = 'other'; return { ok: true, generation: 1, value: {} }; }
    return later(request);
  };
  expect((await f.client.command('snapshot-shortcut-save', '', 'meta+alt+2', 0, f.native, f.files)).message).toContain('scope changed');
  expect(f.calls.some(call => call.op === 'snapshotConfigure')).toBe(false);
});

test('failed shortcut preference commit never installs an unsaved native binding', async () => {
  const f = fixture(); await f.client.command('draft', '', '', 0, f.native, f.files);
  const later = f.native.later;
  f.native.later = async request => ['server.getConfig'].includes(String(obj(request).method)) ? { ok: true, generation: 1, value: {} }
    : obj(request).op === 'snapshotCheckShortcut' ? { ok: true, generation: 1, value: { available: true } } : later(request);
  f.failSave();
  expect((await f.client.command('snapshot-shortcut-save', '', 'meta+alt+2', 0, f.native, f.files)).message).toContain('previous native binding is retained');
  expect(f.calls.some(call => call.op === 'snapshotConfigure')).toBe(false);
  expect(f.client.local.deviceSettings.snapShotShortcut).toBe('shift+shift');
  expect(obj(f.persisted().deviceSettings).snapShotShortcut).toBe('shift+shift');
});

test('A capture ready after navigation to B persists to A and never requests B focus', async () => {
  const f = fixture(); await f.client.command('draft', '', '', 0, f.native, f.files);
  const owner = f.client.snapshotOwner, later = f.native.later;
  f.client.shell.threads.push({ id: 'b', projectId: 'p' }); f.client.threadId = 'b';
  f.native.later = async request => obj(request).op === 'snapshotState' ? { ok: true, generation: 1, value: { captures: [{ id, owner }] } } : later(request);
  await f.client.adoptSnapshots(f.native, f.files);
  expect(f.client.local.snapshotDrafts['env:t']).toHaveLength(1);
  expect(f.client.snapshotDrafts).toEqual([]);
  expect(f.calls.find(call => call.op === 'snapshotAcknowledge')).toMatchObject({ owner, focus: false });
});

test('removed original destination keeps the capture pending instead of selecting another project', async () => {
  const f = fixture(), owner = f.client.snapshotOwner, later = f.native.later;
  f.native.later = async request => obj(request).op === 'http' ? { ok: true, generation: 1, value: { projects: [{ id: 'other' }], threads: [] } }
    : obj(request).op === 'snapshotState' ? { ok: true, generation: 1, value: { captures: [{ id, owner }] } } : later(request);
  await f.client.adoptSnapshots(f.native, f.files);
  expect(f.client.local.snapshotDrafts).toEqual({});
  expect(f.calls.some(call => call.op === 'snapshotDraftSave' || call.op === 'snapshotAcknowledge')).toBe(false);
  expect(f.calls.find(call => call.op === 'snapshotDismiss')).toMatchObject({ id, owner });
  expect(f.client.error).toBe('Snapshot taken, but no project is available. Add a project, then capture the window again.');
  // A pinned, vanished destination is reported once and never refetched.
  const fetches = f.calls.filter(call => call.op === 'http').length; f.client.error = '';
  await f.client.adoptSnapshots(f.native, f.files);
  expect(f.calls.filter(call => call.op === 'http').length).toBe(fetches); expect(f.client.error).toBe('');
});

test('fresh canonical destination wins over absent cached shell and no eligible project remains explicit', async () => {
  const f = fixture(), later = f.native.later; f.client.shell.projects = []; f.client.shell.threads = [];
  f.native.later = async request => obj(request).op === 'http' ? { ok: true, generation: 1, value: { projects: [{ id: 'p' }], threads: [{ id: 't', projectId: 'p' }] } } : later(request);
  await f.client.adoptSnapshots(f.native, f.files);
  expect(f.client.local.snapshotDrafts['env:t']).toHaveLength(1);
  const empty = fixture(); empty.client.projectId = ''; empty.client.threadId = ''; empty.client.shell.projects = []; empty.client.shell.threads = [];
  await empty.client.adoptSnapshots(empty.native, empty.files);
  expect(empty.client.error).toContain('no project is available');
  expect(empty.calls.some(call => call.op === 'snapshotDraftSave' || call.op === 'snapshotAcknowledge')).toBe(false);
});

test('capture with no existing draft chooses default source order only after acquisition', async () => {
  const f = fixture(); f.client.projectId = ''; f.client.threadId = '';
  const later = f.native.later; let selectionDuringRead = '';
  f.native.later = async request => { if (obj(request).op === 'snapshotRead') selectionDuringRead = f.client.projectId; return later(request); };
  await f.client.adoptSnapshots(f.native, f.files);
  expect(selectionDuringRead).toBe('');
  expect(f.client.projectId).toBe('p');
  expect(f.client.local.snapshotDrafts['env:new:p']).toHaveLength(1);
  expect(f.calls.some(call => call.method === 'orchestration.launchThread')).toBe(false);
});

test('destination deletion while durable association awaits compensates before owned file cleanup', async () => {
  const f = fixture(), write = f.files.fs.atomicWriteFile;
  let release!: () => void, entered!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; }), waiting = new Promise<void>(resolve => { entered = resolve; });
  let held = false;
  f.files.fs.atomicWriteFile = async (path, bytes) => {
    await write(path, bytes);
    const saved = obj(JSON.parse(new TextDecoder().decode(bytes)));
    if (!held && obj(saved.snapshotDrafts)['env:t']) { held = true; entered(); await gate; }
  };
  const adoption = f.client.adoptSnapshots(f.native, f.files);
  await waiting; f.client.shell.threads = []; release(); await adoption;
  expect(obj(f.persisted().snapshotDrafts)['env:t']).toEqual([]);
  expect(f.calls.some(call => call.op === 'snapshotDraftRemove' && call.id === id)).toBe(true);
  // Compensation commits before the owned copy is released, then the flight is dismissed.
  const order = f.calls.map(call => String(call.op));
  expect(order.indexOf('snapshotDraftRemove')).toBeLessThan(order.indexOf('snapshotDismiss'));
  expect(f.calls.some(call => call.op === 'snapshotAcknowledge')).toBe(false);
  expect(f.client.local.drafts).toEqual({});
});
