// Conformance's network: the outside servers the apps ask, answered locally
// and generated in code, so every target sees the same answers and a run is a
// function of the commit, never of someone else's server (host/web-js/conform.mjs).
//
// Everything reaches it through a proxy (`startNet().proxy`): a CONNECT is
// tunnelled to a local HTTPS server whose certificate an authority made for
// the run signs. Chrome trusts the server's key (`spki`); Bun and native code
// trust the authority (`env`'s `NODE_EXTRA_CA_CERTS` and `SSL_CERT_FILE`). An
// app's request nothing here answers is kept in `unanswered`, which fails the
// run, since an app catches a failed fetch and shows its fallback. Clients speak
// real HTTP to the server, so CORS, preflights, redirects and Range requests
// behave as they do against the real servers. Loopback (the page's own server,
// an app's local service) is not proxied.
//
// The answers are strict: a request the real API would not answer as the app
// expects (an unknown path or field, a missing parameter, a malformed body) gets
// no answer and lands in `unanswered`, so a broken request cannot pass as a
// fallback screen both targets agree on.
import { spawnSync } from 'node:child_process';
import { createHash, X509Certificate } from 'node:crypto';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer as createHttpServer } from 'node:http';
import { createServer as createHttpsServer } from 'node:https';
import { connect } from 'node:net';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { rootCertificates } from 'node:tls';
import { encodePng } from '../../../scripts/png.mjs';

/** The hosts this module answers, by the app that asks them. */
export const HOSTS = {
  'api.realworld.show': realworld,
  'api.open-meteo.com': openMeteo,
  'geocoding-api.open-meteo.com': geocoding,
  'randomuser.me': portraits,
};

/** Start the proxy and the HTTPS server behind it. `close()` stops both.
 *  Every HTTPS host is tunnelled to the local server, so a request is seen whole and judged by
 *  who sent it, not by its host. `unanswered` fails a run: an app's request (a page's, Bun's,
 *  native code's) nothing here answers, a CONNECT to a port other than 443, or a handshake a
 *  client other than a browser abandoned (one that does not trust the run's authority). The rest
 *  is reported only: `answered`, `tunnels`, `browser` (the browser's own requests, such as
 *  sign-in and updates, answered 404; see `browserOwn`), and `aborted` (a request its client gave
 *  up on, as a page navigating away does, or a browser's abandoned handshake: the browsers here
 *  accept any certificate, so theirs is a preconnect or a shutdown, not a refusal). */
