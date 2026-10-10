// Simulators, phones, signing identities and provisioning profiles: the
// device side of host/apple/build.mjs, shared with scripts/agent.mjs and the
// smokes, which launch the same bundles.
import { spawnSync } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { homedir, networkInterfaces, tmpdir } from 'node:os';
import { createServer as createTCPServer } from 'node:net';
import { resolve } from 'node:path';
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync } from 'node:fs';

const root = resolve(new URL('../..', import.meta.url).pathname);
/** Use Xcode when `xcode-select` names the Command Line Tools, which carry
 * no iOS SDK and no `simctl` (LLP 1054 O2): the iOS build failed deep in a
 * crate's build script with `SDK "iphonesimulator" cannot be located`, and a
 * driver that resolved Xcode apart from the build could not find `simctl`
 * where the build had just installed the app (shop F20, recipes F16). Every
 * `xcrun` here goes through it, the build's and the driver's alike; an
 * explicit `DEVELOPER_DIR` is kept. Said on stderr: a drive's stdout is its reply. */
export function useXcode() {
  if (process.platform !== 'darwin' || process.env.DEVELOPER_DIR) return;
  const selected = spawnSync('xcode-select', ['-p'], { encoding: 'utf8' }).stdout?.trim() ?? '';
  const xcode = '/Applications/Xcode.app/Contents/Developer';
  if (selected.includes('CommandLineTools') && existsSync(xcode)) {
    process.env.DEVELOPER_DIR = xcode;
    console.error(`host/apple: xcode-select names the Command Line Tools (${selected}); using ${xcode}`);
  }
}
const read = (cmd, args, opts = {}) => { useXcode(); return spawnSync(cmd, args, { cwd: root, encoding: 'utf8', ...opts }); };

/** Every available simulator: { udid, name, runtime, state, type } — `type`
 *  the device type's last component (`iPhone-18-Pro`, `Apple-TV-4K-…`), which a
 *  renamed simulator keeps. */
export function simulators() {
  const r = read('xcrun', ['simctl', 'list', 'devices', 'available', '-j']);
  if (r.status !== 0) throw new Error('xcrun simctl list: ' + r.stderr);
  return Object.entries(JSON.parse(r.stdout).devices).flatMap(([runtime, list]) => list.map((d) => ({ udid: d.udid, name: d.name, runtime, state: d.state, type: d.deviceTypeIdentifier?.split('.').pop() ?? '' })));
}

/** The simulator to use — `pick` (a udid or a name; EXACT_SIM by default), else the one booted iPhone (refused when several are), else the iPhone Pro on the newest iOS — booted and waited for. With `tv`, the same choice among Apple TVs on tvOS. An iPhone is one by its device type, not its name: a simulator renamed `work-phone` is still one. */
export function simulator(pick = process.env.EXACT_SIM, { tv = false } = {}) {
  const all = simulators();
  const version = (d) => Number(/(?:iOS|tvOS)-(\d+)-(\d+)/.exec(d.runtime)?.slice(1).join('.') ?? 0);
  const kind = (d) => d.type || d.name.replaceAll(' ', '-');
  const iphones = all.filter((d) => (tv ? /SimRuntime\.tvOS/.test(d.runtime) && /^Apple-TV/.test(kind(d)) : /SimRuntime\.iOS/.test(d.runtime) && /^iPhone/.test(kind(d)))).sort((a, b) => version(b) - version(a) || a.name.localeCompare(b.name));
  let dev = pick ? all.find((d) => d.udid === pick || d.name === pick) : null;
  if (pick && !dev) throw new Error(`no simulator ${pick} (xcrun simctl list devices available)`);
  // Several booted: whose is which is not this process's to guess (an install onto another drive's simulator,
  // 2026-10-09: build.mjs --ios takes --sim, not --phone, for a simulator). One booted, or none, is still chosen.
  const booted = iphones.filter((d) => d.state === 'Booted');
  if (!pick && booted.length > 1) throw new Error(`${booted.length} ${tv ? 'Apple TV' : 'iPhone'} simulators are booted (${booted.map((d) => `${d.name} ${d.udid}`).join(', ')}): name one with --sim <udid|name> or EXACT_SIM`);
  dev ??= booted[0] ?? iphones.find((d) => /^iPhone-\d+-Pro$/.test(kind(d))) ?? iphones[0];
  if (!dev) throw new Error(`no ${tv ? 'Apple TV' : 'iPhone'} simulator on ${tv ? 'tvOS' : 'iOS'}; add one in Xcode, or name any simulator by udid or name with EXACT_SIM (or --sim)`);
  if (dev.state !== 'Booted') {
    const b = read('xcrun', ['simctl', 'boot', dev.udid]);
    if (b.status !== 0 && !/current state: Booted/.test(b.stderr)) throw new Error('simctl boot: ' + b.stderr);
  }
  const s = read('xcrun', ['simctl', 'bootstatus', dev.udid, '-b']);
  if (s.status !== 0) throw new Error('simctl bootstatus: ' + s.stderr);
  return dev;
}

