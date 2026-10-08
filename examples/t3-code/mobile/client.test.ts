import { describe, expect, test } from 'bun:test';
import { readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { mobileCommand, mobileNative, mobilePairingFields, mobilePairingTarget, mobilePairingUrl, mobileSnapshot } from './client';
import { ClientError, type Files, type Native } from './shared/protocol';
import { T3Client } from './shared/client';
import { requestPresentation } from './shared/requests';
import { letGoAware } from './shared/let-go';
import { obj, str, type Obj } from './shared/domain';

const unusedStorage: Files = { fs: {
  async mkdir() { throw new Error('Unexpected storage call'); },
  async readFile() { throw new Error('Unexpected storage call'); },
  async atomicWriteFile() { throw new Error('Unexpected storage call'); },
} };

function refreshFixture() {
  const client = new T3Client(), calls: Obj[] = [], events: Obj[] = [], subscriptions: Record<string, string> = {};
  let generation = 1, sequence = 0, floor = 0, serial = 0;
  const selected = { id: 'thread', projectId: 'project', modelSelection: { instanceId: 'provider', model: 'model' } };
  const config = { environment: { environmentId: 'env', orchestrationProtocolVersion: 2, capabilities: { serverResolvedCommandContext: true } },
    providers: [], shellResumeCompletionMarker: true, threadResumeCompletionMarker: true };
  const projection: Obj = { thread: selected };
  for (const family of ['runs', 'attempts', 'nodes', 'subagents', 'providerSessions', 'providerThreads', 'providerTurns', 'runtimeRequests',
    'messages', 'plans', 'turnItems', 'checkpointScopes', 'checkpoints', 'contextHandoffs', 'contextTransfers', 'visibleTurnItems']) projection[family] = [];
  const good = (value: unknown) => ({ ok: true, generation, value });
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'status') return good({ state: 'connected', origin: 'https://test.invalid', environmentId: 'env', message: '' });
    if (request.op === 'environments') return good({ saved: [] });
    if (request.op === 'http') return good(request.path === '/api/auth/session' ? { authenticated: true, scopes: ['orchestration:operate'] }
      : request.path === '/api/orchestration/shell' ? { snapshotSequence: 1, projects: [{ id: 'project', title: 'Project', workspaceRoot: '/project' }], threads: [selected] }
        : { snapshotSequence: 1, projection });
    if (request.method === 'server.getConfig') return good(config);
    if (request.op === 'subscribe') {
      const key = str(request.key), id = `sub-${++serial}`; subscriptions[key] = id;
      events.push({ seq: ++sequence, generation, key, subscriptionId: id,
        value: key === 'config' ? { type: 'snapshot', config } : { kind: 'synchronized' } });
      return good({ id });
    }
    if (request.op === 'events') return good({ events: events.filter(event => Number(event.seq) > Number(request.after)), latest: sequence, reset: Number(request.after) < floor });
    if (request.op === 'ack') { while (events.length && Number(events[0]!.seq) <= Number(request.through)) events.shift(); return good({ latest: sequence }); }
    return good({});
  } };
  return { client, native, calls, events, reconnect() { generation++; floor = sequence; events.length = 0; },
    overflow() { sequence += 100; floor = sequence; events.length = 0; } };
}

