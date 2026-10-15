// The AppDrop lane of `exact deploy` (rules/DEFERRED.md, "AppDrop lane"):
// when the table has an iOS binary row and app.json names `deploy.appdrop.url`,
// the share link people install from, the publisher builds the unsigned .ipa
// (`host/apple/build.mjs --device --archive --unsigned`) against the stream's
// release record and publishes it as that link's next revision with
// `npx appdrop --update`. Bundle-only changes never reach AppDrop: installed
// apps take them over the air, and every AppDrop revision shows everyone who
// opens the link "Update available".
//
// It runs after the streams, so the cohort a new binary reads already has a
// head when someone installs it. A stream's cohort is frozen at its first
// publication, so a binary change that keeps the compatibility id is reported
// on every later deploy; the binary digest last uploaded to each link is kept
// in `~/.config/exact/appdrop.json` (`EXACT_APPDROP_STATE`) so it goes up
// once. The update key is AppDrop's own — `$APPDROP_MANAGE_KEY`, or the one
// the appdrop CLI saved when this machine made the link — never app.json.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { gpuModules } from './app.mjs';
import { refuse } from './deploy-signing.mjs';

const CLI = process.env.EXACT_APPDROP_CLI ?? 'appdrop@0.5';
const statePath = () => resolve(process.env.EXACT_APPDROP_STATE ?? resolve(homedir(), '.config/exact/appdrop.json'));

/** `deploy.appdrop.url`, the share link (`https://<host>/d/<id>`), or null. */
export function appdropLink(manifest) {
  const url = manifest.deploy?.appdrop?.url;
  if (url === undefined) return null;
  const parsed = URL.canParse(url) ? new URL(url) : null;
  const id = parsed?.pathname.match(/^\/d\/([A-Za-z0-9]+)\/?$/)?.[1];
  if (parsed?.protocol !== 'https:' || !id || parsed.search || parsed.hash) refuse(`deploy.appdrop.url ${JSON.stringify(url)}: expected an AppDrop share link, https://appdrop.app/d/<id> (the update link's key stays out of app.json)`);
  return { url: `${parsed.origin}/d/${id}`, origin: parsed.origin, id };
}

function readState() {
  try { return JSON.parse(readFileSync(statePath(), 'utf8')); } catch { return {}; }
}

/** What this deploy owes AppDrop: null without a link or an iOS binary row; else the link, the binary, and whether that binary already went up. */
export function appdropPlan(app, table, builds) {
  const link = appdropLink(app.manifest);
  if (link && (app.hasGpu || gpuModules(app.manifest).length)) refuse(`deploy.appdrop: ${app.name} has GPU modules, whose baked digests AppDrop's re-signing would break (host/apple/build.mjs --unsigned)`);
  const row = table.rows.find((r) => r.kind === 'binary' && r.platform === 'ios');
  if (!link || !row || !builds.ios) return null;
  const binary = builds.ios.binary.sha256;
  const last = readState()[link.url];
  return { ...link, binary, compatibilityId: row.compatibilityId, store: builds.ios.compat?.inputs?.store?.L ?? 'A', action: last?.binary === binary ? 'current' : 'upload', ...(last ? { last } : {}) };
}

/** The dry-run line, in the table's shape. */
export function renderAppdrop(plan) {
  const detail = plan.action === 'current' ? `binary ${plan.binary.slice(0, 12)} is revision ${plan.last.revision} already` : `new binary ${plan.binary.slice(0, 12)}: a new version of ${plan.url}`;
  return `${'appdrop'.padEnd(8)} ${detail.padEnd(60)} → ${plan.action === 'current' ? 'current' : 'upload (after the streams)'}`;
}

/** Build and upload. `receipt` is the iOS stream's release record on the origin, or null for a store.L=0 binary (no stream). */
export async function publishAppdrop({ plan, app, exactRoot, run, receipt, env, log }) {
  if (plan.action === 'current') return { kind: 'appdrop', ...plan };
  if (plan.store !== '0' && !receipt) throw new Error(`the iOS stream ${plan.compatibilityId.slice(0, 8)} has no release record on the origin, which the binary must embed`);
  const drop = await fetch(`${plan.origin}/api/drops/${plan.id}`, { cache: 'no-store', signal: AbortSignal.timeout(30000) });
  if (!drop.ok) throw new Error(`${plan.url}: AppDrop answered ${drop.status}`);
  const revision = (await drop.json()).revision ?? 1;
  const ipa = resolve(run, 'appdrop', `${app.name}.ipa`);
  mkdirSync(dirname(ipa), { recursive: true });
  log(`  appdrop: building ${app.name} for iOS (build ${revision + 1}) into ${ipa}`);
  // build.mjs reports on stdout; this process's stdout is the table or --json.
  const built = spawnSync(process.execPath, [resolve(exactRoot, 'host/apple/build.mjs'), '--device', app.name, '--archive', ipa, '--unsigned'], {
    cwd: exactRoot, stdio: ['ignore', 2, 2],
    env: { ...env, EXACT_UPDATE_TRUST: 'production', EXACT_BUILD_NUMBER: String(revision + 1), ...(receipt ? { EXACT_UPDATE_RECEIPT: receipt } : {}) },
  });
  if (built.status !== 0 || !existsSync(ipa)) throw new Error(`the iOS archive failed (${built.error?.message ?? `exit ${built.status}`})`);
  log(`  appdrop: uploading to ${plan.url}`);
  const uploaded = spawnSync('npx', ['--yes', CLI, ipa, '--update', plan.url, '--json'], { env, encoding: 'utf8', stdio: ['ignore', 'pipe', 2] });
  if (uploaded.status !== 0) throw new Error(`npx ${CLI} failed (${uploaded.error?.message ?? `exit ${uploaded.status}`})`);
  const result = JSON.parse(uploaded.stdout);
  const state = readState();
  state[plan.url] = { binary: plan.binary, revision: result.revision, app: app.id, at: new Date().toISOString() };
  mkdirSync(dirname(statePath()), { recursive: true });
  writeFileSync(statePath(), `${JSON.stringify(state, null, 2)}\n`);
  return { kind: 'appdrop', ...plan, action: 'published', revision: result.revision };
}
