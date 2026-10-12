import { beforeEach, test, expect } from 'bun:test';
import { resetPrimary } from './local-primary-fixture';
// No embedded server here: the fleet's environments are the saved ones only.
beforeEach(resetPrimary);
import { decodeModelPrefs, groupModels, bulkHidden, applyPickerPrefs, runModelPrefOp, sortModels, instancePrefs, adoptModelPrefs } from './settings-b-models';
import { normalizeScriptId, nextScriptId, commandForScript, keybindingFor, buildScript, resolveScripts, parseProjectScripts, decodeInput, validateInput, runActionOp, projectActions } from './settings-b-actions';
import { scopeMachine, environmentScopeChoices, environmentScopeIcon } from './settings-b-scope';
import { obj, type Obj } from './domain';
import type { Native } from './protocol';
import type { T3Client } from './client';

test('model visibility and order follow ProjectModelsSection grouping and the picker drops hidden built-ins', () => {
  const models = [{ slug: 'a', isCustom: false }, { slug: 'b', isCustom: false }, { slug: 'c', isCustom: false }, { slug: 'x', isCustom: true }];
  const display = groupModels(models, new Set(['c']), new Set(['a', 'x']), ['b', 'a']);
  expect(display.map(entry => [entry.model.slug, entry.group])).toEqual([['c', 'favorite'], ['b', 'visible'], ['x', 'visible'], ['a', 'hidden']]);
  expect(sortModels([{ slug: 'p' }, { slug: 'q' }, { slug: 'r' }], ['r'])).toEqual([{ slug: 'r' }, { slug: 'p' }, { slug: 'q' }]);
  expect(bulkHidden(models, [])).toEqual(['a', 'b', 'c']);
  expect(bulkHidden(models, ['a', 'b', 'c', 'zzz'])).toEqual(['zzz']);
  const local = { favoriteModels: [JSON.stringify(['inst', 'c'])] } as { favoriteModels: string[] };
  runModelPrefOp(local, 'provider-model-hide', 'inst', 'a', 'true', models);
  runModelPrefOp(local, 'provider-model-move', 'inst', 'x', 'up', models);
  expect(instancePrefs(local, 'inst')).toEqual({ hiddenModels: ['a'], modelOrder: ['c', 'x', 'b', 'a'] });
  // Moves never cross groups: the first visible row cannot climb into favorites.
  runModelPrefOp(local, 'provider-model-move', 'inst', 'x', 'up', models);
  expect(instancePrefs(local, 'inst').modelOrder).toEqual(['c', 'x', 'b', 'a']);
  expect(() => runModelPrefOp(local, 'provider-model-hide', 'inst', 'x', 'true', models)).toThrow('Custom models are always shown');
  expect(() => runModelPrefOp(local, 'provider-model-hide', 'inst', 'gone', 'true', models)).toThrow('no longer advertised');
  runModelPrefOp(local, 'provider-model-bulk', 'inst', '', '', models);
  expect(instancePrefs(local, 'inst').hiddenModels).toEqual(['a', 'b', 'c']);
  runModelPrefOp(local, 'provider-model-bulk', 'inst', '', '', models);
  expect(instancePrefs(local, 'inst').hiddenModels).toEqual([]);
  runModelPrefOp(local, 'provider-model-hide', 'inst', 'b', 'true', models);
  const picker = applyPickerPrefs(local, 'inst', models.map(model => ({ ...model })) as unknown as Obj[]);
  expect(picker.map(model => model.slug)).toEqual(['c', 'x', 'a']);
  // Persisted with T3Client.local and restored on load.
  const restored = {}; adoptModelPrefs(restored, JSON.parse(JSON.stringify(local)) as Obj);
  expect(instancePrefs(restored, 'inst')).toEqual(instancePrefs(local, 'inst'));
  expect(decodeModelPrefs({ i: { hiddenModels: ['a', 'a', 3, ''], modelOrder: 'nope' }, j: {} })).toEqual({ i: { hiddenModels: ['a'], modelOrder: [] } });
});

