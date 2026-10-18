// providerUpdateRequiredModels.ts, T3 Code 9c161462c9 (MIT; LICENSE-T3).
import { arr, str, type Obj } from '../client/domain';
import { compareSemver } from '../connections/version-skew';
import { providerDisplayName } from './provider-updates';

export function formatProviderUpdateRequiredNotice(provider: Obj, searchQuery = ''): string | null {
  const query = searchQuery.trim().toLocaleLowerCase();
  const models = arr(provider.updateRequiredModels).filter(model => !query || str(model.name).toLocaleLowerCase().includes(query) || str(model.slug).toLocaleLowerCase().includes(query));
  if (!models.length) return null;
  const version = models.map(model => str(model.minVersion)).reduce((highest, value) => compareSemver(value, highest) > 0 ? value : highest);
  const names = models.map(model => str(model.name));
  const list = names.length <= 2 ? names.join(' and ') : `${names.slice(0, -1).join(', ')}, and ${names.at(-1)}`;
  const runtime = provider.driver === 'codex' ? 'the Codex CLI' : provider.driver === 'claudeAgent' ? 'Claude Code' : providerDisplayName(str(provider.driver));
  return `Update ${runtime} to ${version.startsWith('v') ? version : `v${version}`} or newer to use ${list}.`;
}