/** Bring Simulator.app up showing `dev`, so a person watching sees what is
 *  driven there; `background` (`open -g`) leaves keyboard focus where it was.
 *  Xcode 27 has no Simulator.app: its Device Hub (com.apple.dt.Devices) shows
 *  a simulator in a window of that device's own, opened and raised by the
 *  URL Device Hub registers (`devices://device/open?id=<udid>`; launch
 *  arguments never reach a Device Hub that is already running, and its main
 *  window shows whichever device was last picked in its list). */
export function showSimulator(dev, background = false) {
  const g = background ? ['-g'] : [];
  if (spawnSync('open', [...g, '-a', 'Simulator', '--args', '-CurrentDeviceUDID', dev.udid], { stdio: 'ignore' }).status !== 0) spawnSync('open', [...g, `devices://device/open?id=${dev.udid}`], { stdio: 'ignore' });
}

/** Crash reports macOS wrote since `since` (ms) for an executable named
 *  `name` — a simulator app crashes as a Mac process, reported here. */
export function crashReports(name, since) {
  const dir = resolve(homedir(), 'Library/Logs/DiagnosticReports');
  let names = [];
  try { names = readdirSync(dir); } catch { return []; }
  return names.filter((f) => f.startsWith(name + '-') && /\.(ips|crash)$/.test(f))
    .map((f) => resolve(dir, f)).filter((f) => { try { return statSync(f).mtimeMs >= since; } catch { return false; } });
}


// ---------------------------------------------------------------- a phone: devicectl, a profile, an identity

/** One launch, one phone connection; reject other peers before any agent request.
 * The token crosses via the paired device's launch environment, not a public URL. */
export async function phoneBridge() {
  const interfaces = networkInterfaces();
  const address = process.env.EXACT_AGENT_HOST ?? [...(interfaces.en0 ?? []), ...Object.values(interfaces).flat()]
    .find((n) => n.family === 'IPv4' && !n.internal)?.address;
  if (!address) throw new Error('phone agent needs a reachable Mac IPv4 address (EXACT_AGENT_HOST)');
  const token = randomBytes(32).toString('hex');
  const sockets = new Set();
  let accept, fail;
  const ready = new Promise((resolve, reject) => { accept = resolve; fail = reject; });
  const server = createTCPServer((socket) => {
    if (sockets.size >= 8) { socket.destroy(); return; }
    sockets.add(socket);
    socket.on('close', () => sockets.delete(socket));
    socket.on('error', () => {});
    socket.setTimeout(5000, () => socket.destroy());
    let buf = '';
    socket.setEncoding('utf8');
    const hello = (chunk) => {
      buf += chunk;
      if (buf.length > 4096) { socket.destroy(); return; }
      if (!buf.includes('\n')) return;
      let announcement;
      try { announcement = JSON.parse(buf); } catch { socket.destroy(); return; }
      if (!announcement || announcement.token !== token || announcement.ready !== true) { socket.destroy(); return; }
      delete announcement.token;
      socket.pause();
      socket.removeListener('data', hello);
      socket.setTimeout(0);
      server.close();
      for (const other of sockets) if (other !== socket) other.destroy();
      accept({ socket, announcement });
    };
    socket.on('data', hello);
  });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, address, resolve); });
  server.on('error', fail);
  return {
    ready, fail,
    env: { EXACT_AGENT_CONNECT: `${address}:${server.address().port}`, EXACT_AGENT_TOKEN: token },
    close() { for (const socket of sockets) socket.destroy(); server.close(); },
  };
}


/** Every phone this Mac knows (devicectl, which lists simulators too): { id, udid, name, model, os, reachable, simulated }. */
export function phones() {
  const out = resolve(mkdtempSync(resolve(tmpdir(), 'exact-devices-')), 'devices.json');
  const r = read('xcrun', ['devicectl', 'list', 'devices', '--json-output', out]);
  if (r.status !== 0) throw new Error('xcrun devicectl list devices: ' + r.stderr);
  const list = JSON.parse(readFileSync(out, 'utf8')).result.devices.map((d) => ({
    id: d.identifier, udid: d.hardwareProperties?.udid, name: d.deviceProperties?.name, model: d.hardwareProperties?.marketingName,
    os: d.deviceProperties?.osVersionNumber, reachable: d.connectionProperties?.tunnelState !== 'unavailable', paired: d.connectionProperties?.pairingState === 'paired',
    simulated: d.hardwareProperties?.reality === 'simulated',
  }));
  rmSync(resolve(out, '..'), { recursive: true, force: true });
  return list;
}