test('script identity, shortcuts and t3.json follow projectScripts.ts and the project file schema', () => {
  expect(normalizeScriptId('  Run Tests!  ')).toBe('run-tests');
  expect(normalizeScriptId('***')).toBe('script');
  expect(nextScriptId('Test', ['test', 'test-2'])).toBe('test-3');
  expect(nextScriptId('A very long action name that overflows', [])).toBe('a-very-long-action-name');
  expect(commandForScript('test')).toBe('script.test.run');
  expect(commandForScript('Legacy_ID')).toBeNull();
  expect(keybindingFor([{ command: 'script.test.run', shortcut: { key: 't', modKey: true } }, { command: 'script.test.run', shortcut: { key: 'y', modKey: true, shiftKey: true } }], 'script.test.run')).toBe('mod+shift+y');
  expect(buildScript('t', { name: 'T', command: 'c', icon: 'test', runOnWorktreeCreate: true, waitForSetup: true, keybinding: '', previewUrl: 'http://x', autoOpenPreview: true }))
    .toEqual({ id: 't', name: 'T', command: 'c', icon: 'test', runOnWorktreeCreate: true, async: false, previewUrl: 'http://x', autoOpenPreview: true });
  const settings = { defaultProjectScripts: [{ id: 'd', name: 'D', command: 'd', icon: 'play', runOnWorktreeCreate: false }], projectSettingsOverrides: { p2: { defaultProjectScripts: [] } }, projectSettingsFolded: true };
  expect(resolveScripts(settings, { id: 'p1' }).map(script => script.id)).toEqual(['d']);
  expect(resolveScripts(settings, { id: 'p2' })).toEqual([]);
  expect(resolveScripts({ defaultProjectScripts: [], projectScriptOverrides: {} }, { id: 'p3', scripts: [{ id: 'own', name: 'Own', command: 'o', icon: 'lint' }] }).map(script => script.id)).toEqual(['own']);
  expect(parseProjectScripts(null).status).toBe('missing');
  expect(parseProjectScripts('{').status).toBe('invalid');
  expect(parseProjectScripts('{"scripts":[{"name":"T","command":"t","icon":"rocket"}]}').status).toBe('invalid');
  expect(parseProjectScripts('{"scripts":[{"name":" T ","command":"bun test","icon":"test","runOnWorktreeCreate":true,"async":false}]}'))
    .toEqual({ status: 'valid', scripts: [{ name: 'T', command: 'bun test', icon: 'test', runOnWorktreeCreate: true, async: false, previewUrl: '', autoOpenPreview: false }] });
  const input = decodeInput('id=&name=%20Test%20&command=bun%20test&icon=bogus&setup=false&wait=true&keybinding=mod%2Bshift%2Bt&preview=&auto=true');
  expect(input).toMatchObject({ name: 'Test', command: 'bun test', icon: 'play', waitForSetup: true, keybinding: 'mod+shift+t', autoOpenPreview: true });
  expect(validateInput(input, null, [])).toMatchObject({ waitForSetup: false, autoOpenPreview: false });
  expect(() => validateInput({ ...input, name: '' }, null, [])).toThrow('Name is required.');
  expect(() => validateInput({ ...input, command: '' }, null, [])).toThrow('Command is required.');
  expect(() => validateInput({ ...input, keybinding: 'mod+shift' }, null, [])).toThrow('Invalid keybinding.');
});

class FakeServer {
  settings: Obj = { defaultProjectScripts: [], projectSettingsOverrides: {}, projectSettingsFolded: true };
  keybindings: Obj[] = [];
  calls: Obj[] = [];
  file: string | null = '{"scripts":[{"name":"Test","command":"bun test","icon":"test"},{"name":"Lint","command":"bun run lint","icon":"lint"}]}';
}
function fakeClient(server: FakeServer) {
  const client = {
    writable: true, environmentId: 'env1',
    shell: { projects: [{ id: 'p1', title: 'Proj', workspaceRoot: '/w/p', scripts: [] }], threads: [], sequence: 0 },
    config: { environment: { label: 'Mac', capabilities: { projectSettingsOverrides: true } }, settings: server.settings, keybindings: server.keybindings } as Obj,
    async rpc(_native: Native, method: string, payload: Obj): Promise<Obj> {
      server.calls.push({ method, payload });
      if (method === 'server.getConfig') return { environment: { label: 'Mac', capabilities: { projectSettingsOverrides: true } }, settings: JSON.parse(JSON.stringify(server.settings)), keybindings: server.keybindings };
      if (method === 'projects.readFile') return server.file === null ? { contents: '', truncated: true } : { contents: server.file, truncated: false };
      if (method === 'server.updateSettings') {
        for (const [id, entry] of Object.entries(obj(obj(payload.patch).projectSettingsOverrides))) {
          if (entry === null) delete obj(server.settings.projectSettingsOverrides)[id]; else (server.settings.projectSettingsOverrides as Obj)[id] = entry;
        }
        return server.settings;
      }
      if (method === 'server.upsertKeybinding') {
        server.keybindings = server.keybindings.filter(binding => !(payload.replace && binding.command === obj(payload.replace).command));
        server.keybindings.push({ command: payload.command, shortcut: { key: String(payload.key).split('+').pop(), modKey: String(payload.key).includes('mod'), shiftKey: String(payload.key).includes('shift') } });
        return { keybindings: server.keybindings };
      }
      if (method === 'server.removeKeybinding') { server.keybindings = server.keybindings.filter(binding => binding.command !== payload.command); return { keybindings: server.keybindings }; }
      throw new Error(`unexpected ${method}`);
    },
  };
  return client as unknown as T3Client;
}
const native = { available: true, watch() {}, later: async () => ({}) } as Native;

