// The relying party's routes at the fixture's public origin (LLP 1133.011 §13 question 4): an EAS Hosting API
// route module, which `origin.mjs` bundles as CommonJS, as `expo export` bundles one. EAS Hosting calls the export
// named for the request's method with a `Request` and expects a `Response`. `ORIGIN_RPS` is the RP IDs and origins
// `origin.mjs` defines at bundle time; the HMAC key is the deployment's `PASSKEY_RECORD_KEY`, an EAS environment
// variable that is never in the repository.
import { relyingParty } from './relying-party.mjs';

/* global ORIGIN_RPS */
const cors = {
  'access-control-allow-origin': '*',
  'access-control-allow-headers': 'content-type',
  'access-control-allow-methods': 'POST',
};
const json = (status, body) => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json', 'cache-control': 'no-store', ...cors } });
let rp = null;

export async function POST(request) {
  const key = globalThis.process?.env?.PASSKEY_RECORD_KEY;
  if (!key) return json(500, { error: 'server_error', error_description: 'the deployment has no PASSKEY_RECORD_KEY' });
  rp ??= relyingParty({}, { rps: ORIGIN_RPS, key });
  const route = rp.route(new URL(request.url).pathname.replace(/\/$/, ''));
  if (!route) return json(404, { error: 'not_found' });
  let body;
  try { body = await request.json(); } catch { return json(400, { error: 'invalid_request', error_description: 'the body is not JSON' }); }
  try { return json(200, await route(body ?? {})); } catch (error) { return json(500, { error: 'server_error', error_description: String(error?.message ?? error) }); }
}

export const OPTIONS = () => new Response(null, { status: 204, headers: cors });
