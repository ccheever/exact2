// The first-run decision (onboarding/firstRun.logic.ts) and the project-path
// comparison it uses (packages/shared/src/path.ts), from T3 Code (MIT, see
// LICENSE-T3), reference 1e2ecbd975. pages-welcome.ts decides the primary
// environment's first-run gate with it (20261005-local-primary-environment item 6):
// a desktop with a primary waits for its live workspace evidence; one without
// (local environment off) decides as the hosted client does.

// ── packages/shared/src/path.ts ────────────────────────────────────────────

export function isWindowsDrivePath(value: string): boolean {
  return /^[a-zA-Z]:([/\\]|$)/.test(value);
}

export function isUncPath(value: string): boolean {
  return value.startsWith('\\\\');
}

function isRootPath(value: string): boolean {
  // The drive separator is required: a bare `C:` is not the drive root (it
  // means "current directory on C:"), and treating it as already-canonical
  // would leave it as `C:` while `C:\` and `C:/` normalize to the drive root,
  // so the same location would fail project identity/dedup comparisons.
  return value === '/' || value === '\\' || /^[a-zA-Z]:[/\\]$/.test(value);
}

function trimTrailingPathSeparators(value: string): string {
  if (value.length === 0 || isRootPath(value)) {
    return value;
  }
  const trimmed = value.startsWith('/')
    ? value.replace(/\/+$/g, '')
    : value.replace(/[\\/]+$/g, '');
  if (trimmed.length === 0) {
    return value;
  }
  return /^[a-zA-Z]:$/.test(trimmed) ? `${trimmed}\\` : trimmed;
}

export function normalizeProjectPathForDispatch(value: string): string {
  return trimTrailingPathSeparators(value.trim());
}

export function normalizeProjectPathForComparison(value: string): string {
  const normalized = normalizeProjectPathForDispatch(value);
  if (isWindowsDrivePath(normalized) || isUncPath(normalized)) {
    return normalized.replaceAll('/', '\\').toLowerCase();
  }
  return normalized;
}

// ── apps/web/src/onboarding/firstRun.logic.ts ──────────────────────────────

export type FirstRunDecision = 'pending' | 'app' | 'wizard';

export interface FirstRunGateState {
  readonly decision: FirstRunDecision;
  readonly stalled: boolean;
}

type FirstRunGateEvent =
  | { readonly type: 'evidence'; readonly decision: FirstRunDecision }
  | { readonly type: 'timeout' };

export interface FirstRunWorkspaceInput {
  readonly primaryEnvironmentId: string | null;
  readonly serverCwd: string | null;
  readonly bootstrapProjectId?: string | undefined;
  readonly bootstrapThreadId?: string | undefined;
  readonly bootstrapProjectCreated?: boolean | undefined;
  readonly bootstrapThreadCreated?: boolean | undefined;
  readonly projects: ReadonlyArray<{
    readonly id: string;
    readonly environmentId: string;
    readonly workspaceRoot: string;
  }>;
  readonly threads: ReadonlyArray<{
    readonly id: string;
    readonly projectId: string;
    readonly environmentId: string;
    readonly latestRun: unknown;
    readonly latestUserMessageAt: string | null;
    readonly runtime: unknown;
  }>;
}

export interface FirstRunDecisionInput {
  readonly enabled: boolean;
  readonly hydrated: boolean;
  readonly completed: boolean;
  readonly bootstrapped: boolean;
  readonly authoritative: boolean;
  readonly workspaceAuthoritative: boolean;
  readonly workspaceProvenanceAuthoritative: boolean;
  readonly catalogReady: boolean;
  readonly serverConfigAvailable: boolean;
  readonly workspaceFresh: boolean;
  readonly projectCount: number;
  readonly threadCount: number;
}

export interface HostedFirstRunDecisionInput {
  readonly localEnvironmentDisabled?: boolean;
  readonly hydrated: boolean;
  readonly completed: boolean;
  readonly catalogReady: boolean;
  readonly environmentCount: number;
}