test('project actions add, edit, import, delete and reset through the project override and environment shortcuts', async () => {
  const server = new FakeServer(), client = fakeClient(server);
  const run = async (op: string, value: string) => { await runActionOp(client, native, op, 'p1', value); (client as unknown as { config: Obj }).config.settings = server.settings; (client as unknown as { config: Obj }).config.keybindings = server.keybindings; };
  let view = await projectActions(client, native, client.shell.projects);
  expect(view.actions).toEqual([]);
  expect(view.importable.map(entry => entry.name)).toEqual(['Test', 'Lint']);
  await run('action-add', 'name=Test&command=bun%20test&icon=test&setup=false&wait=false&keybinding=mod%2Bshift%2Bt&preview=&auto=false');
  expect(obj(obj(server.settings.projectSettingsOverrides).p1).defaultProjectScripts).toEqual([{ id: 'test', name: 'Test', command: 'bun test', icon: 'test', runOnWorktreeCreate: false }]);
  expect(server.calls.find(call => call.method === 'server.upsertKeybinding')?.payload).toEqual({ key: 'mod+shift+t', command: 'script.test.run' });
  view = await projectActions(client, native, client.shell.projects);
  expect(view.actions.map(action => [action.name, action.shortcut])).toEqual([['Test', '⇧⌘T']]);
  expect(view.importable.map(entry => entry.name)).toEqual(['Lint']); // same command already imported
  await run('action-import', 'name=Lint&command=bun%20run%20lint&icon=lint&setup=true&wait=true&keybinding=&preview=&auto=false');
  await run('action-save', 'id=test&name=Unit&command=bun%20test%20--watch&icon=test&setup=true&wait=false&keybinding=&preview=&auto=false');
  const saved = obj(obj(server.settings.projectSettingsOverrides).p1).defaultProjectScripts as Obj[];
  // Only one setup action: saving one with setup clears it on the others.
  expect(saved.map(script => [script.id, script.name, script.runOnWorktreeCreate])).toEqual([['test', 'Unit', true], ['lint', 'Lint', false]]);
  expect(server.keybindings).toEqual([]); // cleared shortcut removed
  await run('action-delete', 'id=lint');
  expect((obj(obj(server.settings.projectSettingsOverrides).p1).defaultProjectScripts as Obj[]).map(script => script.id)).toEqual(['test']);
  await run('action-reset', '');
  expect(obj(server.settings.projectSettingsOverrides).p1).toBeUndefined();
  await expect(runActionOp(client, native, 'action-add', 'p9', 'name=X&command=y')).rejects.toThrow('That project is no longer available.');
  await expect(runActionOp(client, native, 'action-add', 'p1', 'name=&command=y')).rejects.toThrow('Name is required.');
  server.file = '{ broken';
  const { forgetProjectFiles } = await import('./settings-b-actions'); forgetProjectFiles();
  expect((await projectActions(client, native, client.shell.projects)).t3Invalid).toBe(true);
});

test('Providers is single-environment: the machine defaults to the connected environment, with its icon', () => {
  const client = { environmentId: 'env1', config: { environment: { platform: { machine: 'laptop' } }, settings: {} } } as unknown as T3Client;
  expect(scopeMachine(client, 'providers', '')).toBe('env1');
  expect(scopeMachine(client, 'general', '')).toBe('');
  const choices = [{ id: '', label: 'All environments', mark: '', ink: '', surface: '', member: '', selected: true, offline: false },
    { id: 'env1', label: 'Mac', mark: '', ink: '', surface: '', member: '', selected: false, offline: false }];
  expect(environmentScopeChoices(client, 'providers', choices).map(choice => [choice.id, choice.mark])).toEqual([['env1', 'laptop']]);
  expect(environmentScopeChoices(client, 'general', choices).map(choice => [choice.id, choice.mark])).toEqual([['', ''], ['env1', 'laptop']]);
  expect(environmentScopeIcon(client, 'env1')).toBe('laptop');
  expect(environmentScopeIcon(client, '')).toBe('');
});