describe('mobile refresh ownership', () => {
  test('a swallowed aborted fleet read cannot watch outside the answer or write a transcript error', async () => {
    const f = refreshFixture(), original = f.native.later;
    let aborted = false, lateWatches = 0;
    f.native.later = async input => {
      if (obj(input).op === 'environments') { aborted = true; throw { name: 'FetchError', kind: 'Aborted' }; }
      return original(input);
    };
    f.native.watch = () => { if (aborted) { lateWatches++; throw new Error('native.watch outside an answer'); } };
    await f.client.refresh(letGoAware(f.native), unusedStorage);
    expect(aborted).toBe(true);
    expect(lateWatches).toBe(0);
    expect(f.client.error).toBe('');
    expect(f.calls.some(call => call.op === 'localBackendStatus')).toBe(false);
  });
  test('a new generation keeps the selected thread and reaches synchronized state', async () => {
    const f = refreshFixture();
    await f.client.refresh(f.native, unusedStorage);
    f.client.threadId = 'thread'; await f.client.openThread(f.native, 'thread');
    await f.client.refresh(f.native, unusedStorage);
    expect(f.client.ready).toBe(true);
    f.reconnect(); f.calls.length = 0;
    await f.client.refresh(f.native, unusedStorage);
    expect(f.client.threadId).toBe('thread');
    expect(obj(f.client.thread?.projection.thread).id).toBe('thread');
    expect(f.client.ready).toBe(true);
    expect(f.client.error).toBe('');
  });
  test('an overflow before the first event drain resnapshots instead of leaving synchronization incomplete', async () => {
    const f = refreshFixture(), original = f.native.later;
    let first = true;
    f.native.later = async input => {
      if (first && obj(input).op === 'events') { first = false; f.overflow(); }
      return original(input);
    };
    await f.client.refresh(f.native, unusedStorage);
    expect(f.client.ready).toBe(true);
    expect(f.client.shellLive).toBe(true);
    expect(f.calls.filter(call => call.path === '/api/orchestration/shell')).toHaveLength(2);
    await f.client.refresh(f.native, unusedStorage);
    expect(f.client.ready).toBe(true);
    expect(f.client.error).toBe('');
    expect(f.events).toEqual([]);
  });
  test('a real overflow within the same generation still resnapshots and drains completion markers', async () => {
    const f = refreshFixture();
    await f.client.refresh(f.native, unusedStorage);
    f.overflow(); f.calls.length = 0;
    await f.client.refresh(f.native, unusedStorage);
    expect(f.calls.filter(call => call.path === '/api/orchestration/shell')).toHaveLength(1);
    expect(f.calls.some(call => call.op === 'ack' && Number(call.through) >= 100)).toBe(true);
    expect(f.client.ready).toBe(true);
    expect(f.events).toEqual([]);
  });
});

