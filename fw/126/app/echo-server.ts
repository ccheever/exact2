for (const port of [8791, 8792]) Bun.serve({
  port, hostname: '127.0.0.1',
  fetch(req, srv) { return srv.upgrade(req) ? undefined : new Response('ws only', { status: 400 }); },
  websocket: {
    open(ws) { console.log(`server ${port}: open`); ws.send('hello from server'); },
    message(ws, msg) { console.log(`server ${port}: received ${String(msg)}`); ws.send(`echo ${msg}`); },
    close(ws, code, reason) { console.log(`server ${port}: close ${code} ${reason}`); },
  },
});
console.log('server: listening 8791, 8792');