// ── Background environments (settings-b-fleet.ts) ─────────────────────────
import { EnvironmentFleet, fleetThreads, focusFleetThread, parseFleetThreadId, environmentKey, fleetTerminalProcessCount } from './settings-b-fleet';

class FleetNative implements Native {
  scopes = ['orchestration:read', 'orchestration:operate'];
  available = true; calls: Obj[] = []; saved: Obj[] = []; states: Record<string, string> = {}; events: Record<string, Obj[]> = {}; generation = 3;
  watch() {}
  async later(input: unknown): Promise<unknown> {
    const request = obj(input), key = String(request.fleet ?? ''); this.calls.push(request);
    const ok = (value: unknown, generation = this.generation) => ({ ok: true, generation, value });
    if (!key) {
      if (request.op === 'environments') return ok({ saved: this.saved });
      if (request.op === 'connect') return ok({ state: 'connected', origin: request.origin, environmentId: 'env-b', message: '' });
      return { ok: false, generation: 0, error: { kind: 'Arguments', message: 'unexpected', uncertain: false } };
    }
    if (request.op === 'fleetStop') { delete this.states[key]; return ok({ stopped: true }, 0); }
    if (request.op === 'connect') { this.states[key] = 'connected'; return ok({ state: 'connected' }); }
    if (request.op === 'status') return ok({ state: this.states[key] ?? 'disconnected', message: '', failureKind: '', traceId: '' });
    if (request.op === 'http' && request.path === '/api/auth/session') return ok({ authenticated: true, scopes: this.scopes });
    if (request.op === 'http' && request.path === '/api/orchestration/shell') return ok({ snapshotSequence: 4, projects: [{ id: 'p1', title: 'Remote project', workspaceRoot: '/r' }],
      threads: [{ id: 't1', projectId: 'p1', title: 'Remote thread', createdAt: '2026-10-01T00:00:00Z', updatedAt: '2026-10-01T00:00:00Z' }] });
    if (request.op === 'request' && request.method === 'server.getConfig') return ok({ environment: { environmentId: 'env-b', label: 'Box', platform: { machine: 'desktop' } }, providers: [], settings: {} });
    if (request.op === 'subscribe') return ok({ id: `${request.key}-sub` });
    if (request.op === 'events') return ok({ events: this.events[key] ?? [], latest: 0 });
    if (request.op === 'ack') return ok({ latest: 0 });
    return { ok: false, generation: 0, error: { kind: 'Arguments', message: `unexpected ${request.op}`, uncertain: false } };
  }
}

test('the fleet keeps every switched-on environment but the focused one connected and lists their threads', async () => {
  const native = new FleetNative(), fleetUnderTest = new EnvironmentFleet();
  native.saved = [{ origin: 'http://127.0.0.1:1', environmentId: 'env-a', enabled: true }, { origin: 'https://box.example.com', environmentId: 'env-b', enabled: true },
    { origin: 'https://off.example.com', environmentId: 'env-c', enabled: false }];
  const focus = { origin: 'http://127.0.0.1:1', environmentId: 'env-a', connection: 'connected' };
  const keyB = environmentKey('https://box.example.com', 'env-b');
  await fleetUnderTest.sync(native, focus);
  expect(native.calls.filter(call => call.op === 'connect').map(call => call.fleet)).toEqual([keyB]); // not the focused one, not the switched-off one
  expect(fleetUnderTest.entries.get(keyB)?.phase).toBe('connecting');
  await fleetUnderTest.sync(native, focus); // connected: bootstrap shell and config
  const entry = fleetUnderTest.entries.get(keyB)!;
  expect(entry.phase).toBe('connected');
  expect(entry.shell.threads.map(thread => thread.id)).toEqual(['t1']);
  const threads = fleetThreads(fleetUnderTest);
  expect(threads.map(thread => [thread.id, thread.projectId, thread.fleetProjectTitle, thread.fleetEnvironmentLabel, thread.fleetMachine])).toEqual([
    ['fleet:env-b:t1', 'fleet:env-b:p1', 'Remote project', 'Box', 'desktop']]);
  expect(parseFleetThreadId('fleet:env-b:t1')).toEqual({ environmentId: 'env-b', threadId: 't1' });
  expect(parseFleetThreadId('t1')).toBeNull();
  // Opening a background thread focuses its environment with the thread selected.
  const client = { local: { selections: {} as Record<string, { projectId: string; threadId: string }> } };
  const focused = await focusFleetThread(client, native, 'fleet:env-b:t1', fleetUnderTest);
  expect(client.local.selections['env-b']).toEqual({ projectId: 'p1', threadId: 't1' });
  expect(focused.value).toMatchObject({ environmentId: 'env-b', state: 'connected' });
  expect(native.calls.some(call => call.op === 'fleetStop' && call.fleet === keyB)).toBe(true);
  // Switching an environment off stops its background transport on the next pass.
  native.saved[1]!.enabled = false;
  await fleetUnderTest.sync(native, focus);
  expect(fleetUnderTest.entries.has(keyB)).toBe(false);
  await expect(focusFleetThread(client, native, 'fleet:env-z:t1', fleetUnderTest)).rejects.toThrow('no longer connected');
});


