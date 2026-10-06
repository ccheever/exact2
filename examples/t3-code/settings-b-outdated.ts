// Settings → Connections: hosts too old for this client (lane r3-settings; upstream
// 22e9d35613 compatibility.ts, onboarding.ts, outdatedHostUpdate.ts,
// ServerUpdateAction.tsx OutdatedServerUpdateAction and ConnectionsSettings.tsx).
// A host whose orchestration protocol is older than this client's cannot open a
// session. One that can update itself is still paired (saved switched off) and
// its row offers Update (Retry update after a failure): T3Fleet.swift
// T3OutdatedHosts opens a bare socket without the protocol gate, runs the
// self-update RPCs, waits for the host to answer protocol 2 and switches it back
// on. Progress lands in the row as "Downloading…" then "Restarting…".
import { obj, str, type Obj } from './domain';
import { bridgeReply, ClientError, type Native } from './protocol';
import { pushToast } from './toast';
import type { T3Client } from './client';
import { withStandardScope } from './remote-scopes';

export const ORCHESTRATION_PROTOCOL_VERSION = 2;
export type OutdatedJob = { status: string; stage: string; fromVersion: string; targetVersion: string; message: string; resultVersion: string; label: string };
export type Compatibility = { message: string; serverUpdateRequired: boolean };

/** compatibility.ts canSelfUpdate. */
export function canSelfUpdate(descriptor: Obj): boolean {
  const capabilities = obj(descriptor.capabilities), selfUpdate = str(capabilities.serverSelfUpdate);
  return selfUpdate !== '' && (selfUpdate !== 'desktop-managed' || capabilities.desktopAppUpdate === true);
}
/** orchestrationProtocolCompatibilityError's detail by direction, null when compatible (a missing version is 1). */
export function compatibility(descriptor: Obj, label: string): Compatibility | null {
  const version = typeof descriptor.orchestrationProtocolVersion === 'number' ? descriptor.orchestrationProtocolVersion : 1;
  if (version === ORCHESTRATION_PROTOCOL_VERSION) return null;
  if (version > ORCHESTRATION_PROTOCOL_VERSION) return { message: `This client is not supported by this server. Update your app or use a compatible release to connect to ${label}.`, serverUpdateRequired: false };
  return { message: `This client requires a newer server. Update T3 Code on ${label} to connect.`, serverUpdateRequired: canSelfUpdate(descriptor) };
}
/** UPDATE_STAGE_LABELS: the sub-second launcher handoff folds into the download. */
export const stageLabel = (stage: string) => stage === 'resuming' ? 'Restarting…' : 'Downloading…';

// ── Facts for the Connections page ────────────────────────────────────────
// Data sources have no clock: each opening of the page probes again (forgetProbes), then reuses the answer.
const probes = new Map<string, { descriptor: Obj | null }>();
let jobs: Record<string, OutdatedJob> = {};
/** Jobs live in this process's T3Fleet only: until one starts (or a read finds one), the fleet pass never asks. */
let jobsMayExist = false;

