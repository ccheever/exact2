#!/usr/bin/env bun
// Builds the terminal page into the app's assets (`assets/terminal/`, ignored by git): the page and
// its one script, and byte copies of the vendored WASM and font. The app shows it in an `iframe` as a
// bundled page (LLP 1020.001), so every file is served beside the page. Reads only this directory and
// the example's own sources; it never reads a T3 Code checkout or the network.
// Run before the app bundle build: `bun terminal-host/build.mjs` (app.json `commands.terminal`).
import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const app = dirname(here);
const assets = join(app, "assets", "terminal");
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
  // The page's own files, and the terminal's server socket (its origin is the environment's, known at run time).
  "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'unsafe-inline'; font-src 'self'; connect-src 'self' ws: wss:; img-src 'self'\">" +
  '<title>Terminal</title></head><body><div id="terminal"></div><script src="terminal.js"></script></body></html>\n';

mkdirSync(assets, { recursive: true });
const written = {
  "terminal.js": script,
  "index.html": html,
};
for (const [name, text] of Object.entries(written)) writeFileSync(join(assets, name), text);
const copies = {
  "ghostty-vt.wasm": "vendor/ghostty-vt.wasm",
  "ghostty-write-pty.wasm": "vendor/ghostty-write-pty.wasm",
  "SymbolsNerdFontMono-Regular.woff2": "vendor/SymbolsNerdFontMono-Regular.woff2",
};
for (const [name, source] of Object.entries(copies)) copyFileSync(join(here, source), join(assets, name));
for (const name of [...Object.keys(written), ...Object.keys(copies)]) {
  const bytes = readFileSync(join(assets, name));
  console.log(`assets/terminal/${name} ${bytes.byteLength} ${createHash("sha256").update(bytes).digest("hex")}`);
}
console.log(`terminal-host: ${inputs.length} sources bundled`);
