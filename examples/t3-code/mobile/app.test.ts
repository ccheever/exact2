import { readFileSync } from 'node:fs';
import { describe, expect, test } from 'bun:test';
import { answer } from './app';
import { overlayMobilePreferenceChoices } from './settings-preferences';
import { contractComponent, value, type Env } from '../tests/support/contract-action-fixture';

const pairingContract = readFileSync(new URL('./mobile-root-state.contract', import.meta.url), 'utf8');
const pairingNames = ['pairingUiInitial', 'pairingUiVisit', 'pairingUiPreparing', 'pairingUiPreparedApplies',
  'pairingUiPrepared', 'pairingUiPreparedReply', 'pairingUiResolved', 'pairingReplyPlan'];

// Only record construction and option matching are supplied as language semantics.
// Every transition expression and root action is read from the production Contract.
function pairingFunctions(): Env {
  const scope: Env = {
    PairingUi: (base: Env, fields: Env = {}) => ({ ...base, ...fields }),
    PairingReplyPlan: (fields: Env) => fields,
  };
  for (const name of pairingNames) {
    const header = new RegExp(`^fn ${name}\\(([^\\n]*)\\):[^\\n]* = `, 'm').exec(pairingContract)!;
    const params = [...header[1]!.matchAll(/(\w+):/g)].map(match => match[1]!);
    const body = pairingContract.slice(header.index + header[0].length).split(/\n(?=fn |shape |\/\/)/, 1)[0]!.trim();
    scope[name] = (...args: unknown[]) => {
      const env = { ...scope, ...Object.fromEntries(params.map((param, index) => [param, args[index]])) };
      const option = /^match (\w+) \{\n\s+case some\((\w+)\) => ([\s\S]*),\n\s+case none => ([\s\S]*)\n\}$/.exec(body);
      if (!option) return value(body, env);
      const present = env[option[1]!] !== null && env[option[1]!] !== undefined;
      return value(option[present ? 3 : 4]!, { ...env, [option[2]!]: env[option[1]!] });
    };
  }
  return scope;
}

function pairingRoot() {
  const root = contractComponent('mobile/app.contract', 'App'), functions = pairingFunctions();
  const initial = functions.pairingUiInitial as () => Env;
  const commands: unknown[][] = [], preparations: Env[] = [], notices: unknown[][] = [];
  const state: Env = { pairingUi: initial(), lastOperation: '', targetEnvironment: '', nav: [{ id: 'home', name: 'home' }] };
  let visit = 0;
  const scope: Env = { ...functions, taskGuardOwner: 'owner', taskGuardRoute: 'guard-route',
    top: (nav: Env[]) => nav.at(-1), path: (name: string) => name,
    push: (nav: Env[], name: string) => [...nav, { name, id: `visit-${++visit}` }],
    back: (nav: Env[]) => nav.slice(0, -1),
    command: (...args: unknown[]) => { commands.push(args); },
    pairingPrefill: (...args: unknown[]) => {
      const result = answer('pairingPrefill', args);
      expect(result).not.toBeInstanceOf(Promise);
      preparations.push(result as Env);
    },
    showMobileNotice: (...args: unknown[]) => { notices.push(args); },
  };
  const call = (name: string, ...args: unknown[]) => root.call(name, state, scope, args);
  const ui = () => state.pairingUi as Env;
  const settle = (prepared = preparations.at(-1)) => { scope.prepared = prepared; call('didPreparePairing'); };
  const change = (message: string) => { scope.changed = { revision: 0, message }; call('didChange'); };
  const reopen = () => { call('back'); call('addEnvironment'); };
  return { state, commands, preparations, notices, call, ui, settle, change, reopen };
}