export function isFirstRunWorkspaceProvenanceAuthoritative(input: {
  readonly welcomeReceived: boolean;
  readonly bootstrapStatus: 'pending' | 'complete' | null;
}): boolean {
  // An empty catalog is not final while cwd auto-bootstrap is pending. Older
  // servers omit bootstrapStatus, so a received welcome with null stays valid.
  return input.welcomeReceived && input.bootstrapStatus !== 'pending';
}

/** Keeps the authenticated app unmounted until workspace evidence settles. */
export function transitionFirstRunGateState(
  state: FirstRunGateState,
  event: FirstRunGateEvent,
): FirstRunGateState {
  if (event.type === 'timeout') {
    return state.decision === 'pending' && !state.stalled ? { ...state, stalled: true } : state;
  }

  if (
    state.decision === 'wizard' ||
    event.decision === 'pending' ||
    (state.decision === 'app' && event.decision !== 'wizard')
  ) {
    return state;
  }

  return { decision: event.decision, stalled: false };
}

/** Only a project and thread created by this startup count as a fresh nonempty workspace. */
export function isFreshFirstRunWorkspace(input: FirstRunWorkspaceInput): boolean {
  if (input.projects.length > 1 || input.threads.length > 1) {
    return false;
  }

  const bootstrapProject = input.projects[0];
  if (bootstrapProject !== undefined) {
    if (
      input.bootstrapProjectCreated !== true ||
      input.bootstrapProjectId !== bootstrapProject.id ||
      input.serverCwd === null ||
      bootstrapProject.environmentId !== input.primaryEnvironmentId ||
      normalizeProjectPathForComparison(bootstrapProject.workspaceRoot) !==
        normalizeProjectPathForComparison(input.serverCwd)
    ) {
      return false;
    }
  }

  const bootstrapThread = input.threads[0];
  if (bootstrapThread === undefined) {
    return true;
  }

  return (
    bootstrapProject !== undefined &&
    input.bootstrapThreadCreated === true &&
    input.bootstrapThreadId === bootstrapThread.id &&
    bootstrapThread.environmentId === input.primaryEnvironmentId &&
    bootstrapThread.projectId === bootstrapProject.id &&
    bootstrapThread.latestRun === null &&
    bootstrapThread.latestUserMessageAt === null &&
    bootstrapThread.runtime === null
  );
}

/** Cached projects may open the app, but only live workspace data may complete onboarding. */
export function resolveFirstRunDecision(input: FirstRunDecisionInput): {
  readonly decision: FirstRunDecision;
  readonly persistCompletion: boolean;
} {
  if (!input.enabled || (input.hydrated && input.completed)) {
    return { decision: 'app', persistCompletion: false };
  }

  if (!input.hydrated) {
    return { decision: 'pending', persistCompletion: false };
  }

  if (input.projectCount > 1 || input.threadCount > 1) {
    return {
      decision: 'app',
      persistCompletion:
        input.bootstrapped &&
        input.authoritative &&
        input.workspaceAuthoritative &&
        input.catalogReady &&
        input.serverConfigAvailable,
    };
  }

  if (
    !input.bootstrapped ||
    !input.authoritative ||
    !input.workspaceProvenanceAuthoritative ||
    !input.catalogReady ||
    !input.serverConfigAvailable
  ) {
    return { decision: 'pending', persistCompletion: false };
  }

  return input.workspaceFresh
    ? { decision: 'wizard', persistCompletion: false }
    : { decision: 'app', persistCompletion: input.workspaceAuthoritative };
}

/** Hosted onboarding depends on saved environments because there is no primary server. */
export function resolveHostedFirstRunDecision(input: HostedFirstRunDecisionInput): {
  readonly decision: FirstRunDecision;
  readonly persistCompletion: boolean;
} {
  if (!input.hydrated) {
    return { decision: 'pending', persistCompletion: false };
  }

  if (input.completed) {
    return { decision: 'app', persistCompletion: false };
  }

  if (!input.catalogReady) {
    return { decision: 'pending', persistCompletion: false };
  }

  // An existing desktop may have disabled its server before onboarding existed.
  // Keep Connections accessible so it can turn local execution back on.
  return input.environmentCount === 0 && !input.localEnvironmentDisabled
    ? { decision: 'wizard', persistCompletion: false }
    : { decision: 'app', persistCompletion: true };
}
