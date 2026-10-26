import { test, expect } from 'bun:test';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { extname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { open } from '../../scripts/agent.mjs';

// The module seam's general pieces on the JS target (LLP 1133.002 D14; LLP
// 1133.008 D8 item 9; LLP 1133.009 D10 items 3, 7, 10-12), through an
// interface outside the tree, as LLP 1133.001 D8 asks: `@fixture/sensor`,
// which is its own substitute, with a web implementation that records what
// it is told, installed into an app `exact new` made outside the repo and
// driven by the agent.

const root = resolve(import.meta.dir, '../..');
const run = (cmd, args, cwd) => {
  const r = spawnSync(cmd, args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout}\n${r.stderr}`);
};

const INTERFACE = {
  kind: 'interface', name: '@fixture/sensor', version: '1.0.0',
  grants: ['device.microphone'], substitute: 'implementation',
  permission: { name: 'microphone', status: 'sensorStatus', field: 'state', denied: 'NotAllowedError' },
  shapes: {
    SensorStatus: { available: 'bool', state: 'enum<granted|prompt|denied|unknown>' },
    Reading: { id: 'string', level: 'number', muted: 'bool' },
  },
  names: [
    { source: 'sensorStatus', live: true, answers: 'SensorStatus', substitute: { default: { available: true } } },
    { send: 'open', request: true, origin: 'app', foreground: true, options: { slow: 'option<bool>' }, answers: 'Reading', handles: { id: 'track' } },
    { source: 'level', live: true, poll: { option: 'interval', default: 100 }, options: { track: 'string', interval: 'option<number>' }, answers: 'Reading' },
  ],
};

// Each call is recorded on the page (`__sensor`); `open(slow=true)` answers after 5 s of wall time, so a second
// open overtakes it (300 ms was sometimes over before a loaded machine's next tap, and nothing was overtaken; a
// settle waits for the answer either way). A level reads the session's clock.
const IMPLEMENTATION = `export function implementation({ agent, now }) {
  const seen = globalThis.__sensor = { agent, opened: [], levels: [], releases: [], disposed: [], preferred: [] };
  let n = 0;
  return {
    open(args) {
      const id = 't' + ++n;
      seen.opened.push(id);
      const reading = { id, level: 0, muted: false };
      return args.slow ? new Promise(done => setTimeout(() => done(reading), 5000)) : reading;
    },
    level(args, extra) { seen.levels.push([args, now(), extra?.preferred ?? null]); return { id: args.track, level: now(), muted: false }; },
    releaseArgs(name, args) { seen.releases.push([name, args]); },
    release(name) { seen.releases.push([name]); },
    dispose(ids) { seen.disposed.push(...ids); },
    preferred(values) { seen.preferred.push(values); },
  };
}
`;

const CONTRACT = `use open, level, sensorStatus, SensorStatus, Reading from "@fixture/sensor"

component Fixture
  state track = "a"
  resource status = sensorStatus() as shape SensorStatus else empty()
  resource meter = level(track=track, interval=100) as shape Reading
  mutation opened as shape Reading
  action start
    send opened = open()
  action startSlow
    send opened = open(slow=true)
  action moveB
    track = "b"
  view
    column gap=8 padding=16
      text status.state testId="status"
      text \`\${meter.id} \${meter.level} \${meter.muted}\` testId="meter"
      match opened
        case some(r)
          text r.id testId="opened"
        case none
          text "none" testId="opened"
      button press=start testId="start"
        text "Open"
      button press=startSlow testId="slow"
        text "Open slowly"
      button press=moveB testId="move"
        text "Track b"
`;

/** The interface package and the app, outside the tree; the app's built page. */
function fixture() {
  const dir = mkdtempSync(join(tmpdir(), 'exact-module-seam-')), pkg = join(dir, 'sensor'), app = join(dir, 'seamfixture');
  mkdirSync(join(pkg, 'web'), { recursive: true });
  writeFileSync(join(pkg, 'package.json'), JSON.stringify({ name: '@fixture/sensor', version: '1.0.0', exports: { '.': './exact.json', './web': './web/exact.json' } }));
  writeFileSync(join(pkg, 'exact.json'), JSON.stringify(INTERFACE));
  writeFileSync(join(pkg, 'web/exact.json'), JSON.stringify({ kind: 'implementation', implements: '@fixture/sensor@^1.0', platform: 'web', targets: ['web'], sources: { javascript: ['index.js'] } }));
  writeFileSync(join(pkg, 'web/index.js'), IMPLEMENTATION);
  run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  writeFileSync(join(app, 'app.contract'), CONTRACT);
  rmSync(join(app, 'app.test.contract'), { force: true });
  const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
  writeFileSync(join(app, 'app.ts'), `export const appId = '${id}';\nexport const grants = 'device.microphone purpose.microphone';\nexport const answer = () => { throw new Error('no data source'); };\n`);
  mkdirSync(join(app, 'strings'), { recursive: true });
  writeFileSync(join(app, 'strings/en.json'), JSON.stringify({ 'purpose.microphone': 'The seam fixture reads a level.' }));
  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  manifest.dependencies = { ...manifest.dependencies, '@fixture/sensor': `file:${pkg}` };
  writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
  run('bun', ['install'], app);
  run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);
  return { dir, dist: join(app, 'target/web-dist') };
}

test('an implementation that is its own substitute answers under the agent, behind admission and its ticket', async () => {
  const { dir, dist } = fixture();
  const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
  const server = Bun.serve({ port: 0, async fetch(req) {
    const asked = new URL(req.url).pathname, file = Bun.file(join(dist, asked === '/' ? 'index.html' : asked.slice(1)));
    return (await file.exists()) ? new Response(file, { headers: { 'content-type': types[extname(asked)] ?? 'text/html' } }) : new Response('not found', { status: 404 });
  } });
  let s;
  try {
    s = await open({ host: 'web', url: `http://127.0.0.1:${server.port}/`, webDist: dist });
    const seen = () => s.carrier.evaluate('globalThis.__sensor ?? null');
    const texts = async () => {
      const { nodes } = await s.tree(), byId = new Map(nodes.map(n => [n.id, n]));
      const text = n => n.props?.text ?? n.children.map(c => text(byId.get(c))).join('');
      return Object.fromEntries(nodes.filter(n => n.props?.testId).map(n => [n.props.testId, text(n)]));
    };
    await s.clock('settle');
    // The permission is the runner's under the agent: its status is not asked of the implementation.
    expect((await texts()).status).toBe('prompt');
    const extensions = (await s.state()).extensions.sensor;
    expect(extensions).toMatchObject({ agent: true, permission: 'prompt' });
    expect((await seen()).agent).toBe(true);
    expect((await texts()).meter).toBe('a 0 false');

    // A request while `prompt` waits on a ticket before the implementation hears of it.
    await s.tap('start');
    await s.clock('settle');
    expect((await seen()).opened).toEqual([]);
    const ticket = (await s.state()).pending.find(p => p.device?.capability === 'permission').ticket;
    await s.op({ op: 'tap', ticket, choice: 'allow' });
    await s.clock('settle');
    expect((await seen()).opened).toEqual(['t1']);
    expect((await texts()).opened).toBe('t1');

    // The preferred value reaches the implementation and is laid over its answer.
    await s.prefer({ extension: 'sensor.level={"muted":true}' });
    await s.clock('settle');
    expect((await seen()).preferred.at(-1).level).toEqual({ muted: true });
    expect((await texts()).meter).toMatch(/ true$/);

    // A jump across many periods polls once, and its reply waits for the answer.
    const before = (await seen()).levels.length;
    await s.clock('+1000');
    expect((await seen()).levels.length).toBe(before + 1);
    expect(Number((await texts()).meter.split(' ')[1])).toBeGreaterThanOrEqual(1000);

    // A reader's new arguments go to the implementation before the old group is released.
    await s.tap('move');
    await s.clock('settle');
    expect((await seen()).releases).toEqual([['level', { track: 'a', interval: 100 }]]);
    expect((await texts()).meter).toMatch(/^b /);

    // A reply overtaken before it landed is dropped, and its handles disposed of.
    await s.tap('slow');
    await s.tap('start');
    await s.clock('settle');
    await new Promise(r => setTimeout(r, 500));
    await s.clock('settle');
    expect((await seen()).disposed).toEqual(['t2']);
    expect((await texts()).opened).toBe('t3');
  } finally {
    await s?.close?.();
    server.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}, 600_000);