describe('mobile pairing visit and retained prefill owners', () => {
  test('discards manual unsubmitted edits on both entry actions without preparing or sending', () => {
    const f = pairingRoot(); f.call('addEnvironment');
    f.call('editHost', '127.0.0.1:1'); f.call('editCode', 'test-local-only');
    f.ui().scannerOpen = true; f.call('scan');
    expect(f.ui()).toMatchObject({ host: '127.0.0.1:1', code: 'test-local-only', scannerOpen: false });
    f.reopen();
    expect(f.ui()).toMatchObject({ host: '', code: '', prefillHost: '', prefillCode: '' });
    const connect = contractComponent('mobile/connect.contract', 'Connect');
    expect(connect.derives({}, { host: f.ui().host, code: f.ui().code, submitting: false }).connectDisabled).toBe(true);
    f.call('editHost', 'unsubmitted.example'); f.call('editCode', 'discard'); f.call('back');
    f.call('detailAction', 'route-add', '', 'saved-environment');
    expect(f.ui()).toMatchObject({ host: '', code: '' });
    expect(f.state.targetEnvironment).toBe('saved-environment');
    expect(f.commands).toEqual([]); expect(f.preparations).toEqual([]);
  });
  test('retains a scan across visits and restores it after unsubmitted manual edits', () => {
    const f = pairingRoot(); f.call('addEnvironment');
    const scan = 'https://scan.example/path#token=scan-code';
    f.ui().scannerOpen = true; f.call('receiveScan', scan); f.settle();
    expect(f.ui()).toMatchObject({ host: 'https://scan.example', code: 'scan-code', prefillUrl: scan, scannerOpen: false });
    f.call('editHost', 'manual.example'); f.call('editCode', 'manual-code');
    f.reopen();
    expect(f.ui()).toMatchObject({ host: 'https://scan.example', code: 'scan-code' });
    f.call('editHost', 'manual.example'); f.call('receiveScan', scan); f.settle();
    expect(f.ui().host).toBe('https://scan.example');
    expect(f.commands).toEqual([]);
  });
  test('retains normalized failed submit fields independently of later manual edits', () => {
    const f = pairingRoot(); f.call('addEnvironment');
    f.call('editHost', '127.0.0.1:1'); f.call('editCode', ' test-local-only '); f.call('connect');
    expect(f.commands).toEqual([['environment-add', '127.0.0.1:1', ' test-local-only ', '', 'owner', 'guard-route']]);
    f.settle(); f.change('Synthetic pairing failure');
    expect(f.ui()).toMatchObject({ host: 'http://127.0.0.1:1', code: 'test-local-only', submitted: false, formError: 'Synthetic pairing failure' });
    f.call('editHost', 'new-manual.example'); f.call('editCode', 'not-submitted');
    expect(f.ui().formError).toBe('');
    f.reopen();
    expect(f.ui()).toMatchObject({ host: 'http://127.0.0.1:1', code: 'test-local-only', formError: '' });
  });
  test('an unchanged submitted URL does not normalize the current raw field edits again', () => {
    const f = pairingRoot(); f.call('addEnvironment');
    f.call('editHost', '127.0.0.1:1'); f.call('editCode', 'code'); f.call('connect'); f.settle(); f.change('Failure');
    const url = f.ui().prefillUrl;
    f.call('editHost', '127.0.0.1:1'); f.call('editCode', ' code '); f.call('connect'); f.settle();
    expect(f.ui()).toMatchObject({ host: '127.0.0.1:1', code: ' code ', prefillUrl: url, prefillHost: 'http://127.0.0.1:1', prefillCode: 'code' });
    expect(f.commands[1]).toEqual(['environment-add', '127.0.0.1:1', ' code ', '', 'owner', 'guard-route']);
    f.change('Failure'); f.reopen();
    expect(f.ui()).toMatchObject({ host: 'http://127.0.0.1:1', code: 'code' });
  });
  test('success clears retained fields and invalidates a normalizer that completes afterward', () => {
    const f = pairingRoot(); f.call('detailAction', 'route-add', '', 'saved-environment');
    f.call('editHost', 'server.example'); f.call('editCode', 'code'); f.call('connect');
    const prepared = f.preparations.at(-1)!;
    expect(f.commands).toEqual([['environment-add-route', 'server.example', 'code', 'saved-environment', 'owner', 'guard-route']]);
    f.change(''); const cleared = { ...f.ui() }; f.settle(prepared);
    expect(f.ui()).toEqual(cleared);
    expect(f.ui()).toMatchObject({ host: '', code: '', prefillUrl: '', prefillHost: '', prefillCode: '', submitted: false });
    expect((f.state.nav as Env[]).at(-1)?.name).toBe('home');
    f.call('addEnvironment'); expect(f.ui()).toMatchObject({ host: '', code: '' });
  });
  test('a newer scan owns retained prefill before an older submit normalization completes', () => {
    const f = pairingRoot(); f.call('addEnvironment');
    f.call('editHost', 'old.example'); f.call('editCode', 'old-code'); f.call('connect');
    const old = f.preparations.at(-1)!;
    f.call('receiveScan', 'https://new.example/#token=new-code'); f.settle();
    const scanned = { ...f.ui() }; f.settle(old);
    expect(f.ui()).toEqual(scanned);
    f.change('Old request failed'); f.reopen();
    expect(f.ui()).toMatchObject({ host: 'https://new.example', code: 'new-code' });
    expect(f.commands).toHaveLength(1);
  });
  test('normalization retains the submitted prefill while protecting subsequent live edits and visits', () => {
    const f = pairingRoot(); f.call('addEnvironment');
    f.call('editHost', 'server.example'); f.call('editCode', 'code'); f.call('connect');
    f.call('editHost', 'new-manual.example'); f.call('editCode', 'new-manual-code'); f.settle();
    expect(f.ui()).toMatchObject({ host: 'new-manual.example', code: 'new-manual-code', prefillHost: 'https://server.example', prefillCode: 'code' });
    f.change('Failure'); f.reopen();
    expect(f.ui()).toMatchObject({ host: 'https://server.example', code: 'code' });
    f.call('editHost', 'later.example'); f.call('connect'); const oldVisit = f.preparations.at(-1)!;
    f.reopen(); const live = { host: f.ui().host, code: f.ui().code }; f.settle(oldVisit);
    expect(f.ui()).toMatchObject(live);
    expect(f.ui().prefillHost).toBe('https://later.example');
  });
});