describe('mobile thread selection error ownership', () => {
  const storage: Files = { fs: {
    async mkdir() {}, async readFile() { return new TextEncoder().encode('{}').buffer; }, async atomicWriteFile() {},
  } };
  async function ready() { const f = refreshFixture(); await f.client.refresh(f.native, storage); return f; }
  function oldSelection(f: ReturnType<typeof refreshFixture>) {
    const original = f.native.later;
    f.native.later = async input => {
      const response = await original(input);
      return str(obj(input).path).endsWith('/bounded') ? { ...obj(response), generation: 0 } : response;
    };
    return () => { f.native.later = original; };
  }
  const select = (f: ReturnType<typeof refreshFixture>) => f.client.command('select-thread', 'thread', '', 0, f.native, storage);

  test('stale selection returns its route error; successful retry and refresh have no composer notice', async () => {
    const f = await ready(), restore = oldSelection(f);
    const failed = await select(f);
    expect(failed.message).toBe('The connection changed. Refresh before continuing.');
    expect(f.client.thread).toBeNull();
    expect(requestPresentation(f.client).error).toBe('');
    restore();
    expect((await select(f)).message).toBe('');
    await f.client.refresh(f.native, storage);
    expect(obj(f.client.thread?.projection.thread).id).toBe('thread');
    expect(f.client.ready).toBe(true);
    expect(requestPresentation(f.client).error).toBe('');
  });

  test('current selection failures remain available to the route alert', async () => {
    const f = await ready(), original = f.native.later;
    f.native.later = async input => str(obj(input).path).endsWith('/bounded')
      ? { ok: false, generation: 1, error: { kind: 'Permission', message: 'Thread read permission denied.', uncertain: false } }
      : original(input);
    expect((await select(f)).message).toBe('Thread read permission denied.');
    expect(requestPresentation(f.client).error).toBe('');
  });

  test('failed and successful selections preserve an unrelated existing notice', async () => {
    const f = await ready(); f.client.error = 'Could not save local drafts.';
    const restore = oldSelection(f);
    expect((await select(f)).message).not.toBe('');
    expect(f.client.error).toBe('Could not save local drafts.');
    restore();
    expect((await select(f)).message).toBe('');
    expect(requestPresentation(f.client).error).toBe('Could not save local drafts.');
  });

  test('a real uncertain write remains pending and visible through selection failure and retry', async () => {
    const f = await ready(), original = f.native.later;
    f.native.later = async input => {
      if (obj(input).method === 'orchestration.dispatchCommand') throw new Error('Connection lost after dispatch.');
      return original(input);
    };
    const pending = { method: 'orchestration.dispatchCommand', payload: { type: 'thread.message.send', commandId: 'write-command', threadId: 'thread' },
      description: 'Send message', threadId: 'thread', text: 'Keep my draft', uncertain: false };
    await expect(f.client.write(f.native, storage, pending)).rejects.toThrow('Connection lost after dispatch.');
    expect(pending.uncertain).toBe(true);
    const writeError = f.client.error;
    expect(writeError).not.toBe('');
    f.native.later = original;
    const restore = oldSelection(f);
    expect((await select(f)).message).not.toBe('');
    expect(f.client.pending).toBe(pending);
    expect(requestPresentation(f.client).error).toBe(writeError);
    restore();
    expect((await select(f)).message).toBe('');
    expect(f.client.pending).toBe(pending);
    expect(pending.uncertain).toBe(true);
    expect(requestPresentation(f.client).error).toBe(writeError);
  });
});

