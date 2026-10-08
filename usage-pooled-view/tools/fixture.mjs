#!/usr/bin/env bun
// usage-pooled-view fixture proxy (uncommitted): HTTP + WebSocket in front of one lane T3 server.
// It overlays the provider snapshots (usage limits, banked credits, a second Codex account, a ready
// Cursor), the hub sources (`usageLimitSourcesUpdated`, only for a config subscription that asked
// for them), the environment's label and the usage summary; it answers provider.consumeResetCredit,
// server.refreshUsageRates and the Cursor Keychain settings write with scripted replies (the real
// redeem would spend a credit; the real Keychain read would raise an OS prompt), and logs every
// RPC (time, tag, payload; never HTTP bodies, tokens or cookies).
//   bun fixture.mjs <a|b> <listen port> <upstream port> <label> <rpc log>
import { appendFileSync, existsSync, readFileSync } from 'node:fs';

const [name, listenText, upstreamText, label, logFile] = process.argv.slice(2);
const listen = Number(listenText), upstream = Number(upstreamText);
const scenarioFile = process.env.UPV_SCENARIO_FILE;
const scenario = () => { try { return existsSync(scenarioFile) ? JSON.parse(readFileSync(scenarioFile, 'utf8')) : {}; } catch { return {}; } };
const log = (line) => appendFileSync(logFile, `${new Date().toISOString()} ${line}\n`);
const state = { cursorEnabled: false, redeems: 0 };

const MIN = 60_000, HOUR = 60 * MIN, DAY = 24 * HOUR;
const at = (offset) => new Date((scenario().epoch ?? Date.now()) + offset).toISOString();
const win = (id, kind, label, usedPercent, mins, resets) => ({ id, kind, label, usedPercent, windowDurationMins: mins, resetsAt: at(resets) });
const codexBase = { driver: 'codex', enabled: true, installed: true, status: 'ready', availability: 'available', version: '0.130.0', slashCommands: [], skills: [] };

function overlayProviders(providers) {
  const list = (providers ?? []).filter((provider) => !(provider.instanceId === 'codex-work' || (name === 'b' && provider.driver === 'cursor')));
  const codex = list.find((provider) => provider.instanceId === 'codex') ?? { instanceId: 'codex', models: [] };
  const shared = { ...codex, ...codexBase, instanceId: 'codex', displayName: codex.displayName ?? 'Codex', message: undefined,
    auth: { status: 'authenticated', type: 'chatgpt', email: 'shared@lane.test', label: 'ChatGPT Pro' } };
  if (name === 'a') {
    shared.usageLimits = { checkedAt: at(-MIN), windows: [win('primary', 'session', '5h limit', 38, 300, 2 * HOUR + 12 * MIN), win('secondary', 'weekly', 'Weekly limit', 71, 10_080, 3 * DAY + 3 * HOUR)],
      resetCredits: { availableCount: state.redeems >= 2 ? 0 : 2 - state.redeems, nextExpiresAt: at(27 * DAY + 23 * HOUR) } };
  } else {
    shared.usageLimits = { checkedAt: at(-5 * MIN), windows: [win('primary', 'session', '5h limit', 50, 300, 2 * HOUR + 12 * MIN), win('secondary', 'weekly', 'Weekly limit', 75, 10_080, 3 * DAY + 3 * HOUR)] };
  }
  const next = list.map((provider) => (provider.instanceId === 'codex' ? shared : provider));
  if (!next.some((provider) => provider.instanceId === 'codex')) next.unshift(shared);
  if (name === 'b') {
    next.push({ ...codexBase, instanceId: 'codex-work', displayName: 'Work', accentColor: '#2563eb', models: codex.models ?? [], checkedAt: at(-MIN),
      auth: { status: 'authenticated', type: 'chatgpt', email: 'work@lane.test', label: 'ChatGPT Plus' },
      usageLimits: { checkedAt: at(-30_000), windows: [win('primary', 'session', '5h limit', 22, 300, 3 * HOUR + 40 * MIN), win('secondary', 'weekly', 'Weekly limit', 48, 10_080, 5 * DAY)],
        resetCredits: { availableCount: 1, nextExpiresAt: at(12 * DAY) }, externalUsage: { label: 'ChatGPT usage', url: 'https://chatgpt.com/#settings/Usage' } } });
    next.push({ instanceId: 'cursor', driver: 'cursor', displayName: 'Cursor', enabled: true, installed: true, status: 'ready', availability: 'available', version: '1.0.0',
      auth: { status: 'unknown' }, checkedAt: at(-MIN), models: [], slashCommands: [], skills: [] });
  }
  return next;
}
const sources = () => name === 'a' ? [
  { id: 'team-hub', kind: 'cliproxy', label: 'Team hub', checkedAt: at(-MIN), accounts: [{ id: 'claude-ops.json', driver: 'claudeAgent', email: 'ops@lane.test', plan: 'Claude Max',
    usageLimits: { checkedAt: at(-MIN), windows: [win('five_hour', 'session', 'Session', 64, 300, HOUR + 5 * MIN), win('seven_day', 'weekly', 'Weekly', 35, 10_080, 4 * DAY)] } }] },
  { id: 'old-hub', kind: 'cliproxy', label: 'Old hub', checkedAt: at(-MIN), accounts: [], error: 'token expired' },
] : [];
const overlayConfig = (config) => ({ ...config, environment: { ...(config.environment ?? {}), label }, providers: overlayProviders(config.providers) });

