// A stand-in for the site's API, so the dev loop, `serve`, `test` and `verify` need no network. `bun server.ts`
// listens at API as Bun reads .env.local and .env; `test` and `verify` run it themselves, given the value the build
// inlines. Its figures are in memory, so a restart undoes a PATCH. With API_BASE_URL naming the real API (.env.local
// or the environment), the site needs none of it.
import { API } from './app.ts';

const PLANETS: Record<string, { summary: string; moons: number; day: string; year: string }> = {
  '1': { summary: 'The smallest planet, and the closest to the Sun.', moons: 0, day: '176 Earth days', year: '88 Earth days' },
  '2': { summary: 'The hottest planet, under clouds of sulphuric acid.', moons: 0, day: '117 Earth days', year: '225 Earth days' },
  '3': { summary: 'The only planet known to hold life.', moons: 1, day: '24 hours', year: '365 days' },
  '4': { summary: 'A cold desert with the tallest volcano in the Solar System.', moons: 2, day: '24.7 hours', year: '687 Earth days' },
  '5': { summary: 'The largest planet, its Great Red Spot a storm wider than Earth.', moons: 95, day: '9.9 hours', year: '11.9 Earth years' },
  '6': { summary: 'A gas giant ringed with ice.', moons: 274, day: '10.7 hours', year: '29.4 Earth years' },
  '7': { summary: 'An ice giant that rolls around the Sun on its side.', moons: 28, day: '17.2 hours', year: '84 Earth years' },
  '8': { summary: 'The windiest planet, and the farthest from the Sun.', moons: 16, day: '16.1 hours', year: '165 Earth years' },
};
// The site's page is another origin: every answer, and the preflight, allows it.
const CORS = { 'access-control-allow-origin': '*', 'access-control-allow-headers': 'content-type', 'access-control-allow-methods': 'GET, PATCH' };
const send = (body: unknown, status = 200) => Response.json(body, { status, headers: CORS });

const { hostname, port } = new URL(API);
Bun.serve({ hostname, port: Number(port), routes: {
  '/planets/:id': {
    OPTIONS: () => new Response(null, { status: 204, headers: CORS }),
    GET: (req) => { const planet = PLANETS[req.params.id]; return planet ? send(planet) : send({ error: 'No such planet.' }, 404); },
    // A new count, as the real API's figures change after a build (app.test.contract's `request PATCH`).
    PATCH: async (req) => {
      const planet = PLANETS[req.params.id], { moons } = await req.json().catch(() => ({}));
      if (!planet) return send({ error: 'No such planet.' }, 404);
      if (!Number.isSafeInteger(moons) || moons < 0) return send({ error: 'Say how many moons, as a whole number.' }, 400);
      planet.moons = moons;
      return send(planet);
    },
  },
} });
console.log(`server.ts: ${API}`);
