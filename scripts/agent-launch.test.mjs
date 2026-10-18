// The drive's launch facts (agent-launch.mjs `launchFacts`, LLP 1027.000.000 D3), and the browser it finds; the fixed
// defaults and overrides are host/web/agent.test.mjs's "launch setup" test.
import { test, expect } from 'bun:test';
import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { Cdp, chromium, launchFacts, launchEnvironment, parseFlags, launchLog, launchFailure, webPort } from './agent-launch.mjs';
import { PassThrough } from 'node:stream';

test('drive clock warnings compare explicit and default epochs with the actual backend clock', async () => {
  const {backendClock, backendClockNote, driveNotes} = await import('./agent-test.mjs');
  const epoch = Date.parse('2026-01-01T00:00:00Z');
  expect(backendClockNote(epoch, epoch + 86_400_000, 'http://backend')).toBe(null);
  expect(backendClockNote(epoch, epoch - 86_400_001, 'http://backend')).toContain('differs from the backend clock');
  expect(backendClockNote(epoch, null, 'http://backend')).toBe(null);
  const server = Bun.serve({port:0,fetch:()=>Response.json({storeId:'test',now:epoch + 86_400_001})});
  const say = console.error, warnings = [];
  try {
    expect(await backendClock(server.url.origin)).toBe(epoch + 86_400_001);
    console.error = line => warnings.push(line);
    const notes = driveNotes({launchFacts:{epoch}}, {host:'web',storage:'test',origins:[server.url.origin]});
    await notes('state'); await notes('state');
    expect(warnings.length).toBe(1);
    expect(warnings[0]).toContain('differs from the backend clock');
  }
  finally { console.error = say; server.stop(true); }
});

test('bare fresh refusals explain roster seeding only when a deny-insert roster exists', async () => {
  const {rosterRefusalNote} = await import('./agent-test.mjs');
  const schema = 'table owners:\n  person: principal\n  insert <- deny\n';
  const refused = new Map([['approve','NOT_OWNER: no owner']]);
  expect(rosterRefusalNote({},schema,refused)).toContain('every privileged operation refused after `fresh`: seed roster tables');
  expect(rosterRefusalNote({seed:'seed.json'},schema,refused)).toBe(null);
  expect(rosterRefusalNote(null,schema,refused)).toBe(null);
  expect(rosterRefusalNote({},schema,new Map())).toBe(null);
  expect(rosterRefusalNote({},'table owners:\n    person: principal\n    insert <- deny -- roster\n',refused)).toContain('seed roster tables');
  expect(rosterRefusalNote({},'table owners:\r\n    insert <- deny -- roster\r\n',refused)).toContain('seed roster tables');
  expect(rosterRefusalNote({},'',refused)).toBe(null);
  expect(rosterRefusalNote({},schema,new Map([['approve','invalid reply shape']]))).toBe(null);
  expect(rosterRefusalNote({},schema,new Map([['approve','E_OFFLINE: offline']]))).toBe(null);
});

test('CDP reports transport closure once to event waiters without a pending request', () => {
  for (const [stream, event, reason] of [
    ['output', 'end', 'the DevTools pipe closed'],
    ['output', 'error', 'the DevTools pipe failed: read failed'],
    ['input', 'error', 'the DevTools pipe failed: write failed'],
  ]) {
    const input = new PassThrough(), output = new PassThrough(), cdp = new Cdp(input, output), observed = [];
    try {
      cdp.closeListeners.add(why => observed.push(why));
      ({ input, output })[stream].emit(event, new Error(stream === 'output' ? 'read failed' : 'write failed'));
      cdp.fail('later failure');
      expect(cdp.pending.size).toBe(0);
      expect(cdp.closed).toBe(reason);
      expect(observed).toEqual([reason]);
      expect(cdp.closeListeners.size).toBe(0);
    } finally { input.destroy(); output.destroy(); }
  }
});
const engineVersion = () => '1.2.3';
function stampedWasm(stamp = {format: 1, version: engineVersion(), capabilities: ['sync-generation-v1']}) {
  const leb = n => { const out = []; do { const b = n & 127; n >>>= 7; out.push(b | (n ? 128 : 0)); } while (n); return Buffer.from(out); };
  const name = Buffer.from('snapback4.engine');
  const body = Buffer.concat([leb(name.length), name, Buffer.isBuffer(stamp) ? stamp : Buffer.from(JSON.stringify(stamp))]);
  return Buffer.concat([Buffer.from([0,97,115,109,1,0,0,0,0]), leb(body.length), body]);
}
import { timeReporter } from '../host/web/navigation.js';

// LLP 1107 D1: `--reader` is a flag of the drive, said to the Android carrier as the variable it reads
// (agent-android.mjs `androidBinary`), and never an op.
test('--reader names the Canvas reader to the Android carrier', () => {
  const before = process.env.EXACT_ANDROID_READER;
  try {
    delete process.env.EXACT_ANDROID_READER;
    expect(parseFlags(['android', '--app', 'shelf', 'tree']).flags.reader).toBeUndefined();
    expect(process.env.EXACT_ANDROID_READER).toBeUndefined();
    const { flags, rest } = parseFlags(['android', '--reader', '--app', 'shelf', 'tree']);
    expect(flags.reader).toBe(true);
    expect(rest).toEqual(['android', 'tree']);
    expect(process.env.EXACT_ANDROID_READER).toBe('1');
  } finally {
    if (before === undefined) delete process.env.EXACT_ANDROID_READER; else process.env.EXACT_ANDROID_READER = before;
  }
});

