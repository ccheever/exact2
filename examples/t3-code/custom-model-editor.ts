// The custom model editor, T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// apps/web/src/components/settings/customModelEditor.logic.ts (whole), CustomModelEditor.tsx
// (its draft updates, presets, "Copy from…", Save and Cancel), ProviderModelsSection.tsx:564-587
// (one editor under the edited row, handleSaveEdit) and ProviderInstanceCard.tsx:168-186
// (deriveProviderModelsForDisplay), with packages/shared/src/model.ts readCustomModelEntries,
// toCustomModelSetting and createModelCapabilities.
// Changes: the editor's draft lives here, one per environment connection (the reference keeps it
// in the editor's React state), and the Contract fields send each change as an `upkeep:cm-*` op;
// typing never bumps a row's revision, so the field being typed in keeps its own text, while a
// structural change (a preset, Copy from, a type) redraws that option from the draft. Save writes
// `config.customModels` through the instance upsert; ACP Registry instances store plain slugs
// (its settings schema is a string list), so their names and options are not kept.
import { arr, obj, str, type Json, type Obj } from './domain';

export interface EditorChoice { readonly key: string; readonly id: string; readonly label: string; readonly isDefault: boolean; readonly description?: string }
export interface EditorDescriptor {
  readonly key: string; readonly type: 'select' | 'boolean'; readonly id: string; readonly label: string;
  readonly choices: readonly EditorChoice[]; readonly currentBooleanValue?: boolean | undefined; readonly description?: string | undefined;
}
export interface CustomModelDraft { readonly slug: string; readonly name: string; readonly descriptors: readonly EditorDescriptor[] }
export interface DescriptorPreset { readonly id: string; readonly label: string; readonly type: 'select' | 'boolean'; readonly choices?: readonly { id: string; label: string; isDefault?: boolean }[] }
export type OptionChoice = { id: string; label: string; description?: string; isDefault?: boolean };
export type OptionDescriptor = { id: string; label: string; type: 'select' | 'boolean'; description?: string; options?: OptionChoice[]; currentValue?: string | boolean; promptInjectedValues?: string[] };
export type ModelCapabilities = { optionDescriptors: OptionDescriptor[] };
export interface CustomModelDefinition { readonly slug: string; readonly name: string; readonly capabilities: ModelCapabilities | null }

const EFFORT_CHOICES = [{ id: 'low', label: 'Low' }, { id: 'medium', label: 'Medium', isDefault: true }, { id: 'high', label: 'High' }, { id: 'xhigh', label: 'Extra High' }];

/** Option ids each adapter actually reads off a turn's model selection, with the usual choices pre-filled. */
export const DESCRIPTOR_PRESETS_BY_KIND: Record<string, readonly DescriptorPreset[]> = {
  codex: [{ id: 'reasoningEffort', label: 'Reasoning', type: 'select', choices: EFFORT_CHOICES },
    { id: 'serviceTier', label: 'Speed', type: 'select', choices: [{ id: 'default', label: 'Standard', isDefault: true }, { id: 'fast', label: 'Fast' }] }],
  claudeAgent: [{ id: 'effort', label: 'Reasoning', type: 'select', choices: [{ id: 'low', label: 'Low' }, { id: 'medium', label: 'Medium' }, { id: 'high', label: 'High', isDefault: true },
    { id: 'xhigh', label: 'Extra High' }, { id: 'max', label: 'Max' }] }, { id: 'fastMode', label: 'Fast Mode', type: 'boolean' }, { id: 'thinking', label: 'Thinking', type: 'boolean' }],
  cursor: [{ id: 'reasoning', label: 'Reasoning', type: 'select', choices: EFFORT_CHOICES }, { id: 'fastMode', label: 'Fast Mode', type: 'boolean' }, { id: 'thinking', label: 'Thinking', type: 'boolean' }],
  grok: [{ id: 'reasoningEffort', label: 'Reasoning', type: 'select', choices: EFFORT_CHOICES }],
  pi: [{ id: 'thinking', label: 'Thinking', type: 'select', choices: [{ id: 'off', label: 'Off' }, { id: 'minimal', label: 'Minimal' }, { id: 'low', label: 'Low' },
    { id: 'medium', label: 'Medium', isDefault: true }, { id: 'high', label: 'High' }, { id: 'xhigh', label: 'Extra High' }, { id: 'max', label: 'Max' }] }],
  opencode: [{ id: 'variant', label: 'Reasoning', type: 'select', choices: EFFORT_CHOICES },
    { id: 'agent', label: 'Agent', type: 'select', choices: [{ id: 'build', label: 'Build', isDefault: true }, { id: 'plan', label: 'Plan' }] }],
};

