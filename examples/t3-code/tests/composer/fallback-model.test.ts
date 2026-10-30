// A new thread falls back when the default provider is out of usage (T3 Code v0.1.5 usageLimits.test.ts, #26164).
import { describe, expect, test } from 'bun:test';
import { exhaustedUsageWindow, resolveNewThreadModelSelection, startOnFallback, draftFallbackOf } from '../../src/composer/composer-provider-selection';
import { usageNotices } from '../../src/composer/composer-controls-usage';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';

const now = Date.parse('2026-09-03T12:00:00Z');
const window = { id: 'weekly', usedPercent: 50, resetsAt: '2026-09-03T18:00:00.000Z' };
const provider = (patch: Obj = {}): Obj => ({ instanceId: 'codex', driver: 'codex', displayName: 'Codex', enabled: true, installed: true, auth: { status: 'authenticated' },
  availability: 'available', models: [{ slug: 'gpt-6.1-sol', name: 'GPT-6.1 Sol' }], usageLimits: { windows: [window] }, ...patch });
const selection = { instanceId: 'codex', model: 'gpt-6.1-sol' }, fallback = { instanceId: 'claude', model: 'claude-fable-5-1' };
const limited = provider({ usageLimits: { windows: [{ ...window, usedPercent: 100 }] } });
const available = provider({ instanceId: 'claude', driver: 'claudeAgent', displayName: 'Claude', models: [{ slug: 'claude-fable-5-1', name: 'Claude Fable 5.1' }] });
const defaults = { defaultSelection: selection, fallbackSelection: fallback, providers: [limited, available], now };

describe('resolveNewThreadModelSelection', () => {
  test.each([
    ['exhausted default', {}, fallback],
    ['explicit selection', { explicitSelection: selection }, selection],
    ['missing default', { defaultSelection: null }, null],
    ['no fallback', { fallbackSelection: null }, selection],
    ['expired window', { now: Date.parse('2026-09-04T00:00:00Z') }, selection],
    ['environment disabled', { settings: { providerInstances: { claude: { driver: 'claudeAgent', enabled: false, config: {} } }, projectSettingsOverrides: {} } }, selection],
    ['project disabled', { settings: { providerInstances: {}, projectSettingsOverrides: { p: { providerInstances: { claude: { enabled: false } } } } }, projectId: 'p' }, selection],
  ] as const)('resolves %s', (_name, overrides, expected) => {
    expect(resolveNewThreadModelSelection({ ...defaults, ...overrides } as never).selection).toEqual(expected);
  });
  test.each([{ enabled: false }, { installed: false }, { auth: { status: 'unauthenticated' } }, { availability: 'unavailable' }, { usageLimits: limited.usageLimits }])(
    'rejects an unusable fallback: %j', overrides => {
      expect(resolveNewThreadModelSelection({ ...defaults, providers: [limited, { ...available, ...overrides }] }).selection).toEqual(selection);
    });
  test.each([undefined, 'invalid'])('treats an unknown reset as exhausted: %s', resetsAt => {
    expect(exhaustedUsageWindow(provider({ usageLimits: { windows: [{ ...window, usedPercent: 100, resetsAt }] } }), now)).toBeDefined();
  });
  test.each([
    ['cursor', ['autoPercentUsed'], 'claude-opus-5-5', false], ['cursor', ['autoPercentUsed'], 'composer-2.5', false],
    ['cursor', ['autoPercentUsed', 'apiPercentUsed'], 'composer-2.5', true], ['antigravity', ['gemini-weekly'], 'claude-opus-5-5', false],
    ['antigravity', ['3p-weekly'], 'claude-opus-5-5', true], ['antigravity', ['gemini-weekly'], 'antigravity-default', true],
    ['opencode', ['go_weekly'], 'opencode/big-pickle', false], ['opencode', ['go_weekly'], 'opencode-go/big-pickle', true],
  ] as const)('matches %s pools %s to %s', (driver, ids, model, usesFallback) => {
    const defaultSelection = { ...selection, model };
    expect(resolveNewThreadModelSelection({ ...defaults, defaultSelection,
      providers: [provider({ driver, usageLimits: { windows: ids.map(id => ({ ...window, id, usedPercent: 100 })) } }), available] }).selection)
      .toEqual(usesFallback ? fallback : defaultSelection);
  });
});

describe('a new thread on the fallback model says so', () => {
  const client = (threadId = '') => ({ threadId, projectId: 'p', environmentId: 'e', providerId: '', modelId: '', busy: false, pending: null,
    config: { providers: [limited, available], settings: { fallbackModelSelection: fallback, providerInstances: { claude: { driver: 'claudeAgent', enabled: true } }, projectSettingsOverrides: {} } },
    shell: { threads: [], projects: [] }, local: { deviceSettings: { timestampFormat: '24-hour' } } }) as unknown as T3Client;
  test('the draft starts on the fallback and the notice names both models; picking the default or dismissing ends it', () => {
    const draft = client();
    const picked = startOnFallback(draft, selection, now)!;
    expect(picked).toEqual(fallback);
    Object.assign(draft, { providerId: picked.instanceId, modelId: picked.model });
    const [notice] = usageNotices(draft, now);
    expect(notice).toMatchObject({ id: 'fallback-model', variant: 'info', icon: 'gauge', title: 'Using Claude Fable 5.1', action: 'model',
      actionLabel: 'Use GPT-6.1 Sol', actionId: 'gpt-6.1-sol', actionValue: 'codex', dismiss: 'cclocal:fallback-dismiss', dismissLabel: 'Dismiss fallback model notice' });
    expect(notice!.description).toStartWith('Codex is out of usage · resets ');
    // Past the default's reset the draft keeps the fallback, but the notice goes (ChatView's nowMinute).
    expect(usageNotices(draft, Date.parse('2026-09-03T18:01:00Z'))).toEqual([]);
    Object.assign(draft, { providerId: 'codex', modelId: 'gpt-6.1-sol' });
    expect(usageNotices(draft, now)).toEqual([]);
  });
  test('a started thread keeps its model', () => {
    const thread = client('t1');
    expect(startOnFallback(thread, selection, now)).toEqual(selection);
    expect(draftFallbackOf(thread)).toBeUndefined();
  });
});
