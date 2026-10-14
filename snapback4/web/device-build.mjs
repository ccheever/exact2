// Build provenance and preflight for the app's optional Snapback device.
import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';

export const deviceHash = bytes => createHash('sha256').update(bytes).digest('hex');
// Same decimal-u32 version grammar as snapback4-core::device_protocol.
export function versionParts(value) {
  if (typeof value !== 'string' || !/^\d+\.\d+\.\d+$/.test(value)) return null;
  const parts = value.split('.').map(Number);
  return parts.every(part => Number.isInteger(part) && part <= 0xffffffff) ? parts : null;
}
// @ref LLP 1086 D12 — the device crate owns build and transport identity.
export function embeddedDeviceStamp(bytes) {
  const sections = WebAssembly.Module.customSections(new WebAssembly.Module(bytes), 'snapback4.engine');
  if (sections.length !== 1) throw new Error('E_DEVICE_VERSION: device needs exactly one snapback4.engine section; run bun exact.mjs snapback --rebuild');
  let stamp, text;
  try { text = new TextDecoder('utf-8', {fatal: true, ignoreBOM: true}).decode(sections[0]); stamp = JSON.parse(text); } catch { throw new Error('E_DEVICE_VERSION: invalid snapback4.engine JSON; run bun exact.mjs snapback --rebuild'); }
  if (!stamp || stamp.format !== 1 || !versionParts(stamp.version) || !Array.isArray(stamp.capabilities) || stamp.capabilities.some(c => typeof c !== 'string' || !c.isWellFormed()))
    throw new Error('E_DEVICE_VERSION: invalid snapback4.engine identity; run bun exact.mjs snapback --rebuild');
  const canonical = JSON.stringify({format: stamp.format, version: stamp.version, capabilities: stamp.capabilities});
  if (text !== canonical) throw new Error('E_DEVICE_VERSION: noncanonical snapback4.engine identity; run bun exact.mjs snapback --rebuild');
  return stamp;
}
export function validateDevice(bytes, stage = 'device') {
  if (!WebAssembly.validate(bytes)) throw new Error(`Snapback4 ${stage}: empty or invalid wasm; rebuild with bun exact.mjs snapback --rebuild`);
}
export function readDeviceStamp(path) {
  if (!existsSync(path)) throw new Error(`Snapback4 device is missing at ${path}; run bun exact.mjs snapback --rebuild`);
  const bytes = readFileSync(path);
  validateDevice(bytes);
  return embeddedDeviceStamp(bytes);
}
export const mountsSnapback = app => Object.values(app.manifest?.typescript?.sources ?? {}).some(path => /(^|\/)snapback4\/ts\/?$/.test(path));
export async function serverDeviceVersion(origin, ms = 1500) {
  for (const path of ['/dev/status', '/']) {
    try {
      const response = await fetch(origin.replace(/\/+$/, '') + path, { signal: AbortSignal.timeout(ms) });
      if (!response.ok) continue;
      const body = await response.json();
      if (Object.hasOwn(body ?? {}, 'device_compatibility')) {
        const c = body.device_compatibility;
        if (!c || !versionParts(c.server_version) || !versionParts(c.minimum_version)
          || !Array.isArray(c.capabilities) || c.capabilities.some(cap => typeof cap !== 'string'))
          throw new Error('E_DEVICE_VERSION: invalid server device_compatibility record');
        return c;
      }
    } catch (error) { if (error.message?.startsWith('E_DEVICE_VERSION:')) throw error; /* Offline wiring can still build. */ }
  }
  return null;
}
/** Read the same embedded fact and floor/capability contract as Snapback check. */
export async function devicePreflight(app, origins = []) {
  if (!mountsSnapback(app)) return;
  // Old apps supplied the handshake as a credential header. The device owns it now.
  const source = resolve(app.dir, 'app.ts');
  if (existsSync(source) && /x-snapback-device/i.test(readFileSync(source, 'utf8')))
    console.warn('Snapback4: obsolete x-snapback-device shim in app.ts; delete it. The device sends its own identity on the device-to-server leg; app headers supply credentials only.');
  const stamp = readDeviceStamp(resolve(app.dir, 'assets/snapback4.wasm'));
  for (const origin of origins) {
    const server = await serverDeviceVersion(origin);
    if (!server) continue;
    const current = versionParts(stamp.version), floor = versionParts(server.minimum_version);
    const older = current[0] < floor[0] || (current[0] === floor[0] && (current[1] < floor[1] || (current[1] === floor[1] && current[2] < floor[2])));
    const missing = server.capabilities.filter(name => !stamp.capabilities.includes(name));
    if (older || missing.length) throw new Error(`E_DEVICE_VERSION: device ${stamp.version}, server ${server.server_version}, floor ${server.minimum_version}${missing.length ? `; lacks ${missing.join(', ')}` : ''}: run bun exact.mjs snapback --rebuild`);
  }
}
