#!/usr/bin/env bun
// `bun exact.mjs share`: build the app for iPhone and print an AppDrop link that
// installs it. The person opens the link on their iPhone, and AppDrop signs the
// app with their own Apple ID: no TestFlight, provisioning profile or cable.
//
//   bun scripts/share.mjs --app <name> [--new-link | --force] [--json]
//
// The link is the last line of stdout; the build and the upload report on
// stderr. Each checkout keeps its link in the app's `.exact/appdrop.json`
// (gitignored), with the git branch it was published from. The first share
// creates the link; later ones publish a new version to it, so a phone that
// installed it sees "Update available". A share from another branch refuses
// (code `link_branch_mismatch`): `--new-link` gives this branch its own link,
// `--force` publishes over the other branch's. When the app's git tree and
// exact2's are clean and at the commits of the last share, nothing is built and
// the same link is printed.
//
// The upload is the appdrop CLI's (`npx appdrop`, or EXACT_APPDROP_CLI), run in
// the app's directory, so the link is labelled with the branch. Its update key
// stays in the CLI's own ~/.appdrop/drops.json and never reaches stdout.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { guardFlags } from './help.mjs';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));
const CLI = process.env.EXACT_APPDROP_CLI ?? 'appdrop@0.8';

const git = (dir, args) => {
  const r = spawnSync('git', ['-C', dir, ...args], { encoding: 'utf8' });
  return r.status === 0 ? r.stdout.trim() : null;
};

/** The branch `dir` is on; a detached HEAD's short commit; null outside git. */
export function branchOf(dir) {
  return git(dir, ['symbolic-ref', '--short', '-q', 'HEAD']) || git(dir, ['rev-parse', '--short', 'HEAD']);
}

/** What the build is made from, when both trees are clean: the app's commit and exact2's. Else null. */
export function sourceOf(dir, exact2 = ROOT) {
  const parts = [];
  for (const tree of [dir, exact2]) {
    const commit = git(tree, ['rev-parse', 'HEAD']), status = git(tree, ['status', '--porcelain', '--', '.']);
    if (!commit || status !== '') return null;
    parts.push(commit);
  }
  return parts.join(' ');
}

/** A share link (`https://<host>/d/<id>`) as `{ url, origin, id }`, or null. */
export function parseLink(url) {
  const parsed = URL.canParse(url) ? new URL(url) : null;
  const id = parsed?.pathname.match(/^\/d\/([A-Za-z0-9]+)\/?$/)?.[1];
  return parsed?.protocol === 'https:' && id ? { url: `${parsed.origin}/d/${id}`, origin: parsed.origin, id } : null;
}

/** What this share does, from the checkout's record, the branch and the source:
 *  `create`, `update`, `current` (nothing to build) or `refuse`. */
export function plan(record, { branch, source, newLink = false, force = false }) {
  const link = !newLink && record?.url ? parseLink(record.url) : null;
  if (!link) return { action: 'create' };
  if (record.branch && branch && record.branch !== branch && !force) return { action: 'refuse', ...link, from: record.branch };
  if (source && record.source === source && record.branch === branch) return { action: 'current', ...link, revision: record.revision };
  return { action: 'update', ...link };
}

/** The refusal for a link another branch published, as the agent reads it. */
export const mismatch = (p, branch) => `This app's AppDrop link, ${p.url}, was last shared from ${p.from}; this checkout is on ${branch}. `
  + `Pass --new-link to give ${branch} its own link, or --force to publish over ${p.from}'s.`;

