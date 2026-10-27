// Provider and model labels (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3): account badges read punctuation as a word
// break (client-runtime state/providerInstanceDisplay.ts providerInstanceInitials), and a model's trigger label is the
// name you would say out loud on its provider (provider-core modelShortName.ts, client-runtime providerClients.ts).
import { describe, expect, test } from 'bun:test';
import { providerBadge, providerInstanceInitials, snapshot } from '../../src/client/presentation';
import { modelShortName, resolveModelShortName } from '../../src/client/model-short-name';
import { arr, obj } from '../../src/client/domain';
import { opened } from '../support/client-fixture';

describe('account badges', () => {
  test('"Codex (hub)" reads CH and "Theo\'s Codex" TC', () => {
    expect(['Codex (hub)', 'work_codex', "Theo's Codex", "Codex 'hub'", '‘hub’', '(hub)', 'Codex'].map(providerInstanceInitials))
      .toEqual(['CH', 'WC', 'TC', 'CH', 'HU', 'HU', 'CO']);
    const hub = { instanceId: 'codex_hub', driver: 'codex', displayName: 'Codex (hub)', enabled: true };
    expect(providerBadge(hub, [hub, { instanceId: 'codex', driver: 'codex', enabled: true }])).toEqual({ providerBadge: 'CH', providerBadgeColor: '' });
  });
});

describe('the spoken model name', () => {
  test.each([
    ['Claude Opus 5.5', ['Claude '], 'Opus 5.5'], ['GPT-6.1-Sol', ['GPT-'], '6.1 Sol'], ['GPT 6.0 Astra', ['GPT-', 'GPT '], '6.0 Astra'],
    ['Grok 4 Pro', [], undefined], ['Claude Opus 5.5', [], 'Opus 5.5'], ['NVIDIA: Nemotron 3 Super', [], 'Nemotron 3 Super'],
    ['GPT-5.5', ['GPT-'], undefined], ['Gemini 3.1 Pro', ['Gemini '], '3.1 Pro'],
  ] as const)('%s on families %j says %s', (name, families, expected) => {
    expect(modelShortName(name, families)).toBe(expected);
  });

  test('the server\'s shortName wins, else the driver\'s families decide', () => {
    expect(resolveModelShortName({ name: 'Claude Opus 5.5', shortName: 'Opus' }, 'claudeAgent')).toBe('Opus');
    expect([resolveModelShortName({ name: 'Grok 4 Pro' }, 'cursor'), resolveModelShortName({ name: 'Grok 4 Pro' }, 'codex')]).toEqual(['4 Pro', undefined]);
  });

  test('the composer\'s trigger label is the short name on its driver', async () => {
    const { client } = await opened();
    const provider = arr(client.config.providers)[0]!;
    Object.assign(provider, { driver: 'claudeAgent' });
    obj(arr(provider.models)[0]).name = 'Claude Opus 5.5';
    expect(snapshot(client).modelLabel).toBe('Opus 5.5');
  });
});
