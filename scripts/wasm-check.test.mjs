// scripts/wasm-check.mjs, which caps.mjs runs before every push: the crates a web app links, compiled for wasm32
// when a change touches Rust. The compile itself is the lane's (its wasm builds); here, what is checked and when.
import { test, expect } from 'bun:test';
import { resolve } from 'node:path';
import { wasmCheck, wasmCrates } from './wasm-check.mjs';

const ROOT = resolve(import.meta.dir, '..');

test('the crates checked are the libraries the web apps link for wasm32, the apps and vendored crates aside', () => {
  const crates = wasmCrates(ROOT);
  // contract: G2's cfg(unix)-only name (5540abbfc0) broke it there; exact-web: the host every web app links
  for (const c of ['contract', 'exact-web', 'exact-runner', 'exact-kernel']) expect(crates).toContain(c);
  expect(crates.some((c) => c.endsWith('-web') && c !== 'exact-web' && c !== 'exact-js-web')).toBe(false);
  expect(crates).not.toContain('taffy');
}, 30_000);

test('no Rust changed checks nothing, and EXACT_CAPS_WASM=0 skips it', () => {
  for (const changed of [[], ['host/web/glue.js', 'docs/x.md', 'Cargo.md']]) {
    expect(wasmCheck(ROOT, { env: {}, changed })).toEqual({ problems: [], counted: 'wasm32: no Rust changed since origin/main, not checked' });
  }
  expect(wasmCheck(ROOT, { env: { EXACT_CAPS_WASM: '0' }, changed: ['contract/cli/src/gallery.rs'] }).counted).toBe('wasm32: skipped (EXACT_CAPS_WASM=0)');
});