// App farm round 2: a drive against a live backend opts into the machine's clock, read once at launch; the host is
// told those milliseconds, so `state.time.epochAtZero` is the launch instant and `clock +N` moves the date from it.
test('--epoch now is the machine clock read once at launch, reported as that instant', () => {
  const {flags} = parseFlags(['web', '--epoch', 'now', 'state']);
  expect(flags.epoch).toBe('now');
  const before = Date.now(), facts = launchFacts(flags), after = Date.now();
  expect(facts.epoch).toBeGreaterThanOrEqual(before);
  expect(facts.epoch).toBeLessThanOrEqual(after);
  expect(launchEnvironment(facts).EXACT_AGENT_EPOCH).toBe(String(facts.epoch));
  // A relaunch from the environment the driver hands a host keeps the instant it read, not a later one.
  expect(launchFacts({env:launchEnvironment(facts)}).epoch).toBe(facts.epoch);
  const fromEnv = launchFacts({env:{EXACT_AGENT_EPOCH:'now'}}).epoch;
  expect(Math.abs(fromEnv - Date.now())).toBeLessThan(1000);
  const time = timeReporter(new URLSearchParams({agent:'1', ...facts}), new Proxy({}, {get() { throw new Error('agent read the platform'); }}));
  expect(time(0)).toEqual([facts.epoch, 0]);
  expect(time(60000)).toEqual([facts.epoch, 0]);
});

test('Windows browser discovery accepts an installed Chrome and explicit executable paths with spaces', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact browser paths-'));
  try {
    const chrome = resolve(dir, 'Google/Chrome/Application/chrome.exe');
    mkdirSync(dirname(chrome), {recursive:true});
    writeFileSync(chrome, '');
    chmodSync(chrome, 0o755);
    expect(chromium({ProgramFiles:dir}, 'win32').executable).toBe(chrome);
    expect(chromium({CHROME:chrome}, 'win32').unavailable).toBe(null);
    expect(chromium({CHROME:resolve(dir, 'missing.exe')}, 'win32').unavailable).toBeTruthy();
  } finally { rmSync(dir, {recursive:true, force:true}); }
});
// App farm wave 1 (21 builds): a builder on a shared machine keeps every listener in its assigned range.
test('a web drive takes its port from --port, else EXACT_PORT, and refuses anything but a port', () => {
  expect(parseFlags(['web', '--port', '50821', 'tree']).flags.port).toBe('50821');
  expect(webPort('50821', {EXACT_PORT: '50822'})).toBe(50821);
  expect(webPort(undefined, {EXACT_PORT: '50822'})).toBe(50822);
  expect(webPort(undefined, {})).toBe(null);
  expect(() => webPort('99999', {})).toThrow('--port 99999: a port number');
  expect(() => webPort(undefined, {EXACT_PORT: 'x'})).toThrow('EXACT_PORT x: a port number');
  expect(parseFlags(['web', '--storage', 'proof', '--keep-storage', 'tree']).flags.keepStorage).toBe(true);
  expect(parseFlags(['web', '--storage', 'proof', '--reset-storage', 'tree']).flags.resetStorage).toBe(true);
  expect(() => parseFlags(['web', '--keep-storage'])).toThrow('need --storage');
  expect(() => parseFlags(['web', '--storage', 'proof', '--keep-storage', '--reset-storage'])).toThrow('mutually exclusive');
});

// App farm 082: Chrome's fatal line came before a native stack the tail kept instead.
test("a Chrome that dies at launch is reported by its first fatal line, before the stack's tail", () => {
  const log = launchLog();
  log.add('[1:1:1011/000000.000:FATAL:process_singleton_posix.cc(313)] Socket path too long: /long/.org.chromium.Chromium.x/SingletonSocket');
  for (let i = 0; i < 20; i++) log.add(`  r${i}: 0000000000000006`);
  const message = launchFailure(new Error('the DevTools pipe closed'), log, {alive: false, chrome: '/c', env: {TMPDIR: '/long'}}).message;
  expect(message).toContain('FATAL:process_singleton_posix.cc(313)] Socket path too long');
  expect(message).toContain('r19: 0000000000000006');
  expect(message).toContain('give the drive a shorter TMPDIR');
  expect(launchFailure(new Error('closed'), launchLog(), {alive: false, chrome: '/c'}).message).toContain('(it printed nothing)\nSet CHROME');
});

