// A WLED device simulator for Lights' drives (LLP 1133.036 §10): the subset
// of WLED's JSON API Lights reads and writes (https://kno.wled.ge/interfaces/json-api/),
// one light per loopback port, with WLED's `Access-Control-Allow-Origin: *`.
//
//   bun apps/lights/fixture/server.ts [Name[=mac] …]
//
// prints one line of JSON, `{"<Name>": <port>, …}`, and serves until killed.
// Besides WLED's routes each light answers a control route the drives use as
// the wall switch and the record of what reached it:
//   POST /control/state   {"on":…, "bri":…} changes the light without Lights
//   GET  /control/log     the bodies POSTed to /json/state, in order
// It never advertises: drives give Lights the light through the agent's
// `prefer extension local-network.browseServices=…`, and a real DNS-SD drive
// uses its own type (§14).

type Light = { name: string; mac: string; on: boolean; bri: number; log: unknown[] };

const cors = { 'access-control-allow-origin': '*', 'access-control-allow-headers': 'content-type' };
const reply = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json', ...cors } });

function serve(light: Light) {
  return Bun.serve({
    hostname: '127.0.0.1',
    port: 0,
    async fetch(request) {
      const path = new URL(request.url).pathname;
      if (request.method === 'OPTIONS') return new Response(null, { status: 204, headers: { ...cors, 'access-control-allow-methods': 'GET, POST' } });
      const state = () => ({ on: light.on, bri: light.bri, seg: [{ id: 0, col: [[255, 160, 80]] }] });
      if (request.method === 'GET' && path === '/json/si') {
        return reply({ state: state(), info: { name: light.name, ver: '0.15.0', leds: { count: 30 }, mac: light.mac } });
      }
      if (request.method === 'GET' && path === '/json/state') return reply(state());
      if (request.method === 'GET' && path === '/control/log') return reply(light.log);
      if (request.method === 'POST' && (path === '/json/state' || path === '/control/state')) {
        const body = await request.json().catch(() => null) as { on?: unknown; bri?: unknown } | null;
        if (!body || typeof body !== 'object') return reply({ error: 9 }, 400);
        if (path === '/json/state') light.log.push(body);
        if (typeof body.on === 'boolean') light.on = body.on;
        if (typeof body.bri === 'number') light.bri = Math.max(0, Math.min(255, Math.round(body.bri)));
        return reply(path === '/json/state' ? { success: true } : state());
      }
      return reply({ error: 'not found' }, 404);
    },
  });
}

const named = process.argv.slice(2).length ? process.argv.slice(2) : ['Desk=a1b2c3d4e5f6'];
const ports: Record<string, number> = {};
named.forEach((arg, i) => {
  const [name, mac = `a1b2c3d4e5${(0xf6 + i).toString(16).slice(-2)}`] = arg.split('=');
  ports[name] = serve({ name, mac, on: true, bri: 128, log: [] }).port;
});
console.log(JSON.stringify(ports));
