#!/usr/bin/env bun
import { readFileSync } from 'node:fs';

const portFlag = process.argv.indexOf('--port');
const port = Number(portFlag >= 0 ? process.argv[portFlag + 1] : 43000);
let board = JSON.parse(readFileSync(new URL('../data/board.json', import.meta.url)));
const clients = new Set();
const send = (client, message) => client.send(JSON.stringify(message));
const broadcast = (message) => { for (const client of clients) send(client, message); };
const move = (message) => {
  const from = board.columns.find((column) => column.id === message.from);
  const to = board.columns.find((column) => column.id === message.to);
  if (!from || !to) return false;
  const index = from.cards.findIndex((card) => card.id === message.cardId);
  if (index < 0) return false;
  const [card] = from.cards.splice(index, 1);
  to.cards.splice(Math.max(0, Math.min(Number(message.index) || 0, to.cards.length)), 0, card);
  return true;
};
const server = Bun.serve({ port, fetch(request, server) {
  const url = new URL(request.url);
  if (url.pathname === '/simulate') { broadcast({ type: 'move', from: 'backlog', to: 'doing', cardId: board.columns[0].cards[0]?.id, index: 0 }); return new Response('ok'); }
  if (url.pathname !== '/socket' || !server.upgrade(request, { data: {} })) return new Response('not found', { status: 404 });
}, websocket: {
  open(client) { clients.add(client); send(client, { type: 'board', board }); broadcast({ type: 'presence', count: clients.size - 1 }); },
  message(client, raw) { let message; try { message = JSON.parse(String(raw)); } catch { return; }
    if (message.type === 'move' && move(message)) broadcast({ type: 'move', ...message });
    if (message.type === 'edit') broadcast(message);
  },
  close(client) { clients.delete(client); broadcast({ type: 'presence', count: Math.max(0, clients.size - 1) }); },
} });
console.log(`LISTENING ${server.port}`);
