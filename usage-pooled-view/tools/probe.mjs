// Probe a fixture proxy as a client would (never prints the token).
import { readFileSync } from 'node:fs';
import { connect } from '../../../examples/t3-code/tools/github-lane/lib.mjs';
const [name] = process.argv.slice(2);
const url = new URL(readFileSync(`${import.meta.dir}/${name}/pairing-url`, 'utf8').trim());
const conn = await connect(url.origin, new URLSearchParams(url.hash.slice(1)).get('token'));
const config = await conn.request('server.getConfig', {});
console.log('label', config.environment.label, 'caps.usageLimitSources', config.environment.capabilities?.usageLimitSources);
console.log('providers', config.providers.map(p => `${p.instanceId}:${p.status}:${p.usageLimits ? p.usageLimits.windows.length : '-'}`).join(' '));
const events = await conn.stream('subscribeServerConfig', { usageLimitSources: true }, values => values.length >= 2, 10000);
console.log('events', events.map(e => `${e.type}${e.type === 'usageLimitSourcesUpdated' ? `(${e.payload.sources.length})` : ''}`).join(' '));
const plain = await conn.stream('subscribeServerConfig', {}, values => values.length >= 1, 10000);
console.log('plain events', plain.map(e => e.type).join(' '));
const summary = await conn.request('server.getUsageSummary', { sinceDay: '2026-10-01', untilDay: '2026-10-08', timeZone: 'Asia/Seoul', resolution: 'day' });
console.log('summary', summary.contractVersion, summary.buckets.length, summary.sources.map(s => s.action ?? s.status).join(','));
conn.close();
process.exit(0);
