// The model picker's effort column and footer. Adapted from T3 Code 2359643cff (MIT; see LICENSE-T3):
// chat/ComposerModelEffort.tsx (splitSpeed, columnControls, EffortLevels, ComposerEffortColumn, SpeedControl),
// with client-runtime modelTraits.ts selectableProviderOptionChoices. The column lists the model's reasoning
// control first, under the header that names it, then its other options; speed sits in the footer.
import { arr, str, type Obj } from '../client/domain';
import { optionLabel, optionValue, REASONING_SELECT_DESCRIPTOR_IDS, resolvedCurrent, type Selection } from './composer-controls-model';
import { ULTRATHINK_LOCKED_MESSAGE } from './composer-provider-state';

/** An option row of the column: a `level` of the reasoning control, or another option's. */
export type EffortRow = { key: string; descriptor: string; value: string; label: string; description: string; selected: boolean;
  isDefault: boolean; disabled: boolean; level: boolean; index: number };
/**
 * A block of the column: a control (a radio group named by its label, which shows except over the reasoning control
 * the header already names; `note` while "ultrathink" in the prompt locks it), an unavailable model's option and its
 * value, or "No effort levels" (`none`).
 */
export type EffortGroup = { key: string; kind: 'control' | 'value' | 'none'; label: string; showLabel: boolean; note: string; value: string; options: EffortRow[] };
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

export type EffortInput = { driver: string; descriptors: Obj[]; selections: Obj[]; selection: Selection | null; reported: Selection | null;
  ultra: { primaryId: string; controlled: boolean; inBody: boolean }; unavailable: boolean; labels: Record<string, string> };

/** traits.selectValue: the checked option, "ultrathink" while the prompt controls the primary effort. */
function selectValue(input: EffortInput, descriptor: Obj): string {
  if (str(descriptor.id) === input.ultra.primaryId && input.ultra.controlled) return 'ultrathink';
  return str(optionValue(descriptor, resolvedCurrent(descriptor, input.selections), input.selection, input.reported));
}
const booleanValue = (input: EffortInput, descriptor: Obj) => resolvedCurrent(descriptor, input.selections) === true;
const labelOf = (input: EffortInput, descriptor: Obj) => str(descriptor.label, input.labels[str(descriptor.id)] ?? str(descriptor.id));

/** ComposerEffortColumn: the header's label (the reasoning control's, else "Effort") and the blocks under it. */
export function effortColumn(input: EffortInput): { header: string; groups: EffortGroup[]; rows: EffortRow[] } {
  const groups: EffortGroup[] = [];
  const group = (fields: Partial<EffortGroup> & { key: string; kind: EffortGroup['kind'] }) =>
    groups.push({ label: '', showLabel: false, note: '', value: '', options: [], ...fields });
  const { columnSelects, columnToggles } = splitSpeed(input.driver, input.descriptors);
  const controls = [...columnSelects, ...columnToggles];
  const effort = controls.find(descriptor => REASONING_SELECT_DESCRIPTOR_IDS.has(str(descriptor.id))) ?? null;
  const header = effort ? labelOf(input, effort) : 'Effort';
  if (input.unavailable && input.descriptors.length) {
    // An unavailable model's options are read-only: each one's name and current value.
    for (const descriptor of input.descriptors) {
      const value = optionLabel(descriptor, resolvedCurrent(descriptor, input.selections), input.selection, input.reported);
      if (value) group({ key: `value:${str(descriptor.id)}`, kind: 'value', label: labelOf(input, descriptor), showLabel: true, value });
    }
    return { header, groups, rows: [] };
  }
  if (!controls.length) {
    group({ key: 'none', kind: 'none', label: 'No effort levels' });
    return { header, groups, rows: [] };
  }
  let index = 0;
  for (const descriptor of effort ? [effort, ...controls.filter(control => control !== effort)] : controls) {
    const id = str(descriptor.id), level = descriptor === effort, options: EffortRow[] = [];
    const option = (fields: Partial<EffortRow> & { key: string; value: string; label: string }) =>
      options.push({ descriptor: id, description: '', selected: false, isDefault: false, disabled: false, level, index: index++, ...fields });
    let note = '';
    if (descriptor.type === 'boolean') {
      const on = booleanValue(input, descriptor);
      for (const value of [true, false]) option({ key: `${id}:${value}`, value: String(value), label: value ? 'On' : 'Off', selected: on === value });
    } else {
      const locked = id === input.ultra.primaryId && input.ultra.inBody, current = selectValue(input, descriptor);
      if (locked) note = ULTRATHINK_LOCKED_MESSAGE;
      for (const choice of selectableChoices(descriptor)) {
        option({ key: `${id}:${str(choice.id)}`, value: str(choice.id), label: str(choice.label, str(choice.id)), description: str(choice.description),
          selected: choice.id === current, isDefault: choice.isDefault === true, disabled: locked });
      }
    }
    group({ key: `control:${id}`, kind: 'control', label: labelOf(input, descriptor), showLabel: !level, note, options });
  }
  return { header, groups, rows: groups.flatMap(entry => entry.options) };
}

/**
 * The column's natural height, in points at the root size 16: the header (pt-2.5, 33), the list's padding (pt-2.5,
 * pb-1.5), 32pt rows, 26pt labels, a note's three 16pt lines and a value's line (each pb-1.5), all 2pt apart.
 */
export function effortColumnHeight(groups: EffortGroup[]): number {
  const lines = groups.flatMap(entry => [...(entry.showLabel ? [26] : []), ...(entry.note ? [54] : []), ...(entry.kind === 'value' ? [22] : []),
    ...(entry.kind === 'none' ? [32] : []), ...entry.options.map(() => 32)]);
  return 43 + 10 + 6 + lines.reduce((sum, height) => sum + height, 0) + 2 * Math.max(0, lines.length - 1);
}

/**
 * ProviderModelPicker focusEffort: the row a model pick (`levels`) or composer.effort (`any`) moves the focus to — the
 * checked level, else the first level, and for `any` then the column's other rows likewise; -1 for none.
 */
export function effortFocus(rows: EffortRow[], scope: 'levels' | 'any'): number {
  const first = (match: (row: EffortRow) => boolean) => (rows.find(row => match(row) && row.selected) ?? rows.find(match))?.index ?? -1;
  const level = first(row => row.level && !row.disabled);
  return level >= 0 || scope === 'levels' ? level : first(row => !row.disabled);
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