let nextKey = 0;
function newEditorKey(): string { nextKey += 1; return `k${nextKey}`; }
export function choiceFromPreset(choice: { id: string; label: string; isDefault?: boolean }): EditorChoice {
  return { key: newEditorKey(), id: choice.id, label: choice.label, isDefault: !!choice.isDefault };
}
export function descriptorFromPreset(preset: DescriptorPreset): EditorDescriptor {
  return { key: newEditorKey(), type: preset.type, id: preset.id, label: preset.label, choices: (preset.choices ?? []).map(choiceFromPreset) };
}
export function emptyEditorDescriptor(): EditorDescriptor { return { key: newEditorKey(), type: 'select', id: '', label: '', choices: [] }; }
export function emptyEditorChoice(): EditorChoice { return { key: newEditorKey(), id: '', label: '', isDefault: false }; }

/** Prompt-injected choices (Claude's `ultrathink`) are dropped rather than stored as a plain option value. */
function descriptorToEditor(descriptor: OptionDescriptor): EditorDescriptor {
  const promptInjected = new Set(descriptor.type === 'select' ? descriptor.promptInjectedValues ?? [] : []);
  const choices = descriptor.type === 'select' ? (descriptor.options ?? []).filter(option => !promptInjected.has(option.id)) : [];
  const defaultChoice = choices.find(option => option.id === descriptor.currentValue) ?? choices.find(option => option.isDefault);
  return {
    key: newEditorKey(), type: descriptor.type, id: descriptor.id, label: descriptor.label,
    ...(descriptor.description !== undefined ? { description: descriptor.description } : {}),
    ...(descriptor.type === 'boolean' && descriptor.currentValue !== undefined ? { currentBooleanValue: descriptor.currentValue as boolean } : {}),
    choices: choices.map(option => ({ key: newEditorKey(), id: option.id, label: option.label, ...(option.description !== undefined ? { description: option.description } : {}), isDefault: option === defaultChoice })),
  };
}
export function draftFromDefinition(entry: CustomModelDefinition): CustomModelDraft {
  return { slug: entry.slug, name: entry.name === entry.slug ? '' : entry.name, descriptors: (entry.capabilities?.optionDescriptors ?? []).map(descriptorToEditor) };
}
/** Claude context choices require runtime suffix mappings that custom entries do not carry. */
export function descriptorsFromCapabilities(capabilities: ModelCapabilities | null | undefined, driverKind: string | null): EditorDescriptor[] {
  return (capabilities?.optionDescriptors ?? []).filter(descriptor => driverKind !== 'claudeAgent' || descriptor.id !== 'contextWindow').map(descriptorToEditor);
}
/** The first problem in reading order, or null when the draft is sound. */
export function validateDraft(draft: CustomModelDraft): string | null {
  const seenIds = new Set<string>();
  for (const [index, descriptor] of draft.descriptors.entries()) {
    const position = `Option ${index + 1}`, id = descriptor.id.trim();
    if (!id) return `${position} needs an id.`;
    if (seenIds.has(id)) return `${position}: id "${id}" is used twice.`;
    seenIds.add(id);
    if (!descriptor.label.trim()) return `${position} needs a label.`;
    if (descriptor.type !== 'select') continue;
    if (descriptor.choices.length === 0) return `${position} needs at least one choice.`;
    const seenChoices = new Set<string>();
    for (const choice of descriptor.choices) {
      const choiceId = choice.id.trim();
      if (!choiceId) return `${position} has a choice without a value.`;
      if (seenChoices.has(choiceId)) return `${position}: choice "${choiceId}" is used twice.`;
      seenChoices.add(choiceId);
    }
  }
  return null;
}
/** createModelCapabilities: a copy of each descriptor. */
const createModelCapabilities = (input: { optionDescriptors: readonly OptionDescriptor[] }): ModelCapabilities =>
  ({ optionDescriptors: input.optionDescriptors.map(descriptor => descriptor.type === 'select'
    ? { ...descriptor, options: [...descriptor.options ?? []], ...(descriptor.promptInjectedValues ? { promptInjectedValues: [...descriptor.promptInjectedValues] } : {}) } : { ...descriptor }) });
