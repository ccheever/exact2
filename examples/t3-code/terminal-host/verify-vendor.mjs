#!/usr/bin/env bun
// Checks the vendored terminal binaries against vendor/VENDOR.json: every file's size and
// sha256, and the Ghostty revision the WASM embeds (ghostty_build_info) against vendor/VERSION,
// as T3 Code's runtimeAbi.test.ts:17-47 does. Reports every mismatch, then exits 1 if any.
// Reads only this directory; it never rebuilds anything.
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const manifest = JSON.parse(readFileSync(join(here, "vendor/VENDOR.json"), "utf8"));
const problems = [];

for (const entry of manifest.files) {
  const path = join(here, entry.path);
  if (!existsSync(path)) {
    problems.push(`${entry.path}: missing`);
    continue;
  }
  const bytes = readFileSync(path);
  const sha256 = createHash("sha256").update(bytes).digest("hex");
  if (bytes.byteLength !== entry.bytes) problems.push(`${entry.path}: ${bytes.byteLength} bytes, expected ${entry.bytes}`);
  if (sha256 !== entry.sha256) problems.push(`${entry.path}: sha256 ${sha256}, expected ${entry.sha256}`);
  if (entry.licenseFile && !existsSync(join(here, entry.licenseFile))) problems.push(`${entry.path}: license file ${entry.licenseFile} missing`);
  console.log(`${entry.path} ${bytes.byteLength} ${sha256}`);
}

const version = readFileSync(join(here, "vendor/VERSION"), "utf8").trim();
if (version !== manifest.ghostty.revision) problems.push(`vendor/VERSION ${version}, VENDOR.json names ${manifest.ghostty.revision}`);
try {
  const { instance } = await WebAssembly.instantiate(readFileSync(join(here, "vendor/ghostty-vt.wasm")), { env: { log: () => {} } });
  const memory = instance.exports.memory;
  const call = (name, ...args) => instance.exports[name](...args);
  const out = call("ghostty_wasm_alloc_u8_array", 8);
  const status = call("ghostty_build_info", 10, out);
  const view = new DataView(memory.buffer, out, 8);
  const embedded = new TextDecoder().decode(new Uint8Array(memory.buffer, view.getUint32(0, true), view.getUint32(4, true)));
  call("ghostty_wasm_free_u8_array", out, 8);
  if (status !== 0) problems.push(`ghostty_build_info returned ${status}`);
  else if (embedded !== version) problems.push(`ghostty-vt.wasm embeds revision ${embedded}, vendor/VERSION is ${version}`);
  else console.log(`ghostty-vt.wasm embeds ${embedded}`);
} catch (error) {
  problems.push(`ghostty-vt.wasm: ${error instanceof Error ? error.message : String(error)}`);
}

for (const problem of problems) console.error(`verify-vendor: ${problem}`);
if (problems.length > 0) process.exit(1);
console.log(`verify-vendor: ${manifest.files.length} files match VENDOR.json`);
