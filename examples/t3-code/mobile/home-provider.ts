// @ref llp/1109.004-home-projection.decision.md#decision
// T3 365aa87982 thread-provider-instance, threadListV2 and providerInstanceDisplay.
import { arr, obj, str, type Obj } from './shared/domain';
import { driverMeta } from './shared/providers-meta';
import { mobileProviderIconURL } from './environment-detail';

export interface HomeProvider {
  visible: boolean; driver: string; iconUrl: string;
  previous: { key: string; driver: string }[];
  showBadge: boolean; badge: string; badgeColor: string;
}
export const blankHomeProvider = (): HomeProvider => ({ visible: false, driver: '', iconUrl: '', previous: [], showBadge: false, badge: '', badgeColor: '' });
const humanize = (slug: string): string => slug.replace(/([a-z])([A-Z])/g, '$1 $2').replace(/[_-]+/g, ' ').trim().replace(/\b\w/g, char => char.toUpperCase());
function displayName(provider: Obj): string {
  const driver = str(provider.driver), label = driverMeta(driver)?.label ?? humanize(driver), supplied = str(provider.displayName).trim();
  if (supplied && supplied !== label) return supplied;
  if (provider.instanceId !== driver) {
    const custom = humanize(str(provider.instanceId));
    if (custom) return custom;
  }
  return supplied || label;
}
function initials(label: string): string {
  const words = label.replace(/[_-]+/g, ' ').split(/\s+/u).filter(Boolean);
  return (words.length === 1 ? Array.from(words[0]!).slice(0, 2).join('')
    : words.slice(0, 2).map(word => Array.from(word)[0] ?? '').join('')).toUpperCase();
}

/** Resolve solely against the row's environment. A missing current owner hides its history too. */
export function mobileHomeProvider(thread: Obj, config: Obj): HomeProvider {
  const providers = arr(config.providers), current = obj(thread.runtime).providerInstanceId ?? obj(thread.modelSelection).instanceId;
  const provider = providers.find(candidate => candidate.instanceId === current);
  if (!provider) return blankHomeProvider();
  const driver = str(provider.driver), accent = str(provider.accentColor).trim();
  const badgeColor = /^#[0-9a-fA-F]{6}$/u.test(accent) ? accent : '';
  // Source mobile passes driver-only entries, including for ACP registry agents.
  const showBadge = !!badgeColor || providers.filter(candidate => candidate.driver === driver).length > 1;
  const history = Array.isArray(thread.providerInstanceHistory) ? thread.providerInstanceHistory : [];
  const previous = history.filter(id => id !== current).slice(-2).flatMap((id, index) => {
    const prior = providers.find(candidate => candidate.instanceId === id);
    return prior ? [{ key: `${index}:${str(id)}`, driver: str(prior.driver) }] : [];
  });
  return { visible: true, driver, iconUrl: mobileProviderIconURL(provider.iconUrl), previous,
    showBadge, badge: initials(displayName(provider)), badgeColor };
}
