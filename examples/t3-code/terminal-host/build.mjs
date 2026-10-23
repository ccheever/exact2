#!/usr/bin/env bun
// Builds the terminal page into the app's assets (flat names; ignored by git): the page and
// its one script, and byte copies of the vendored WASM and font. T3TerminalAssets.swift serves
// exactly these files under the `t3-terminal` scheme. Reads only this directory and the
// example's terminal-links.ts; it never reads a T3 Code checkout or the network.
// Run before the app bundle build: `bun terminal-host/build.mjs` (app.json `commands.terminal`).
import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const app = dirname(here);
const assets = join(app, "assets");
const forbidden = ["effect", "@t3tools", "tailwind-merge", "zustand"];

const result = await Bun.build({
  entrypoints: [join(here, "src/entry.ts")],
  target: "browser",
  format: "iife",
  minify: true,
  metafile: true,
});
if (!result.success) {
  for (const log of result.logs) console.error(log);
  process.exit(1);
}
const inputs = Object.keys(result.metafile?.inputs ?? {});
const leaked = inputs.filter((input) => forbidden.some((name) => input.includes(`/${name}/`) || input.startsWith(name)));
const outside = inputs.filter((input) => relative(app, join(process.cwd(), input)).startsWith(".."));
if (leaked.length > 0 || outside.length > 0) {
  console.error(`terminal-host: the bundle reads ${[...leaked, ...outside].join(", ")}`);
  process.exit(1);
}
const script = await result.outputs[0].text();
const html =
  '<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width">' +
  // No network: every fetch directive names only the page's own scheme.
  "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src t3-terminal: 'wasm-unsafe-eval'; style-src 'unsafe-inline'; font-src t3-terminal:; connect-src t3-terminal:; img-src t3-terminal:\">" +
  '<title>Terminal</title></head><body><div id="terminal"></div><script src="terminal-host.js"></script></body></html>\n';

mkdirSync(assets, { recursive: true });
const written = {
  "terminal-host.js": script,
  "terminal-host.html": html,
};
for (const [name, text] of Object.entries(written)) writeFileSync(join(assets, name), text);
const copies = {
  "terminal-ghostty-vt.wasm": "vendor/ghostty-vt.wasm",
  "terminal-ghostty-write-pty.wasm": "vendor/ghostty-write-pty.wasm",
  "terminal-symbols-nerd-font-mono.woff2": "vendor/SymbolsNerdFontMono-Regular.woff2",
};
for (const [name, source] of Object.entries(copies)) copyFileSync(join(here, source), join(assets, name));
for (const name of [...Object.keys(written), ...Object.keys(copies)]) {
  const bytes = readFileSync(join(assets, name));
  console.log(`assets/${name} ${bytes.byteLength} ${createHash("sha256").update(bytes).digest("hex")}`);
}
console.log(`terminal-host: ${inputs.length} sources bundled`);