describe('pinned shared sources', () => {
  test('every TS copy matches its immutable pin apart from explicit mobile adaptations', () => {
    const directory = new URL('./shared/', import.meta.url).pathname;
    const names: string[] = [];
    function visit(dir: string) {
      for (const entry of readdirSync(dir, { withFileTypes: true })) {
        if (entry.isDirectory()) visit(join(dir, entry.name));
        else if (entry.name.endsWith('.ts')) names.push(relative(directory, join(dir, entry.name)));
      }
    }
    visit(directory);
    expect(names.length).toBe(297);
    const copies = names.map(name => {
      const local = readFileSync(join(directory, name), 'utf8').split('\n');
      expect(local[0]).toContain('GAP 001');
      const pin = name === 'shell-vcs.ts' ? '81c704c7d12afef7233b12b1f2e7118fd6e84677' : name === 'let-go.ts' ? '669968e248e3a3ca29dbfeada999af2114141223'
        : ['client.ts', 'local-backend.ts', 'timestamp-format.ts'].includes(name)
          ? '38352ceaf4cd35a40b7b24ce992db87c2357a99b' : '887b2491b182f851b11253655f6aa84fe2a26708';
      const adapted = ['client.ts', 'client-ops-composer.ts', 'project-clones-live.ts', 'r8-pointer-reconnect.ts', 'r4-git-branch.ts', 'composer-editor.ts'].includes(name);
      expect(local[1]).toBe(`// ${adapted ? 'Adapted' : 'Unchanged'} body from examples/t3-code/${name} at ${pin}.`);
      return { name, local, pin };
    });
    const objects = copies.map(copy => `${copy.pin}:examples/t3-code/${copy.name}`);
    objects.push('758e03d8c48f086698e9cbf2da838da7ccb16ad2:examples/t3-code/composer-editor.ts');
    const result = Bun.spawnSync(['git', 'cat-file', '--batch'], {
      cwd: directory, stdin: new TextEncoder().encode(objects.join('\n') + '\n'),
    });
    expect(result.exitCode).toBe(0);
    // Git's batch protocol reports UTF-8 body lengths in bytes, not JavaScript characters.
    const bytes = result.stdout, decoder = new TextDecoder(); let offset = 0;
    const bodies = objects.map(source => {
      const newline = bytes.indexOf(10, offset);
      const header = decoder.decode(bytes.subarray(offset, newline));
      expect(header, source).toMatch(/^[0-9a-f]{40} blob \d+$/);
      const size = Number(header.split(' ')[2]); offset = newline + 1;
      const body = decoder.decode(bytes.subarray(offset, offset + size)); offset += size;
      expect(bytes[offset++]).toBe(10);
      return body;
    });
    expect(offset).toBe(bytes.length);
    for (const [index, { name, local }] of copies.entries()) {
      let expected = bodies[index]!;
      if (name === 'client.ts') expected = "// Mobile 365aa87982: selection errors belong to the requesting route, not the thread composer.\n// Additive cleanup visibility from shared commit af0a96dddbd500aa50bc5bbe69ec59597e34efee.\n" + expected
        .replace("const formCommand = ['settings-core',", "const formCommand = ['select-thread', 'settings-core',")
        .replace("  private finishPending(", "  protected finishPending(");
      if (name === 'client-ops-composer.ts') expected = "// Mobile 365aa87982: send admission, retained options and independent model-pick ownership differ.\nimport { mobileNewTaskDirectText, mobileNewTaskMessageContext as withMessageContext, mobileNewTaskSendGuard } from '../mobile-new-task-context-send';\nimport { mobileModelSelectionUnavailable } from '../model-availability';\nimport { mobileDispatchSelection as dispatchSelection } from '../model-send-selection';\n" + expected
        .replace("import { dispatchSelection, promptForSend, ultrathinkChoice }", "import { promptForSend, ultrathinkChoice }")
        .replace("if (!arr(provider.models).some(model => model.slug === this.modelId)) throw new ClientError('Choose one of the models advertised by T3.');",
          "if (!this.modelId || mobileModelSelectionUnavailable(this.config, { instanceId: this.providerId, model: this.modelId })) throw new ClientError('Model unavailable. Open model settings.');")
        .replace("  if (op === 'model' && await additiveGesture(this, native)) {", `  const owner = JSON.stringify([this.origin, this.environmentId, this.generation, this.projectId, this.threadId, this.draftKey]);
  const independent = this.draftKey.startsWith('new-task:');
  const additive = op === 'model' && await additiveGesture(this, native);
  // additiveGesture tolerates native errors. Recheck before it can mutate a newer draft.
  if (independent && owner !== JSON.stringify([this.origin, this.environmentId, this.generation, this.projectId, this.threadId, this.draftKey]))
    throw new ClientError('The draft changed while settings were saving.', 'superseded');
  if (additive) {`)
        .replace("import { withMessageContext } from './composer-editor';\n", '')
        .replace('async function send(this: T3Client, native: Native, storage: Files, value: string): Promise<void> {', 'async function send(this: T3Client, native: Native, storage: Files, value: string): Promise<void> {\n  const assertDraft = mobileNewTaskSendGuard(this);')
        .replace('const terminalSubmission = omitExpiredTerminalContexts(this, rawText, this.snapshotDrafts.length > 0);', 'const terminalSubmission = mobileNewTaskDirectText(this, rawText) ?? omitExpiredTerminalContexts(this, rawText, this.snapshotDrafts.length > 0);')
        .replace('  assertOwner();\n  const [commandId,', '  assertOwner(); assertDraft();\n  const [commandId,')
        .replace('  assertOwner();\n  if (selection.threadId)', '  assertOwner(); assertDraft();\n  if (selection.threadId)')
        .replace("description: 'Create thread', threadId, text, uncertain: false }, assertOwner);", "description: 'Create thread', threadId, text, uncertain: false }, () => { assertOwner(); assertDraft(); });")
        .replace('} else if (fanoutSelections(this)) {', "} else if (!this.draftKey.startsWith('new-task:') && fanoutSelections(this)) {");

      if (name === 'composer-editor.ts') {
        const source = bodies[copies.length]!;
        const pick = source.slice(source.indexOf('const contextPickStamp ='), source.indexOf('const CLOSED:'))
          .replace('client.snapshotOwner, client.draftKey, client.draft]);', 'client.snapshotOwner, client.draftKey, mobileNewTaskDraftCurrent(client)?.createdAt, client.draft]);');
        expected = "// Mobile additive context selection export from shared commit 758e03d8c48f086698e9cbf2da838da7ccb16ad2.\nimport { mobileNewTaskDraftCurrent } from '../mobile-new-task-drafts';\n" + expected
          .replace('  trigger: EditorTrigger | null; rows: MenuRow[]; owner: string;', '  trigger: EditorTrigger | null; rows: MenuRow[]; owner: string;\n  pickStamp?: string;')
          .replace('const CLOSED:', pick + 'const CLOSED:')
          .replace("async function editorView(client: T3Client, native: Native | null | undefined, now: number): Promise<Omit<ComposerEditorView, 'drawer'>> {\n  const entry = cache(client);", "async function editorView(client: T3Client, native: Native | null | undefined, now: number): Promise<Omit<ComposerEditorView, 'drawer'>> {\n  const pickStamp = contextPickStamp(client), entry = cache(client);")
          .replace('  entry.rows = rows;', "  entry.rows = rows;\n  entry.pickStamp = contextPickStamp(client) === pickStamp ? pickStamp : '';");
      }
      if (name === 'project-clones-live.ts') expected = "// Mobile 365aa87982 apps/mobile/src/state/projectClones.ts: failed subscriptions read as empty.\n" + expected
        .replace("  const clones = liveEnvironment(client, null, client.environmentId)?.clones.value ?? [];",
          "  const stream = liveEnvironment(client, null, client.environmentId)?.clones;\n  const clones = stream?.error ? [] : stream?.value ?? [];");
      if (name === 'r8-pointer-reconnect.ts') expected = "// Mobile: interrupted catalog reads do not consume reconnect-on-launch; only the newest read may connect.\n" + expected
        .replace('const asked = new WeakSet<object>();', 'const asked = new WeakSet<object>();\nconst reading = new WeakMap<object, object>();')
        .replace("  asked.add(client);\n  if (str(status.state) !== 'disconnected' || str(status.environmentId)) return false;",
          "  if (str(status.state) !== 'disconnected' || str(status.environmentId)) {\n    asked.add(client);\n    return false;\n  }\n  const attempt = {};\n  reading.set(client, attempt);")
        .replace('  const target = relaunchTarget(saved, str(status.origin));',
          '  if (asked.has(client) || reading.get(client) !== attempt) return false;\n  reading.delete(client);\n  asked.add(client);\n  const target = relaunchTarget(saved, str(status.origin));');
      if (name === 'r4-git-branch.ts') expected = "// Mobile adaptation: a guarded read leaves draft-context adoption to its captured owner.\n" + expected
        .replace("async function loadRefs(client: T3Client, native: Native, cwd: string, query: string): Promise<Refs> {\n", "async function loadRefs(client: T3Client, native: Native, cwd: string, query: string, currentOwner?: () => boolean): Promise<Refs> {\n")
        .replace("  if (current && current.cwd === cwd && current.query === query && current.generation === client.generation && !current.stale)\n    return Object.assign(current, await morePages(current, client.presentation, 'details-refs', list));\n", "  if (current && current.cwd === cwd && current.query === query && current.generation === client.generation && !current.stale) {\n    const page = await morePages(current, client.presentation, 'details-refs', list);\n    if (currentOwner && !currentOwner()) throw new ClientError('The selected workspace changed.', 'superseded');\n    return Object.assign(current, page);\n  }\n")
        .replace("  const next: Refs = { cwd, query, ...firstPage(result, scrollEnds(client.presentation, 'details-refs')), loaded: true, generation: client.generation, stale: false };\n", "  if (currentOwner && !currentOwner()) throw new ClientError('The selected workspace changed.', 'superseded');\n  const next: Refs = { cwd, query, ...firstPage(result, scrollEnds(client.presentation, 'details-refs')), loaded: true, generation: client.generation, stale: false };\n")
        .replace("export async function cardBranchView(client: T3Client, native: Native, cwd: string, root: string, isRepo: boolean) {\n", "export async function cardBranchView(client: T3Client, native: Native, cwd: string, root: string, isRepo: boolean, currentOwner?: () => boolean) {\n")
        .replace("  let refs: Refs | null = null;\n", "  if (currentOwner && !currentOwner()) throw new ClientError('The selected workspace changed.', 'superseded');\n  let refs: Refs | null = null;\n")
        .replace("  try { refs = await loadRefs(client, native, cwd, state.query.trim()); } catch { refs = state.refs && state.refs.cwd === cwd ? state.refs : null; }\n", "  try { refs = await loadRefs(client, native, cwd, state.query.trim(), currentOwner); } catch { refs = state.refs && state.refs.cwd === cwd ? state.refs : null; }\n  if (currentOwner && !currentOwner()) throw new ClientError('The selected workspace changed.', 'superseded');\n")
        .replace("  if (!client.threadId && !selectingBase && strip.branch && context.branch !== strip.branch) {\n", "  if (!currentOwner && !client.threadId && !selectingBase && strip.branch && context.branch !== strip.branch) {\n");
      expect(local.slice(2).join('\n')).toBe(expected);
    }
  });
});

