// LLP 1133.024 §20 stage 2's drive: the Apple share extension's inbox,
// claimed by Fieldnotes cold (before its boot) and warm (on the extension's
// Darwin notification, and at a foreground). Each share is written as the
// extension writes it (`host/apple/share/ShareInbox.swift`): a directory of
// counter-named files and `share.json` last.
//
// On the iOS simulator the inbox is the App Group's real container, which
// `simctl get_app_container … groups` names. A Mac drive build is ad hoc
// and has no group, so the claim reads `EXACT_AGENT_SHARE_INBOX` instead.
//
// usage: bun apps/fieldnotes/share-inbox.drives.mjs <macos|ios> [--sim <udid>]
//   (Fieldnotes built for that host first: host/apple/build.mjs [--ios] fieldnotes-apple)
import { mkdtempSync, mkdirSync, rmSync, writeFileSync, readdirSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { checker } from '../../packages/pedometer/test/fixture.mjs';

const root = resolve(import.meta.dir, '../..');
const argv = process.argv.slice(2), host = argv[0] ?? 'macos';
if (!['macos', 'ios'].includes(host)) throw new Error('share-inbox.drives.mjs: macos or ios');
if (argv.includes('--sim')) process.env.EXACT_SIM = argv[argv.indexOf('--sim') + 1];
process.env.EXACT_IDENTITY ??= '-';
const { open } = await import(resolve(root, 'scripts/agent.mjs'));
const { check, failed } = checker();
const appId = 'com.exact.fieldnotes', group = `group.${appId}`;
const run = `inbox-${Date.now().toString(36)}`;

const sh = (cmd, args) => {
  const r = spawnSync(cmd, args, { encoding: 'utf8' });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${r.stderr}`);
  return r.stdout.trim();
};
const udid = () => {
  const name = process.env.EXACT_SIM;
  const devices = Object.values(JSON.parse(sh('xcrun', ['simctl', 'list', 'devices', '-j'])).devices).flat();
  const found = devices.find(d => d.udid === name || d.name === name) ?? devices.find(d => d.state === 'Booted');
  if (!found) throw new Error('no simulator: pass --sim');
  return found.udid;
};

/** The inbox: the simulator's group container (the app installed by its build), or a scratch directory on the Mac. */
let inbox, env = {};
if (host === 'ios') {
  const containers = sh('xcrun', ['simctl', 'get_app_container', udid(), appId, 'groups']);
  const line = containers.split('\n').find(l => l.startsWith(`${group}\t`) || l.startsWith(`${group} `));
  if (!line) throw new Error(`the app has no ${group} container (is the build entitled to it?): ${containers}`);
  inbox = join(line.slice(group.length).trim(), 'shares');
} else {
  inbox = mkdtempSync(join(tmpdir(), 'fieldnotes-inbox-'));
  env = { EXACT_AGENT_SHARE_INBOX: inbox };
}
/** The extension's ping. The simulator has no `notifyutil`, and the Mac's
 * notifications do not reach it, so a three-line `notify_post` is built for
 * the simulator and run inside it. */
const ping = () => {
  if (host === 'macos') return sh('notifyutil', ['-p', `${appId}.shares`]);
  const dir = mkdtempSync(join(tmpdir(), 'notify-post-')), c = join(dir, 'post.c'), bin = join(dir, 'post');
  writeFileSync(c, '#include <notify.h>\nint main(int argc, char **argv) { return argc > 1 ? notify_post(argv[1]) : 2; }\n');
  sh('xcrun', ['--sdk', 'iphonesimulator', 'clang', '-target', 'arm64-apple-ios17.0-simulator', c, '-o', bin]);
  sh('xcrun', ['simctl', 'spawn', udid(), bin, `${appId}.shares`]);
  rmSync(dir, { recursive: true, force: true });
};

/** One share as the extension writes it; `complete` false leaves it without `share.json`, as a share being written. */
function write({ title, text, files = [], dropped = {}, complete = true }) {
  const dir = join(inbox, randomUUID());
  mkdirSync(dir, { recursive: true });
  const listed = files.map(([name, type, body], i) => {
    const file = `${i + 1}.${name.split('.').pop()}`;
    writeFileSync(join(dir, file), body);
    return { file, name, type, member: 'files' };
  });
  if (complete) {
    writeFileSync(join(dir, 'share.json.partial'), JSON.stringify({ v: 1, title, text, files: listed, dropped }));
    spawnSync('mv', [join(dir, 'share.json.partial'), join(dir, 'share.json')]);
  }
  return dir;
}

const shown = (tree, id) => (tree?.nodes ?? []).filter(n => n.props?.testId === id).map(n => n.props.value ?? n.props.text ?? n.text ?? '');
const until = async (s, ok, tries = 20) => { for (let i = 0; i < tries; i++) { const t = await s.tree(); if (ok(t)) return t; await s.clock('+250 real'); } return s.tree(); };
const left = () => existsSync(inbox) ? readdirSync(inbox).filter(n => !n.startsWith('.')) : [];

mkdirSync(inbox, { recursive: true });
for (const n of left()) rmSync(join(inbox, n), { recursive: true, force: true });
try {
  // 1: cold. The oldest complete share is the launch location; its file lands; a share still being written stays.
  const partial = write({ text: 'Not yet', complete: false });
  const cold = write({ title: 'From the inbox', text: 'Packed by the extension', files: [['packing.md', 'text/markdown', '# Packing\nwater, map']], dropped: { files: 1 } });
  const s = await open({ host, app: 'fieldnotes', storage: `fn-${run}`, env });
  try {
    const t = await until(s, t => (shown(t, 'share-body')[0] ?? '').includes('# Packing'));
    check(host, '1: a cold inbox share opens on its route with its title and text', shown(t, 'share-title').includes('From the inbox') && (shown(t, 'share-body')[0] ?? '').startsWith('Packed by the extension'), JSON.stringify(t).slice(0, 400));
    check(host, '1: its file lands under its sender\'s name and joins the body', shown(t, 'share-file').join(',') === 'packing.md' && (shown(t, 'share-body')[0] ?? '').includes('# Packing'), shown(t, 'share-file').join(','));
    check(host, '1: the extension\'s dropped count reaches the route', shown(t, 'share-dropped').includes("1 file couldn't be added."), shown(t, 'share-dropped'));
    check(host, '1: the claimed share\'s directory is gone; the incomplete one stays', !existsSync(cold) && existsSync(partial), left().join(','));
    rmSync(partial, { recursive: true, force: true });
    await s.tap('save-share');
    await s.clock('settle');

    // 2: warm, on the ping: the app is open, the extension writes and posts.
    write({ title: 'Second', text: 'While it runs' });
    ping();
    const w = await until(s, t => shown(t, 'share-title').includes('Second'));
    check(host, '2: a warm share arrives on the Darwin notification', shown(w, 'share-title').includes('Second') && shown(w, 'share-body').includes('While it runs'), JSON.stringify(w).slice(0, 400));
    check(host, '2: the inbox is empty after the claim', left().length === 0, left().join(','));
    const logs = ((await s.logs()).lines ?? []).join('\n');
    check(host, '2: the journal names the members, never the text', /share: received title, text/.test(logs) && !logs.includes('While it runs'), logs.split('\n').filter(l => l.includes('share')).join(' | '));
  } finally { await s.close(); }
  // The boot with a cold share beside one with an empty inbox: the claim before the boot reads a record and renames.
  const empty = await open({ host, app: 'fieldnotes', storage: `fn-${run}-empty`, env });
  await empty.close();
  console.log(`boot: ${s.boot} ms with a cold share, ${empty.boot} ms with an empty inbox`);
} finally {
  if (host === 'macos') rmSync(inbox, { recursive: true, force: true });
}
console.log(`${failed()} failed`);
process.exit(failed() ? 1 : 0);
