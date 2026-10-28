// The seam is public (LLP 1133.001 D8, LLP 1133.034 §12 stage 5): two tests
// that need nothing an outside author lacks, in Chromium.
//
// - Out of the tree: this package, packed as a registry would carry it
//   (`bun pm pack`), installed into an app `exact new` made outside the repo,
//   builds unchanged and its web implementation answers.
// - Replaced: a fixture implementation outside the tree, a direct dependency
//   of the app, replaces the endorsed web implementation, its button view
//   included: it answers `paymentRequestStatus`, and its `showPaymentRequest`
//   calls the app's `placeOrder` through `invoke` (D5), which pays the print
//   lab with the fixture's token.
//
// usage: bun packages/payments/test/seams.mjs
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { checker, inPage, lab, makeFixture, pkg, run, webBuild } from './fixture.mjs';

const { check, failed } = checker();
const scratch = mkdtempSync(join(tmpdir(), 'exact-payments-seams-'));
const prints = await lab();
const made = [];
try {
  // Out of the tree: the tarball, installed as any registry package is.
  run('bun', ['pm', 'pack', '--destination', scratch], pkg);
  const tarball = join(scratch, readdirSync(scratch).find((f) => f.endsWith('.tgz')));
  const packed = makeFixture({ payments: `file:${tarball}` });
  made.push(packed.dir);
  const installed = JSON.parse(readFileSync(join(packed.app, 'node_modules/@exact/payments/package.json'), 'utf8'));
  check('packed', 'the tarball is installed, not a link into the checkout', installed.name === '@exact/payments' && !readFileSync(join(packed.app, 'package.json'), 'utf8').includes(`file:${pkg}`), JSON.stringify(installed));
  webBuild(packed.app);
  const packedRun = await inPage(packed.app, 'chromium', async (page, until, tap) => {
    await tap('start');
    await page.getByTestId('pay-plain').waitFor({ timeout: 10000 });
    await tap('pay-plain');
    return { wallet: await until('wallet', (t) => t !== ''), failed: await until('failed', (t) => t !== '') };
  });
  check('packed', 'it builds unchanged and the web implementation answers', packedRun.wallet === 'true false' && packedRun.failed === 'UnknownError', JSON.stringify(packedRun));

  // Replaced: an implementation outside the tree, its own button and wallet.
  const dir = join(scratch, 'fixture-payments-web');
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, 'package.json'), JSON.stringify({ name: '@fixture/payments-web', version: '1.0.0', exports: { '.': './exact.json' } }));
  writeFileSync(join(dir, 'exact.json'), JSON.stringify({ kind: 'implementation', implements: '@exact/payments@^1.0', platform: 'web', targets: ['web'], views: ['exact-payment-button'], sources: { javascript: ['index.js'] } }));
  writeFileSync(join(dir, 'index.js'), `class Button {
  constructor({ element, events }) {
    this.button = element.ownerDocument.createElement('button');
    this.button.dataset.fixture = 'payments';
    this.button.textContent = 'Fixture pay';
    this.button.addEventListener('click', () => events.press());
    element.append(this.button);
  }
  setProps() {}
  destroy() { this.button.remove(); }
}
export const roster = { 'exact-payment-button': { view: Button } };
export function implementation({ invoke }) {
  const GOOGLE = 'https://google.com/pay';
  return {
    async paymentRequestStatus() { return { available: true, canMakePayment: true, hasEnrolledInstrument: true, methodName: GOOGLE }; },
    async showPaymentRequest(args, { request }) {
      const ship = { country: 'US', region: 'CA', city: 'Palo Alto', postalCode: '94301' };
      const a = await invoke('request:' + request, args.onpaymentauthorized, { requestId: args.details.id, methodName: GOOGLE,
        details: { paymentMethodData: { tokenizationData: { token: 'fixture-token' } } }, shippingAddress: ship, shippingOption: null,
        payerName: null, payerEmail: 'fixture@example.com', payerPhone: null });
      const answer = JSON.parse(a.text);
      return { requestId: args.details.id, methodName: GOOGLE, shippingAddress: ship, shippingOption: null, payerName: null,
        payerEmail: 'fixture@example.com', payerPhone: null, result: answer.result, message: null,
        data: answer.data ? JSON.stringify(answer.data) : null, apple: null, android: null };
    },
  };
}
`);
  const replaced = makeFixture({ dependencies: { '@fixture/payments-web': `file:${dir}` } });
  made.push(replaced.dir);
  webBuild(replaced.app);
  const replacedRun = await inPage(replaced.app, 'chromium', async (page, until, tap) => {
    await tap('start');
    const wallet = await until('wallet', (t) => t === 'true true');
    await page.getByTestId('pay').waitFor({ timeout: 10000 });
    // A package view mounts after its element: wait for the fixture's button, else say what is there.
    const view = await page.locator('[data-testid="pay"] [data-fixture="payments"]').waitFor({ timeout: 10000 })
      .then(() => true, () => page.evaluate(() => document.querySelector('[data-testid="pay"]')?.outerHTML.slice(0, 600) ?? 'no pay'));
    await tap('pay');
    return { wallet, view, line: await until('line', (t) => t !== '') };
  });
  check('replaced', "a direct dependency's view replaces the endorsed button, and its wallet answers", replacedRun.view === true && replacedRun.wallet === 'true true', JSON.stringify(replacedRun));
  check('replaced', "its sheet calls the app's placeOrder through invoke, which pays the lab", replacedRun.line === 'success https://google.com/pay', JSON.stringify(replacedRun));
  const log = await prints.log();
  check('replaced', "the lab has the fixture's payment, paid once", log.payments.filter((p) => p.outcome === 'paid' && p.tokenLength === 'fixture-token'.length).length === 1, JSON.stringify(log.payments));
} finally {
  prints.stop();
  for (const d of made) rmSync(d, { recursive: true, force: true });
  rmSync(scratch, { recursive: true, force: true });
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
