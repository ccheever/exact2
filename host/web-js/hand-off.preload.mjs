// Test-only (dev-page-streams.test.mjs, `bun --preload`): the old [::1] door, a raw socket handed to the loop's HTTP
// server by emit('connection'), here on EXACT_TEST_HAND_OFF's second port for the server listening on its first.
// Under Bun a request that comes so never fires req 'close': only a page stream's res 'close' backstop ends it.
import { Server } from 'node:http';
import { createServer } from 'node:net';

const [forPort, onPort] = (process.env.EXACT_TEST_HAND_OFF ?? '').split(':').map(Number);
const listen = Server.prototype.listen;
Server.prototype.listen = function (...args) {
  if (args[0] === forPort && args[1] !== '::1') createServer(socket => this.emit('connection', socket)).listen(onPort, '127.0.0.1');
  return listen.apply(this, args);
};
