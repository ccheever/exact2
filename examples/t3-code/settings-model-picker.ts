// Settings' model pickers: the composer's ProviderModelPicker catalog (model-catalog.ts) over a settings
// row, at T3 Code 1e2ecbd975 (MIT, see LICENSE-T3).
// - General (New threads › Model, Text generation model): ProjectDefaultsSettings.tsx and SettingsPanels.tsx
//   give the shared picker lockedProvider null, the representative environment's instance entries (only
//   those that support text generation, for that row), the setting's own selection and
//   getModelDisabledReason (useScopedModelAvailability.ts). Its target is `<row>:|<machine>|<projectKey>|<checkout>`.
// - Scheduled Tasks › New task › Model (model-picker-parity S2-4, ScheduledTasksSettings.tsx:798-811): the
//   editing environment's instances, the draft's `instance:model` (or the first instance), no disabled reasons and
//   no provider setup. Its target is `task-model:|<environmentId>|<modelKey>`, both URI-encoded.
// - Source Control › the writer model (SourceControlWritingSettings.tsx:302-310): the text generation instances,
//   the writer selection, useScopedModelDisabledReason over the page's one environment and the provider setup.
//   Its target is `source-control-writer-model:|<environmentId>:<projectId>`, the page's scope.
// Browsing, searching and favoriting never write anything; a chosen row does (app.contract selectModel).
import type { T3Client } from './client';
import { arr, obj, str, type Obj } from './domain';
import { pickerCatalog } from './model-catalog';
import { scopedModelReason, settingsModelTarget } from './settings-core';
import { liveEnvironments } from './live-streams';

type Badge = Parameters<typeof pickerCatalog>[3];
type View = Parameters<typeof pickerCatalog>[0];

const view = (client: T3Client, providers: Obj[], providerId: string, modelId: string): View =>
  ({ config: { providers }, local: client.local, providerId, modelId });

/** Scheduled Tasks' Model: the editing environment's instances and the draft's model, or the first instance. */
export function taskModelView(client: T3Client, context: string): View {
  const [environment = '', key = ''] = context.split('|').map(part => { try { return decodeURIComponent(part); } catch { return ''; } });
  const live = liveEnvironments(client, null).find(entry => entry.environmentId === environment);
  const providers = arr(obj(live?.config ?? (environment && environment !== client.environmentId ? {} : client.config)).providers);
  const at = key.indexOf(':');
  const instanceId = at > 0 ? key.slice(0, at) : str(providers[0]?.instanceId);
  return view(client, providers, instanceId, at > 0 ? key.slice(at + 1) : '');
}

/** The writer model's selection for the page's project (its override, else the environment's). */
export function writerSelection(settings: Obj, projectId: string): Obj {
  const override = obj(obj(settings.projectSettingsOverrides)[projectId]);
  return obj(projectId && Object.prototype.hasOwnProperty.call(override, 'sourceControlWriterModelSelection') ? override.sourceControlWriterModelSelection : settings.sourceControlWriterModelSelection);
}

/** useScopedModelDisabledReason over the Source Control page's one environment. */
export function writerModelReason(client: T3Client, instanceId: string, model: string): string {
  const entry = arr(client.config.providers).find(provider => provider.instanceId === instanceId);
  if (entry && entry.enabled === true && entry.availability !== 'unavailable' && arr(entry.models).some(option => option.slug === model && option.isUnavailable !== true)) return '';
  return `This model is unavailable on ${str(obj(client.config.environment).label, 'a selected environment')}. Select that environment to choose its model separately.`;
}

/** The picker's catalog for one Settings model row; an unknown or unavailable target lists nothing. */
export function settingsPickerCatalog(client: T3Client, target: string, requested: string, query: string, badge: Badge) {
  if (target.startsWith('task-model:|')) {
    return pickerCatalog(taskModelView(client, target.slice('task-model:|'.length)), requested, query, badge, { reason: () => '', setup: false });
  }
  if (target.startsWith('source-control-writer-model:|')) {
    const scope = target.slice('source-control-writer-model:|'.length), projectId = scope.slice(scope.lastIndexOf(':') + 1);
    const providers = arr(client.config.providers).filter(provider => provider.supportsTextGeneration !== false);
    const selection = writerSelection(obj(client.config.settings), projectId);
    return pickerCatalog(view(client, providers, str(selection.instanceId), str(selection.model)), requested, query, badge,
      { reason: (instanceId, model) => writerModelReason(client, instanceId, model) });
  }
  let resolved: ReturnType<typeof settingsModelTarget> = null;
  try { resolved = settingsModelTarget(client, target); } catch { resolved = null; }
  return pickerCatalog(view(client, resolved?.providers ?? [], resolved?.instanceId ?? '', resolved?.model ?? ''), requested, query, badge,
    { reason: (instanceId, model) => resolved ? scopedModelReason(resolved.context, instanceId, model) : '' });
}