function overlaySummary(summary, input) {
  const days = [];
  for (let day = Date.parse(`${input.sinceDay}T00:00:00Z`); day <= Date.parse(`${input.untilDay}T00:00:00Z`) && days.length < 90; day += DAY) days.push(new Date(day).toISOString().slice(0, 10));
  const home = `/lane/${name}/codex`;
  const buckets = days.flatMap((day, index) => {
    const cost = name === 'a' ? 1.2 + (index % 5) * 0.8 : 0.6 + (index % 3) * 0.5;
    return [{ day, provider: 'codex', model: name === 'a' ? 'gpt-5.5' : 'gpt-5.5-mini', sourcePath: home, costUsd: cost, records: 4, unpricedRecords: 0,
      totals: { uncachedInputTokens: 120_000 + index * 900, cachedInputTokens: 380_000, cacheCreationTokens: 0, outputTokens: 40_000 } }];
  });
  const sourcesList = [{ status: 'ok', distinctSessions: name === 'a' ? 14 : 6, scannedFiles: 3, skippedFiles: 0, malformedRecords: 0, message: null,
    fingerprint: { hostId: `host-${name}`, provider: 'codex', resolvedHomePath: home, volumeId: `volume-${name}` } }];
  if (name === 'b' && !state.cursorEnabled) {
    sourcesList.push({ status: 'ok', distinctSessions: 0, scannedFiles: 0, skippedFiles: 0, malformedRecords: 0, message: 'Cursor account usage is off on this environment.',
      action: 'enableCursorKeychain', fingerprint: { hostId: `host-${name}`, provider: 'cursor', resolvedHomePath: '/lane/b/.cursor/auth.json', volumeId: `volume-${name}` } });
  }
  return { ...summary, buckets, sources: sourcesList, readAt: at(0) };
}

const success = (requestId, value) => JSON.stringify({ _tag: 'Exit', requestId, exit: { _tag: 'Success', value } });
const failure = (requestId, error) => JSON.stringify({ _tag: 'Exit', requestId, exit: { _tag: 'Failure', cause: [{ _tag: 'Fail', error }] } });
const later = (ms, fn) => setTimeout(fn, ms);

