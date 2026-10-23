// Where the terminal page loads its vendored binaries. The reference imports them with Vite
// `?url` (runtime.ts:1-2, surface.ts:17); in the app they are served by the web view's
// `t3-terminal` scheme handler from the bundle's assets (T3TerminalAssets.swift), so the page
// loads nothing from the network. Tests point these at the vendored files instead.
export const TERMINAL_ASSET_ORIGIN = "t3-terminal://host/";

export let ghosttyWasmUrl = `${TERMINAL_ASSET_ORIGIN}ghostty-vt.wasm`;
export let ghosttyWritePtyWasmUrl = `${TERMINAL_ASSET_ORIGIN}ghostty-write-pty.wasm`;
export let symbolsFontUrl = `${TERMINAL_ASSET_ORIGIN}SymbolsNerdFontMono-Regular.woff2`;

/** Tests: load the binaries from a directory URL (Bun's fetch reads `file:` URLs). */
export function useTerminalAssetBase(base: string): void {
  ghosttyWasmUrl = `${base}ghostty-vt.wasm`;
  ghosttyWritePtyWasmUrl = `${base}ghostty-write-pty.wasm`;
  symbolsFontUrl = `${base}SymbolsNerdFontMono-Regular.woff2`;
}
