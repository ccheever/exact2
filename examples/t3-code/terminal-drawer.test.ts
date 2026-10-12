// Tests copied from T3 Code 1e2ecbd975 (MIT reference, see LICENSE-T3), with their original names:
// apps/web/src/components/ThreadTerminalDrawer.test.ts ("handles an exit that lands while the terminal
// surface is still loading"), apps/web/src/components/ChatView.logic.test.ts (reconcileMountedTerminalThreadIds),
// apps/web/src/appearanceFonts.test.ts (resolveTerminalFontPreference, resolveTerminalFontSizePreference),
// apps/web/src/projectScripts.test.ts (the three runtime env / cwd cases).
// Changes from the reference: `vite-plus/test` is `bun:test`. The drawer height clamp tests are new
// (the reference exports neither function); they pin the 280 default, 180 floor and 75% ceiling.
import { describe, expect, it } from "bun:test";

import {
  clampDrawerHeight,
  MAX_HIDDEN_MOUNTED_TERMINAL_THREADS,
  maxDrawerHeight,
  projectScriptCwd,
  projectScriptRuntimeEnv,
  reconcileMountedTerminalThreadIds,
  resolveTerminalFontPreference,
  resolveTerminalFontSizePreference,
  serverTerminalIdsStrictSubsetOfClient,
  shouldHandleTerminalExit,
  terminalIdListsEqual,
  writeTerminalOutputUpdate,
} from "./terminal-drawer";

describe("ThreadTerminalDrawer", () => {
  it("handles an exit that lands while the terminal surface is still loading", () => {
    expect(shouldHandleTerminalExit("exited", "running", false)).toBe(true);
    expect(shouldHandleTerminalExit("exited", "exited", false)).toBe(false);
    expect(shouldHandleTerminalExit("closed", "running", true)).toBe(false);
  });

  it("writes a reset with resetAndWrite, an append with write, and nothing for none", () => {
    const calls: string[] = [];
    const terminal = { resetAndWrite: (data: string) => calls.push(`reset:${data}`), write: (data: string) => calls.push(`write:${data}`) };
    const cursor = { generation: 0, resetVersion: 0, offset: 0 };
    writeTerminalOutputUpdate(terminal, { type: "reset", data: "hello", cursor });
    writeTerminalOutputUpdate(terminal, { type: "append", data: " world", cursor });
    writeTerminalOutputUpdate(terminal, { type: "none", cursor });
    expect(calls).toEqual(["reset:hello", "write: world"]);
  });
});

describe("drawer height", () => {
  it("keeps the 280 default without a window and never goes below 180", () => {
    expect(maxDrawerHeight()).toBe(280);
    expect(clampDrawerHeight(280)).toBe(280);
    expect(clampDrawerHeight(120)).toBe(180);
    expect(clampDrawerHeight(Number.NaN)).toBe(280);
  });

  it("caps at 75% of the window height, floored, and never below 180", () => {
    expect(maxDrawerHeight(840)).toBe(630);
    expect(clampDrawerHeight(400, 840)).toBe(400);
    expect(clampDrawerHeight(10_000, 840)).toBe(630);
    expect(clampDrawerHeight(2, 840)).toBe(180);
    expect(maxDrawerHeight(620)).toBe(465);
    expect(maxDrawerHeight(200)).toBe(180);
    expect(clampDrawerHeight(400, 200)).toBe(180);
    expect(clampDrawerHeight(400.6, 840)).toBe(401);
  });
});

