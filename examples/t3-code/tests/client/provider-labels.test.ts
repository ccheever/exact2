// Provider and model labels (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3): account badges read punctuation as a word
// break (client-runtime state/providerInstanceDisplay.ts providerInstanceInitials), and a model's trigger label is the
// name you would say out loud on its provider (provider-core modelShortName.ts, client-runtime providerClients.ts).
import { describe, expect, test } from 'bun:test';
import { providerBadge, providerInstanceInitials, snapshot } from '../../src/client/presentation';
import { modelShortName, resolveModelShortName } from '../../src/client/model-short-name';
import { arr, obj } from '../../src/client/domain';
import { opened } from '../support/client-fixture';
import { appText } from '../support/app-files';
import { contractComponent, value } from '../support/contract-action-fixture';
import type { T3Client } from '../../src/client/client';

/** The Contract expression `attribute` takes (a balanced `(…)`) on composer-controls.contract's line holding `marker`. */
function expression(marker: string, attribute: string): string {
  const line = appText('src/composer/composer-controls.contract').split('\n').find(text => text.includes(marker))!;
  const open = line.indexOf(attribute) + attribute.length;
  for (let at = open, depth = 0; at < line.length; at++) {
    if (line[at] === '"') { at = line.indexOf('"', at + 1); continue; }
    if (line[at] === '(') depth++;
    else if (line[at] === ')' && --depth === 0) return line.slice(open, at + 1);
  }
  throw new Error(`no ${attribute} on the line with ${marker}`);
}
/** What ModelTrigger shows for the client's snapshot: its text, its accessible name and whether the badge is drawn. */
function trigger(client: T3Client) {
  const data = snapshot(client), component = contractComponent('src/composer/composer-controls.contract', 'ModelTrigger');
  const scope = { data, xs: false, open: false, label: true, keyModels: '', disabled: false, press: () => {} };
  return { text: value(expression('testId="model-picker-label"', 'text '), scope), name: value(expression('testId=`${composerScope}model-picker` ', 'aria-label='), scope),
    badge: component.drawn('testId="model-picker-unavailable"', component.state(), scope) };
}

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

  // ProviderModelPicker.tsx: the trigger's text is triggerTitle, its aria-label triggerLabel, and an unavailable model
  // gets the Unavailable badge after the text.
  test('the trigger shows the short name, is named by the full one, and badges an unavailable model', async () => {
    const { client } = await opened();
    const provider = arr(client.config.providers)[0]!;
    Object.assign(provider, { driver: 'claudeAgent' });
    const model = obj(arr(provider.models)[0]);
    model.name = 'Claude Opus 5.5';
    expect(trigger(client)).toEqual({ text: 'Opus 5.5', name: 'Claude Opus 5.5', badge: false });
    model.isUnavailable = true;
    expect(trigger(client)).toEqual({ text: 'Opus 5.5', name: 'Claude Opus 5.5 (Unavailable)', badge: true });
    // The catalog's own shortName wins; a name with no short form on its driver shows in full.
    Object.assign(model, { isUnavailable: false, shortName: 'Opus' });
    expect(trigger(client)).toEqual({ text: 'Opus', name: 'Claude Opus 5.5', badge: false });
    Object.assign(provider, { driver: 'codex' });
    Object.assign(model, { name: 'Grok 4 Pro', shortName: undefined });
    expect(trigger(client)).toEqual({ text: 'Grok 4 Pro', name: 'Grok 4 Pro', badge: false });
  });
});
