// The model picker's effort column and footer. Adapted from T3 Code 5d3d78ab93 (MIT; see LICENSE-T3):
// chat/ComposerModelEffort.tsx (splitSpeed, columnControls, EffortLevels, EffortRows, OptionMenuRow, OptionSwitchRow,
// ComposerEffortColumn, SpeedControl), with client-runtime modelTraits.ts selectableProviderOptionChoices and
// isReasoningControl. The column lists the model's reasoning levels under the header that names it; its other options,
// such as an agent's session mode, sit below as one row each. Speed sits in the footer.
import { isReasoningControl } from '../client/server-fields';
import { arr, str, type Obj } from '../client/domain';
import { optionLabel, optionValue, resolvedCurrent, type Selection } from './composer-controls-model';
import { ULTRATHINK_LOCKED_MESSAGE } from './composer-provider-state';
import { textWidth } from './composer-controls-view';

/**
 * A focusable row of the column (`index` its focus order): a level of the reasoning control (`level`), or, in an option
 * group's `options`, a choice its side menu offers (`index` its place there). `tabStop` is the column's one tab stop.
 */
export type EffortRow = { key: string; descriptor: string; value: string; label: string; description: string; selected: boolean;
  isDefault: boolean; disabled: boolean; level: boolean; tabStop: boolean; index: number };
/**
 * A block of the column: the reasoning control (`control`, a radio group named by its label; `note` while "ultrathink"
 * in the prompt locks it), another option as one row (`option`: a select naming its choice and opening its `options`
 * beside the column in a menu `menuWidth` by `menuHeight`, or a switch when `toggle`, `on` its state; `divider` on the
 * first one under the levels), an unavailable model's option and its value (`value`), or "No effort levels" (`none`).
 */
export type EffortGroup = { key: string; kind: 'control' | 'option' | 'value' | 'none'; descriptor: string; label: string; showLabel: boolean; note: string;
  value: string; options: EffortRow[]; toggle: boolean; on: boolean; divider: boolean; index: number; tabStop: boolean; menuWidth: number; menuHeight: number };
export type SpeedSegment = { key: string; descriptor: string; value: string; label: string; description: string; checked: boolean; bolt: string; first: boolean; last: boolean };

/** selectableProviderOptionChoices: prompt-injected levels such as Ultrathink still work when typed, but pickers do not offer them. */
export function selectableChoices(descriptor: Obj): Obj[] {
  const injected = Array.isArray(descriptor.promptInjectedValues) ? descriptor.promptInjectedValues as unknown[] : [];
  return arr(descriptor.options).filter(option => !injected.includes(option.id));
}

/** splitSpeed: Codex service tiers and Claude's Fast mode share the footer's speed control. */
function splitSpeed(driver: string, descriptors: Obj[]) {
  const selects = descriptors.filter(descriptor => descriptor.type === 'select'), toggles = descriptors.filter(descriptor => descriptor.type === 'boolean');
  const speedSelect = driver === 'codex' ? selects.find(descriptor => descriptor.id === 'serviceTier') ?? null : null;
  const fastToggle = toggles.find(descriptor => descriptor.id === 'fastMode') ?? null;
  return { speedSelect, fastToggle, columnSelects: selects.filter(descriptor => descriptor !== speedSelect), columnToggles: toggles.filter(descriptor => descriptor !== fastToggle) };
}

/** columnControls: the reasoning control, wherever the agent put it among its options, and the others in order. */
export function columnControls(driver: string, descriptors: Obj[]): { effort: Obj | null; others: Obj[] } {
  const { columnSelects, columnToggles } = splitSpeed(driver, descriptors);
  const controls = [...columnSelects, ...columnToggles];
  const effort = controls.find(isReasoningControl) ?? null;
  return { effort, others: controls.filter(control => control !== effort) };
}

export type EffortInput = { driver: string; descriptors: Obj[]; selections: Obj[]; selection: Selection | null; reported: Selection | null;
  ultra: { primaryId: string; controlled: boolean; inBody: boolean }; unavailable: boolean; labels: Record<string, string> };