describe('mobile pairing source', () => {
  test('keeps the copied parser identical to its declared immutable source', () => {
    const pin = '887b2491b182f851b11253655f6aa84fe2a26708';
    const source = 'examples/t3-code/r10-connect-pairing.ts';
    const local = readFileSync(new URL('./shared/r10-connect-pairing.ts', import.meta.url));
    const prefix = Buffer.from('// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.\n'
      + `// Unchanged body from ${source} at ${pin}.\n`);
    expect(local.subarray(0, prefix.length)).toEqual(prefix);
    const result = Bun.spawnSync(['git', 'cat-file', 'blob', `${pin}:${source}`], { cwd: new URL('./', import.meta.url).pathname });
    expect(result.exitCode).toBe(0);
    expect(local.subarray(prefix.length)).toEqual(Buffer.from(result.stdout));
  });
  test('dispatches retained prefill parsing for scans and submits with their captured owners', () => {
    const url = 'https://t3.codes/pair?host=https%3A%2F%2Fserver.example#token=fixture';
    expect(answer('pairingPrefill', ['scan', url, '', 7, 'scan-visit', 3]))
      .toEqual({ url, host: 'https://server.example', code: 'fixture', generation: 7, visit: 'scan-visit', editRevision: 3, scanned: true });
    expect(answer('pairingPrefill', ['submit', '127.0.0.1:1', ' test-local-only ', 8, 'submit-visit', 4]))
      .toEqual({ url: 'http://127.0.0.1:1/#token=test-local-only', host: 'http://127.0.0.1:1', code: 'test-local-only', generation: 8, visit: 'submit-visit', editRevision: 4, scanned: false });
  });
  test('keeps rejected scan input as displayed text without prefilling a code', () => {
    const source = '  https://not a url/pair#token=fixture  ';
    expect(answer('pairingPrefill', ['scan', source, '', 9, 'invalid-scan-visit', 5]))
      .toEqual({ url: source.trim(), host: source.trim(), code: '', generation: 9, visit: 'invalid-scan-visit', editRevision: 5, scanned: true });
  });
  test('answers empty scan input and rejects obsolete or unknown sources', () => {
    expect(answer('pairingPrefill', ['scan', '', '', 0, '', 0]))
      .toEqual({ url: '', host: '', code: '', generation: 0, visit: '', editRevision: 0, scanned: true });
    expect(() => answer('pairingFields', [])).toThrow('Unknown mobile source: pairingFields');
    expect(() => answer('missing', [])).toThrow('Unknown mobile source');
  });
});

