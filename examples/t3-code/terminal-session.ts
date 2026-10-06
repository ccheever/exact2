// Copied from T3 Code 1e2ecbd975 (MIT reference, see LICENSE-T3: packages/client-runtime/src/state/terminalSession.ts).
// Changes from the reference:
// - The `@t3tools/contracts` types are declared here (packages/contracts/src/terminal.ts shapes);
//   `EnvironmentId` and `ThreadId` are plain strings.
// - `./terminalOutput.ts` is `./terminal-output`.
// The drawer's native session (modules/apple/T3TerminalSessions.swift) folds the attach stream the
// same way; this module is the reducer the bun tests and the metadata stream use.
export type TerminalSessionStatus = "starting" | "running" | "exited" | "error";
export interface TerminalSessionSnapshot {
  readonly threadId: string;
  readonly terminalId: string;
  readonly cwd: string;
  readonly worktreePath: string | null;
  readonly status: TerminalSessionStatus;
  readonly pid: number | null;
  readonly history: string;
  readonly exitCode: number | null;
  readonly exitSignal: number | null;
  readonly label: string;
  readonly updatedAt: string;
  readonly sequence?: number;
}
export interface TerminalSummary {
  readonly threadId: string;
  readonly terminalId: string;
  readonly cwd: string;
  readonly worktreePath: string | null;
  readonly status: TerminalSessionStatus;
  readonly pid: number | null;
  readonly exitCode: number | null;
  readonly exitSignal: number | null;
  readonly hasRunningSubprocess: boolean;
  readonly label: string;
  readonly updatedAt: string;
}
export type TerminalMetadataStreamEvent =
  | { readonly type: "snapshot"; readonly terminals: ReadonlyArray<TerminalSummary> }
  | { readonly type: "upsert"; readonly terminal: TerminalSummary }
  | { readonly type: "remove"; readonly threadId: string; readonly terminalId: string };
type TerminalEventBase = { readonly threadId: string; readonly terminalId: string; readonly sequence?: number };
export type TerminalAttachStreamEvent =
  | { readonly type: "snapshot"; readonly snapshot: TerminalSessionSnapshot }
  | (TerminalEventBase & { readonly type: "output"; readonly data: string })
  | (TerminalEventBase & { readonly type: "exited"; readonly exitCode: number | null; readonly exitSignal: number | null })
  | (TerminalEventBase & { readonly type: "closed" })
  | (TerminalEventBase & { readonly type: "error"; readonly message: string })
  | (TerminalEventBase & { readonly type: "cleared" })
  | (TerminalEventBase & { readonly type: "restarted"; readonly snapshot: TerminalSessionSnapshot })
  | (TerminalEventBase & { readonly type: "activity"; readonly hasRunningSubprocess: boolean; readonly label: string });
type EnvironmentId = string;
type ThreadId = string;

import {
  appendOutput,
  DEFAULT_MAX_TERMINAL_BUFFER_BYTES,
  EMPTY_TERMINAL_OUTPUT_STATE,
  resetOutput,
  type TerminalOutputState,
} from "./terminal-output";

export {
  DEFAULT_MAX_TERMINAL_BUFFER_BYTES,
  INITIAL_TERMINAL_OUTPUT_CURSOR,
  readTerminalOutputUpdate,
  terminalOutputText,
  type TerminalOutputCursor,
  type TerminalOutputState,
  type TerminalOutputUpdate,
} from "./terminal-output";

export interface TerminalSessionState {
  readonly summary: TerminalSummary | null;
  readonly output: TerminalOutputState;
  readonly status: TerminalSessionSnapshot["status"] | "closed";
  readonly error: string | null;
  readonly hasRunningSubprocess: boolean;
  readonly updatedAt: string | null;
  readonly version: number;
  readonly lifecycleVersion: number;
}

export interface TerminalBufferState {
  readonly output: TerminalOutputState;
  readonly status: TerminalSessionSnapshot["status"] | "closed";
  readonly error: string | null;
  readonly updatedAt: string | null;
  readonly version: number;
  readonly lifecycleVersion: number;
}

export interface KnownTerminalSessionTarget {
  readonly environmentId: EnvironmentId;
  readonly threadId: ThreadId;
  readonly terminalId: string;
}

export interface KnownTerminalSession {
  readonly target: KnownTerminalSessionTarget;
  readonly state: TerminalSessionState;
}

export function selectRunningSubprocessTerminalIds(
  sessions: ReadonlyArray<KnownTerminalSession>,
): ReadonlyArray<string> {
  return sessions
    .filter((session) => session.state.hasRunningSubprocess)
    .map((session) => session.target.terminalId);
}