// App farm wave 1 (four builds): `test web` against no `snapback4 dev` failed as data mismatches. The loopback origins
// app.ts fetches are asked first; a Snapback app's run is refused naming the origin, any other app's is told.
test('a web test run asks the backend its app fetches first', async () => {
  const { backendPreflight, loopbackOrigins, fetchOrigins } = await import('./agent-test.mjs');
  const { createServer } = await import('node:net');
  const { mkdtempSync, mkdirSync, rmSync, writeFileSync } = await import('node:fs');
  const { tmpdir } = await import('node:os');
  const { resolve } = await import('node:path');
  const dist = mkdtempSync(resolve(tmpdir(), 'exact-preflight-')), server = createServer().listen(0, '127.0.0.1');
  try {
    await new Promise((ok) => server.once('listening', ok));
    const up = `http://127.0.0.1:${server.address().port}`, down = 'http://127.0.0.1:1';
    mkdirSync(resolve(dist, '.gen'));
    const grants = (...lines) => writeFileSync(resolve(dist, '.gen/admission-data.js'), `export const tsGrantSet=createGrantSet(${JSON.stringify({ entries: lines.map((l, i) => [i, l, [], null]) })});`);
    grants(`net.fetch ${up}`, `net.fetch ${down}/api`, 'net.fetch https://example.com', 'sqlite.open app:/data');
    expect(loopbackOrigins(dist)).toEqual([up, down]);
    expect(fetchOrigins(dist)).toEqual([up, down, 'https://example.com']);
    const snapback = { dir: dist, manifest: { typescript: { sources: { snapback4: '../../exact2/snapback4/ts' } } } }, said = [];
    const { deviceHash } = await import('../snapback4/web/device-build.mjs');
    const wasm = stampedWasm();
    mkdirSync(resolve(dist, 'assets'));
    writeFileSync(resolve(dist, 'assets/snapback4.wasm'), wasm);
    writeFileSync(resolve(dist, 'assets/snapback4.wasm.json'), JSON.stringify({engineVersion: engineVersion(), sha256: deviceHash(wasm)}));
    await expect(backendPreflight(snapback, dist)).rejects.toThrow(`no backend at ${down} (source: built net.fetch grant (rebuild after changing the backend selection)): nothing listens there`);
    await backendPreflight({ manifest: {} }, dist, (line) => said.push(line));
    expect(said).toEqual([`note: no backend at ${down} (source: built net.fetch grant): nothing listens there, and app.ts fetches it (a net.fetch grant); a test that loads from it fails`]);
    grants(`net.fetch ${up}`);
    await backendPreflight(snapback, dist);
    expect(loopbackOrigins(resolve(dist, 'none'))).toEqual([]);
  } finally { server.close(); rmSync(dist, { recursive: true, force: true }); }
});

test('device preflight refuses server skew, stale provenance and empty wasm before a drive', async () => {
  const { devicePreflight, deviceHash, readDeviceStamp, validateDevice } = await import('../snapback4/web/device-build.mjs');
  const { createServer } = await import('node:http');
  const { mkdtempSync, mkdirSync, rmSync, writeFileSync } = await import('node:fs');
  const { tmpdir } = await import('node:os');
  const { resolve } = await import('node:path');
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-device-'));
  const app = {dir, manifest: {typescript: {sources: {snapback4: '../../exact2/snapback4/ts'}}}};
  let version = '1.2.4', stamped = false;
  const server = createServer((req, res) => {
    res.setHeader('content-type', 'application/json');
    res.end(JSON.stringify(req.url === '/dev/status' ? {storeId: 'dev'} : {device_compatibility: {server_version: version, minimum_version: version, capabilities: stamped ? ['future'] : ['sync-generation-v1']}}));
  }).listen(0, '127.0.0.1');
  try {
    await new Promise(ok => server.once('listening', ok));
    const origin = `http://127.0.0.1:${server.address().port}`;
    const path = resolve(dir, 'assets/snapback4.wasm'), wasm = stampedWasm();
    mkdirSync(resolve(dir, 'assets'));
    writeFileSync(path, Buffer.from([0,97,115,109,1,0,0,0]));
    await expect(devicePreflight(app, [origin])).rejects.toThrow('snapback4.engine');
    writeFileSync(path, wasm);
    writeFileSync(path + '.json', JSON.stringify({engineVersion: engineVersion(), sha256: deviceHash(wasm)}));
    await expect(devicePreflight(app, [origin + '/'])).rejects.toThrow(/E_DEVICE_VERSION.*server.*1.2.4.*snapback --rebuild/);
    version = engineVersion();
    writeFileSync(resolve(dir, 'app.ts'), "const headers = { 'X-Snapback-Device': 'old' };\n");
    const warnings = [], warn = console.warn;
    console.warn = message => warnings.push(message);
    try { await devicePreflight(app, [origin]); } finally { console.warn = warn; }
    expect(warnings.join('\n')).toMatch(/obsolete x-snapback-device shim in app.ts.*device-to-server leg/);
    version = 'x';
    await expect(devicePreflight(app, [origin])).rejects.toThrow('invalid server device_compatibility');
    version = '1.2.2'; // compatible skew above floor is allowed
    await devicePreflight(app, [origin]);
    stamped = true;
    await expect(devicePreflight(app, [origin])).rejects.toThrow('lacks future');
    writeFileSync(path + '.json', JSON.stringify({engineVersion: version, sha256: 'wrong'}));
    expect(readDeviceStamp(path).version).toBe(engineVersion()); // sidecar is diagnostic only
    writeFileSync(path, stampedWasm({format: 1, version: '4294967296.0.0', capabilities: ['sync-generation-v1']}));
    expect(() => readDeviceStamp(path)).toThrow('invalid snapback4.engine identity');
    writeFileSync(path, stampedWasm(null));
    expect(() => readDeviceStamp(path)).toThrow('invalid snapback4.engine identity');
    for (const payload of [
      Buffer.from('{"format":0,"format":1,"version":"1.2.3","capabilities":[]}'),
      Buffer.from('{"format":1.0,"version":"1.2.3","capabilities":[]}'),
      Buffer.concat([Buffer.from([0xef,0xbb,0xbf]), Buffer.from('{"format":1,"version":"1.2.3","capabilities":[]}')]),
      Buffer.from(JSON.stringify({format: 1, version: '1.2.3', capabilities: [String.fromCharCode(0xd800)]})),
      Buffer.concat([Buffer.from('{"format":1,"version":"1.2.3","capabilities":["'), Buffer.from([0xff]), Buffer.from('"]}')]),
    ]) {
      writeFileSync(path, stampedWasm(payload));
      expect(() => readDeviceStamp(path)).toThrow('E_DEVICE_VERSION');
    }
    const duplicate = Buffer.concat([wasm, wasm.subarray(8)]);
    writeFileSync(path, duplicate);
    expect(() => readDeviceStamp(path)).toThrow('exactly one');
    expect(() => validateDevice(Buffer.alloc(0), 'wasm-opt output')).toThrow('empty or invalid wasm');
  } finally { server.close(); rmSync(dir, {recursive: true, force: true}); }
});

