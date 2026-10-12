// Load balancing and GitHub sharing's machine list (ConnectionsSettings.tsx
// loadBalancingEnvironments, f870c419fc): the primary environment leads and is
// always counted, then every switched-on saved environment; the sections show
// once that list holds two machines. The primary is the embedded server ("This
// machine", local-primary.ts), which has no switch of its own there; its row reads
// "This machine" (environmentTransportLabel). With the Local environment switched
// off there is no primary, and only switched-on saved environments count.
export type BalanceSource = { key: string; origin: string; enabled: boolean; primary?: boolean };

/** [primary, ...switched-on others], each with whether it is the primary. */
export function balanceSources<T extends BalanceSource>(listed: T[]): { source: T; primary: boolean }[] {
  const primary = listed.find(source => source.primary === true);
  return [
    ...(primary ? [{ source: primary, primary: true }] : []),
    ...listed.filter(source => source !== primary && source.enabled).map(source => ({ source, primary: false })),
  ];
}

/** environmentTransportLabel for a balancing row: the primary is "This machine". */
export const balanceSubtitle = (primary: boolean, transport: string) => primary ? 'This machine' : transport;
