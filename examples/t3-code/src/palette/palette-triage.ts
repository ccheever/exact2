// Adapted from T3 Code 6d450a4284 (MIT); see LICENSE-T3.
// ⌘K's Triage an issue (CommandPalette.tsx, hooks/useStartTriage.ts, lib/triageTarget.ts): in-app `t3 triage`. The
// server writes the session's files (server.prepareTriage), then a No project draft opens holding its prompt, which
// links the playbook; the draft hero explains the rest (pages-hero.contract TriageHero). A failure says to run the
// command in a terminal on that machine, with a button that copies it.
import type { T3Client } from '../client/client';
import { obj, str, type Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { can } from '../connections/command-permissions';
import { primaryEnvironmentId } from '../connections/local-primary';
import { fleet, type EnvironmentFleet } from '../connections/settings-b-fleet';
import { openScratchProject } from '../composer/scratch-project';
import { sleep, wakeShell } from '../client/command-waits';
import { letGo } from '../client/let-go';
import { TRIAGE_CLI_COMMAND, scratchRoot } from '../pages/pages-home';
import { dismissToast, pushToast } from '../shell/toast';

/**
 * pickTriageEnvironment: the machine triage runs on is the one on screen, else the primary if it supports triage, else
 * the first connected machine that does. Without No project threads the action still shows and explains the terminal
 * fallback, so this does not check for them.
 */
export function pickTriageEnvironment<Id extends string>(input: { onScreen: Id | null; primary: Id | null; connected: readonly Id[]; supportsTriage: (environmentId: Id) => boolean }): Id | null {
  if (input.onScreen !== null) return input.onScreen;
  if (input.primary !== null && input.supportsTriage(input.primary)) return input.primary;
  return input.connected.find(input.supportsTriage) ?? null;
}

const capabilities = (config: Obj) => obj(obj(config.environment).capabilities);
/**
 * The environment ⌘K offers Triage an issue on, or null when it offers none: the picked machine must report `triage`
 * and allow `server.prepareTriage` (orchestration:operate). The palette starts a draft on the environment this window
 * shows, so a pick elsewhere offers none.
 */
export function triageEnvironment(client: T3Client, source: EnvironmentFleet = fleet): string | null {
  const entries = [...source.entries.values()].filter(entry => entry.phase === 'connected');
  const configOf = (environmentId: string): Obj | null => environmentId === client.environmentId ? (client.ready ? client.config : null)
    : obj(entries.find(entry => entry.environmentId === environmentId)?.config);
  const supportsTriage = (environmentId: string) => capabilities(configOf(environmentId) ?? {}).triage === true;
  const picked = pickTriageEnvironment({
    onScreen: client.ready && (client.threadId || client.projectId) ? client.environmentId : null,
    primary: primaryEnvironmentId(),
    connected: [...(client.ready ? [client.environmentId] : []), ...entries.map(entry => entry.environmentId)],
    supportsTriage,
  });
  return picked !== null && picked === client.environmentId && supportsTriage(picked) && can(client, 'orchestration:operate') ? picked : null;
}

/** useStartTriage's fail: with no draft to show, a toast is the only place to point at the CLI. */
function triageFailed(client: T3Client, description: string): void {
  pushToast(client, { kind: 'error', title: 'Could not start triage', description: `${description} Run ${TRIAGE_CLI_COMMAND} in a terminal on that machine instead.`,
    timeoutMs: 0, stacked: true, hideCopy: true, action: { label: 'Copy command', op: 'pageslocal:copy', id: '', value: TRIAGE_CLI_COMMAND } });
}

/**
 * useStartTriage: the server prepares the session, the machine's No project folder opens, and its new draft takes the
 * prompt unless the person has already written there. A slow start shows "Starting triage…" after 400 ms. Returns the
 * project the draft opens in, or '' when it failed.
 */
export async function startTriage(client: T3Client, native: Native): Promise<string> {
  if (!scratchRoot(client.config)) {
    triageFailed(client, 'This machine cannot start threads without a project.');
    return '';
  }
  let finished = false, pending = 0;
  void sleep(400).then(async () => {
    if (finished) return;
    pending = pushToast(client, { kind: 'loading', title: 'Starting triage…', timeoutMs: 0 });
    await wakeShell(native).catch(() => undefined);
  });
  try {
    const prompt = await client.prepareTriage(native);
    const projectId = await openScratchProject(client, native);
    const key = `${client.environmentId}:new:${projectId}`;
    if (!str(client.local.drafts[key]).trim()) client.local.drafts[key] = prompt;
    return projectId;
  } catch (error) {
    if (letGo(error)) throw error;
    triageFailed(client, error instanceof Error ? error.message : 'An error occurred.');
    return '';
  } finally {
    finished = true;
    if (pending) dismissToast(client, pending);
  }
}
