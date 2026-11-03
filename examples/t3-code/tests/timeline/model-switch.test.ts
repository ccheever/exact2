// T3 Code 5d3d78ab93 packages/client-runtime/src/handoff.test.ts "model switch change" (MIT, see LICENSE-T3), as labels.
import { describe, expect, test } from 'bun:test';
import type { Obj } from '../../src/client/domain';
import { modelSwitchLabel } from '../../src/timeline/timeline-model-switch';
import { transcriptRows, timelineMessages } from '../../src/timeline/timeline-presentation';
import { opened } from '../support/composer-controls-fixture';

const instanceId = 'codex';
const selection = (model: string, options?: Record<string, string>) => ({
  instanceId, model, ...(options === undefined ? {} : { options: Object.entries(options).map(([id, value]) => ({ id, value })) }),
});
const providers: Obj[] = [{
  instanceId, driver: 'codex',
  models: [['gpt', 'GPT'], ['mini', 'Mini']].map(([slug, name]) => ({ slug, name, capabilities: { optionDescriptors: [
    { id: 'serviceTier', type: 'select', options: [{ id: 'fast', label: 'Fast' }] },
    { id: 'thought_level', type: 'select', role: 'reasoning', options: [{ id: 'medium', label: 'Medium', isDefault: true }, { id: 'high', label: 'High' }] },
  ] } })),
}];

describe('ModelSwitchRow (#28995)', () => {
  test('shows only what changed, reading an unset effort as the default', () => {
    expect(modelSwitchLabel({ from: selection('gpt'), to: selection('gpt', { thought_level: 'high' }) }, providers)).toBe('Medium → High');
    expect(modelSwitchLabel({ from: selection('gpt'), to: selection('mini') }, providers)).toBe('GPT → Mini');
    expect(modelSwitchLabel({ from: selection('gpt'), to: selection('mini', { thought_level: 'high' }) }, providers)).toBe('GPT Medium → Mini High');
  });
  test('ignores changes the user cannot see', () => {
    expect(modelSwitchLabel({ from: selection('gpt'), to: selection('gpt', { thought_level: 'medium' }) }, providers)).toBe('');
    expect(modelSwitchLabel({ from: selection('gpt'), to: selection('gpt', { serviceTier: 'fast' }) }, providers)).toBe('');
    // A cataloged model without an effort control ignores stale effort options.
    expect(modelSwitchLabel({ from: selection('plain', { reasoningEffort: 'low' }), to: selection('plain', { reasoningEffort: 'high' }) }, [{ instanceId, models: [{ slug: 'plain' }] }])).toBe('');
    expect(modelSwitchLabel(undefined, providers)).toBe('');
  });
  test('falls back to stated efforts without a catalog, and names an unknown model as sent', () => {
    expect(modelSwitchLabel({ from: selection('gpt', { reasoningEffort: 'low' }), to: selection('gpt', { reasoningEffort: 'high' }) }, [])).toBe('low → high');
    expect(modelSwitchLabel({ from: selection('pi', { thinking: 'low' }), to: selection('pi', { thinking: 'high' }) }, [])).toBe('low → high');
    expect(modelSwitchLabel({ from: selection('gpt'), to: selection('gpt', { reasoningEffort: 'high' }) }, [])).toBe('');
    expect(modelSwitchLabel({ from: selection('gpt-a'), to: selection('gpt-b') }, [])).toBe('gpt-a → gpt-b');
  });
});

test('the transcript places the switch before its user message and gives prose its direction', async () => {
  const { client } = await opened();
  client.config.providers = providers;
  const item = { id: 'm', type: 'user_message', text: 'שלום עולם `one two three four`', attachments: [],
    createdAt: '2026-11-03T00:00:00Z', modelSwitch: { from: selection('gpt'), to: selection('mini') } };
  client.projection.visibleTurnItems = [{ sourceThreadId: 't1', sourceItemId: 'm', position: 0, visibility: 'local', item }];
  const rows = transcriptRows(client);
  const index = rows.findIndex(row => row.kind === 'user');
  expect(index).toBeGreaterThan(0);
  expect(rows[index - 1]).toMatchObject({ kind: 'event', title: 'GPT → Mini', icon: '' });
  expect(rows[index]).toMatchObject({ body: item.text, dir: 'rtl' });
  expect(timelineMessages(client, rows, 0)[index]).toMatchObject({ dir: 'rtl' });
});
