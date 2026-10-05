// Lane r12-sidebar: Settings → Connections' Environments list (ConnectionsSettings.tsx
// savedEnvironments, f870c419fc; MIT, see LICENSE-T3). The reference's primary
// environment ("This machine", the server it is served from) never has a row there.
// This client's stand-in for it is the first loopback saved environment
// (r11-misc-connections.ts balanceSources), which keeps its row and switch so it
// can be switched off and on. Once it is switched off and another listed machine is
// switched on, the page reads as the reference's: Load balancing and GitHub sharing
// still count it as "This machine", and the Environments list leaves it out. With
// nothing else switched on it stays listed, so it can be switched back on.
import { isLoopback } from './settings-b-fleet';
import type { BalanceSource } from './r11-misc-connections';

/** The Environments rows: every listed environment but a switched-off loopback primary while another is switched on. */
export function environmentRows<T extends BalanceSource>(listed: T[]): T[] {
  const primary = listed.find(source => isLoopback(source.origin));
  if (!primary || primary.enabled || !listed.some(source => source !== primary && source.enabled)) return listed;
  return listed.filter(source => source !== primary);
}