Bun.serve({
  port: listen, hostname: '127.0.0.1',
  async fetch(request, server) {
    const url = new URL(request.url);
    if (url.pathname === '/ws' && server.upgrade(request, { data: { query: url.search } })) return undefined;
    const headers = new Headers(request.headers);
    headers.delete('host');
    const response = await fetch(`http://127.0.0.1:${upstream}${url.pathname}${url.search}`, { method: request.method, headers,
      body: request.method === 'GET' || request.method === 'HEAD' ? undefined : await request.arrayBuffer(), redirect: 'manual' });
    const out = new Headers(response.headers);
    out.delete('content-encoding'); out.delete('content-length');
    if (url.pathname === '/.well-known/t3/environment' && response.ok) {
      const descriptor = await response.json();
      return Response.json({ ...descriptor, label }, { status: response.status, headers: out });
    }
    return new Response(await response.arrayBuffer(), { status: response.status, headers: out });
  },
  websocket: {
    open(ws) {
      ws.data.pending = [];
      ws.data.tags = new Map();
      const upstreamSocket = new WebSocket(`ws://127.0.0.1:${upstream}/ws${ws.data.query}`);
      ws.data.upstream = upstreamSocket;
      upstreamSocket.onopen = () => { for (const frame of ws.data.pending.splice(0)) upstreamSocket.send(frame); };
      upstreamSocket.onclose = (event) => { try { ws.close(event.code === 1005 ? 1000 : event.code, event.reason); } catch {} };
      upstreamSocket.onmessage = (event) => {
        const text = typeof event.data === 'string' ? event.data : new TextDecoder().decode(event.data);
        let parsed;
        try { parsed = JSON.parse(text); } catch { ws.send(text); return; }
        const frames = [parsed].flat().map((frame) => {
          const entry = ws.data.tags.get(String(frame.requestId));
          if (!entry) return frame;
          if (frame._tag === 'Exit' && frame.exit?._tag === 'Success') {
            ws.data.tags.delete(String(frame.requestId));
            if (entry.tag === 'server.getConfig') return { ...frame, exit: { ...frame.exit, value: overlayConfig(frame.exit.value) } };
            if (entry.tag === 'server.getUsageSummary') return { ...frame, exit: { ...frame.exit, value: overlaySummary(frame.exit.value, entry.payload) } };
          }
          if (frame._tag === 'Chunk' && entry.tag === 'subscribeServerConfig') {
            const values = [];
            for (const value of frame.values ?? []) {
              if (value.type === 'snapshot') {
                values.push({ ...value, config: overlayConfig(value.config) });
                if (entry.payload?.usageLimitSources === true) values.push({ type: 'usageLimitSourcesUpdated', payload: { sources: sources() } });
              } else if (value.type === 'providerStatuses') values.push({ ...value, payload: { ...value.payload, providers: overlayProviders(value.payload?.providers) } });
              else if (value.type === 'usageLimitSourcesUpdated') { if (entry.payload?.usageLimitSources === true) values.push({ ...value, payload: { sources: sources() } }); }
              else values.push(value);
            }
            return { ...frame, values };
          }
          return frame;
        });
        const send = (out) => { if (ws.readyState === 1) ws.send(JSON.stringify(Array.isArray(parsed) ? out : out[0])); };
        const delay = ws.data.summaryDelay?.get?.(String(parsed.requestId));
        if (delay) { ws.data.summaryDelay.delete(String(parsed.requestId)); later(delay, () => send(frames)); } else send(frames);
      };
    },
    message(ws, message) {
      const text = typeof message === 'string' ? message : new TextDecoder().decode(message);
      let frame;
      try { frame = JSON.parse(text); } catch { frame = null; }
      const forward = () => { if (ws.data.upstream.readyState === 1) ws.data.upstream.send(text); else ws.data.pending.push(text); };
      if (!frame || frame._tag !== 'Request') { forward(); return; }
      const { id, tag, payload } = frame;
      log(`${name} ${tag} ${JSON.stringify(payload ?? {}).slice(0, 400)}`);
      const plan = scenario();
      if (tag === 'provider.consumeResetCredit') {
        const outcomes = plan.redeem ?? ['reset'];
        const outcome = outcomes[Math.min(state.redeems, outcomes.length - 1)];
        state.redeems += 1;
        later(plan.redeemDelayMs ?? 1500, () => {
          if (outcome === 'error') ws.send(failure(id, { _tag: 'UsageLimitSourceError', detail: 'The hub refused the credit.' }));
          else ws.send(success(id, outcome === 'warning' ? { outcome: 'reset', warning: 'Redeemed, but the hub cooldown could not be cleared.' } : { outcome }));
          log(`${name} reply provider.consumeResetCredit ${outcome}`);
        });
        return;
      }
      if (tag === 'server.refreshUsageRates') {
        const result = plan.rates?.[name] ?? 'ok';
        later(plan.ratesDelayMs ?? 800, () => {
          if (result === 'fail') ws.send(failure(id, { _tag: 'UsagePricingError', message: 'Pricing could not be refreshed.' }));
          else ws.send(success(id, { status: 'fresh', source: 'lane', fetchedAt: at(0), knownModels: 12 }));
          log(`${name} reply server.refreshUsageRates ${result}`);
        });
        return;
      }
      if (tag === 'server.updateSettings' && payload?.patch?.cursorKeychainUsageEnabled === true) {
        state.cursorEnabled = true;
        later(300, () => { ws.send(success(id, { settings: {} })); log(`${name} reply server.updateSettings cursorKeychainUsageEnabled (scripted, no Keychain read)`); });
        return;
      }
      if (['server.getConfig', 'server.getUsageSummary', 'subscribeServerConfig'].includes(tag)) ws.data.tags.set(String(id), { tag, payload });
      if (tag === 'server.getUsageSummary' && plan.summaryDelayMs?.[name]) { ws.data.summaryDelay = ws.data.summaryDelay ?? new Map(); ws.data.summaryDelay.set(String(id), plan.summaryDelayMs[name]); }
      forward();
    },
    close(ws) { try { ws.data.upstream?.close(); } catch {} },
  },
});
log(`${name} fixture listening on ${listen} -> ${upstream} (${label})`);
