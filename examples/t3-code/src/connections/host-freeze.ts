// Why a host stopped answering (T3 Code 74541bab17, MIT, see LICENSE-T3:
// packages/client-runtime/src/connection/hostFreeze.ts and
// apps/web/src/components/HostFreezeToastCoordinator.tsx). A server whose event loop stalled for 5 s or
// more reports the stall in its config for five minutes (`lastEventLoopStall`), so a client that
// reconnects after the drop can say why, once per stall, as a warning toast. The reference reads every
// environment's server config; here that is the focused connection's and each background environment's
// (settings-b-fleet.ts `freezeSources`). The clone keeps no server config across launches, so a stall
// from an earlier run cannot read as a new freeze.
import type { T3Client } from '../client/client';
import { obj, str, type Obj } from '../client/domain';
import { pushToast } from '../shell/toast';

/** ServerEventLoopStall (packages/contracts/src/server.ts). `loadAverage` is absent on Windows hosts. */
export type ServerEventLoopStall = { detectedAt: string; durationMs: number; loadAverage?: number; cpuCount: number };
export interface HostFreezeNotice { readonly title: string; readonly description: string }

/**
 * Says why a host stopped answering, from the recent stall it reports on connect: an overloaded CPU
 * when the load was above the core count, else T3 Code itself.
 */
export function hostFreezeNotice(hostLabel: string, stall: ServerEventLoopStall | undefined): HostFreezeNotice | null {
  if (stall === undefined) return null;
  const { loadAverage, cpuCount } = stall;
  return {
    title: `${hostLabel} froze for ${Math.round(stall.durationMs / 1000)}s`,
    description: loadAverage !== undefined && loadAverage > cpuCount
      ? `Its CPU is overloaded (load ${Math.round(loadAverage)} on ${cpuCount} cores).`
      : `T3 Code on ${hostLabel} was too busy to answer.`,
  };
}

/** The config's `lastEventLoopStall` when it decodes as ServerEventLoopStall; anything else is ignored. */
export function eventLoopStall(config: Obj): ServerEventLoopStall | undefined {
  const raw = config.lastEventLoopStall;
  if (typeof raw !== 'object' || raw === null || Array.isArray(raw)) return undefined;
  const { detectedAt, durationMs, cpuCount, loadAverage } = raw as Obj;
  if (typeof detectedAt !== 'string' || !detectedAt) return undefined;
  if (typeof durationMs !== 'number' || !Number.isInteger(durationMs) || durationMs < 0) return undefined;
  if (typeof cpuCount !== 'number' || !Number.isInteger(cpuCount) || cpuCount <= 0) return undefined;
  const load = typeof loadAverage === 'number' && Number.isFinite(loadAverage) ? { loadAverage } : {};
  return { detectedAt, durationMs, ...load, cpuCount };
}

/** One environment's server config and the label the catalog gives it ('' falls back to the config's). */
export type FreezeSource = { environmentId: string; label: string; config: Obj };

const shown = new WeakMap<object, Set<string>>();

/** HostFreezeToastCoordinator: one warning per environment and stall, for the life of the window's client. */
export function announceHostFreezes(client: T3Client, sources: Iterable<FreezeSource>): void {
  let seen = shown.get(client);
  if (!seen) { seen = new Set(); shown.set(client, seen); }
  for (const source of sources) {
    const stall = eventLoopStall(source.config);
    if (stall === undefined) continue;
    const key = `${source.environmentId}:${stall.detectedAt}`;
    if (seen.has(key)) continue;
    seen.add(key);
    const label = source.label || str(obj(source.config.environment).label) || source.environmentId;
    const notice = hostFreezeNotice(label, stall);
    if (notice !== null) pushToast(client, { kind: 'warning', ...notice });
  }
}
