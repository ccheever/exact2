#!/usr/bin/env bun
import { cpSync, mkdirSync, readdirSync, writeFileSync, readFileSync } from 'node:fs';
import { createServer, connect } from 'node:net';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const [device, mode, payload = '[]', name = 'capture'] = process.argv.slice(2);
if (!device) throw new Error('Usage: bun kit/control.mjs UDID serve | flow JSON name');
const env = { ...process.env, DEVELOPER_DIR: '/Applications/Xcode.app/Contents/Developer' };
const run = cmd => { const p = Bun.spawnSync(cmd, { env, stdout: 'pipe', stderr: 'pipe' });
  if (p.exitCode) throw new Error(p.stderr.toString() + p.stdout.toString()); return p.stdout.toString().trim(); };
if (mode !== 'serve') {
  const state = JSON.parse(readFileSync(`${root}/build/control.json`, 'utf8'));
  if (state.device !== device) throw new Error('Device differs from server');
  const start = performance.now();
  const request = JSON.parse(payload.startsWith("@") ? readFileSync(payload.slice(1), "utf8") : payload);
  writeFileSync(`${root}/out/${name}-steps.json`, JSON.stringify(request, null, 2));
  const ask = steps => new Promise((resolve, reject) => {
    const socket = connect(state.port, '127.0.0.1'); let buf = '', settled = false;
    socket.on('connect', () => socket.write(JSON.stringify({role: 'client', steps}) + '\n'));
    socket.on('data', chunk => { buf += chunk; if (buf.includes('\n')) { settled = true; socket.end(); resolve(JSON.parse(buf.split('\n')[0])); } });
    socket.setTimeout(55000, () => { socket.destroy(); reject(new Error('Controller timed out')); });
    socket.on('close', () => { if (!settled) reject(new Error('Controller closed before replying')); });
    socket.on('error', reject);
  });
  let result = await ask(Array.isArray(request) ? request : request.steps ?? []);
  const deadline = performance.now() + 20000;
  while (request.expect && !result.tree?.includes(request.expect) && performance.now() < deadline) {
    await Bun.sleep(100); result = await ask([]);
  }
  writeFileSync(`${root}/out/${name}.json`, JSON.stringify(result, null, 2));
  console.log(JSON.stringify({ name, seconds: (performance.now() - start) / 1000, actionSeconds: result.seconds, error: result.error ?? result.extras?.error }));
  run(['xcrun', 'simctl', 'io', device, 'screenshot', `${root}/out/${name}.png`]);
  if (request.expect && !result.tree?.includes(request.expect)) throw new Error(`Missing ${request.expect}`);
  process.exit(result.error || result.extras?.error ? 1 : 0);
}
const dir = `${root}/build/control`, app = `${dir}/ArchiveDriver.app`, bundle = `${app}/PlugIns/ArchiveDriver.xctest`;
const platform = `${env.DEVELOPER_DIR}/Platforms/iPhoneSimulator.platform/Developer`;
mkdirSync(dir, { recursive: true });
const plist = (path, data) => { writeFileSync(`${path}.json`, JSON.stringify(data)); run(['plutil', '-convert', 'xml1', '-o', path, `${path}.json`]); };
cpSync(`${platform}/Library/Xcode/Agents/XCTRunner.app`, app, { recursive: true });
for (const [key, value] of Object.entries({ CFBundleExecutable: 'XCTRunner', CFBundleIdentifier: 'com.exact.probe.archivedriver', CFBundleName: 'ArchiveDriver' }))
  run(['plutil', '-replace', key, '-string', value, `${app}/Info.plist`]);
const frameworks = `${app}/Frameworks`;
mkdirSync(frameworks, { recursive: true });
for (const [from, pattern] of [['Library/Frameworks', /^(XCTest|XCUIAutomation|Testing|_Testing_Foundation)\.framework$/],
  ['Library/PrivateFrameworks', /^(XCT|XCUnit)/], ['usr/lib', /^(libXCTestSwiftSupport|lib_TestingInterop)\.dylib$/]])
  for (const name of readdirSync(`${platform}/${from}`).filter(n => pattern.test(n)))
    cpSync(`${platform}/${from}/${name}`, `${frameworks}/${name}`, { recursive: true });
