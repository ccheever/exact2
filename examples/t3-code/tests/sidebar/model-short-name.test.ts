// The row's and the subagent card's model label (T3 Code 7c3b3b3605ef, MIT; see LICENSE-T3: chat/providerIconUtils.ts
// getTriggerDisplayModelName, subagentDisplay.ts resolveSubagentMetadata). The spoken short name itself is
// client/model-short-name.ts's, tested in tests/client/provider-labels.test.ts.
import { expect, test } from 'bun:test';
import { triggerDisplayModelName } from '../../src/sidebar/model-short-name';
import { subagentMetadata } from '../../src/sidebar/subagent-card';

test('the label is the spoken name under the driver, a server short name first, a sub-provider prefix gone', () => {
  expect(triggerDisplayModelName({ name: 'Claude Opus 5.5' }, 'claudeAgent')).toBe('Opus 5.5');
  expect(triggerDisplayModelName({ name: 'Claude Sonnet 5', shortName: 'Sonnet' }, 'claudeAgent')).toBe('Sonnet');
  expect(triggerDisplayModelName({ name: 'OpenRouter: Claude Haiku 4.5', subProvider: 'OpenRouter' }, 'opencode')).toBe('Haiku 4.5');
  expect(triggerDisplayModelName({ name: 'GPT-5.5' }, 'codex')).toBe('GPT-5.5');
  // Off its native provider "3.1 Pro" names no model, so the family stays.
  expect(triggerDisplayModelName({ name: 'Gemini 3.1 Pro' }, 'opencode')).toBe('Gemini 3.1 Pro');
  expect(triggerDisplayModelName({ name: 'Gemini 3.1 Pro' }, 'antigravity')).toBe('3.1 Pro');
});

test('a subagent on a Claude model reads its spoken name', () => {
  const provider = { instanceId: 'claudeAgent', driver: 'claudeAgent', models: [{ slug: 'claude-opus-5-5', name: 'Claude Opus 5.5' }] };
  expect(subagentMetadata({ model: 'claude-opus-5-5', provider }).modelLabel).toBe('Opus 5.5');
});
