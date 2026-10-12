// Lane r3-protocol: wire errors, config folding, read tagging and abandoned-reply traces.
import { describe, test, expect } from 'bun:test';
import { ClientError, reply, bridgeReply, applyConfig, type Native } from './protocol';
import { keybindingsToastDecision, configEventSideEffects } from './r3-protocol-config';
import { traceRpc, statusTicket, settleTraces } from './r3-protocol-reader';
import { tracking } from './shell-slow';
import { toasts } from './toast';
import { T3Client } from './client';
import { compatibilityProblem, canSelfUpdate } from './r3-protocol-outdated';
import { obj, type Obj } from './domain';

describe('typed server errors (7bc161f)', () => {
  test('reason and detail travel from the bridge into ClientError', async () => {
    const parsed = reply({ ok: false, generation: 3, error: { kind: 'PullRequestOperationError', message: 'Pull request operation getDetail failed: nope', uncertain: false, reason: 'not-found', detail: 'nope' } });
    expect(parsed.error).toEqual({ kind: 'PullRequestOperationError', message: 'Pull request operation getDetail failed: nope', uncertain: false, reason: 'not-found', detail: 'nope' });
    const plain = reply({ ok: false, generation: 3, error: { kind: 'Network', message: 'down', uncertain: true, reason: '', detail: '' } });
    expect(plain.error).toEqual({ kind: 'Network', message: 'down', uncertain: true });
    const native: Native = { available: true, watch() {}, later: async () => ({ ok: false, generation: 1, error: { kind: 'PullRequestOperationError', message: 'm', uncertain: false, reason: 'not-found', detail: 'd' } }) };
    const result = await bridgeReply(native, { op: 'request' });
    const error = new ClientError(result.error!.message, result.error!.kind, result.error!.uncertain, result.error!);
    expect([error.kind, error.reason, error.detail]).toEqual(['PullRequestOperationError', 'not-found', 'd']);
    expect(new ClientError('x').reason).toBe('');
  });
});

