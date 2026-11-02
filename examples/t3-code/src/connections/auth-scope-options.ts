// T3 Code 2359643cff (MIT, LICENSE-T3): packages/shared/src/authScopeOptions.ts.
import { AUTH_STANDARD_CLIENT_SCOPES } from './remote-scopes';
export type Permission = (typeof AUTH_STANDARD_CLIENT_SCOPES)[number];

export const AUTH_SCOPE_OPTIONS: ReadonlyArray<{
  readonly scope: Permission;
  readonly title: string;
  readonly description: string;
}> = [
  {
    scope: 'orchestration:read',
    title: "View environment",
    description: "Read threads, status, checkpoints, and configuration.",
  },
  {
    scope: 'orchestration:operate',
    title: "Operate tasks",
    description: "Start, update, and stop tasks.",
  },
  {
    scope: 'settings:write',
    title: "Change environment settings",
    description: "Edit environment preferences and keybindings.",
  },
  {
    scope: 'providers:manage',
    title: "Manage providers",
    description: "Configure, install, sign in to, and update providers and usage sources.",
  },
  {
    scope: 'environment:maintain',
    title: "Maintain environment",
    description: "Update the server and control environment processes.",
  },
  {
    scope: 'preview:operate',
    title: "Control previews",
    description: "Open browser previews and host browser automation.",
  },
  {
    scope: 'diagnostics:read',
    title: "View diagnostics and usage",
    description: "Read process diagnostics, resource history, and usage totals.",
  },
  {
    scope: 'terminal:read',
    title: "View terminals",
    description: "Read existing terminal output and status.",
  },
  {
    scope: 'terminal:operate',
    title: "Use terminals",
    description: "Create terminals and send input to running shells.",
  },
  {
    scope: 'source-control:write',
    title: "Change source control",
    description: "Commit, push, manage branches and repositories, and change pull requests.",
  },
  {
    scope: 'filesystem:read',
    title: "Read files",
    description: "Browse host files, search workspaces, and inspect local changes.",
  },
  {
    scope: 'filesystem:write',
    title: "Write files",
    description: "Edit workspace files and save plans to disk.",
  },
  {
    scope: 'access:read',
    title: "View access",
    description: "Inspect pairing links and authorized clients.",
  },
  {
    scope: 'access:write',
    title: "Manage access",
    description: "Issue and revoke credentials for other clients.",
  },
  {
    scope: 'relay:read',
    title: "View relay",
    description: "Inspect managed relay connectivity.",
  },
  {
    scope: 'relay:write',
    title: "Manage relay",
    description: "Change managed tunnel connectivity.",
  },
];

export const READ_ONLY_SCOPES: readonly Permission[] = ['orchestration:read', 'filesystem:read', 'diagnostics:read', 'terminal:read'];
export const pairingScopeOptions = () => AUTH_SCOPE_OPTIONS.map((option, index) => ({ ...option, index, readOnly: READ_ONLY_SCOPES.includes(option.scope) }));
export const sessionPermissionRows = (permissions: readonly string[]) => AUTH_SCOPE_OPTIONS.map(option => ({ scope: option.scope, title: option.title, allowed: permissions.includes(option.scope) }));

/** What the session read has told this client: nothing yet, a failure (retried while connected), not signed in, or the grants. */
export type SessionCheck = 'pending' | 'failed' | 'rejected' | 'granted';
/** Shown instead of the granted scopes while they are unknown, so every surface says the same thing. */
export function sessionPermissionsUnavailableMessage(input: { readonly connected: boolean; readonly failed: boolean; readonly rejected: boolean }): string {
  if (!input.connected) return 'Connect to this environment to view this session’s permissions.';
  if (input.rejected) return 'This environment did not accept your session. Reconnect to try again.';
  return input.failed ? 'Could not verify your permissions. Retrying automatically.' : 'Checking permissions…';
}