/** A validated draft as a definition. Blank name → slug. */
export function definitionFromDraft(draft: CustomModelDraft): CustomModelDefinition {
  const descriptors: OptionDescriptor[] = draft.descriptors.map(descriptor => {
    const id = descriptor.id.trim(), label = descriptor.label.trim();
    if (descriptor.type === 'boolean') {
      return { id, label, type: 'boolean', ...(descriptor.description !== undefined ? { description: descriptor.description } : {}),
        ...(descriptor.currentBooleanValue !== undefined ? { currentValue: descriptor.currentBooleanValue } : {}) };
    }
    const options = descriptor.choices.map(choice => ({ id: choice.id.trim(), label: choice.label.trim() || choice.id.trim(),
      ...(choice.description !== undefined ? { description: choice.description } : {}), ...(choice.isDefault ? { isDefault: true } : {}) }));
    const currentValue = options.find(option => option.isDefault)?.id;
    return { id, label, type: 'select', ...(descriptor.description !== undefined ? { description: descriptor.description } : {}), options, ...(currentValue ? { currentValue } : {}) };
  });
  const name = draft.name.trim();
  return { slug: draft.slug, name: name || draft.slug, capabilities: descriptors.length > 0 ? createModelCapabilities({ optionDescriptors: descriptors }) : null };
}

// --- packages/shared/src/model.ts ------------------------------------------------------------

const isText = (value: unknown): value is string => typeof value === 'string' && value.trim() !== '';
/** decodeCustomModelCapabilities (Schema.decodeUnknownOption(ModelCapabilities)): null unless every descriptor is well formed. */
function decodeCapabilities(value: unknown): ModelCapabilities | null {
  const descriptors = obj(value).optionDescriptors;
  if (descriptors !== undefined && !Array.isArray(descriptors)) return null;
  const list = arr(descriptors);
  const valid = list.length === (Array.isArray(descriptors) ? descriptors.length : 0) && list.every(item => isText(item.id) && isText(item.label)
    && (item.type === 'boolean' ? item.currentValue === undefined || typeof item.currentValue === 'boolean'
      : item.type === 'select' && Array.isArray(item.options) && arr(item.options).length === item.options.length && arr(item.options).every(option => isText(option.id) && isText(option.label))));
  return valid ? { optionDescriptors: list as unknown as OptionDescriptor[] } : null;
}
/** readCustomModelEntries: bare slugs and records; slugs trimmed and deduplicated (first wins); name falls back to the slug. */
export function readCustomModelEntries(value: unknown): CustomModelDefinition[] {
  if (!Array.isArray(value)) return [];
  const entries: CustomModelDefinition[] = [], seen = new Set<string>();
  for (const raw of value) {
    const record = typeof raw === 'string' ? { slug: raw } : raw !== null && typeof raw === 'object' ? raw as Obj : null;
    if (!record) continue;
    const slug = typeof record.slug === 'string' ? record.slug.trim() : '';
    if (!slug || seen.has(slug)) continue;
    seen.add(slug);
    const name = (typeof record.name === 'string' ? record.name.trim() : '') || slug;
    const capabilities = record.capabilities === undefined || record.capabilities === null ? null : decodeCapabilities(record.capabilities);
    entries.push({ slug, name, capabilities: capabilities ? createModelCapabilities({ optionDescriptors: capabilities.optionDescriptors }) : null });
  }
  return entries;
}
/** toCustomModelSetting: a bare slug unless it has a name or options. */
export function toCustomModelSetting(entry: CustomModelDefinition): Json {
  const descriptors = entry.capabilities?.optionDescriptors ?? [], name = entry.name !== entry.slug ? entry.name : undefined;
  if (!name && descriptors.length === 0) return entry.slug;
  return { slug: entry.slug, ...(name ? { name } : {}), ...(descriptors.length > 0 ? { capabilities: createModelCapabilities({ optionDescriptors: descriptors }) as unknown as Json } : {}) } as Json;
}
/** The stored `customModels` for a driver: ACP Registry keeps plain slugs. */
export function storedCustomModels(driver: string, entries: readonly CustomModelDefinition[]): Json[] {
  return driver === 'acpRegistry' ? entries.map(entry => entry.slug) : entries.map(toCustomModelSetting);
}

/**
 * deriveProviderModelsForDisplay: built-ins from the server, custom rows from the current
 * config, each keeping the server's capabilities only when the entry has none of its own.
 */
export function deriveProviderModelsForDisplay(input: { liveModels: readonly Obj[] | undefined; customModels: readonly CustomModelDefinition[] }): Obj[] {
  const liveCustom = new Map((input.liveModels ?? []).filter(model => model.isCustom === true).map(model => [str(model.slug), model] as const));
  const serverModels = (input.liveModels ?? []).filter(model => model.isCustom !== true);
  const customModels = input.customModels.map(entry => ({ slug: entry.slug, name: entry.name, isCustom: true,
    capabilities: (entry.capabilities ?? liveCustom.get(entry.slug)?.capabilities ?? null) as Json }));
  return [...serverModels, ...customModels];
}