describe('upstream mobile pairing forms', () => {
  test('unwraps mobile QR/deep links before the shared hosted parser', () => {
    const nested = 'https://t3.codes/pair?host=https%3A%2F%2Fmy-server.example#token=one-time';
    const input = `t3code://connect?pairingUrl=${encodeURIComponent(nested)}`;
    expect(mobilePairingUrl(input)).toBe(nested);
    expect(mobilePairingFields(input)).toEqual({ source: input, host: 'https://my-server.example', code: 'one-time' });
    expect(mobilePairingTarget(input, '')).toEqual({ origin: 'https://my-server.example', credential: 'one-time' });
  });
  test('accepts direct URLs, hash before query tokens, and a URL pasted into code', () => {
    const input = 'https://server.example/path?token=query#token=hash';
    expect(mobilePairingFields(input)).toEqual({ source: input, host: 'https://server.example', code: 'hash' });
    expect(mobilePairingTarget('unused.example', input)).toEqual({ origin: 'https://server.example', credential: 'hash' });
  });
  test('uses HTTP for bare IP + code, HTTPS for a bare DNS name', () => {
    expect(mobilePairingTarget('127.0.0.1:3773', 'token')).toEqual({ origin: 'http://127.0.0.1:3773', credential: 'token' });
    expect(mobilePairingTarget('[::1]:3773', 'token')).toEqual({ origin: 'http://[::1]:3773', credential: 'token' });
    expect(mobilePairingTarget('server.example:3773', 'token')).toEqual({ origin: 'https://server.example:3773', credential: 'token' });
    expect(mobilePairingTarget('https://127.0.0.1:3773', 'token').origin).toBe('https://127.0.0.1:3773');
  });
  test('host-only parsing preserves mobile form text and strips URL paths', () => {
    expect(mobilePairingFields('server.example')).toEqual({ source: 'server.example', host: 'server.example', code: '' });
    expect(mobilePairingFields('https://server.example/path?query=yes')).toEqual({ source: 'https://server.example/path?query=yes', host: 'https://server.example', code: '' });
    expect(mobilePairingFields('')).toEqual({ source: '', host: '', code: '' });
  });
  test('invalid addresses do not echo a pairing code in an error', () => {
    expect(() => mobilePairingTarget('not a host', 'secret-code')).toThrow('Enter a valid T3 server address or pairing link.');
    expect(() => mobilePairingTarget('ftp://server.example', 'secret-code')).toThrow(ClientError);
  });
});