describe('generated source dispatcher', () => {
  test('unopened settings choices have a neutral first-frame result', () => {
    expect(answer('settingsChoices', ['', '', false])).toEqual({ key: '', title: '', footer: '', ready: false, options: [] });
  });
  test('scheduled task identifiers stay in the command owner, outside the UI shape', () => {
    const result = answer('automationSnapshot', [{ revision: 0 }, false]);
    expect(result.editor).not.toHaveProperty('taskId');
  });
  test('provider preparation errors stay inside the actual provider snapshot shape', () => {
    const result = answer('providerSnapshot', ['[]', true, { error: 'Cannot read providers', arbitrary: 'not a UI field' }]);
    expect(result).toEqual({ ready: false, error: 'Cannot read providers', emptyMessage: '', sections: [] });
  });
  test('stale new-task projection owners return neutral data without native calls', () => {
    let calls = 0;
    const native = { available: true, watch() {}, async later() { calls++; throw new Error('stale projection reached native'); } };
    expect(answer('voiceFocus', [true, 'Draft', 0, 'stale-owner', 'stale-route'], undefined, undefined, native)).toEqual({ owner: '', label: 'Draft' });
    expect(answer('composerAttachments', [0, 'stale-owner', 'stale-route', 0], undefined, undefined, native)).toMatchObject({ contentOwner: '', items: [], canPick: false });
    expect(answer('newTaskPrepare', ['', true, 0, true, 'e', 'p', '', 1, 'stale-owner', 'stale-route'], undefined, undefined, native)).toMatchObject({ loaded: false });
    expect(answer('mediaPreview', ['', '', '', '', '', true, 0, 0, 0, 'stale-owner', 'stale-route'], undefined, undefined, native)).toMatchObject({ ready: false, identifier: '' });
    expect(answer('attachmentDocument', ['', '', '', true, false, true, 'file', 0, 'stale-owner', 'stale-route'], undefined, undefined, native)).toMatchObject({ ready: false });
    expect(answer('composerSettings', ['', '', false, 0, 'stale-owner', 'stale-route'], undefined, undefined, native)).toMatchObject({ open: false, canEdit: false, models: [] });
    expect(calls).toBe(0);
  });
  test('stale new-task commands and inherited property names refuse before dispatch', () => {
    expect(() => answer('command', ['draft', '', '', '', 'stale-owner', 'stale-route'])).toThrow('Choose a project');
    expect(() => answer('constructor', [])).toThrow('Unknown mobile source: constructor');
    expect(() => answer('__proto__', [])).toThrow('Unknown mobile source: __proto__');
  });
  test('browser and device source names select their exact snapshot shapes', async () => {
    const browser = await answer('browserStatus', ['browser', '', false, { revision: 0 }, 0]);
    const devices = await answer('devicesStatus', ['devices', '', false, { revision: 0 }, 0]);
    expect(browser).toHaveProperty('tabs'); expect(browser).not.toHaveProperty('devices');
    expect(devices).toHaveProperty('devices'); expect(devices).not.toHaveProperty('tabs');
  });
});