describe('config stream folding (0080e80, serverConfigProjection.ts)', () => {
  const environment = (themes: boolean, sources: boolean) => ({ environmentId: 'env', capabilities: { environmentThemes: themes, usageLimitSources: sources } });
  const snapshot = (config: Obj) => ({ version: 1, type: 'snapshot', config: { providers: [], keybindings: [], ...config } });
  test('a late snapshot keeps the published themes and usage sources a capable server streamed', () => {
    let config = applyConfig({}, snapshot({ environment: environment(true, true), availableEditors: [] }));
    config = applyConfig(config, { type: 'environmentThemesUpdated', payload: { themes: [{ id: 'nord' }] } });
    config = applyConfig(config, { type: 'usageLimitSourcesUpdated', payload: { sources: [{ id: 'hub' }] } });
    config = applyConfig(config, { type: 'settingsUpdated', payload: { settings: { a: 1 } } });
    const late = applyConfig(config, snapshot({ environment: environment(true, true), availableEditors: ['cursor'], settings: { a: 1 } }));
    expect(late.availableEditors).toEqual(['cursor']);
    expect(late.environmentThemes).toEqual([{ id: 'nord' }]);
    expect(late.usageLimitSources).toEqual([{ id: 'hub' }]);
    // A downgraded server cannot send a later removal, so its snapshot drops the sets.
    const legacy = applyConfig(late, snapshot({ environment: environment(false, false) }));
    expect('environmentThemes' in legacy || 'usageLimitSources' in legacy).toBe(false);
    // A snapshot's own (never-sent) copy is not trusted over the stream.
    expect(applyConfig({}, snapshot({ environment: environment(true, true), environmentThemes: [{ id: 'x' }] })).environmentThemes).toBeUndefined();
  });
  test('keybindingsUpdated replaces keybindings and issues; empty theme and source sets clear', () => {
    let config = applyConfig({}, snapshot({ environment: environment(true, true), issues: [] }));
    config = applyConfig(config, { type: 'keybindingsUpdated', payload: { keybindings: [{ command: 'composer.sendAndNewThread' }], issues: [{ kind: 'keybindings.invalid-entry', message: 'bad' }] } });
    expect(config.keybindings).toEqual([{ command: 'composer.sendAndNewThread' }]);
    expect(config.issues).toEqual([{ kind: 'keybindings.invalid-entry', message: 'bad' }]);
    config = applyConfig(config, { type: 'environmentThemesUpdated', payload: { themes: [{ id: 'a' }] } });
    config = applyConfig(config, { type: 'environmentThemesUpdated', payload: { themes: [] } });
    expect('environmentThemes' in config).toBe(false);
    config = applyConfig(config, { type: 'usageLimitSourcesUpdated', payload: { sources: [] } });
    expect('usageLimitSources' in config).toBe(false);
    expect(applyConfig({}, { type: 'providerStatuses', payload: { providers: [] } })).toEqual({});
    expect(() => applyConfig({}, { type: 'snapshot', config: {} })).toThrow('invalid provider configuration');
  });
  test('a keybindings reload toasts like KeybindingsUpdateToast', () => {
    expect(keybindingsToastDecision({ type: 'settingsUpdated' })).toBeNull();
    expect(keybindingsToastDecision({ type: 'keybindingsUpdated', payload: { issues: [{ kind: 'provider.x', message: 'p' }] } })).toEqual({ tag: 'Success' });
    expect(keybindingsToastDecision({ type: 'keybindingsUpdated', payload: { issues: [{ kind: 'keybindings.malformed-config', message: 'Expected a JSON array' }] } }))
      .toEqual({ tag: 'InvalidConfiguration', message: 'Expected a JSON array' });
    const client = new T3Client(); client.environmentId = 'env';
    configEventSideEffects(client, { type: 'keybindingsUpdated', payload: { keybindings: [], issues: [] } });
    configEventSideEffects(client, { type: 'keybindingsUpdated', payload: { keybindings: [], issues: [] } });
    expect(toasts(client).map(toast => [toast.kind, toast.title, toast.description])).toEqual([['success', 'Keybindings updated', 'Keybindings configuration reloaded successfully.']]);
    configEventSideEffects(client, { type: 'keybindingsUpdated', payload: { keybindings: [], issues: [{ kind: 'keybindings.invalid-entry', message: 'Entry 3 is invalid.' }] } });
    const warning = toasts(client).at(-1)!;
    expect([warning.kind, warning.title, warning.description, warning.action?.label, warning.action?.op, warning.action?.id, warning.actionVariant, warning.stacked])
      .toEqual(['warning', 'Invalid keybindings configuration', 'Entry 3 is invalid.', 'Open keybindings.json', 'rest:keybinding-open', 'env:', 'outline', true]);
    expect(toasts(client).map(toast => toast.title)).toEqual(['Keybindings updated', 'Invalid keybindings configuration']);
  });
});

describe('abandon-safe reads and traces', () => {
  test('a request whose reply was dropped stops counting as slow once the transport no longer lists it', () => {
    const client = new T3Client();
    const lost = traceRpc(client, { op: 'request', method: 'server.getConfig', payload: {} });
    const live = traceRpc(client, { op: 'request', method: 'server.getSettings', payload: {} });
    const http = traceRpc(client, { op: 'http', path: '/x' });
    expect(lost.request.trace).toBe(1); expect(live.request.trace).toBe(2); expect(http.request.trace).toBeUndefined();
    expect(tracking(client)).toBe(true);
    // The first status read may race the requests; it never settles them.
    expect(settleTraces(client, { traces: [] }, statusTicket(client))).toBe(0);
    // The next read lists only the one still pending at the transport.
    expect(settleTraces(client, { traces: [2] }, statusTicket(client))).toBe(1);
    expect(tracking(client)).toBe(true);
    live.done();
    expect(tracking(client)).toBe(false);
    lost.done(); // a late normal completion stays harmless
    expect(settleTraces(client, {}, statusTicket(client))).toBe(0);
  });
  test('the snapshot read needs no reader tags or readEnd since exact2 #183', async () => {
    const seen: Obj[] = [];
    const later = async (input: unknown) => {
      const request = obj(input); seen.push(request);
      if (request.op === 'readPreferences') return { ok: true, generation: 1, value: { text: '' } };
      if (request.op === 'status') return { ok: true, generation: 1, value: { state: 'disconnected', origin: 'http://127.0.0.1:1', environmentId: '', message: 'Disconnected.', traces: [] } };
      if (request.fleet !== undefined || request.op === 'environments') return { ok: true, generation: 1, value: { saved: [] } };
      return { ok: true, generation: 1, value: {} };
    };
    const native: Native = { available: true, watch() {}, later };
    const client = new T3Client();
    const files = { fs: { async mkdir() {}, async readFile(): Promise<ArrayBuffer> { throw new Error('missing'); }, async atomicWriteFile() {} } };
    await client.refresh(native, files);
    expect(seen.some(request => request.op === 'status')).toBe(true);
    expect(seen.filter(request => request.op === 'readEnd' || 'reader' in request || 'readerSession' in request)).toEqual([]);
  });
});