/** The phone to use: `pick` (a udid or a name; EXACT_PHONE by default), else a reachable phone, else the one phone this Mac knows — the bundle is built and signed for it either way; installing needs it connected (`reachable`). Unpicked, a simulator devicectl lists is no phone. With `tv`, the same choice among physical Apple TVs. */
export function phone(pick = process.env.EXACT_PHONE, { tv = false } = {}) {
  const named = phones().filter((d) => !tv || (!d.simulated && /Apple TV/.test(d.model ?? ''))), all = pick ? named : named.filter((d) => !d.simulated);
  const dev = pick ? all.find((d) => d.udid === pick || d.id === pick || d.name === pick) : all.find((d) => d.reachable) ?? (all.length === 1 ? all[0] : null);
  if (!dev) throw new Error(pick ? `no phone ${pick} (xcrun devicectl list devices)` : `no phone is known to this Mac (${all.length ? all.map((d) => `${d.name}, not connected`).join('; ') : 'xcrun devicectl list devices shows none'}): plug one in, unlock it, and trust this Mac`);
  return dev;
}

/**
 * A phone cannot read a plan or asset directory on this Mac. When the caller
 * names the dev server with EXACT_DEV_PLAN, carry that URL into the launched
 * process; the envelope supplies its own complete asset URLs.
 */
export function deviceLaunchArgs(device, id, environment = process.env) {
  const args = ['devicectl', 'device', 'process', 'launch', '--terminate-existing', '--device', device];
  const locator = environment.EXACT_DEV_PLAN;
  if (locator) {
    let url;
    try { url = new URL(locator); }
    catch { throw new Error(`--device cannot open EXACT_DEV_PLAN=${locator} on this Mac; name the dev server's http(s) URL`); }
    if (url.protocol !== 'http:' && url.protocol !== 'https:') throw new Error(`--device requires EXACT_DEV_PLAN to be an http(s) dev-server URL, not ${locator}`);
    args.push('--environment-variables', JSON.stringify({ EXACT_DEV_PLAN: url.href }));
  }
  args.push(id);
  return args;
}

/** The explicit app URL: the launch locator, and the one dev-server origin a
 * development build admits opening links for (`developmentAdmission`).
 * Validate it before invoking any build or signing tools. @ref LLP 1030.000 §7 */
export function developmentLaunchEnvironment(args, environment = process.env) {
  const index = args.indexOf('--url');
  if (index < 0) return { ...environment };
  if (args.lastIndexOf('--url') !== index) throw new Error('--url may be specified only once');
  if (!['--run', '--bundle', '--ios', '--device'].some((flag) => args.includes(flag))) throw new Error('--url requires --run or a client bundle (--bundle, --ios, --device)');
  let url;
  try { url = new URL(args[index + 1]); } catch { /* diagnosed below */ }
  if (!url || !['http:', 'https:'].includes(url.protocol) || !url.hostname) {
    throw new Error('--url requires an absolute http(s) app URL');
  }
  return { ...environment, EXACT_DEV_PLAN: url.href };
}

/** Whether a profile allows every entitlement in `required` (a grant's
 * signing entitlements, LLP 1069.008.000 D4). */
export const allows = (p, required = []) => !!p && required.every((name) => p.entitlements.includes(name));

/** What a profile that lacks a grant's entitlement needs, said once. */
const missing = (bundle, required, why) => new Error(`grant-device-profile: ${required.join(', ')} ${required.length > 1 ? 'are' : 'is'} needed by this app's grants, and ${why} (a team wildcard never allows HealthKit). In the Apple Developer portal, register the App ID ${bundle} with that capability, make a development profile for it that includes this phone, and install it (or name it with EXACT_PROFILE).`);

/** A development profile on this Mac covering the phone and the bundle id (the team's wildcard or the id itself, under the profile's App ID prefix, which an older team's App IDs keep apart from its team id), unexpired; EXACT_PROFILE names one.
 * With `required` (a grant's signing entitlements), only one that allows them all, which is one for the id itself. */
