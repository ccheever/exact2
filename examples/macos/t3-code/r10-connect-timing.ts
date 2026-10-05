// Lane r10-connect: wall-clock waits for a command in flight. A command's own
// native requests are in flight, so their replies arrive; a detached promise's
// may not. `settle` waits inside the command (the native module's sleep), and
// `wakeShell` asks the shell resource to read again now (R10ConnectWake.swift
// emits `t3.notify`, which shellView watches), so a state a command set before
// waiting is drawn while it waits. Without a native module (unit tests) neither
// waits.
import type { Native } from './protocol';

/** Resolves after `ms` of wall time (useDebouncedValue's wait); at once without a module. */
export async function settle(native: Native, ms: number): Promise<void> {
  try { await native.later({ op: 'timelineSleep', ms }); } catch { /* no module: no wait */ }
}

/** The shell resource reads again now (its answer shows the command's state so far). */
export async function wakeShell(native: Native): Promise<void> {
  try { await native.later({ op: 'r10Wake', topic: 't3.notify' }); } catch { /* no module: nothing to wake */ }
}
