// The development server's install pages and its Mac-local iOS build (LLP
// 1030.003 D6a/D6b), for either dev loop: the JS target's (host/web-js/dev.mjs)
// and the resident wasm loop (host/web/dev.mjs). Only a page read over
// loopback, from a loopback peer, carries the per-process token that starts
// a build; every reader gets the server's addresses.
// @ref LLP 1030.003#d6b--local-ios-installation
import { spawn } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { resolve } from 'node:path';
import { developmentInstallPage, installNetworkPage, localInstallURL, INSTALL_FILES, LOCAL_IOS_INSTALL_ENDPOINT } from '../../scripts/install-page.mjs';
import { phones, simulators } from '../apple/build.mjs';

const root = resolve(new URL('../..', import.meta.url).pathname);
const publicTarget = target => ({ id: target.id, kind: target.kind, name: target.name, model: target.model, os: target.os, state: target.state });
const simulatorOS = runtime => (/(?:^|\.)iOS-(\d+)-(\d+)(?:-(\d+))?$/.exec(runtime)?.slice(1).filter(Boolean).join('.') ?? runtime);

/** `app()` is the app being served now; `origins` and `port` are the
 * addresses the server printed, `listener` the address it is bound to. */
export function localInstaller({ app, origins, port, gate, listener }) {
  const token = randomBytes(32).toString('hex');
  let child = null;
  let targets = [], targetsAt = 0, targetError = null;
  let state = { state: 'idle', message: 'Looking for an iOS Simulator or paired device…', log: '' };
  function refreshTargets(force = false) {
    if (process.platform !== 'darwin') {
      targets = []; targetError = 'Local iOS builds require a Mac running this development server.';
      return;
    }
    if (!force && Date.now() - targetsAt < 3000) return;
    targetsAt = Date.now();
    const found = [], errors = [];
    try { found.push(...phones().filter(device => device.reachable && device.paired).map(device => ({
      id: `device:${device.id}`, value: device.id, kind: 'device', name: device.name, model: device.model, os: device.os,
    }))); } catch (error) { errors.push(error.message); }
    try { found.push(...simulators().filter(device => /SimRuntime\.iOS/.test(device.runtime) && /^(iPhone|iPad)/.test(device.name)).map(device => ({
      id: `simulator:${device.udid}`, value: device.udid, kind: 'simulator', name: device.name,
      model: 'Simulator', os: simulatorOS(device.runtime), state: device.state,
    }))); } catch (error) { errors.push(error.message); }
    targets = found.sort((a, b) => Number(b.state === 'Booted') - Number(a.state === 'Booted') || a.kind.localeCompare(b.kind) || a.name.localeCompare(b.name));
    targetError = found.length ? null : errors.join(' ');
  }
  function status(forceTargets = false) {
    if (state.state !== 'building') refreshTargets(forceTargets);
    let message = state.message;
    if (state.state === 'idle') {
      if (targetError) message = targetError;
      else if (!targets.length) message = 'No available iOS Simulator or reachable paired device was found. Add a Simulator in Xcode, or unlock and pair a physical device.';
      else message = 'Ready to build locally. The first build can take a few minutes.';
    }
    return { ...state, message, available: process.platform === 'darwin' && !targetError, targets: targets.map(publicTarget) };
  }
  const appendLog = chunk => { state.log = (state.log + String(chunk)).replace(/\r/g, '').slice(-16000); };
  function start(targetId) {
    if (child) { const error = new Error('A local iOS build is already running.'); error.status = 409; throw error; }
    refreshTargets(true);
    if (targetError) { const error = new Error(targetError); error.status = 503; throw error; }
    const target = targets.find(candidate => candidate.id === targetId);
    if (!target) { const error = new Error('That Simulator or device is no longer available. Refresh the target list.'); error.status = 400; throw error; }
    const served = app(), appURL = new URL(localInstallURL(target.kind, origins, port));
    state = { state: 'building', message: `Building ${served.displayName} for ${target.name}. This can take a few minutes…`, log: '', target: publicTarget(target), startedAt: new Date().toISOString() };
    const destination = target.kind === 'simulator' ? ['--ios', served.crate('apple'), '--sim', target.value] : ['--device', served.crate('apple'), '--phone', target.value];
    const build = child = spawn(process.execPath, [resolve(root, 'host/apple/build.mjs'), ...destination, '--run', '--url', appURL.href], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: served.dir, EXACT_UPDATE_TRUST: 'development' }, stdio: ['ignore', 'pipe', 'pipe'],
    });
    build.stdout.on('data', appendLog);
    build.stderr.on('data', appendLog);
    build.on('error', error => {
      if (child !== build) return;
      child = null;
      state = { ...state, state: 'failed', message: `The local build could not start: ${error.message}`, completedAt: new Date().toISOString() };
    });
    build.on('exit', (code, signal) => {
      if (child !== build) return;
      child = null;
      const failed = code !== 0;
      const detail = state.log.trim().split('\n').filter(Boolean).at(-1);
      state = { ...state, state: failed ? 'failed' : 'installed',
        message: failed ? `The local build failed${detail ? `: ${detail}` : ` (${signal ?? `exit ${code}`})`}` : `${served.displayName} was installed and opened on ${target.name}.`,
        completedAt: new Date().toISOString() };
    });
    return status();
  }
  return {
    /** The running build, for a server's shutdown to stop and wait on. */
    get child() { return child; },
    /** A baked install page as this server serves it; any other body unchanged. */
    page(route, body, access) {
      if (!INSTALL_FILES.includes(route)) return body;
      const page = process.platform === 'darwin' && access.local
        ? developmentInstallPage(body.toString(), token)
        : body.toString().replace('<!-- exact-serving -->Static hosting<!-- /exact-serving -->', 'Development server');
      return installNetworkPage(page, listener);
    },
    /** Answers `LOCAL_IOS_INSTALL_ENDPOINT` (GET: status, POST: build); false for any other path. */
    async handle(req, res, url, access) {
      if (url.pathname !== LOCAL_IOS_INSTALL_ENDPOINT) return false;
      const json = (code, body) => { res.writeHead(code, { 'content-type': 'application/json', 'cache-control': 'no-store' }); res.end(JSON.stringify(body) + '\n'); };
      if (!access.local || req.headers['x-exact-install-token'] !== token) { json(404, { message: 'Not found.' }); return true; }
      if (req.method === 'GET' || req.method === 'HEAD') {
        const body = status(url.searchParams.get('refresh') === '1');
        res.writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-store' });
        res.end(req.method === 'HEAD' ? undefined : JSON.stringify(body) + '\n');
        return true;
      }
      if (req.method !== 'POST') { res.writeHead(405); res.end(); return true; }
      try {
        if (!gate.loopbackOrigins.includes(req.headers.origin)) { const error = new Error('The install request must come from this development server.'); error.status = 403; throw error; }
        if (!/^application\/json(?:\s*;|$)/i.test(req.headers['content-type'] ?? '')) { const error = new Error('The install request must be JSON.'); error.status = 415; throw error; }
        let size = 0, encoded = '';
        for await (const chunk of req) {
          size += chunk.length;
          if (size > 1024) { const error = new Error('The install request is too large.'); error.status = 413; throw error; }
          encoded += chunk;
        }
        const body = JSON.parse(encoded || '{}');
        if (typeof body.target !== 'string' || body.target.length > 128) { const error = new Error('Choose an available iOS Simulator or paired device.'); error.status = 400; throw error; }
        json(202, start(body.target));
      } catch (error) {
        json(error.status ?? 400, { message: error instanceof SyntaxError ? 'The install request must be JSON.' : error.message });
      }
      return true;
    },
  };
}
