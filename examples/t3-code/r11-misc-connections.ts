// Load balancing and GitHub sharing's machine list (ConnectionsSettings.tsx
// loadBalancingEnvironments, f870c419fc): the primary environment leads and is
// always counted, then every switched-on saved environment; the sections show
// once that list holds two machines. The reference's primary is the server it
// is served from (the desktop's bundled backend: "This machine"), which has no
// switch. This client bundles no server, so the saved environment that runs on
// this Mac (the first loopback origin) plays that part: it counts even while
// switched off, and its row reads "This machine" (environmentTransportLabel).
import { isLoopback } from './settings-b-fleet';

export type BalanceSource = { key: string; origin: string; enabled: boolean };

/** [primary, ...switched-on others], each with whether it is the primary. */
export function balanceSources<T extends BalanceSource>(listed: T[]): { source: T; primary: boolean }[] {
  const primary = listed.find(source => isLoopback(source.origin));
  return [
    ...(primary ? [{ source: primary, primary: true }] : []),
    ...listed.filter(source => source !== primary && source.enabled).map(source => ({ source, primary: false })),
  ];
}

/** environmentTransportLabel for a balancing row: the primary is "This machine". */
export const balanceSubtitle = (primary: boolean, transport: string) => primary ? 'This machine' : transport;