test('background terminal metadata follows its generation, activity and transport retry without rebootstrap', async () => {
  const native = new FleetNative(), source = new EnvironmentFleet();
  native.scopes.push('terminal:operate');
  native.saved = [{ origin: 'https://box.example.com', environmentId: 'env-b', enabled: true }];
  const focus = { origin: 'http://localhost:3773', environmentId: 'env-a', connection: 'connected' };
  const key = environmentKey('https://box.example.com', 'env-b');
  await source.sync(native, focus); await source.sync(native, focus);
  const countSubscriptions = () => native.calls.filter(call => call.method === 'subscribeTerminalMetadata').length;
  expect(countSubscriptions()).toBe(1);
  const entry = source.entries.get(key)!;
  const summary = (threadId: string, terminalId: string, busy: boolean) => ({ threadId, terminalId, cwd: '/r', worktreePath: null,
    status: 'running', pid: 1, exitCode: null, exitSignal: null, hasRunningSubprocess: busy, label: 'sh', updatedAt: '2026-10-06T00:00:00Z' });
  let seq = 0;
  const emit = async (value: Obj, generation = native.generation, subscriptionId = 'terminal-metadata-sub') => {
    native.events[key] = [{ seq: ++seq, key: 'terminal-metadata', subscriptionId, generation, value }];
    await source.sync(native, focus);
  };
  await emit({ type: 'snapshot', terminals: [summary('t1', 'term-1', true), summary('t1', 'term-2', false), summary('t2', 'term-1', true)] });
  expect(fleetTerminalProcessCount('env-b', 't1', source)).toBe(1);
  expect(fleetTerminalProcessCount('env-a', 't1', source)).toBe(0);
  expect(fleetTerminalProcessCount('env-b', 't2', source)).toBe(1);
  await emit({ type: 'upsert', terminal: summary('t1', 'term-2', true) });
  expect(fleetTerminalProcessCount('env-b', 't1', source)).toBe(2);
  await emit({ type: 'remove', threadId: 't1', terminalId: 'term-1' });
  expect(fleetTerminalProcessCount('env-b', 't1', source)).toBe(1);
  await emit({ type: 'snapshot', terminals: [] }, native.generation - 1);
  await emit({ type: 'snapshot', terminals: [] }, native.generation, 'old-subscription');
  await emit({ type: 'upsert', terminal: { threadId: 't1', hasRunningSubprocess: true } });
  expect(fleetTerminalProcessCount('env-b', 't1', source)).toBe(1);
  const configReads = native.calls.filter(call => call.method === 'server.getConfig').length;
  await emit({ _transportError: { kind: 'EnvironmentAuthorizationError', message: 'missing terminal:operate' } });
  await source.sync(native, focus); await source.sync(native, focus);
  expect(countSubscriptions()).toBe(1);
  expect(native.calls.filter(call => call.method === 'server.getConfig').length).toBe(configReads);
  await emit({ _retryDue: true });
  expect(countSubscriptions()).toBe(2);
  native.generation++;
  await source.sync(native, focus);
  expect(countSubscriptions()).toBe(3);
  expect(fleetTerminalProcessCount('env-b', 't1', source)).toBe(0);
  native.scopes = ['orchestration:read', 'orchestration:operate']; native.generation++;
  await source.sync(native, focus);
  expect(countSubscriptions()).toBe(3);
  expect(entry.terminalMetadata).toBeNull();
});
