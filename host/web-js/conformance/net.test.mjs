import { test, expect } from 'bun:test';
import { request } from 'node:http';
import { answer, startNet } from './net.mjs';

const body = reply => JSON.parse(reply.body);

test('the generated RealWorld API carries what realworld.steps names', () => {
  const list = body(answer('api.realworld.show', 'GET', '/api/articles?limit=10&offset=0'));
  expect(list.articlesCount).toBeGreaterThan(10); // a second page
  expect(list.articles).toHaveLength(10);
  expect(list.articles.map(a => a.slug)).toContain('how-to-learn-javascript-efficiently');
  expect(body(answer('api.realworld.show', 'GET', '/api/tags')).tags).toContain('javascript');
  expect(body(answer('api.realworld.show', 'GET', '/api/articles/how-to-learn-javascript-efficiently/comments')).comments).toHaveLength(2);
  const login = Buffer.from(JSON.stringify({ user: { email: 'a@example.com', password: 'x' } }));
  expect(answer('api.realworld.show', 'POST', '/api/users/login', {}, login).status).toBe(422);
});

test('a request the real API would not answer gets no answer', () => {
  expect(answer('api.realworld.show', 'POST', '/api/users/login', {}, Buffer.from('{"email":"a@example.com"}'))).toBeNull();
  expect(answer('api.realworld.show', 'POST', '/api/typo')).toBeNull();
  expect(answer('api.realworld.show', 'POST', '/api/articles/x/favorite').status).toBe(401);
  expect(answer('api.open-meteo.com', 'GET', '/v1/forecast?current=temperature_2m')).toBeNull();
  expect(answer('api.open-meteo.com', 'GET', '/v1/forecast?latitude=999&longitude=999&current=temperature_2m')).toBeNull();
  expect(answer('api.open-meteo.com', 'GET', '/v1/forecast?latitude=1&longitude=1&current=no_such_field')).toBeNull();
});

test('the generated forecast has the current hour, a day of hours ahead, and a week of days', () => {
  const fields = 'current=temperature_2m,is_day&hourly=temperature_2m&daily=temperature_2m_max,sunrise';
  const forecast = body(answer('api.open-meteo.com', 'GET', `/v1/forecast?latitude=51.5&longitude=-0.1&timezone=auto&${fields}`));
  const now = forecast.hourly.time.indexOf(forecast.current.time);
  expect(now).toBeGreaterThanOrEqual(0);
  expect(forecast.hourly.time.length - now).toBeGreaterThan(24);
  expect(forecast.daily.time).toHaveLength(7);
  expect(Object.keys(forecast.current)).toEqual(['time', 'interval', 'temperature_2m', 'is_day']);
  const other = body(answer('api.open-meteo.com', 'GET', `/v1/forecast?latitude=40.7&longitude=-74&${fields}`));
  expect(other.hourly.temperature_2m).not.toEqual(forecast.hourly.temperature_2m);
});

test('the forecast honours its length and temperature unit', () => {
  const ask = more => body(answer('api.open-meteo.com', 'GET', `/v1/forecast?latitude=37.4&longitude=-122.1&current=temperature_2m&daily=temperature_2m_max${more}`));
  const celsius = ask(''), fahrenheit = ask('&temperature_unit=fahrenheit&forecast_days=2');
  expect(fahrenheit.daily.time).toHaveLength(2);
  expect(fahrenheit.current.temperature_2m).toBeCloseTo(celsius.current.temperature_2m * 9 / 5 + 32, 0);
});

test('different portraits differ, and nothing answers an unknown host or path', () => {
  expect(answer('randomuser.me', 'GET', '/api/portraits/women/1.jpg').body)
    .not.toEqual(answer('randomuser.me', 'GET', '/api/portraits/men/2.jpg').body);
  expect(answer('example.com', 'GET', '/')).toBeNull();
  expect(answer('api.realworld.show', 'GET', '/elsewhere')).toBeNull();
});

test('the proxy tunnels HTTPS and refuses and records other ports', async () => {
  const net = await startNet();
  try {
    const connect = target => new Promise((ok, fail) => {
      const [host, port] = net.proxy.split(':');
      request({ host, port, method: 'CONNECT', path: target }).on('connect', (res, socket) => { socket.destroy(); ok(res.statusCode); }).on('error', fail).end();
    });
    expect(await connect('api.realworld.show:443')).toBe(200);
    expect(await connect('www.google.com:443')).toBe(200);
    expect(await connect('api.realworld.show:8443')).toBe(403);
    expect(net.tunnels).toEqual(['api.realworld.show:443', 'www.google.com:443']);
    expect(net.unanswered).toEqual(['CONNECT api.realworld.show:8443']);
    expect(net.env.ALL_PROXY).toBe(`http://${net.proxy}`);
    expect(net.env.no_proxy).not.toContain('*');
  } finally {
    await net.close();
  }
});

test("an app request to an unanswered host fails; the browser's own requests are kept apart", async () => {
  const net = await startNet();
  try {
    const ask = (url, headers) => fetch(url, { proxy: `http://${net.proxy}`, headers, tls: { rejectUnauthorized: false } }).then(r => r.status); // as Chrome's SPKI pin does
    const chrome = 'Mozilla/5.0 (Macintosh) Chrome/140.0';
    expect(await ask('https://www.google.com/x', { 'user-agent': chrome, 'sec-fetch-site': 'cross-site' })).toBe(404);
    expect(await ask('https://accounts.google.com/ListAccounts', { 'user-agent': chrome })).toBe(404);
    expect(await ask('https://api.example.org/data', { 'user-agent': 'Bun/1.4' })).toBe(404);
    expect(net.unanswered).toEqual(['GET https://www.google.com/x', 'GET https://api.example.org/data']);
    expect(net.browser).toEqual(['GET https://accounts.google.com/ListAccounts']);
    // A client checking names (Bun's fetch, with the run's certificate) refuses any host the certificate does not name
    await fetch('https://elsewhere.example/', { proxy: `http://${net.proxy}`, tls: { ca: net.cert } }).catch(() => {});
    await new Promise(ok => setTimeout(ok, 100));
    expect(net.unanswered.at(-1)).toBe("CONNECT elsewhere.example:443: TLS handshake failed (is the run's certificate trusted?)");
  } finally {
    await net.close();
  }
});

test("a client that refuses the certificate fails the run; a browser's abandoned handshake does not", async () => {
  for (const [userAgent, ledger] of [['Bun/1.4', 'unanswered'], ['Mozilla/5.0 (Macintosh) Chrome/140.0', 'aborted']]) {
    const net = await startNet();
    try {
      const [host, port] = net.proxy.split(':');
      const tunnel = await new Promise((ok, fail) => request({ host, port, method: 'CONNECT', path: 'api.realworld.show:443', headers: { 'user-agent': userAgent } })
        .on('connect', (res, socket) => ok(socket)).on('error', fail).end());
      const { connect } = await import('node:tls');
      await new Promise(done => connect({ socket: tunnel, servername: 'api.realworld.show' }).on('error', done).on('secureConnect', done));
      await new Promise(ok => setTimeout(ok, 100));
      expect(net[ledger]).toEqual(["CONNECT api.realworld.show:443: TLS handshake failed (is the run's certificate trusted?)"]);
      expect(net[ledger === 'unanswered' ? 'aborted' : 'unanswered']).toEqual([]);
    } finally {
      await net.close();
    }
  }
});