export function profile(udid, bundle, required = []) {
  if (process.env.EXACT_PROFILE) {
    const named = decodeProfile(process.env.EXACT_PROFILE);
    if (!allows(named, required)) throw missing(bundle, required, `EXACT_PROFILE (${named.name}) does not allow ${required.length > 1 ? 'them' : 'it'}`);
    return named;
  }
  const dirs = ['Library/Developer/Xcode/UserData/Provisioning Profiles', 'Library/MobileDevice/Provisioning Profiles'].map((d) => resolve(homedir(), d)).filter(existsSync);
  const covering = dirs.flatMap((d) => readdirSync(d).filter((f) => f.endsWith('.mobileprovision')).map((f) => decodeProfile(resolve(d, f))))
    .filter((p) => p.dev && p.expires > new Date() && p.devices.includes(udid) && (p.appId === `${p.prefix}.*` || p.appId === `${p.prefix}.${bundle}`))
    .sort((a, b) => b.expires - a.expires);
  if (!covering.length) throw new Error(`no development provisioning profile on this Mac covers ${bundle} on this phone (${udid}); run any app on it from Xcode once with team signing, or name one with EXACT_PROFILE`);
  const found = covering.filter((p) => allows(p, required));
  if (!found.length) throw missing(bundle, required, `no development profile on this Mac for ${bundle} on this phone allows ${required.length > 1 ? 'them' : 'it'}`);
  return found[0];
}

/** The fields of a `.mobileprovision` this script reads (it is a CMS-signed XML plist). */
function decodeProfile(path) {
  const xml = read('security', ['cms', '-D', '-i', path]).stdout ?? '';
  const str = (key) => new RegExp(`<key>${key}</key>\\s*<string>([^<]*)</string>`).exec(xml)?.[1];
  const team = /<key>TeamIdentifier<\/key>\s*<array>\s*<string>([^<]*)<\/string>/.exec(xml)?.[1];
  const devices = [...(/<key>ProvisionedDevices<\/key>\s*<array>([\s\S]*?)<\/array>/.exec(xml)?.[1] ?? '').matchAll(/<string>([^<]*)<\/string>/g)].map((m) => m[1]);
  const expires = /<key>ExpirationDate<\/key>\s*<date>([^<]*)<\/date>/.exec(xml)?.[1];
  // The keys of its Entitlements whose value is not `false`: what it allows.
  const allowed = /<key>Entitlements<\/key>\s*<dict>([\s\S]*?)<\/dict>/.exec(xml)?.[1] ?? '';
  const entitlements = [...allowed.matchAll(/<key>([^<]+)<\/key>\s*(<false\/>)?/g)].filter((m) => !m[2]).map((m) => m[1]);
  const appId = str('application-identifier');
  return { path, name: str('Name'), team, appId, prefix: appId?.split('.')[0], devices, dev: /<key>get-task-allow<\/key>\s*<true\/>/.test(xml), expires: new Date(expires ?? 0), entitlements };
}

/** The Apple Development identity (its SHA-1) for a team, from the keychain; EXACT_IDENTITY names one. */
export function identity(team) {
  if (process.env.EXACT_IDENTITY) return process.env.EXACT_IDENTITY;
  const valid = [...(read('security', ['find-identity', '-v', '-p', 'codesigning']).stdout ?? '').matchAll(/\d+\) ([0-9A-F]{40}) "(Apple Development: [^"]+)"/g)].map((m) => ({ sha1: m[1], name: m[2] }));
  const pems = (read('security', ['find-certificate', '-a', '-c', 'Apple Development', '-p']).stdout ?? '').split('-----END CERTIFICATE-----').filter((c) => c.includes('BEGIN CERTIFICATE'));
  for (const pem of pems) {
    const x = read('openssl', ['x509', '-noout', '-subject', '-fingerprint', '-sha1'], { input: pem + '-----END CERTIFICATE-----\n' }).stdout ?? '';
    const sha1 = /Fingerprint=([0-9A-F:]+)/i.exec(x)?.[1].replace(/:/g, '');
    const ou = /OU\s*=\s*([A-Z0-9]+)/.exec(x)?.[1];
    const id = valid.find((v) => v.sha1 === sha1);
    if (id && ou === team) return id.sha1;
  }
  throw new Error(`no valid "Apple Development" identity for team ${team} in the keychain (security find-identity -v -p codesigning); name one with EXACT_IDENTITY=<sha1>`);
}

/** The first valid "Apple Development" identity in the keychain (EXACT_IDENTITY names one), or null: the macOS binary is then ad-hoc signed. */
export function macIdentity() {
  if (process.env.EXACT_IDENTITY) return process.env.EXACT_IDENTITY;
  const m = /\d+\) ([0-9A-F]{40}) "Apple Development: /.exec(read('security', ['find-identity', '-v', '-p', 'codesigning']).stdout ?? '');
  return m ? m[1] : null;
}
