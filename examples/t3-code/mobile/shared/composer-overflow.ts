// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/composer-overflow.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// The "More composer controls" menu's rows (task composer-fidelity, G11),
// adapted from T3 Code 1e2ecbd975 (MIT); see LICENSE-T3. Sources:
// apps/web/src/components/chat/CompactComposerControlsMenu.tsx and
// ChatComposer.tsx (hiddenRestingBlockIds: traits content when the traits block
// is hidden, the Mode radio when Plan UI is on and the mode block is hidden,
// then Access).
// Changes: one flat row list in the traits menu's shape; a mode row carries the
// descriptor "__mode" and an access row "__runtime", so the Contract menu routes
// each pick to the interaction, runtime or model-option command.
type Row = { key: string; kind: string; descriptor: string; value: string; label: string; description: string; selected: boolean; isDefault: boolean; index: number; disabled: boolean };

export function overflowMenu(input: { traits: Row[]; traitsHidden: boolean; modeHidden: boolean; planVisible: boolean; interactionMode: string;
  runtimes: Array<{ mode: string; label: string; selected: boolean }> }): { items: Row[]; count: number } {
  const items: Row[] = [];
  if (!input.modeHidden && !input.traitsHidden) return { items, count: 0 };
  let index = 0;
  const row = (fields: Partial<Row> & { key: string; kind: string }): Row => ({ descriptor: '', value: '', label: '', description: '', selected: false, isDefault: false, index: -1, disabled: false, ...fields });
  if (input.traitsHidden && input.traits.length) {
    for (const item of input.traits) items.push(item.kind === 'option' ? { ...item, key: `trait:${item.key}`, index: index++ } : { ...item, key: `trait:${item.key}` });
    items.push(row({ key: 'divider:traits', kind: 'divider' }));
  }
  if (input.planVisible && input.modeHidden) {
    items.push(row({ key: 'header:mode', kind: 'header', label: 'Mode' }));
    for (const [value, label] of [['default', 'Chat'], ['plan', 'Plan']] as const) {
      items.push(row({ key: `mode:${value}`, kind: 'option', descriptor: '__mode', value, label, selected: (input.interactionMode === 'plan') === (value === 'plan'), index: index++ }));
    }
    items.push(row({ key: 'divider:mode', kind: 'divider' }));
  }
  items.push(row({ key: 'header:access', kind: 'header', label: 'Access' }));
  for (const option of input.runtimes) items.push(row({ key: `runtime:${option.mode}`, kind: 'option', descriptor: '__runtime', value: option.mode, label: option.label, selected: option.selected, index: index++ }));
  return { items, count: index };
}
