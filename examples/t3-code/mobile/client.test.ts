import { describe, expect, test } from 'bun:test';
import { readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { mobileClient, mobileCommand, mobileNative, mobilePairingFields, mobilePairingPrefill, mobilePairingTarget, mobilePairingUrl, mobileSnapshotRead, mobileSnapshotProjection } from './client';
import { bridgeReply, ClientError, type Files, type Native } from './shared/protocol';
import { T3Client } from './shared/client';
import { MobileDraftClient } from './mobile-draft-recovery';
import { requestPresentation } from './shared/requests';
import { letGoAware } from './shared/let-go';
import { obj, str, type Obj } from './shared/domain';
import { homeConnectionTitle } from './home-chrome';
import { mobileHomeAction, mobileHomeActionsObserve, mobileHomeMenu } from './home-actions';

const unusedStorage: Files = { fs: {
  async mkdir() { throw new Error('Unexpected storage call'); },
  async readFile() { throw new Error('Unexpected storage call'); },
  async atomicWriteFile() { throw new Error('Unexpected storage call'); },
} };

function refreshFixture(client: T3Client = new T3Client()) {
  const calls: Obj[] = [], events: Obj[] = [], subscriptions: Record<string, string> = {};
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
  return { client, native, calls, events, generation: () => generation, reconnect() { generation++; floor = sequence; events.length = 0; },
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

describe('mobile connection feedback', () => {
  // Mobile 365aa87982: connectionError is transport only (remote-environment-projections.ts); a read-only
  // connection says so when an action is attempted (useThreadListActions.ts), not in the Home header.
  async function refreshed(scopes: string[], resolved: boolean, authenticated = true) {
    const f = refreshFixture(), original = f.native.later;
    f.native.later = async input => {
      const request = obj(input), response = obj(await original(input));
      if (request.path === '/api/auth/session') return { ...response, value: { authenticated, scopes } };
      if (request.method !== 'server.getConfig') return response;
      const config = obj(response.value);
      return { ...response, value: { ...config, environment: { ...obj(config.environment), capabilities: { serverResolvedCommandContext: resolved } } } };
    };
    await f.client.refresh(f.native, unusedStorage);
    return f;
  }
  const title = (client: T3Client) => homeConnectionTitle({ connectionError: client.error || null, connectingEnvironments: [], hasConnectingEnvironment: false,
    hasPendingShellSnapshot: client.connection === 'connected' && !client.shellLoaded, hasLoadedShellSnapshot: client.shellLoaded, hasReadyEnvironment: client.ready }).label;
  const read = ['orchestration:read', 'filesystem:read', 'diagnostics:read', 'terminal:read'];
  test('a ready read-only connection has no Home status and still refuses thread actions', async () => {
    for (const resolved of [true, false]) {
      const f = await refreshed(read, resolved);
      expect(f.client.ready).toBe(true);
      expect(f.client.error).toBe('');
      expect(title(f.client)).toBe('');
      mobileHomeActionsObserve('home', true, false, f.client); f.calls.length = 0;
      expect((await mobileHomeAction('home', 'env', 'thread', 'pin', '', 0, f.native, f.client)).message).toBe('This connection cannot change threads.');
      expect(f.calls.filter(call => call.method === 'orchestration.dispatchCommand')).toEqual([]);
    }
  });
  test('an old writable server and a refused session keep their Home status', async () => {
    const writable = await refreshed(['orchestration:operate', ...read], true);
    expect(title(writable.client)).toBe('');
    expect(mobileHomeMenu('env', 'thread', 0, writable.client).length).toBeGreaterThan(0);
    const old = await refreshed(['orchestration:operate', ...read], false);
    expect(old.client.ready).toBe(true);
    expect(title(old.client)).toBe('Update the T3 server to send messages from this client. Existing threads remain readable.');
    const refused = await refreshed(read, true, false);
    expect(refused.client.ready).toBe(false);
    expect(title(refused.client)).toBe('Pair with the T3 server again.');
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
    const generated = new Set(['r12-render-grammar.ts', 'r12-render-grammar-more.ts', 'r7-polish-css-words.ts', 'settings-b-lucide.ts', 'file-markdown-parser.d.ts']);
    const names: string[] = [];
    function visit(dir: string) {
      for (const entry of readdirSync(dir, { withFileTypes: true })) {
        if (entry.isDirectory()) visit(join(dir, entry.name));
        else if (entry.name.endsWith('.ts') && !generated.has(relative(directory, join(dir, entry.name)))) names.push(relative(directory, join(dir, entry.name)));
      }
    }
    visit(directory);
    expect(names.length).toBe(294);
    const copies = names.map(name => {
      const local = readFileSync(join(directory, name), 'utf8').split('\n');
      expect(local[0]).toContain('GAP 001');
      const pin = name === 'pages-usage.ts' ? 'c65b209d587e246d05f2fd6fd67fd12d1d6fa5a0'
        : name === 'word-diff.ts' ? '633320b410683cc88018c8f66bc908177fffd0f4'
        : name === 'timeline-t3tools.ts' ? 'eefa4fe3744d6c4e0dc5c9f0a84bdfb9d3961ba4'
        : name === 'shell-vcs.ts' ? 'c280acc1806c4e1fea5d4797b1278b99b5ee2ba0' : name === 'let-go.ts' ? '669968e248e3a3ca29dbfeada999af2114141223'
        : ['client.ts', 'local-backend.ts', 'timestamp-format.ts'].includes(name)
          ? '38352ceaf4cd35a40b7b24ce992db87c2357a99b' : '887b2491b182f851b11253655f6aa84fe2a26708';
      const adapted = ['client.ts', 'client-ops-composer.ts', 'composer-controls.ts', 'domain.ts', 'project-clones-live.ts', 'r8-pointer-reconnect.ts', 'r4-git-branch.ts', 'composer-editor.ts', 'timeline-rows.ts', 'timeline-presentation.ts', 'protocol.ts', 'r5-composer-citation.ts'].includes(name);
      if (name === 'word-diff.ts') expect(local.slice(0, 3)).toEqual([
        '// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.',
        '// Unchanged body from examples/t3-code/word-diff.ts; engine/span source extracted from',
        '// settings-font-previews.ts at 5b7eb54af3aa972f92d7fb62ce809f433a8774cb.',
      ]);
      else if (name !== 'pages-usage.ts') expect(local[1]).toBe(`// ${adapted ? 'Adapted' : 'Unchanged'} body from examples/t3-code/${name} at ${pin}.`);
      return { name, local, pin };
    });
    const objects = copies.map(copy => copy.name === 'pages-usage.ts' ? copy.pin : `${copy.pin}:examples/t3-code/${copy.name}`);
    objects.push('db560228e1cb2ad00c9cbff76ed8d22a05438249:examples/t3-code/composer-editor.ts');
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
      if (name === 'pages-usage.ts') {
        expect(local.join('\n')).toBe(expected);
        continue;
      }
      if (name === 'domain.ts') expected = '// Mobile365aa87982: successful older-page adoption excludes expanded history from disk caching.\n' + expected
        .replace('  latestLocalTurnOrdinal: number | null;\n}', '  latestLocalTurnOrdinal: number | null;\n  /** Absent on a fresh snapshot. Live events preserve successful load-earlier growth. */\n  historyExpanded?: true;\n}')
        .replace('    ...state, historyCursor, hasMore,', '    ...state, historyCursor, hasMore, historyExpanded: true,');
      if (name === 'protocol.ts') expected = '// Mobile: release the fully assembled native value and fragment references before JSON decoding.\n'
        + '// @ref examples/t3-code/mobile/docs/1130.003-pairing-and-transport.decision.md#complete-transfer-decoding-2026-10-10\n' + expected
          .replace('  const fragments: string[] = [];', "  const fragments: string[] = [];\n  let text = '';")
          .replace("    return { ...response, value: JSON.parse(fragments.join('')) as unknown };", "    text = fragments.join('');")
          .replace("  } finally {\n    await native.later({ op: 'releaseChunk', id, generation }).catch(() => {});\n  }",
            "  } finally {\n    fragments.length = 0;\n    await native.later({ op: 'releaseChunk', id, generation }).catch(() => {});\n  }\n  return { ...response, value: JSON.parse(text) as unknown };");
      if (name === 'timeline-presentation.ts') expected = "// Mobile365aa87982: pass the first-assistant fold policy through the existing pure row derivation.\n" + expected
        .replace('export function transcriptRows(client: T3Client): Message[] {', 'export function transcriptRows(client: T3Client, keepFirstAssistant = false): Message[] {')
        .replace('root, rollback: provider?.supportsConversationRollback !== false });', 'root, rollback: provider?.supportsConversationRollback !== false, keepFirstAssistant });');
      if (name === 'timeline-rows.ts') expected = "// Mobile365aa87982: keep the first assistant visible and anchor its fold at the first hidden entry.\n" + expected
        .replace('  root: string; rollback: boolean;', '  root: string; rollback: boolean;\n  keepFirstAssistant?: boolean;')
        .replace('new Set([...activeRuns, ...failedRuns]), runlessWorkActive);', 'new Set([...activeRuns, ...failedRuns]), runlessWorkActive, input.keepFirstAssistant === true);')
        .replace('unfolded: Set<string>, runlessWorkActive: boolean): Map<string, Fold>', 'unfolded: Set<string>, runlessWorkActive: boolean, keepFirstAssistant: boolean): Map<string, Fold>')
        .replace('interface Group { entries: Entry[]; terminal:', 'interface Group { entries: Entry[]; firstAssistant: Entry | null; terminal:')
        .replace('group = { entries: [], terminal:', 'group = { entries: [], firstAssistant: null, terminal:')
        .replace("    if (entry.kind === 'message') {", "    if (entry.kind === 'message') {\n      group.firstAssistant ??= entry;")
        .replace('if (entry === group.terminal) return;', 'if (entry === group.terminal || keepFirstAssistant && entry === group.firstAssistant) return;')
        .replace('    result.set(group.anchor, { runId, createdAt: group.start ?? first.createdAt, hidden, label });',
          '    const firstHidden = group.entries.find(entry => hidden.has(entry.id))!;\n    result.set(keepFirstAssistant ? firstHidden.id : group.anchor,\n      { runId, createdAt: keepFirstAssistant ? firstHidden.createdAt : group.start ?? first.createdAt, hidden, label });');
      if (name === 'client.ts') expected = "// Mobile 365aa87982: selection errors belong to the requesting route, not the thread composer.\n// Additive cleanup visibility from shared commit af0a96dddbd500aa50bc5bbe69ec59597e34efee.\n// Mobile 365aa87982: a read-only connection has no status; its actions refuse (useThreadListActions.ts).\n" + expected
        .replace("const formCommand = ['settings-core',", "const formCommand = ['select-thread', 'settings-core',")
        .replace("  private finishPending(", "  protected finishPending(")
        .replace("      this.error = !this.scopes.includes('orchestration:operate') ? 'This connection is read-only.'\n        : obj(environment.capabilities).serverResolvedCommandContext !== true\n          ? 'Update the T3 server",
          "      this.error = this.scopes.includes('orchestration:operate') && obj(environment.capabilities).serverResolvedCommandContext !== true\n        ? 'Update the T3 server");
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

      if (name === 'composer-controls.ts') expected = "// Mobile 365aa87982: only New Task writes the sticky model; an existing thread keeps per-model options.\n" + expected
        .replace("/** setStickyModelSelection: a picked model becomes the next draft's model; its own remembered traits come back with it. */\nexport function rememberModel(client: T3Client, providerId: string, modelId: string): Obj[] {\n  const state = prefs(client);\n  const remembered = state.stickyOptionsByModel[providerId]?.[modelId] ?? [];\n  state.stickyByProvider[providerId] = { model: modelId, options: remembered };\n  state.stickyProvider = providerId;\n  return remembered.map(option => ({ ...option }));\n}\n/** setProviderModelOptions(..., persistSticky): traits stick to the sticky model and are remembered for this model. */\nexport function rememberOptions(client: T3Client, providerId: string, modelId: string, chosen: Obj[]): void {\n  const state = prefs(client);\n  const base = state.stickyByProvider[providerId]?.model || modelId;\n  state.stickyByProvider[providerId] = { model: base, options: chosen.map(option => ({ ...option })) };\n  state.stickyOptionsByModel[providerId] = { ...state.stickyOptionsByModel[providerId], [modelId]: chosen.map(option => ({ ...option })) };\n  state.stickyProvider = providerId;\n}\n",
          "/** withRememberedModelOptions: a picked model brings back its own remembered traits. In New Task\n * (setStickyComposerModelSelection) it also becomes the next draft's model; a thread pick does not. */\nexport function rememberModel(client: T3Client, providerId: string, modelId: string): Obj[] {\n  const state = prefs(client);\n  const remembered = state.stickyOptionsByModel[providerId]?.[modelId] ?? [];\n  if (!client.threadId) { state.stickyByProvider[providerId] = { model: modelId, options: remembered }; state.stickyProvider = providerId; }\n  return remembered.map(option => ({ ...option }));\n}\n/** rememberModelOptions: traits are remembered for this model. In New Task this model and its traits also stick. */\nexport function rememberOptions(client: T3Client, providerId: string, modelId: string, chosen: Obj[]): void {\n  const state = prefs(client);\n  state.stickyOptionsByModel[providerId] = { ...state.stickyOptionsByModel[providerId], [modelId]: chosen.map(option => ({ ...option })) };\n  if (!client.threadId) { state.stickyByProvider[providerId] = { model: modelId, options: chosen.map(option => ({ ...option })) }; state.stickyProvider = providerId; }\n}\n");
      // Mobile 365aa87982: a started thread keeps its edited model and modes locally; Send carries them and keeps them.
      if (name === 'composer-controls.ts') expected = expected
        .replace("// Mobile 365aa87982: only New Task writes the sticky model; an existing thread keeps per-model options.\n", "// Mobile 365aa87982: only New Task writes the sticky model; an existing thread keeps per-model options.\n// Mobile 365aa87982: a started thread keeps only its edited model and modes as local saved settings (use-composer-drafts); Send carries them to the server and keeps them.\n")
        .replace("/** Next-turn choices made in a started thread while a turn runs (the reference keeps them in the draft). */\nexport type StagedSelection = { providerId: string; modelId: string; options: Obj[]; runtimeMode: string; interactionMode: string };\n", "/** A started thread's own model and mode choices (the reference keeps them in its draft). The model group is\n * all or nothing; a field never chosen is absent and follows the thread. */\nexport type StagedSelection = { providerId?: string; modelId?: string; options?: Obj[]; runtimeMode?: string; interactionMode?: string };\n")
        .replace("  for (const [key, entry] of Object.entries(obj(value.staged)).slice(0, 200)) {\n    const staged = obj(entry);\n    if (str(staged.providerId) && str(staged.modelId)) next.staged[key] = { providerId: str(staged.providerId), modelId: str(staged.modelId),\n      options: options(staged.options), runtimeMode: str(staged.runtimeMode, 'approval-required'), interactionMode: str(staged.interactionMode, 'default') };\n  }\n", "  for (const [key, entry] of Object.entries(obj(value.staged))) {\n    const saved = obj(entry), staged: StagedSelection = {};\n    if (str(saved.providerId) && str(saved.modelId) && Array.isArray(saved.options))\n      Object.assign(staged, { providerId: str(saved.providerId), modelId: str(saved.modelId), options: options(saved.options) });\n    if (str(saved.runtimeMode)) staged.runtimeMode = str(saved.runtimeMode);\n    if (str(saved.interactionMode)) staged.interactionMode = str(saved.interactionMode);\n    if (Object.keys(staged).length) next.staged[key] = staged;\n  }\n")
        .replace("// \u2500\u2500 Next-turn staging in started threads \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\n\n/** A started thread stages its next-turn choices while a turn runs, or once a staged choice exists. */\nexport function stagesChanges(client: T3Client): boolean {\n  return !!client.threadId && (!!activeRun(client.projection) || !!prefs(client).staged[client.draftKey]);\n}\nexport function stage(client: T3Client, change: Partial<StagedSelection>): void {\n  const state = prefs(client);\n  const current = state.staged[client.draftKey] ?? { providerId: client.providerId, modelId: client.modelId,\n    options: client.modelOptions.map(option => ({ ...option })), runtimeMode: client.runtimeMode, interactionMode: client.interactionMode };\n  const next = { ...current, ...change };\n  const thread = obj(client.projection.thread), selection = obj(thread.modelSelection);\n  const same = next.providerId === str(selection.instanceId) && next.modelId === str(selection.model)\n    && JSON.stringify(next.options) === JSON.stringify(arr(selection.options)) && next.runtimeMode === str(thread.runtimeMode, 'approval-required')\n    && next.interactionMode === str(thread.interactionMode, 'default');\n  if (same) delete state.staged[client.draftKey]; else state.staged[client.draftKey] = next;\n  client.providerId = next.providerId; client.modelId = next.modelId; client.modelOptions = next.options.map(option => ({ ...option }));\n  client.runtimeMode = next.runtimeMode; client.interactionMode = next.interactionMode;\n}\n/** After the thread's own selection is read, a staged choice still owns the composer. */\nexport function applyStaged(client: T3Client): void {\n  const staged = prefs(client).staged[client.draftKey];\n  if (!staged) return;\n  client.providerId = staged.providerId; client.modelId = staged.modelId; client.modelOptions = staged.options.map(option => ({ ...option }));\n  client.runtimeMode = staged.runtimeMode; client.interactionMode = staged.interactionMode;\n}\n", "// \u2500\u2500 A started thread's own choices \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\n\n/** updateComposerDraftSettings: write only the edited fields. A choice equal to the thread's stays explicit. */\nexport function stage(client: T3Client, change: StagedSelection): void {\n  const state = prefs(client);\n  state.staged[client.draftKey] = { ...state.staged[client.draftKey], ...change };\n  applyStaged(client);\n}\n/** After the thread's own selection is read, each chosen field still owns the composer (`draft.x ?? thread.x`). */\nexport function applyStaged(client: T3Client): void {\n  const staged = prefs(client).staged[client.draftKey];\n  if (!staged) return;\n  if (staged.providerId && staged.modelId && staged.options) {\n    client.providerId = staged.providerId; client.modelId = staged.modelId; client.modelOptions = staged.options.map(option => ({ ...option }));\n  }\n  if (staged.runtimeMode) client.runtimeMode = staged.runtimeMode;\n  if (staged.interactionMode) client.interactionMode = staged.interactionMode;\n}\n");
      if (name === 'client-ops-composer.ts') expected = expected
        .replace("// Mobile 365aa87982: send admission, retained options and independent model-pick ownership differ.\n", "// Mobile 365aa87982: send admission, retained options and independent model-pick ownership differ.\n// Mobile 365aa87982: a started thread's model, option and mode picks are local saved settings; Send carries them to the server and keeps them.\n")
        .replace("import { composerNow, stagesChanges, stage, rememberModel, rememberOptions, stagedFor, clearStaged, nextTurnCommands,", "import { composerNow, stage, rememberModel, rememberOptions, stagedFor, nextTurnCommands,")
        .replace("import { ClientError, activeRun, providerAvailable, modelSelection, sendPayload,", "import { ClientError, activeRun, providerAvailable, sendPayload,")
        .replace("    const key = this.draftKey, staged = stagedFor(this);\n", "    const staged = stagedFor(this);\n")
        .replace("    await this.dispatch(native, storage, payload, 'Send', assertOwner);\n    clearStaged(this, key);\n", "    await this.dispatch(native, storage, payload, 'Send', assertOwner);\n")
        .replace("  if (stagesChanges(this)) { stage(this, { providerId, modelId, options: remembered }); return; }\n  if (this.threadId) {\n    const [commandId] = await this.ids(native, 1);\n    await this.dispatch(native, storage, { type: 'thread.model-selection.set', commandId, threadId: this.threadId,\n      modelSelection: modelSelection(providerId, modelId, remembered) }, 'Change model');\n  }\n", "  // onUpdateModelSelection: a provider without Plan also writes Build.\n  if (this.threadId) { stage(this, { providerId, modelId, options: remembered, ...provider.showInteractionModeToggle === false ? { interactionMode: 'default' } : {} }); return; }\n")
        .replace("  if (stagesChanges(this)) { stage(this, { options }); return; }\n  if (this.threadId) {\n    const [commandId] = await this.ids(native, 1);\n    await this.dispatch(native, storage, { type: 'thread.model-selection.set', commandId, threadId: this.threadId,\n      modelSelection: modelSelection(this.providerId, this.modelId, options) }, 'Change reasoning effort');\n  }\n", "  if (this.threadId) { stage(this, { providerId: this.providerId, modelId: this.modelId, options }); return; }\n")
        .replace("  if (stagesChanges(this)) { stage(this, op === 'runtime' ? { runtimeMode: value } : { interactionMode: value }); return; }\n  if (this.threadId) {\n    const [commandId] = await this.ids(native, 1);\n    const field = op === 'runtime' ? 'runtimeMode' : 'interactionMode';\n    await this.dispatch(native, storage, { type: `thread.${op === 'runtime' ? 'runtime' : 'interaction'}-mode.set`, commandId, threadId: this.threadId, [field]: value }, 'Change mode');\n  }\n", "  if (this.threadId) { stage(this, op === 'runtime' ? { runtimeMode: value } : { interactionMode: value }); return; }\n");
      if (name === 'composer-editor.ts') {
        const source = bodies[copies.length]!;
        const pick = source.slice(source.indexOf('const contextPickStamp ='), source.indexOf('const CLOSED:'))
          .replace('client.snapshotOwner, client.draftKey, client.draft]);', 'client.snapshotOwner, client.draftKey, mobileNewTaskDraftCurrent(client)?.createdAt, client.draft]);');
        expected = "// Mobile additive context selection export from shared commit db560228e1cb2ad00c9cbff76ed8d22a05438249.\nimport { mobileNewTaskDraftCurrent } from '../mobile-new-task-drafts';\n" + expected
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
      // Mobile365aa87982: HTML is a persistent row before folds, with an exact source tuple.
      if (name === 'domain.ts') expected = expected
        .replace("  revert?: number; intent?: string; intentTip?: string; attribution?: string; targetId?: string; actionLabel?: string; copied?: number; copyFailed?: boolean;\n  meta?: boolean; streaming?: boolean; actionsId?: string; collapsible?: boolean;\n  code?: { id: string; code: string; icon: string; tokens: { id: string; text: string; cls: string }[] }[];\n", "  revert?: number; intent?: string; intentTip?: string; attribution?: string; targetId?: string; actionLabel?: string; copied?: number; copyFailed?: boolean;\n  meta?: boolean; streaming?: boolean; actionsId?: string; collapsible?: boolean; htmlRenderKey?: string;\n  code?: { id: string; code: string; icon: string; tokens: { id: string; text: string; cls: string }[] }[];\n");
      if (name === 'timeline-presentation.ts') expected = expected
        .replace("    case 'compaction': return { ...base, kind: 'compaction', title: row.label, live: row.active, icon: 'minimize', activities: [keyedLabel(row, row.label)] };\n    case 'plan': {\n", "    case 'compaction': return { ...base, kind: 'compaction', title: row.label, live: row.active, icon: 'minimize', activities: [keyedLabel(row, row.label)] };\n    case 'html-render': return { ...base, kind: 'html-render', runId: str(row.entry.item.runId),\n      htmlRenderKey: JSON.stringify([str(row.entry.row.visibility), row.entry.row.sourceThreadId, row.entry.row.sourceItemId]) };\n    case 'plan': {\n");
      if (name === 'timeline-rows.ts') expected = expected
        .replace("import { arr, obj, str, type Obj } from './domain';\nimport {\n", "import { arr, obj, str, type Obj } from './domain';\nimport { htmlRenderFromToolItem } from '../html-render';\nimport {\n")
        .replace("  | { kind: 'work'; id: string; createdAt: string; row: Obj; entry: WorkEntry; attempt?: Attempt }\n  | { kind: 'event'; id: string; createdAt: string; row: Obj; item: Obj; attempt?: Attempt }\n", "  | { kind: 'work'; id: string; createdAt: string; row: Obj; entry: WorkEntry; attempt?: Attempt }\n  | { kind: 'html-render'; id: string; createdAt: string; row: Obj; item: Obj; attempt?: Attempt }\n  | { kind: 'event'; id: string; createdAt: string; row: Obj; item: Obj; attempt?: Attempt }\n")
        .replace("  | { kind: 'plan'; id: string; createdAt: string; entry: Extract<Entry, { kind: 'plan' }> }\n  | { kind: 'checkpoint'; id: string; createdAt: string; entry: Extract<Entry, { kind: 'checkpoint' }> }\n", "  | { kind: 'plan'; id: string; createdAt: string; entry: Extract<Entry, { kind: 'plan' }> }\n  | { kind: 'html-render'; id: string; createdAt: string; entry: Extract<Entry, { kind: 'html-render' }> }\n  | { kind: 'checkpoint'; id: string; createdAt: string; entry: Extract<Entry, { kind: 'checkpoint' }> }\n")
        .replace("    else if (STANDALONE.has(type)) entries.push({ ...base, kind: 'event', item });\n    else entries.push({ ...base, kind: 'work', entry: { ...projectedWorkEntry(row), id } });\n", "    else if (STANDALONE.has(type)) entries.push({ ...base, kind: 'event', item });\n    else if (htmlRenderFromToolItem(item)) entries.push({ ...base, kind: 'html-render', item });\n    else entries.push({ ...base, kind: 'work', entry: { ...projectedWorkEntry(row), id } });\n")
        .replace("const runOf = (entry: Entry): string => entry.kind === 'message' ? entry.role === 'assistant' ? str(entry.item.runId) : ''\n  : entry.kind === 'plan' ? str(entry.item.runId) : entry.kind === 'work' ? entry.entry.runId : '';\nconst startsResponse = (entry: Entry) => entry.kind === 'message' && entry.role === 'user' && entry.item.inputIntent !== 'steer'\n  && entry.item.inputIntent !== 'promoted_queued_to_steer' || entry.kind === 'work' && entry.entry.itemType === 'notification';\nconst persistentResource = (entry: Entry) => entry.kind === 'event' && PERSISTENT.has(str(entry.item.type));\nconst failedItem = (entry: Entry): Obj | null => {\n", "const runOf = (entry: Entry): string => entry.kind === 'message' ? entry.role === 'assistant' ? str(entry.item.runId) : ''\n  : entry.kind === 'plan' || entry.kind === 'html-render' ? str(entry.item.runId) : entry.kind === 'work' ? entry.entry.runId : '';\nconst startsResponse = (entry: Entry) => entry.kind === 'message' && entry.role === 'user' && entry.item.inputIntent !== 'steer'\n  && entry.item.inputIntent !== 'promoted_queued_to_steer' || entry.kind === 'work' && entry.entry.itemType === 'notification';\nconst persistentResource = (entry: Entry) => entry.kind === 'html-render' || entry.kind === 'event' && PERSISTENT.has(str(entry.item.type));\nconst failedItem = (entry: Entry): Obj | null => {\n")
        .replace("    }\n    if (entry.kind === 'plan') { rows.push({ kind: 'plan', id: entry.id, createdAt: entry.createdAt, entry }); continue; }\n", "    }\n    if (entry.kind === 'html-render') { rows.push({ kind: 'html-render', id: entry.id, createdAt: entry.createdAt, entry }); continue; }\n    if (entry.kind === 'plan') { rows.push({ kind: 'plan', id: entry.id, createdAt: entry.createdAt, entry }); continue; }\n")
        .replace("    if (entry.kind === 'work' && entry.entry.itemType === 'system_notice') runId = null;\n    else if (entry.kind === 'message' && entry.role === 'assistant' || entry.kind === 'work') runId = runOf(entry) || runless;\n    else if (entry.kind === 'event' && (persistentResource(entry) || entry.item.type === 'subagent')) runId = str(entry.item.runId) || runless;\n", "    if (entry.kind === 'work' && entry.entry.itemType === 'system_notice') runId = null;\n    else if (entry.kind === 'message' && entry.role === 'assistant' || entry.kind === 'work' || entry.kind === 'html-render') runId = runOf(entry) || runless;\n    else if (entry.kind === 'event' && (persistentResource(entry) || entry.item.type === 'subagent')) runId = str(entry.item.runId) || runless;\n");
      let actual = local.slice(name === 'word-diff.ts' ? 3 : 2).join('\n');
      if (name === 'timeline-presentation.ts') {
        const history = expected.match(/^  if \(op === 'history'\) \{\n[\s\S]*?^  }\n/m)?.[0];
        expect(history).toBeDefined();
        expect(actual).toContain("op === 'history-auto' || op === 'history-retry'");
        actual = actual.replace("import { mobileLoadThreadHistory, mobileThreadHistoryError } from '../thread-history';\n", '')
          .replace('import { deriveRows, timelineEntries, type Row }', 'import { deriveRows, type Row }')
          .replace('  revert: { threadId:', '  historyError: Map<string, string>; revert: { threadId:')
          .replace("nonce: 0 }, revert: null", "nonce: 0 }, historyError: new Map(), revert: null")
          .replace(/^  if \(op === 'history-auto' \|\| op === 'history-retry'\) \{\n[\s\S]*?^  }\n/m, history!)
          .replace('historyError: mobileThreadHistoryError(client)', "historyError: view.historyError.get(client.threadId) ?? ''");
      }
      if (name === 'client.ts') {
        expect(actual).toContain('// Mobile history ownership: invocation-local tokens and captured transfer cleanup.');
        const method = /^  private ownedNative\([^\n]*\n[\s\S]*?^  }\n/m;
        const pinnedGuard = expected.match(method)?.[0]; expect(pinnedGuard).toBeDefined();
        const history = /^  async history\(native: Native[^\n]*\n[\s\S]*?^  }\n/m;
        const pinnedHistory = expected.match(history)?.[0]; expect(pinnedHistory).toBeDefined();
        expect(actual.match(history)).not.toBeNull();
        actual = actual.replace('// Mobile history ownership: invocation-local tokens and captured transfer cleanup.\n', '')
          .replace('// @ref examples/t3-code/mobile/docs/1130.003-pairing-and-transport.decision.md#stable-history-reader-and-thread-ownership\n', '')
          .replace(/^  \/\/ Mobile history ownership begin:[^\n]*\n[\s\S]*?^  \/\/ Mobile history ownership end\.\n/gm, '')
          .replace(method, pinnedGuard!)
          .replace(history, pinnedHistory!)
          .replace('      this.historyRequest = null;\n', '')
          .replace('      this.historyEpoch++;\n', '')
          .replace('  private async readThread(', '  async openThread(')
          .replace('storage: Files, claimed?: () => void): Promise<void>', 'storage: Files): Promise<void>')
          .replace('claimed?.(); this.available = false;', 'this.available = false;')
          .replace(/^.*\/\/ Mobile history ownership\n/gm, '')
          .replace(/^.*claimed\?\.\(\); \/\/ Mobile cache[^\n]*\n/gm, '')
          .replace('() => epoch === this.refreshEpoch, true)', '() => epoch === this.refreshEpoch)')
          .replace('() => epoch === this.synchronizationEpoch, true)', '() => epoch === this.synchronizationEpoch)')
          .replace(/^    \/\/ Explicit Refresh replaces an open;[^\n]*\n    \/\/ Retire before[^\n]*\n    if \(op === 'refresh'\) \{ this.threadEpoch\+\+; this.threadOpening = null; \}\n/gm, '')
          .replace(/^.*this.threadEpoch\+\+; \/\/ Mobile: a stream reset[^\n]*\n/gm, '')
          .replace(/^.*if \(this.openingThread\(\)\) return; \/\/ Mobile:[^\n]*\n/gm, '');
      }
      if (name === 'r5-composer-citation.ts') {
        const history = `type CitedHistoryOutcome = 'found' | 'aborted' | { title: string; description: string };
const MAX_CITATION_PAGES = 20;
const COULD_NOT_LOAD = { title: 'Could not load the cited response', description: 'Load earlier turns, then click the citation to try again. Your saved quote is unchanged.' };
const UNAVAILABLE = { title: 'The cited response is unavailable', description: 'It may have been removed. The selected text is still saved in your citation.' };
const NOT_AN_ANSWER = { title: 'The citation does not refer to an assistant response', description: 'The selected text is still saved in your citation.' };
const bare = (id: string) => id.replace(/^(?:turn-item|message):/, '');

function citedItem(client: T3Client, messageId: string): Obj | null {
  const wanted = bare(messageId);
  if (!wanted) return null;
  return arr(client.projection.visibleTurnItems).map(row => obj(row.item)).find(item =>
    ['assistant_message', 'user_message'].includes(str(item.type))
      && (bare(str(item.messageId)) === wanted || bare(str(item.id)) === wanted)) ?? null;
}

/** Page toward a cited response before asking the native transcript to place it. */
export async function loadCitedHistory(client: T3Client, native: Native, threadId: string, messageId: string): Promise<CitedHistoryOutcome> {
  const visit = \`citation:\${threadId}:\${messageId}\`;
  for (let pages = 0; ; pages++) {
    for (let wait = 0; client.threadId === threadId && client.historyLoading && wait < 40; wait++) {
      await native.later({ op: 'timelineSleep', ms: 180 }).catch(() => undefined);
    }
    if (client.threadId !== threadId) return 'aborted';
    if (client.historyLoading) return COULD_NOT_LOAD;
    const item = citedItem(client, messageId);
    if (item) return str(item.type) === 'assistant_message' ? 'found' : NOT_AN_ANSWER;
    if (!client.thread?.hasMore) return UNAVAILABLE;
    if (pages >= MAX_CITATION_PAGES) return COULD_NOT_LOAD;
    const cursor = client.thread.historyCursor;
    try {
      await client.history(native, visit, () => client.threadId === threadId, messageId);
    } catch (error) {
      if (letGo(error)) throw error;
      return COULD_NOT_LOAD;
    }
    if (client.threadId !== threadId) return 'aborted';
    if (client.thread?.hasMore && client.thread.historyCursor === cursor) return UNAVAILABLE;
  }
}

`;
        expected = expected
          .replace("import { ClientError, type Native } from './protocol';\n", "import { ClientError, type Native } from './protocol';\nimport { arr, obj, str, type Obj } from './domain';\n")
          .replace("import { pushToast } from './toast';\n", "import { pushToast } from './toast';\nimport { letGo } from './let-go';\n")
          .replace("/**\n * Opens the cited thread", history + "/**\n * Opens the cited thread")
          .replace("  if (threadId !== client.threadId) await client.openSelected(native, threadId);\n", "  if (threadId !== client.threadId) await client.openSelected(native, threadId);\n  const outcome = await loadCitedHistory(client, native, threadId, messageId);\n  if (outcome === 'aborted') return '';\n  if (outcome !== 'found') { pushToast(client, { kind: 'warning', ...outcome }); return ''; }\n");
      }
      expect(actual).toBe(expected);
    }
  });
});

describe('upstream mobile pairing forms', () => {
  test('normalizes retained form prefill synchronously without validating a native target', () => {
    const cases = [
      ['127.0.0.1:1', 'test-local-only', 'http://127.0.0.1:1/#token=test-local-only', 'http://127.0.0.1:1', 'test-local-only'],
      ['[::1]:3773', 'code', 'http://[::1]:3773/#token=code', 'http://[::1]:3773', 'code'],
      [' SERVER.example:3773 ', ' a b&c ', 'https://server.example:3773/#token=a+b%26c', 'https://server.example:3773', 'a b&c'],
      ['https://server.example/path?token=query#token=old', 'new', 'https://server.example/path?token=query#token=new', 'https://server.example', 'new'],
      [' server.example ', '', 'server.example', 'server.example', ''],
      ['https://server.example/path?token=query#token=%20hash%20', '', 'https://server.example/path?token=query#token=%20hash%20', 'https://server.example', ' hash '],
      ['https://t3.codes/pair?host=https%3A%2F%2Fserver.example%2F#token=%20code%20', '', 'https://t3.codes/pair?host=https%3A%2F%2Fserver.example%2F#token=%20code%20', 'https://server.example', 'code'],
      ['not a host', 'synthetic', 'not a host#token=synthetic', 'not a host#token=synthetic', ''],
      ['', 'unused', '', '', ''],
    ];
    for (const [host, code, url, expectedHost, expectedCode] of cases) {
      const result = mobilePairingPrefill('submit', host!, code!, 7, 'visit', 3);
      expect(result).not.toBeInstanceOf(Promise);
      expect(result).toEqual({ url, host: expectedHost, code: expectedCode, generation: 7, visit: 'visit', editRevision: 3, scanned: false });
    }
    const pastedCode = 'https://other.example/#token=code';
    expect(mobilePairingPrefill('submit', 'server.example', pastedCode, 1, 'visit', 0)).toMatchObject({
      host: 'https://server.example', code: pastedCode,
    });
  });
  test('retains the extracted scan URL and uses the reference display parser', () => {
    const url = 'https://t3.codes/pair?host=https%3A%2F%2Fserver.example#token=scan';
    const nested = `t3code://connect?pairingUrl=${encodeURIComponent(url)}`;
    expect(mobilePairingPrefill('scan', nested, '', 2, 'visit', 4)).toEqual({
      url, host: 'https://server.example', code: 'scan', generation: 2, visit: 'visit', editRevision: 4, scanned: true,
    });
    expect(mobilePairingPrefill('scan', 'server.example#token=raw', '', 3, 'visit', 4))
      .toMatchObject({ url: 'server.example#token=raw', host: 'server.example#token=raw', code: '' });
    expect(mobilePairingPrefill('scan', 'https://server.example/?token=query#token=%20', '', 4, 'visit', 4))
      .toMatchObject({ host: 'https://server.example', code: ' ' });
    expect(mobilePairingPrefill('scan', 'https://t3.codes/?host=https%3A%2F%2Fserver.example&token=query#token=%20', '', 5, 'visit', 4))
      .toMatchObject({ host: 'https://server.example', code: 'query' });
    expect(mobilePairingPrefill('submit', nested, '', 6, 'visit', 4)).toMatchObject({ url: nested, host: 't3code://connect', code: '' });
  });
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
  test('the old command entry cannot bypass ordinary admission or normalize its draft', async () => {
    const oldThread = mobileClient.threadId, oldEnvironment = mobileClient.environmentId;
    const before = JSON.stringify(mobileClient.local), calls: unknown[] = [];
    const native: Native = { available: true, watch() {}, async later(input) { calls.push(input); throw new Error('unexpected'); } };
    try {
      mobileClient.threadId = 'ordinary-send-boundary'; mobileClient.environmentId = 'ordinary-boundary-env';
      for (const op of ['send', 'send-alternate']) {
        const result = await mobileCommand([op, 'stale-owner'], native, unusedStorage);
        expect(result.message).toBe('Send this draft from its current thread composer.');
      }
      expect(JSON.stringify(mobileClient.local)).toBe(before);
      expect(calls).toEqual([]);
    } finally { mobileClient.threadId = oldThread; mobileClient.environmentId = oldEnvironment; }
  });
  test('bake snapshot is disconnected without reading files or manufacturing environments', async () => {
    await mobileSnapshotRead(undefined, unusedStorage);
    const snapshot = mobileSnapshotProjection();
    expect(snapshot.nativeAvailable).toBe(false);
    expect(snapshot.ready).toBe(false);
    expect(snapshot.environments).toEqual([]);
    expect(snapshot.projects).toEqual([]);
    expect(snapshot.origin).toBe('');
    expect(JSON.stringify(snapshot)).not.toContain('access_token');
  });
  test('catalog readiness requires a current successful catalog independently of preferences', () => {
    const result = Bun.spawnSync([process.execPath, join(import.meta.dir, 'tests/catalog-readiness.ts')], { timeout: 5000 });
    expect({ code: result.exitCode, stdout: result.stdout.toString(), stderr: result.stderr.toString() })
      .toEqual({ code: 0, stdout: 'Catalog readiness verified.\n', stderr: '' });
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

// The native store keeps chunks replayable until an explicit release. This
// fixture models that existing contract, including connection-generation guards.
class TransferWire implements Native {
  available = true; generation = 1; calls: Obj[] = [];
  retained = new Map<string, string[]>();
  beforeRead?: (request: Obj) => Promise<void> | void;
  beforeRelease?: (request: Obj) => Promise<void> | void;
  watch() {}
  add(id: string, text: string, width = 9) {
    this.retained.set(id, Array.from({ length: Math.ceil(text.length / width) }, (_, index) => text.slice(index * width, (index + 1) * width)));
  }
  good(value: unknown) { return { ok: true, generation: this.generation, value }; }
  async later(input: unknown): Promise<unknown> {
    const request = obj(input); this.calls.push(request);
    if (request.generation !== undefined && request.generation !== this.generation)
      return { ok: false, generation: this.generation, error: { kind: 'stale', message: 'The connection changed.' } };
    const id = str(request.id);
    if (request.op === 'readChunk') {
      await this.beforeRead?.(request);
      const text = this.retained.get(id)?.[Number(request.index)];
      return text === undefined ? { ok: false, generation: this.generation, error: { kind: 'stale', message: 'The response was released.' } }
        : this.good({ text });
    }
    if (request.op === 'releaseChunk') { await this.beforeRelease?.(request); this.retained.delete(id); return this.good({}); }
    return this.good({ _nativeTransfer: { id, parts: this.retained.get(id)?.length } });
  }
}
function transferWait() {
  let resolve!: () => void;
  const promise = new Promise<void>(done => { resolve = done; });
  return { promise, resolve };
}

describe('mobile complete transfer decoding', () => {
  test('releases the complete native value before parsing a lossless 12 MiB reply', async () => {
    const wire = new TransferWire(), value = { text: 'x'.repeat(12 * 1024 * 1024) + '💻café漢字\t\nend', empty: [], nested: { enabled: true } };
    wire.add('large', JSON.stringify(value), 48 * 1024);
    const parse = JSON.parse; let retainedAtParse = -1;
    try {
      JSON.parse = text => { retainedAtParse = wire.retained.size; return parse(text); };
      expect(await bridgeReply(wire, { op: 'http', id: 'large' })).toEqual({ ok: true, generation: 1, value });
    } finally { JSON.parse = parse; }
    expect(retainedAtParse).toBe(0);
    expect(wire.calls.filter(call => call.op === 'releaseChunk')).toEqual([{ op: 'releaseChunk', id: 'large', generation: 1 }]);
    expect(wire.calls.filter(call => call.op === 'readChunk').length).toBeGreaterThan(256);
  });
  test('a chunk can be replayed while another read is paused, and only its own release removes it', async () => {
    const wire = new TransferWire(), reached = transferWait(), resume = transferWait();
    const value = { text: 'complete overlapping response' };
    wire.add('first', JSON.stringify(value)); wire.add('second', JSON.stringify({ text: 'second' }));
    wire.beforeRead = async request => { if (request.id === 'first' && request.index === 1) { reached.resolve(); await resume.promise; } };
    const first = bridgeReply(wire, { op: 'http', id: 'first' });
    await reached.promise;
    const read = { op: 'readChunk', id: 'first', index: 0, generation: 1 };
    expect(await wire.later(read)).toEqual(await wire.later(read));
    expect(wire.retained.has('first')).toBe(true);
    expect((await bridgeReply(wire, { op: 'http', id: 'second' })).value).toEqual({ text: 'second' });
    expect(wire.retained.has('first')).toBe(true); expect(wire.retained.has('second')).toBe(false);
    resume.resolve();
    expect((await first).value).toEqual(value);
    expect(wire.retained.size).toBe(0);
  });
  test('an ordinary interrupted chunk read releases its own transfer and never parses a prefix', async () => {
    const wire = new TransferWire(), failure = new Error('read failed');
    wire.add('interrupted', JSON.stringify({ text: 'not a truncated result' }));
    wire.add('unrelated', JSON.stringify({ text: 'still owned' }));
    wire.beforeRead = request => { if (request.index === 1) throw failure; };
    const parse = JSON.parse; let parses = 0;
    try {
      JSON.parse = text => { parses++; return parse(text); };
      await expect(bridgeReply(wire, { op: 'http', id: 'interrupted' })).rejects.toBe(failure);
    } finally { JSON.parse = parse; }
    expect(parses).toBe(0);
    expect(wire.retained.has('interrupted')).toBe(false); expect(wire.retained.has('unrelated')).toBe(true);
  });
  test('generation change refuses the old reply without releasing a new-generation value', async () => {
    const wire = new TransferWire(); wire.add('old', JSON.stringify({ text: 'old' }));
    wire.beforeRead = () => { wire.generation++; wire.retained.clear(); wire.add('new', JSON.stringify({ text: 'new' })); };
    await expect(bridgeReply(wire, { op: 'http', id: 'old' })).rejects.toMatchObject({ kind: 'transport' });
    expect(wire.retained.has('new')).toBe(true);
    expect(wire.calls.at(-1)).toEqual({ op: 'releaseChunk', id: 'old', generation: 1 });
    wire.beforeRead = undefined;
    expect((await bridgeReply(wire, { op: 'http', id: 'new' })).value).toEqual({ text: 'new' });
    expect(wire.retained.size).toBe(0);
  });
  test('malformed complete JSON releases before its parse failure', async () => {
    const wire = new TransferWire(); wire.add('malformed', '{"text":');
    const parse = JSON.parse; let retainedAtParse = -1;
    try {
      JSON.parse = text => { retainedAtParse = wire.retained.size; return parse(text); };
      await expect(bridgeReply(wire, { op: 'http', id: 'malformed' })).rejects.toBeInstanceOf(SyntaxError);
    } finally { JSON.parse = parse; }
    expect(retainedAtParse).toBe(0);
  });
  test('best-effort release failure still preserves a complete response', async () => {
    const wire = new TransferWire(), value = { text: 'complete' }; wire.add('kept', JSON.stringify(value));
    wire.beforeRelease = () => { throw new Error('release failed'); };
    expect((await bridgeReply(wire, { op: 'http', id: 'kept' })).value).toEqual(value);
    expect(wire.retained.has('kept')).toBe(true);
  });
  test('Exact cancellation still refuses cleanup calls, so prompt abandonment remains unresolved', async () => {
    const wire = new TransferWire(); wire.add('abandoned', JSON.stringify({ text: 'native expiry still owns this value' }));
    wire.beforeRead = request => { if (request.index === 1) throw { name: 'FetchError', kind: 'Aborted' }; };
    const native = letGoAware(wire);
    await expect(bridgeReply(native, { op: 'http', id: 'abandoned' })).rejects.toMatchObject({ kind: 'superseded' });
    expect(wire.calls.filter(call => call.op === 'releaseChunk')).toEqual([]);
    expect(wire.retained.has('abandoned')).toBe(true);
    const count = wire.calls.length;
    await expect(native.later({ op: 'releaseChunk', id: 'abandoned', generation: 1 })).rejects.toMatchObject({ kind: 'superseded' });
    expect(wire.calls.length).toBe(count);
  });
});

// Actual mobile client and protocol seam: every invocation owns its own Native.
function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
const historyStorage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode('{}').buffer; }, async atomicWriteFile() {} } };
async function multipartSelection() {
  const f = refreshFixture(new MobileDraftClient()); await f.client.refresh(letGoAware(f.native), historyStorage);
  f.client.threadId = 'thread'; f.calls.length = 0;
  const original = f.native.later, entered = deferred<void>(), pause = deferred<void>();
  const held = new Map<string, { parts: string[]; generation: number }>();
  let serial = 0, failure = '', pausedOp = 'readChunk', hasPaused = false;
  const replies: Obj[] = [];
  f.native.later = async input => {
    const request = obj(input);
    if (request.op === 'http' && str(request.path).endsWith('/bounded')) {
      f.calls.push(request);
      if (failure === 'http') return { ok: false, generation: f.generation(), error: { kind: 'Limit', message: 'Synthetic admission refused.' } };
      const envelope = obj(await original(input)), snapshot = { ...obj(JSON.parse(JSON.stringify(envelope.value))), snapshotSequence: 7, historyCursor: 'cursor', hasMoreHistory: true };
      obj(obj(snapshot.projection).thread).id = decodeURIComponent(str(request.path).split('/').at(-2)!);
      obj(snapshot.projection).messages = [{ id: 'unicode', role: 'assistant', text: 'Complete 💻 café 漢字\nend' }];
      const scalars = Array.from(failure === 'json' ? '{invalid' : JSON.stringify(snapshot));
      const parts = Array.from({ length: Math.ceil(scalars.length / 64) }, (_, index) => scalars.slice(index * 64, (index + 1) * 64).join(''));
      const id = `transfer-${++serial}`, generation = f.generation(); held.set(id, { parts, generation });
      const response = { ok: true, generation, value: { _nativeTransfer: { id, parts: parts.length } } };
      replies.push(response);
      if (pausedOp === 'http' && !hasPaused) { hasPaused = true; entered.resolve(); await pause.promise; }
      return response;
    }
    if (request.op === 'readChunk' || request.op === 'releaseChunk') {
      f.calls.push(request); const transfer = held.get(str(request.id));
      if (!transfer) throw new Error('Unknown fixture transfer');
      expect(request.generation).toBe(transfer.generation);
      if (!hasPaused && (request.op === pausedOp && (pausedOp !== 'readChunk' || request.index === 1))) {
        hasPaused = true; entered.resolve(); await pause.promise;
      }
      if (request.op === 'releaseChunk') { held.delete(str(request.id)); return { ok: true, generation: transfer.generation, value: {} }; }
      if (failure === 'chunk' && request.index === 2) return { ok: false, generation: transfer.generation, error: { kind: 'Read', message: 'Synthetic chunk failed.' } };
      return { ok: true, generation: transfer.generation, value: { text: transfer.parts[Number(request.index)] } };
    }
    return original(input);
  };
  const answer = () => letGoAware(f.native);
  const refresh = () => f.client.refresh(answer(), historyStorage);
  const select = (id = 'thread') => f.client.command('select-thread', id, '', 0, answer(), historyStorage);
  // The fixture delegates GET decoding to the original source, so count unique
  // reply descriptors rather than its two logging seams.
  return { ...f, entered, pause, held, replies, answer, refresh, select,
    fail(kind: string) { failure = kind; }, pauseAt(op: string) { pausedOp = op; },
    reads: () => f.calls.filter(call => call.op === 'readChunk'), releases: () => f.calls.filter(call => call.op === 'releaseChunk'),
    subscriptions: () => f.calls.filter(call => call.op === 'subscribe' && call.key === 'thread') };
}

describe('mobile history invocation ownership', () => {
  test('selection owns multipart load across refresh, synchronize and repeated selection demand', async () => {
    const f = await multipartSelection(), first = f.select(); await f.entered.promise;
    const epoch = f.client.threadEpoch;
    await f.refresh(); await f.client.synchronize(f.answer()); expect((await f.select()).message).toBe('');
    expect(f.client.threadEpoch).toBe(epoch); expect(f.replies).toHaveLength(1); expect(f.client.thread).toBeNull();
    f.pause.resolve(); expect((await first).message).toBe('');
    expect(f.reads().length).toBe(Number(obj(obj(f.replies[0]!.value)._nativeTransfer).parts));
    expect(f.releases()).toHaveLength(1); expect(f.held.size).toBe(0); expect(f.subscriptions()).toHaveLength(1);
    expect(obj(f.client.thread?.projection.thread).id).toBe('thread'); expect(f.client.thread?.historyCursor).toBe('cursor');
    expect(obj((f.client.projection.messages as Obj[])[0]).text).toBe('Complete 💻 café 漢字\nend');
    // The Runner integration witness consumes this wake; this unit test does not inject an extra refresh.
    expect(f.client.ready).toBe(false);
    expect(f.calls.filter(call => call.op === 'r10Wake')).toEqual([{ op: 'r10Wake', topic: 't3.notify', generation: 1 }]);
    expect(f.events.some(event => event.key === 'thread')).toBe(true);
  });
  test('a second same-generation refresh leaves the first refresh and its cache boundary current', async () => {
    const f = await multipartSelection(), first = f.refresh(); await f.entered.promise;
    const epoch = f.client.threadEpoch;
    await f.refresh(); expect(f.client.threadEpoch).toBe(epoch); expect(f.replies).toHaveLength(1);
    f.pause.resolve(); await first;
    expect(f.client.ready).toBe(true); expect(f.client.error).toBe(''); expect(f.releases()).toHaveLength(1); expect(f.held.size).toBe(0);
  });
  test('same active selection retains the expanded graph, cursor and sequence without IO', async () => {
    const f = await multipartSelection(), first = f.select(); await f.entered.promise; f.pause.resolve(); await first; await f.refresh();
    const thread = f.client.thread!; thread.historyExpanded = true; thread.historyCursor = 'older-cursor'; thread.sequence = 42;
    f.calls.length = 0; await f.client.openSelected(f.answer(), 'thread');
    expect(f.client.thread).toBe(thread); expect(f.client.thread.historyCursor).toBe('older-cursor'); expect(f.client.thread.sequence).toBe(42);
    expect(f.calls.some(call => call.op === 'http' || call.op === 'subscribe')).toBe(false);
    f.client.threadSubscription = ''; await f.client.openSelected(f.answer(), 'thread');
    expect(f.client.thread).toBe(thread); expect(f.calls.find(call => call.key === 'thread')?.payload).toMatchObject({ afterSequence: 42 });
    expect(f.replies).toHaveLength(1);
  });
  test('draft selection retires the old load without stale adoption', async () => {
    const f = await multipartSelection(), first = f.select(); await f.entered.promise;
    await f.client.openDraft(f.answer(), 'project'); f.pause.resolve(); await first;
    expect(f.client.threadId).toBe(''); expect(f.client.thread).toBeNull(); expect(f.subscriptions()).toHaveLength(0); expect(f.held.size).toBe(0);
    expect((await f.select()).message).toBe(''); expect(f.client.threadId).toBe('thread'); expect(f.replies).toHaveLength(2);
  });
  test('a different thread is never hidden by the pending target gate', async () => {
    const f = await multipartSelection(), first = f.select(); await f.entered.promise;
    f.client.shell.threads.push({ ...f.client.shell.threads[0]!, id: 'other-thread' });
    expect((await f.select('other-thread')).message).toBe('');
    const current = f.client.thread; f.pause.resolve(); await first;
    expect(f.client.thread).toBe(current); expect(obj(f.client.thread?.projection.thread).id).toBe('other-thread');
    expect(f.client.threadSubscription).toBe('other-thread'); expect(f.replies).toHaveLength(2);
    expect(f.calls.filter(call => call.op === 'r10Wake')).toHaveLength(1);
  });
  test('new generation bypasses the gate and stale first refresh releases its captured lease', async () => {
    const f = await multipartSelection(), first = f.refresh(); await f.entered.promise;
    f.reconnect(); await f.refresh(); expect(f.client.generation).toBe(2); expect(f.client.ready).toBe(true);
    const current = f.client.thread; f.pause.resolve(); await first;
    expect(f.client.thread).toBe(current); expect(f.client.ready).toBe(true); expect(f.held.size).toBe(0);
    expect(f.releases().map(call => [call.id, call.generation])).toEqual(f.replies.slice(1).map(reply =>
      [obj(obj(reply.value)._nativeTransfer).id, 2]).concat([[obj(obj(f.replies[0]!.value)._nativeTransfer).id, 1]]));
    expect(new Set(f.releases().map(call => call.id)).size).toBe(f.replies.length);
    expect(f.subscriptions()).toHaveLength(2); // Current-generation inbox reset legitimately opens again.
  });
  for (const owner of ['selection', 'reader']) test(`manual Refresh replaces a pending ${owner} open before clearing the graph`, async () => {
    const f = await multipartSelection(), prior = owner === 'selection' ? f.select() : f.refresh(); await f.entered.promise;
    const oldDescriptor = str(obj(obj(f.replies[0]!.value)._nativeTransfer).id);
    const changed = await f.client.command('refresh', '', '', 0, f.answer(), historyStorage);
    expect(changed.message).toBe(''); expect(f.client.shellLoaded).toBe(true);
    expect(obj(f.client.thread?.projection.thread).id).toBe('thread');
    const replacement = f.client.thread, currentEpoch = f.client.threadEpoch;
    f.pause.resolve(); await prior;
    expect(f.client.thread).toBe(replacement); expect(f.client.threadEpoch).toBe(currentEpoch);
    // Old command leases remain subject to native expiry; a live reader can release its captured lease.
    expect(f.held.has(oldDescriptor)).toBe(owner === 'selection');
    expect(f.calls.some(call => call.op === 'r10Wake' && call.topic === 't3.notify')).toBe(true);
  });
  for (const boundary of ['subscribe', 'r10Wake']) test(`real Aborted at ${boundary} clears the gate without another native dispatch`, async () => {
    const f = await multipartSelection(), original = f.native.later;
    let refuse = true;
    f.native.later = async input => {
      if (refuse && obj(input).op === boundary && (boundary !== 'subscribe' || obj(input).key === 'thread')) {
        f.calls.push(obj(input)); throw { name: 'FetchError', kind: 'Aborted' };
      }
      return original(input);
    };
    const selected = f.select(); await f.entered.promise; f.pause.resolve(); await selected;
    const stoppedAt = f.calls.length; await Promise.resolve(); expect(f.calls.length).toBe(stoppedAt);
    expect(f.client.error).toBe('');
    expect(f.calls.filter(call => call.op === 'r10Wake')).toHaveLength(boundary === 'r10Wake' ? 1 : 0);
    refuse = false; await f.refresh(); expect(f.client.ready).toBe(true);
  });
  test('malformed same-generation status is not hidden by the pending gate', async () => {
    const f = await multipartSelection(), first = f.refresh(); await f.entered.promise;
    const original = f.native.later;
    f.native.later = async input => obj(input).op === 'status'
      ? { ok: true, generation: f.generation(), value: {} } : original(input);
    await f.refresh(); expect(f.client.error).toContain('invalid connection status');
    f.pause.resolve(); await first; expect(f.client.thread).toBeNull(); expect(f.held.size).toBe(0);
    f.native.later = original; await f.refresh(); expect(f.client.ready).toBe(true);
  });
  test('a late descriptor after refresh supersession releases only that observed old reply', async () => {
    const f = await multipartSelection(); f.pauseAt('http'); const first = f.refresh(); await f.entered.promise;
    // No thread token from a different generation can block this owner.
    f.reconnect(); await f.refresh(); const current = f.client.thread;
    f.pause.resolve(); await first;
    expect(f.client.thread).toBe(current); expect(f.reads().every(call => call.generation === 2)).toBe(true);
    expect(f.releases().map(call => [call.id, call.generation])).toEqual(f.replies.slice(1).map(reply =>
      [obj(obj(reply.value)._nativeTransfer).id, 2]).concat([[obj(obj(f.replies[0]!.value)._nativeTransfer).id, 1]]));
    expect(new Set(f.releases().map(call => call.id)).size).toBe(f.replies.length); expect(f.held.size).toBe(0);
  });
  for (const boundary of ['http', 'readChunk', 'releaseChunk']) test(`real Aborted at ${boundary} settles, refuses later native calls and permits a fresh retry`, async () => {
    const f = await multipartSelection(); f.pauseAt(boundary); const first = f.refresh(); await f.entered.promise;
    const calls = f.calls.length;
    f.pause.reject({ name: 'FetchError', kind: 'Aborted' }); await first;
    expect(f.client.error).toBe(''); expect(f.calls.length).toBe(calls);
    const orphanCount = f.held.size; expect(orphanCount).toBe(1);
    await f.refresh(); expect(f.client.ready).toBe(true); expect(f.client.error).toBe('');
    expect(f.held.size).toBe(orphanCount); // True abandonment remains native expiry/reset, never a borrowed ticket.
  });
  for (const failure of ['http', 'json', 'chunk']) test(`${failure} failure clears ownership and a fresh refresh retries`, async () => {
    const f = await multipartSelection(); f.fail(failure); f.pause.resolve(); await f.refresh();
    expect(f.client.thread).toBeNull(); expect(f.client.error).not.toBe(''); expect(f.held.size).toBe(0);
    const error = f.client.error;
    f.fail(''); await f.refresh(); expect(f.client.ready).toBe(true); expect(f.client.error).toBe(error); expect(f.held.size).toBe(0);
  });
});


test('silent successful catalog and routing writes announce only through the existing metadata wake', async () => {
  const requests: Obj[] = []; let fail = false;
  const native = mobileNative({ available: true, watch() {}, async later(input) {
    const request = obj(input); requests.push(request);
    return { ok: !fail || request.op === 'r10Wake', generation: 1, value: {} };
  } });
  for (const op of ['setConnectionPreferences', 'setRoutes', 'setEnvironmentEnabled', 'forgetEnvironment']) {
    requests.length = 0; await native.later({ op });
    expect(requests.map(request => request.op)).toEqual([op, 'r10Wake']);
    expect(requests[1]).toEqual({ op: 'r10Wake', topic: 't3.notify' });
  }
  requests.length = 0; fail = true; await native.later({ op: 'setRoutes' });
  expect(requests.map(request => request.op)).toEqual(['setRoutes']);
});
