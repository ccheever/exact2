// Pinned365aa87982 modelOptions.ts and projectSettings.ts default resolution.
// @ref llp/1107.005-composer-and-transcript.decision.md#new-task-ownership
import { arr, obj, str, type Obj } from './shared/domain';
import { driverMeta } from './shared/providers-meta';
import type { T3Client } from './shared/client';

type Selection = { instanceId: string; model: string; options: Obj[] };
function selection(value: unknown): Selection | null {
  const raw = obj(value), instanceId = str(raw.instanceId), model = str(raw.model);
  return instanceId && model ? { instanceId, model, options: arr(raw.options).map(option => ({ ...option })) } : null;
}
function providerEnabled(settings: Obj, id: string): boolean {
  const value = obj(settings.providerInstances)[id];
  if (value !== undefined) {
    const instance = obj(value), enabled = obj(instance.config).enabled;
    if (instance.enabled === false || enabled === false) return false;
    if (instance.enabled === true || enabled === true) return true;
    return driverMeta(str(instance.driver))?.enabledByDefault ?? true;
  }
  return /^[a-zA-Z][a-zA-Z0-9_-]{0,63}$/.test(id) && obj(obj(settings.providers)[id]).enabled === true;
}
function projectDefault(client: T3Client): Selection | null {
  const settings = obj(client.config.settings), project = client.shell.projects.find(value => value.id === client.projectId);
  const overrides = { ...(!settings.projectSettingsFolded && project?.defaultModelSelection != null
    ? { defaultModelSelection: project.defaultModelSelection } : {}), ...obj(obj(settings.projectSettingsOverrides)[client.projectId]) };
  const raw = overrides.defaultModelSelection, override = selection(raw);
  // Present null clears the environment choice; undefined is not an override.
  return raw !== undefined && (raw === null || override && providerEnabled(settings, override.instanceId))
    ? override : selection(settings.defaultModelSelection);
}
function defaultable(config: Obj, value: Selection | null): Selection | null {
  if (!value) return null;
  const provider = arr(config.providers).find(candidate => candidate.instanceId === value.instanceId);
  const driver = str(provider?.driver, str(obj(obj(obj(config.settings).providerInstances)[value.instanceId]).driver));
  if (driver === 'antigravity') return value;
  if (!provider?.enabled || !provider.installed || obj(provider.auth).status === 'unauthenticated') return null;
  return arr(provider.models).find(model => model.slug === value.model)?.isLegacy === true ? null : value;
}
/** Fresh-draft defaults only. Existing explicit settings actions still own picks;
 * this runs at the shared client's existing reset/reconnect entry points. */
export function mobileNewTaskDefaultModel(client: T3Client): Selection | null {
  const configured = defaultable(client.config, projectDefault(client));
  if (configured) return configured;
  const prefs = client.local.composerControls, sticky = prefs.stickyByProvider[prefs.stickyProvider];
  const remembered = defaultable(client.config, sticky ? selection({ instanceId: prefs.stickyProvider, ...sticky }) : null);
  if (remembered) return remembered;
  // Source buildModelOptions preserves provider/catalog order and does not
  // require runtime readiness to display a choice. Send keeps its own guards.
  const choices = arr(client.config.providers).flatMap(provider => {
    if (!provider.enabled || !provider.installed || obj(provider.auth).status === 'unauthenticated'
      || provider.driver === 'antigravity' && provider.availability === 'unavailable') return [];
    return arr(provider.models).map(model => ({ instanceId: str(provider.instanceId), model: str(model.slug), options: [], isDefault: model.isDefault === true }));
  });
  const picked = choices.find(value => value.isDefault) ?? choices[0];
  return picked ? { instanceId: picked.instanceId, model: picked.model, options: [] } : null;
}
