// scripts/wasm-check.mjs, which caps.mjs runs before every push: the crates a web app links, compiled for wasm32
// when a change touches Rust. The compile itself is the lane's (its wasm builds); here, what is checked and when.
import { test, expect } from 'bun:test';
import { resolve } from 'node:path';
import { wasmCheck, wasmCrates } from './wasm-check.mjs';

const ROOT = resolve(import.meta.dir, '..');

test('the crates checked are the libraries the web apps link for wasm32, the apps and vendored crates aside', () => {
  const { crates, bins } = wasmCrates(ROOT);
  expect(bins).toEqual([['exact-textflow', 'textflow-web']]);
  // contract: G2's cfg(unix)-only name (5540abbfc0) broke it there; exact-web: the host every web app links
  for (const c of ['contract', 'exact-web', 'exact-runner', 'exact-kernel', 'exact-markdown-editor', 'exact-web-js-motion']) expect(crates).toContain(c);
  expect(crates.some((c) => c.endsWith('-web') && c !== 'exact-web' && c !== 'exact-js-web')).toBe(false);
  expect(crates).not.toContain('taffy');
}, 30_000);

test('a target directory with no check yet waits for a Rust change; EXACT_CAPS_WASM=0 skips it', () => {
  for (const changed of [[], ['docs/x.md', 'scripts/agent.mjs', 'Cargo.md', 'host/web/tests/resize-boot.test.mjs', 'host/web/agent.test.mjs']]) {
    expect(wasmCheck(ROOT, { env: {}, changed, hot: false })).toEqual({ problems: [], counted: 'wasm32: no Rust changed since origin/main and no earlier check in this target directory, not checked' });
  }
  // A crate's own files wait for nothing: kernel/build.rs reads kernel/tables/schema.json, host/web includes its index.html
  for (const changed of [['kernel/tables/schema.json'], ['host/web/index.html'], ['.cargo/config.toml'], ['a/b.rs'], ['rust-toolchain.toml']]) {
    expect(wasmCheck(ROOT, { env: {}, changed, hot: false, dry: true }).counted).toMatch(/would check/);
  }
  expect(wasmCheck(ROOT, { env: { EXACT_CAPS_WASM: '0' }, changed: ['contract/cli/src/gallery.rs'] }).counted).toBe('wasm32: skipped (EXACT_CAPS_WASM=0)');
}, 60_000); // a cargo tree for each
