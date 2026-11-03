// What a server built at 5d3d78ab93 adds that this client reads before the features that use it (T3 Code 5d3d78ab93,
// MIT, see LICENSE-T3: packages/contracts scheduledTask.ts, threadTags.ts, settings.ts, usage.ts, managedPolicy.ts,
// orchestrationV2.ts, model.ts; packages/client-runtime modelTraits.ts, state/server.ts, state/usage.ts; packages/shared
// t3McpToolPresentation.ts): once schedules, thread tags and their event, the usage model filter, the managed policy,
// model switches, reasoning by role, the Kiro driver, subagent work and Codex's direct-only T3 tools.
import { describe, expect, test } from 'bun:test';
import { applyThread, messages, obj, threadSnapshot, type Obj } from '../../src/client/domain';
import { hiddenThreadTags, isIsoWithOffset, isReasoningControl, isReasoningSelect, onceSchedule, oneTimeSchedules, sameUsageModelFilter, threadTags, usageModelFilter,
  usageSummaryInput } from '../../src/client/server-fields';
import { clampManagedRuntimeMode, isManagedDriverAllowed, isManagedSettingPath, managedCapabilityDenial, managedPolicySummary, managedRuntimeModeDenial,
  managedUpdatesDenial } from '../../src/client/managed-policy';
import { failure } from '../../src/transport/transport-wire';
import { providerDisplayName } from '../../src/providers/provider-updates';
import { DEFAULT_MODEL_BY_PROVIDER } from '../../src/composer/composer-provider-selection';
import { resolveName } from '../../src/timeline/timeline-t3tools';
import { connected } from '../support/client-fixture';

const at = '2026-11-03T00:00:00.000Z';

describe('once schedules', () => {
  test('runAt is an ISO date and time with an offset that names a real day', () => {
    expect(['2026-11-03T09:30-08:00', '2026-11-03T17:30:00Z', '2028-02-29T00:00:00.5+09:00'].map(isIsoWithOffset)).toEqual([true, true, true]);
    expect(['2026-11-03T09:30', '2026-02-30T09:30Z', '2027-02-29T00:00Z', '2026-11-03 09:30Z', '2026-11-03T24:00Z'].map(isIsoWithOffset)).toEqual([false, false, false, false, false]);
  });

  test('a once schedule reads with its trimmed runAt; other schedules and a bad time do not; the capability says the server takes them', () => {
    expect(onceSchedule({ type: 'once', runAt: ' 2026-11-03T09:30-08:00 ' })).toEqual({ type: 'once', runAt: '2026-11-03T09:30-08:00' });
    expect([onceSchedule({ type: 'interval', everyMs: 60000 }), onceSchedule({ type: 'once', runAt: '2026-11-03T09:30' }), onceSchedule(undefined)]).toEqual([null, null, null]);
    expect([oneTimeSchedules({ environment: { capabilities: { oneTimeSchedules: true } } }), oneTimeSchedules({ environment: { capabilities: {} } })]).toEqual([true, false]);
  });
});

describe('thread tags', () => {
  test('stored tags read leniently: any entry that is not a string empties them', () => {
    expect([threadTags({ tags: ['private', 'demo'] }), threadTags({}), threadTags({ tags: ['ok', 3] }), threadTags({ tags: 'private' })]).toEqual([['private', 'demo'], [], [], []]);
  });

  test('hidden tags apply only while hideTaggedThreads is on; entries are trimmed and bad ones drop alone', () => {
    expect(hiddenThreadTags({ hiddenThreadTags: ['private'] })).toBeNull();
    expect(hiddenThreadTags({ hideTaggedThreads: true, hiddenThreadTags: [' private', 7, '', 'demo '] })).toEqual(['private', 'demo']);
    expect(hiddenThreadTags({ hideTaggedThreads: true, hiddenThreadTags: ['  '] })).toBeNull();
    expect(hiddenThreadTags({ hideTaggedThreads: true })).toBeNull();
  });

  test('thread.tags-updated replaces the thread without counting as activity', () => {
    const thread = { id: 't', title: 'T', projectId: 'p', createdAt: at, updatedAt: at };
    const state = threadSnapshot({ snapshotSequence: 1, projection: { thread, runs: [], attempts: [], nodes: [], subagents: [], providerSessions: [], providerThreads: [],
      providerTurns: [], runtimeRequests: [], messages: [], plans: [], turnItems: [], checkpointScopes: [], checkpoints: [], contextHandoffs: [], contextTransfers: [],
      visibleTurnItems: [], updatedAt: at } });
    const next = applyThread(state, { kind: 'event', sequence: 2, event: { type: 'thread.tags-updated', threadId: 't', occurredAt: '2026-11-03T01:00:00.000Z',
      payload: { ...thread, tags: ['private'] } } });
    expect(next.sequence).toBe(2);
    expect(threadTags(obj(next.projection.thread))).toEqual(['private']);
    expect(next.projection.updatedAt).toBe(at);
  });
});

