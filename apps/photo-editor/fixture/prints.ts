// The photo editor's print lab, a fixture (LLP 1133.034 §10, D6): it keeps
// checkouts made before the sheet, with their prices fixed, quotes shipping
// for the sheet's redacted address, and records each payment's attempt. It
// never charges a card: a payment's token is checked for its method's shape,
// its length recorded, and nothing is sent to a processor.
//
// usage: bun apps/photo-editor/fixture/prints.ts [--agent] [--port 4334]
//   [--decline-first] [--reject-postal] [--slow <ms>] [--lose-answer]
//   [--merchant-cert <pem path>]
//
// - `--agent` takes the substitute's synthetic payments (`exactSubstitute`)
//   and answers merchant validation with a synthetic session; without it
//   both are refused, as the recipe tells a production server to refuse them.
// - `--decline-first` declines each checkout's first attempt; `--reject-postal`
//   answers 422 with a postal-code error until an attempt's postal code ends
//   in 0; `--slow <ms>` holds each attempt indeterminate that long before it
//   settles as paid; `--lose-answer` pays, then fails the response.
// - `POST /fixture/switches {…}` changes the switches while it runs;
//   `GET /fixture/log` is what it saw; `POST /fixture/reset` forgets it all.
//
// Merchant validation (Apple Pay on the web, D8): only Apple's listed gateway
// hosts, over HTTPS, are accepted, and no redirect is followed. With a
// merchant identity certificate it posts to that URL; without one it answers 503.

type Amount = { currency: string; value: string };
type Checkout = {
  id: string;
  size: string;
  price: string;
  state: 'open' | 'paid' | 'indeterminate' | 'needs-action';
  attempts: { id: string; key: string; method: string; tokenLength: number; outcome: string }[];
  address: Record<string, unknown> | null;
  option: string | null;
  email: string | null;
  orderId: string | null;
  shipping: string;
};

const argv = process.argv.slice(2);
const flag = (name: string) => argv.includes(name);
const value = (name: string) => (argv.includes(name) ? argv[argv.indexOf(name) + 1] : undefined);
const PORT = Number(value('--port') ?? 4334);
const switches = {
  agent: flag('--agent'),
  declineFirst: flag('--decline-first'),
  rejectPostal: flag('--reject-postal'),
  slow: Number(value('--slow') ?? 0),
  loseAnswer: flag('--lose-answer'),
};
const CERT = value('--merchant-cert');

const PRICES: Record<string, string> = { '4x6': '4.00', '5x7': '8.00', '8x10': '15.00' };
const SHIPS = ['US', 'CA', 'GB'];
const STANDARD: Record<string, string> = { US: '5.00', CA: '9.00', GB: '12.00' };
const EXPRESS = '15.00';
/** Apple's gateway hosts for merchant validation (Apple Pay on the web's "Setting up your server"). */
const APPLE_HOSTS = new Set([
  'apple-pay-gateway.apple.com', 'cn-apple-pay-gateway.apple.com', 'apple-pay-gateway-nc-pod1.apple.com',
  'apple-pay-gateway-nc-pod2.apple.com', 'apple-pay-gateway-nc-pod3.apple.com', 'apple-pay-gateway-nc-pod4.apple.com',
  'apple-pay-gateway-nc-pod5.apple.com', 'apple-pay-gateway-pr-pod1.apple.com', 'apple-pay-gateway-pr-pod2.apple.com',
  'apple-pay-gateway-pr-pod3.apple.com', 'apple-pay-gateway-pr-pod4.apple.com', 'apple-pay-gateway-pr-pod5.apple.com',
  'cn-apple-pay-gateway-sh-pod1.apple.com', 'cn-apple-pay-gateway-sh-pod2.apple.com', 'cn-apple-pay-gateway-sh-pod3.apple.com',
  'cn-apple-pay-gateway-tj-pod1.apple.com', 'cn-apple-pay-gateway-tj-pod2.apple.com', 'cn-apple-pay-gateway-tj-pod3.apple.com',
  'apple-pay-gateway-cert.apple.com', 'cn-apple-pay-gateway-cert.apple.com',
]);

