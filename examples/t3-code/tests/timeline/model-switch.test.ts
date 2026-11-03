// T3 Code 5d3d78ab93 packages/client-runtime/src/handoff.test.ts "model switch change" (MIT, see LICENSE-T3), as labels.
import { describe, expect, test } from 'bun:test';
import type { Obj } from '../../src/client/domain';
import { modelSwitchLabel } from '../../src/timeline/timeline-model-switch';

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