/** traits.selectValue: the checked option, "ultrathink" while the prompt controls the primary effort. */
function selectValue(input: EffortInput, descriptor: Obj): string {
  if (str(descriptor.id) === input.ultra.primaryId && input.ultra.controlled) return 'ultrathink';
  return str(optionValue(descriptor, resolvedCurrent(descriptor, input.selections), input.selection, input.reported));
}
const booleanValue = (input: EffortInput, descriptor: Obj) => resolvedCurrent(descriptor, input.selections) === true;
const labelOf = (input: EffortInput, descriptor: Obj) => str(descriptor.label, input.labels[str(descriptor.id)] ?? str(descriptor.id));

/**
 * An option's side menu (MenuPopup min-w-40 max-w-72, p-1 inside its border): as wide as its widest choice, a label
 * (14pt) or a description (12pt), with "Default" and the check after it, inside px-2 and gap-3; each choice is at least
 * 28pt (min-h-7, py-1), a description adding up to two 16pt lines.
 */
function optionMenuSize(choices: EffortRow[]): { width: number; height: number } {
  const extra = (choice: EffortRow) => (choice.isDefault ? 12 + textWidth('Default', true) : 0) + (choice.selected ? 12 + 14 : 0) + 16 + 8 + 2;
  const width = Math.min(288, Math.max(160, Math.ceil(Math.max(0, ...choices.map(choice => Math.max(textWidth(choice.label), textWidth(choice.description, true)) + extra(choice))))));
  const lines = (choice: EffortRow) => choice.description ? Math.min(2, Math.ceil(textWidth(choice.description, true) / Math.max(1, width - extra(choice)))) : 0;
  return { width, height: 10 + choices.reduce((sum, choice) => sum + Math.max(28, 28 + 16 * lines(choice)), 0) };
}

/** ComposerEffortColumn: the header's label (the reasoning control's, "Options" with only other options, else "Effort") and its blocks. */
export function effortColumn(input: EffortInput): { header: string; groups: EffortGroup[]; rows: EffortRow[]; order: number[] } {
  const groups: EffortGroup[] = [];
  const group = (fields: Partial<EffortGroup> & { key: string; kind: EffortGroup['kind'] }) =>
    groups.push({ descriptor: '', label: '', showLabel: false, note: '', value: '', options: [], toggle: false, on: false, divider: false, index: -1, tabStop: false,
      menuWidth: 0, menuHeight: 0, ...fields });
  const { effort, others } = columnControls(input.driver, input.descriptors);
  const header = effort ? labelOf(input, effort) : others.length ? 'Options' : 'Effort';
  if (input.unavailable && input.descriptors.length) {
    // An unavailable model's options are read-only: each one's name and current value.
    for (const descriptor of input.descriptors) {
      const value = optionLabel(descriptor, resolvedCurrent(descriptor, input.selections), input.selection, input.reported);
      if (value) group({ key: `value:${str(descriptor.id)}`, kind: 'value', label: labelOf(input, descriptor), showLabel: true, value });
    }
    return { header, groups, rows: [], order: [] };
  }
  if (!effort && !others.length) {
    group({ key: 'none', kind: 'none', label: 'No effort levels' });
    return { header, groups, rows: [], order: [] };
  }
  let index = 0;
  const rows: EffortRow[] = [], order: number[] = [];
  // Ultrathink in the prompt locks the levels; the first option then holds the tab stop.
  const locked = !!effort && str(effort.id) === input.ultra.primaryId && input.ultra.inBody;
  if (effort) {
    const id = str(effort.id), options: EffortRow[] = [];
    const row = (fields: Partial<EffortRow> & { key: string; value: string; label: string }) =>
      options.push({ descriptor: id, description: '', selected: false, isDefault: false, disabled: false, level: true, tabStop: false, index: index++, ...fields });
    if (effort.type === 'boolean') {
      const on = booleanValue(input, effort);
      for (const value of [true, false]) row({ key: `${id}:${value}`, value: String(value), label: value ? 'On' : 'Off', selected: on === value, tabStop: on === value });
    } else {
      const current = selectValue(input, effort), choices = selectableChoices(effort);
      // An "Ultrathink:" prefix selects a level the column does not offer, so the first level holds the tab stop.
      const shown = choices.some(choice => choice.id === current);
      choices.forEach((choice, at) => row({ key: `${id}:${str(choice.id)}`, value: str(choice.id), label: str(choice.label, str(choice.id)),
        description: str(choice.description), selected: choice.id === current, isDefault: choice.isDefault === true, disabled: locked,
        tabStop: shown ? choice.id === current : at === 0 }));
    }
    group({ key: `control:${id}`, kind: 'control', descriptor: id, label: labelOf(input, effort), note: locked ? ULTRATHINK_LOCKED_MESSAGE : '', options });
    rows.push(...options);
    order.push(...options.filter(option => !option.disabled).map(option => option.index));
  }
  others.forEach((descriptor, at) => {
    const id = str(descriptor.id), tabStop = (!effort || locked) && at === 0, own = index++;
    order.push(own);
    if (descriptor.type === 'boolean') {
      const on = booleanValue(input, descriptor);
      group({ key: `option:${id}`, kind: 'option', descriptor: id, label: labelOf(input, descriptor), value: on ? 'On' : 'Off', toggle: true, on, divider: !!effort && at === 0, index: own, tabStop });
      return;
    }
    // A choice's `index` is its place in the menu.
    const current = selectValue(input, descriptor), choices = selectableChoices(descriptor).map((choice, place): EffortRow => ({ key: `${id}:${str(choice.id)}`,
      descriptor: id, value: str(choice.id), label: str(choice.label, str(choice.id)), description: str(choice.description), selected: choice.id === current,
      isDefault: choice.isDefault === true, disabled: false, level: false, tabStop: false, index: place }));
    const menu = optionMenuSize(choices);
    group({ key: `option:${id}`, kind: 'option', descriptor: id, label: labelOf(input, descriptor), value: choices.find(choice => choice.selected)?.label ?? '',
      options: choices, divider: !!effort && at === 0, index: own, tabStop, menuWidth: menu.width, menuHeight: menu.height });
  });
  return { header, groups, rows, order };
}