test('a store named .locks cannot remove another profile ownership lock', async () => {
  const { exclusiveWebStore, driveStore } = await import('./agent-launch.mjs');
  const { resolveApp } = await import('./app.mjs');
  const { mkdtempSync, rmSync } = await import('node:fs');
  const { tmpdir } = await import('node:os');
  const { resolve } = await import('node:path');
  const cache = mkdtempSync(resolve(tmpdir(), 'exact-profile-lock-'));
  const options = {app: 'caltrain', browser: 'chrome', storage: 'proof', env: {XDG_CACHE_HOME: cache}};
  let owner, named, competitor;
  try {
    owner = await exclusiveWebStore(options, async () => ({close: async () => {}}));
    named = await exclusiveWebStore({...options, storage: '.locks'}, async () => {
      rmSync(driveStore(resolveApp(options.app), '.locks', options.env).profile, {recursive: true, force: true});
      return {close: async () => {}};
    });
    let launched = false;
    const attempt = exclusiveWebStore(options, async () => { launched = true; return {close: async () => {}}; });
    attempt.then(carrier => { competitor = carrier; }, () => {});
    await expect(attempt).rejects.toThrow('profile lock failed');
    expect(launched).toBe(false);
  } finally {
    await competitor?.close(); await named?.close(); await owner?.close(); rmSync(cache, {recursive: true, force: true});
  }
});

