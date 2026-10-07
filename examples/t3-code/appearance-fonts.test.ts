// Tests copied from T3 Code 1e2ecbd975 (MIT reference, see LICENSE-T3), with their original names:
// apps/web/src/appearanceFonts.test.ts `describe("font size clamping")`. Changes from the reference:
// `vite-plus/test` is `bun:test`. The last case is new: a stored non-number takes the default.
import { describe, expect, it } from "bun:test";

import { clampCodeFontSize, clampInterfaceFontSize, clampPromptFontSize } from "./appearance-fonts";
import { chatLaneMetrics } from "./chat-canvas-layout";
// The look first: importing r5-composer-menus on its own enters an import cycle at r6-polish-measure.
import { chatMaxWidth } from "./settings-appearance-look";
import { atRootFontSize } from "./r5-composer-menus";

describe("font size clamping", () => {
  it("keeps sizes inside the ranges the UI can absorb", () => {
    expect(clampInterfaceFontSize(16)).toBe(16);
    expect(clampInterfaceFontSize(2)).toBe(12);
    expect(clampInterfaceFontSize(96)).toBe(20);
    expect(clampPromptFontSize(40)).toBe(20);
    expect(clampCodeFontSize(1)).toBe(10);
  });

  it("rounds fractional values and falls back for unusable input", () => {
    expect(clampCodeFontSize(13.4)).toBe(13);
    expect(clampInterfaceFontSize(Number.NaN)).toBe(16);
    expect(clampPromptFontSize(Number.POSITIVE_INFINITY)).toBe(14);
  });

  it("takes the default for a stored value that is not a number", () => {
    expect(clampInterfaceFontSize("20")).toBe(16);
    expect(clampInterfaceFontSize(undefined)).toBe(16);
    expect(clampCodeFontSize(null)).toBe(13);
  });
});

// The clone's own: the JS layout values that follow the root font size (not in the reference's tests).
describe("root font size in the JS layout", () => {
  it("scales the chat width setting and the lane's rem constants", () => {
    expect(chatMaxWidth("comfortable", 16)).toBe(736);
    expect(chatMaxWidth("comfortable", 20)).toBe(920);
    expect(chatMaxWidth("wide", 12)).toBe(864);
    expect(chatMaxWidth("full", 20)).toBe(100000);
    expect(chatLaneMetrics(0, 20)).toEqual({ padding: 25, maxChatWidth: 920, minChatWidth: 800 });
  });

  it("scales the composer's measured menus and leaves them alone at 16", () => {
    const row = { effortMenuWidth: 200, runtimeMenuHeight: 102, sendTipWidth: 80, other: 7 };
    expect(atRootFontSize(row, 16)).toBe(row);
    expect(atRootFontSize(row, 20)).toEqual({ effortMenuWidth: 250, runtimeMenuHeight: 127.5, sendTipWidth: 100, other: 7 });
  });
});