describe("reconcileMountedTerminalThreadIds", () => {
  it("keeps open threads and makes the active thread most recent", () => {
    expect(
      reconcileMountedTerminalThreadIds({
        currentThreadIds: ["thread-a", "thread-b", "thread-c"],
        openThreadIds: ["thread-a", "thread-b", "thread-c"],
        activeThreadId: "thread-a",
        activeThreadTerminalOpen: true,
        maxHiddenThreadCount: 2,
      }),
    ).toEqual(["thread-b", "thread-c", "thread-a"]);
  });

  it("drops closed threads and enforces the hidden mounted cap", () => {
    const ids = Array.from(
      { length: MAX_HIDDEN_MOUNTED_TERMINAL_THREADS + 2 },
      (_, index) => `thread-${index}`,
    );
    expect(
      reconcileMountedTerminalThreadIds({
        currentThreadIds: ids,
        openThreadIds: ids.slice(1),
        activeThreadId: null,
        activeThreadTerminalOpen: false,
      }),
    ).toEqual(ids.slice(-MAX_HIDDEN_MOUNTED_TERMINAL_THREADS));
  });

  it("keeps twelve open threads to the active one plus the ten most recent hidden ones", () => {
    let mounted: string[] = [];
    const ids = Array.from({ length: 12 }, (_, index) => `thread-${index}`);
    for (const id of ids) {
      mounted = reconcileMountedTerminalThreadIds({ currentThreadIds: mounted, openThreadIds: ids, activeThreadId: id, activeThreadTerminalOpen: true });
    }
    expect(mounted).toHaveLength(MAX_HIDDEN_MOUNTED_TERMINAL_THREADS + 1);
    expect(mounted).toEqual(ids.slice(1));
  });
});

describe("terminal id reconciliation guards (ChatView)", () => {
  it("compares ids without order and spots a server list that lags a fresh open", () => {
    expect(terminalIdListsEqual(["term-2", "term-1"], ["term-1", "term-2"])).toBe(true);
    expect(terminalIdListsEqual(["term-1"], ["term-1", "term-2"])).toBe(false);
    expect(serverTerminalIdsStrictSubsetOfClient(["term-1"], ["term-1", "term-2"])).toBe(true);
    expect(serverTerminalIdsStrictSubsetOfClient([], ["term-1"])).toBe(true);
    expect(serverTerminalIdsStrictSubsetOfClient(["term-3"], ["term-1", "term-2"])).toBe(false);
    expect(serverTerminalIdsStrictSubsetOfClient(["term-1"], ["term-1"])).toBe(false);
  });
});

describe("resolveTerminalFontPreference", () => {
  it("inherits the code font in simple mode", () => {
    expect(
      resolveTerminalFontPreference({ advanced: false, code: "Fira Code", terminal: "" }),
    ).toBe("Fira Code");
    expect(
      resolveTerminalFontPreference({
        advanced: false,
        code: "Fira Code",
        terminal: "Berkeley Mono",
      }),
    ).toBe("Fira Code");
  });

  it("keeps code and terminal fonts independent in advanced mode", () => {
    expect(resolveTerminalFontPreference({ advanced: true, code: "Fira Code", terminal: "" })).toBe(
      "",
    );
    expect(
      resolveTerminalFontPreference({
        advanced: true,
        code: "Fira Code",
        terminal: "Berkeley Mono",
      }),
    ).toBe("Berkeley Mono");
  });
});

describe("resolveTerminalFontSizePreference", () => {
  it("inherits the code font size in simple mode", () => {
    expect(resolveTerminalFontSizePreference({ advanced: false, code: 15, terminal: 12 })).toBe(15);
  });

  it("keeps code and terminal font sizes independent in advanced mode", () => {
    expect(resolveTerminalFontSizePreference({ advanced: true, code: 15, terminal: 12 })).toBe(12);
  });
});

describe("projectScripts", () => {
  it("builds default runtime env for scripts", () => {
    const env = projectScriptRuntimeEnv({
      project: { cwd: "/repo" },
      worktreePath: "/repo/worktree-a",
    });

    expect(env).toMatchObject({
      T3CODE_PROJECT_ROOT: "/repo",
      T3CODE_WORKTREE_PATH: "/repo/worktree-a",
    });
  });

  it("allows overriding runtime env values", () => {
    const env = projectScriptRuntimeEnv({
      project: { cwd: "/repo" },
      extraEnv: {
        T3CODE_PROJECT_ROOT: "/custom-root",
        CUSTOM_FLAG: "1",
      },
    });

    expect(env.T3CODE_PROJECT_ROOT).toBe("/custom-root");
    expect(env.CUSTOM_FLAG).toBe("1");
    expect(env.T3CODE_WORKTREE_PATH).toBeUndefined();
  });

  it("prefers the worktree path for script cwd resolution", () => {
    expect(
      projectScriptCwd({
        project: { cwd: "/repo" },
        worktreePath: "/repo/worktree-a",
      }),
    ).toBe("/repo/worktree-a");
    expect(
      projectScriptCwd({
        project: { cwd: "/repo" },
        worktreePath: null,
      }),
    ).toBe("/repo");
  });
});