export const EMPTY_TERMINAL_BUFFER_STATE = Object.freeze<TerminalBufferState>({
  output: EMPTY_TERMINAL_OUTPUT_STATE,
  status: "closed",
  error: null,
  updatedAt: null,
  version: 0,
  lifecycleVersion: 0,
});

export const EMPTY_TERMINAL_SESSION_STATE = Object.freeze<TerminalSessionState>({
  summary: null,
  output: EMPTY_TERMINAL_OUTPUT_STATE,
  status: "closed",
  error: null,
  hasRunningSubprocess: false,
  updatedAt: null,
  version: 0,
  lifecycleVersion: 0,
});

let terminalAttachGeneration = 0;

/** A reinstalled attach stream must not reuse an old renderer's output cursor. */
export function nextTerminalAttachSeedState(): TerminalBufferState {
  return {
    ...EMPTY_TERMINAL_BUFFER_STATE,
    output: {
      ...EMPTY_TERMINAL_OUTPUT_STATE,
      generation: ++terminalAttachGeneration,
    },
  };
}

function terminalBufferStateFromSnapshot(
  snapshot: TerminalSessionSnapshot,
  maxBufferBytes: number,
  current: TerminalBufferState = EMPTY_TERMINAL_BUFFER_STATE,
): TerminalBufferState {
  return {
    output: resetOutput(current.output, snapshot.history, maxBufferBytes),
    status: snapshot.status,
    error: null,
    updatedAt: snapshot.updatedAt,
    version: current.version + 1,
    lifecycleVersion: current.lifecycleVersion,
  };
}

function latestTimestamp(left: string | null, right: string | null): string | null {
  if (left === null) return right;
  if (right === null) return left;
  return Date.parse(left) >= Date.parse(right) ? left : right;
}

export function combineTerminalSessionState(
  summary: TerminalSummary | null,
  buffer: TerminalBufferState,
): TerminalSessionState {
  return {
    summary,
    output: buffer.output,
    status: buffer.version > 0 ? buffer.status : (summary?.status ?? buffer.status),
    error: buffer.error,
    hasRunningSubprocess: summary?.hasRunningSubprocess ?? false,
    updatedAt: latestTimestamp(summary?.updatedAt ?? null, buffer.updatedAt),
    version: buffer.version,
    lifecycleVersion: buffer.lifecycleVersion,
  };
}

export function applyTerminalAttachStreamEvent(
  current: TerminalBufferState,
  event: TerminalAttachStreamEvent,
  maxBufferBytes = DEFAULT_MAX_TERMINAL_BUFFER_BYTES,
): TerminalBufferState {
  switch (event.type) {
    case "snapshot":
      return {
        ...terminalBufferStateFromSnapshot(event.snapshot, maxBufferBytes, current),
        lifecycleVersion:
          current.version === 0 ? current.lifecycleVersion : current.lifecycleVersion + 1,
      };
    case "restarted":
      return {
        ...terminalBufferStateFromSnapshot(event.snapshot, maxBufferBytes, current),
        lifecycleVersion: current.lifecycleVersion + 1,
      };
    case "output":
      return {
        ...current,
        output: appendOutput(current.output, event.data, maxBufferBytes),
        status: current.status === "closed" ? "running" : current.status,
        error: null,
        version: current.version + 1,
      };
    case "cleared":
      return {
        ...current,
        output: resetOutput(current.output, "", maxBufferBytes),
        error: null,
        version: current.version + 1,
      };
    case "exited":
      return {
        ...current,
        status: "exited",
        error: null,
        version: current.version + 1,
      };
    case "closed":
      return {
        ...current,
        status: "closed",
        error: null,
        version: current.version + 1,
      };
    case "error":
      return {
        ...current,
        status: "error",
        error: event.message,
        version: current.version + 1,
      };
    case "activity":
      return current;
  }
}

export function applyTerminalMetadataStreamEvent(
  current: ReadonlyArray<TerminalSummary>,
  event: TerminalMetadataStreamEvent,
): ReadonlyArray<TerminalSummary> {
  if (event.type === "snapshot") {
    return event.terminals;
  }
  if (event.type === "remove") {
    return current.filter(
      (terminal) =>
        terminal.threadId !== event.threadId || terminal.terminalId !== event.terminalId,
    );
  }
  const next = current.filter(
    (terminal) =>
      terminal.threadId !== event.terminal.threadId ||
      terminal.terminalId !== event.terminal.terminalId,
  );
  return [...next, event.terminal];
}
