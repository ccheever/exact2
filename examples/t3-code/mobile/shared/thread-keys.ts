// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/thread-keys.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// The queue and host key commands (task thread-commands-and-keys, G14/A14),
// adapted from T3 Code 1e2ecbd975 (MIT); see LICENSE-T3. Sources:
// apps/web/src/components/ChatView.tsx (the window keydown handler:
// thread.steerQueuedMessage, thread.editQueuedMessage, composer.host,
// composer.cycleHost), chat/QueuedRunsControl.tsx (steerNext, editLatest) and
// chat/ChatComposer.tsx (openControl, isCaretAtStart).
// Changes: each command is a dispatch row (keyboard-dispatch.ts), present only
// when the reference would consume the key, so an absent row leaves the key to
// the focused view. ⌥↑ is no menu chord: its row is a composer key button the
// native composer presses only with the caret at the very start
// (T3ComposerQueueKey.swift); anywhere else the text view moves the caret. Key
// repeat is consumed and ignored by the host for every dispatch button.
import { str } from './domain';
import type { T3Client } from './client';
import type { DispatchAdd, DispatchContext } from './keyboard-dispatch';
import type { Obj } from './domain';
import { queueState, queuedEdit } from './composer-controls-queue';
import { environmentOptions } from './r4-git-env';
import { fleet, type EnvironmentFleet } from './settings-b-fleet';

/** steerNext and editLatest: the first queued message as a steer, the last one into the composer. Not on a draft. */
export function queueKeyRows(add: DispatchAdd, client: T3Client, context: DispatchContext): void {
  if (context.draftThreadRoute || !client.threadId) return;
  if (!client.projection) return;
  const state = queueState(client.projection);
  const first = state.queued[0], last = state.queued[state.queued.length - 1];
  // workflow.canPromoteToSteer: a running turn whose provider steers (queueState canSteer).
  if (first && state.canSteer) add('thread.steerQueuedMessage', 'command', 'cc:queued-steer', 'Steer First Queued Message', str(first.run.id));
  // Declines while a queued message is already being edited, so the key keeps moving the caret in that draft.
  if (last && !queuedEdit(client)) add('thread.editQueuedMessage', 'composer-key', 'cclocal:queued-edit', 'Edit Last Queued Message', str(last.run.id));
}

/**
 * composer.host opens the strip's "Run on" control; composer.cycleHost (no
 * default chord) moves the draft to the next machine, wrapping. Both only on a
 * draft that can move machines (envLocked is a started thread).
 */
export function hostKeyRows(add: DispatchAdd, client: T3Client, source: EnvironmentFleet = fleet): void {
  if (client.threadId || !client.projectId) return;
  const options = environmentOptions(client, source);
  if (options.length < 2) return;
  add('composer.host', 'options', 'workspace', 'Run On');
  const at = options.findIndex(option => option.selected);
  const next = options[(at + 1) % options.length];
  if (next && !next.selected) add('composer.cycleHost', 'command-value', 'environment-run-on', 'Cycle Host', next.id);
}

/** The main window's row for this task's commands (keyboard-dispatch.ts MAIN_ROWS). */
export function threadCommandRows(add: DispatchAdd, client: T3Client, threads: Obj[], browseProvider: string, modelQuery: string, context: DispatchContext): void {
  void threads; void browseProvider; void modelQuery;
  queueKeyRows(add, client, context);
  hostKeyRows(add, client);
}
