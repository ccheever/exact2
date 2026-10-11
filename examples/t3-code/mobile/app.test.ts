import { readFileSync } from 'node:fs';
import { describe, expect, test } from 'bun:test';
import { answer } from './app';

describe('mobile pairing source', () => {
  test('keeps the copied parser identical to the adopted desktop source', () => {
    const local = readFileSync(new URL('./shared/r10-connect-pairing.ts', import.meta.url), 'utf8');
    const desktop = readFileSync(new URL('../r10-connect-pairing.ts', import.meta.url), 'utf8');
    expect(local.split('\n').slice(2).join('\n')).toBe(desktop);
  });
  test('uses the shared hosted-link parser', () => {
    expect(answer('pairingFields', ['https://t3.codes/pair?host=https%3A%2F%2Fserver.example#token=fixture']))
      .toEqual({ source: 'https://t3.codes/pair?host=https%3A%2F%2Fserver.example#token=fixture', host: 'https://server.example', code: 'fixture' });
  });
  test('answers empty input at bake and rejects unknown sources', () => {
    expect(answer('pairingFields', [''])).toEqual({ source: '', host: '', code: '' });
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
});