describe('stable snapshot reader and pure root projection', () => {
  test('theme, clock and local data projections stay synchronous while the native reader is paused', async () => {
    const { mobileClient, mobileSnapshotProjection } = await import('./client');
    const { mobileGitFeedbackError, mobileGitFeedbackSnapshot } = await import('./git-feedback');
    let enter!: () => void, finish!: () => void, calls = 0;
    const entered = new Promise<void>(resolve => { enter = resolve; });
    const paused = new Promise<void>(resolve => { finish = resolve; });
    const native = { available: true, watch() {}, async later(request: unknown) {
      calls++;
      const operation = (request as { op?: string }).op;
      if (operation === 'status') { enter(); await paused; }
      return { ok: true, generation: mobileClient.generation, value: operation === 'status'
        ? { state: 'disconnected', origin: mobileClient.origin, environmentId: '', message: '' }
        : operation === 'readPreferences' ? { text: '{}' } : operation === 'environments' ? { saved: [] } : {} };
    } };
    const prior = { config: mobileClient.config, shell: mobileClient.shell, connection: mobileClient.connection, environmentId: mobileClient.environmentId,
      thread: mobileClient.thread, threadId: mobileClient.threadId, projectId: mobileClient.projectId };
    const reader = answer('snapshotRead', [], undefined, undefined, native); await entered;
    const atPause = calls;
    mobileClient.connection = 'connected'; mobileClient.environmentId = 'projection-env';
    mobileClient.config = { environment: { label: 'Current environment' } };
    mobileClient.threadId = 'current-thread'; mobileClient.projectId = 'current-project';
    mobileClient.thread = { projection: { thread: { id: 'current-thread' } }, sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null };
    const light = answer('snapshot', ['light', 't3-code', 0, 1000], undefined, undefined, native);
    const dark = answer('snapshot', ['dark', 't3-code', 0, 61000], undefined, undefined, native);
    expect(light).not.toBeInstanceOf(Promise); expect(dark).not.toBeInstanceOf(Promise);
    expect(light.threadId).toBe('current-thread'); expect(dark.threadId).toBe('current-thread');
    expect(light.environments[0]?.statusColor).not.toBe(dark.environments[0]?.statusColor);
    expect(mobileSnapshotProjection(61000).projection).toBe(mobileClient.thread.projection);
    mobileClient.threadId = 'next-thread';
    expect(answer('snapshot', ['dark', 't3-code', 0, 62000], undefined, undefined, native).threadId).toBe('next-thread');
    mobileGitFeedbackError(mobileClient, 1000, 'Synthetic completion');
    answer('snapshot', ['dark', 't3-code', 0, 120000], undefined, undefined, native);
    expect(mobileGitFeedbackSnapshot(120000, mobileClient).deadline).toBe(125000);
    expect(calls).toBe(atPause);
    finish(); await reader; Object.assign(mobileClient, prior);
  });
  test('completed metadata publishes independently of revision and stale reads cannot replace it', async () => {
    const { mobileClient } = await import('./client');
    let finish!: () => void, enter!: () => void;
    const entered = new Promise<void>(resolve => { enter = resolve; });
    const paused = new Promise<void>(resolve => { finish = resolve; });
    let catalogReads = 0;
    const fixture = (label: string, hold: boolean) => ({ available: true, watch() {}, async later(input: unknown) {
      const op = (input as { op?: string }).op;
      if (op === 'environments' && ++catalogReads === 2 && hold) { enter(); await paused; }
      return { ok: true, generation: mobileClient.generation, value: op === 'status'
        ? { state: 'disconnected', origin: mobileClient.origin, environmentId: '', message: '' }
        : op === 'environments' ? { saved: [{ environmentId: label, origin: `https://${label}.invalid`, label, enabled: false }] }
        : op === 'readPreferences' ? { text: '{}' } : {} };
    } });
    const old = answer('snapshotRead', [], undefined, undefined, fixture('old-metadata', true)); await entered;
    const beforeMetadata = mobileClient.revision;
    const current = await answer('snapshotRead', [], undefined, undefined, fixture('new-metadata', false));
    expect(mobileClient.revision).toBe(beforeMetadata);
    const projection = answer('snapshot', ['light', 't3-code', current.serial, 130000]);
    expect(projection.environments.map(row => row.label)).toContain('new-metadata');
    finish(); const stale = await old; expect(stale.serial).toBe(current.serial);
    expect(answer('snapshot', ['light', 't3-code', stale.serial, 130001]).environments.map(row => row.label)).toContain('new-metadata');
    expect(current.serial).toBeGreaterThan(0);
  });
  test('current canonical catalog reaches root and Home while an older metadata reply is held', async () => {
    const { mobileClient } = await import('./client');
    const { MobileDraftClient } = await import('./mobile-draft-recovery');
    const { fleet } = await import('./shared/settings-b-fleet');
    const priorClient = { ...mobileClient }, priorFleet = { ...fleet };
    let finish!: () => void, enter!: () => void, catalogs = 0;
    const entered = new Promise<void>(resolve => { enter = resolve; });
    const paused = new Promise<void>(resolve => { finish = resolve; });
    const old = { environmentId: 'old-catalog', origin: 'https://old.invalid', mobileLabel: 'Old catalog', enabled: false };
    const current = { environmentId: 'current-catalog', origin: 'https://current.invalid', mobileLabel: 'Current catalog', enabled: false };
    const native = { available: true, watch() {}, async later(input: unknown) {
      const op = (input as { op?: string }).op;
      if (op === 'environments' && ++catalogs === 2) { enter(); await paused; }
      return { ok: true, generation: 1, value: op === 'status'
        ? { state: 'disconnected', origin: '', environmentId: '', message: '' }
        : op === 'environments' ? { saved: [old] } : op === 'readPreferences' ? { text: '{}' } : {} };
    } };
    try {
      Object.assign(mobileClient, new MobileDraftClient()); Object.assign(fleet, { saved: [], entries: new Map(), revision: 0 });
      const read = answer('snapshotRead', [], undefined, undefined, native); await entered;
      const revision = fleet.revision; fleet.saved = [current];
      const assertCurrent = () => {
        const root = answer('snapshot', ['light', 't3-code', 0, 1000]);
        expect(root.environments.map(row => row.label)).toEqual(['Current catalog']);
        return root;
      };
      const root = assertCurrent(); expect(fleet.revision).toBe(revision);
      const home = await answer('homeView', [root.revision, 1000, '', 10, true, false, false, false, false,
        '', '', '', 'repository', 'catalog-boundary', true, false, root.environments]);
      expect(home.emptyTitle).toBe('Environment unavailable'); expect(home.loading).toBe(false);
      finish(); await read; assertCurrent(); expect(fleet.saved).toEqual([current]);
    } finally { finish(); Object.assign(mobileClient, priorClient); Object.assign(fleet, priorFleet); }
  });
  test('root metadata cannot overwrite preferences applied by the current appearance resource', async () => {
    const { mobileClient } = await import('./client');
    const { MobileDraftClient } = await import('./mobile-draft-recovery');
    const { fleet } = await import('./shared/settings-b-fleet');
    const priorClient = { ...mobileClient }, priorFleet = { ...fleet };
    let finish!: () => void, enter!: () => void;
    const entered = new Promise<void>(resolve => { enter = resolve; });
    const paused = new Promise<void>(resolve => { finish = resolve; });
    let preferences = { planModeEnabled: false, followUpBehavior: 'queue' };
    const topics: string[] = [], announced: string[] = [];
    const native = { available: true, watch(topic: string) { topics.push(topic); }, async later(input: unknown) {
      const request = input as { op?: string; patch?: Partial<typeof preferences> }, op = request.op;
      // Unique root metadata boundary; refresh/fleet do not ask for connectionPreferences.
      if (op === 'connectionPreferences') { enter(); await paused; }
      if (op === 'mobilePreferencesPatch') {
        preferences = { ...preferences, ...request.patch }; announced.push('t3.mobile-preferences');
      }
      return { ok: true, generation: 1, value: op === 'status'
        ? { state: 'disconnected', origin: '', environmentId: '', message: '' }
        : op === 'environments' ? { saved: [] } : op === 'readPreferences' ? { text: '{}' }
          : op === 'mobilePreferences' || op === 'mobilePreferencesPatch' ? { ...preferences, revision: 1 } : {} };
    } };
    try {
      Object.assign(mobileClient, new MobileDraftClient()); Object.assign(fleet, { saved: [], entries: new Map(), revision: 0 });
      const pending = answer('snapshotRead', [], undefined, undefined, native); await entered;
      expect((await answer('preferenceChange', ['planModeEnabled', 'true'], undefined, undefined, native)).message).toBe('');
      expect((await answer('preferenceChange', ['followUpBehavior', 'steer'], undefined, undefined, native)).message).toBe('');
      const current = await answer('preferences', [], undefined, undefined, native);
      answer('appearance', [current.serialized, 'light']);
      expect(mobileClient.local.deviceSettings.planModeEnabled).toBe(true);
      expect(mobileClient.local.clientSettings.followUpBehavior).toBe('steer');
      finish(); await pending;
      expect(mobileClient.local.deviceSettings.planModeEnabled).toBe(true);
      expect(mobileClient.local.clientSettings.followUpBehavior).toBe('steer');
      expect(topics).toContain('t3.mobile-preferences');
      expect(announced).toEqual(['t3.mobile-preferences', 't3.mobile-preferences']);
    } finally { finish(); Object.assign(mobileClient, priorClient); Object.assign(fleet, priorFleet); }
  });
  test('appearance projects current composer preferences without native IO', () => {
    let calls = 0;
    const native = { available: true, watch() {}, async later() { calls++; throw new Error('Appearance reached native'); } };
    const projected = answer('appearance', [JSON.stringify({ planModeEnabled: true, followUpBehavior: 'steer' }), 'light'], undefined, undefined, native);
    expect(projected.scheme).toBe('light'); expect(calls).toBe(0);
  });
  test('local Thread switches retain unavailable and invalid-write errors before preferences load', async () => {
    let calls = 0;
    const native = { available: false, watch() {}, async later() { calls++; throw new Error('Unavailable preference owner was called'); } };
    const before = await answer('preferences', [], undefined, undefined, native);
    expect(before.ready).toBe(false);
    for (const key of ['workingEnabled', 'planModeEnabled']) {
      const result = await answer('preferenceChange', [key, 'true'], undefined, undefined, native);
      expect(result).toEqual({ revision: 0, message: before.error });
      native.available = true;
      expect(await answer('preferenceChange', [key, 'yes'], undefined, undefined, native))
        .toEqual({ revision: 0, message: 'Choose on or off.' });
      native.available = false;
    }
    expect(await answer('preferences', [], undefined, undefined, native)).toEqual(before);
    expect(calls).toBe(0);
  });
});