async function main(argv) {
  guardFlags('share', argv);
  const name = argv[argv.indexOf('--app') + 1];
  const json = argv.includes('--json'), newLink = argv.includes('--new-link'), force = argv.includes('--force');
  const say = (line) => console.error(line);
  const fail = (code, error, extra = {}) => {
    if (json) console.log(JSON.stringify({ code, error, ...extra }));
    say(error);
    return 1;
  };
  if (newLink && force) return fail('share_flags', 'share: --new-link and --force are opposites; pass one.');
  const { resolveApp } = await import('./app.mjs');
  const app = resolveApp(name);
  const recordPath = resolve(app.dir, '.exact/appdrop.json');
  const record = existsSync(recordPath) ? JSON.parse(readFileSync(recordPath, 'utf8')) : null;
  const branch = branchOf(app.dir), source = sourceOf(app.dir);
  const step = plan(record, { branch, source, newLink, force });
  if (step.action === 'refuse') return fail('link_branch_mismatch', mismatch(step, branch), { url: step.url, branch: step.from });
  if (step.action === 'current') {
    say(`share: nothing changed since build ${step.revision} of ${step.url}; not rebuilt.`);
    console.log(json ? JSON.stringify({ url: step.url, revision: step.revision, branch, action: 'current' }) : step.url);
    return 0;
  }

  // AppDrop refuses a build number it already published: the next one is its revision + 1.
  let revision = 0;
  if (step.action === 'update') {
    const drop = await fetch(`${step.origin}/api/drops/${step.id}`, { cache: 'no-store', signal: AbortSignal.timeout(30000) }).catch((e) => ({ ok: false, status: e.message }));
    if (drop.status === 404) return fail('link_gone', `share: ${step.url} no longer exists (deleted?). Pass --new-link to make another.`);
    if (!drop.ok) return fail('appdrop_unavailable', `share: AppDrop answered ${drop.status} for ${step.url}; try again.`);
    revision = (await drop.json()).revision ?? 1;
  }

  const ipa = resolve(app.dir, '.exact/share', `${app.name}.ipa`);
  rmSync(ipa, { force: true });
  mkdirSync(dirname(ipa), { recursive: true });
  say(`share: building ${app.displayName ?? app.name} for iPhone (build ${revision + 1}); the first build takes minutes.`);
  // An app with an update store starts its own stream: this build is a tester's, not a release in one.
  const streamed = app.manifest.deploy?.store?.ios !== '0';
  const built = spawnSync(process.execPath, [resolve(ROOT, 'host/apple/build.mjs'), '--device', `${name}-apple`, '--archive', ipa, '--unsigned'], {
    cwd: ROOT, stdio: ['ignore', 2, 2],
    env: { ...process.env, EXACT_BUILD_NUMBER: String(revision + 1), ...(streamed ? { EXACT_UPDATE_GENESIS: '1' } : {}) },
  });
  if (built.status !== 0 || !existsSync(ipa)) return fail('build_failed', `share: the iPhone build failed (${built.error?.message ?? `exit ${built.status}`}); its errors are above.`);

  say(step.action === 'create' ? 'share: uploading to a new AppDrop link' : `share: uploading to ${step.url}`);
  const uploaded = spawnSync('npx', ['--yes', CLI, ipa, ...(step.action === 'update' ? ['--update', step.url] : []), '--json'], {
    cwd: app.dir, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'],
  });
  // The CLI prints a new link's update link, which holds its key: keep that line out.
  for (const line of (uploaded.stderr ?? '').split('\n')) if (line && !line.includes('/manage#')) say(`  appdrop: ${line}`);
  let result = null;
  try { result = JSON.parse(uploaded.stdout); } catch {}
  if (uploaded.status !== 0 || !parseLink(result?.url ?? '')) {
    return fail('upload_failed', `share: npx ${CLI} failed: ${result?.error ?? uploaded.error?.message ?? `exit ${uploaded.status}`}`);
  }
  const url = parseLink(result.url).url;
  writeFileSync(recordPath, `${JSON.stringify({ url, branch, source, revision: result.revision ?? revision + 1, at: new Date().toISOString() }, null, 2)}\n`);
  rmSync(ipa, { force: true });
  say(`share: open this on an iPhone to install ${app.displayName ?? app.name}; it installs with that person's own Apple ID.`);
  console.log(json ? JSON.stringify({ url, revision: result.revision ?? revision + 1, branch, action: step.action === 'create' ? 'created' : 'updated' }) : url);
  return 0;
}

if (import.meta.main) process.exit(await main(process.argv.slice(2)));
