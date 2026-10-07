// Echo server: greets on open, logs and echoes every client frame.
const server = Bun.serve({
  port: 28791, hostname: '127.0.0.1',
  fetch(req, srv) { console.log(`server: ${req.method} ${new URL(req.url).pathname}`); return srv.upgrade(req) ? undefined : new Response('ws only', { status: 400 }); },
  websocket: {
    open(ws) { console.log('server: open'); ws.send('hello from server'); },
    message(ws, msg) { console.log(`server: received ${String(msg)}`); ws.send(`echo ${msg}`); },
    close(ws, code, reason) { console.log(`server: close ${code} ${reason}`); },
  },
});
console.log(`server: listening ${server.url}`);
