// @fixture/swatch on Apple (LLP 1133.001 D1, LLP 1133.008 D8 items 1 and 2):
// the fixture app made outside the repo, built for macOS, an iOS simulator
// or the sample host (`--host`, two sessions in one process), and driven by
// the agent. The package's view is made by its session's instance of the
// implementation, which hears the view's displayed state and its `tint`
// binding change with the origin of the commit that made each (`app` here:
// the agent's taps are a person's input; a tool's origin is the runner's,
// tested in contract/cli/tests/it/module_views.rs, since no Apple host has a
// tool door yet), and answers what it heard as `swatchReports`.
//
// usage: bun packages/fixtures/swatch/test/apple.mjs [macos] [ios] [host] [--sim <name|udid>]
import { rmSync } from 'node:fs';
import { basename, dirname, resolve } from 'node:path';
import { checker, makeFixture, root, run } from './fixture.mjs';

const argv = process.argv.slice(2), sim = argv.includes('--sim') ? argv[argv.indexOf('--sim') + 1] : process.env.EXACT_SIM;
const hosts = argv.filter((a, i) => ['macos', 'ios', 'host'].includes(a) && argv[i - 1] !== '--sim');
const { app } = makeFixture(), name = basename(app);
Object.assign(process.env, { EXACT_APP_DIR: app, EXACT2: root, HERMES_LEAN_SYS_OFFLINE: '1' });
const { open } = await import(resolve(root, 'scripts/agent.mjs'));
const { check, failed } = checker();

/** Each testId's text in the tree. */
const texts = async (s) => {
  const { nodes } = await s.tree(), byId = new Map(nodes.map((n) => [n.id, n]));
  const text = (n) => n.props?.text ?? n.children.map((c) => text(byId.get(c))).join('');
  return Object.fromEntries(nodes.filter((n) => n.props?.testId).map((n) => [n.props.testId, text(n)]));
};
/** `heard` as its lines once `want` holds of them, or the last seen. */
const heard = async (s, want, ms = 10000) => {
  const end = Date.now() + ms;
  for (;;) {
    await s.clock('settle');
    const lines = ((await texts(s)).heard ?? '').split('; ').filter(Boolean);
    if (want(lines) || Date.now() >= end) return lines;
    await s.clock('+100 real');
  }
};
const count = (lines, line) => lines.filter((l) => l === line).length;

/** One session's drive: shown, rebound, hidden and shown, tapped, removed. */
async function drive(s, where) {
  const none = await heard(s, () => true, 0);
  check(where, 'nothing is reported before the view is placed', none.length === 0, none.join('; '));
  await s.tap('show');
  const shown = await heard(s, (l) => l.some((x) => x.startsWith('made ')) && l.some((x) => x.startsWith('displayed ')) && l.some((x) => x.startsWith('bound ')));
  const id = shown.find((x) => x.startsWith('made '))?.split(' ')[1];
  check(where, "the session's instance made the view", Boolean(id), shown.join('; '));
  const box = (await s.tree()).nodes.find((n) => n.props?.testId === 'box');
  check(where, "the view's id is its node's", String(box?.id) === id, `${box?.id} beside made ${id}`);
  check(where, "shown by the app's commit: displayed true app", shown.includes(`displayed ${id} true app`), shown.join('; '));
  check(where, 'its binding prop reported with its origin', shown.includes(`bound ${id} tint app`), shown.join('; '));
  const state = await s.state();
  const views = state.extensions?.swatch?.views ?? [];
  check(where, 'state.extensions names the view and who showed it', views.length === 1 && views[0].displayed === true && views[0].origin === 'app', JSON.stringify(state.extensions));

  await s.tap('recolor');
  const rebound = await heard(s, (l) => count(l, `bound ${id} tint app`) === 2);
  check(where, 'a rebinding is reported again', count(rebound, `bound ${id} tint app`) === 2, rebound.join('; '));
  await s.tap('hide');
  const hidden = await heard(s, (l) => l.includes(`displayed ${id} false app`));
  check(where, 'display: none is reported hidden', hidden.includes(`displayed ${id} false app`), hidden.join('; '));
  await s.tap('hide');
  const again = await heard(s, (l) => count(l, `displayed ${id} true app`) === 2);
  check(where, 'shown again is reported again', count(again, `displayed ${id} true app`) === 2, again.join('; '));

  const tapped = await s.tap('box').catch((e) => ({ error: e.message }));
  await s.clock('settle');
  const last = (await texts(s)).last;
  check(where, "the view's event reaches its handler", last === 'tap', `${last} after ${JSON.stringify(tapped)}`);

  await s.tap('show');
  const removed = await heard(s, (l) => count(l, `displayed ${id} false app`) === 2);
  check(where, 'a shown view leaving the tree is reported hidden', count(removed, `displayed ${id} false app`) === 2, removed.join('; '));
  return id;
}

for (const host of hosts.length ? hosts : ['macos', 'ios', 'host']) {
  const where = host === 'ios' ? 'ios-simulator' : host === 'host' ? 'sample-host' : 'macos';
  run('bun', [resolve(root, 'host/apple/build.mjs'), ...(host === 'ios' ? ['--ios', ...(sim ? ['--sim', sim] : [])] : host === 'host' ? ['--host'] : []), `${name}-apple`], root);
  if (host !== 'host') {
    // A view's recognizers take only a real touch on iOS (`--touch platform`).
    const s = await open({ host, app: name, ...(host === 'ios' ? { touch: 'platform' } : {}) });
    try { await drive(s, where); } finally { await s.close(); }
    continue;
  }
  // Two sessions of the one plan in one process, each its own instance: a's
  // reports never reach b's, though their node ids are the same.
  const s = await open({ host: 'host', app: name, session: 'a' });
  try {
    check(where, 'the host runs sessions a and b', s.sessions?.join(',') === 'a,b', JSON.stringify(s.sessions));
    const a = await drive(s, `${where} a`);
    s.session = 'b';
    const quiet = await heard(s, () => true, 0);
    check(where, "b's instance heard nothing of a's view", quiet.length === 0, quiet.join('; '));
    await s.tap('show');
    const b = await heard(s, (l) => l.some((x) => x.startsWith('displayed ')));
    const id = b.find((x) => x.startsWith('made '))?.split(' ')[1];
    check(where, "b's view reached b's own instance", b.includes(`displayed ${id} true app`) && b.filter((x) => x.startsWith('made ')).length === 1, b.join('; '));
    check(where, 'the two sessions share node ids, not instances', id === a, `a ${a}, b ${id}`);
    s.session = 'a';
    const after = await heard(s, () => true, 0);
    check(where, "a's instance heard nothing of b's view", after.filter((x) => x.startsWith('made ')).length === 1 && after.every((x) => !x.endsWith('true app') || count(after, x) === 2), after.join('; '));
  } finally {
    await s.close();
  }
}
rmSync(dirname(app), { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
