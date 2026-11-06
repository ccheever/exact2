// Attached threads and the machines that can read them, adapted from T3 Code 74541bab17 (MIT; see LICENSE-T3):
// packages/shared/src/composerContextReferences.ts (threadContextsOutsideEnvironment), ChatView.tsx (onSend's
// stranded-thread toast and the load-balancing candidate filter) and ThreadContextChip.tsx (the unreadable chip).
// A thread read is environment-local even when a draft moves between machines. The reference keeps each chip's
// environment in its draft record; here a chip's machine is the one whose shell holds its thread: this machine's
// first, then a background machine's.
import type { T3Client } from '../client/client';
import { obj, str } from '../client/domain';
import { fleet, type EnvironmentFleet } from '../connections/settings-b-fleet';
import { isPrimaryEnvironment } from '../connections/local-primary';
import { machineKind } from '../connections/connections';
import { environmentIndicator } from '../details/shell-details';
import { contextId, contextLabel, contextReferences } from './composer-editor-menu';

export type AttachedThread = { contextId: string; threadId: string; environmentId: string; title: string };

export const UNREADABLE_THREAD_TIP = "On another machine, so this machine's agent can't read it. Remove it or switch back to send.";

/** Each thread chip in `text` whose thread a known machine holds, with that machine. */
export function attachedThreads(client: T3Client, text: string, source: EnvironmentFleet = fleet): AttachedThread[] {
  const shells = [{ environmentId: client.environmentId, threads: client.shell.threads },
    ...[...source.entries.values()].filter(entry => entry.environmentId !== client.environmentId).map(entry => ({ environmentId: entry.environmentId, threads: entry.shell.threads }))];
  return contextReferences(text).flatMap(reference => {
    if (reference.kind !== 'thread') return [];
    for (const shell of shells) {
      const thread = shell.threads.find(entry => contextId('thread', str(entry.id)) === reference.id);
      if (thread) return [{ contextId: reference.id, threadId: str(thread.id), environmentId: shell.environmentId, title: contextLabel(reference.label, 'thread') }];
    }
    return [];
  });
}

/** threadContextsOutsideEnvironment: the attached threads this machine's agent cannot read. */
export function unreadableThreads(client: T3Client, text: string, source: EnvironmentFleet = fleet): AttachedThread[] {
  return attachedThreads(client, text, source).filter(thread => thread.environmentId !== client.environmentId);
}

/** Whether a machine can read every attached thread (the load-balancing candidate filter). */
export const readsEveryThread = (threads: readonly AttachedThread[], environmentId: string): boolean =>
  threads.every(thread => thread.environmentId === environmentId);

/** environmentById's label for a background machine, else "its machine". */
export function machineLabel(environmentId: string, source: EnvironmentFleet = fleet): string {
  const entry = [...source.entries.values()].find(candidate => candidate.environmentId === environmentId);
  if (!entry) return 'its machine';
  return environmentIndicator({ isPrimary: isPrimaryEnvironment(environmentId), available: 2, environmentId,
    runtimeLabel: str(obj(entry.config.environment).label), savedLabel: '', machine: machineKind(entry.config) }).envLabel;
}