// --- The editor's session: one draft per connection ------------------------------------------

type Session = { instanceId: string; driver: string; open: number; draft: CustomModelDraft; error: string; revs: Map<string, number> };
const sessions = new WeakMap<object, Session>();
let opened = 0;

/** The open editor of `instanceId`, if any (a different instance's editor closes, as its card unmounts). */
export function editorSession(owner: object, instanceId: string): Session | null {
  const session = sessions.get(owner);
  if (session && session.instanceId !== instanceId) { sessions.delete(owner); return null; }
  return session ?? null;
}
const rev = (session: Session, key: string) => session.revs.get(key) ?? 0;
function touch(session: Session, ...keys: string[]) { for (const key of keys) session.revs.set(key, rev(session, key) + 1); }

const CUSTOM_ID_VALUE = '__custom__';
/** The editor as the Contract draws it (CustomModelEditor), or none. */
export function customModelEditorView(owner: object, instanceId: string, driver: string, liveModels: readonly Obj[]) {
  const session = editorSession(owner, instanceId);
  if (!session) return [];
  const presets = DESCRIPTOR_PRESETS_BY_KIND[driver] ?? [], draft = session.draft;
  const startFrom = liveModels.filter(model => model.isCustom !== true && arr(obj(model.capabilities).optionDescriptors).length > 0);
  return [{
    key: `${instanceId}:${draft.slug}:${session.open}`, instanceId, slug: draft.slug, name: draft.name, error: session.error,
    empty: draft.descriptors.length === 0, hasPresets: presets.length > 0,
    copyFrom: startFrom.map(model => ({ value: str(model.slug), label: str(model.name) || str(model.slug), note: '', selected: false })),
    addPresets: presets.filter(preset => !draft.descriptors.some(descriptor => descriptor.id === preset.id)).map(preset => ({ id: preset.id, label: preset.label })),
    descriptors: draft.descriptors.map((descriptor, index) => {
      const isPreset = presets.some(preset => preset.id === descriptor.id);
      return {
        key: `${descriptor.key}:${rev(session, descriptor.key)}`, ref: descriptor.key, index, position: `Option ${index + 1}`,
        idSelect: isPreset ? descriptor.id : CUSTOM_ID_VALUE, idSelectLabel: isPreset ? descriptor.id : 'Custom…', customId: !isPreset,
        idOptions: [...presets.map(preset => ({ value: preset.id, label: preset.id, note: preset.label, selected: isPreset && preset.id === descriptor.id })),
          { value: CUSTOM_ID_VALUE, label: 'Custom…', note: '', selected: !isPreset }],
        id: descriptor.id, label: descriptor.label, type: descriptor.type, typeLabel: descriptor.type === 'boolean' ? 'Toggle' : 'Choices', isSelect: descriptor.type === 'select',
        typeOptions: [{ value: 'select', label: 'Choices', note: '', selected: descriptor.type === 'select' }, { value: 'boolean', label: 'Toggle', note: '', selected: descriptor.type === 'boolean' }],
        choices: descriptor.choices.map(choice => ({ key: `${descriptor.key}:${choice.key}`, ref: `${descriptor.key}:${choice.key}`, id: choice.id, label: choice.label, isDefault: choice.isDefault })),
      };
    }),
  }];
}

const updateDescriptor = (session: Session, key: string, patch: Partial<EditorDescriptor>) =>
  ({ ...session.draft, descriptors: session.draft.descriptors.map(descriptor => descriptor.key === key ? { ...descriptor, ...patch } : descriptor) });

export interface CustomModelHost { config: Obj }
/**
 * One editor op (`upkeep:cm-<what>`): `id` is the instance, `field` names the target
 * (`name`; `d:<key>:id|label`; `c:<descriptorKey>:<choiceKey>:id|label`; a descriptor or
 * `<descriptorKey>:<choiceKey>` ref), `value` the new text or choice. Returns the definition to
 * save for `save`, else null.
 */
