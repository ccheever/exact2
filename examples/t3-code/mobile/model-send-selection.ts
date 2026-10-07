// @ref llp/1107.005-composer-and-transcript.decision.md#retained-model-availability
// Pinned mobile 365aa87982 thread-outbox-model.ts preserves a retained selection.
import { arr, type Obj } from './shared/domain';
import { dispatchSelection } from './shared/composer-ultrathink';
import { modelSelection } from './shared/protocol';

export function mobileDispatchSelection(client: Parameters<typeof dispatchSelection>[0], providerId: string, modelId: string, options: unknown): Obj {
  const provider = arr(client.config.providers).find(value => value.instanceId === providerId);
  if (provider && provider.driver !== 'antigravity' && !arr(provider.models).some(model => model.slug === modelId)) {
    return modelSelection(providerId, modelId, options);
  }
  return dispatchSelection(client, providerId, modelId, options);
}
