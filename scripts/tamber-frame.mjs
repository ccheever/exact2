// Whether the dev loop's page lets the design tool frame it (the second hands-on pass, finding 3): a tool on another
// port than the one the loop was started for (EXACT_TOOL_PORT) is refused by the page's frame-ancestors, and Chrome
// shows an empty canvas, saying why only in the console. `tamber tool --via` asks the page itself, says it once on
// the terminal, and answers the tool's page at /__tamber/tool/canvas-check (the tool's own, not tamber/1).
import { upstream } from './tamber-request.mjs';

/** Whether a page at `page` whose Content-Security-Policy is `csp` (the header's value: policies joined by commas,
 * as fetch joins several headers) lets `ancestor` frame it (CSP3 frame-ancestors). Every policy that has the
 * directive must allow it; with none, framing is allowed. `origins`: the sources of the first policy that refuses,
 * as written (`'self'` as the page's origin). */
export function framesAllowed(csp, ancestor, page) {
  const a = new URL(ancestor), self = new URL(page).origin;
  const lists = String(csp ?? '').split(',').map(policy => policy.split(';').map(d => d.trim().split(/\s+/)).find(([name]) => name?.toLowerCase() === 'frame-ancestors')?.slice(1)).filter(Boolean);
  for (const sources of lists) {
    // (A port fixes it only when it would satisfy every policy: one 'none' among them and no port does.)
    if (!sources.some(s => matches(s, a, self))) return { allowed: false, origins: sources.map(s => named(s, self)).filter(Boolean), portFixes: lists.every(l => l.some(s => matches(s, a, self) || portFixes(s, self))) };
  }
  return { allowed: true, origins: [], portFixes: false };
}

const DEFAULT_PORT = { 'http:': '80', 'https:': '443' };
const network = a => a.protocol === 'http:' || a.protocol === 'https:';
/** A scheme as a source allows: the same, or https for http (CSP3 "scheme-part matches"). */
const schemeAllows = (scheme, a) => a.protocol === scheme || (scheme === 'http:' && a.protocol === 'https:');

// A host source (CSP3's grammar: no bracketed IPv6 literal, which a browser ignores as invalid): its scheme, host,
// port and path as written.
const HOST_SOURCE = /^(?:([a-z][a-z0-9+.-]*):\/\/)?(\*|(?:\*\.)?[a-z0-9-]+(?:\.[a-z0-9-]+)*)(?::(\*|\d+))?(\/[^?#]*)?$/i;

function matches(source, a, self) {
  const s = source.toLowerCase();
  if (s === "'none'") return false;
  // 'self': the page's origin, or its host upgraded from http to https (both on their default ports).
  if (s === "'self'") {
    const p = new URL(self);
    return a.origin === self || (p.protocol === 'http:' && a.protocol === 'https:' && a.hostname === p.hostname && !p.port && !a.port);
  }
  if (s === '*') return network(a);
  if (/^[a-z][a-z0-9+.-]*:$/.test(s)) return schemeAllows(s, a);
  const m = HOST_SOURCE.exec(source);
  if (!m) return false;
  const [, scheme, host, port, path] = m;
  // No scheme: the page's own (an http page allows https too).
  if (!schemeAllows(scheme ? `${scheme.toLowerCase()}:` : new URL(self).protocol, a)) return false;
  const hostname = a.hostname.toLowerCase(), h = host.toLowerCase();
  if (h === '*') { if (!network(a)) return false; }
  else if (h.startsWith('*.')) { if (!hostname.endsWith(h.slice(1))) return false; }
  else if (hostname !== h) return false;
  // No port: only the URL's own default (CSP3: a null port matches a null port); `*` any.
  if (port !== '*' && (port === undefined ? a.port !== '' : Number(a.port || DEFAULT_PORT[a.protocol]) !== Number(port))) return false;
  // An ancestor is its origin, whose path is `/`: a longer path never matches.
  return path === undefined || path === '/';
}

/** A source as the refusal names it: an origin (a host source without its path; 'self' as the page's), a scheme,
 * `*`; nothing for 'none' or what isn't a source. */
function named(source, self) {
  const s = source.toLowerCase();
  if (s === "'none'") return '';
  if (s === "'self'") return self;
  if (s === '*' || /^[a-z][a-z0-9+.-]*:$/.test(s)) return s;
  const m = HOST_SOURCE.exec(source);
  return m ? `${m[1] ? m[1].toLowerCase() : new URL(self).protocol.slice(0, -1)}://${m[2].toLowerCase()}${m[3] ? `:${m[3] === '*' ? '*' : Number(m[3])}` : ''}` : '';
}

/** Whether a tool on another loopback port would match: a source naming http://127.0.0.1 (written, or by the page's
 * scheme) at a port, with no path past `/`. The advice to set EXACT_TOOL_PORT rests on it, not on the names shown. */
function portFixes(source, self) {
  const m = HOST_SOURCE.exec(source);
  if (!m || !m[3] || m[3] === '*' || (m[4] !== undefined && m[4] !== '/')) return false;
  return (m[1] ? m[1].toLowerCase() : new URL(self).protocol.slice(0, -1)) === 'http' && m[2].toLowerCase() === '127.0.0.1';
}

/** The loop at `via` asked whether its page lets `tool` frame it: `{via, allowed, origins}`, allowed when the page
 * can't be asked (a loop that is down says so elsewhere). The canvas frames the page at `localhost` (the tool's
 * hello `frame`), so `'self'` is that origin. */
export async function canvasCheck(via, tool) {
  try {
    const got = await upstream(`${via}/`, { headers: { accept: 'text/html' }, redirect: 'manual', signal: AbortSignal.timeout(5000) });
    await got.body?.cancel();
    return { via, ...framesAllowed(got.headers.get('content-security-policy'), tool, `http://localhost:${new URL(via).port}`) };
  } catch { return { via, allowed: true, origins: [], portFixes: false }; }
}

/** A refusal in words: the loop's own port for the tool (EXACT_TOOL_PORT) when it names a loopback tool, else the
 * page's own policy, which no port changes. The tool's page words it the same (apps/design-tool/canvas-check.ts). */
export function refusalWords(check, tool) {
  const named = check.origins.join(' ');
  const head = `the dev loop at ${check.via} ${named ? `lets only ${named} frame its page` : 'lets no page frame it'}, not this tool (${tool}), so the canvas stays empty`;
  return check.portFixes ? `${head}: start the loop with EXACT_TOOL_PORT=${new URL(tool).port}` : `${head}: its page's own policy refuses framing, which no port changes`;
}

/** What the terminal is told: a refusal, once each time it starts (not on every check). */
export function canvasLine(check, tool, said) {
  const now = check.allowed ? '' : `tamber tool: ${refusalWords(check, tool)}`;
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
  res.end(JSON.stringify({ via: check.via, allowed: check.allowed, origins: check.origins, portFixes: check.portFixes }));
}
