// A user message that switched the model or effort on the same harness (T3 Code 5d3d78ab93, MIT; see LICENSE-T3):
// apps/web/src/components/chat/V2LifecycleRow.tsx ModelSwitchRow, TimelineUserMessageRow.tsx UserMessageRow, and
// packages/client-runtime/src/handoff.ts resolveModelSwitchChange, over shared/model.ts getModelSelectionStringOptionValue
// and getProviderOptionCurrentValue and modelTraits.ts isReasoningSelect. The server records the switch on the message
// (`modelSwitch`, {from, to}); the divider above it names only what changed.
import { arr, obj, str, type Obj } from '../client/domain';
import { triggerModelName } from '../composer/composer-controls-model';
import { isReasoningControl, REASONING_EFFORT_OPTION_IDS } from '../client/server-fields';

const statedValue = (selection: Obj, id: string): string | undefined => {
  const value = arr(selection.options).find(option => option.id === id)?.value;
  return typeof value === 'string' ? value : undefined;
};
/** isReasoningSelect: a select that is a known effort id, or one its agent marked as reasoning. */
const isReasoningSelect = (descriptor: Obj) => descriptor.type === 'select' && isReasoningControl(descriptor);

/** getProviderOptionCurrentValue without a reported selection: what the descriptor resolves to for this selection. */
function currentValue(descriptor: Obj, selection: Obj): unknown {
  if (descriptor.id === 'variant' && !arr(selection.options).some(option => option.id === descriptor.id)) return undefined;
  if (descriptor.type === 'boolean') return descriptor.currentValue;
  if (descriptor.currentValue) return descriptor.currentValue;
  return arr(descriptor.options).find(option => option.isDefault === true)?.id;
}

/** The composer's reasoning control for the model; stated values only when the catalog lacks it. */
function resolveEffort(selection: Obj, models: Obj[] | undefined): { value: string; label: string } | undefined {
  const model = models?.find(candidate => candidate.slug === selection.model);
  if (model === undefined) {
    // Pi states its effort as `thinking`; the catalog would otherwise mark it a reasoning control.
    const value = [...REASONING_EFFORT_OPTION_IDS, 'thinking'].map(id => statedValue(selection, id)).find(candidate => candidate !== undefined);
    return value === undefined ? undefined : { value, label: value };
  }
  const descriptor = arr(obj(model.capabilities).optionDescriptors).find(isReasoningSelect);
  if (descriptor === undefined) return undefined;
  const value = statedValue(selection, str(descriptor.id)) ?? currentValue(descriptor, selection);
  if (typeof value !== 'string') return undefined;
  return { value, label: str(arr(descriptor.options).find(option => option.id === value)?.label, value) };
}

/**
 * ModelSwitchRow's label, "<from> → <to>": each side's model when the model changed, then its effort when the effort
 * changed (an unset effort reads as the model's default); '' when the switch changed neither.
 */
export function modelSwitchLabel(modelSwitch: unknown, providers: Obj[]): string {
  const from = obj(obj(modelSwitch).from), to = obj(obj(modelSwitch).to);
  if (!str(from.model) || !str(to.model)) return '';
  const providerOf = (selection: Obj) => providers.find(provider => provider.instanceId === selection.instanceId);
  const fromEffort = resolveEffort(from, providerOf(from) && arr(providerOf(from)!.models)), toEffort = resolveEffort(to, providerOf(to) && arr(providerOf(to)!.models));
  const modelChanged = from.model !== to.model;
  const effort = fromEffort && toEffort && fromEffort.value !== toEffort.value ? { from: fromEffort.label, to: toEffort.label } : null;
  if (!modelChanged && !effort) return '';
  const describe = (selection: Obj, effortLabel: string | undefined) => {
    const provider = providerOf(selection), model = arr(provider?.models).find(candidate => candidate.slug === selection.model);
    const modelName = model ? triggerModelName(model, str(provider?.driver) || undefined) : str(selection.model);
    return [modelChanged ? modelName : null, effortLabel].filter(Boolean).join(' ');
  };
  return `${describe(from, effort?.from)} → ${describe(to, effort?.to)}`;
}
