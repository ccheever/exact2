// A fetch the app aborts is cancelled on the wire, on the web's two targets (the abort runtime gap): a TypeScript
// source POSTs to a server of this machine that never answers, another answer aborts it, and the server sees the
// connection close; the aborted answer goes on with the rejection, and the next request goes out at once.
// host/web-js/abort.test.mjs runs it on the JS target; host/web/abort.test.mjs on the wasm target, against a dist
// built as `native` below says.
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

export const CONTRACT = `shape Result
  text: string

component AbortFixture
  mutation held as shape Result
  mutation stop as shape Result
  mutation next as shape Result
  derive heldText = match held { case some(r) => r.text, case none => "" }
  derive nextText = match next { case some(r) => r.text, case none => "" }
  action hold
    send held = holdIt()
  action halt
    send stop = haltIt()
  action follow
    send next = nextIt()
  view
    column
      button press=hold testId="hold"
        text "Hold"
      button press=halt testId="halt"
        text "Abort"
      button press=follow testId="follow"
        text "Next"
      text heldText testId="held"
      text nextText testId="next"
`;

export const TESTS = `test "an aborted fetch is cancelled on the wire, its answer goes on, and the next request goes out at once"
  tap "hold"
  clock +200
  tap "halt"
  clock data
  expect state heldText == "aborted: by the app"
  tap "follow"
  clock data
  expect state nextText == "next"
`;

/** `/held` is never answered: its closing is recorded (`closed`) when the client lets the connection go. `/next`
 * answers at once. `seen` lists the paths asked, in order. */
export function abortServer({ port = 0 } = {}) {
  const seen = [], closed = [];
  const cors = { 'access-control-allow-origin': '*', 'access-control-allow-headers': '*' };
  const server = Bun.serve({ hostname: '127.0.0.1', port, idleTimeout: 0, fetch(req) {
    if (req.method === 'OPTIONS') return new Response(null, { headers: cors });
    const path = new URL(req.url).pathname;
    seen.push(path);
    if (path === '/next') return new Response('next', { headers: cors });
    if (path === '/held') return new Promise(() => req.signal.addEventListener('abort', () => closed.push(path)));
    return new Response('not found', { status: 404, headers: cors });
  } });
  return { origin: `http://127.0.0.1:${server.port}`, seen, closed, stop: () => server.stop(true) };
}

/** The app in a fresh directory under `root` (target/), asking `origin`. */
export function abortApp(root, origin) {
  mkdirSync(resolve(root, 'target'), { recursive: true });
  const dir = mkdtempSync(resolve(root, 'target/abort-test-'));
  try {
    writeApp(dir, origin);
  } catch (e) {
    rmSync(dir, { recursive: true, force: true });
    throw e;
  }
  return dir;
}

function writeApp(dir, origin) {
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Abort Fixture', app: { id: 'test.abortfixture', name: 'Abort Fixture' }, host: { web: {} } }));
  writeFileSync(resolve(dir, 'app.contract'), CONTRACT);
  writeFileSync(resolve(dir, 'app.ts'), `export const appId = 'test.abortfixture';
export const grants = ${JSON.stringify(`net.fetch ${origin}`)};
const origin = ${JSON.stringify(origin)};
let held: AbortController | null = null;
export function answer(name: string): any {
  switch (name) {
    case 'holdIt': {
      held = new AbortController();
      return fetch(origin + '/held', { method: 'POST', body: 'x', signal: held.signal })
        .then(r => r.text()).then(t => ({ text: 'answered: ' + t }), (e: Error) => ({ text: 'aborted: ' + e.message }));
    }
    case 'haltIt': held?.abort(new Error('by the app')); return { text: 'halted' };
    case 'nextIt': return fetch(origin + '/next').then(r => r.text()).then(text => ({ text }));
    default: throw new Error('no source ' + name);
  }
}
`);
}

/** Until `ready()`, or `ms` pass. */
export async function within(ms, ready) {
  for (const end = performance.now() + ms; !ready() && performance.now() < end;) await Bun.sleep(10);
  return ready();
}

// `bun scripts/abort-fixture.mjs native <dir> [port]`: the app as an `exact new` app at `dir` (outside the repo),
// asking 127.0.0.1:<port> (47713), for the wasm build host/web/abort.test.mjs runs against.
if (import.meta.main) {
  const [verb, dir, port = '47713'] = process.argv.slice(2);
  if (verb !== 'native' || !dir) { console.error('usage: bun scripts/abort-fixture.mjs native <dir> [port]'); process.exit(2); }
  const { spawnSync } = await import('node:child_process');
  const { cpSync } = await import('node:fs');
  const root = resolve(import.meta.dir, '..');
  const made = spawnSync(process.execPath, ['scripts/exact.mjs', 'new', resolve(dir), '--name', 'Abort Fixture', '--bundle-id', 'test.abortfixture'], { cwd: root, stdio: 'inherit' });
  if (made.status) process.exit(made.status);
  const app = abortApp(root, `http://127.0.0.1:${port}`);
  for (const f of ['app.contract', 'app.ts']) cpSync(resolve(app, f), resolve(dir, f));
  rmSync(app, { recursive: true, force: true });
  rmSync(resolve(dir, 'app.test.contract'), { force: true });
  console.log(`${dir}: build its wasm dist with (cd ${dir} && EXACT_APP_DIR=${resolve(dir)} EXACT_WEB_DIST=<dist> bun ${root}/host/web/build.mjs <its name>-web --wasm)`);
}
