// Whether the dev loop's page lets the design tool frame it (the second hands-on pass, finding 3): a tool on another
// port than the one the loop was started for (EXACT_TOOL_PORT) is refused by the page's frame-ancestors, and Chrome
// shows an empty canvas, saying why only in the console. `tamber tool --via` asks the page itself, says it once on
// the terminal, and answers the tool's page at /__tamber/tool/canvas-check (the tool's own, not tamber/1).

/** Whether a page at `page` whose Content-Security-Policy is `csp` (the header's value: policies joined by commas,
 * as fetch joins several headers) lets `ancestor` frame it (CSP3 frame-ancestors). Every policy that has the
 * directive must allow it; with none, framing is allowed. `origins`: the sources of the first policy that refuses,
 * as written (`'self'` as the page's origin). */
export function framesAllowed(csp, ancestor, page) {
  const a = new URL(ancestor), self = new URL(page).origin;
  const lists = String(csp ?? '').split(',').map(policy => policy.split(';').map(d => d.trim().split(/\s+/)).find(([name]) => name?.toLowerCase() === 'frame-ancestors')?.slice(1)).filter(Boolean);
  for (const sources of lists) {
    if (!sources.some(s => matches(s, a, self))) return { allowed: false, origins: sources.filter(s => s.toLowerCase() !== "'none'").map(s => (s.toLowerCase() === "'self'" ? self : s)) };
  }
  return { allowed: true, origins: [] };
}

const DEFAULT_PORT = { 'http:': '80', 'https:': '443' };
const network = a => a.protocol === 'http:' || a.protocol === 'https:';
/** A scheme as a source allows: the same, or https for http (CSP3 "scheme-part matches"). */
const schemeAllows = (scheme, a) => a.protocol === scheme || (scheme === 'http:' && a.protocol === 'https:');

function matches(source, a, self) {
  const s = source.toLowerCase();
  if (s === "'none'") return false;
  if (s === "'self'") return a.origin === self;
  if (s === '*') return network(a);
  if (/^[a-z][a-z0-9+.-]*:$/.test(s)) return schemeAllows(s, a);
  const m = /^(?:([a-z][a-z0-9+.-]*):\/\/)?(\*|\*\.[^:/[\]]+|\[[0-9a-f:.]+\]|[^:/[\]]+)(?::(\*|\d+))?(?:\/.*)?$/.exec(s);
  if (!m) return false;
  const [, scheme, host, port] = m;
  // No scheme: the page's own (an http page allows https too).
  if (!schemeAllows(scheme ? `${scheme}:` : new URL(self).protocol, a)) return false;
  const hostname = a.hostname.toLowerCase();
  if (host === '*') { if (!network(a)) return false; }
  else if (host.startsWith('*.')) { if (!hostname.endsWith(host.slice(1))) return false; }
  else if (hostname !== host) return false;
  if (port === '*') return true;
  return (a.port || DEFAULT_PORT[a.protocol]) === (port ?? DEFAULT_PORT[scheme ? `${scheme}:` : a.protocol]);
}

/** The loop at `via` asked whether its page lets `tool` frame it: `{via, allowed, origins}`, allowed when the page
 * can't be asked (a loop that is down says so elsewhere). The canvas frames the page at `localhost` (the tool's
 * hello `frame`), so `'self'` is that origin. */
export async function canvasCheck(via, tool) {
  try {
    const got = await fetch(`${via}/`, { headers: { accept: 'text/html' }, redirect: 'manual', signal: AbortSignal.timeout(5000) });
    await got.body?.cancel();
    return { via, ...framesAllowed(got.headers.get('content-security-policy'), tool, via.replace('127.0.0.1', 'localhost')) };
  } catch { return { via, allowed: true, origins: [] }; }
}

/** What the terminal is told: a refusal, once each time it starts (not on every check). */
export function canvasLine(check, tool, said) {
  const now = check.allowed ? '' : `tamber tool: the dev loop at ${check.via} lets only ${check.origins.join(' ') || 'no page'} frame its page, not this tool (${tool}), so the canvas stays empty: start the loop with EXACT_TOOL_PORT=${new URL(tool).port}`;
  return { line: now && now !== said ? now : '', said: now };
}

/** `/__tamber/tool/canvas-check` for the tool's own page: a local GET from its own origin, no store, the check alone
 * (no token, no path). Anything else is not found. */
export async function answerCanvasCheck(req, res, access, via, tool, tell) {
  const own = (req.headers.origin === undefined || req.headers.origin === tool) && (req.headers['sec-fetch-site'] ?? 'same-origin') === 'same-origin';
  if (req.method !== 'GET' || !access?.local || !own) { res.writeHead(404, { 'cache-control': 'no-store' }); res.end(); return; }
  const check = await canvasCheck(via, tool);
  tell(check);
  res.writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-store' });
  res.end(JSON.stringify({ via: check.via, allowed: check.allowed, origins: check.origins }));
}