export function customModelEditorOp(owner: object, host: CustomModelHost, what: string, id: string, field: string, value: string): CustomModelDefinition | null {
  if (what === 'open') {
    const current = sessions.get(owner);
    if (current?.instanceId === id && current.draft.slug === field) { sessions.delete(owner); return null; } // the pencil toggles (setEditingSlug)
    const instance = obj(obj(obj(host.config.settings).providerInstances)[id]);
    const entry = readCustomModelEntries(obj(instance.config).customModels).find(candidate => candidate.slug === field);
    if (!entry) return null;
    sessions.set(owner, { instanceId: id, driver: str(instance.driver), open: ++opened, draft: draftFromDefinition(entry), error: '', revs: new Map() });
    return null;
  }
  const session = editorSession(owner, id);
  if (!session) return null;
  const presets = DESCRIPTOR_PRESETS_BY_KIND[session.driver] ?? [];
  const [kind, descriptorKey = '', third = '', fourth = ''] = field.split(':');
  switch (what) {
    case 'cancel': sessions.delete(owner); return null;
    case 'text':
      if (kind === 'name') { session.draft = { ...session.draft, name: value }; return null; }
      session.error = '';
      if (kind === 'd') session.draft = updateDescriptor(session, descriptorKey, third === 'label' ? { label: value } : { id: value });
      else if (kind === 'c') session.draft = { ...session.draft, descriptors: session.draft.descriptors.map(descriptor => descriptor.key !== descriptorKey ? descriptor
        : { ...descriptor, choices: descriptor.choices.map(choice => choice.key === third ? { ...choice, ...(fourth === 'label' ? { label: value } : { id: value }) } : choice) }) };
      return null;
    case 'preset-id': {
      // applyPresetId: a preset id fills label, type and choices; "Custom…" clears the id to type into.
      session.error = '';
      if (value === CUSTOM_ID_VALUE) session.draft = updateDescriptor(session, field, { id: '' });
      else {
        const preset = presets.find(candidate => candidate.id === value);
        if (!preset) return null;
        session.draft = updateDescriptor(session, field, { id: preset.id, label: preset.label, type: preset.type, choices: (preset.choices ?? []).map(choiceFromPreset), currentBooleanValue: undefined, description: undefined });
      }
      touch(session, field);
      return null;
    }
    case 'type': session.error = ''; session.draft = updateDescriptor(session, field, { type: value === 'boolean' ? 'boolean' : 'select' }); touch(session, field); return null;
    case 'remove-option': session.error = ''; session.draft = { ...session.draft, descriptors: session.draft.descriptors.filter(descriptor => descriptor.key !== field) }; return null;
    case 'add-option': {
      const preset = presets.find(candidate => candidate.id === field);
      session.error = '';
      session.draft = { ...session.draft, descriptors: [...session.draft.descriptors, preset ? descriptorFromPreset(preset) : emptyEditorDescriptor()] };
      return null;
    }
    case 'add-choice': {
      const descriptor = session.draft.descriptors.find(candidate => candidate.key === field);
      if (!descriptor) return null;
      session.error = ''; session.draft = updateDescriptor(session, field, { choices: [...descriptor.choices, emptyEditorChoice()] });
      return null;
    }
    case 'remove-choice': {
      const [ownerKey = '', choiceKey = ''] = field.split(':'), descriptor = session.draft.descriptors.find(candidate => candidate.key === ownerKey);
      if (!descriptor) return null;
      session.error = ''; session.draft = updateDescriptor(session, ownerKey, { choices: descriptor.choices.filter(choice => choice.key !== choiceKey) });
      return null;
    }
    case 'default': {
      // Only one choice can be the default.
      const [ownerKey = '', choiceKey = ''] = field.split(':'), checked = value === 'true';
      session.error = '';
      session.draft = { ...session.draft, descriptors: session.draft.descriptors.map(descriptor => descriptor.key !== ownerKey ? descriptor
        : { ...descriptor, choices: descriptor.choices.map(choice => choice.key === choiceKey ? { ...choice, isDefault: checked } : checked ? { ...choice, isDefault: false } : choice) }) };
      return null;
    }
    case 'copy-from': {
      const instanceProviders = arr(host.config.providers).find(provider => provider.instanceId === id);
      const model = arr(instanceProviders?.models).find(candidate => candidate.isCustom !== true && candidate.slug === field && arr(obj(candidate.capabilities).optionDescriptors).length > 0);
      if (!model) return null;
      session.error = ''; session.draft = { ...session.draft, descriptors: descriptorsFromCapabilities(obj(model.capabilities) as unknown as ModelCapabilities, session.driver) };
      return null;
    }
    case 'save': {
      const problem = validateDraft(session.draft);
      if (problem) { session.error = problem; return null; }
      return definitionFromDraft(session.draft);
    }
    case 'saved': sessions.delete(owner); return null;
    default: return null;
  }
}
