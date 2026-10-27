#!/usr/bin/env bun
// Deploys the auth fixture's public origin, https://exact-auth-fixture.expo.app, on EAS Hosting (LLP 1133.011
// §13 question 4; the domain named and approved by Charlie, 2026-10-26), and checks it. It serves:
//
//   /.well-known/apple-app-site-association  `webcredentials` for <host.ios.team>.<app id>, and for
//                                            SIMULATORX.<app id>, a simulator build's prefix (D14)
//   /.well-known/assetlinks.json             `get_login_creds` for the package and `host.android.certificates` (D6)
//   POST /passkey/…                          the relying party: relying-party.mjs through origin-routes.mjs
//
// `scripts/origin.mjs` has no EAS Hosting adapter (LLP 1030.003 D7), so this drives `eas` itself, on an export it
// writes by hand in the layout `expo export` makes for a server output: `client/` static files, and `server/`
// with `_expo/routes.json` and the API route bundles.
//
// usage: bun apps/auth-fixture/origin.mjs [--owner <account>] [--stage <dir>] [--check]
//   Deploys with `eas` (`EAS` names another binary), logged in or given EXPO_TOKEN, as an account with access to
//   <account> (default ccheever), then checks the live origin. `--stage <dir>` writes the export there and stops;
//   `--check` checks the live origin and deploys nothing.
//
// The first deploy makes the project @<account>/exact-auth-fixture, claims the subdomain, and sets
// PASSKEY_RECORD_KEY, a random HMAC key, in the project's production environment. It is `sensitive`, not
// `secret`: `eas deploy` reads each variable's value into the deployment, and a `secret` one's is withheld from
// the CLI. The key is passed to `eas` alone, never written to disk or printed.
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { respond } from '../../packages/passkeys/test/authenticator.mjs';
import { androidOrigin } from './relying-party.mjs';

export const DOMAIN = 'exact-auth-fixture.expo.app';
const SLUG = 'exact-auth-fixture';
// host/apple/build.mjs's SIMULATOR_PREFIX: the application identifier's prefix a simulator build signs with.
const SIMULATOR_PREFIX = 'SIMULATORX';
export const ROUTES = ['/passkey/register/options', '/passkey/register', '/passkey/sign-in/options', '/passkey/sign-in', '/passkey/accepted'];

const args = process.argv.slice(2), flag = (name) => (args.includes(name) ? args[args.indexOf(name) + 1] : undefined);
const owner = flag('--owner') ?? 'ccheever';
const app = JSON.parse(readFileSync(new URL('./app.json', import.meta.url), 'utf8'));
const team = app.host?.ios?.team, certificates = app.host?.android?.certificates ?? [];
if (!team || !certificates.length) throw new Error('app.json needs host.ios.team and host.android.certificates');

/** The two association files' contents, and the origins the relying party accepts for the domain (D6). */
export const files = {
  'apple-app-site-association': { webcredentials: { apps: [`${team}.${app.id}`, `${SIMULATOR_PREFIX}.${app.id}`] } },
  'assetlinks.json': [{ relation: ['delegate_permission/common.get_login_creds'],
    target: { namespace: 'android_app', package_name: app.id, sha256_cert_fingerprints: certificates } }],
};
const rps = { [DOMAIN]: [`https://${DOMAIN}`, ...certificates.map(androidOrigin)] };

async function stage(dir) {
  rmSync(dir, { recursive: true, force: true });
  const client = join(dir, 'dist/client'), server = join(dir, 'dist/server');
  mkdirSync(join(client, '.well-known'), { recursive: true });
  mkdirSync(join(server, '_expo/functions'), { recursive: true });
  writeFileSync(join(dir, 'package.json'), JSON.stringify({ name: SLUG, private: true }, null, 2));
  writeFileSync(join(dir, 'app.json'), JSON.stringify({ expo: { name: SLUG, slug: SLUG, owner, web: { output: 'server' } } }, null, 2));
  for (const [name, body] of Object.entries(files)) writeFileSync(join(client, '.well-known', name), JSON.stringify(body) + '\n');
  writeFileSync(join(client, 'index.html'), `<!doctype html><meta charset="utf-8"><title>Auth fixture relying party</title>
<p>The exact2 auth fixture's passkey relying party (LLP 1133.011). It holds test credentials only.</p>\n`);
  const built = await Bun.build({ entrypoints: [resolve(import.meta.dir, 'origin-routes.mjs')], format: 'cjs', target: 'browser',
    define: { ORIGIN_RPS: JSON.stringify(rps) } });
  if (!built.success) throw new AggregateError(built.logs, 'bundling origin-routes.mjs');
  const file = '_expo/functions/passkey+api.js';
  writeFileSync(join(server, file), await built.outputs[0].text());
  const escape = (path) => path.replace(/[.*+?^${}()|[\]\\/]/g, '\\$&');
  writeFileSync(join(server, '_expo/routes.json'), JSON.stringify({
    apiRoutes: ROUTES.map((page) => ({ file, page, namedRegex: `^${escape(page)}(?:/)?$`, routeKeys: {} })),
    htmlRoutes: [], notFoundRoutes: [], redirects: [], rewrites: [],
  }, null, 2));
}