describe('mobile wire attribution', () => {
  test('changes only newly authored orchestration payloads without mutating shared state', async () => {
    const requests: unknown[] = [], topics: string[] = [];
    const native = mobileNative({ available: true, watch: topic => topics.push(topic), later: async request => { requests.push(request); return { ok: true, generation: 9, value: {} }; } });
    const request = { op: 'request', method: 'orchestration.dispatchCommand', generation: 9, payload: { type: 'message.dispatch', creationSource: 'web', commandId: 'same-id', text: 'hello' } };
    const result = await native.later(request);
    expect(request.payload.creationSource).toBe('web');
    expect(requests[0]).toEqual({ ...request, payload: { ...request.payload, creationSource: 'mobile' } });
    expect(result).toEqual({ ok: true, generation: 9, value: {} });
    const launch = { op: 'request', method: 'orchestration.launchThread', payload: { creationSource: 'web', threadId: 'thread' } };
    await native.later(launch);
    expect(requests[1]).toEqual({ ...launch, payload: { ...launch.payload, creationSource: 'mobile' } });
    const other = { op: 'request', method: 'server.updateSettings', payload: { creationSource: 'web' } };
    await native.later(other);
    expect(requests[2]).toBe(other);
    const serverCreated = { ...request, payload: { ...request.payload, creationSource: 'server' } };
    await native.later(serverCreated);
    expect(requests[3]).toBe(serverCreated);
    native.watch('t3.events');
    expect(topics).toEqual(['t3.events']);
  });
  test('preserves native rejection identity for let-go handling', async () => {
    const error = { name: 'FetchError', kind: 'Aborted' };
    const native = mobileNative({ available: true, watch() {}, later: async () => { throw error; } });
    await expect(native.later({ op: 'status' })).rejects.toBe(error);
  });
});