/** The page closed: its next opening probes every blocked environment again. */
export function forgetProbes(): void { probes.clear(); }
/** The unauthenticated descriptors of switched-off or unsupported environments, once per opening of the page. */
export async function probeDescriptors(native: Native, keys: string[]): Promise<Map<string, Obj>> {
  await Promise.all(keys.map(async key => {
    if (probes.has(key)) return;
    probes.set(key, { descriptor: null });
    try {
      const reply = await bridgeReply(native, { op: 'fleetOutdatedProbe', fleet: key });
      probes.set(key, { descriptor: reply.ok ? obj(reply.value) : null });
    } catch { probes.set(key, { descriptor: null }); }
  }));
  const out = new Map<string, Obj>();
  for (const key of keys) { const descriptor = probes.get(key)?.descriptor; if (descriptor) out.set(key, descriptor); }
  return out;
}
export const probedDescriptor = (key: string): Obj | undefined => probes.get(key)?.descriptor ?? undefined;
const decodeJob = (value: unknown): OutdatedJob => {
  const job = obj(value);
  return { status: str(job.status), stage: str(job.stage), fromVersion: str(job.fromVersion), targetVersion: str(job.targetVersion), message: str(job.message), resultVersion: str(job.resultVersion), label: str(job.label) };
};
/** Every outdated-host update job T3Fleet holds, by environment key. */
export async function readJobs(native: Native): Promise<Record<string, OutdatedJob>> {
  try {
    const reply = await bridgeReply(native, { op: 'fleetOutdatedJobs', fleet: '\n' });
    if (reply.ok) jobs = Object.fromEntries(Object.entries(obj(obj(reply.value).jobs)).map(([key, job]) => [key.replace(/\/+(?=\n)/, ''), decodeJob(job)]));
    jobsMayExist = Object.keys(jobs).length > 0;
  } catch { /* keep the last answer */ }
  return jobs;
}
export const jobFor = (key: string): OutdatedJob | undefined => jobs[key];
/** Test seam: replace the jobs the page reads. */
export function setJobs(next: Record<string, OutdatedJob>): void { jobs = next; jobsMayExist = Object.keys(next).length > 0; }

/** One saved row's outdated-host facts: whether it is blocked, its Update label and progress line. */
export function outdatedRow(key: string, label: string, descriptor: Obj | undefined, job: OutdatedJob | undefined) {
  const blocked = descriptor ? compatibility(descriptor, label) : null;
  const running = job?.status === 'running';
  return {
    blocked, running,
    resuming: running && job?.stage === 'resuming',
    action: blocked?.serverUpdateRequired && !running ? (job?.status === 'failed' ? 'Retry update' : 'Update') : '',
    progress: running ? stageLabel(job!.stage) : '',
    failure: job?.status === 'failed' ? job.message : '',
    fromVersion: str(descriptor?.serverVersion),
  };
}

/** Starts the single-flight update; T3Fleet answers at once and reports through "t3.fleet". */
export async function startOutdatedUpdate(native: Native, key: string, label: string, fromVersion: string, targetVersion: string): Promise<boolean> {
  const reply = await bridgeReply(native, { op: 'fleetOutdatedUpdate', fleet: key, label, fromVersion, targetVersion });
  if (!reply.ok) throw new Error(reply.error!.message);
  jobsMayExist = true;
  probes.delete(key);
  return obj(reply.value).started === true;
}

/** preparePairingRegistration for an outdated host: saved switched off when it can update itself. */
export async function pairOutdated(native: Native, origin: string, credential: string): Promise<Obj> {
  const reply = await bridgeReply(native, { op: 'fleetOutdatedPair', fleet: `${origin.replace(/\/+$/, '')}\n`, ...withStandardScope({ credential }) });
  if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
  return obj(reply.value);
}

const announced = new Set<string>();
/**
 * OutdatedServerUpdateAction's toasts, once per finished job: success acknowledges
 * the job; a failure stays (the row reads "Retry update") until the next attempt.
 */
export async function announceJobs(native: Native, client: T3Client | null): Promise<void> {
  if (!jobsMayExist) return;
  const current = await readJobs(native);
  for (const [key, job] of Object.entries(current)) {
    const signature = `${key}\n${job.status}\n${job.message}\n${job.resultVersion}`;
    if (job.status === 'running') { for (const seen of [...announced]) if (seen.startsWith(`${key}\n`)) announced.delete(seen); continue; }
    if (announced.has(signature)) continue;
    announced.add(signature);
    const server = `${job.label || 'Environment'} server`;
    if (job.status === 'done') {
      if (client) pushToast(client, { kind: 'success', title: `${server} updated`, description: `Reconnected on t3@${job.resultVersion || job.targetVersion}.` });
      await native.later({ op: 'fleetOutdatedAck', fleet: key }).catch(() => {});
    } else if (job.status === 'failed' && client) {
      pushToast(client, { kind: 'error', title: 'Server update failed', description: job.message || 'Server update failed.', stacked: true });
    }
  }
}
