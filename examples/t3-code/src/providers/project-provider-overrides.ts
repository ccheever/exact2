// ProviderSettingsPanel.logic.ts, T3 Code 9c161462c9 (MIT; LICENSE-T3).
import { obj, type Obj } from '../client/domain';

export function buildProviderInstanceOverridePatch(settings: Obj, projectIds: readonly string[], instanceId: string, value: boolean | undefined): Obj {
  const patch: Obj = {};
  for (const projectId of projectIds) {
    const current = obj(obj(settings.projectSettingsOverrides)[projectId]);
    const instances = { ...obj(current.providerInstances) }, instance = { ...obj(instances[instanceId]) };
    if (value === undefined) delete instance.enabled; else instance.enabled = value;
    if (Object.keys(instance).length) instances[instanceId] = instance; else delete instances[instanceId];
    const { providerInstances: _omit, ...rest } = current;
    // Explicit {} clears an old provider map; omitting the field preserves it on the server.
    patch[projectId] = Object.keys(rest).length || Object.keys(instances).length ? { ...rest, providerInstances: instances } : null;
  }
  return patch;
}

export function projectProviderEnabled(settings: Obj, projectId: string, instanceId: string, inherited: boolean): boolean {
  const enabled = obj(obj(obj(settings.projectSettingsOverrides)[projectId]).providerInstances)[instanceId];
  return typeof obj(enabled).enabled === 'boolean' ? obj(enabled).enabled === true : inherited;
}
