// Copied from T3 Code 1e2ecbd975 (MIT reference, see LICENSE-T3: packages/shared/src/terminalLabels.test.ts).
// Changes from the reference: `vite-plus/test` is `bun:test`; `DEFAULT_TERMINAL_ID` comes from `./terminal-ui-state`.
import { describe, expect, it } from "bun:test";

import type { TerminalSummary } from "./terminal-session";
import { DEFAULT_TERMINAL_ID } from "./terminal-ui-state";

import { getTerminalLabel, nextTerminalId, resolveTerminalSessionLabel } from "./terminal-labels";

describe("getTerminalLabel", () => {
  it("uses the numeric suffix for term-* ids", () => {
    expect(getTerminalLabel(DEFAULT_TERMINAL_ID)).toBe("Terminal 1");
    expect(getTerminalLabel("term-2")).toBe("Terminal 2");
    expect(getTerminalLabel("term-12")).toBe("Terminal 12");
    expect(getTerminalLabel("terminal-3")).toBe("Terminal 3");
  });

  it("falls back to the raw id for unknown shapes", () => {
    expect(getTerminalLabel("custom-session")).toBe("custom-session");
  });
});

describe("resolveTerminalSessionLabel", () => {
  it("prefers a non-empty summary label", () => {
    const summary = { label: "  bun  " } as Pick<TerminalSummary, "label">;
    expect(resolveTerminalSessionLabel("term-1", summary)).toBe("bun");
  });

  it("falls back to getTerminalLabel when summary is missing or blank", () => {
    expect(resolveTerminalSessionLabel(DEFAULT_TERMINAL_ID, { label: "   " })).toBe("Terminal 1");
    expect(resolveTerminalSessionLabel(DEFAULT_TERMINAL_ID, null)).toBe("Terminal 1");
    expect(resolveTerminalSessionLabel("term-2", undefined)).toBe("Terminal 2");
  });
});

describe("nextTerminalId", () => {
  it("allocates term-1 when no terminals are listed yet", () => {
    expect(nextTerminalId([])).toBe(DEFAULT_TERMINAL_ID);
  });

  it("allocates term-2 when only term-1 exists", () => {
    expect(nextTerminalId([DEFAULT_TERMINAL_ID])).toBe("term-2");
  });

  it("skips over taken term-N slots", () => {
    expect(nextTerminalId([DEFAULT_TERMINAL_ID, "term-2", "term-3"])).toBe("term-4");
    expect(nextTerminalId([DEFAULT_TERMINAL_ID, "term-3"])).toBe("term-2");
    expect(nextTerminalId(["term-2", "term-3"])).toBe("term-1");
  });

  it("ignores blank/whitespace-only ids", () => {
    expect(nextTerminalId(["", "  ", DEFAULT_TERMINAL_ID])).toBe("term-2");
    expect(nextTerminalId(["", "  "])).toBe("term-1");
  });
});
