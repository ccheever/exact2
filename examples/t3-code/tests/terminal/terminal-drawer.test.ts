// Tests copied from T3 Code 1e2ecbd975 (MIT reference, see LICENSE-T3), with their original names:
// apps/web/src/components/ThreadTerminalDrawer.test.ts ("handles an exit that lands while the terminal
// surface is still loading"), apps/web/src/components/ChatView.logic.test.ts (reconcileMountedTerminalThreadIds),
// apps/web/src/appearanceFonts.test.ts (resolveTerminalFontPreference, resolveTerminalFontSizePreference),
// apps/web/src/projectScripts.test.ts (the three runtime env / cwd cases); at 74541bab17,
// apps/web/src/lib/projectScriptTerminal.test.ts (findReusableProjectScriptTerminal) and
// ChatView.logic.test.ts (resolveTerminalWorktreePath).
// Changes from the reference: `vite-plus/test` is `bun:test`; the `$name` table titles are `%s` rows.
// projectScriptTerminalInput's test is new (the reference's covers it through the server's script runner). The drawer height clamp tests are new
// (the reference exports neither function); they pin the 280 default, 180 floor and 75% ceiling.
import { describe, expect, it } from "bun:test";

import {
  clampDrawerHeight,
  findReusableProjectScriptTerminal,
  MAX_HIDDEN_MOUNTED_TERMINAL_THREADS,
  maxDrawerHeight,
  projectScriptCwd,
  projectScriptRuntimeEnv,
  projectScriptTerminalInput,
  reconcileMountedTerminalThreadIds,
  resolveTerminalWorktreePath,
  resolveTerminalFontPreference,
  resolveTerminalFontSizePreference,
  serverTerminalIdsStrictSubsetOfClient,
  shouldHandleTerminalExit,
  terminalIdListsEqual,
  writeTerminalOutputUpdate,
} from "../../src/terminal/terminal-drawer";

describe("ThreadTerminalDrawer", () => {
  it("handles an exit that lands while the terminal surface is still loading", () => {
    expect(shouldHandleTerminalExit("exited", "running", false, 1)).toBe(true);
    expect(shouldHandleTerminalExit("exited", "exited", false, 1)).toBe(false);
    expect(shouldHandleTerminalExit("closed", "running", true, 1)).toBe(false);
    // Before the attach stream has answered, the seed's "closed" is no exit.
    expect(shouldHandleTerminalExit("closed", "running", false, 0)).toBe(false);
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

describe("projectScriptTerminalInput", () => {
  it("prefixes a space so the shell leaves the script out of its history", () => {
    expect(projectScriptTerminalInput("bun dev")).toBe(" bun dev\r");
  });
});

describe("findReusableProjectScriptTerminal", () => {
  it.each(
    [
      {
        name: "never reuses a right-panel terminal, even when stale in drawer state",
        drawerTerminalIds: ["term-1"],
        activeTerminalId: "term-1",
        panelTerminalIds: ["term-1"],
        runningTerminalIds: [],
        expected: null,
      },
      {
        name: "reuses the active idle drawer terminal",
        drawerTerminalIds: ["term-1", "term-2"],
        activeTerminalId: "term-2",
        panelTerminalIds: [],
        runningTerminalIds: [],
        expected: "term-2",
      },
      {
        name: "falls back to another idle drawer terminal when the active one is busy",
        drawerTerminalIds: ["term-1", "term-2"],
        activeTerminalId: "term-1",
        panelTerminalIds: [],
        runningTerminalIds: ["term-1"],
        expected: "term-2",
      },
      {
        name: "finds nothing when every drawer terminal is busy",
        drawerTerminalIds: ["term-1"],
        activeTerminalId: "term-1",
        panelTerminalIds: [],
        runningTerminalIds: ["term-1"],
        expected: null,
      },
    ].map((row) => [row.name, row] as const),
  )("%s", (_name, { panelTerminalIds, expected, name: _, ...input }) => {
    expect(
      findReusableProjectScriptTerminal({ ...input, panelTerminalIds: new Set(panelTerminalIds) }),
    ).toBe(expected);
  });
});

describe("resolveTerminalWorktreePath", () => {
  it("uses the terminal's checkout identity until neither launch nor session is known", () => {
    const threadWorktreePath = "/repo/.worktrees/feature";

    expect(
      resolveTerminalWorktreePath({
        launchContext: { worktreePath: null },
        summary: { worktreePath: "/repo/.worktrees/other" },
        threadWorktreePath,
      }),
    ).toBeNull();

    expect(
      resolveTerminalWorktreePath({
        launchContext: null,
        summary: { worktreePath: null },
        threadWorktreePath,
      }),
    ).toBeNull();

    expect(
      resolveTerminalWorktreePath({ launchContext: null, summary: null, threadWorktreePath }),
    ).toBe(threadWorktreePath);
  });
});