describe('the usage model filter', () => {
  test('a valid filter is sent with each summary request; an invalid one is ignored', () => {
    const filter = usageModelFilter({ usageModelFilter: { include: [{ provider: 'codex', model: 'gpt-6-astra' }], exclude: [{ model: 'gpt-6-luna' }] } });
    expect(filter).toEqual({ include: [{ provider: 'codex', model: 'gpt-6-astra' }], exclude: [{ model: 'gpt-6-luna' }] });
    expect([usageModelFilter({}), usageModelFilter({ usageModelFilter: { include: [{ provider: 'codex' }] } }), usageModelFilter({ usageModelFilter: [] })]).toEqual([undefined, undefined, undefined]);
    expect(usageSummaryInput({ range: '7d' }, filter)).toEqual({ range: '7d', modelFilter: filter });
    expect(usageSummaryInput({ range: '7d' }, undefined)).toEqual({ range: '7d' });
  });

  test('a summary under another filter is not a missing one', () => {
    expect(sameUsageModelFilter({}, {})).toBe(true);
    expect(sameUsageModelFilter({ modelFilter: { exclude: [{ model: 'm' }] } }, {})).toBe(false);
  });
});

describe('the managed policy', () => {
  const config = { managedPolicy: { organization: ' Example Corp ', lockedSettings: ['telemetry', 'storageCleanup.logsAfterDays'],
    limits: { maxRuntimeMode: 'auto-accept-edits', clients: ['desktop', 'local-browser'], webhookTasks: false, providers: { allowedDrivers: ['codex'] }, updates: { mode: 'off' } } } };

  test('the summary reads; an unmanaged or malformed one is absent', () => {
    expect(managedPolicySummary(config)).toMatchObject({ organization: 'Example Corp', lockedSettings: ['telemetry', 'storageCleanup.logsAfterDays'] });
    expect([managedPolicySummary({}), managedPolicySummary({ managedPolicy: { organization: 'X' } }), managedPolicySummary({ managedPolicy: { organization: 'X', lockedSettings: [], limits: { maxRuntimeMode: 'yolo' } } })])
      .toEqual([undefined, undefined, undefined]);
  });

  test('locked paths, the runtime ceiling, capabilities, drivers and updates read as the reference words them', () => {
    const summary = managedPolicySummary(config);
    expect(['telemetry', 'storageCleanup', 'storageCleanup.logsAfterDays', 'storageCleanup.other', 'telemetryX'].map(path => isManagedSettingPath(summary, path))).toEqual([true, true, true, false, false]);
    expect(managedRuntimeModeDenial(summary, 'full-access')).toBe('Example Corp limits threads to Auto-accept edits.');
    expect(managedRuntimeModeDenial(summary, 'approval-required')).toBeUndefined();
    expect(clampManagedRuntimeMode(summary, 'full-access')).toBe('auto-accept-edits');
    expect(managedCapabilityDenial(summary, 'remoteClients')).toBe("Example Corp's policy does not allow connecting from other devices.");
    expect(managedCapabilityDenial(summary, 'webhookTasks')).toBe("Example Corp's policy does not allow webhook tasks.");
    expect(managedCapabilityDenial(summary, 'connect')).toBeUndefined();
    expect([isManagedDriverAllowed(summary, 'codex'), isManagedDriverAllowed(summary, 'cursor'), isManagedDriverAllowed(undefined, 'cursor')]).toEqual([true, false, true]);
    expect(managedUpdatesDenial(summary)).toBe('Example Corp manages T3 Code updates.');
  });

  test("a settings write the policy refuses reads the reference's sentence", () => {
    const refusal = (error: Obj) => failure({ _tag: 'Fail', error: { _tag: 'ServerSettingsError', settingsPath: '/h/settings.json', ...error } }).message;
    expect(refusal({ operation: 'write-managed-setting', managedSetting: 'telemetry', managedBy: 'Example Corp' })).toBe('telemetry is managed by Example Corp and cannot be changed.');
    expect(refusal({ operation: 'use-disallowed-provider', managedBy: 'Example Corp', providerLabel: 'Cursor' })).toBe("Example Corp's policy does not allow the Cursor provider.");
    expect(refusal({ operation: 'use-disallowed-provider' })).toBe('Your organization policy does not allow the this provider.');
    expect(refusal({ operation: 'write', providerInstanceId: 'codex' })).toBe('Server settings write failed for provider codex at /h/settings.json.');
  });
});

