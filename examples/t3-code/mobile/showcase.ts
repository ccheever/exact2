// @ref llp/1107.002-design-system-parity.spec.md#source-authority
// Fixture values: T3 Code 365aa87982 showcaseEnvironmentRows.ts (MIT, ../LICENSE-T3).
// This module never selects showcase mode. The native explicit test launch flag owns it.
const displayURLs: Readonly<Record<string, string>> = {
  'Moonbase Terminal': 'https://moonbase.tail9f3a.ts.net/',
  'Suspense Station': 'https://suspense-vps.hel1.t3.sh/',
  'Kernel Cabin': 'http://100.82.16.5:3773/',
};
export function showcaseDisplayURL(enabled: boolean, label: string, actual: string): string {
  return enabled === true ? displayURLs[label] ?? actual : actual;
}
export function showcaseSubmittedURL(enabled: boolean, actual: string, presented: string, submitted: string): string {
  return enabled === true && submitted === presented ? actual : submitted;
}
export function showcaseCloudEnvironments(enabled: boolean) {
  const endpoint = { httpBaseUrl: 'https://pocket-pi.t3.sh', wsBaseUrl: 'wss://pocket-pi.t3.sh', providerKind: 't3_relay' };
  return {
    showcase: enabled === true,
    connected: enabled === true ? [{ environmentId: 'showcase-aurora-gpu', environmentLabel: 'Aurora GPU Pod',
      displayUrl: 'https://aurora-gpu.t3.sh', isRelayManaged: true, isEnabled: true, connectionState: 'connected',
      connectionError: null, connectionErrorTraceId: null }] : [],
    available: enabled === true ? [{ environment: { environmentId: 'showcase-pocket-pi', label: 'Pocket Pi', endpoint,
      linkedAt: '2026-07-16T08:00:00.000Z' }, availability: 'online', status: { environmentId: 'showcase-pocket-pi', endpoint,
      status: 'online', checkedAt: '2026-07-16T08:41:00.000Z' }, error: null, traceId: null }] : [],
  };
}
