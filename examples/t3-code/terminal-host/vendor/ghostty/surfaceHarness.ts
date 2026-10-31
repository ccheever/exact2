// Copied from T3 Code 1e2ecbd975 (MIT reference, see LICENSE-T3: apps/web/src/terminal/ghostty/surface.test.ts),
// with 74541bab17's `repeat` and target in `key` and `canvas` in the harness.
// This part holds the surface tests' key event helper and browser harness. The reference keeps them
// inside surface.test.ts's "GhosttyTerminalSurface visibility" block.
// Changes from the reference:
// - split out of surface.test.ts for the 1,500-line cap, so surface.test.ts and
//   surfaceSynchronized.test.ts share them; the block's `surfaces` set is this module's, and
//   `disposeHarnessSurfaces` is its afterEach.
// - `vite-plus/test` is `bun:test`.
import { vi } from "bun:test";
import "../../src/test-assets";

import { GhosttyTerminalCore } from "./core";
import { GhosttyTerminalSurface, type GhosttyTerminalSurfaceOptions } from "./surface";

const surfaces = new Set<GhosttyTerminalSurface>();

export function disposeHarnessSurfaces(): void {
  for (const surface of surfaces) surface.dispose();
  surfaces.clear();
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
}

export function key(
  surface: GhosttyTerminalSurface,
  key: string,
  code: string,
  modifiers: Partial<
    Pick<
      KeyboardEvent,
      | "altKey"
      | "ctrlKey"
      | "metaKey"
      | "shiftKey"
      | "isComposing"
      | "repeat"
      | "keyCode"
      | "getModifierState"
    >
  > = {},
  type = "keydown",
  target: EventTarget = surface.input,
) {
  const event = Object.assign(new Event(type, { cancelable: true }), {
    key,
    code,
    altKey: false,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    isComposing: false,
    repeat: false,
    keyCode: 0,
    getModifierState: () => false,
    ...modifiers,
  });
  target.dispatchEvent(event);
  return event;
}

// Keep the real surface, renderer, and WASM core. Only browser layout and
// scheduling are replaced so tests can count work while the terminal is hidden.
export function createHarness() {
  vi.useFakeTimers();
  const frames = new Map<number, FrameRequestCallback>();
  const resizeCallbacks = new Set<() => void>();
  const paint = vi.fn((_operation: string, _args: ReadonlyArray<unknown>) => {});
  let frameId = 0;
  const requestFrame = vi.fn((callback: FrameRequestCallback) => {
    frames.set(++frameId, callback);
    return frameId;
  });

  class TerminalTestElement extends EventTarget {
    style: Record<string, string> = {};
    parentElement: TerminalTestElement | null = null;
    clientWidth = 168;
    clientHeight = 104;
    width = 300;
    height = 150;
    value = "";
    private readonly captures = new Set<number>();

    setAttribute() {}
    append(...children: TerminalTestElement[]) {
      for (const child of children) child.parentElement = this;
    }
    replaceChildren(...children: TerminalTestElement[]) {
      this.append(...children);
    }
    remove() {
      this.parentElement = null;
    }
    getContext() {
      return context;
    }
    focus() {
      this.dispatchEvent(new Event("focus"));
    }
    setPointerCapture(pointerId: number) {
      this.captures.add(pointerId);
    }
    hasPointerCapture(pointerId: number) {
      return this.captures.has(pointerId);
    }
    releasePointerCapture(pointerId: number) {
      this.captures.delete(pointerId);
    }
    getBoundingClientRect() {
      return { left: 0, top: 0, right: 168, bottom: 104, width: 168, height: 104 };
    }
  }

  const canvas = new TerminalTestElement();
  const mount = new TerminalTestElement();
  const context = {
    canvas,
    beginPath() {},
    clip() {},
    rect() {},
    resetTransform() {},
    restore() {},
    save() {},
    setTransform() {},
    fillRect: (...args: number[]) => paint("fillRect", args),
    strokeRect: (...args: number[]) => paint("strokeRect", args),
    fillText: (...args: [string, number, number, number?]) => paint("fillText", args),
    measureText: (text: string) => ({
      width: text.length * 8,
      actualBoundingBoxAscent: 9,
      actualBoundingBoxDescent: 3,
    }),
  };
  vi.stubGlobal("document", {
    createElement: (tag: string) => (tag === "canvas" ? canvas : new TerminalTestElement()),
    fonts: Object.assign(new EventTarget(), { load: async () => [], add() {} }),
  });
  vi.stubGlobal(
    "window",
    Object.assign(new EventTarget(), {
      devicePixelRatio: 1,
      innerHeight: 104,
      requestAnimationFrame: requestFrame,
      cancelAnimationFrame: (id: number) => frames.delete(id),
      setTimeout,
      clearTimeout,
      setInterval,
      clearInterval,
      matchMedia: () => Object.assign(new EventTarget(), { matches: false }),
    }),
  );
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(private readonly callback: () => void) {
        resizeCallbacks.add(callback);
      }
      observe() {}
      disconnect() {
        resizeCallbacks.delete(this.callback);
      }
    },
  );
  const snapshot = vi.spyOn(GhosttyTerminalCore.prototype, "snapshot");
  const onData = vi.fn<(data: string) => void>();

  return {
    canvas,
    mount,
    frames,
    paint,
    requestFrame,
    snapshot,
    onData,
    get renderedSnapshot() {
      const result = snapshot.mock.results.at(-1);
      if (result?.type !== "return") throw new Error("No terminal snapshot was rendered");
      return result.value;
    },
    flushFrame() {
      const queued = [...frames.values()];
      frames.clear();
      for (const callback of queued) callback(0);
    },
    resize() {
      for (const callback of resizeCallbacks) callback();
    },
    pointer(type: string, clientX: number, buttons: number, shiftKey = false, button = 0, clientY = 5) {
      canvas.dispatchEvent(
        Object.assign(new Event(type, { cancelable: true }), {
          clientX,
          clientY,
          pointerId: 1,
          button,
          buttons,
          shiftKey,
        }),
      );
    },
    async create(options: Partial<GhosttyTerminalSurfaceOptions> = {}) {
      const surface = await GhosttyTerminalSurface.create(mount as unknown as HTMLElement, {
        theme: {
          foreground: { r: 255, g: 255, b: 255 },
          background: { r: 0, g: 0, b: 0 },
          cursor: { r: 255, g: 255, b: 255 },
        },
        onData,
        onResize() {},
        onSelectionChange() {},
        beforeKey: () => false,
        onLinkActivate() {},
        ...options,
        get visible() {
          return options.visible ?? true;
        },
      });
      surfaces.add(surface);
      return surface;
    },
  };
}
