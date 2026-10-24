// @exact/local-auth's store half on the Linux host under the agent (LLP 1133.010
// §7, §11 stage 2): the vault Android's reader shares (`host/linux/src/
// presence_secrets.rs`), here headless on this machine, with the substitute's
// tickets for the person and the drive's scratch store for the locked items.
// The fixture app is the one `exact new` makes outside the repo (fixture.mjs).
//
// usage: bun packages/local-auth/test/linux.mjs [--app <dir>]
//   --app: a fixture an earlier run made and built, as it stands.
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { resolve } from 'node:path';
import { checker, makeFixture, root, run } from './fixture.mjs';

const argv = process.argv.slice(2);
const kept = argv.includes('--app') ? argv[argv.indexOf('--app') + 1] : null;
const { open, textOf } = await import(resolve(root, 'scripts/agent.mjs'));
const { check, failed } = checker();
const { app, id } = kept ? { app: kept, id: JSON.parse(readFileSync(resolve(kept, 'app.json'), 'utf8')).app.id } : makeFixture();
if (!kept) run('bun', ['exact.mjs', 'linux'], app);
const storage = `la-linux-${Date.now().toString(36)}`;

const launch = async () => {
  const s = await open({ host: 'linux', app, storage });
  await s.prefer({ 'permission.local-auth': 'granted' });
  return s;
};
const text = async (s, testId) => {
  const nodes = (await s.tree()).nodes, node = nodes.find((n) => n.props?.testId === testId);
  return node ? textOf(nodes, node) ?? '' : null;
};
/** Press `testId` and read what `wrote` says once the clock settles; a held check is answered `allow`. */
async function press(s, testId, { allow = false } = {}) {
  await s.tap(testId);
  await s.clock('settle');
  if (allow) {
    await s.answer('tap', '@presence', 'allow');
    await s.clock('settle');
  }
  return text(s, 'wrote');
}
/** The scratch tree's files: plain copies by name, and locked items (the kv store names its files by an encoding of the key). */
function disk() {
  const tree = resolve(process.env.XDG_CACHE_HOME ?? resolve(homedir(), '.cache'), 'exact', id, 'agent', storage);
  const files = (dir) => (existsSync(dir) ? readdirSync(dir).filter((f) => !f.startsWith('.')).sort() : []);
  return { plain: files(resolve(tree, 'secrets')), locked: files(resolve(tree, 'kv/exact.presence.agent')) };
}

try {
  let s = await launch();
  try {
    await s.clock('settle');
    const key = await press(s, 'set-key');
    const facts = await text(s, 'facts');
    check('linux', 'a mandatory name is kept locked at once', key === 'kept' && facts === 'false false true true', `${key} / ${facts}`);
    const read = await press(s, 'read-key');
    check('linux', 'get refuses a locked name', read?.startsWith('InvalidAccessError'), read);
    await press(s, 'set-token');
    const plain = await text(s, 'facts');
    check('linux', 'an optional name is kept plain', plain === 'true false true true', plain);
    const protect = await press(s, 'protect');
    check('linux', 'protect locks it with no check', protect === 'protected' && (await text(s, 'facts')) === 'true true true true', protect);
    const unlocked = await press(s, 'unlock', { allow: true });
    check('linux', 'unlock answers the value after the check', unlocked === 'unlocked open', unlocked);
    const state = JSON.stringify(await s.state());
    check('linux', 'no locked bytes in the state', !state.includes('locked-value'), 'locked-value in state');
    const held = disk();
    check('linux', 'the scratch store holds two locked items and no plain copy', held.plain.length === 0 && held.locked.length === 2, JSON.stringify(held));
  } finally { await s.close(); }
  s = await launch();
  try {
    await s.clock('settle');
    const facts = await text(s, 'facts');
    check('linux', 'a relaunch finds both locked', facts === 'true true true true', facts);
    const unprotected = await press(s, 'unprotect', { allow: true });
    const plain = await text(s, 'facts');
    check('linux', 'unprotect, after the check, keeps the token plain', unprotected === 'unprotected' && plain === 'true false true true', `${unprotected} / ${plain}`);
    const after = disk();
    check('linux', 'the token\'s plain copy is back and its locked item gone', JSON.stringify(after.plain) === '["token"]' && after.locked.length === 1, JSON.stringify(after));
  } finally { await s.close(); }
} catch (e) {
  check('linux', 'ran', false, e.stack ?? e);
}
console.log(`${failed()} failed`);
process.exit(failed() ? 1 : 0);
