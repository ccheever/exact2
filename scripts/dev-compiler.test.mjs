import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { compilerCommand } from './dev-compiler.mjs';

test('native compiler captures validate real Cargo sources, declared inputs, environment and executable bytes', async () => {
  const dir = realpathSync(mkdtempSync(resolve(tmpdir(), 'exact-native-compiler-')));
  const workspace = resolve(dir, 'source'), target = resolve(dir, 'target'), bin = resolve(dir, 'bin'), count = resolve(dir, 'build-count');
  const quote = value => "'" + value.replaceAll("'", "'\\''") + "'";
  try {
    for (const path of ['source/src','source/support/src','source/watched','bin']) mkdirSync(resolve(dir, path), {recursive:true});
    writeFileSync(resolve(workspace, 'rust-toolchain.toml'), readFileSync(new URL('../rust-toolchain.toml', import.meta.url)));
    writeFileSync(resolve(workspace, 'Cargo.toml'), `[workspace]\nmembers=["support"]\nresolver="2"\n[package]\nname="fixture-web"\nversion="0.1.0"\nedition="2021"\n[dependencies]\nsupport={path="support"}\n`);
    writeFileSync(resolve(workspace, 'support/Cargo.toml'), '[package]\nname="support"\nversion="0.1.0"\nedition="2021"\n');
    const support = resolve(workspace, 'support/src/lib.rs'), message = resolve(workspace, 'message.txt');
    writeFileSync(support, 'pub fn value() -> &\'static str { "before" }');
    writeFileSync(message, 'first');
    writeFileSync(resolve(workspace, 'src/main.rs'), 'include!(concat!(env!("OUT_DIR"),"/values.rs")); fn main() { println!("{}-{}-{}-{}",support::value(),MESSAGE,ENVIRONMENT,OPTIONAL); }');
    writeFileSync(resolve(workspace, 'build.rs'), `fn main() {
      println!("cargo:rerun-if-changed=message.txt");
      println!("cargo:rerun-if-changed=watched");
      println!("cargo:rerun-if-env-changed=COMPILER_FIXTURE_VALUE");
      let message=std::fs::read_to_string("message.txt").unwrap();
      let environment=std::env::var("COMPILER_FIXTURE_VALUE").unwrap();
      let optional=std::fs::read_to_string("watched/optional.txt").unwrap_or_default();
      let text=format!("const MESSAGE:&str={message:?}; const ENVIRONMENT:&str={environment:?}; const OPTIONAL:&str={optional:?};");
      std::fs::write(std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("values.rs"),text).unwrap();
    }`);
    const cargo = Bun.which('cargo'); assert.ok(cargo);
    writeFileSync(resolve(bin, 'cargo'), `#!/bin/sh\nif [ "$1" = build ]; then\n  printf x >> ${quote(count)}\n  case "$*" in\n    *"--bin fixture-web"*) kind=native ;;\n    *) kind=bootstrap ;;\n  esac\n  if [ -n "$COMPILER_TEST_STALL" ] && [ "$COMPILER_TEST_STALL_KIND" = "$kind" ]; then printf ready > "$COMPILER_TEST_STALL"; sleep 30; fi\nfi\nexec ${quote(cargo)} "$@"\n`, {mode:0o755});
    const env = {...process.env, PATH:bin + ':' + process.env.PATH, COMPILER_FIXTURE_VALUE:'one'};
    const request = {workspace, target, package:'fixture-web'};
    const builds = () => readFileSync(count, 'utf8').length;
    const run = async (extra = {}) => {
      const command = await compilerCommand(request, {...env, ...extra});
      const child = spawnSync(command.command, command.args, {cwd:command.cwd, env:{...env,...extra}, encoding:'utf8'});
      assert.equal(child.status, 0, child.stderr); return {text:child.stdout.trim(), command};
    };
    const first = await run(); assert.equal(first.text, 'before-first-one-');
    const cold = builds(); assert.equal(cold, 2, 'discovery is followed by an independently rebuilt verification capture');
    assert.equal((await run()).text, first.text); assert.equal(builds(), cold, 'warm startup invokes no Cargo build');
    const stamp = statSync(support);
    writeFileSync(support, 'pub fn value() -> &\'static str { "after!" }'); utimesSync(support, stamp.atime, stamp.mtime);
    assert.equal((await run()).text, 'after!-first-one-'); assert.equal(builds(), cold + 1, 'same-mtime transitive edits rebuild actual code');
    const inputStamp = statSync(message); writeFileSync(message, 'other'); utimesSync(message, inputStamp.atime, inputStamp.mtime);
    assert.equal((await run()).text, 'after!-other-one-'); assert.equal(builds(), cold + 2);
    writeFileSync(resolve(workspace, 'watched/optional.txt'), 'added');
    assert.equal((await run()).text, 'after!-other-one-added'); assert.equal(builds(), cold + 3, 'declared directory additions invalidate');
    const changed = await run({COMPILER_FIXTURE_VALUE:'two'});
    assert.equal(changed.text, 'after!-other-two-added'); assert.equal(builds(), cold + 4, 'declared environment invalidates');
    writeFileSync(changed.command.command, 'damaged');
    assert.equal((await run({COMPILER_FIXTURE_VALUE:'two'})).text, changed.text); assert.equal(builds(), cold + 5, 'damaged captured bytes are rebuilt');
    mkdirSync(resolve(workspace, '.cargo')); writeFileSync(resolve(workspace, '.cargo/config.toml'), '[build]\njobs=1\n');
    const fallback = await compilerCommand(request, env);
    assert.equal(fallback.command, 'cargo'); assert.ok(fallback.args.includes('run'), 'custom configurations remain Cargo-owned');
    rmSync(resolve(workspace, '.cargo'), {recursive:true});
    writeFileSync(support, 'pub fn value() -> &\'static str { "again!" }');
    for (const kind of ['bootstrap', 'native']) {
      const marker = resolve(dir, 'building-' + kind);
      const child = spawn(process.execPath, [new URL('./dev-compiler.mjs', import.meta.url).pathname, JSON.stringify(request)],
        {cwd:workspace, env:{...env, COMPILER_TEST_STALL:marker, COMPILER_TEST_STALL_KIND:kind}, detached:true, stdio:'ignore'});
      const closed = new Promise((ok, fail) => { child.once('error', fail); child.once('close', ok); });
      try {
        const deadline = Date.now() + 20000;
        while (!existsSync(marker) && Date.now() < deadline && child.exitCode === null) await new Promise(ok => setTimeout(ok, 20));
        assert.ok(existsSync(marker), `${kind} acquired its build lock before cancellation`);
      } finally { try { process.kill(-child.pid, 'SIGKILL'); } catch {} await closed; }
    }
    assert.equal((await run({COMPILER_FIXTURE_VALUE:'two'})).text, 'again!-other-two-added', 'SIGKILL releases build ownership for the next launch');
  } finally { rmSync(dir, {recursive:true, force:true}); }
}, 120000);
