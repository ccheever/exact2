// The text generation model a setting resolves to, adapted from T3 Code
// 9c161462c9 (MIT); see LICENSE-T3. Sources: apps/web/src/modelSelection.ts
// (resolveAppModelSelectionState, resolveAppModelSelectionForInstance) and
// packages/contracts/src/model.ts (DEFAULT_TEXT_GENERATION_MODEL_BY_PROVIDER).
// Changes: `providers` are already the enabled, available instances that
// support text generation; a model's options are the provider's catalog.
import { arr, obj, str, type Obj } from '../client/domain';
import { resolveSelectableModel } from '../composer/composer-controls-model';

/** Each driver's cheap text generation model; the catalog default is the flagship chat model. */
export const DEFAULT_TEXT_GENERATION_MODEL_BY_PROVIDER: Record<string, string> = {
  codex: 'gpt-6-luna', antigravity: 'antigravity-default', claudeAgent: 'claude-haiku-5-5', cursor: 'composer-2', opencode: 'openai/gpt-5',
};

/**
 * resolveAppModelSelectionState: the saved instance while it is offered, else
 * the first offered one; then the saved model, the driver's text generation
 * default, the catalog default, the first model. Options carry over only on
 * the saved instance.
 */
export function resolveTextGenerationSelection(selection: Obj | null, providers: Obj[]): { provider: Obj; model: Obj | undefined; options: Obj[] } | null {
  const selected = selection ? providers.find(provider => provider.instanceId === selection.instanceId) : undefined;
  const provider = selected ?? providers[0];
  if (!provider) return null;
  const driver = str(provider.driver), models = arr(provider.models);
  const slug = (selected ? resolveSelectableModel(driver, selection!.model, models) : '')
    || resolveSelectableModel(driver, DEFAULT_TEXT_GENERATION_MODEL_BY_PROVIDER[driver] ?? '', models)
    || str(models.find(model => model.isDefault === true)?.slug, str(models[0]?.slug));
  return { provider, model: models.find(model => model.slug === slug), options: selected ? arr(obj(selection).options) : [] };
}