test('Firefox and WebKit refuse missing and skewed devices before launching their carrier', async () => {
  const { open } = await import('./agent.mjs');
  const { createApp, wireSnapback } = await import('../game/new.mjs');
  const { deviceHash } = await import('../snapback4/web/device-build.mjs');
  const { mkdtempSync, mkdirSync, rmSync, writeFileSync } = await import('node:fs');
  const { tmpdir } = await import('node:os');
  const { resolve } = await import('node:path');
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-carrier-preflight-'));
  const dir = resolve(parent, 'app'), dist = resolve(parent, 'dist'), before = process.env.EXACT_APP_DIR;
  const server = Bun.serve({hostname: '127.0.0.1', port: 0, fetch: request => Response.json(new URL(request.url).pathname === '/dev/status' ? {storeId: 'dev'} : {device_compatibility: {server_version: '1.2.4', minimum_version: '1.2.4', capabilities: []}})});
  try {
    createApp(dir); wireSnapback(dir, {build: false, port: server.port});
    process.env.EXACT_APP_DIR = dir;
    mkdirSync(resolve(dist, '.gen'), {recursive: true});
    writeFileSync(resolve(dist, '.gen/admission-data.js'), `export const tsGrantSet=createGrantSet(${JSON.stringify({entries: [[0, `net.fetch http://127.0.0.1:${server.port}`, [], null]]})});`);
    for (const browser of ['firefox', 'webkit']) await expect(open({host: 'web', browser, webDist: dist})).rejects.toThrow('device is missing');
    const path = resolve(dir, 'assets/snapback4.wasm'), wasm = stampedWasm();
    mkdirSync(resolve(dir, 'assets'), {recursive: true}); writeFileSync(path, wasm);
    writeFileSync(path + '.json', JSON.stringify({engineVersion: '0.0.1', sha256: deviceHash(wasm)}));
    for (const browser of ['firefox', 'webkit']) await expect(open({host: 'web', browser, webDist: dist})).rejects.toThrow(/E_DEVICE_VERSION.*server/);
  } finally {
    if (before === undefined) delete process.env.EXACT_APP_DIR; else process.env.EXACT_APP_DIR = before;
    server.stop(true); rmSync(parent, {recursive: true, force: true});
  }
});

// `fresh` (the author's ruling of 2026-10-12, via the sb4 app farm): before a test, each snapback4 dev server the app
// fetches is emptied through `POST /dev/reset`, seeded from the line's file; a server without the door (404, as 0.4.22
// answers) refuses the test, saying what to do. Stub servers stand in for snapback4 dev, old and new.
test('fresh empties each snapback4 dev server through its reset door, and refuses one without it', async () => {
  const { resetBackends, freshRows } = await import('./agent-test.mjs');
  const { createServer } = await import('node:http');
  const { mkdtempSync, rmSync, writeFileSync } = await import('node:fs');
  const { tmpdir } = await import('node:os');
  const { resolve } = await import('node:path');
  const posted = [];
  const serve = (handler) => new Promise((ok) => { const server = createServer(handler).listen(0, '127.0.0.1', () => ok(server)); });
  const json = (res, status, body) => { res.writeHead(status, { 'content-type': 'application/json' }); res.end(JSON.stringify(body)); };
  let generation = 1;
  const door = await serve((req, res) => {
    if (req.url === '/dev/status') return json(res, 200, { storeId: 'a1', generation, personas: ['alice'] });
    if (req.url !== '/dev/reset' || req.method !== 'POST') return json(res, 404, {});
    let body = ''; req.on('data', (c) => { body += c; }).on('end', () => {
      const rows = body ? JSON.parse(body) : [];
      posted.push(rows);
      json(res, 200, { reset: true, seeded: rows.length, generation: ++generation });
    });
  });
  const old = await serve((req, res) => req.url === '/dev/status' ? json(res, 200, { storeId: 'b2' })
    : json(res, 404, { denied: { code: 'E_OP', message: 'no route /dev/reset; operations are POST /q/<op> and POST /m/<op>' } }));
  const other = await serve((req, res) => json(res, 404, {}));
  const at = (server) => `http://127.0.0.1:${server.address().port}`;
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-fresh-'));
  try {
    // An origin that is no snapback4 dev server is left alone; the one that is, emptied, then seeded.
    expect(await resetBackends([at(other), at(door)])).toEqual([`${at(door)} reset to generation 2`]);
    writeFileSync(resolve(dir, 'seed.json'), JSON.stringify([{ table: 'owners', row: { id: 'o1', person: 'dev:alice' } }]));
    const rows = freshRows(resolve(dir, 'app.test.contract'), 'seed.json');
    expect(await resetBackends([at(door)], rows)).toEqual([`${at(door)} reset to generation 3, 1 row seeded`]);
    expect(posted).toEqual([[], rows]);
    expect(freshRows('x.test.contract', null)).toBe(null);
    writeFileSync(resolve(dir, 'bad.json'), '{"table":"owners"}');
    expect(() => freshRows(resolve(dir, 'a.test.contract'), 'bad.json')).toThrow(/bad\.json is not a list of \{"table": "…", "row": \{…\}\}/);
    expect(() => freshRows(resolve(dir, 'a.test.contract'), 'none.json')).toThrow(/none\.json: no such file/);
    // A snapback4 older than the door, nothing to reset, and nothing listening each refuse with what to do.
    await expect(resetBackends([at(old)])).rejects.toThrow(/has no reset door \(POST \/dev\/reset answered 404: .*E_OP.*older than the snapback4 that adds it/);
    await expect(resetBackends([at(other)])).rejects.toThrow(`fresh: no snapback4 dev server at ${at(other)}`);
    await expect(resetBackends([])).rejects.toThrow(/net\.fetch grants: none/);
    await expect(resetBackends(['http://127.0.0.1:1'])).rejects.toThrow(/nothing answered at http:\/\/127\.0\.0\.1:1 .*snapback4 dev --port 1/);
  } finally { for (const s of [door, old, other]) s.close(); rmSync(dir, { recursive: true, force: true }); }
});

// A mutation's refused reply is named with why, from either runner's line (sb4 farm wave 6, E3); a resource's is not.
test("a mutation whose reply was refused is named with the field its shape lacks", async () => {
  const { refusedReplies } = await import('./agent-test.mjs');
  const why = '`post` answered outside its shape: field `seq` is not in the shape';
  expect([...refusedReplies([
    `t=12 refused reply posted; wall 3 ms: ${why}`,
    `t=12 request 5 (posted) failed and is no longer pending: it ends unsent: ${why}`, // the web runtime
    'request 7 (saved) failed and is no longer pending: it is not retried: `save` answered outside its shape: field `id` is missing', // the runner
    'request 8 (board) failed and is no longer pending: it keeps its last value: E_OFFLINE', // a resource: `state.failed` says it
    { not: 'a line' },
  ])]).toEqual([['posted', why], ['saved', '`save` answered outside its shape: field `id` is missing']]);
  expect(refusedReplies(undefined).size).toBe(0);
});

// A named port a page just let go of is asked again for a moment; one held for good is refused.
test('a named port still held is asked again, then refused', async () => {
  const { listenOn } = await import('./agent-launch.mjs');
  const { createServer } = await import('node:http');
  const holder = createServer(), next = createServer();
  const port = await listenOn(holder, 0, '127.0.0.1', 0, false, {});
  try {
    const started = Date.now();
    setTimeout(() => holder.close(), 300);
    expect(await listenOn(next, port)).toBe(port);
    expect(Date.now() - started).toBeGreaterThanOrEqual(250);
    const third = createServer();
    await expect(listenOn(third, port, '127.0.0.1', 300)).rejects.toMatchObject({ code: 'EADDRINUSE' });
    // A port the OS handed out is in its ephemeral range, which a named port's refusal names where it can read it.
    const said = await listenOn(createServer(), port, '127.0.0.1', 0, true).catch((e) => e.message);
    expect(said).toContain(`port ${port} (--port or EXACT_PORT) is in use`);
    if (process.platform === 'linux') expect(said).toContain("in this machine's ephemeral range");
  } finally { holder.close(); next.close(); }
});


test('temporary test pages ignore a stale storage port while explicit ports remain authoritative', async () => {
  const { drivePagePort, listenOn } = await import('./agent-launch.mjs');
  const { createServer } = await import('node:net');
  const stale = createServer(), page = createServer();
  try {
    await listenOn(stale, 0);
    const kept = {port: stale.address().port};
    expect(drivePagePort(null, kept, true)).toBe(0);
    await listenOn(page, drivePagePort(null, kept, true));
    expect(page.address().port).not.toBe(kept.port);
    expect(drivePagePort(50821, kept, true)).toBe(50821);
    expect(drivePagePort(null, kept, false)).toBe(kept.port);
  } finally {
    await Promise.all([stale, page].map(server => new Promise(done => server.close(done))));
  }
});


test('automatic web listeners stay in PORT_LO/PORT_HI and refuse exhaustion', async () => {
  const {createServer} = await import('node:http');
  const {listenOn, webPortRange, drivePagePort} = await import('./agent-launch.mjs');
  const holder = createServer(), page = createServer(), exhausted = createServer();
  const port = await listenOn(holder, 0, '127.0.0.1', 0, false, {});
  const env = {PORT_LO: String(port), PORT_HI: String(port)};
  try {
    expect(webPortRange(env)).toEqual([port, port]);
    expect(drivePagePort(null, {port: port - 1}, false, env)).toBe(0);
    expect(drivePagePort(port - 1, null, true, env)).toBe(port - 1);
    await expect(listenOn(exhausted, 0, '127.0.0.1', 0, false, env)).rejects.toThrow(/PORT_LO\/PORT_HI.*all in use/);
    await new Promise(ok => holder.close(ok));
    expect(await listenOn(page, 0, '127.0.0.1', 0, false, env)).toBe(port);
    expect(() => webPortRange({PORT_LO: '2'})).toThrow('inclusive port range');
    expect(() => webPortRange({PORT_LO: '4', PORT_HI: '3'})).toThrow('inclusive port range');
    expect(() => webPortRange({PORT_LO: '1', PORT_HI: '65536'})).toThrow('inclusive port range');
  } finally {
    for (const server of [holder, page, exhausted]) if (server.listening) await new Promise(ok => server.close(ok));
  }
});

test('backend fetch prefixes follow the built origin and refuse ambiguous grants', async () => {
  const {backendFetchPrefixes} = await import('./agent-test.mjs');
  const tests = () => [{steps: [{op: 'fail-fetch', prefix: 'backend', line: 1}, {op: 'pass-fetch', prefix: 'backend', line: 2}, {op: 'fail-fetch', prefix: 'https://other.example', line: 3}]}];
  expect(backendFetchPrefixes(tests(), ['http://127.0.0.1:30125/'])[0].steps.map(s => s.prefix))
    .toEqual(['http://127.0.0.1:30125', 'http://127.0.0.1:30125', 'https://other.example']);
  expect(() => backendFetchPrefixes(tests(), [])).toThrow(/exactly one net.fetch origin/);
  expect(() => backendFetchPrefixes(tests(), ['https://one.example', 'https://two.example'])).toThrow(/literal URL/);
});

test('backend selection changes captured origin and grants while preserving unrelated literals', async () => {
  const {backendOrigin, injectBackendOrigin} = await import('../js/bake/src/typescript.mjs');
  const {parseSync} = await import('rolldown/utils');
  const code = `// const origin = 'http://comment';
const label = '☃';
const origin = 'http://127.0.0.1:4400/';
export const grants = [\`net.fetch \${origin}\`, 'net.fetch http://127.0.0.1:4400', 'net.fetch https://other.example'].join('\\n');
const other = 'http://127.0.0.1:4400';`;
  const next = injectBackendOrigin(code, 'https://backend.example', parseSync);
  const template=injectBackendOrigin("const origin='http://old'; export const grants=`net.fetch http://old\\nfs.read app:/tmp`;", 'https://backend.example', parseSync);
  expect(parseSync('app.ts',template).errors).toEqual([]);
  expect(template).toContain('"net.fetch https://backend.example\\nfs.read app:/tmp"');

  expect(next).toContain('const origin = "https://backend.example"');
  expect(next).toContain('"net.fetch https://backend.example"');
  expect(next).toContain('net.fetch https://other.example');
  expect(next).toContain("const other = 'http://127.0.0.1:4400'");
  expect(next).toContain("// const origin = 'http://comment'");
  expect(() => injectBackendOrigin('const origin = compute();', 'http://new', parseSync)).toThrow(/top-level literal/);
  for (const value of ['file:///tmp/a', 'https://user:pw@backend.example', 'http://backend.example/path', ''])
    expect(() => backendOrigin('.', {EXACT_BACKEND_ORIGIN: value})).toThrow(/EXACT_BACKEND_ORIGIN/);
  expect(backendOrigin('.', {EXACT_BACKEND_ORIGIN: 'https://backend.example/'})).toBe('https://backend.example');
});

test('interactive faults resolve backend, warn on impossible prefixes, and reload keeps storage and live faults', async () => {
  const {faultRequest, reloadWeb, driverCapabilities} = await import('./agent-test.mjs');
  const warnings=[];
  expect(faultRequest('fail', ['fetch','backend','times','2'], ['http://127.0.0.1:30100'], s=>warnings.push(s)))
    .toEqual({op:'prefer',faults:{fail:'http://127.0.0.1:30100',times:2}});
  expect(faultRequest('pass',['fetch','backend'],['https://cdn.example','http://127.0.0.1:30100'])).toEqual({op:'prefer',faults:{pass:'http://127.0.0.1:30100'}});
  faultRequest('fail', ['fetch','http://wrong'], ['http://127.0.0.1:30100'], s=>warnings.push(s));
  expect(warnings).toHaveLength(1);
  expect(warnings[0]).toContain('matches no granted origin');
  expect(() => faultRequest('pass', ['fetch','backend'], [])).toThrow(/exactly one/);
  const calls=[];
  const session={host:'web', now:12, logCursor:9,
    state: async()=>({background:{queued:1,inFlight:0},faults:[]}),
    clock:async arg=>{calls.push(arg);return {settled:true};}, carrier:{reset:async options=>{calls.push(options);}}};
  expect(await reloadWeb(session)).toEqual({reloaded:true});
  expect(calls).toEqual(['data',{keep:true,failFetch:''}]);
  expect(session.now).toBe(0);
  expect(driverCapabilities()).toContain('reload (interactive)');
});

test('authored-test clock advances once per mutation request, not per input reply or resource', async()=>{
  const {testMutationClock}=await import('./agent-test.mjs');
  let requests=2;
  const calls=[], clock=testMutationClock();
  const session={state:async()=>({mutationRequests:requests}),clock:async value=>{calls.push(value);}};
  expect(await clock.advance(session)).toBe(2);
  expect(await clock.advance(session)).toBe(0);
  requests++;
  expect(await clock.advance(session)).toBe(1);
  expect(calls).toEqual(['+2','+1']);
  clock.reset(); requests=1;
  expect(await clock.advance(session)).toBe(1);
});

test('a reused captured stage restores the authored backend after an override is removed',async()=>{
  const {configure}=await import('../js/bake/src/typescript.mjs');
  const {parseSync}=await import('rolldown/utils');
  const {mkdtempSync,mkdirSync,writeFileSync,readFileSync,rmSync}=await import('node:fs');
  const {tmpdir}=await import('node:os');const {join}=await import('node:path');
  const root=mkdtempSync(join(tmpdir(),'exact-backend-stage-')),app=join(root,'app'),stage=join(root,'stage');
  const env=process.env.EXACT_BACKEND_ORIGIN; delete process.env.EXACT_BACKEND_ORIGIN;
  try {
    mkdirSync(app);mkdirSync(stage);mkdirSync(join(app,'.exact'));
    const source="const origin='http://127.0.0.1:4400'; export const grants=`net.fetch ${origin}`;";
    writeFileSync(join(stage,'app.ts'),source);writeFileSync(join(stage,'__exact_backend_source.json'),JSON.stringify(source));
    writeFileSync(join(stage,'__exact_paths.json'),JSON.stringify({app,mounts:[]}));
    writeFileSync(join(app,'.exact/backend-origin'),'http://127.0.0.1:4412');
    configure(stage,true,parseSync);
    expect(readFileSync(join(stage,'app.ts'),'utf8')).toContain('4412');
    rmSync(join(app,'.exact/backend-origin'));configure(stage,true,parseSync);
    expect(readFileSync(join(stage,'app.ts'),'utf8')).toBe(source);
    // An authored edit equal to the previous injection is still a new captured source.
    const edited=source.replace('4400','4412');writeFileSync(join(stage,'__exact_backend_source.json'),JSON.stringify(edited));
    configure(stage,true,parseSync);expect(readFileSync(join(stage,'app.ts'),'utf8')).toBe(edited);
  } finally {if(env!==undefined) process.env.EXACT_BACKEND_ORIGIN=env;rmSync(root,{recursive:true,force:true});}
});

test('mutation requests issued by an automatic clock step receive their own millisecond',async()=>{
  const {testMutationClock}=await import('./agent-test.mjs');
  let requests=1,now=0;
  const clock=testMutationClock();
  const session={state:async()=>({mutationRequests:requests}),clock:async spec=>{now+=Number(spec.slice(1));if(requests===1)requests++;}};
  expect(await clock.advance(session)).toBe(2);expect(now).toBe(2);
  expect(await clock.advance(session)).toBe(0);expect(now).toBe(2);
});


test('web freshness tracks backend selection creation, edits and deletion',async()=>{
  const {webChanges,webInputDigests}=await import('./agent-launch.mjs');
  const {mkdtempSync,mkdirSync,writeFileSync,rmSync}=await import('node:fs');
  const {tmpdir}=await import('node:os');const {join}=await import('node:path');
  const dir=mkdtempSync(join(tmpdir(),'exact-origin-freshness-'));
  const app={dir,manifest:{}},dist=join(dir,'dist'),selection=join(dir,'.exact/backend-origin');
  try {
    mkdirSync(dist);mkdirSync(join(dir,'.exact'));
    const bake=()=>writeFileSync(join(dist,'.exact-build.json'),JSON.stringify({target:'js',inputs:webInputDigests(app,true)}));
    bake();expect(webChanges(dist,app).all).toEqual([]);
    writeFileSync(selection,'http://127.0.0.1:4412');expect(webChanges(dist,app).app).toEqual([selection]);
    bake();expect(webChanges(dist,app).all).toEqual([]);
    writeFileSync(selection,'http://127.0.0.1:4413');expect(webChanges(dist,app).app).toEqual([selection]);
    bake();rmSync(selection);expect(webChanges(dist,app).app).toEqual([selection]);
    bake();expect(webChanges(dist,app).all).toEqual([]);
  } finally {rmSync(dir,{recursive:true,force:true});}
});

test('widget variants address the gallery without changing viewport flags', () => {
  const { flags, rest } = parseFlags(['ios', 'widget', 'next-trains', '--family', 'systemMedium', '--mode', 'accented', '--scheme', 'dark', '--size', 'xxxl', '--sim', 's1126-wp6', 'tree', 'screenshot shot.png', 'state']);
  expect(flags.widget).toEqual({ entry: 'next-trains', family: 'systemMedium', mode: 'accented', scheme: 'dark', size: 'xxxl' });
  expect(flags.size).toBeUndefined();
  expect(flags.sim).toBe('s1126-wp6');
  expect(rest).toEqual(['ios', 'tree', 'screenshot shot.png', 'state']);
  expect(parseFlags(['macos', '--size', '420x900', 'tree']).flags.size).toEqual([420, 900]);
  expect(() => parseFlags(['web', 'widget', 'next-trains', 'tree'])).toThrow('ios or macos');
  expect(() => parseFlags(['ios', 'widget', '--mode', 'dark'])).toThrow('name an entry');
});


test('widget addresses occupy the command word and accept only strict reads or app clock', async () => {
  const { widgetStep } = await import('./agent-widget.mjs');
  expect(parseFlags(['--app', 'caltrain', 'ios', 'widget', 'tree', '--props', '{"station":"Given"}', 'tree']).flags.widget).toEqual({entry:'tree', props:'{"station":"Given"}'});
  expect(parseFlags(['macos', '--open', 'widget', 'tree']).flags.widget).toBeUndefined();
  for (const line of ['tags', 'logs', 'tap entry', 'screenshot a.png window', 'screenshot a.png compare b.png', 'screenshot a.png over 600 every 50', 'screenshot a.apng']) {
    expect(() => parseFlags(['ios', 'widget', 'next-trains', line])).toThrow('widget: tree, state');
  }
  expect(() => parseFlags(['ios', 'widget', 'next-trains', '--scale', '2', 'tree'])).toThrow('--scale is unavailable');
  for (const size of ['xxxl', '390xNaN', '0x900']) expect(() => parseFlags(['ios', '--size', size, 'tree'])).toThrow('--size:');
  expect(() => parseFlags(['ios', 'widget', 'next-trains', '--props', '[]', 'tree'])).toThrow('JSON object');
  const calls = [], session = { clock: async value => {calls.push(value); return {clock:60000};}, op: async request => {calls.push(request); return request;} };
  expect(await widgetStep(session, {entry:'next-trains'}, 'clock +60000')).toEqual(['clock', {clock:60000}]);
  await widgetStep(session, {entry:'next-trains'}, 'clock +60000');
  expect(calls).toEqual([{op:'state', widget:{entry:'next-trains'}, availability:true}, '+60000', '+60000']);
  expect((await widgetStep(session, {entry:'next-trains'}, 'screenshot a.png'))[1].path).toBe(resolve('a.png'));
});

test('backend diagnostics name the selected source and warn when a saved selection shadows the literal', async () => {
  const {backendOriginSelection, backendOriginWarning} = await import('../js/bake/src/typescript.mjs');
  const {backendOriginSource, resetBackends} = await import('./agent-test.mjs');
  const {parseSync} = await import('rolldown/utils');
  const dir=mkdtempSync(resolve(tmpdir(),'exact-origin-source-'));
  const literal='http://127.0.0.1:1', saved='http://127.0.0.1:2', explicit='http://127.0.0.1:3';
  const code=`const origin = '${literal}';`;
  try {
    writeFileSync(resolve(dir,'app.ts'),code);
    expect(backendOriginSelection(dir,{})).toBe(null);
    expect(backendOriginSource(dir,literal,{})).toBe('app.ts origin literal');
    mkdirSync(resolve(dir,'.exact'));
    writeFileSync(resolve(dir,'.exact/backend-origin'),saved);
    const selection=backendOriginSelection(dir,{});
    expect(selection).toEqual({origin:saved,source:'.exact/backend-origin'});
    expect(backendOriginSource(dir,saved,{})).toBe('.exact/backend-origin');
    expect(backendOriginSource(dir,explicit,{EXACT_BACKEND_ORIGIN:explicit})).toBe('EXACT_BACKEND_ORIGIN');
    expect(backendOriginSource(dir,'http://127.0.0.1:4',{})).toContain('rebuild');
    expect(backendOriginWarning(code,selection,parseSync)).toContain(`overriding the app.ts origin literal "${literal}"`);
    expect(backendOriginWarning(`const origin = '${saved}/';`,selection,parseSync)).toBe(null);
    expect(backendOriginWarning(code,backendOriginSelection(dir,{EXACT_BACKEND_ORIGIN:explicit}),parseSync)).toBe(null);
    rmSync(resolve(dir,'.exact/backend-origin'));
    await expect(resetBackends([literal],null,1000,{dir})).rejects.toThrow('(source: app.ts origin literal)');
  } finally { rmSync(dir,{recursive:true,force:true}); }
});