// A minimal connected transport for client-level tests of the lane's plumbing.
class Wire implements Native {
  available = true; generation = 1; serial = 0; eventSequence = 0;
  calls: Obj[] = []; events: Obj[] = []; subscriptions: Record<string, string> = {};
  config: Obj = { environment: { environmentId: 'env1', orchestrationProtocolVersion: 2, capabilities: { serverResolvedCommandContext: true } }, providers: [], settings: {},
    shellResumeCompletionMarker: true, threadResumeCompletionMarker: true };
  shell: Obj = { snapshotSequence: 10, projects: [{ id: 'p1', title: 'P', workspaceRoot: '/r' }], threads: [], archivedThreads: [] };
  watch() {}
  good(value: unknown) { return { ok: true, generation: this.generation, value }; }
  emit(key: string, value: Obj, subscriptionId = this.subscriptions[key]) { this.events.push({ seq: ++this.eventSequence, generation: this.generation, key, subscriptionId, value }); }
  async later(input: unknown): Promise<unknown> {
    const request = obj(input); this.calls.push(request);
    const op = request.op;
    if (op === 'status') return this.good({ state: 'connected', origin: 'http://127.0.0.1:3773', environmentId: 'env1', message: '', traces: [] });
    if (op === 'http' && request.path === '/api/auth/session') return this.good({ authenticated: true, scopes: ['orchestration:read', 'orchestration:operate'] });
    if (op === 'http' && request.path === '/api/orchestration/shell') return this.good(this.shell);
    if (op === 'request' && request.method === 'server.getConfig') return this.good(this.config);
    if (op === 'subscribe') {
      const key = String(request.key), id = `sub-${++this.serial}`;
      this.subscriptions[key] = id;
      if (key === 'config') this.emit(key, { type: 'snapshot', config: this.config });
      else this.emit(key, { kind: 'synchronized' });
      return this.good({ id });
    }
    if (op === 'events') return this.good({ events: this.events.filter(event => Number(event.seq) > Number(request.after)), latest: this.eventSequence, reset: false });
    if (op === 'ack') { this.events = this.events.filter(event => Number(event.seq) > Number(request.through)); return this.good({ latest: this.eventSequence }); }
    return this.good({});
  }
}
const disk = () => ({ fs: { async mkdir() {}, async readFile(): Promise<ArrayBuffer> { throw new Error('missing'); }, async atomicWriteFile() {} } });

