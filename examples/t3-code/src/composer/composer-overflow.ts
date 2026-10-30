// The "More composer controls" menu's rows,
// adapted from T3 Code v0.1.5 (MIT); see LICENSE-T3. Sources:
// apps/web/src/components/chat/CompactComposerControlsMenu.tsx and
// ComposerControlsBar.tsx (hiddenRestingBlockIds: traits content when the traits block
// is hidden, the Mode radio when Plan UI is on and the mode block is hidden,
// then Permissions while their home is in the menu, resolveComposerControlsOwnership).
// Changes: one flat row list in the traits menu's shape; a mode row carries the
// descriptor "__mode", so the Contract menu routes each pick to the interaction or
// model-option command. The Permissions section is composer-controls.contract's, after
// these rows; `count` counts only these rows.
type Row = { key: string; kind: string; descriptor: string; value: string; label: string; description: string; selected: boolean; isDefault: boolean; index: number; disabled: boolean };

export function overflowMenu(input: { traits: Row[]; traitsHidden: boolean; modeHidden: boolean; planVisible: boolean; interactionMode: string; showPermissions: boolean }): { items: Row[]; count: number } {
  const items: Row[] = [];
  if (!input.modeHidden && !input.traitsHidden) return { items, count: 0 };
  let index = 0;
  const row = (fields: Partial<Row> & { key: string; kind: string }): Row => ({ descriptor: '', value: '', label: '', description: '', selected: false, isDefault: false, index: -1, disabled: false, ...fields });
  const mode = input.planVisible && input.modeHidden;
  if (input.traitsHidden && input.traits.length) {
    for (const item of input.traits) items.push(item.kind === 'option' ? { ...item, key: `trait:${item.key}`, index: index++ } : { ...item, key: `trait:${item.key}` });
    if (mode || input.showPermissions) items.push(row({ key: 'divider:traits', kind: 'divider' }));
  }
  if (mode) {
    items.push(row({ key: 'header:mode', kind: 'header', label: 'Mode' }));
    for (const [value, label] of [['default', 'Chat'], ['plan', 'Plan']] as const) {
      items.push(row({ key: `mode:${value}`, kind: 'option', descriptor: '__mode', value, label, selected: (input.interactionMode === 'plan') === (value === 'plan'), index: index++ }));
    }
    if (input.showPermissions) items.push(row({ key: 'divider:mode', kind: 'divider' }));
  }
  return { items, count: index };
}
