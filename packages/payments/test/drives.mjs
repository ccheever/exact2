// LLP 1133.034 §10's acceptance drives under the agent, with the photo
// editor's Order a Print against the print lab (apps/photo-editor/fixture/
// prints.ts, `--agent`). The sheet is a `payment` ticket: `type` answers it
// as the person's changes in the sheet, `tap … authorize` as Face ID, and the
// app's handlers run against the lab over real I/O. Drive 8's origin rule is
// the runner's test (contract/cli/tests/it/payments.rs).
//
// usage: bun packages/payments/test/drives.mjs [web|ios|macos|linux] [--only 1,2,…]
//   Builds nothing on Apple or Linux: build the photo editor first
//   (host/apple/build.mjs photo-editor-apple [--ios --sim <udid>]; cargo
//   build --profile host-dev -p photo-editor-linux), which the driver checks
//   is current. Starts the lab on 4334 when none answers there, and stops
//   what it started.
import { spawn } from 'node:child_process';
import { resolve } from 'node:path';

const root = resolve(import.meta.dir, '../../..');
const argv = process.argv.slice(2), host = argv.find((a) => ['web', 'ios', 'macos', 'linux'].includes(a)) ?? 'web';
const only = argv.includes('--only') ? argv[argv.indexOf('--only') + 1].split(',').map(Number) : null;
const { open } = await import(resolve(root, 'scripts/agent.mjs'));
const LAB = 'http://127.0.0.1:4334';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let failed = 0;
const check = (drive, what, ok, detail = '') => {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${host} drive ${drive}: ${what}${ok ? '' : `\n     ${detail}`}`);
  if (!ok) failed++;
};

const up = () => fetch(`${LAB}/fixture/log`).then((r) => r.ok, () => false);
let server = null;
if (!(await up())) {
  server = spawn(process.execPath, [resolve(root, 'apps/photo-editor/fixture/prints.ts'), '--agent'], { stdio: 'ignore' });
  for (let i = 0; i < 50 && !(await up()); i++) await sleep(100);
}
const lab = () => fetch(`${LAB}/fixture/log`).then((r) => r.json());
const switches = (s) => fetch(`${LAB}/fixture/switches`, { method: 'POST', body: JSON.stringify(s) });
const OFF = { declineFirst: false, rejectPostal: false, slow: 0, loseAnswer: false };

const ids = async (s) => new Set((await s.tree()).nodes.map((n) => n.props?.testId).filter(Boolean));
const text = async (s, id) => (await s.find(id, false).catch(() => null))?.props?.text ?? null;
const sheets = async (s) => ((await s.state()).pending ?? []).filter((p) => p.device?.capability === 'payment');
// The lab answers over real I/O, which `clock settle` does not wait for on every host: poll a while.
const until = async (s, read, ms = 8000) => {
  for (const end = Date.now() + ms; ; await sleep(50)) {
    await s.clock('settle');
    const value = await read();
    if (value || Date.now() > end) return value;
  }
};
/** The sheet once it takes an answer (merchant validation done on the web, no handler running). */
const sheet = (s, want = () => true) => until(s, async () => (await sheets(s)).find((t) => !t.device.args.waiting && t.device.args.phase !== 'validating' && want(t.device.args)));
const gone = (s) => until(s, async () => !(await sheets(s)).length);
const US = { country: 'US', region: 'CA', city: 'Palo Alto', postalCode: '94301' };

/** A tap on `id` once its sheet is in place. On the web a modal presented right after one was dismissed slides in
 * on the agent's clock, but `clock settle` does not wait for the slide (QUEUE), so a miss moves the clock on. */
async function settled(s, id) {
  for (let tries = 0; ; tries++) {
    try {
      return await s.tap(id);
    } catch (e) {
      if (!/outside the viewport/.test(e.message) || tries === 4) throw e;
      await s.clock('+500');
    }
  }
}

/** The print sheet for `size`, its checkout made at the lab. */
async function printing(s, size = '8x10') {
  await s.tap('order-print');
  await until(s, async () => (await ids(s)).has('size-4x6'));
  await settled(s, `size-${size}`);
  return until(s, async () => (await text(s, 'print-price'))?.includes(size.replace('x', '×')));
}
const pay = async (s) => { await s.tap('pay'); return sheet(s); };
const answer = (s, t, value) => s.type(`@${t.ticket}`, JSON.stringify(value));
const tap = (s, t, choice) => s.tap(`@${t.ticket}`, { choice });
const line = (s, want) => until(s, async () => { const l = await text(s, 'print-line'); return want(l ?? '') ? l ?? '' : null; });
const checkout = async (id) => (await lab()).checkouts.find((c) => c.id === id);
const back = async (s) => { await s.tap('close-print'); await until(s, async () => !(await ids(s)).has('print')); };

await fetch(`${LAB}/fixture/reset`, { method: 'POST' });
await switches(OFF); // a lab left running by an earlier drive keeps its switches
const s = await open({ host, app: 'photo-editor', storage: `payments-${Date.now().toString(36)}` });
const run = (n) => !only || only.includes(n);
try {
  await s.clock('settle');
  if (host === 'linux') {
    // Drive 10's Linux part: no wallet, so no row.
    const status = (await s.state()).resources?.wallet;
    check(10, 'Linux has no wallet: `available` false', status?.available === false, JSON.stringify(status));
    check(10, 'and shows no Order a Print', !(await ids(s)).has('order-print'));
  } else {
    if (run(1)) {
      check(1, 'Order a Print is shown while the wallet can pay', !!(await until(s, async () => (await ids(s)).has('order-print'))));
      await printing(s);
      let t = await pay(s);
      const method = 'https://apple.com/apple-pay'; // the web's first method, and Apple's
      check(1, 'the payment button holds a payment ticket with the checkout\'s total', t?.device.args.total?.amount?.value === '15.00' && t.device.args.method === method, JSON.stringify(t?.device.args));
      if (host === 'web') {
        const sessions = (await lab()).sessions;
        check(9, 'Apple Pay on the web validated the merchant first, at an Apple gateway', sessions.at(-1)?.outcome === 'synthetic' && /apple\.com$/.test(sessions.at(-1)?.host ?? ''), JSON.stringify(sessions));
      }
      await answer(s, t, { shippingAddress: US });
      t = await sheet(s, (a) => a.total?.amount?.value === '20.00');
      check(1, 'an address adds Standard shipping to the total', !!t, JSON.stringify((await sheets(s))[0]?.device.args));
      await answer(s, t, { shippingOption: 'express' });
      t = await sheet(s, (a) => a.total?.amount?.value === '30.00');
      check(1, 'Express\'s total', !!t, JSON.stringify((await sheets(s))[0]?.device.args));
      const id = t?.device.args.requestId;
      await tap(s, t, 'authorize');
      await gone(s);
      const said = await line(s, (l) => l.startsWith('Ordered'));
      check(1, 'the confirmation', !!said, said ?? (await text(s, 'print-line')));
      const c = await checkout(id);
      check(1, 'the lab\'s checkout is paid, with the address', c?.state === 'paid' && c.address?.postalCode === '94301' && c.attempts.length === 1, JSON.stringify(c));
      const orders = await until(s, async () => (await ids(s)).has(`order-${id}`));
      check(1, 'Orders lists it', !!orders);
      const st = JSON.stringify(await s.state()), lines = (await s.logs({ all: true })).lines.join('\n');
      check(1, 'no state field or journal line holds the payment', !st.includes('exactSubstitute') && !lines.includes('exactSubstitute'), (st.includes('exactSubstitute') ? 'state' : 'journal'));
      await back(s);
    }
    if (run(2)) {
      await printing(s, '5x7');
      let t = await pay(s);
      await answer(s, t, { shippingAddress: { country: 'FR', city: 'Paris', postalCode: '75001' } });
      t = await sheet(s, (a) => a.shippingAddressErrors?.country);
      check(2, 'a country the lab does not ship to: the ticket shows the country error', !!t && !(t.device.args.shippingOptions ?? []).length, JSON.stringify((await sheets(s))[0]?.device.args));
      await answer(s, t, { shippingAddress: { ...US, addressLine: ['1 Fixture Way'], recipient: 'Fixture Payer' } });
      t = await sheet(s, (a) => !a.shippingAddressErrors && !a.error);
      check(2, 'a US address clears it', !!t, JSON.stringify((await sheets(s))[0]?.device.args));
      const quotes = (await lab()).quotes;
      check(2, 'the lab\'s quotes have no addressLine or recipient', quotes.every((q) => !q.address?.addressLine && !q.address?.recipient), JSON.stringify(quotes.at(-1)));
      const id = t?.device.args.requestId;
      await tap(s, t, 'authorize');
      await gone(s);
      const c = await until(s, async () => { const c = await checkout(id); return c?.state === 'paid' ? c : null; });
      check(2, 'the paid checkout has the full address', c?.address?.addressLine?.[0] === '1 Fixture Way', JSON.stringify(c ?? await checkout(id)));
      await back(s);
    }
    if (run(3)) {
      await switches({ ...OFF, rejectPostal: true });
      await printing(s, '4x6');
      let t = await pay(s);
      await answer(s, t, { shippingAddress: US });
      t = await sheet(s, (a) => a.total?.amount?.value === '9.00');
      await tap(s, t, 'authorize');
      t = await sheet(s, (a) => a.shippingAddressErrors?.postalCode);
      check(3, 'retry: the ticket stays with the postalCode error', t?.device.args.phase === 'editing', JSON.stringify((await sheets(s))[0]?.device.args));
      await answer(s, t, { shippingAddress: { ...US, postalCode: '94300' } });
      t = await sheet(s, (a) => !a.shippingAddressErrors);
      const id = t?.device.args.requestId;
      await tap(s, t, 'authorize');
      await gone(s);
      const c = await until(s, async () => { const c = await checkout(id); return c?.state === 'paid' ? c : null; });
      check(3, 'a corrected address and authorize again: one order', c?.attempts.filter((a) => a.outcome === 'paid').length === 1, JSON.stringify(c ?? await checkout(id)));
      await switches(OFF);
      await back(s);
    }
    if (run(4)) {
      await switches({ ...OFF, declineFirst: true });
      await printing(s, '4x6');
      let t = await pay(s);
      await answer(s, t, { shippingAddress: US });
      t = await sheet(s, (a) => a.total?.amount?.value === '9.00');
      const id = t?.device.args.requestId;
      await tap(s, t, 'authorize');
      await gone(s);
      const said = await line(s, (l) => l.includes('didn’t go through'));
      check(4, 'a decline: the send answers fail and the screen says so', !!said, await text(s, 'print-line'));
      check(4, 'the checkout stays open', (await checkout(id))?.state === 'open', JSON.stringify(await checkout(id)));
      t = await pay(s);
      await answer(s, t, { shippingAddress: US });
      t = await sheet(s, (a) => a.total?.amount?.value === '9.00');
      await tap(s, t, 'authorize');
      await gone(s);
      const c = await until(s, async () => { const c = await checkout(id); return c?.state === 'paid' ? c : null; });
      check(4, 'a second press on the same checkout pays it, with one charge', c?.attempts.filter((a) => a.outcome === 'paid').length === 1, JSON.stringify(c ?? await checkout(id)));
      await switches({ ...OFF, loseAnswer: true });
      await back(s);
      await printing(s, '5x7');
      t = await pay(s);
      await answer(s, t, { shippingAddress: US });
      t = await sheet(s, (a) => a.total?.amount?.value === '13.00');
      const lost = t?.device.args.requestId;
      await tap(s, t, 'authorize');
      await gone(s);
      await line(s, (l) => l.includes('didn’t go through'));
      await switches(OFF);
      t = await pay(s);
      await answer(s, t, { shippingAddress: US });
      t = await sheet(s, (a) => a.total?.amount?.value === '13.00');
      await tap(s, t, 'authorize');
      await gone(s);
      const ordered = await line(s, (l) => l.startsWith('Ordered'));
      const c2 = await checkout(lost);
      check(4, 'a lost answer: the second payment finds it paid and charges nothing more', !!ordered && c2?.attempts.length === 1 && c2.state === 'paid', JSON.stringify(c2));
      await back(s);
    }
    if (run(5)) {
      await printing(s, '4x6');
      const t = await pay(s);
      const id = t?.device.args.requestId;
      await tap(s, t, 'cancel');
      await gone(s);
      await s.clock('settle');
      check(5, 'cancel: no order, and the print sheet stays', (await ids(s)).has('print') && !(await text(s, 'print-line')) && (await checkout(id))?.attempts.length === 0, JSON.stringify(await checkout(id)));
      await back(s);
    }
    if (run(6)) {
      await printing(s, '4x6');
      let t = await pay(s);
      await back(s);
      check(6, 'leaving before authorize retires the ticket', !!(await gone(s)), JSON.stringify(await sheets(s)));
      await switches({ ...OFF, slow: 1500 });
      await printing(s, '4x6');
      t = await pay(s);
      await answer(s, t, { shippingAddress: US });
      t = await sheet(s, (a) => a.total?.amount?.value === '9.00');
      const id = t?.device.args.requestId;
      await tap(s, t, 'authorize');
      await back(s);
      await until(s, async () => (await checkout(id))?.state === 'paid', 6000);
      await gone(s);
      const lines = (await s.logs({ all: true })).lines.join('\n');
      check(6, 'left while authorizing: the order completes, and the journal says the sheet closed after its call ended', lines.includes('closed after its call ended') && (await checkout(id))?.state === 'paid', lines.split('\n').filter((l) => l.includes('payment')).slice(-4).join(' | '));
      await switches(OFF);
    }
    if (run(7)) {
      await switches({ ...OFF, slow: 30000 });
      await printing(s, '4x6');
      let t = await pay(s);
      await answer(s, t, { shippingAddress: US });
      t = await sheet(s, (a) => a.total?.amount?.value === '9.00');
      const id = t?.device.args.requestId;
      await tap(s, t, 'authorize');
      // The lab holds the attempt indeterminate while it answers slowly (30 s of wall time).
      const held = await until(s, async () => (await checkout(id))?.state === 'indeterminate');
      check(7, 'the lab holds the attempt indeterminate', !!held, JSON.stringify(await checkout(id)));
      await s.clock('+25000');
      await gone(s);
      // The answer reaches the screen once the lab's slow request ends, on the wall clock.
      const said = await until(s, async () => { const l = await text(s, 'print-line'); return l?.includes('didn’t go through') ? l : null; }, 45000);
      check(7, 'past the budget: the authorization fails', !!said, await text(s, 'print-line'));
      await switches(OFF);
      await back(s);
    }
    if (run(10)) {
      await s.prefer({ extension: 'payments.paymentRequestStatus={"available":false,"canMakePayment":false,"hasEnrolledInstrument":false}' });
      const hidden = await until(s, async () => !(await ids(s)).has('order-print'));
      check(10, 'with no wallet, no Order a Print', !!hidden);
    }
  }
} finally {
  await s.close();
  server?.kill();
}
console.log(failed ? `${failed} failed` : 'all passed');
process.exit(failed ? 1 : 0);
