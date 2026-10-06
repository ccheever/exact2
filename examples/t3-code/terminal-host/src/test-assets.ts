// Test fixtures standing in for the reference tests' Vite `?inline` / `?raw` imports
// (runtimeAbi.test.ts:3-5): the vendored WASM as data URLs and the pinned Ghostty revision.
// It also points the runtime's asset URLs at the vendored files, as the reference's
// `vi.mock("./vendor/ghostty-vt.wasm?url", …)` did.
import { vi } from "bun:test";
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import { useTerminalAssetBase } from "./shims/assets";

const vendor = new URL("../vendor/", import.meta.url);

function dataUrl(name: string): string {
  return `data:application/wasm;base64,${readFileSync(new URL(name, vendor)).toString("base64")}`;
}

export const wasmDataUrl = dataUrl("ghostty-vt.wasm");
export const writePtyWasmDataUrl = dataUrl("ghostty-write-pty.wasm");
export const pinnedVersion = readFileSync(new URL("VERSION", vendor), "utf8");

useTerminalAssetBase(pathToFileURL(vendor.pathname).href.replace(/\/?$/, "/"));

// Bun's `vi` has no `stubGlobal` / `unstubAllGlobals` / `waitFor`, which the surface tests use to swap
// `document`, `window` and the scheduling globals; these do what Vitest's do.
type StubbableVi = typeof vi & {
  stubGlobal?: (name: string, value: unknown) => void;
  unstubAllGlobals?: () => void;
  waitFor?: (check: () => unknown) => Promise<void>;
};
const stubbed = new Map<string, PropertyDescriptor | undefined>();
const testVi = vi as StubbableVi;
testVi.stubGlobal ??= (name, value) => {
  if (!stubbed.has(name)) stubbed.set(name, Object.getOwnPropertyDescriptor(globalThis, name));
  Object.defineProperty(globalThis, name, { value, writable: true, configurable: true, enumerable: true });
};
testVi.unstubAllGlobals ??= () => {
  for (const [name, descriptor] of stubbed) {
    if (descriptor) Object.defineProperty(globalThis, name, descriptor);
    else delete (globalThis as Record<string, unknown>)[name];
  }
  stubbed.clear();
};
// Under fake timers the awaited work is promise jobs only, so retrying across microtask
// turns is enough (Vitest's polls with a real interval).
testVi.waitFor ??= async (check) => {
  let failure: unknown;
  for (let turn = 0; turn < 1000; turn += 1) {
    try {
      check();
      return;
    } catch (error) {
      failure = error;
      await Promise.resolve();
    }
  }
  throw failure;
};
