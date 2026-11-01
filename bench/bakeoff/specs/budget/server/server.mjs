#!/usr/bin/env bun
import { readFileSync } from 'node:fs';

const portFlag = process.argv.indexOf('--port');
const port = Number(portFlag >= 0 ? process.argv[portFlag + 1] : 43000);
const seed = JSON.parse(readFileSync(new URL('../data/transactions.json', import.meta.url)));
let rows = seed.map((row) => ({ ...row }));
let next = Math.max(...rows.map((row) => row.id)) + 1;
const json = (value, status = 200) => new Response(JSON.stringify(value), { status, headers: { 'content-type': 'application/json', 'access-control-allow-origin': '*' } });
const read = async (request) => { try { return await request.json(); } catch { return null; } };
const server = Bun.serve({ port, async fetch(request) {
  const url = new URL(request.url);
  if (request.method === 'OPTIONS') return new Response('', { headers: { 'access-control-allow-origin': '*', 'access-control-allow-methods': 'GET,POST,PATCH,DELETE,OPTIONS', 'access-control-allow-headers': 'content-type' } });
  if (url.pathname === '/health') return json({ ok: true });
  if (url.pathname === '/transactions' && request.method === 'GET') return json(rows);
  const match = url.pathname.match(/^\/transactions\/(\d+)$/);
  if (match && ['PATCH', 'DELETE'].includes(request.method)) {
    const id = Number(match[1]), index = rows.findIndex((row) => row.id === id);
    if (index < 0) return json({ error: 'transaction_not_found' }, 404);
    if (request.method === 'DELETE') { rows.splice(index, 1); return json({ ok: true }); }
    const update = await read(request);
    if (Number(update?.amount) > 500) return json({ error: 'amount_limit', message: 'Transactions over 500 require approval.' }, 422);
    rows[index] = { ...rows[index], ...update, id };
    return json(rows[index]);
  }
  if (url.pathname === '/transactions' && request.method === 'POST') {
    const body = await read(request);
    if (Number(body?.amount) > 500) return json({ error: 'amount_limit', message: 'Transactions over 500 require approval.' }, 422);
    const row = { ...body, id: next++ }; rows.unshift(row); return json(row, 201);
  }
  return json({ error: 'not_found' }, 404);
} });
console.log(`LISTENING ${server.port}`);
