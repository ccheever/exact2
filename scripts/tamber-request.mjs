// A design server request's body and its JSON (scripts/tamber-server.mjs):
// read whole up to a limit, parsed to an object, and how deep it nests.

/** A request whose body is refused: answered first (a 413), its connection
 * closing after. Until the answer has gone out the rest goes unread; then it
 * is read and dropped, and the connection closed when it ends, so the client's
 * last bytes meet no reset that could cost it the answer (closing with its
 * bytes unread resets the connection); 5 s at most, whatever the client does.
 * The design server's and the canvas gateway's. */
export const refuseBody = (req, res, answer = () => {}) => {
  req.pause();
  res.setHeader('connection', 'close');
  const fallback = setTimeout(() => req.destroy?.(), 5000);
  fallback.unref?.();
  // (Its socket's close, not the response's or the request's: Bun's node:http fires those before the answer is out.)
  (req.socket ?? res.socket)?.once?.('close', () => clearTimeout(fallback));
  res.once('finish', () => {
    req.on('data', () => {});
    req.once('end', () => { clearTimeout(fallback); req.destroy?.(); });
    req.resume();
  });
  answer();
};

/** The body as text (`raw`: its bytes); null when over `limit` bytes,
 * undefined when the client left (`== null` for either). Over it, with `res`, refused (`refuseBody`: the caller's
 * answer goes out first); without `res`, the connection is cut. */
export const readBody = (req, limit, res = null, raw = false) => new Promise(done => {
  if (req.destroyed) return done(undefined);
  const over = () => (res ? refuseBody(req, res) : req.destroy?.());
  if (Number(req.headers['content-length']) > limit) { over(); return done(null); }
  const chunks = []; let size = 0, settled = false;
  // (its connection's close, below, let go of once settled: a keep-alive connection's requests add none for good)
  const gone = () => finish(undefined);
  const finish = v => { if (!settled) { settled = true; req.socket?.off?.('close', gone); done(v); } };
  req.on('data', chunk => {
    if (settled) return;
    size += chunk.length;
    if (size > limit) { chunks.length = 0; finish(null); over(); } else chunks.push(chunk);
  });
  req.on('end', () => finish(raw ? Buffer.concat(chunks) : Buffer.concat(chunks).toString('utf8')));
  req.on('error', () => finish(undefined));
  // The client left: settled here, or on its connection's close below. One that stays and only trickles is
  // cut by its door's connection deadline (10 s, `timeoutMs` in tamber-remote.mjs's guard), which runs while
  // a body arrives; a loopback request has the dev loop's.
  req.on('close', () => finish(undefined));
  // (and when its connection closes: Bun loses `req`'s close on a socket a TLS door hands to the HTTP
  // server, so a client that left mid-body would hold this, and its chunks, for good)
  req.socket?.once?.('close', gone);
});

/** A remote door's body, read while the server may still be starting (`ready`):
 * 'over' (refuse it now: refuseBody's 5 s would otherwise run out behind a
 * slow start), 'gone' (the client left: answer nothing, hold nothing), or
 * 'go' (the body, or the server ready first). */
export const earlyAnswer = async (early, ready) => {
  const first = await Promise.race([early, ready.then(() => '')]);
  return first === null ? 'over' : first === undefined ? 'gone' : 'go';
};

/** Whether `v` nests deeper than `max` arrays or objects (iterative, so a
 * deep value can't exhaust the stack here; stage 10 A2). */
export const tooDeep = (v, max = 64) => {
  const stack = [[v, 0]];
  while (stack.length) {
    const [x, d] = stack.pop();
    if (x === null || typeof x !== 'object') continue;
    if (d >= max) return true;
    for (const y of Object.values(x)) stack.push([y, d + 1]);
  }
  return false;
};

/** `text` as a JSON object, or null for anything else. */
export const parseRequest = text => {
  try { const v = JSON.parse(text); return v && typeof v === 'object' && !Array.isArray(v) ? v : null; } catch { return null; }
};

/** Whether a request may carry a paired credential: from this machine, or
 * through one of stage 10 B's doors (its TLS listener or canvas port, or a
 * tunnel the developer trusts). Plain HTTP from off this machine (a --lan
 * bind's peer) never may (LLP 1122 D12): the wire and the canvas both ask
 * this, so the rule lives here once. */
export const viaDoor = access => access?.local === true || access?.remote === true || access?.tunnel === true;

