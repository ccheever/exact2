// Settings › Appearance › Effort display and Threads › Fallback model, from the settings write to what the composer
// reads: the effort as bars, and a new draft that starts on the fallback while its default's provider is out of usage
// (T3 Code v0.1.5 #25869, #26164).
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { composerView } from '../../src/composer/composer-presentation';
import { draftFallbackOf, startOnFallback } from '../../src/composer/composer-provider-selection';
import { applyCoreSetting, decodeClientPrefs, DEVICE_DEFAULTS } from '../../src/settings/settings-core';

const now = Date.parse('2026-10-31T12:00:00Z');
const effort = { id: 'reasoningEffort', label: 'Reasoning', type: 'select', options: [{ id: 'low', label: 'Low' }, { id: 'medium', label: 'Medium', isDefault: true }, { id: 'high', label: 'High' }] };
const provider = (instanceId: string, driver: string, slug: string, name: string, usedPercent: number) => ({ instanceId, driver, displayName: instanceId, enabled: true, installed: true,
  status: 'ready', availability: 'available', auth: { status: 'authenticated' }, models: [{ slug, name, capabilities: { optionDescriptors: [effort] } }],
  usageLimits: { windows: [{ id: 'weekly', usedPercent, resetsAt: '2026-11-01T00:00:00.000Z' }] } });

function fake() {
  const client = {
    local: { deviceSettings: { ...DEVICE_DEFAULTS }, clientSettings: decodeClientPrefs({}), groupingMode: 'repository', favoriteModels: [] as string[] },
    ready: true, environmentId: 'env1', threadId: 't1', projectId: 'p1', providerId: 'codex', modelId: 'gpt-6', modelOptions: [], runtimeMode: 'full-access',
    interactionMode: 'default', connection: 'connected', projection: {}, presentation: {}, shell: { threads: [], projects: [] }, writes: [] as Obj[],
    config: { environment: { environmentId: 'env1', label: 'Studio', serverVersion: '0.1.5', capabilities: { projectSettingsOverrides: true } },
      providers: [provider('codex', 'codex', 'gpt-6', 'GPT-6', 100), provider('claudeAgent', 'claudeAgent', 'claude-opus-5-5', 'Claude Opus 5.5', 10)],
      settings: { projectSettingsOverrides: {} } as Obj },
    projectGroups: () => [],
    async settingsCoreRequest(_native: Native, method: string, payload: Obj) {
      if (method === 'server.getConfig') return client.config as Obj;
      client.writes.push(payload);
      client.config.settings = { ...client.config.settings, ...(payload.patch as Obj) }; // server.updateSettings answers the new settings
      return client.config.settings;
    },
  };
  return client;
}
const native = { available: true, watch() {}, later: async () => ({ ok: true, generation: 0, value: { fonts: [] } }) } as unknown as Native;
const as = (client: ReturnType<typeof fake>) => client as unknown as T3Client;

describe('the composer reads what Settings writes', () => {
  test('Effort display: Word shows the level, Bars draws it, and Reset goes back to the word', async () => {
    const client = fake();
    const view = () => composerView(as(client), { approval: false, question: false, choiceOnly: false });
    expect(view()).toMatchObject({ effortWord: 'Medium', effortBars: [] });
    await applyCoreSetting(as(client), native, 'effort-display:effortDisplay|||', 'bars');
    expect((client.local.clientSettings as Obj).effortDisplay).toBe('bars');
    expect(view().effortWord).toBe('');
    expect(view().effortBars.map(bar => bar.on)).toEqual([true, true, false]);
    await applyCoreSetting(as(client), native, 'effort-display:reset|||', 'effortDisplay');
    expect(view()).toMatchObject({ effortWord: 'Medium', effortBars: [] });
  });

  test('Fallback model: once saved, a new draft whose default is out of usage starts on it; reset to None, it keeps the default', async () => {
    const client = fake(), pick = { instanceId: 'codex', model: 'gpt-6' };
    const draft = () => Object.assign(as(client), { threadId: '' });
    expect(startOnFallback(draft(), pick, now)).toEqual(pick);
    await applyCoreSetting(as(client), native, 'fallback-model:|||', 'claudeAgent|claude-opus-5-5');
    expect(client.writes.at(-1)).toEqual({ patch: { fallbackModelSelection: { instanceId: 'claudeAgent', model: 'claude-opus-5-5' } } });
    expect(startOnFallback(draft(), pick, now)).toEqual({ instanceId: 'claudeAgent', model: 'claude-opus-5-5' });
    expect(draftFallbackOf(client)).toMatchObject({ defaultSelection: pick });
    await applyCoreSetting(as(client), native, 'fallback-model:reset|||', '');
    expect(client.config.settings.fallbackModelSelection).toBeNull();
    expect(startOnFallback(draft(), pick, now)).toEqual(pick);
  });
});
