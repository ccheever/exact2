// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/terminal-drawer.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// The thread terminal drawer's pure logic, copied from T3 Code 1e2ecbd975 (MIT reference, see LICENSE-T3):
// - apps/web/src/components/ThreadTerminalDrawer.tsx: `MIN_DRAWER_HEIGHT`, `MAX_DRAWER_HEIGHT_RATIO`,
//   `maxDrawerHeight`, `clampDrawerHeight`, `writeTerminalOutputUpdate`, `shouldHandleTerminalExit`,
//   `runtimeEnvSignature`.
// - apps/web/src/components/ChatView.logic.ts: `MAX_HIDDEN_MOUNTED_TERMINAL_THREADS`,
//   `reconcileMountedTerminalThreadIds`.
// - apps/web/src/components/ChatView.tsx: `terminalIdListsEqual`, `serverTerminalIdsStrictSubsetOfClient`
//   (private there).
// - apps/web/src/appearanceFonts.ts: `resolveTerminalFontPreference`, `resolveTerminalFontSizePreference`.
// - packages/shared/src/projectScripts.ts: `projectScriptCwd`, `projectScriptRuntimeEnv`.
// Changes from the reference:
// - `maxDrawerHeight` and `clampDrawerHeight` take the window height (the reference reads
//   `window.innerHeight`); without one they answer as the reference does with no window (the default).
// - `writeTerminalOutputUpdate` takes any `{ resetAndWrite, write }` (the native view's bridge has the same
//   two calls: T3TerminalView.swift resetAndWrite / write).
// - `toSorted` is `[...list].sort` (the clone's TypeScript lib is ES2020).
// The native session (T3TerminalSessions.swift) runs the same exit rule as `shouldHandleTerminalExit`
// against the same cases (macos/tests/terminal).
import type { TerminalOutputUpdate } from "./terminal-output";
import type { TerminalSessionState } from "./terminal-session";
import { DEFAULT_THREAD_TERMINAL_HEIGHT } from "./terminal-ui-state";

export const MIN_DRAWER_HEIGHT = 180;
export const MAX_DRAWER_HEIGHT_RATIO = 0.75;

export function maxDrawerHeight(windowHeight?: number): number {
  if (windowHeight === undefined || !Number.isFinite(windowHeight)) return DEFAULT_THREAD_TERMINAL_HEIGHT;
  return Math.max(MIN_DRAWER_HEIGHT, Math.floor(windowHeight * MAX_DRAWER_HEIGHT_RATIO));
}

export function clampDrawerHeight(height: number, windowHeight?: number): number {
  const safeHeight = Number.isFinite(height) ? height : DEFAULT_THREAD_TERMINAL_HEIGHT;
  const maxHeight = maxDrawerHeight(windowHeight);
  return Math.min(Math.max(Math.round(safeHeight), MIN_DRAWER_HEIGHT), maxHeight);
}

export function writeTerminalOutputUpdate(
  terminal: { resetAndWrite: (data: string) => void; write: (data: string) => void },
  update: TerminalOutputUpdate,
): void {
  if (update.type === "reset") {
    terminal.resetAndWrite(update.data);
  } else if (update.type === "append") {
    terminal.write(update.data);
  }
}

export function runtimeEnvSignature(runtimeEnv: Record<string, string> | undefined): string {
  if (!runtimeEnv) return "";
  return JSON.stringify(
    Object.entries(runtimeEnv)
      .filter(([key, value]) => key.length > 0 && typeof value === "string")
      .sort(([leftKey], [rightKey]) => leftKey.localeCompare(rightKey)),
  );
}

export function shouldHandleTerminalExit(
  current: TerminalSessionState["status"],
  synchronized: TerminalSessionState["status"],
  alreadyHandled: boolean,
): boolean {
  return (
    (current === "closed" || current === "exited") && current !== synchronized && !alreadyHandled
  );
}