describe('same-session stream retries (c5a929e)', () => {
  test('a failed shell stream resubscribes from its sequence when the transport says the retry is due', async () => {
    const wire = new Wire(), client = new T3Client(), files = disk();
    await client.refresh(wire, files);
    expect(client.ready).toBe(true);
    const failed = wire.subscriptions.shell!;
    wire.emit('shell', { _transportError: { kind: 'OrchestrationV2ShellStreamError', message: 'Shell stream failed.' }, retryAfterMs: 250 });
    await client.refresh(wire, files);
    expect(client.error).toBe('Shell stream failed.');
    expect(client.ready).toBe(false);
    const before = wire.calls.filter(call => call.op === 'subscribe').length;
    // No full resynchronization: config and shell HTTP are not asked again.
    await client.refresh(wire, files);
    expect(wire.calls.filter(call => call.op === 'subscribe').length).toBe(before);
    wire.emit('shell', { _retryDue: true }, failed);
    await client.refresh(wire, files);
    const resubscribed = wire.calls.filter(call => call.op === 'subscribe').slice(before);
    expect(resubscribed.map(call => [call.key, obj(call.payload).afterSequence, obj(call.payload).requestCompletionMarker])).toEqual([['shell', 10, true]]);
    expect(client.error).toBe('');
    expect(client.ready).toBe(true);
  });
  test('an authorization failure is not retried and does not resynchronize', async () => {
    const wire = new Wire(), client = new T3Client(), files = disk();
    await client.refresh(wire, files);
    const subscribes = wire.calls.filter(call => call.op === 'subscribe').length;
    wire.emit('shell', { _transportError: { kind: 'EnvironmentAuthorizationError', message: 'The authenticated token is missing required scope: orchestration:read.' } });
    await client.refresh(wire, files);
    await client.refresh(wire, files);
    expect(client.error).toBe('The authenticated token is missing required scope: orchestration:read.');
    expect(wire.calls.filter(call => call.op === 'subscribe').length).toBe(subscribes);
    // A stale retry for a replaced subscription is ignored.
    wire.emit('shell', { _retryDue: true }, 'sub-old');
    await client.refresh(wire, files);
    expect(wire.calls.filter(call => call.op === 'subscribe').length).toBe(subscribes);
  });
});

describe('outdated servers (22e9d35)', () => {
  test('the compatibility message names the direction and whether this client can update the host', () => {
    expect(compatibilityProblem({ orchestrationProtocolVersion: 2, label: 'Box' })).toBeNull();
    expect(compatibilityProblem({ orchestrationProtocolVersion: 3, label: 'Box' })).toEqual({ message: 'This client is not supported by this server. Update your app or use a compatible release to connect to Box.', serverUpdateRequired: false });
    expect(compatibilityProblem({ label: 'Box', capabilities: { serverSelfUpdate: 'boot-service' } })).toEqual({ message: 'This client requires a newer server. Update T3 Code on Box to connect.', serverUpdateRequired: true });
    expect(compatibilityProblem({ orchestrationProtocolVersion: 1, label: 'Box', capabilities: { serverSelfUpdate: 'desktop-managed' } })!.serverUpdateRequired).toBe(false);
    expect(canSelfUpdate({ capabilities: { serverSelfUpdate: 'desktop-managed', desktopAppUpdate: true } })).toBe(true);
    expect(canSelfUpdate({})).toBe(false);
  });
});

describe('answers Exact lets go mid-flight', () => {
  test('a read stranded inside snapshot adoption does not block the next read from adopting', async () => {
    const seen: Obj[] = [];
    let strand = true;
    const native: Native = { available: true, watch() {}, later: request => {
      const value = obj(request); seen.push(value);
      if (value.op === 'snapshotConfigure' && strand) { strand = false; return new Promise(() => {}); } // never answered
      if (value.op === 'status') return Promise.resolve({ ok: true, generation: 1, value: { state: 'disconnected', origin: 'http://127.0.0.1:1', environmentId: '', message: 'Disconnected.', traces: [] } });
      if (value.op === 'snapshotState') return Promise.resolve({ ok: true, generation: 1, value: { captures: [] } });
      return Promise.resolve({ ok: true, generation: 1, value: { saved: [] } });
    } };
    const saved = new TextEncoder().encode(JSON.stringify({ version: 1, deviceSettings: { snapShotEnabled: true } })).buffer;
    const files = { fs: { async mkdir() {}, async readFile() { return saved; }, async atomicWriteFile() {} } };
    const client = new T3Client();
    void client.refresh(native, files); // its answer is let go while it waits
    await new Promise(resolve => setTimeout(resolve, 10));
    await client.refresh(native, files);
    expect(seen.filter(request => request.op === 'snapshotConfigure').length).toBe(2);
    expect(seen.filter(request => request.op === 'snapshotState').length).toBe(1);
  });
});
