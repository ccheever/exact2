// A design server request's body and its JSON (scripts/tamber-server.mjs):
// read whole up to a limit, parsed to an object, and how deep it nests.

/** The body as text; null when over `limit` bytes (the connection is cut)
 * or when the client left. */
export const readBody = (req, limit) => new Promise(done => {
  if (req.destroyed) return done(null);
  if (Number(req.headers['content-length']) > limit) { req.destroy?.(); return done(null); }
  const chunks = []; let size = 0, settled = false;
  const finish = v => { if (!settled) { settled = true; done(v); } };
  req.on('data', chunk => {
    size += chunk.length;
    if (size > limit) { finish(null); req.destroy?.(); } else chunks.push(chunk);
  });
  req.on('end', () => finish(Buffer.concat(chunks).toString('utf8')));
  req.on('error', () => finish(null));
  req.on('close', () => finish(null));
});

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
