// @exact/child-process (LLP 1133.007): programs a desktop app's data code
// starts, with Node's `child_process` names. The executor implements it on
// macOS and Linux (`executor` in exact.json); the bake writes this file as
// the package's binding on every target, and where no host starts programs
// every name rejects NotSupportedError and `processStatus().available` is
// false. Its declared deviations from Node are LLP 1133.007 D1's.

/** The signals `kill` and `killSignal` take (D2). */
export type Signal =
  | "SIGTERM" | "SIGINT" | "SIGHUP" | "SIGQUIT" | "SIGUSR1" | "SIGUSR2" | "SIGSTOP" | "SIGCONT" | "SIGKILL";

/** One event of a child, in order (D2). */
export type ChildEvent =
  | { type: "spawn"; pid: number | null; restarts: number }
  | { type: "stdout" | "stderr"; data: string | ArrayBuffer }
  | { type: "exit"; code: number | null; signal: Signal | null }
  | { type: "close"; code: number | null; signal: Signal | null; drained: boolean }
  | { type: "drain" }
  | { type: "ready" }
  | { type: "ready-timeout" }
  | { type: "error"; kind: string; message: string };

/** One incarnation's handle (D2): its stdin, and signals to its process group. */
export interface Child {
  readonly pid: number | null;
  readonly exitCode: number | null;
  readonly signalCode: Signal | null;
  /** Bytes queued for its stdin: the host's as the turn began, and this turn's. */
  readonly writableLength: number;
  /** Staged with the commit; `false` past 64 KiB queued: wait for `drain`. */
  write(data: string | ArrayBuffer | ArrayBufferView): boolean;
  end(): void;
  kill(signal?: Signal): boolean;
}

/** What every call takes. */
export interface CommonOptions {
  /** A `home:`, `app:` or absolute path; default `home:/`. */
  cwd?: string;
  /** The child's whole environment, as Node's `env` replaces it; omitted, the app's own. */
  env?: Record<string, string | undefined>;
  encoding?: "utf8" | "buffer";
  killSignal?: Signal;
  /** A file as stdin, read by the host under `fs.read`. */
  exactStdinFrom?: string;
}

/** `spawn`'s options (D2, D4). */
export interface SpawnOptions<T> extends CommonOptions {
  stdio?: "pipe" | "ignore" | Array<"pipe" | "ignore">;
  /** After release's signal, `SIGKILL` follows this many milliseconds later (default 5000). */
  exactForceKillAfter?: number;
  /** How long output may follow `exit` before the pipes close (default 5000). */
  exactDrainTimeout?: number;
  exactRestart?: { when: "always" | "on-failure"; delay?: number; maxDelay?: number; preventExitStatus?: number[] };
  exactReady?: { url: string; interval?: number; timeout?: number; accept?: [number, number]; onTimeout?: "retry" | "fail" };
  /** Maps each event to the resource's answer, now. */
  exactStream: (event: ChildEvent, child: Child) => T;
}

/** `execFile`'s options (D3). */
export interface ExecFileOptions extends CommonOptions {
  /** Written to stdin, which is then closed. */
  input?: string | ArrayBuffer | ArrayBufferView;
  timeout?: number;
  maxBuffer?: number;
  signal?: AbortSignal;
}

/** What `execFile` answers. */
export interface ExecFileResult<O = string | ArrayBuffer> {
  stdout: O;
  stderr: O;
  code: number | null;
  signal: Signal | null;
}

/** The module's permission fact (§6): never `prompt`, since nothing asks. */
export interface ProcessStatus {
  available: boolean;
  state: "granted" | "denied" | "unknown";
}

type Implementation = {
  spawn(file: string, args: string[], options: SpawnOptions<unknown>): Promise<unknown>;
  execFile(file: string, args: string[], options?: ExecFileOptions): Promise<ExecFileResult>;
  execPath(target: string): Promise<string | null>;
  loginShellEnvironment(): Promise<Record<string, string>>;
  appEnvironment(): Promise<Record<string, string>>;
  processStatus(): ProcessStatus;
};

const implementation = (globalThis as { __exact_child_process?: Implementation }).__exact_child_process;

function absent(name: string): Promise<never> {
  const error = Object.assign(new Error(`${name}: NotSupportedError: this platform starts no programs (LLP 1133.007)`), { code: "NotSupportedError" });
  return Promise.reject(error);
}

/** A child as a stream resource: the answer is what `exactStream` returns, event by event. */
export function spawn<T>(file: string, args: string[], options: SpawnOptions<T>): Promise<T> {
  return implementation ? (implementation.spawn(file, args, options as SpawnOptions<unknown>) as Promise<T>) : absent("spawn");
}

/** A one-shot run to `close`. A non-zero exit, a signal, the timeout or `maxBuffer` rejects. */
export function execFile(file: string, args?: string[], options?: ExecFileOptions & { encoding: "utf8" }): Promise<ExecFileResult<string>>;
export function execFile(file: string, args?: string[], options?: ExecFileOptions): Promise<ExecFileResult>;
export function execFile(file: string, args: string[] = [], options?: ExecFileOptions): Promise<ExecFileResult> {
  return implementation ? implementation.execFile(file, args, options) : absent("execFile");
}

/** The native path a spawn of `target` would run, or `null`. */
export function execPath(target: string): Promise<string | null> {
  return implementation ? implementation.execPath(target) : absent("execPath");
}

/** The login shell's variables that `env.read login:<NAME>` lines name (D5). */
export function loginShellEnvironment(): Promise<Record<string, string>> {
  return implementation ? implementation.loginShellEnvironment() : absent("loginShellEnvironment");
}

/** The app's own variables that `env.read <NAME>` lines name (D5). */
export function appEnvironment(): Promise<Record<string, string>> {
  return implementation ? implementation.appEnvironment() : absent("appEnvironment");
}

/** Whether programs may start here, and whether the grants name one. */
export function processStatus(): ProcessStatus {
  return implementation ? implementation.processStatus() : { available: false, state: "unknown" };
}