export async function startNet(directory) {
  // Its trust files go in `directory`, or in a scratch one `close()` removes
  const scratch = directory ? null : mkdtempSync(resolve(tmpdir(), 'exact-conform-net-'));
  directory ??= scratch;
  const { key, chain, cert, spki } = certificate(Object.keys(HOSTS));
  // Native code (rustls's native roots, OpenSSL) reads `SSL_CERT_FILE` in place of the system's roots:
  // the authority joins the usual public roots there, so a toolchain's own downloads still verify
  const authority = resolve(directory, 'net-authority.pem'), trust = resolve(directory, 'net-trust.pem');
  writeFileSync(authority, cert);
  writeFileSync(trust, [cert.toString().trim(), ...rootCertificates].join('\n') + '\n');
  const unanswered = [], answered = [], tunnels = [], aborted = [], browser = [];
  const sockets = new Set(), own = socket => { sockets.add(socket); socket.on('close', () => sockets.delete(socket)); };
  const https = createHttpsServer({ key, cert: chain }, (req, res) => {
    const host = (req.headers.host ?? '').split(':')[0], request = `${req.method} https://${host}${req.url}`;
    const chunks = [];
    res.on('close', () => { if (!res.writableFinished) aborted.push(request); });
    req.on('data', c => chunks.push(c));
    req.on('end', () => {
      const reply = answer(host, req.method, req.url, req.headers, Buffer.concat(chunks));
      (reply ? answered : browserOwn(req) ? browser : unanswered).push(request);
      const { status, headers, body } = reply ?? { status: 404, headers: { 'content-type': 'text/plain' }, body: 'conformance net: no generated answer\n' };
      res.writeHead(status, { ...cors(req), ...headers });
      res.end(body);
    });
  });
  // A tunnel the client spoke on whose connection closes without a completed handshake is a client
  // that refused the certificate, unless the client is a browser (one never spoken on is a
  // preconnect left unused). Matched by
  // port, the tunnel's local end, as Bun emits no `tlsClientError`.
  const tunnelOf = new Map(), secured = new Set();
  https.on('connection', socket => {
    own(socket);
    const port = socket.remotePort;
    socket.on('close', () => {
      const tunnel = tunnelOf.get(port);
      if (tunnel?.spoke && !secured.has(port)) {
        (tunnel.browser ? aborted : unanswered).push(`CONNECT ${tunnel.url}: TLS handshake failed (is the run's certificate trusted?)`);
      }
      tunnelOf.delete(port); secured.delete(port);
    });
  });
  https.on('secureConnection', socket => secured.add(socket.remotePort));
  await listen(https);
  const proxy = createHttpServer((req, res) => {
    // A plain-HTTP request through the proxy: none of the answered hosts is HTTP
    (browserOwn(req) ? browser : unanswered).push(`${req.method} ${req.url}`);
    res.writeHead(502, { 'content-type': 'text/plain' });
    res.end('conformance net: outside HTTP refused\n');
  });
  proxy.on('connection', own);
  proxy.on('connect', (req, socket) => {
    const [host, port] = req.url.split(':');
    if (port !== '443') {
      unanswered.push(`CONNECT ${req.url}`);
      socket.end('HTTP/1.1 403 Forbidden\r\n\r\n');
      return;
    }
    tunnels.push(req.url);
    const upstream = connect(https.address().port, '127.0.0.1', () => {
      const tunnel = { url: req.url, spoke: false, browser: BROWSER.test(req.headers['user-agent'] ?? '') };
      tunnelOf.set(upstream.localPort, tunnel);
      socket.once('data', () => { tunnel.spoke = true; });
      socket.write('HTTP/1.1 200 Connection Established\r\n\r\n');
      upstream.pipe(socket); socket.pipe(upstream);
    });
    own(upstream);
    upstream.on('error', () => socket.destroy());
    socket.on('error', () => upstream.destroy());
  });
  await listen(proxy);
  const at = `127.0.0.1:${proxy.address().port}`, url = `http://${at}`;
  return {
    proxy: at, spki, cert, unanswered, answered, tunnels, aborted, browser,
    /** What a child process needs to go through this proxy and trust its server (scripts/agent.mjs,
     *  Bun's fetch, Rust's HTTP clients): every spelling of the proxy variables, so an inherited one
     *  (`ALL_PROXY`, `no_proxy=*`) cannot route around it. */
    env: {
      EXACT_NET_PROXY: at, EXACT_NET_SPKI: spki, NODE_EXTRA_CA_CERTS: authority, SSL_CERT_FILE: trust,
      ...Object.fromEntries(['HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY'].flatMap(name => [[name, url], [name.toLowerCase(), url]])),
      NO_PROXY: LOOPBACK, no_proxy: LOOPBACK,
      // Cargo fetches the toolchain's crates, not an app's data: an empty proxy of its own sends it direct
      CARGO_HTTP_PROXY: '',
    },
    // Open tunnels and keep-alive connections are closed, or the servers' close would wait on them
    close: () => {
      const closed = Promise.all([new Promise(ok => https.close(ok)), new Promise(ok => proxy.close(ok))]);
      for (const socket of sockets) socket.destroy();
      if (scratch) rmSync(scratch, { recursive: true, force: true });
      return closed;
    },
  };
}

const LOOPBACK = '127.0.0.1,localhost,::1';

