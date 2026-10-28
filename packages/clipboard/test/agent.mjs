// @exact/clipboard under the agent (LLP 1133.006 §9 stage 1 item 7, §7):
// the fixture app made outside the repo and driven on each host named. The
// implementation is its own substitute: each write is decoded, bounded and
// sanitized as in production, lands on the session's private pasteboard (in
// memory on the web), and shows in `state.extensions.clipboard`.
//
// usage: bun packages/clipboard/test/agent.mjs [web] [macos] [ios] [--sim <name|udid>]
import { basename, resolve } from 'node:path';
import { KINDS, checker, discard, makeFixture, root, run } from './fixture.mjs';

const argv = process.argv.slice(2), sim = argv.includes('--sim') ? argv[argv.indexOf('--sim') + 1] : process.env.EXACT_SIM;
const hosts = argv.filter((a, i) => ['web', 'macos', 'ios'].includes(a) && argv[i - 1] !== '--sim');
const { app } = makeFixture(), env = { ...process.env, EXACT_IDENTITY: process.env.EXACT_IDENTITY ?? '-' };
Object.assign(process.env, { EXACT_APP_DIR: app, EXACT2: root, HERMES_LEAN_SYS_OFFLINE: '1', ...(sim ? { EXACT_SIM: sim } : {}) });
const { open } = await import(resolve(root, 'scripts/agent.mjs'));
const { check, failed } = checker();

/** Each testId's text in the tree. */
const texts = async (s) => {
  const { nodes } = await s.tree(), byId = new Map(nodes.map((n) => [n.id, n]));
  const text = (n) => n.props?.text ?? n.children.map((c) => text(byId.get(c))).join('');
  return Object.fromEntries(nodes.filter((n) => n.props?.testId).map((n) => [n.props.testId, text(n)]));
};
/** What a `lastWrite` must hold of `want`: its types, text and image size. */
const holds = (got, want) => JSON.stringify(got?.types) === JSON.stringify(want.types) && (want.text === undefined || got?.text === want.text)
  && (!want.image || (got?.image?.width === want.image.width && got?.image?.height === want.image.height && /^[0-9a-f]{64}$/.test(got?.image?.sha256 ?? '')));

for (const host of hosts.length ? hosts : ['web']) {
  if (host === 'web') run('bun', ['exact.mjs', 'web-build'], app, env);
  if (host === 'macos') run('bun', ['exact.mjs', 'mac'], app, env);
  if (host === 'ios') run('bun', ['exact.mjs', 'ios', ...(sim ? ['--sim', sim] : [])], app, env);
  const s = await open({ host, app: basename(app), storage: `clipboard-${process.pid}` });
  try {
    await s.clock('data');
    const supports = (await texts(s)).supports;
    check(host, 'ClipboardItem.supports answers from the declaration', supports === 'image/png=true text/plain=true text/html=false image/svg+xml=false', supports);
    let writes = 0, sha = {};
    for (const [kind, answer, landed] of KINDS) {
      await s.tap(kind);
      for (let i = 0; i < 4; i++) await s.clock('data');
      const got = (await texts(s)).wrote;
      check(host, `${kind}: answers ${answer}`, got === answer, got);
      const own = (await s.state()).extensions?.clipboard?.implementation ?? {};
      if (landed) writes++;
      check(host, `${kind}: ${landed ? 'lands' : 'leaves the clipboard as it was'}`, own.writes === writes && (!landed || holds(own.lastWrite, landed)), JSON.stringify(own));
      if (landed?.image) sha[kind] = own.lastWrite?.image?.sha256;
    }
    // The sanitized PNG of the red file and of its bytes are the same; the APNG's frame 0 (green) differs from both.
    check(host, 'a path and bytes of one PNG write the same sanitized image', sha.png && sha.png === sha['png-bytes'] && sha.png === sha.both, JSON.stringify(sha));
    check(host, 'the APNG writes frame 0, not its default image', sha.apng && sha.apng !== sha.png, JSON.stringify(sha));
    const logs = (await s.logs?.()) ?? '';
    if (logs) check(host, 'the journal names each write and refusal', /clipboard: wrote image\/png 8×8/.test(JSON.stringify(logs)) && /clipboard: write refused: DataError/.test(JSON.stringify(logs)), String(JSON.stringify(logs)).slice(-600));
  } finally {
    await s.close();
  }
}
discard(app);
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
