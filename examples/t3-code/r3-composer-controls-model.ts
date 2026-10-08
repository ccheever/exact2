// Model names and provider-reported option values (lane composer-controls,
// round 3), adapted from T3 Code (MIT); see LICENSE-T3. Sources:
// packages/shared/src/model.ts (resolveSelectableModel, formatModelSlugName,
// normalizeModelSlug, getProviderOptionCurrentValue/Label),
// packages/contracts/src/model.ts (MODEL_SLUG_ALIASES_BY_PROVIDER),
// apps/web/src/components/chat/providerIconUtils.ts (getTriggerDisplayModelName)
// and packages/client-runtime/src/state/threadExecution.ts
// (deriveReportedModelSelection).
import { arr, obj, str, type Obj } from './domain';

const ALIASES: Record<string, Record<string, string>> = {
  codex: { 'gpt-5-codex': 'gpt-5.4', '5.4': 'gpt-5.4', '5.3': 'gpt-5.3-codex', 'gpt-5.3': 'gpt-5.3-codex',
    '5.3-spark': 'gpt-5.3-codex-spark', 'gpt-5.3-spark': 'gpt-5.3-codex-spark' },
  cursor: { composer: 'composer-2', 'composer-1.5': 'composer-1.5', 'composer-1': 'composer-1.5', 'opus-4.6-thinking': 'claude-opus-4-6',
    'opus-4.6': 'claude-opus-4-6', 'sonnet-4.6-thinking': 'claude-sonnet-4-6', 'sonnet-4.6': 'claude-sonnet-4-6',
    'opus-4.5-thinking': 'claude-opus-4-5', 'opus-4.5': 'claude-opus-4-5' },
};

/** resolveSelectableModel: slug, then name (any case), then an alias, then the driver's slug aliases. */
export function resolveSelectableModel(driver: string, value: unknown, models: Obj[]): string {
  const trimmed = typeof value === 'string' ? value.trim() : '';
  if (!trimmed) return '';
  const lower = trimmed.toLowerCase();
  const found = models.find(model => model.slug === trimmed) ?? models.find(model => str(model.name).toLowerCase() === lower)
    ?? models.find(model => Array.isArray(model.aliases) && (model.aliases as unknown[]).some(alias => typeof alias === 'string' && alias.toLowerCase() === lower));
  if (found) return str(found.slug);
  const aliases = ALIASES[driver] ?? {};
  const normalized = Object.prototype.hasOwnProperty.call(aliases, trimmed) ? aliases[trimmed]! : trimmed;
  return str(models.find(model => model.slug === normalized)?.slug);
}

