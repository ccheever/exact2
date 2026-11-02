// The client operations 0.1.5 and 0.1.6 servers add (T3 Code 2359643cff, MIT, see LICENSE-T3: packages/contracts/src/rpc.ts,
// preview.ts, server.ts, project.ts, pullRequest.ts, orchestrationV2.ts; apps/web/src/connection/clientMetadata.ts). Each sends one request
// on the focused connection; T3Client's methods of the same names call these. A refusal is the transport's typed
// error (transport-wire.ts), whose message is the reference's sentence.
import type { T3Client } from './client';
import { obj, str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { CLIENT_VERSION } from '../connections/version-skew';

/** clientPresentationMetadata for the desktop app: how the server labels the session it starts for this client. */
export const clientPresentation = (): Obj => ({ label: 'T3 Code Desktop', deviceType: 'desktop', os: 'macOS', surface: 'desktop', appVersion: CLIENT_VERSION });

const port = (value: number) => Number.isInteger(value) && value > 0 && value < 65_536;

/** preview.stopLocalServer: stops a discovered dev server, named by its process and port (PreviewStopLocalServerInput). */
export async function stopLocalServer(client: T3Client, native: Native, input: { pid: number; port: number }): Promise<void> {
  if (!Number.isInteger(input.pid) || input.pid <= 0 || !port(input.port)) throw new ClientError('That server is not a running local server.');
  await client.request(native, 'preview.stopLocalServer', { pid: input.pid, port: input.port }, client.generation, true);
}

/** server.prepareTriage: the server writes the triage session's files and answers the composer text that links them. */
export async function prepareTriage(client: T3Client, native: Native): Promise<string> {
  return str((await client.request(native, 'server.prepareTriage', { client: clientPresentation() }, client.generation, true)).prompt);
}

/** projects.createFromThread: makes a project of a thread's workspace and answers its id. */
export async function createProjectFromThread(client: T3Client, native: Native, threadId: string): Promise<string> {
  if (!threadId) throw new ClientError('Choose a thread.');
  const projectId = str((await client.request(native, 'projects.createFromThread', { threadId }, client.generation, true)).projectId);
  if (!projectId) throw new ClientError('The server did not name the new project.');
  return projectId;
}

/** OrchestrationV2ForceStopThreadResult: how the thread's agent was stopped. */
export interface ForceStopResult { outcome: 'killed' | 'released' | 'releasing' | 'shared'; sharedWithThreadIds: string[] }
const FORCE_STOP_OUTCOMES: readonly ForceStopResult['outcome'][] = ['killed', 'released', 'releasing', 'shared'];

/**
 * orchestration.forceStopThread (0.1.6, capability `threadForceStop`): kills the agent of a turn that ignored Stop. `runId`
 * names the run the user saw ignore it; the server refuses once a newer run has started (`superseded`).
 */
export async function forceStopThread(client: T3Client, native: Native, threadId: string, runId?: string): Promise<ForceStopResult> {
  if (!threadId) throw new ClientError('Choose a thread.');
  const result = obj(await client.request(native, 'orchestration.forceStopThread', runId ? { threadId, runId } : { threadId }, client.generation, true));
  const outcome = FORCE_STOP_OUTCOMES.find(value => value === result.outcome);
  if (!outcome) throw new ClientError('The server did not say how the agent was stopped.');
  return { outcome, sharedWithThreadIds: Array.isArray(result.sharedWithThreadIds) ? result.sharedWithThreadIds.filter((id): id is string => typeof id === 'string') : [] };
}

/** pullRequests.update with maintainerCanModify alone: GitHub's "Allow edits by maintainers" (PullRequestActionsMenu). */
export async function setMaintainerEdits(client: T3Client, native: Native, reference: Obj, allowed: boolean): Promise<void> {
  await client.rpc(native, 'pullRequests.update', { ...reference, maintainerCanModify: allowed }, true);
}