let checkouts = new Map<string, Checkout>();
let log = { quotes: [] as unknown[], payments: [] as unknown[], sessions: [] as unknown[], refused: [] as string[] };
let next = 0;

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json', 'access-control-allow-origin': '*', 'access-control-allow-headers': '*' } });
const money = (n: number): Amount => ({ currency: 'USD', value: n.toFixed(2) });
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** The checkout as the app's screen reads it. */
const shown = (c: Checkout) => ({ id: c.id, size: c.size, price: c.price, shipping: c.shipping, state: c.state, orderId: c.orderId ?? '', attempts: c.attempts.length, charged: c.attempts.filter((a) => a.outcome === 'paid').length });

/** The shipping options and total for a redacted address (D2): a country, a region, a postal code. */
function quote(c: Checkout, address: Record<string, unknown> | null, option: string | null) {
  const country = String(address?.country ?? '');
  if (!SHIPS.includes(country)) {
    return { error: `Prints don’t ship to ${country || 'that address'} yet.`, shippingAddressErrors: { country: `We ship to ${SHIPS.join(', ')}.` }, shippingOptions: [] };
  }
  const options = [{ id: 'standard', label: 'Standard', amount: money(Number(STANDARD[country])) }];
  if (country === 'US') options.push({ id: 'express', label: 'Express', amount: money(Number(EXPRESS)) });
  const chosen = options.find((o) => o.id === option) ?? options[0];
  const shipping = Number(chosen.amount.value);
  c.shipping = chosen.amount.value;
  return {
    total: { label: 'Photo Editor Prints', amount: money(Number(c.price) + shipping) },
    displayItems: [{ label: `${c.size} print`, amount: money(Number(c.price)) }, { label: chosen.label, amount: chosen.amount }],
    shippingOptions: options.map((o) => ({ ...o, selected: o.id === chosen.id })),
  };
}

/** The method's payment, checked for its shape: its token's length, or why it is refused. */
function token(method: string, payment: any): { length: number; key: string } | string {
  if (method === 'https://apple.com/apple-pay') {
    const data = payment?.token?.paymentData, id = payment?.token?.transactionIdentifier;
    if (!data || typeof id !== 'string') return 'an ApplePayPayment has token.paymentData and token.transactionIdentifier';
    if (data.exactSubstitute && !switches.agent) return 'a substitute payment, refused outside --agent';
    return { length: JSON.stringify(data).length, key: id };
  }
  if (method === 'https://google.com/pay') {
    const t = payment?.paymentMethodData?.tokenizationData?.token;
    if (typeof t !== 'string') return 'a PaymentData has paymentMethodData.tokenizationData.token';
    if (t.includes('exactSubstitute') && !switches.agent) return 'a substitute payment, refused outside --agent';
    return { length: t.length, key: `g-${Bun.hash(t).toString(16)}` };
  }
  return `no method ${method}`;
}

async function pay(c: Checkout, body: any): Promise<Response> {
  const t = token(body.method, body.payment);
  if (typeof t === 'string') {
    log.refused.push(t);
    return json({ error: t }, 400);
  }
  // A repeated submission of one payment, or a paid checkout, charges nothing more.
  const same = c.attempts.find((a) => a.key === t.key);
  if (same || c.state === 'paid' || c.state === 'indeterminate') {
    log.payments.push({ checkout: c.id, method: body.method, tokenLength: t.length, outcome: `already ${c.state}` });
    return json({ ...shown(c) });
  }
  const postal = String(body.ship?.postalCode ?? '');
  if (switches.rejectPostal && !postal.endsWith('0')) {
    log.payments.push({ checkout: c.id, method: body.method, tokenLength: t.length, outcome: 'retry' });
    return json({ errors: { shippingAddress: { postalCode: 'Check the ZIP code.' } } }, 422);
  }
  const attempt = { id: `a${c.attempts.length + 1}`, key: t.key, method: body.method, tokenLength: t.length, outcome: 'pending' };
  c.attempts.push(attempt);
  if (switches.declineFirst && c.attempts.length === 1) {
    attempt.outcome = 'declined';
    log.payments.push({ checkout: c.id, method: body.method, tokenLength: t.length, outcome: 'declined' });
    return json({ ...shown(c), state: 'declined' });
  }
  c.address = body.ship ?? null;
  c.option = body.option ?? null;
  c.email = body.email ?? null;
  const settle = () => {
    attempt.outcome = 'paid';
    c.state = 'paid';
    c.orderId = `PR-${c.id.slice(1)}`;
  };
  if (switches.slow > 0) {
    c.state = 'indeterminate';
    attempt.outcome = 'indeterminate';
    log.payments.push({ checkout: c.id, method: body.method, tokenLength: t.length, outcome: 'indeterminate' });
    await sleep(switches.slow);
    settle();
    return json(shown(c));
  }
  settle();
  log.payments.push({ checkout: c.id, method: body.method, tokenLength: t.length, outcome: 'paid' });
  if (switches.loseAnswer) return new Response('the answer was lost', { status: 502 });
  return json(shown(c));
}

