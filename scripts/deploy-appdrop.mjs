// The AppDrop lane of `exact deploy` (rules/DEFERRED.md, "AppDrop lane"):
// when app.json names `deploy.appdrop` and the table has an iOS binary row,
// the publisher builds the unsigned .ipa (`host/apple/build.mjs --device
// --archive --unsigned`) against the stream's release record and publishes it
// to AppDrop: as a new link the first time, then as that link's next revision
// with `npx appdrop --update`. Bundle-only changes never reach AppDrop:
// installed apps take them over the air, and every AppDrop revision shows
// everyone who opens the link "Update available".
//
// It runs after the streams, so the cohort a new binary reads already has a
// head when someone installs it. A new link is written into the live
// app.json's `deploy.appdrop.url`, to commit, so the install page offers it
// and every machine updates the same link. A link the appdrop CLI already
// made on this machine for the bundle id is adopted rather than duplicated.
// A stream's cohort is frozen at its first publication, so a binary change
// that keeps the compatibility id is reported on every later deploy; the
// binary digest last uploaded to each link is kept in
// `~/.config/exact/appdrop.json` (`EXACT_APPDROP_STATE`) so it goes up once.
// The update key is AppDrop's own — `$APPDROP_MANAGE_KEY`, or the one the
// appdrop CLI saved in `~/.appdrop/drops.json` — never app.json or a log.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { gpuModules } from './app.mjs';
import { refuse } from './deploy-signing.mjs';
import { streamPath } from './origin.mjs';

const CLI = process.env.EXACT_APPDROP_CLI ?? 'appdrop@0.5';
const statePath = () => resolve(process.env.EXACT_APPDROP_STATE ?? resolve(homedir(), '.config/exact/appdrop.json'));
const readJson = (path) => { try { return JSON.parse(readFileSync(path, 'utf8')); } catch { return {}; } };

/** A share link (`https://<host>/d/<id>`) as `{ url, origin, id }`. */
export function parseLink(url, at = 'deploy.appdrop.url') {
  const parsed = URL.canParse(url) ? new URL(url) : null;
  const id = parsed?.pathname.match(/^\/d\/([A-Za-z0-9]+)\/?$/)?.[1];
  if (parsed?.protocol !== 'https:' || !id || parsed.search || parsed.hash) refuse(`${at} ${JSON.stringify(url)}: expected an AppDrop share link, https://appdrop.app/d/<id> (the update link's key stays out of app.json)`);
  return { url: `${parsed.origin}/d/${id}`, origin: parsed.origin, id };
}

/** The link app.json names, else the newest one the appdrop CLI saved on this machine for the bundle id. */
function linkFor(app, config) {
  if (config.url !== undefined) return parseLink(config.url);
  const saved = Object.values(readJson(resolve(process.env.APPDROP_HOME ?? resolve(homedir(), '.appdrop'), 'drops.json')))
    .filter((drop) => drop?.bundleId === app.id && drop.url).sort((a, b) => String(b.savedAt).localeCompare(String(a.savedAt)));
  return saved.length ? { ...parseLink(saved[0].url, '~/.appdrop/drops.json'), adopted: true } : null;
}

/** What this deploy owes AppDrop: null without `deploy.appdrop` or an iOS binary row; else `create`, `upload` or `current`. */
export function appdropPlan(app, table, builds) {
  const config = app.manifest.deploy?.appdrop;
  if (config === undefined) return null;
  if (app.hasGpu || gpuModules(app.manifest).length) refuse(`deploy.appdrop: ${app.name} has GPU modules, whose baked digests AppDrop's re-signing would break (host/apple/build.mjs --unsigned)`);
  const link = linkFor(app, config);
  const row = table.rows.find((r) => r.kind === 'binary' && r.platform === 'ios');
  if (!row || !builds.ios) return null;
  const binary = builds.ios.binary.sha256;
  const last = link && readJson(statePath())[link.url];
  const action = !link ? 'create' : last?.binary === binary ? 'current' : 'upload';
  return { ...link, binary, compatibilityId: row.compatibilityId, store: builds.ios.compat?.inputs?.store?.L ?? 'A', action, ...(last ? { last } : {}) };
}

/** The dry-run line, in the table's shape. */
export function renderAppdrop(plan) {
  const detail = plan.action === 'create' ? `new binary ${plan.binary.slice(0, 12)}: a new AppDrop link, written to app.json`
    : plan.action === 'current' ? `binary ${plan.binary.slice(0, 12)} is revision ${plan.last.revision} already`
    : `new binary ${plan.binary.slice(0, 12)}: a new version of ${plan.url}${plan.adopted ? ' (saved by the appdrop CLI)' : ''}`;
  return `${'appdrop'.padEnd(8)} ${detail.padEnd(60)} → ${plan.action === 'current' ? 'current' : `${plan.action} (after the streams)`}`;
}

