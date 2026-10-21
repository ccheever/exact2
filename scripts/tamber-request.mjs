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
  const finish = v => { if (!settled) { settled = true; done(v); } };
  req.on('data', chunk => {
    if (settled) return;
    size += chunk.length;
    if (size > limit) { chunks.length = 0; finish(null); over(); } else chunks.push(chunk);
  });
  req.on('end', () => finish(raw ? Buffer.concat(chunks) : Buffer.concat(chunks).toString('utf8')));
  req.on('error', () => finish(undefined));
  req.on('close', () => finish(undefined));
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