/** A request sent on to a design server or dev loop on this machine (`tamber tool --via`'s proxy, the canvas gateway,
 * their probes, the tool tests' proxy), on a connection of its own. A pooled one can be one the server has just
 * closed as idle (Bun's node:http closes a kept-alive socket after 5 s and a second's grace): sent on it, the request
 * meets a reset (ECONNRESET, "the socket connection was closed unexpectedly"). That is the likely cause of the
 * design-tool test split's one ECONNRESET under load (QUEUE 2026-10-22), where the time before this side sees the
 * close is long enough to land in; tamber-upstream.test.mjs makes the reuse certain. A loopback connect costs next
 * to nothing. (`keepalive: false` turns pooling off in Bun's fetch, which runs these; Node's and a browser's read it
 * otherwise.) */
export const upstream = (url, init = {}) => {
  // (A `connection: keep-alive` passed on from the client would put the socket back in the pool all the same.)
  const headers = new Headers(init.headers);
  headers.delete('connection'); headers.delete('keep-alive');
  return fetch(url, { ...init, headers, keepalive: false });
};

/** A design server's `{app, epoch}` at `origin`, as `tamber tool` probes it: `{answer}` (`{app: null}` from a server
 * with no design server), `{wait}` while there is no answer yet (`'listening'`: refused, a dev loop still on its first
 * build, or reset, one restarting under the request; `'starting'`: it listens, but its token or hello timed out, its design server starting), or `{failed}`. A
 * token probe that times out is tried again, `tries` in all (a loaded dev loop answers late). */
export async function probeTool(origin, tries = 1) {
  const why = e => (e?.name === 'TimeoutError' ? { wait: 'starting' } : ['ConnectionRefused', 'ECONNREFUSED', 'ECONNRESET'].includes(e?.code ?? e?.cause?.code) ? { wait: 'listening' } : { failed: true });
  let got;
  try { for (let i = 1; !got; i++) try { got = await upstream(`${origin}/__tamber/token`, { redirect: 'error', signal: AbortSignal.timeout(2000) }); } catch (e) { if (e?.name !== 'TimeoutError' || i >= tries) throw e; } } catch (e) { return why(e); }
  if (!got.ok) return { answer: { app: null } };
  try {
    const { token } = await got.json();
    // (Held while its design server starts: a dev loop's first compiler build.)
    const said = await upstream(`${origin}/__tamber`, { method: 'POST', redirect: 'error', headers: { 'x-exact-tamber-token': token }, body: '{"method":"hello"}', signal: AbortSignal.timeout(tries > 1 ? 120_000 : 10_000) });
    // (An HTTP error status is an answer, whatever its body does after: no design server here, as for the token.)
    if (!said.ok) return { answer: { app: null } };
    const hello = await said.json();
    return { answer: { app: hello.ok?.app ?? null, epoch: hello.ok?.epoch ?? null } };
  } catch (e) { return why(e); } // (refused or reset now: it stopped between the two, a loop restarting)
}

/** `probe()` again every `everyMs` while it says `{wait}` (`tamber tool --via` while the dev loop starts, the design
 * tool's third pass's #9), each new reason said once (`say`), for at most `capMs`: its first other answer, or
 * `{waited: ms}` past the cap. Ctrl-c meanwhile exits 130 (SIGTERM 143), before anything is listening. */
export async function waitForAnswer(probe, { say, capMs, everyMs = 2000, now = Date.now, exit = code => process.exit(code) }) {
  const start = now(), codes = { SIGINT: 130, SIGTERM: 143 }, handlers = Object.entries(codes).map(([signal, code]) => [signal, () => exit(code)]);
  let said = null;
  try {
    for (const [signal, handler] of handlers) process.once(signal, handler);
    // (The cap bounds a probe in flight too: a hello held 120 s ends the wait at the cap, not after it.)
    let timer;
    const capped = new Promise(done => { timer = setTimeout(() => done({ waited: now() - start }), Math.max(0, capMs)); });
    try {
      for (;;) {
        const got = await Promise.race([probe(), capped]);
        if (!got.wait) return got;
        if (got.wait !== said) say((said = got.wait));
        if (now() - start >= capMs) return { waited: now() - start };
        const next = await Promise.race([new Promise(done => setTimeout(() => done(null), everyMs)), capped]);
        if (next) return next;
      }
    } finally { clearTimeout(timer); }
  } finally {
    for (const [signal, handler] of handlers) process.off(signal, handler);
  }
}