/** Whether a request is the browser's own (its sign-in, update and push services) rather than an
 *  app's: a page's requests, fetches and loads alike, carry `Sec-Fetch-Site` naming the page's
 *  relation to the target, which the browser's services leave out (or send as `none`, as for a
 *  typed address), and a browser's user agent; Bun and native code send neither. */
function browserOwn(req) {
  const site = req.headers['sec-fetch-site'];
  return (site === undefined || site === 'none') && BROWSER.test(req.headers['user-agent'] ?? '');
}

const BROWSER = /^Mozilla\/5\.0 /;

/** The reply for one request, or null when nothing here answers it. */
export function answer(host, method, url, headers = {}, body = Buffer.alloc(0)) {
  const handler = HOSTS[host];
  if (!handler) return null;
  if (method === 'OPTIONS') return { status: 204, headers: {}, body: '' };
  return handler(method, new URL(url, `https://${host}`), headers, body);
}

const listen = server => new Promise((ok, fail) => { server.once('error', fail); server.listen(0, '127.0.0.1', ok); });

// Cross-origin reads are allowed, as the real APIs allow them.
function cors(req) {
  return {
    'access-control-allow-origin': req.headers.origin ?? '*',
    'access-control-allow-methods': 'GET, POST, PUT, DELETE, OPTIONS',
    'access-control-allow-headers': req.headers['access-control-request-headers'] ?? 'authorization, content-type',
    'access-control-max-age': '600',
    vary: 'origin',
  };
}

/** A certificate authority made for this run (`cert`, what clients trust) and a server certificate
 *  it signs for `hosts` (`chain` and `key`, what the server presents; `spki`, its key's digest for
 *  Chrome's `--ignore-certificate-errors-spki-list`). Rustls needs the two apart: webpki takes
 *  neither a self-signed server certificate as its own anchor nor a CA certificate as a server's. */