/** Runs `eas` in the stage. `quiet` captures its output, which this never prints whole, as it may echo a value. */
function eas(dir, argv, { quiet = false } = {}) {
  const r = spawnSync(process.env.EAS ?? 'eas', argv, { cwd: dir, env: { ...process.env, EAS_NO_VCS: '1' }, encoding: 'utf8', stdio: quiet ? 'pipe' : 'inherit' });
  if (r.error) throw r.error;
  return r;
}

/** The live origin: both files with no redirect and a JSON type, and a whole ceremony with a software authenticator. */
export async function check() {
  let failed = 0;
  const say = (ok, what, detail = '') => { if (!ok) failed++; console.log(`${ok ? 'ok  ' : 'FAIL'} ${what}${ok ? '' : `\n     ${detail}`}`); };
  for (const [name, want] of Object.entries(files)) {
    const url = `https://${DOMAIN}/.well-known/${name}`;
    const r = await fetch(url, { redirect: 'manual' }).catch((e) => ({ status: 0, headers: new Headers(), text: async () => String(e) }));
    const type = r.headers.get('content-type') ?? '', body = await r.text();
    let same = false;
    try { same = JSON.stringify(JSON.parse(body)) === JSON.stringify(want); } catch {}
    say(r.status === 200 && /^application\/json/.test(type) && same, `${url}: 200, ${type}, the expected entries`, `${r.status} ${type} ${body.slice(0, 300)}`);
  }
  const post = async (path, body) => {
    const r = await fetch(`https://${DOMAIN}${path}`, { method: 'POST', redirect: 'manual', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) });
    return r.status === 200 ? r.json() : { status: r.status, text: await r.text() };
  };
  const dir = mkdtempSync(join(tmpdir(), 'auth-fixture-origin-')), keys = join(dir, 'keys.json'), origin = `https://${DOMAIN}`;
  try {
    const handle = `check-${Date.now().toString(36)}`;
    const created = await post('/passkey/register/options', { username: handle, rpId: DOMAIN, records: [] });
    say(created.publicKey?.rp?.id === DOMAIN, `register options for ${DOMAIN}`, JSON.stringify(created));
    const made = created.publicKey && await respond({ ceremony: 'create', rpId: DOMAIN, options: created.publicKey }, keys, origin);
    const registered = made?.response ? await post('/passkey/register', { response: made.response }) : {};
    say(registered.ok === true && typeof registered.record === 'string', 'a registration verifies and answers a signed record', JSON.stringify(registered));
    const asked = await post('/passkey/sign-in/options', { rpId: DOMAIN });
    const got = asked.publicKey && await respond({ ceremony: 'get', rpId: DOMAIN, options: asked.publicKey }, keys, origin);
    const signed = got?.response ? await post('/passkey/sign-in', { response: got.response, record: registered.record }) : {};
    say(signed.ok === true && signed.handle === handle, 'a sign-in with that record verifies', JSON.stringify(signed));
    const unknown = got?.response ? await post('/passkey/sign-in', { response: got.response, record: null }) : {};
    say(unknown.error === 'challenge' || unknown.error === 'unknown-credential', 'the spent challenge or a missing record is refused', JSON.stringify(unknown));
    const accepted = await post('/passkey/accepted', { userId: registered.userId, records: [registered.record, 'forged.record'] });
    say(JSON.stringify(accepted.ids) === JSON.stringify([registered.credentialId]), 'accepted lists the signed record and not a forged one', JSON.stringify(accepted));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
  console.log(failed ? `${failed} failed` : `https://${DOMAIN}: all checks passed`);
  return failed;
}

if (import.meta.main) {
  if (args.includes('--check')) process.exit(await check() ? 1 : 0);
  const dir = flag('--stage') ? resolve(flag('--stage')) : mkdtempSync(join(tmpdir(), 'auth-fixture-origin-'));
  await stage(dir);
  if (flag('--stage')) { console.log(`${dir}: the export, not deployed`); process.exit(0); }
  try {
    if (eas(dir, ['init', '--account', owner, '--non-interactive']).status !== 0) throw new Error(`eas init @${owner}/${SLUG} failed`);
    const listed = eas(dir, ['env:list', 'production', '--format', 'short', '--non-interactive'], { quiet: true });
    if (listed.status !== 0) throw new Error('eas env:list failed');
    if (!/^PASSKEY_RECORD_KEY=/m.test(listed.stdout)) {
      const key = Buffer.from(crypto.getRandomValues(new Uint8Array(32))).toString('hex');
      const set = eas(dir, ['env:set', 'production', '--name', 'PASSKEY_RECORD_KEY', '--value', key, '--type', 'string',
        '--visibility', 'sensitive', '--non-interactive'], { quiet: true });
      if (set.status !== 0) throw new Error(`eas env:set PASSKEY_RECORD_KEY failed: ${set.stderr.replaceAll(key, '…')}`);
      console.log('set PASSKEY_RECORD_KEY (sensitive) in the production environment');
    }
    const deployed = eas(dir, ['deploy', '--prod', '--environment', 'production', '--export-dir', 'dist', '--dev-domain', SLUG, '--non-interactive']);
    if (deployed.status !== 0) throw new Error('eas deploy failed');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
  process.exit(await check() ? 1 : 0);
}