describe('pending local preference choices', () => {
  const resource = () => ({ ready: false, error: 'Read pending', revision: 7,
    serialized: JSON.stringify({ ready: false, error: 'Read pending', revision: 7, baseFontSize: 19,
      composerEnterBehavior: 'send', followUpBehavior: 'queue', projectGroupingMode: 'repository' }) });
  const choice = (key: string, value: string, extra = {}) => ({ mutation: 'choicePreferenceChanged',
    source: 'preferenceChange', args: [key, value], answered: false, ...extra });
  test('folds queued choices per field without changing read metadata or inputs', () => {
    const before = resource(), writes = [choice('composerEnterBehavior', 'newline'), choice('followUpBehavior', 'steer'),
      choice('composerEnterBehavior', 'send'), choice('projectGroupingMode', 'separate')];
    const frozen = JSON.stringify({ before, writes });
    const effective = overlayMobilePreferenceChoices(before, writes)!;
    expect(effective).toEqual({ ...before, serialized: JSON.stringify({ ...JSON.parse(before.serialized),
      composerEnterBehavior: 'send', followUpBehavior: 'steer', projectGroupingMode: 'separate' }) });
    expect(JSON.stringify({ before, writes })).toBe(frozen);
  });
  test('old failures and answered writes cannot remove a newer equal-valued intent', () => {
    const before = resource();
    const effective = overlayMobilePreferenceChoices(before, [choice('composerEnterBehavior', 'newline', { reply: { revision: 0, message: 'No' } }),
      choice('followUpBehavior', 'steer', { answered: true, reply: { revision: 8, message: '' } }),
      choice('composerEnterBehavior', 'newline')])!;
    expect(JSON.parse(effective.serialized as string)).toEqual({ ...JSON.parse(before.serialized), composerEnterBehavior: 'newline' });
    expect(overlayMobilePreferenceChoices(before, [choice('composerEnterBehavior', 'newline', { reply: { revision: 8, message: '' } })])).toBeDefined();
    expect(overlayMobilePreferenceChoices(before, [choice('composerEnterBehavior', 'newline', { answered: true, reply: { revision: 8, message: '' } })])).toBeUndefined();
  });
  test('refuses other lanes, malformed replies, invalid choices and malformed answers', () => {
    const before = resource();
    expect(overlayMobilePreferenceChoices(before, [choice('composerEnterBehavior', 'newline', { mutation: 'preferenceChanged' }),
      choice('composerEnterBehavior', 'newline', { source: 'other' }), choice('workingEnabled', 'true'),
      choice('composerEnterBehavior', 'unknown'), choice('__proto__', 'newline'),
      choice('followUpBehavior', 'steer', { reply: {} })])).toBeUndefined();
    for (const serialized of ['bad', 'null', '[]', '1'])
      expect(overlayMobilePreferenceChoices({ ...before, serialized }, [choice('composerEnterBehavior', 'newline')])).toBeUndefined();
    expect(overlayMobilePreferenceChoices({}, [choice('composerEnterBehavior', 'newline')])).toBeUndefined();
  });
});
