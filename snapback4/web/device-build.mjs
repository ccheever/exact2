// Build provenance and preflight for the app's optional Snapback device.
import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';

const workspace = resolve(import.meta.dirname, '..');
export const deviceHash = bytes => createHash('sha256').update(bytes).digest('hex');
export function engineVersion() {
  const lock = readFileSync(resolve(workspace, 'Cargo.lock'), 'utf8');
  const version = /\[\[package\]\]\s+name = "snapback4-core"\s+version = "([^"]+)"/.exec(lock)?.[1];
  if (!version) throw new Error('Snapback4: Cargo.lock has no device engine version');
  return version;
}
export function validateDevice(bytes, stage = 'device') {
  if (!WebAssembly.validate(bytes)) throw new Error(`Snapback4 ${stage}: empty or invalid wasm; rebuild with bun exact.mjs snapback --rebuild`);
}
export function readDeviceStamp(path) {
  if (!existsSync(path)) throw new Error(`Snapback4 device is missing at ${path}; run bun exact.mjs snapback --rebuild`);
  const bytes = readFileSync(path);
  validateDevice(bytes);
  // Prefer the engine's embedded stamp when supplied by Snapback's build.
  const embedded = WebAssembly.Module.customSections(new WebAssembly.Module(bytes), 'snapback4.build');
  let stamp;
  try {
    stamp = embedded.length ? JSON.parse(Buffer.from(embedded[0]).toString()) : JSON.parse(readFileSync(path + '.json', 'utf8'));
  } catch { throw new Error('Snapback4 device has no readable build stamp; run bun exact.mjs snapback --rebuild'); }
  if (typeof stamp.engineVersion !== 'string' || (!embedded.length && stamp.sha256 !== deviceHash(bytes))) {
    throw new Error('Snapback4 device build stamp does not match its wasm; run bun exact.mjs snapback --rebuild');
  }
  return stamp;
}
export const mountsSnapback = app => Object.values(app.manifest?.typescript?.sources ?? {}).some(path => /(^|\/)snapback4\/ts\/?$/.test(path));
export async function serverDeviceVersion(origin, ms = 1500) {
  for (const path of ['/dev/status', '/']) {
    try {
      const response = await fetch(origin + path, { signal: AbortSignal.timeout(ms) });
      if (!response.ok) continue;
      const body = await response.json();
      const version = body.device?.engineVersion ?? body.engineVersion ?? body.snapback4;
      if (typeof version === 'string' && /^\d+\.\d+\.\d+/.test(version)) return { ...body.device, engineVersion: version };
    } catch { /* Older servers expose their version at GET / instead. */ }
  }
  return null;
}
/** No UI has opened yet. Old servers compare versions; stamped servers can
 * also name required device capabilities. Offline wiring can still build. */
export async function devicePreflight(app, origins = []) {
  if (!mountsSnapback(app)) return;
  const stamp = readDeviceStamp(resolve(app.dir, 'assets/snapback4.wasm'));
  const installedPath = resolve(dirname(app.dir), 'node_modules/snapback4/package.json');
  const installed = existsSync(installedPath) ? JSON.parse(readFileSync(installedPath, 'utf8')).version : null;
  const compare = (version, where, required = []) => {
    const missing = required.filter(name => !stamp.capabilities?.includes(name));
    if (stamp.engineVersion !== version || missing.length) throw new Error(`E_DEVICE_VERSION: device built for snapback4 ${stamp.engineVersion}; ${where} ${version}${missing.length ? `; lacks ${missing.join(', ')}` : ''}: run bun exact.mjs snapback --rebuild`);
  };
  if (installed) compare(installed, 'installed');
  for (const origin of origins) {
    const server = await serverDeviceVersion(origin);
    if (server) compare(server.engineVersion, `server at ${origin}`, server.requiredCapabilities ?? []);
  }
}
