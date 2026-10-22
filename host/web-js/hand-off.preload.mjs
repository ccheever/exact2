// Test-only (`bun --preload`: dev-page-streams.test.mjs, host/web/tests/dev-stream-hand-off.test.mjs): the old [::1]
// door, a raw socket handed to the loop's HTTP server by emit('connection'), here on EXACT_TEST_HAND_OFF's second port
// for the server last listening on its first (a probe that listened there before it, closed, is passed by). Under Bun a
// request that comes so never fires req 'close': only a page stream's res 'close' backstop ends it.
import { Server } from 'node:http';
import { createServer } from 'node:net';

const [forPort, onPort] = (process.env.EXACT_TEST_HAND_OFF ?? '').split(':').map(Number);
const listen = Server.prototype.listen;
let to = null, side = null;
Server.prototype.listen = function (...args) {
  if (args[0] === forPort && args[1] !== '::1') {
    to = this;
    side ??= createServer(socket => to.emit('connection', socket)).listen(onPort, '127.0.0.1');
  }
  return listen.apply(this, args);
};
