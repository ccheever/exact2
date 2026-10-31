import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.starter';
// The API's origin, a setting (app.json's "env"): the environment's value, else .env.local's, else .env's (server.ts,
// the stand-in). It is inlined into the module on every host, so it is public: never a secret.
export const API = import.meta.env.API_BASE_URL;
export const grants = `net.fetch ${API}`;

// The site's own list, fixed when it is built: its page renders at build (render=build).
const PLANETS: Result<'loadPlanets'> = [
  { id: '1', name: 'Mercury', kind: 'Rocky', au: 0.39 },
  { id: '2', name: 'Venus', kind: 'Rocky', au: 0.72 },
  { id: '3', name: 'Earth', kind: 'Rocky', au: 1 },
  { id: '4', name: 'Mars', kind: 'Rocky', au: 1.52 },
  { id: '5', name: 'Jupiter', kind: 'Gas giant', au: 5.2 },
  { id: '6', name: 'Saturn', kind: 'Gas giant', au: 9.58 },
  { id: '7', name: 'Uranus', kind: 'Ice giant', au: 19.19 },
  { id: '8', name: 'Neptune', kind: 'Ice giant', au: 30.07 },
];

type Details = Result<'loadDetails'>;
function isDetails(value: unknown): value is Omit<Details, 'id'> {
  if (value === null || typeof value !== 'object') return false;
  const v = value as Record<string, unknown>;
  return typeof v.summary === 'string' && Number.isSafeInteger(v.moons) && typeof v.day === 'string' && typeof v.year === 'string';
}

const sources: Sources = {
  loadPlanets: async () => PLANETS,
  // A planet's figures, the API's, asked when its page renders. A failed fetch or a bad answer fails the
  // resource, which the page says; the list page asks for none.
  loadDetails: async ([id]): Promise<Details> => {
    if (!id) return { id: '', summary: '', moons: 0, day: '', year: '' };
    const response = await fetch(`${API}/planets/${encodeURIComponent(id)}`);
    if (!response.ok) throw new Error(`The API answered ${response.status}.`);
    const body: unknown = await response.json();
    if (!isDetails(body)) throw new Error('The API answered with something other than a planet.');
    return { id, summary: body.summary, moons: body.moons, day: body.day, year: body.year };
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