async function session(body: any): Promise<Response> {
  let url: URL;
  try {
    url = new URL(String(body?.url ?? ''));
  } catch {
    return json({ error: 'no validation URL' }, 400);
  }
  if (url.protocol !== 'https:' || !APPLE_HOSTS.has(url.hostname)) {
    log.sessions.push({ host: url.hostname, outcome: 'refused' });
    return json({ error: `${url.hostname} is no Apple Pay gateway` }, 403);
  }
  if (switches.agent) {
    log.sessions.push({ host: url.hostname, outcome: 'synthetic' });
    return json({ merchantSession: { exactSubstitute: true, epochTimestamp: Date.now(), expiresAt: Date.now() + 300000, merchantSessionIdentifier: `SSH${++next}`, displayName: 'Photo Editor Prints' } });
  }
  if (!CERT) return json({ error: 'no merchant identity certificate' }, 503);
  const cert = Bun.file(CERT);
  const pem = await cert.text();
  const res = await fetch(url, {
    method: 'POST', redirect: 'error',
    body: JSON.stringify({ merchantIdentifier: 'merchant.com.exact.photoeditor', displayName: 'Photo Editor Prints', initiative: 'web', initiativeContext: body?.domain ?? 'localhost' }),
    tls: { cert: pem, key: pem },
  } as RequestInit);
  log.sessions.push({ host: url.hostname, outcome: String(res.status) });
  return json({ merchantSession: await res.json() }, res.ok ? 200 : 502);
}

const server = Bun.serve({
  port: PORT,
  hostname: '127.0.0.1',
  async fetch(req) {
    const url = new URL(req.url), path = url.pathname;
    if (req.method === 'OPTIONS') return json({});
    const body = req.method === 'POST' ? await req.json().catch(() => ({})) : {};
    if (path === '/fixture/log') return json({ ...log, checkouts: [...checkouts.values()], switches });
    if (path === '/fixture/reset') {
      checkouts = new Map();
      log = { quotes: [], payments: [], sessions: [], refused: [] };
      return json({ ok: true });
    }
    if (path === '/fixture/switches') {
      Object.assign(switches, body);
      return json(switches);
    }
    if (path === '/checkouts' && req.method === 'GET') return json({ orders: [...checkouts.values()].reverse().map(shown) });
    if (path === '/checkouts') {
      const size = String(body.size ?? '');
      if (!PRICES[size]) return json({ error: `no size ${size}` }, 400);
      const id = `c${++next}`;
      checkouts.set(id, { id, size, price: PRICES[size], state: 'open', attempts: [], address: null, option: null, email: null, orderId: null, shipping: '0.00' });
      return json({ id, size, price: PRICES[size] });
    }
    if (path === '/apple-pay/session') return session(body);
    const m = path.match(/^\/checkouts\/([^/]+)\/(quote|pay)$/);
    const c = m && checkouts.get(m[1]);
    if (!m || !c) return json({ error: 'no such checkout' }, 404);
    if (m[2] === 'quote') {
      log.quotes.push({ checkout: c.id, address: body.address, option: body.option });
      return json(quote(c, body.address ?? null, body.option ?? null));
    }
    return pay(c, body);
  },
});
console.log(`prints: http://127.0.0.1:${server.port}${switches.agent ? ' (--agent)' : ''}`);
