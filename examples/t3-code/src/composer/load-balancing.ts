// Auto balance's choice of machine (T3 Code MIT, see LICENSE-T3; reference 1e2ecbd975:
// packages/client-runtime/src/load-balancing.ts chooseLoadBalancedEnvironment and
// packages/contracts/src/resourceTelemetry.ts HostResourcesSnapshot). Unchanged in logic;
// the clone's caller (auto-balance.ts) passes the snapshot's wall time as `now`.

/** Whole-host capacity, as `server.getHostResources` reports it. */
export type HostResourcesSnapshot = {
  readonly sampledAt: number;
  readonly cpuUtilization: number | null;
  readonly cpuCount: number;
  readonly availableMemoryBytes: number;
  readonly totalMemoryBytes: number;
};

export type LoadBalancingCandidate = {
  readonly environmentId: string;
  readonly resources: HostResourcesSnapshot | null;
  /** Client receipt time avoids comparing clocks on different machines. */
  readonly receivedAt?: number;
  readonly weight: number;
};

/** HostResourcesSnapshot's decoder: a reply that does not match is no sample. */
export function decodeHostResources(value: unknown): HostResourcesSnapshot | null {
  if (!value || typeof value !== 'object') return null;
  const raw = value as Record<string, unknown>;
  const count = (key: string) => typeof raw[key] === 'number' && Number.isInteger(raw[key]) && (raw[key] as number) >= 0 ? raw[key] as number : null;
  const cpu = raw.cpuUtilization;
  const sampledAt = count('sampledAt'), cpuCount = count('cpuCount'), available = count('availableMemoryBytes'), total = count('totalMemoryBytes');
  if (sampledAt === null || cpuCount === null || available === null || total === null) return null;
  if (cpu !== null && !(typeof cpu === 'number' && cpu >= 0 && cpu <= 1)) return null;
  return { sampledAt, cpuUtilization: cpu, cpuCount, availableMemoryBytes: available, totalMemoryBytes: total };
}

/** Callers supply only connected machines hosting the project and selected provider. */
export function chooseLoadBalancedEnvironment(candidates: ReadonlyArray<LoadBalancingCandidate>, now: number): string | null {
  let selected: string | null = null;
  let bestScore = 0;
  for (const { environmentId, resources, receivedAt, weight } of candidates) {
    const sampledAt = receivedAt ?? resources?.sampledAt ?? 0;
    if (
      !resources ||
      !Number.isFinite(weight) ||
      weight <= 0 ||
      now - sampledAt > 15_000 ||
      sampledAt > now + 5_000 ||
      resources.cpuUtilization === null ||
      resources.cpuUtilization >= 0.95 ||
      resources.totalMemoryBytes <= 0 ||
      resources.cpuCount <= 0
    ) {
      continue;
    }
    const memoryAvailable = resources.availableMemoryBytes / resources.totalMemoryBytes;
    if (memoryAvailable <= 0.05) continue;
    const score = weight * resources.cpuCount * (1 - resources.cpuUtilization) * memoryAvailable;
    if (score > bestScore) {
      selected = environmentId;
      bestScore = score;
    }
  }
  return selected;
}