function certificate(hosts) {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-conform-net-'));
  try {
    const run = args => {
      const r = spawnSync('openssl', args, { cwd: dir, encoding: 'utf8' });
      if (r.status !== 0) throw new Error(`conformance net: openssl ${args[0]} failed: ${r.stderr || r.error?.message}`);
      return r;
    };
    const ec = ['-newkey', 'ec', '-pkeyopt', 'ec_paramgen_curve:prime256v1', '-nodes'];
    run(['req', '-x509', ...ec, '-days', '2', '-subj', '/CN=exact conformance authority',
      '-addext', 'basicConstraints=critical,CA:TRUE', '-addext', 'keyUsage=critical,keyCertSign',
      '-keyout', 'authority-key.pem', '-out', 'authority.pem']);
    run(['req', ...ec, '-subj', '/CN=exact conformance', '-keyout', 'key.pem', '-out', 'request.pem']);
    writeFileSync(resolve(dir, 'server.ext'), [`subjectAltName=${hosts.map(h => `DNS:${h}`).join(',')}`,
      'basicConstraints=critical,CA:FALSE', 'keyUsage=critical,digitalSignature', 'extendedKeyUsage=serverAuth'].join('\n') + '\n');
    run(['x509', '-req', '-in', 'request.pem', '-CA', 'authority.pem', '-CAkey', 'authority-key.pem', '-CAcreateserial',
      '-days', '2', '-extfile', 'server.ext', '-out', 'server.pem']);
    const read = name => readFileSync(resolve(dir, name)), server = read('server.pem');
    const spki = new X509Certificate(server).publicKey.export({ type: 'spki', format: 'der' });
    return { key: read('key.pem'), chain: Buffer.concat([server, read('authority.pem')]), cert: read('authority.pem'),
      spki: createHash('sha256').update(spki).digest('base64') };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

const json = (status, value) => ({ status, headers: { 'content-type': 'application/json; charset=utf-8' }, body: JSON.stringify(value) });
const png = image => ({ status: 200, headers: { 'content-type': 'image/png', 'cache-control': 'max-age=3600' }, body: encodePng(image) });

/** A small deterministic number from a string, for generated values. */
function seed(text) {
  return createHash('sha256').update(text).digest().readUInt32BE(0);
}

/** A `size`×`size` portrait: two bands in colours drawn from `name`, so different pictures differ. */
function picture(name, size = 24) {
  const n = seed(name), data = Buffer.alloc(size * size * 4);
  const top = [n & 255, (n >> 8) & 255, (n >> 16) & 255], bottom = [(n >> 24) & 255, (n >> 4) & 255, (n >> 12) & 255];
  for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
    const c = y < size / 2 ? top : bottom, i = (y * size + x) * 4;
    data[i] = c[0]; data[i + 1] = c[1]; data[i + 2] = c[2]; data[i + 3] = 255;
  }
  return { width: size, height: size, data };
}

// randomuser.me: `/api/portraits/{women|men}/N.jpg` (apps/spark). A PNG body: browsers decode by content.
function portraits(method, url) {
  if (method !== 'GET' || !/^\/api\/portraits\/(women|men)\/\d+\.jpg$/.test(url.pathname)) return null;
  return png(picture(url.pathname));
}

// RealWorld's API (apps/realworld/app.ts; the spec at docs.realworld.show): twelve articles from
// three templates, the slug and tag `realworld.steps` names among them, comments, profiles, a
// refused sign-in (no account exists) and a signed-out `/user`.
const TEMPLATES = [
  { title: 'How to learn JavaScript efficiently', tags: ['javascript', 'learning'],
    body: '# Learning\n\nRead a little **every day**, then build something.\n\n- Small programs\n- Real projects\n\n```js\nconsole.log("hi");\n```\n' },
  { title: 'Welcome to RealWorld', tags: ['welcome'],
    body: 'A place to share *articles*.\n\n> Write what you know.\n\n![a picture](https://api.realworld.show/images/welcome.png)\n' },
  { title: 'Notes on layout and long titles that wrap across more than one line', tags: ['javascript', 'css'],
    body: 'Paragraph one.\n\nParagraph two, with [a link](https://example.com) and `code`.\n' },
];
const AUTHORS = ['ada', 'grace', 'linus'];
const slugOf = title => title.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
const profileOf = name => ({ username: name, bio: `${name} writes here.`, image: `https://api.realworld.show/images/${name}.png`, following: false });
const ARTICLES = Array.from({ length: 12 }, (_, i) => {
  const t = TEMPLATES[i % 3], round = Math.floor(i / 3);
  const title = round ? `${t.title} (${round + 1})` : t.title;
  const day = String(28 - i).padStart(2, '0');
  return { slug: slugOf(title), title, description: `About ${title.toLowerCase()}.`, body: t.body, tagList: t.tags,
    createdAt: `2025-12-${day}T10:00:00.000Z`, updatedAt: `2025-12-${day}T10:00:00.000Z`, favorited: false, favoritesCount: i % 4,
    author: profileOf(AUTHORS[i % 3]) };
});
const REFUSED = json(422, { errors: { 'email or password': ['is invalid'] } });

function realworld(method, url, headers, body) {
  const path = url.pathname, q = url.searchParams;
  if (method === 'GET' && /^\/images\/[a-z-]+\.png$/.test(path)) return png(picture(path));
  if (!path.startsWith('/api/')) return null;
  const rest = path.slice(4);
  if (method === 'GET' && rest === '/tags') return json(200, { tags: ['javascript', 'welcome', 'learning', 'css'] });
  if (method === 'GET' && rest === '/user') return json(401, { errors: { token: ['is missing'] } });
  if (method === 'GET' && rest === '/articles/feed') return json(401, { errors: { token: ['is missing'] } });
  if (method === 'GET' && rest === '/articles') {
    const tag = q.get('tag'), author = q.get('author'), favorited = q.get('favorited');
    const all = ARTICLES.filter(a => (!tag || a.tagList.includes(tag)) && (!author || a.author.username === author) && !favorited); // no one has favorited anything
    const offset = Number(q.get('offset') ?? 0), limit = Number(q.get('limit') ?? 10);
    return json(200, { articles: all.slice(offset, offset + limit).map(({ body, ...a }) => a), articlesCount: all.length });
  }
  const one = /^\/articles\/([^/]+)(\/comments)?$/.exec(rest);
  if (method === 'GET' && one) {
    const a = ARTICLES.find(x => x.slug === decodeURIComponent(one[1]));
    if (!a) return json(404, { errors: { article: ['not found'] } });
    if (!one[2]) return json(200, { article: a });
    return json(200, { comments: [0, 1].map(k => ({ id: seed(a.slug + k) % 10000, body: `Comment ${k + 1} on ${a.title}.`,
      createdAt: '2025-12-30T09:00:00.000Z', updatedAt: '2025-12-30T09:00:00.000Z', author: profileOf(AUTHORS[(k + 1) % 3]) })) });
  }
  const profile = /^\/profiles\/([^/]+)$/.exec(rest);
  if (method === 'GET' && profile) {
    const name = decodeURIComponent(profile[1]);
    return AUTHORS.includes(name) ? json(200, { profile: profileOf(name) }) : json(404, { errors: { profile: ['not found'] } });
  }
  // Signing in or up is refused (no account exists), but only for the body the API takes
  if (method === 'POST' && (rest === '/users/login' || rest === '/users')) {
    const user = parsed(body)?.user, strings = keys => keys.every(k => typeof user?.[k] === 'string');
    return strings(rest === '/users' ? ['username', 'email', 'password'] : ['email', 'password']) ? REFUSED : null;
  }
  // The API's other changes need a signed-in user, which no one is here
  const change = MUTATIONS.find(([m, pattern]) => m === method && pattern.test(rest));
  if (change && !headers.authorization) return json(401, { errors: { token: ['is missing'] } });
  return null;
}
const MUTATIONS = [
  ['PUT', /^\/user$/], ['POST', /^\/articles$/], ['PUT', /^\/articles\/[^/]+$/], ['DELETE', /^\/articles\/[^/]+$/],
  ['POST', /^\/articles\/[^/]+\/favorite$/], ['DELETE', /^\/articles\/[^/]+\/favorite$/],
  ['POST', /^\/articles\/[^/]+\/comments$/], ['DELETE', /^\/articles\/[^/]+\/comments\/[^/]+$/],
  ['POST', /^\/profiles\/[^/]+\/follow$/], ['DELETE', /^\/profiles\/[^/]+\/follow$/],
];
const parsed = body => { try { return JSON.parse(body.toString('utf8')); } catch { return null; } };

// Open-Meteo's forecast (apps/weatherlight, apps/expose): at a fixed time, the fields asked for
// (`current`, `hourly`, `daily`) and no others, over `forecast_days` days, in the asked
// temperature unit; the values are drawn from the place, so two places differ and one is always
// the same. A missing or out-of-range place, an unknown field or option gets no answer.
const NOW = '2026-01-01T12:00';
const HOURLY = {
  temperature_2m: (i, n) => 5 + n % 20 + 6 * Math.sin((i % 24 - 9) / 24 * 2 * Math.PI),
  apparent_temperature: (i, n) => 4 + n % 20 + 6 * Math.sin((i % 24 - 9) / 24 * 2 * Math.PI),
  relative_humidity_2m: (i, n) => 50 + (i * 7 + n) % 40, is_day: i => i % 24 >= 7 && i % 24 < 18 ? 1 : 0,
  precipitation: (i, n) => (i + n) % 11 === 0 ? 0.4 : 0, weather_code: (i, n) => [0, 1, 2, 3, 61, 3][(i + n) % 6],
  cloud_cover: (i, n) => (i * 13 + n) % 100, wind_speed_10m: (i, n) => 4 + (i + n) % 25,
  precipitation_probability: (i, n) => (i * 17 + n) % 100,
};
const DAILY = {
  weather_code: (d, n) => [0, 2, 3, 61, 1, 3, 0][(d + n) % 7], temperature_2m_max: (d, n) => 11 + n % 20 + (d + n) % 3,
  temperature_2m_min: (d, n) => -1 + n % 20 + (d + n) % 2, sunrise: (d, n, day) => `${day}T07:21`, sunset: (d, n, day) => `${day}T16:58`,
  uv_index_max: (d, n) => 1 + (d + n) % 7, precipitation_probability_max: (d, n) => (d * 23 + n) % 100,
};
const TEMPERATURES = new Set(['temperature_2m', 'apparent_temperature', 'temperature_2m_max', 'temperature_2m_min']);
const OPTIONS = new Set(['latitude', 'longitude', 'timezone', 'forecast_days', 'temperature_unit', 'current', 'hourly', 'daily']);
function openMeteo(method, url) {
  if (method !== 'GET' || url.pathname !== '/v1/forecast') return null;
  const q = url.searchParams, lat = Number(q.get('latitude') ?? NaN), lon = Number(q.get('longitude') ?? NaN);
  const days = Number(q.get('forecast_days') ?? 7), unit = q.get('temperature_unit') ?? 'celsius', zone = q.get('timezone') ?? 'GMT';
  const list = name => q.get(name)?.split(',') ?? [];
  const current = list('current'), hourly = list('hourly'), daily = list('daily');
  if (![...q.keys()].every(k => OPTIONS.has(k)) || !(Math.abs(lat) <= 90) || !(Math.abs(lon) <= 180)
    || !Number.isInteger(days) || days < 1 || days > 16 || !['celsius', 'fahrenheit'].includes(unit) || !['auto', 'GMT', 'UTC'].includes(zone)
    || ![...current, ...hourly].every(f => Object.hasOwn(HOURLY, f)) || !daily.every(f => Object.hasOwn(DAILY, f))) return null;
  const n = seed(`${lat},${lon}`), round = v => Math.round(v * 10) / 10;
  const value = (f, v) => typeof v === 'number' ? round(TEMPERATURES.has(f) && unit === 'fahrenheit' ? v * 9 / 5 + 32 : v) : v;
  const hours = Array.from({ length: days * 24 }, (_, i) => `2026-01-${String(1 + Math.floor(i / 24)).padStart(2, '0')}T${String(i % 24).padStart(2, '0')}:00`);
  const dayList = Array.from({ length: days }, (_, d) => `2026-01-${String(1 + d).padStart(2, '0')}`);
  const reply = { latitude: lat, longitude: lon, timezone: 'GMT', timezone_abbreviation: 'GMT', utc_offset_seconds: 0 };
  if (current.length) reply.current = { time: NOW, interval: 900, ...Object.fromEntries(current.map(f => [f, value(f, HOURLY[f](12, n))])) };
  if (hourly.length) reply.hourly = { time: hours, ...Object.fromEntries(hourly.map(f => [f, hours.map((_, i) => value(f, HOURLY[f](i, n)))])) };
  if (daily.length) reply.daily = { time: dayList, ...Object.fromEntries(daily.map(f => [f, dayList.map((day, d) => value(f, DAILY[f](d, n, day)))])) };
  return json(200, reply);
}

// Open-Meteo's geocoding (apps/weatherlight's search): one generated place per query.
function geocoding(method, url) {
  if (method !== 'GET' || url.pathname !== '/v1/search') return null;
  const name = url.searchParams.get('name') ?? '', n = seed(name);
  // The API finds nothing for fewer than two letters
  if (name.length < 2) return json(200, { generationtime_ms: 0.1 });
  return json(200, { results: [{ id: n % 100000, name: name.slice(0, 1).toUpperCase() + name.slice(1), latitude: (n % 1600) / 10 - 80,
    longitude: (n % 3400) / 10 - 170, country: 'Exampleland', admin1: 'Region' }] });
}
