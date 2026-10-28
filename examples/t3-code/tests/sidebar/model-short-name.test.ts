// T3 Code 7c3b3b3605ef (MIT; see LICENSE-T3) packages/provider-core/src/modelShortName.test.ts, and the subagent card's
// label (subagentDisplay.ts resolveSubagentMetadata) under the driver's families.
import { expect, test } from 'bun:test';
import { modelShortName, resolveModelShortName, triggerDisplayModelName } from '../../src/sidebar/model-short-name';
import { subagentMetadata } from '../../src/sidebar/subagent-card';

test.each([
  ['Claude Opus 5.5', ['Claude '], 'Opus 5.5'],
  ['GPT-6.1-Sol', ['GPT-'], '6.1 Sol'],
  ['GPT 6.0 Astra', ['GPT-', 'GPT '], '6.0 Astra'],
  ['Grok 4 Pro', [], undefined],
  ['Claude Opus 5.5', [], 'Opus 5.5'],
  ['NVIDIA: Nemotron 3 Super', [], 'Nemotron 3 Super'],
  ['GPT-5.5', ['GPT-'], undefined],
  ['Gemini 3.1 Pro', ['Gemini '], '3.1 Pro'],
] as const)('%s on families %j is said as %s', (name, families, expected) => {
  expect(modelShortName(name, families)).toBe(expected);
});

test('the driver picks the families; a server short name wins; a sub-provider qualifier goes', () => {
  expect(resolveModelShortName({ name: 'GPT-5.5' }, 'codex')).toBeUndefined();
  expect(resolveModelShortName({ name: 'Gemini 3.1 Pro' }, 'antigravity')).toBe('3.1 Pro');
  // Off its native provider "3.1 Pro" names no model, so the family stays.
  expect(resolveModelShortName({ name: 'Gemini 3.1 Pro' }, 'opencode')).toBeUndefined();
  expect(triggerDisplayModelName({ name: 'Claude Sonnet 5', shortName: 'Sonnet' }, 'claudeAgent')).toBe('Sonnet');
  expect(triggerDisplayModelName({ name: 'OpenRouter: Claude Haiku 4.5', subProvider: 'OpenRouter' }, 'opencode')).toBe('Haiku 4.5');
  expect(triggerDisplayModelName({ name: 'GPT-5.5' }, 'codex')).toBe('GPT-5.5');
});

test('a subagent on a Claude model reads its spoken name', () => {
  const provider = { instanceId: 'claudeAgent', driver: 'claudeAgent', models: [{ slug: 'claude-opus-5-5', name: 'Claude Opus 5.5' }] };
  expect(subagentMetadata({ model: 'claude-opus-5-5', provider }).modelLabel).toBe('Opus 5.5');
});