describe('bake and command boundary', () => {
  test('bake snapshot is disconnected without reading files or manufacturing environments', async () => {
    const snapshot = await mobileSnapshot(undefined, unusedStorage);
    expect(snapshot.nativeAvailable).toBe(false);
    expect(snapshot.ready).toBe(false);
    expect(snapshot.environments).toEqual([]);
    expect(snapshot.projects).toEqual([]);
    expect(snapshot.origin).toBe('');
    expect(JSON.stringify(snapshot)).not.toContain('access_token');
  });
  test('unavailable host and invalid mobile addresses never dispatch native work', async () => {
    expect((await mobileCommand(['connect'], undefined, unusedStorage)).message).toContain('iPhone or iPad');
    const requests: unknown[] = [];
    const native: Native = { available: true, watch() {}, later: async request => { requests.push(request); throw new Error('unexpected'); } };
    expect((await mobileCommand(['connect', 'not a host', 'secret-code'], native, unusedStorage)).message).toContain('valid T3 server');
    expect((await mobileCommand(['environment-ssh-connect'], native, unusedStorage)).message).toContain('desktop app');
    expect(requests).toEqual([]);
  });
});

describe('mobile environment editing', () => {
  test('saves local connection fields through the native catalog, never server update', async () => {
    const requests: unknown[] = [];
    const native: Native = { available: true, watch() {}, later: async request => {
      requests.push(request); return { ok: true, generation: 0, value: { saved: [] } };
    } };
    const result = await mobileCommand(['save-environment', 'environment-one', JSON.stringify({ label: ' My Mac ', url: 'https://server.example/path' })], native, unusedStorage);
    expect(result.message).toBe('');
    expect(requests).toEqual([{ op: 'mobileUpdateEnvironment', environmentId: 'environment-one', label: 'My Mac', origin: 'https://server.example' }]);
  });
  test('rejects blank labels, pairing tokens and malformed settings before native work', async () => {
    let calls = 0;
    const native: Native = { available: true, watch() {}, later: async () => { calls++; throw new Error('unexpected'); } };
    expect((await mobileCommand(['save-environment', 'env', JSON.stringify({ label: ' ', url: 'https://host.example' })], native, unusedStorage)).message).toContain('label cannot be empty');
    expect((await mobileCommand(['save-environment', 'env', JSON.stringify({ label: 'Host', url: 'https://host.example#token=secret' })], native, unusedStorage)).message).toContain('without a pairing code');
    expect((await mobileCommand(['save-environment', 'env', '{'], native, unusedStorage)).message).toBe('The environment settings are invalid.');
    expect(calls).toBe(0);
  });
});

