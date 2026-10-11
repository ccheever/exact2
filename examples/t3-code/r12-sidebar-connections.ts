// Lane r12-sidebar: Settings → Connections' Environments list (ConnectionsSettings.tsx
// savedEnvironments, f870c419fc; MIT, see LICENSE-T3). The reference's primary
// environment ("This machine", the embedded server: local-primary.ts) never has a row
// there; it has its own section (this-machine.ts). Every saved environment keeps its
// row and switch, switched off or not.
import type { BalanceSource } from './r11-misc-connections';

/** The Environments rows: every listed environment but the primary. */
export function environmentRows<T extends BalanceSource>(listed: T[]): T[] {
  return listed.filter(source => source.primary !== true);
}
