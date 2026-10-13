// How `build.mjs --test` runs the Swift host tests once they are built:
// on this Mac, a test class to an `xctest` process; on a simulator, the
// `.xctestrun` xcodebuild wrote, without the Thread Performance Checker.
import { spawn, spawnSync } from 'node:child_process';
import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { availableParallelism } from 'node:os';
import { resolve } from 'node:path';

const read = (cmd, args, opts = {}) => spawnSync(cmd, args, { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024, ...opts });

/** The Swift host tests on this Mac, built under `scratch`: each test class
 *  in an `xctest` process of its own, as many at once as there are cores, so
 *  a crash ends its class and not the run. The classes whose files use what
 *  every process on the Mac shares (the general pasteboard, the active app,
 *  the mouse, a capture of the screen) share one process beside them, one
 *  class after another. A window's key status and first responder belong
 *  to its own process. */
export async function macTests(pkg, scratch, env) {
  const listed = read('swift', ['test', '--skip-build', '--list-tests', '--scratch-path', scratch], { cwd: pkg, env });
  if (listed.status !== 0) throw new Error(`swift test --list-tests failed: ${listed.stderr}`);
  const classes = [...new Set(listed.stdout.match(/^ExactKitTests\.\w+(?=\/)/gm) ?? [])];
  const macWide = /NSPasteboard\.general|\bactivate\(|NSEvent\.mouseLocation|CGWindowListCreateImage/;
  const shared = new Set();
  for (const file of readdirSync(resolve(pkg, 'tests/ExactKitTests')).filter(f => f.endsWith('.swift'))) {
    const source = readFileSync(resolve(pkg, 'tests/ExactKitTests', file), 'utf8');
    if (macWide.test(source)) for (const [, name] of source.matchAll(/class (\w+)\s*:\s*XCTestCase/g)) shared.add(`ExactKitTests.${name}`);
  }
  const groups = [classes.filter(c => shared.has(c)), ...classes.filter(c => !shared.has(c)).map(c => [c])].filter(g => g.length);
  const bundle = resolve(read('swift', ['build', '--show-bin-path', '--scratch-path', scratch], { cwd: pkg, env }).stdout.trim(), 'ExactKitTests.xctest');
  const failed = [];
  const one = (group) => new Promise((done) => {
    const child = spawn('xcrun', ['xctest', '-XCTest', group.join(','), bundle], { cwd: pkg, env, stdio: ['ignore', 'pipe', 'pipe'] });
    let out = '';
    child.stdout.on('data', (d) => { out += d; });
    child.stderr.on('data', (d) => { out += d; });
    child.on('error', (e) => { out += `${e.message}\n`; });
    child.on('close', (status, signal) => {
      process.stdout.write(out);
      if (status !== 0) failed.push(`${group.join(', ')} (xctest ${signal ?? `exited ${status}`})`);
      done();
    });
  });
  // One queue, several takers: `next` is read and advanced with no await between.
  let next = 0;
  const taker = async () => { while (next < groups.length) await one(groups[next++]); };
  await Promise.all(Array.from({ length: Math.min(groups.length, availableParallelism()) }, taker));
  console.log(`host/apple: ${classes.length} test classes in ${groups.length} processes, ${failed.length} failed`);
  if (failed.length) throw new Error(`swift tests failed:\n  ${failed.join('\n  ')}`);
}

/** The newest `.xctestrun` `build-for-testing` wrote in `products`, as
 *  `exact-tests.xctestrun` beside it without the Thread Performance Checker:
 *  the scheme Xcode makes for the package inserts it (libRPAC), and it
 *  writes a symbolicated backtrace at every wait on a thread of lower
 *  priority, which is most of the button tests' time, and fails no test.
 *  The Main Thread Checker stays. Returns the new file's path. */
export function withoutPerformanceChecker(products) {
  const built = readdirSync(products).filter(f => f.endsWith('.xctestrun') && f !== 'exact-tests.xctestrun')
    .map(f => resolve(products, f)).sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0];
  if (!built) throw new Error(`no .xctestrun in ${products}`);
  const converted = read('plutil', ['-convert', 'json', '-o', '-', built]);
  if (converted.status !== 0) throw new Error(`plutil ${built}: ${converted.stderr}`);
  const plan = JSON.parse(converted.stdout);
  const strip = (o) => {
    if (!o || typeof o !== 'object') return;
    for (const [key, value] of Object.entries(o)) {
      if (!/^(Testing)?EnvironmentVariables$/.test(key)) { strip(value); continue; }
      for (const name of Object.keys(value)) if (name.startsWith('PERFC_')) delete value[name];
      const inserted = (value.DYLD_INSERT_LIBRARIES ?? '').split(':').filter(l => l && !l.endsWith('/libRPAC.dylib'));
      if (inserted.length) value.DYLD_INSERT_LIBRARIES = inserted.join(':'); else delete value.DYLD_INSERT_LIBRARIES;
    }
  };
  strip(plan);
  const xctestrun = resolve(products, 'exact-tests.xctestrun'), json = `${xctestrun}.json`;
  writeFileSync(json, JSON.stringify(plan));
  const written = read('plutil', ['-convert', 'xml1', '-o', xctestrun, json]);
  if (written.status !== 0) throw new Error(`plutil ${json}: ${written.stderr}`);
  return xctestrun;
}
