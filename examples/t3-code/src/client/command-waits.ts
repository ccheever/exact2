// Waits for a command in flight. `sleep` waits on a timer of the answer that awaits it (LLP 1027.000 D1), on the app's
// clock, so under the agent `clock +N` moves it. Its answer owns it: one let go meanwhile never resumes, and a sleep
// begun after its answer was let go or gave its value is refused, which reads as that answer's let-go (let-go.ts), so
// the flow unwinds as it does at a let-go native call. `wakeShell` asks the shell resource to read again now
// (T3LatinChords.swift emits `t3.notify`, which shellView watches), so a state a command set before waiting is drawn
// while it waits; without a native module (unit tests) it wakes nothing.
import { ClientError, type Native } from './protocol';

/** Resolves after `ms` on the app's clock. */
export function sleep(ms: number): Promise<void> {
  return new Promise((resolve, reject) => {
    try { setTimeout(resolve, Math.max(0, ms)); }
    catch (error) { reject(new ClientError(error instanceof Error ? error.message : 'This operation was superseded.', 'superseded')); }
  });
}

/** The shell resource reads again now (its answer shows the command's state so far). */
export async function wakeShell(native: Native): Promise<void> {
  try { await native.later({ op: 'wake', topic: 't3.notify' }); } catch { /* no module: nothing to wake */ }
}