export const MAX_HIDDEN_MOUNTED_TERMINAL_THREADS = 10;

export function reconcileMountedTerminalThreadIds(input: {
  currentThreadIds: ReadonlyArray<string>;
  openThreadIds: ReadonlyArray<string>;
  activeThreadId: string | null;
  activeThreadTerminalOpen: boolean;
  maxHiddenThreadCount?: number;
}): string[] {
  const openThreadIdSet = new Set(input.openThreadIds);
  const hiddenThreadIds = input.currentThreadIds.filter(
    (threadId) => threadId !== input.activeThreadId && openThreadIdSet.has(threadId),
  );
  const maxHiddenThreadCount = Math.max(
    0,
    input.maxHiddenThreadCount ?? MAX_HIDDEN_MOUNTED_TERMINAL_THREADS,
  );
  const nextThreadIds =
    hiddenThreadIds.length > maxHiddenThreadCount
      ? hiddenThreadIds.slice(-maxHiddenThreadCount)
      : hiddenThreadIds;

  if (
    input.activeThreadId &&
    input.activeThreadTerminalOpen &&
    !nextThreadIds.includes(input.activeThreadId)
  ) {
    nextThreadIds.push(input.activeThreadId);
  }

  return nextThreadIds;
}

/** Same terminal ids (order ignored) — avoids reconcile when only server session ordering differs. */
export function terminalIdListsEqual(left: readonly string[], right: readonly string[]): boolean {
  if (left.length !== right.length) {
    return false;
  }
  if (left.length === 0) {
    return true;
  }
  const sortedLeft = [...left].sort((a, b) => a.localeCompare(b));
  const sortedRight = [...right].sort((a, b) => a.localeCompare(b));
  for (let index = 0; index < sortedLeft.length; index += 1) {
    if (sortedLeft[index] !== sortedRight[index]) {
      return false;
    }
  }
  return true;
}

/**
 * Server knows about fewer sessions than the client, but every server id still exists locally.
 * Typical right after `terminal.open`: known-session list lags; reconciling would drop the new id
 * and later re-add it as a separate group (no split layout).
 */
export function serverTerminalIdsStrictSubsetOfClient(
  serverIds: readonly string[],
  clientIds: readonly string[],
): boolean {
  if (serverIds.length >= clientIds.length || clientIds.length === 0) {
    return false;
  }
  const clientSet = new Set(clientIds);
  for (const id of serverIds) {
    if (!clientSet.has(id)) {
      return false;
    }
  }
  return true;
}

/**
 * Simple typography treats the terminal as another monospace surface. In
 * Advanced mode an empty terminal preference means the terminal default,
 * keeping later code-font changes isolated to code surfaces.
 */
export function resolveTerminalFontPreference(input: {
  readonly advanced: boolean;
  readonly code: string;
  readonly terminal: string;
}): string {
  if (input.advanced) return input.terminal;
  return input.code;
}

export function resolveTerminalFontSizePreference(input: {
  readonly advanced: boolean;
  readonly code: number;
  readonly terminal: number;
}): number {
  if (input.advanced) return input.terminal;
  return input.code;
}

interface ProjectScriptRuntimeEnvInput {
  project: {
    cwd: string;
  };
  worktreePath?: string | null;
  extraEnv?: Record<string, string>;
}

export function projectScriptCwd(input: {
  project: {
    cwd: string;
  };
  worktreePath?: string | null;
}): string {
  return input.worktreePath ?? input.project.cwd;
}

export function projectScriptRuntimeEnv(
  input: ProjectScriptRuntimeEnvInput,
): Record<string, string> {
  const env: Record<string, string> = {
    T3CODE_PROJECT_ROOT: input.project.cwd,
  };
  if (input.worktreePath) {
    env.T3CODE_WORKTREE_PATH = input.worktreePath;
  }
  if (input.extraEnv) {
    return { ...env, ...input.extraEnv };
  }
  return env;
}
