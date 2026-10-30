// The client operations a 0.1.5 server adds (T3 Code 6d450a4284, MIT, see LICENSE-T3: packages/contracts/src/rpc.ts,
// preview.ts, server.ts, project.ts, pullRequest.ts; apps/web/src/connection/clientMetadata.ts). Each sends one request
// on the focused connection; T3Client's methods of the same names call these. A refusal is the transport's typed
// error (transport-wire.ts), whose message is the reference's sentence.
import type { T3Client } from './client';
import { str, type Obj } from './domain';
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

/** pullRequests.update with maintainerCanModify alone: GitHub's "Allow edits by maintainers" (PullRequestActionsMenu). */
export async function setMaintainerEdits(client: T3Client, native: Native, reference: Obj, allowed: boolean): Promise<void> {
  await client.rpc(native, 'pullRequests.update', { ...reference, maintainerCanModify: allowed }, true);
}
