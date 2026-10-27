// ProviderInstanceIcon's agent icon for the composer's model trigger and the picker's rail (T3 Code, MIT; see
// LICENSE-T3: applyProviderInstanceSettings). An ACP Registry instance's agent id and icon come from its settings
// entry's config; a local agent has none, and draws the registry glyph. The sidebar's rows read the same
// (sidebar-view.ts providerIconUrl).
import { obj, str, type Obj } from '../client/domain';
import { instanceIconUrl } from '../providers/acp-icons';

/** The instance's ACP agent icon URL from its environment's config: '' for another driver or a local agent. */
export function acpInstanceIcon(config: Obj, instance: Obj | undefined): string {
  if (str(instance?.driver) !== 'acpRegistry') return '';
  const entry = obj(obj(obj(obj(config.settings).providerInstances)[str(instance?.instanceId)]).config);
  return entry.source === 'local' ? '' : instanceIconUrl({ config: entry }, 'acpRegistry');
}