describe('models', () => {
  test('a select its agent marks as reasoning is the reasoning control, as the known effort ids are', () => {
    expect(isReasoningControl({ id: 'thought_level', type: 'select', role: 'reasoning' })).toBe(true);
    expect(isReasoningControl({ id: 'thought_level', type: 'select' })).toBe(false);
    expect(isReasoningSelect({ id: 'reasoningEffort', type: 'select' })).toBe(true);
    expect(isReasoningSelect({ id: 'thinking', type: 'boolean' })).toBe(false);
  });

  test("a user message's model switch decodes; a malformed one fails the thread as any known field does", () => {
    const item = (extra: Obj) => ({ id: 'u', threadId: 't', runId: 'r', nodeId: 'n', ordinal: 1, type: 'user_message', status: 'completed', text: 'Hi', messageId: 'm',
      inputIntent: 'turn_start', attachments: [], ...extra });
    const snapshot = (list: Obj[]) => threadSnapshot({ snapshotSequence: 1, projection: { thread: { id: 't', title: 'T' }, runs: [], attempts: [], nodes: [], subagents: [],
      providerSessions: [], providerThreads: [], providerTurns: [], runtimeRequests: [], messages: [], plans: [], turnItems: list, checkpointScopes: [], checkpoints: [],
      contextHandoffs: [], contextTransfers: [], updatedAt: at,
      visibleTurnItems: list.map((entry, position) => ({ position, visibility: 'local', sourceThreadId: 't', sourceItemId: entry.id, item: entry })) } });
    const modelSwitch = { from: { instanceId: 'codex', model: 'gpt-6-luna' }, to: { instanceId: 'codex', model: 'gpt-6-astra', options: [{ id: 'reasoningEffort', value: 'high' }] } };
    expect(messages(snapshot([item({ modelSwitch })]))[0]).toMatchObject({ kind: 'user', body: 'Hi', modelSwitch });
    expect(messages(snapshot([item({})]))[0]!.modelSwitch).toBeUndefined();
    expect(() => snapshot([item({ modelSwitch: { from: { instanceId: 'codex' }, to: modelSwitch.to } })])).toThrow('modelSwitch.from.model');
  });

  test('Kiro is a driver: its name, and "default" keeps the session model', () => {
    expect(providerDisplayName('kiro')).toBe('Kiro');
    expect(DEFAULT_MODEL_BY_PROVIDER.kiro).toBe('default');
  });
});

describe("Codex's direct-only T3 MCP server", () => {
  test('t3-code-direct and t3_code_direct name T3 tools, as the other T3 server names do', () => {
    expect(['mcp__t3-code-direct__t3_thread_wait', 'mcp__t3_code_direct__t3_thread_wait', 't3-code-direct.t3_thread_wait', 't3_code_direct · t3_thread_wait', 'mcp__t3-code__t3_thread_wait']
      .map(resolveName)).toEqual(Array(5).fill('t3_thread_wait'));
    expect(resolveName('mcp__other-direct__t3_thread_wait')).toBeNull();
  });
});

describe("a subagent's native child work", () => {
  test('orchestration.getNativeChildWork names the thread, needs orchestration read, and reads the children', async () => {
    const context = await connected(), calls: Obj[] = [], later = context.native.later.bind(context.native);
    context.client.scopes = ['orchestration:read'];
    context.native.later = async request => {
      const call = obj(request);
      if (call.op === 'request' && call.method === 'orchestration.getNativeChildWork') {
        calls.push({ method: call.method, payload: call.payload });
        return context.native.good({ children: [
          { conversationId: 'c1', role: 'researcher', prompt: 'Look', truncated: true, steps: [{ index: 0, type: 'tool', status: null, text: 'Read', toolCalls: [{ name: 'grep', input: '{}' }] }] },
          { conversationId: 'c2', role: 'writer', prompt: 'Write', truncated: false, steps: null },
          { conversationId: ' ', role: 'none', prompt: '', truncated: false, steps: null },
        ] });
      }
      return later(request);
    };
    expect(await context.client.getNativeChildWork(context.native, 't1')).toEqual([
      { conversationId: 'c1', role: 'researcher', prompt: 'Look', truncated: true, steps: [{ index: 0, type: 'tool', status: null, text: 'Read', toolCalls: [{ name: 'grep', input: '{}' }] }] },
      { conversationId: 'c2', role: 'writer', prompt: 'Write', truncated: false, steps: null },
    ]);
    expect(calls).toEqual([{ method: 'orchestration.getNativeChildWork', payload: { threadId: 't1' } }]);
    expect(failure({ _tag: 'Fail', error: { _tag: 'OrchestrationV2GetNativeChildWorkError', threadId: 't1' } }).message).toBe("Failed to read the subagent's work.");
  });
});