describe('GitHub routing permissions', () => {
  test('shows a single switched-off saved environment and reads trust by its route identity', async () => {
    const { mobileRoutingRows } = await import('./client');
    const { environmentSources } = await import('./shared/connections');
    const { T3Client } = await import('./shared/client');
    const { gitHubRoutingConnectionKey } = await import('./shared/connection-routes');
    const saved = [{ origin: 'https://host.example', environmentId: 'env', label: 'Host', enabled: false, routes: [{ id: 'direct', origin: 'https://host.example', credential: 'https://host.example' }] }];
    const sources = environmentSources(new T3Client(), saved, new Map());
    const key = gitHubRoutingConnectionKey(sources[0]!);
    expect(key).not.toBeNull();
    const preferences = JSON.stringify({ githubRouting: { [key!]: 'read-write' } });
    expect(mobileRoutingRows(sources, saved, preferences, true)).toEqual([
      { id: 'https://host.example\nenv', label: 'Host', url: 'https://host.example/', permission: 'read-write', disabled: false },
    ]);
    expect(mobileRoutingRows(sources, saved, preferences, false)[0]?.disabled).toBe(true);
    const changed = [{ ...sources[0]!, routes: [{ id: 'direct', origin: 'https://new-host.example' }] }];
    expect(mobileRoutingRows(changed, saved, preferences, true)[0]?.permission).toBe('off');
  });
});

describe('mobile environment retry ownership', () => {
  test('retries only the saved background transport and preserves focus', async () => {
    const { mobileClient } = await import('./client');
    const focused = mobileClient.environmentId, origin = mobileClient.origin;
    const calls: Record<string, unknown>[] = [];
    const native: Native = { available: true, watch() {}, async later(input) {
      const request = input as Record<string, unknown>; calls.push(request);
      return { ok: true, generation: 7, value: request.op === 'environments'
        ? { saved: [{ origin: 'https://background.example', environmentId: 'background', enabled: true }] } : {} };
    } };
    const result = await mobileCommand(['environment-reconnect', 'https://background.example\nbackground'], native, unusedStorage);
    expect(result.message).toBe('');
    expect(calls).toEqual([{ op: 'environments' }, { op: 'retry', fleet: 'https://background.example\nbackground' }]);
    expect(mobileClient.environmentId).toBe(focused);
    expect(mobileClient.origin).toBe(origin);
  });
  test('does not open an unknown or disabled saved environment', async () => {
    const calls: unknown[] = [];
    const native: Native = { available: true, watch() {}, async later(input) {
      calls.push(input); return { ok: true, generation: 0, value: { saved: [
        { origin: 'https://disabled.example', environmentId: 'disabled', enabled: false },
      ] } };
    } };
    for (const key of ['https://unknown.example\nunknown', 'https://disabled.example\ndisabled']) {
      expect((await mobileCommand(['environment-reconnect', key], native, unusedStorage)).message).toContain('Switch on');
    }
    expect(calls).toEqual([{ op: 'environments' }, { op: 'environments' }]);
  });
});
