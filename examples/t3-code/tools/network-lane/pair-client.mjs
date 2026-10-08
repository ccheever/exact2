#!/usr/bin/env bun
// A second client for the lane (20261005-this-machine-network-access, "Pair another client" and
// "Revoke"): redeems a pairing URL as a client does (POST /oauth/token with the URL's #token, a
// websocket ticket, then /ws), stays connected, and logs each step with a timestamp (never the
// credential or the bearer). Once the session is revoked it logs the refusal of a new socket and
// exits. Usage: bun pair-client.mjs <file holding the pairing URL> <log file> [client label]
import { appendFileSync, readFileSync } from 'node:fs';
const [urlFile, logFile, label = 'Lane second client'] = process.argv.slice(2);
const log = (line) => appendFileSync(logFile, `${new Date().toISOString()} ${line}\n`);
const pairing = new URL(readFileSync(urlFile, 'utf8').trim());
const token = new URLSearchParams(pairing.hash.slice(1)).get('token');
const origin = pairing.searchParams.get('host') ? new URL(pairing.searchParams.get('host')).origin : pairing.origin;
const form = new URLSearchParams({ grant_type: 'urn:ietf:params:oauth:grant-type:token-exchange', subject_token: token,
  subject_token_type: 'urn:t3:params:oauth:token-type:environment-bootstrap', requested_token_type: 'urn:ietf:params:oauth:token-type:access_token',
  client_label: label, client_device_type: 'desktop', client_os: 'macOS' });
const exchanged = await fetch(`${origin}/oauth/token`, { method: 'POST', headers: { 'content-type': 'application/x-www-form-urlencoded' }, body: form });
log(`exchange ${origin} HTTP ${exchanged.status}`);
if (!exchanged.ok) { log(`exchange body ${(await exchanged.text()).slice(0, 200)}`); process.exit(1); }
const { access_token: bearer, scope } = await exchanged.json();
log(`scope ${scope}`);
const ticketReply = await fetch(`${origin}/api/auth/websocket-ticket`, { method: 'POST', headers: { authorization: `Bearer ${bearer}` } });
log(`ticket HTTP ${ticketReply.status}`);
const { ticket } = await ticketReply.json();
const ws = new WebSocket(`${origin.replace(/^http/, 'ws')}/ws?wsTicket=${encodeURIComponent(ticket)}&orchestrationProtocol=2`);
ws.addEventListener('open', () => log('socket open'));
ws.addEventListener('close', (event) => log(`socket closed code=${event.code}`));
ws.addEventListener('error', () => log('socket error'));
// The server checks a session when a socket opens and on each request (SessionStore verify): poll the
// session each second; once it is revoked, try what a client does next (a new websocket ticket) and exit.
let last = true;
setInterval(async () => {
  const reply = await fetch(`${origin}/api/auth/session`, { headers: { authorization: `Bearer ${bearer}` } }).catch(() => null);
  if (!reply) return;
  const authenticated = (await reply.json().catch(() => ({}))).authenticated === true;
  if (authenticated === last) return;
  last = authenticated;
  log(`session check: authenticated=${authenticated} (HTTP ${reply.status})`);
  if (!authenticated) {
    const again = await fetch(`${origin}/api/auth/websocket-ticket`, { method: 'POST', headers: { authorization: `Bearer ${bearer}` } }).catch(() => null);
    log(`new websocket ticket after revoke: HTTP ${again?.status ?? 'none'}`);
    ws.close(); setTimeout(() => process.exit(0), 300);
  }
}, 1000);
