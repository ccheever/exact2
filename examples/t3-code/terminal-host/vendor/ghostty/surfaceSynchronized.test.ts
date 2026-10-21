// Copied from T3 Code 74541bab17 (MIT reference, see LICENSE-T3: apps/web/src/terminal/ghostty/surface.test.ts):
// the synchronized output tests 26338ea544 added to the "GhosttyTerminalSurface visibility" block.
// Changes from the reference:
// - split out of surface.test.ts for the 1,500-line cap; the harness is surfaceHarness.ts.
// - `vite-plus/test` is `bun:test`.
import { afterEach, describe, expect, it, vi } from "bun:test";

import { createHarness, disposeHarnessSurfaces } from "./surfaceHarness";

describe("GhosttyTerminalSurface synchronized output", () => {
  afterEach(disposeHarnessSurfaces);

  it("holds the complete frame across split synchronized markers and a DPR change", async () => {
    const harness = createHarness();
    const surface = await harness.create();
    surface.write("\x1b[?1049h\x1b[HOLD first\x1b[2;1HOLD second");
    harness.flushFrame();
    expect(harness.renderedSnapshot.rowData.slice(0, 2).map((row) => row.text)).toEqual([
      "OLD first",
      "OLD second",
    ]);
    const { width, height } = harness.canvas;
    harness.snapshot.mockClear();
    harness.paint.mockClear();

    surface.write("\x1b[?20");
    surface.write("26h\x1b[HNEW first\x1b[5n");
    expect(harness.onData).toHaveBeenCalledWith("\x1b[0n");
    harness.flushFrame();
    window.devicePixelRatio = 2;
    harness.resize();
    vi.advanceTimersByTime(200);
    surface.write("\x1b[2;1HNEW second\x1b[?2026");
    harness.flushFrame();
    expect(harness.snapshot).not.toHaveBeenCalled();
    expect(harness.paint).not.toHaveBeenCalled();
    expect([harness.canvas.width, harness.canvas.height]).toEqual([width, height]);

    surface.write("l");
    harness.flushFrame();
    expect(harness.snapshot).toHaveBeenCalledTimes(1);
    expect(harness.renderedSnapshot.rowData.slice(0, 2).map((row) => row.text)).toEqual([
      "NEW first",
      "NEW second",
    ]);
    expect([harness.canvas.width, harness.canvas.height]).toEqual([width * 2, height * 2]);
    expect(
      harness.paint.mock.calls
        .filter(([operation]) => operation === "fillText")
        .map(([, args]) => args[0]),
    ).toEqual(expect.arrayContaining(["NEW first", "NEW second"]));
    vi.advanceTimersByTime(1_000);
    harness.flushFrame();
    expect(harness.snapshot).toHaveBeenCalledTimes(1);
  });

  it("recovers an unterminated synchronized update at 500ms despite later output", async () => {
    const harness = createHarness();
    const surface = await harness.create();
    surface.write("before");
    harness.flushFrame();
    harness.snapshot.mockClear();
    harness.paint.mockClear();
    surface.write("\x1b[?2026h\rpartial");
    vi.advanceTimersByTime(499);
    surface.write(" frame");
    harness.flushFrame();
    expect(harness.snapshot).not.toHaveBeenCalled();
    expect(harness.paint).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    harness.flushFrame();
    expect(harness.snapshot).toHaveBeenCalledTimes(1);
    expect(harness.renderedSnapshot.rowData[0]?.text).toBe("partial frame");
    vi.advanceTimersByTime(1_000);
    harness.flushFrame();
    expect(harness.snapshot).toHaveBeenCalledTimes(1);
    surface.write("!");
    harness.flushFrame();
    expect(harness.renderedSnapshot.rowData[0]?.text).toBe("partial frame!");
  });

  it("gives a synchronized group its own deadline when one write closes A and opens B", async () => {
    const harness = createHarness();
    const surface = await harness.create();
    surface.write("old");
    harness.flushFrame();
    harness.snapshot.mockClear();
    harness.paint.mockClear();
    surface.write("\x1b[?2026h\rAAA");
    vi.advanceTimersByTime(450);
    surface.write("\x1b[?2026l\x1b[?2026h\rBBB");
    vi.advanceTimersByTime(50);
    harness.flushFrame();
    expect(harness.snapshot).not.toHaveBeenCalled();
    expect(harness.paint).not.toHaveBeenCalled();
    vi.advanceTimersByTime(449);
    harness.flushFrame();
    expect(harness.snapshot).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    harness.flushFrame();
    expect(harness.snapshot).toHaveBeenCalledTimes(1);
    expect(harness.renderedSnapshot.rowData[0]?.text).toBe("BBB");
  });

  it.each(["hide", "zero-size", "dispose"] as const)(
    "cancels synchronized recovery on %s",
    async (action) => {
      const harness = createHarness();
      const surface = await harness.create();
      surface.write("old");
      harness.flushFrame();
      surface.write("\x1b[?2026h\rnew");
      harness.snapshot.mockClear();
      harness.paint.mockClear();
      if (action === "hide") surface.setVisible(false);
      else if (action === "zero-size") {
        harness.mount.clientWidth = 0;
        harness.resize();
      } else surface.dispose();
      vi.advanceTimersByTime(1_000);
      harness.flushFrame();
      expect(harness.snapshot).not.toHaveBeenCalled();
      expect(harness.paint).not.toHaveBeenCalled();
      if (action === "dispose") return;
      if (action === "hide") surface.setVisible(true);
      else {
        harness.mount.clientWidth = 168;
        harness.resize();
      }
      expect(harness.snapshot).not.toHaveBeenCalled();
      surface.write("\x1b[?2026l");
      harness.flushFrame();
      expect(harness.renderedSnapshot.rowData[0]?.text).toBe("new");
    },
  );

  it("replaces an open synchronized update on reset without a stale recovery paint", async () => {
    const harness = createHarness();
    const surface = await harness.create();
    surface.write("\x1b[?2026hpartial");
    harness.snapshot.mockClear();
    surface.resetAndWrite("replacement");
    harness.flushFrame();
    expect(harness.snapshot).toHaveBeenCalledTimes(1);
    expect(harness.renderedSnapshot.rowData[0]?.text).toBe("replacement");
    vi.advanceTimersByTime(1_000);
    harness.flushFrame();
    expect(harness.snapshot).toHaveBeenCalledTimes(1);
  });

  it("paints a grid resize immediately when Ghostty ends synchronized output", async () => {
    const harness = createHarness();
    const surface = await harness.create();
    surface.write("old");
    harness.flushFrame();
    surface.write("\x1b[?2026h\rnew");
    harness.snapshot.mockClear();
    harness.mount.clientWidth = 176;
    harness.resize();
    expect(harness.snapshot).toHaveBeenCalledTimes(1);
    expect(harness.canvas.width).toBe(176);
    expect(harness.renderedSnapshot.rowData[0]?.text).toBe("new");
    vi.advanceTimersByTime(1_000);
    harness.flushFrame();
    expect(harness.snapshot).toHaveBeenCalledTimes(1);
  });
});
