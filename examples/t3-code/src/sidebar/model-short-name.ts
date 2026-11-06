// The model label a row shows (T3 Code 7c3b3b3605ef, MIT; see LICENSE-T3: chat/providerIconUtils.ts
// getTriggerDisplayModelName): the composer's trigger label over the model's spoken short name under its driver, as
// client/presentation.ts builds the composer's own.
import type { Obj } from '../client/domain';
import { resolveModelShortName } from '../client/model-short-name';
import { triggerModelName } from '../composer/composer-controls-model';

/** getTriggerDisplayModelName(model, driverKind). */
export function triggerDisplayModelName(model: Obj, driver: string): string {
  return triggerModelName({ ...model, shortName: resolveModelShortName(model, driver) ?? '' });
}
