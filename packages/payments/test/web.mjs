// @exact/payments on the web, real (LLP 1133.034 §12 stage 2): the fixture
// app built for the web, outside the agent, in Playwright's Chromium, Firefox
// and WebKit. No wallet is signed in to any of them, so what runs is what a
// browser answers without one:
//
// - `paymentRequestStatus` reads `PaymentRequest`'s presence and each
//   method's `canMakePayment()`. Playwright's Firefox and WebKit have no
//   `PaymentRequest`; its Chromium has one that can pay with no method here.
// - A press sends `showPaymentRequest` in its own dispatch (`"gesture"`), and
//   the send fails with the browser's answer: `NotSupportedError` where there
//   is no `PaymentRequest`, and in Playwright's Chromium, which has no payment
//   service, `UnknownError` from `show()` itself. That build answers the same
//   without activation, so the press's activation is not shown here.
// - The checkout comes from the print lab, through the fixture's `net.fetch`
//   grant, and no payment reaches it.
//
// Google Pay's sheet in Chrome needs a Google test account in the profile,
// and Apple Pay's in Safari the public origin: checklist.md's.
//
// usage: bun packages/payments/test/web.mjs
import { rmSync } from 'node:fs';
import { checker, inPage, lab, makeFixture, webBuild } from './fixture.mjs';

const { check, failed } = checker();
const fixture = makeFixture();
const prints = await lab();
try {
  webBuild(fixture.app);
  const want = { chromium: 'true false', firefox: 'false false', webkit: 'false false' };
  const failure = { chromium: 'UnknownError', firefox: 'NotSupportedError', webkit: 'NotSupportedError' };
  for (const engine of ['chromium', 'firefox', 'webkit']) {
    const seen = await inPage(fixture.app, engine, async (page, until, tap) => {
      const exceptions = [];
      page.on('pageerror', (e) => exceptions.push(String(e)));
      const before = await until('wallet', (t) => /^(true|false) (true|false)$/.test(t));
      await tap('start');
      const plain = await page.getByTestId('pay-plain').waitFor({ timeout: 10000 }).then(() => true, () => false);
      const error = await until('error', () => true, 500);
      const status = await until('wallet', (t) => /^(true|false) (true|false)$/.test(t));
      const button = await page.getByTestId('pay').count();
      if (plain) await tap('pay-plain');
      const failed = plain ? await until('failed', (t) => t !== '') : '';
      return { before, status, error, button, failed, exceptions };
    });
    check(engine, `the wallet reads ${want[engine]} with no wallet signed in`, seen.status === want[engine] && seen.before === want[engine], JSON.stringify(seen));
    check(engine, 'the lab made the checkout, and no payment button shows where the wallet cannot pay', seen.error === '' && seen.button === 0, JSON.stringify(seen));
    check(engine, `a press sends the request, and the send fails with the browser's ${failure[engine]}`, seen.failed === failure[engine] && !seen.exceptions.length, JSON.stringify(seen));
  }
  const log = await prints.log();
  check('lab', 'a checkout per browser, and no payment or refused token reached the lab', log.checkouts.length >= 3 && !log.payments.length && !log.refused.length, JSON.stringify(log).slice(0, 2000));
} finally {
  prints.stop();
  rmSync(fixture.dir, { recursive: true, force: true });
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
