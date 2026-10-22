// scripts/tamber-frame.mjs: frame-ancestors read as a browser reads it, and the tool's canvas check.
import { expect, test } from 'bun:test';
import { createServer } from 'node:http';
import { answerCanvasCheck, canvasCheck, canvasLine, framesAllowed } from './tamber-frame.mjs';

const TOOL = 'http://127.0.0.1:17977', PAGE = 'http://localhost:17978';
const allows = (csp, tool = TOOL, page = PAGE) => framesAllowed(csp, tool, page).allowed;

test('frame-ancestors: the tool by its origin, another port, none, self, wildcards, schemes, several policies', () => {
  // The loop's own header: the tool named, or the default port's tool only.
  expect(allows(`frame-ancestors 'self' ${TOOL}`)).toBe(true);
  expect(framesAllowed("frame-ancestors 'self' http://127.0.0.1:7740", TOOL, PAGE)).toEqual({ allowed: false, origins: [PAGE, 'http://127.0.0.1:7740'] });
  // None at all: allowed, nothing to say. Another directive only: the same.
  expect(allows(null)).toBe(true);
  expect(allows("default-src 'self'")).toBe(true);
  // 'none'; 'self' is the page's origin (a tool on it would be allowed).
  expect(framesAllowed("frame-ancestors 'none'", TOOL, PAGE)).toEqual({ allowed: false, origins: [] });
  expect(allows("frame-ancestors 'self'", 'http://localhost:17978')).toBe(true);
  expect(allows("frame-ancestors 'self'")).toBe(false);
  // A port wildcard, a host wildcard, `*`, a scheme; a host source with no scheme takes the page's (http allows https).
  expect(allows('frame-ancestors http://127.0.0.1:*')).toBe(true);
  expect(allows('frame-ancestors http://*.example.com')).toBe(false);
  expect(allows('frame-ancestors *.example.com', 'https://tool.example.com')).toBe(true);
  expect(allows('frame-ancestors *')).toBe(true);
  expect(allows('frame-ancestors http:')).toBe(true);
  expect(allows('frame-ancestors https:')).toBe(false);
  expect(allows('frame-ancestors 127.0.0.1:17977')).toBe(true);
  expect(allows('frame-ancestors 127.0.0.1')).toBe(false);
  expect(allows('frame-ancestors https://127.0.0.1:17977')).toBe(false);
  // Directive names and keywords in any case.
  expect(allows(`FRAME-ANCESTORS 'SELF' ${TOOL.toUpperCase()}`)).toBe(true);
  // Several policies (headers joined by commas): every one with the directive must allow; the most restrictive wins.
  expect(allows(`frame-ancestors ${TOOL}, frame-ancestors 'none'`)).toBe(false);
  expect(allows(`frame-ancestors ${TOOL}, default-src 'self'`)).toBe(true);
  expect(allows(`script-src 'self'; frame-ancestors http://127.0.0.1:*, frame-ancestors ${TOOL}`)).toBe(true);
});

test('the canvas check: asked of the page, said once each time it starts refusing, answered only to the tool itself', async () => {
  let csp = "frame-ancestors 'self' http://127.0.0.1:7740";
  const loop = createServer((req, res) => { res.writeHead(200, { 'content-type': 'text/html', 'content-security-policy': csp }); res.end('<p>page</p>'); });
  await new Promise(ok => loop.listen(0, '127.0.0.1', ok));
  const via = `http://127.0.0.1:${loop.address().port}`;
  try {
    const refused = await canvasCheck(via, TOOL);
    expect(refused).toEqual({ via, allowed: false, origins: [via.replace('127.0.0.1', 'localhost'), 'http://127.0.0.1:7740'] });
    // The terminal: once, not on each check; again only after it allowed in between.
    let said = '', lines = [];
    for (const c of [refused, refused]) ({ said } = ((r) => (r.line && lines.push(r.line), r))(canvasLine(c, TOOL, said)));
    expect(lines).toHaveLength(1);
    expect(lines[0]).toContain('EXACT_TOOL_PORT=17977');
    csp = `frame-ancestors 'self' ${TOOL}`;
    const allowed = await canvasCheck(via, TOOL);
    expect(allowed.allowed).toBe(true);
    ({ said } = canvasLine(allowed, TOOL, said));
    expect(said).toBe('');
    // A loop that doesn't answer: nothing to say.
    expect((await canvasCheck('http://127.0.0.1:9', TOOL)).allowed).toBe(true);
    // The tool's endpoint: its own page (no Origin, or its own; same-origin) and local; anything else not found.
    const answer = async (headers, access = { local: true }, method = 'GET') => {
      const out = { status: 0, headers: {}, body: '' };
      await answerCanvasCheck({ method, headers }, { writeHead(s, h) { out.status = s; out.headers = h; }, end(b) { out.body = b ?? ''; } }, access, via, TOOL, () => {});
      return out;
    };
    const own = await answer({ 'sec-fetch-site': 'same-origin' });
    expect([own.status, own.headers['cache-control'], JSON.parse(own.body)]).toEqual([200, 'no-store', { via, allowed: true, origins: [] }]);
    expect((await answer({})).status).toBe(200);
    expect((await answer({ origin: 'http://localhost:17978' })).status).toBe(404);
    expect((await answer({ 'sec-fetch-site': 'cross-site' })).status).toBe(404);
    expect((await answer({}, { local: false })).status).toBe(404);
    expect((await answer({}, { local: true }, 'POST')).status).toBe(404);
  } finally { loop.close(); }
});
