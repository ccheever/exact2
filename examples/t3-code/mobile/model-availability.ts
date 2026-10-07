// @ref llp/1106.005-composer-and-transcript.decision.md#settings-ownership
// Pinned mobile modelOptions.ts at365aa87982 preserves non-Antigravity selections
// absent from the current catalog. The server decides whether that model can run.
import { arr, obj, str, type Obj } from './shared/domain';
import { providerAvailable } from './shared/protocol';

export function mobileModelSelectionUnavailable(config: Obj | null | undefined, selection: Obj | null | undefined): boolean {
  if (!config || !selection) return false;
  const provider = arr(config.providers).find(value => value.instanceId === selection.instanceId);
  const instance = obj(obj(obj(config.settings).providerInstances)[str(selection.instanceId)]);
  const driver = str(provider?.driver, str(instance.driver));
  return driver === 'antigravity' && (!provider || !provider.enabled || !provider.installed
    || obj(provider.auth).status === 'unauthenticated' || provider.availability === 'unavailable'
    || !arr(provider.models).some(model => model.slug === selection.model));
}

/** Keep transport readiness separate from the mobile model-unavailable notice. */
export function mobileModelSelectionReady(config: Obj, selection: Obj): boolean {
  const provider = arr(config.providers).find(value => value.instanceId === selection.instanceId);
  return !!str(selection.instanceId) && !!str(selection.model) && !!provider && providerAvailable(provider)
    && !mobileModelSelectionUnavailable(config, selection);
}
