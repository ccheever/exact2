// scripts/wasm-check.mjs, which caps.mjs runs before every push: the crates a web app links, compiled for wasm32
// when a change touches Rust. The compile itself is the lane's (its wasm builds); here, what is checked, when, and
// what a failure says.
import { test, expect } from 'bun:test';
import { mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { report, warm, wasmCheck, wasmCrates } from './wasm-check.mjs';

const ROOT = resolve(import.meta.dir, '..');
const TARGET = mkdtempSync(join(tmpdir(), 'wasm-check-')); // a cold target directory, never compiled into
const FOUND = { crates: ['contract', 'exact-kernel', 'exact-web'], bins: [['exact-textflow', 'textflow-web']], dirs: ['contract/cli/', 'kernel/', 'host/web/', 'textflow/'], target: TARGET };

test('the crates checked are the libraries the web apps link for wasm32, the apps and vendored crates aside', () => {
  const { crates, bins, roots } = wasmCrates(ROOT);
  expect(bins).toEqual([['exact-textflow', 'textflow-web']]);
  expect(roots).toContain('apps/caltrain/web');
  // Harmony's crates build in their own workspace, outside the root's members (LLP 1153 D2.4).
  expect(roots).not.toContain('apps/harmony/app/web');
  // contract: G2's cfg(unix)-only name (5540abbfc0) broke it there; exact-web: the host every web app links
  for (const c of ['contract', 'exact-web', 'exact-runner', 'exact-kernel', 'exact-markdown-editor', 'exact-web-js-motion']) expect(crates).toContain(c);
  expect(crates.some((c) => c.endsWith('-web') && c !== 'exact-web' && c !== 'exact-js-web')).toBe(false);
  expect(crates).not.toContain('taffy');
}, 30_000);

test("warm is exact-web's own wasm32 debug fingerprint, not a sibling crate's or another profile's", () => {
  const dir = mkdtempSync(join(tmpdir(), 'wasm-check-warm-'));
  try {
    const prints = (profile) => join(dir, 'wasm32-unknown-unknown', profile, '.fingerprint');
    expect(warm(dir)).toBe(false);
    mkdirSync(join(prints('debug'), 'exact-web-js-motion-e5e559bbbf3de2f3'), { recursive: true });
    mkdirSync(join(prints('debug'), 'exact-web-geometry-a8f8e2a7e50310f2'));
    mkdirSync(join(prints('web'), 'exact-web-54f62345199d9191'), { recursive: true });
    expect(warm(dir)).toBe(false);
    mkdirSync(join(prints('debug'), 'exact-web-54f62345199d9191'));
    expect(warm(dir)).toBe(true);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('a cold target directory waits for Rust or a crate\'s own files; a warm one leaves it to Cargo; EXACT_CAPS_WASM=0 skips', () => {
  const at = (changed, extra = {}) => wasmCheck(ROOT, { env: {}, changed, dry: true, found: FOUND, ...extra }).counted;
  for (const changed of [[], ['docs/x.md', 'scripts/agent.mjs', 'Cargo.md', 'host/web/tests/resize-boot.test.mjs', 'host/web/agent.test.mjs']]) {
    expect(at(changed)).toBe('wasm32: no Rust changed since origin/main and no earlier check in this target directory, not checked');
    expect(at(changed, { hot: true })).toBe('wasm32: would check (Cargo decides what changed)');
  }
  // kernel/build.rs reads kernel/tables/schema.json; host/web includes its index.html
  for (const changed of [['kernel/tables/schema.json'], ['host\\web\\index.html'], ['.cargo/config.toml'], ['a/b.rs'], ['rust-toolchain.toml'], ['Cargo.lock']]) {
    expect(at(changed)).toBe("wasm32: would check (Rust or a crate's files changed since origin/main: 1)");
  }
  expect(at(null)).toBe('wasm32: would check (no origin/main to compare)');
  expect(wasmCheck(ROOT, { env: { CARGO_BUILD_BUILD_DIR: TARGET + '-elsewhere' }, changed: [], dry: true, found: { ...FOUND } }).counted).toMatch(/not checked/);
  expect(wasmCheck(ROOT, { env: { EXACT_CAPS_WASM: '0' }, changed: ['contract/cli/src/gallery.rs'] }).counted).toBe('wasm32: skipped (EXACT_CAPS_WASM=0)');
});

test('a failure says every diagnostic, a build script\'s cause beneath it, and a rerun for each check that failed', () => {
  const lib = ['--lib', '--keep-going', '-p', 'contract', '-p', 'exact-kernel', '-p', 'exact-web'], bin = ['-p', 'exact-textflow', '--bin', 'textflow-web'];
  const { problems, counted } = report([
    { args: lib, status: 101, stderr: [
      'contract/cli/src/gallery.rs:444:5: error[E0425]: cannot find value `made` in this scope: not found in this scope',
      'error: could not compile `contract` (lib) due to 1 previous error',
      'error: failed to run custom build command for `exact-kernel v0.1.0 (/x/kernel)`', '', 'Caused by:',
      '  process didn\'t exit successfully: `/x/build-script-build` (exit status: 101)', '  --- stderr',
      "  thread 'main' panicked at kernel/build.rs:12:5:", '  schema.json: expected value at line 3 column 1',
      'warning: build failed, waiting for other jobs to finish...'].join('\n') },
    { args: bin, status: 101, stderr: 'textflow/src/web.rs:9:1: error[E0433]: failed to resolve: use of undeclared crate `libc`\nerror: could not compile `exact-textflow` (bin "textflow-web") due to 1 previous error' },
  ], FOUND, 'Cargo decides what changed');
  expect(counted).toBe('wasm32: 3 crates and 1 binary the web apps link checked (Cargo decides what changed), 3 errors');
  expect(problems.length).toBe(1);
  const [{ message, detail }] = problems;
  expect(message).toContain('3 errors in contract, exact-kernel, exact-textflow.');
  expect(message).toContain('needs an arm for neither');
  expect(detail).toContain('gallery.rs:444:5: error[E0425]');
  expect(detail).toContain('schema.json: expected value at line 3 column 1');
  expect(detail).toContain('textflow/src/web.rs:9:1: error[E0433]');
  expect(detail).toContain('Rerun: cargo check --target wasm32-unknown-unknown --lib -p contract -p exact-kernel; cargo check --target wasm32-unknown-unknown -p exact-textflow --bin textflow-web');
  // One that could not run is said beside the others, not in their place
  const partial = report([{ args: lib, status: 0, stderr: '' }, { args: bin, thrown: 'cargo check exited null: killed' }], FOUND, 'x');
  expect(partial.problems[0].detail).toContain('could not run: cargo check exited null: killed');
});
