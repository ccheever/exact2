// Where the terminal page loads its vendored binaries. The reference imports them with Vite
// `?url` (runtime.ts:1-2, surface.ts:17); in the app they are the files beside the page under
// `assets/terminal/` (terminal-host/build.mjs), resolved against the page's own URL, so the page
// loads nothing from the network. Tests point these at the vendored files instead.
export let ghosttyWasmUrl = "ghostty-vt.wasm";
export let ghosttyWritePtyWasmUrl = "ghostty-write-pty.wasm";
export let symbolsFontUrl = "SymbolsNerdFontMono-Regular.woff2";

/** Tests: load the binaries from a directory URL (Bun's fetch reads `file:` URLs). */
export function useTerminalAssetBase(base: string): void {
  ghosttyWasmUrl = `${base}ghostty-vt.wasm`;
  ghosttyWritePtyWasmUrl = `${base}ghostty-write-pty.wasm`;
  symbolsFontUrl = `${base}SymbolsNerdFontMono-Regular.woff2`;
}
