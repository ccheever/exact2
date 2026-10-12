// Orchestration protocol compatibility (lane r3-protocol; MIT reference f90b77d,
// commit 22e9d35: client-runtime connection/compatibility.ts). The message names
// the direction: an older server is updated on its host, a newer one needs a newer
// client. T3Transport.swift refuses both with the same text; the outdated-host
// update itself (bare socket, self-update RPCs, descriptor poll) is T3Fleet's
// fleetOutdated* operations and settings-b-outdated.ts.
import { obj, str, type Obj } from './domain';

export const ORCHESTRATION_PROTOCOL_VERSION = 2;

/** orchestrationProtocolCompatibilityError: null when compatible; a server without the field speaks protocol 1. */
export function compatibilityProblem(descriptor: Obj): { message: string; serverUpdateRequired: boolean } | null {
  const version = typeof descriptor.orchestrationProtocolVersion === 'number' ? descriptor.orchestrationProtocolVersion : 1;
  if (version === ORCHESTRATION_PROTOCOL_VERSION) return null;
  const label = str(descriptor.label, 'this server');
  if (version > ORCHESTRATION_PROTOCOL_VERSION) {
    return { message: `This client is not supported by this server. Update your app or use a compatible release to connect to ${label}.`, serverUpdateRequired: false };
  }
  return { message: `This client requires a newer server. Update T3 Code on ${label} to connect.`, serverUpdateRequired: canSelfUpdate(descriptor) };
}

/** Whether this client can drive the host's update remotely. */
export function canSelfUpdate(descriptor: Obj): boolean {
  const capabilities = obj(descriptor.capabilities), method = capabilities.serverSelfUpdate;
  return typeof method === 'string' && (method !== 'desktop-managed' || capabilities.desktopAppUpdate === true);
}