mkdirSync(bundle, { recursive: true });
run(['xcrun', '--sdk', 'iphonesimulator', 'swiftc', '-emit-library', '-O', '-module-name', 'ArchiveDriver',
  '-target', 'arm64-apple-ios27.0-simulator', '-F', `${platform}/Library/Frameworks`, '-F', `${platform}/Library/PrivateFrameworks`,
  '-I', `${platform}/usr/lib`, '-L', `${platform}/usr/lib`, '-framework', 'XCTest', '-framework', 'XCUIAutomation',
  '-Xlinker', '-rpath', '-Xlinker', '@executable_path/Frameworks', '-module-cache-path', `${dir}/cache`,
  '-o', `${bundle}/ArchiveDriver`, `${root}/kit/Control.swift`]);
plist(`${bundle}/Info.plist`, { CFBundleExecutable: 'ArchiveDriver', CFBundleIdentifier: 'com.exact.probe.archivedriver.tests',
  CFBundleName: 'ArchiveDriver', CFBundlePackageType: 'BNDL', CFBundleVersion: '1', CFBundleSupportedPlatforms: ['iPhoneSimulator'] });
for (const path of [bundle, ...readdirSync(frameworks).map(n => `${frameworks}/${n}`)]) run(['codesign', '--force', '--sign', '-', path]);
run(['codesign', '--force', '--sign', '-', '--entitlements', `${app}/RunnerEntitlements.plist`, app]);
let runner = null, pending = null;
const server = createServer(socket => {
  let role = null, buf = '';
  socket.on('data', chunk => {
    buf += chunk;
    while (buf.includes('\n')) {
      const at = buf.indexOf('\n'), line = buf.slice(0, at); buf = buf.slice(at + 1);
      const data = JSON.parse(line);
      if (data.role === 'runner') { runner = socket; role = 'runner'; console.log('READY ' + JSON.stringify(data)); }
      else if (data.role === 'client') {
        if (!runner || pending) { socket.end(JSON.stringify({error:'Runner unavailable or busy'}) + '\n'); continue; }
        pending = socket; runner.write(JSON.stringify(data.steps) + '\n');
      } else if (role === 'runner' && pending) { pending.end(line + '\n'); pending = null; }
    }
  });
});
await new Promise(r => server.listen(0, '127.0.0.1', r));
const port = server.address().port;
plist(`${dir}/drive.xctestrun`, { ArchiveDriver: { IsUITestBundle: true, TestHostPath: app,
  TestBundlePath: '__TESTHOST__/PlugIns/ArchiveDriver.xctest', UITargetAppPath: app,
  TestingEnvironmentVariables: { PROBE_PORT: String(port), PROBE_APP_ID: process.env.PROBE_APP_ID ?? "com.exact.probe.archive" } }, __xctestrun_metadata__: { FormatVersion: 1 } });
const p = Bun.spawn(['xcodebuild', 'test-without-building', '-derivedDataPath', `${root}/build/derived`, '-resultBundlePath', `${root}/build/results-${process.pid}.xcresult`, '-xctestrun', `${dir}/drive.xctestrun`, '-destination', `id=${device}`,
  '-parallel-testing-enabled', 'NO', '-collect-test-diagnostics', 'never', '-only-testing:ArchiveDriver/ArchiveDriver/testDrive'],
  { env, stdout: 'inherit', stderr: 'inherit' });
writeFileSync(`${root}/build/control.json`, JSON.stringify({device, port, serverPid:process.pid, xcodebuildPid:p.pid}));
console.log('START ' + JSON.stringify({device, port, serverPid:process.pid, xcodebuildPid:p.pid}));
await p.exited;
server.close();