/** Set `deploy.appdrop.url` in the app's own app.json, the file the next snapshot reads. */
export function writeLink(manifestPath, url) {
  const text = readFileSync(manifestPath, 'utf8');
  const empty = /"appdrop"\s*:\s*\{\s*\}/;
  if (empty.test(text)) return writeFileSync(manifestPath, text.replace(empty, `"appdrop": { "url": ${JSON.stringify(url)} }`));
  const manifest = JSON.parse(text);
  manifest.deploy = { ...manifest.deploy, appdrop: { ...manifest.deploy?.appdrop, url } };
  writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
}

/** The lane after the streams: find the release record the binary embeds, build, create or upload, record. A failure is a row in `failed`. */
export async function runAppdrop({ table, published, failed, origin, app, channel, release, exactRoot, run, live, env, latestRecord, log }) {
  const plan = table.appdrop;
  if (plan.action === 'current') return published.push({ kind: 'appdrop', ...plan });
  try {
    const stream = { channel, compatibilityId: plan.compatibilityId };
    const ios = [...published, ...failed].find((p) => p.platform === 'ios' && p.compatibilityId === stream.compatibilityId);
    const record = ios?.action === 'published' ? release : ios && !ios.error ? (await latestRecord(origin, app, stream))?.release : null;
    if (plan.store !== '0' && !record) throw new Error(`the iOS stream ${plan.compatibilityId.slice(0, 8)} has no release record on the origin, which the binary must embed`);
    let revision = 0;
    if (plan.action === 'upload') {
      const drop = await fetch(`${plan.origin}/api/drops/${plan.id}`, { cache: 'no-store', signal: AbortSignal.timeout(30000) });
      if (!drop.ok) throw new Error(`${plan.url}: AppDrop answered ${drop.status}`);
      revision = (await drop.json()).revision ?? 1;
    }
    const ipa = resolve(run, 'appdrop', `${app.name}.ipa`);
    mkdirSync(dirname(ipa), { recursive: true });
    log(`  appdrop: building ${app.name} for iOS (build ${revision + 1}) into ${ipa}`);
    // build.mjs reports on stdout; this process's stdout is the table or --json.
    const built = spawnSync(process.execPath, [resolve(exactRoot, 'host/apple/build.mjs'), '--device', app.name, '--archive', ipa, '--unsigned'], {
      cwd: exactRoot, stdio: ['ignore', 2, 2],
      env: { ...env, EXACT_UPDATE_TRUST: 'production', EXACT_BUILD_NUMBER: String(revision + 1), ...(record ? { EXACT_UPDATE_RECEIPT: resolve(origin.dir, streamPath(stream), 'releases', `${record}.json`) } : {}) },
    });
    if (built.status !== 0 || !existsSync(ipa)) throw new Error(`the iOS archive failed (${built.error?.message ?? `exit ${built.status}`})`);
    log(`  appdrop: ${plan.action === 'create' ? 'creating a link' : `uploading to ${plan.url}`}`);
    // The CLI prints a new link's update link (its key) on stderr: keep it out of the log.
    const uploaded = spawnSync('npx', ['--yes', CLI, ipa, ...(plan.action === 'upload' ? ['--update', plan.url] : []), '--json'], { env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
    for (const line of (uploaded.stderr ?? '').split('\n')) if (line && !line.includes('/manage#')) log(`  appdrop: ${line}`);
    if (uploaded.status !== 0) throw new Error(`npx ${CLI} failed (${uploaded.error?.message ?? `exit ${uploaded.status}`})`);
    const result = JSON.parse(uploaded.stdout);
    const url = plan.url ?? parseLink(result.url, 'the appdrop CLI').url;
    const state = readJson(statePath());
    state[url] = { binary: plan.binary, revision: result.revision, app: app.id, at: new Date().toISOString() };
    mkdirSync(dirname(statePath()), { recursive: true });
    writeFileSync(statePath(), `${JSON.stringify(state, null, 2)}\n`);
    if (plan.action === 'create' || plan.adopted) {
      writeLink(resolve(live, 'app.json'), url);
      log(`  appdrop: ${url} is written to ${resolve(live, 'app.json')}; commit it so the install page offers it and other machines update it (its key is in ~/.appdrop/drops.json)`);
    }
    published.push({ kind: 'appdrop', ...plan, url, action: 'published', revision: result.revision });
  } catch (e) {
    failed.push({ kind: 'appdrop', platform: 'ios', error: e.message });
    log(`  appdrop: failed — ${e.message}`);
  }
}