/**
 * ProviderModelPicker focusEffort: the row a model pick (`levels`) or composer.effort (`any`) moves the focus to — the
 * checked level, else the first level, and for `any` then the column's first enabled row (an option's); -1 for none.
 */
export function effortFocus(column: { rows: EffortRow[]; order: number[] }, scope: 'levels' | 'any'): number {
  const enabled = column.rows.filter(row => row.level && !row.disabled);
  const level = (enabled.find(row => row.selected) ?? enabled[0])?.index ?? -1;
  return level >= 0 || scope === 'levels' ? level : column.order[0] ?? -1;
}

/** SpeedControl: the Codex service tiers, or Standard and Fast for a fast-mode toggle; none for an unavailable model. */
export function speedSegments(input: EffortInput): { label: string; segments: SpeedSegment[] } {
  const { speedSelect, fastToggle } = splitSpeed(input.driver, input.descriptors);
  if (input.unavailable || (!speedSelect && !fastToggle)) return { label: '', segments: [] };
  const ends = (segments: Array<Omit<SpeedSegment, 'first' | 'last'>>): SpeedSegment[] =>
    segments.map((segment, index) => ({ ...segment, first: index === 0, last: index === segments.length - 1 }));
  const bolt = (label: string) => label === 'Fast' ? 'fast' : label === 'Ultrafast' ? 'ultra' : '';
  if (speedSelect) {
    const current = selectValue(input, speedSelect), id = str(speedSelect.id);
    return { label: labelOf(input, speedSelect), segments: ends(selectableChoices(speedSelect).map(option => {
      const label = str(option.label, str(option.id));
      return { key: `${id}:${str(option.id)}`, descriptor: id, value: str(option.id), label, description: str(option.description), checked: option.id === current, bolt: bolt(label) };
    })) };
  }
  const id = str(fastToggle!.id), on = booleanValue(input, fastToggle!);
  return { label: labelOf(input, fastToggle!), segments: ends([false, true].map(value => ({ key: `${id}:${value}`, descriptor: id, value: String(value),
    label: value ? 'Fast' : 'Standard', description: '', checked: on === value, bolt: value ? 'fast' : '' }))) };
}