/** formatModelSlugName: a slug the catalog does not describe, in the catalog's spelling where the family is known. */
export function formatModelSlugName(slug: string): string {
  const separator = slug.lastIndexOf('/') + 1, prefix = slug.slice(0, separator), name = slug.slice(separator);
  if (/^gpt-\d/i.test(name)) return prefix + name.replace(/^gpt/i, 'GPT').replace(/-([a-z])/g, (_, letter: string) => `-${letter.toUpperCase()}`);
  if (!/^(claude-(opus|sonnet|haiku|fable)|gemini|grok|composer)-\d/i.test(name)) return slug;
  return prefix + name.replace(/^(claude-[a-z]+-\d+)-(\d{1,2})(?=-|\[|$)/i, '$1.$2').split('-').map(part => part.charAt(0).toUpperCase() + part.slice(1)).join(' ');
}

/** getTriggerDisplayModelName: the short name when there is one, minus a leading sub-provider qualifier. */
export function triggerModelName(model: Obj): string {
  const name = str(model.shortName) || str(model.name), qualifier = str(model.subProvider).trim();
  if (!qualifier) return name;
  const pattern = new RegExp(`^${qualifier.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}(?:\\s*[.:/-]\\s*|\\s+)`, 'iu');
  return name.replace(pattern, '').trim() || name;
}

/** The read-only bar's model label: a reported dated id or alias resolves to its catalog entry. */
export function reportedModelLabel(driver: string, reported: string, models: Obj[]): string {
  const slug = resolveSelectableModel(driver, reported, models);
  const model = slug ? models.find(candidate => candidate.slug === slug) : undefined;
  return model ? triggerModelName(model) : formatModelSlugName(reported);
}

export type Selection = { instanceId: string; model: string; options: Obj[] };

/** deriveReportedModelSelection: what the active provider thread (never a previous handoff's) says it runs. */
export function reportedSelection(projection: Obj): Selection | null {
  const thread = obj(projection.thread), instanceId = str(obj(thread.modelSelection).instanceId);
  const providerThread = arr(projection.providerThreads).find(entry => entry.id === thread.activeProviderThreadId && entry.providerInstanceId === instanceId);
  const reported = obj(obj(providerThread?.nativeMetadata).modelSelection);
  if (!str(reported.instanceId) || !str(reported.model)) return null;
  return { instanceId: str(reported.instanceId), model: str(reported.model), options: arr(reported.options) };
}

const raw = (options: Obj[], id: string): unknown => options.find(option => option.id === id)?.value;
/** withDescriptorCurrentValue: a stored choice (when the descriptor offers it) becomes the descriptor's current value. */
export function resolvedCurrent(descriptor: Obj, selections: Obj[]): unknown {
  const chosen = raw(selections, str(descriptor.id));
  if (descriptor.type === 'boolean') return typeof chosen === 'boolean' ? chosen : descriptor.currentValue;
  const options = arr(descriptor.options), fallback = descriptor.currentValue ?? options.find(option => option.isDefault === true)?.id;
  const trimmed = typeof chosen === 'string' ? chosen.trim() : '';
  if (!trimmed) return fallback;
  if (!options.length) return trimmed;
  const injected = Array.isArray(descriptor.promptInjectedValues) && (descriptor.promptInjectedValues as unknown[]).includes(trimmed);
  if (injected && options.some(option => option.id === trimmed)) return options.find(option => option.isDefault === true)?.id;
  return options.some(option => option.id === trimmed) ? trimmed : fallback;
}

/** getReportedOptionValue: the provider's report wins only for the same instance and model, and only while the user has not chosen. */
function reportedValue(id: string, selection: Selection | null, reported: Selection | null): unknown {
  if (!selection || !reported || selection.instanceId !== reported.instanceId || selection.model !== reported.model
    || selection.options.some(option => option.id === id)) return undefined;
  return raw(reported.options, id);
}

/** getProviderOptionCurrentValue over a descriptor whose current value already folds in the stored choice. */
export function optionValue(descriptor: Obj, current: unknown, selection: Selection | null, reported: Selection | null): unknown {
  const id = str(descriptor.id), value = reportedValue(id, selection, reported);
  if (value !== undefined) return value;
  if (id === 'variant' && selection && !selection.options.some(option => option.id === id)) return undefined;
  if (descriptor.type === 'boolean') return current;
  if (current) return current;
  return arr(descriptor.options).find(option => option.isDefault === true)?.id;
}

/** getProviderOptionCurrentLabel: "Default" for a reported default, "Unknown" for an unreported OpenCode variant. */
export function optionLabel(descriptor: Obj, current: unknown, selection: Selection | null, reported: Selection | null): string {
  if (descriptor.type === 'boolean') return typeof current === 'boolean' ? (current ? 'On' : 'Off') : '';
  const value = optionValue(descriptor, current, selection, reported);
  const label = arr(descriptor.options).find(option => option.id === value)?.label;
  if (typeof label === 'string') return label;
  return reportedValue(str(descriptor.id), selection, reported) === 'default' ? 'Default' : descriptor.id === 'variant' ? 'Unknown' : '';
}

/** formatShortcutLabel for an `aria-keyshortcuts` chord (Meta+Shift+M → ⇧⌘M): ⌃⌥⇧⌘, then the key. */
export function chordGlyphs(chord: string): string {
  if (!chord) return '';
  const parts = chord.split('+'), key = parts.pop() ?? '', has = (name: string) => parts.includes(name);
  const names: Record<string, string> = { Escape: 'Esc', ArrowUp: 'Up', ArrowDown: 'Down', ArrowLeft: 'Left', ArrowRight: 'Right' };
  return `${has('Control') ? '⌃' : ''}${has('Alt') ? '⌥' : ''}${has('Shift') ? '⇧' : ''}${has('Meta') ? '⌘' : ''}${names[key] ?? (key.length === 1 ? key.toUpperCase() : key)}`;
}

/** buildTraitsTriggerDisplay: speed traits become a bolt (two for Ultrafast); booleans read "<label> On|Off". */
export function traitsDisplay(driver: string, descriptors: Obj[], selections: Obj[], selection: Selection | null = null, reported: Selection | null = null,
  ultra: { primaryId: string; controlled: boolean } = { primaryId: '', controlled: false }) {
  let speed = '', fallback = '';
  const labels: string[] = [];
  const current = (descriptor: Obj) => resolvedCurrent(descriptor, selections) ?? arr(descriptor.options).find(option => option.isDefault === true)?.id;
  for (const descriptor of descriptors) {
    if (descriptor.id === 'fastMode' && descriptor.type === 'boolean') {
      speed = current(descriptor) === true ? 'fast' : ''; fallback = speed ? 'Fast' : 'Normal'; continue;
    }
    if (driver === 'codex' && descriptor.id === 'serviceTier' && descriptor.type === 'select') {
      const value = current(descriptor), options = arr(descriptor.options);
      const fast = options.find(option => option.label === 'Fast'), ultra = options.find(option => option.label === 'Ultrafast');
      if (((fast || ultra) && value === 'default') || (fast && value === fast.id) || (ultra && value === ultra.id)) {
        speed = ultra && value === ultra.id ? 'ultrafast' : fast && value === fast.id ? 'fast' : '';
        fallback = str(options.find(option => option.id === value)?.label, 'Normal'); continue;
      }
    }
    // composer-fidelity G9: the prompt-controlled primary effort reads "Ultrathink" (buildTraitsTriggerDisplay).
    if (ultra.controlled && descriptor.id === ultra.primaryId) { labels.push('Ultrathink'); continue; }
    if (descriptor.type === 'boolean') { labels.push(`${str(descriptor.label, str(descriptor.id))} ${current(descriptor) === true ? 'On' : 'Off'}`); continue; }
    if (descriptor.type !== 'select') continue;
    // getProviderOptionCurrentLabel: a provider-reported value labels an option the user left unset.
    const label = optionLabel(descriptor, resolvedCurrent(descriptor, selections), selection, reported);
    if (label) labels.push(label);
  }
  if (!labels.length && fallback) return { label: fallback, speed: '' };
  return { label: labels.join(' · '), speed };
}
