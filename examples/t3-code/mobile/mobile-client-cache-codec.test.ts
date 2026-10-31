import { describe, expect, test } from 'bun:test';
import { applyShell, initialShell, messages, obj, threadSnapshot, type Obj } from './shared/domain';
import {
  decodeMobileConfigCache, decodeMobileShellCache, decodeMobileThreadCache,
  encodeMobileConfigCache, encodeMobileShellCache, encodeMobileThreadCache,
  MOBILE_CLIENT_CACHE_FORMAT, MOBILE_CLIENT_CACHE_SCHEMA_VERSION,
} from './mobile-client-cache-codec';

// Complete base shell/projection fields from pin365aa87982's
// packages/client-runtime/src/state/orchestrationV2TestFixtures.ts, with JSON
// timestamps as its Stored*Json schema encodes them. No Effect runtime is imported.
const now = '2026-06-20T00:00:00.000Z', environmentId = 'environment-cache-test', threadId = 'thread-v2';
function fixture() {
  const project: Obj = { id: 'project-v2', title: 'Project 中文', workspaceRoot: '/workspace/project', repositoryIdentity: null,
    defaultModelSelection: null, scripts: [], createdAt: now, updatedAt: now };
  const thread: Obj = { id: threadId, projectId: project.id, title: 'Thread', providerInstanceId: 'codex',
    modelSelection: { instanceId: 'codex', model: 'gpt-5.4' }, runtimeMode: 'full-access', interactionMode: 'default',
    branch: null, worktreePath: null, activeProviderThreadId: null,
    lineage: { rootThreadId: threadId, parentThreadId: null, relationshipToParent: null }, forkedFrom: null,
    createdBy: 'user', creationSource: 'web', createdAt: now, updatedAt: now, archivedAt: null,
    settledOverride: null, settledAt: null, lastVisitedAt: null, deletedAt: null };
  const shellRow = { ...thread, latestRunId: null, activeRunId: null, status: 'idle',
    pendingRuntimeRequest: { id: 'request-v2', kind: 'command', createdAt: now },
    latestVisibleMessage: { id: 'message-v2', role: 'assistant', text: 'Done', updatedAt: now },
    latestUserMessageAt: now, hasActionableProposedPlan: false, itemCount: 200, visibleItemCount: 200,
    titleRegeneration: { requestId: 'title-regeneration-v2', startedAt: now } };
  const item: Obj = { id: 'notice-cache', threadId, runId: null, nodeId: null, providerThreadId: null,
    providerTurnId: null, nativeItemRef: null, parentItemId: null, ordinal: 190, status: 'completed', title: null,
    startedAt: now, completedAt: now, updatedAt: now, type: 'system_notice', message: 'Provider changed after safeguards.' };
  const projection: Obj = { thread, runs: [], attempts: [], nodes: [], subagents: [], providerSessions: [], providerThreads: [],
    providerTurns: [], runtimeRequests: [], messages: [], plans: [], turnItems: [item], checkpointScopes: [], checkpoints: [],
    contextHandoffs: [], contextTransfers: [], visibleTurnItems: [
      { position: 0, visibility: 'local', sourceThreadId: threadId, sourceItemId: item.id, item },
    ], updatedAt: now };
  return { wireShell: { schemaVersion: 1, snapshotSequence: 8, projects: [project], threads: [shellRow],
    archivedThreads: [{ ...shellRow, id: 'archived', archivedAt: now }] },
    wireThread: { projection, snapshotSequence: 14, historyCursor: 'opaque-before-190', hasMoreHistory: true, latestLocalTurnOrdinal: 200 } };
}
function rewrite(text: string, update: (saved: any) => void): string {
  const saved = JSON.parse(text); update(saved); return JSON.stringify(saved);
}

const configFixture = (): Obj => ({ environment: { environmentId, label: 'My machine', orchestrationProtocolVersion: 2,
  capabilities: { serverResolvedCommandContext: true, environmentThemes: true, usageLimitSources: true } },
  auth: { mode: 'required' }, cwd: '/workspace/project', keybindingsConfigPath: '/config/keybindings.json',
  keybindings: [], issues: [], availableEditors: ['vscode'], observability: {},
  providers: [{ instanceId: 'codex', driver: 'codex', displayName: 'Codex', enabled: true, installed: true,
    auth: { status: 'authenticated' }, status: 'ready', models: [{ slug: 'gpt-5.4', name: 'GPT-5.4' }] }],
  settings: { defaultModelSelection: { instanceId: 'codex', model: 'gpt-5.4' }, defaultRuntimeMode: 'full-access' },
  shellResumeCompletionMarker: true, threadResumeCompletionMarker: true,
  environmentThemes: [{ id: 'old-theme' }], usageLimitSources: [{ id: 'old-quota' }],
});

describe('app-owned mobile read cache codec', () => {
  test('labels normalized active shell honestly and preserves complete retained rows', () => {
    const { wireShell } = fixture(), shell = applyShell(initialShell(), wireShell);
    const encoded = encodeMobileShellCache(environmentId, shell), saved = JSON.parse(encoded);
    expect(saved.format).toBe(MOBILE_CLIENT_CACHE_FORMAT);
    expect(saved.schemaVersion).toBe(MOBILE_CLIENT_CACHE_SCHEMA_VERSION);
    expect(saved.schemaVersion).toBe(1);
    expect(saved.snapshot.archivedThreads).toBeUndefined();
    expect(saved.snapshot.schemaVersion).toBeUndefined();
    const restored = decodeMobileShellCache(encoded, environmentId)!;
    expect(restored).toEqual(shell);
    expect(obj(restored.threads[0]!.latestVisibleMessage).updatedAt).toBe(now);
    expect(obj(restored.threads[0]!.titleRegeneration).requestId).toBe('title-regeneration-v2');
    shell.projects[0]!.title = 'Changed after capture';
    expect(restored.projects[0]!.title).toBe('Project 中文');
  });

  test('preserves committed projection, opaque cursor and local ordinal as one snapshot', () => {
    const thread = threadSnapshot(fixture().wireThread), text = encodeMobileThreadCache(environmentId, threadId, thread);
    const restored = decodeMobileThreadCache(text, environmentId, threadId)!;
    expect(restored).toEqual(thread);
    expect(restored.sequence).toBe(14);
    expect(restored.historyCursor).toBe('opaque-before-190');
    expect(restored.hasMore).toBe(true);
    expect(restored.latestLocalTurnOrdinal).toBe(200);
    expect(messages(restored).map(message => message.body)).toEqual(['Provider changed after safeguards.']);
    obj(thread.projection.thread).title = 'Changed after capture';
    expect(obj(restored.projection.thread).title).toBe('Thread');
  });

  test('explicit full-history nulls survive without inventing a page boundary', () => {
    const wire = { ...fixture().wireThread, historyCursor: null, hasMoreHistory: false, latestLocalTurnOrdinal: null };
    const thread = threadSnapshot(wire);
    expect(decodeMobileThreadCache(encodeMobileThreadCache(environmentId, threadId, thread), environmentId, threadId)).toEqual(thread);
  });

  test('removes themes and quota on save AND load while keeping offline provider catalog', () => {
    const config = configFixture(), text = encodeMobileConfigCache(environmentId, config), saved = JSON.parse(text);
    expect(saved.config.environmentThemes).toBeUndefined();
    expect(saved.config.usageLimitSources).toBeUndefined();
    const injected = rewrite(text, record => { record.config.environmentThemes = [{ id: 'stale' }]; record.config.usageLimitSources = [{ id: 'stale' }]; });
    const restored = decodeMobileConfigCache(injected, environmentId)!;
    expect(restored.environmentThemes).toBeUndefined();
    expect(restored.usageLimitSources).toBeUndefined();
    expect(restored.providers).toEqual(config.providers);
    expect(restored.settings).toEqual(config.settings);
    expect(config.environmentThemes).toEqual([{ id: 'old-theme' }]);
    expect(config.usageLimitSources).toEqual([{ id: 'old-quota' }]);
  });

  test.each(['environmentId', 'format', 'kind', 'schemaVersion'])('rejects incompatible %s across all record families', field => {
    const f = fixture();
    const values = [encodeMobileShellCache(environmentId, applyShell(initialShell(), f.wireShell)),
      encodeMobileThreadCache(environmentId, threadId, threadSnapshot(f.wireThread)), encodeMobileConfigCache(environmentId, configFixture())];
    const changed = values.map(text => rewrite(text, saved => { saved[field] = field === 'schemaVersion' ? 3 : 'wrong'; }));
    expect(decodeMobileShellCache(changed[0]!, environmentId)).toBeNull();
    expect(decodeMobileThreadCache(changed[1]!, environmentId, threadId)).toBeNull();
    expect(decodeMobileConfigCache(changed[2]!, environmentId)).toBeNull();
  });

  test.each(['{', 'null', '[]', '42', '"text"'])('discards malformed envelope %s', text => {
    expect(decodeMobileShellCache(text, environmentId)).toBeNull();
    expect(decodeMobileThreadCache(text, environmentId, threadId)).toBeNull();
    expect(decodeMobileConfigCache(text, environmentId)).toBeNull();
  });

  test('reuses shell validation and cannot mistake a completion marker for a cached snapshot', () => {
    const text = encodeMobileShellCache(environmentId, applyShell(initialShell(), fixture().wireShell));
    for (const bad of [{ kind: 'synchronized' }, { snapshotSequence: -1, projects: [], threads: [] },
      { snapshotSequence: 1, projects: [{ id: null }], threads: [] }, { snapshotSequence: 1, projects: [], threads: 'bad' }]) {
      expect(decodeMobileShellCache(rewrite(text, saved => { saved.snapshot = bad; }), environmentId)).toBeNull();
    }
  });

  test('checks both thread identities and refuses missing or corrupt history metadata', () => {
    const text = encodeMobileThreadCache(environmentId, threadId, threadSnapshot(fixture().wireThread));
    expect(decodeMobileThreadCache(text, environmentId, 'other')).toBeNull();
    expect(decodeMobileThreadCache(rewrite(text, saved => { saved.snapshot.projection.thread.id = 'other'; }), environmentId, threadId)).toBeNull();
    for (const key of ['historyCursor', 'hasMoreHistory', 'latestLocalTurnOrdinal']) {
      expect(decodeMobileThreadCache(rewrite(text, saved => { delete saved.snapshot[key]; }), environmentId, threadId)).toBeNull();
      expect(decodeMobileThreadCache(rewrite(text, saved => { saved.snapshot[key] = {}; }), environmentId, threadId)).toBeNull();
    }
    expect(decodeMobileThreadCache(rewrite(text, saved => { saved.snapshot.projection.visibleTurnItems[0].item.ordinal = -1; }), environmentId, threadId)).toBeNull();
    expect(decodeMobileThreadCache(rewrite(text, saved => { delete saved.snapshot.projection.runs; }), environmentId, threadId)).toBeNull();
  });

  test('config identity and malformed provider containers are rejected', () => {
    const text = encodeMobileConfigCache(environmentId, configFixture());
    expect(decodeMobileConfigCache(rewrite(text, saved => { saved.config.environment.environmentId = 'other'; }), environmentId)).toBeNull();
    for (const providers of [null, {}, [null], ['bad']]) {
      expect(decodeMobileConfigCache(rewrite(text, saved => { saved.config.providers = providers; }), environmentId)).toBeNull();
    }
  });

  test('invalid adopted input is refused before persistence', () => {
    const f = fixture(), thread = threadSnapshot(f.wireThread);
    expect(() => encodeMobileShellCache('', applyShell(initialShell(), f.wireShell))).toThrow();
    expect(() => encodeMobileThreadCache(environmentId, 'other', thread)).toThrow();
    expect(() => encodeMobileConfigCache('other', configFixture())).toThrow();
    expect(() => encodeMobileShellCache(environmentId, { projects: [], threads: [], sequence: NaN })).toThrow();
  });
});

test('deleted thread projections cannot be saved or restored as offline history', () => {
  const thread = threadSnapshot(fixture().wireThread), valid = encodeMobileThreadCache(environmentId, threadId, thread);
  thread.projection.thread = { ...obj(thread.projection.thread), deletedAt: now };
  expect(() => encodeMobileThreadCache(environmentId, threadId, thread)).toThrow('Deleted thread');
  const deleted = rewrite(valid, saved => { saved.snapshot.projection.thread.deletedAt = now; });
  expect(decodeMobileThreadCache(deleted, environmentId, threadId)).toBeNull();
});
