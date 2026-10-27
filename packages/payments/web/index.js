// @exact/payments on the web (LLP 1133.034 D8): the W3C Payment Request API,
// whichever of Apple Pay (Safari) and Google Pay (Chrome) the browser has.
// The page runtime (host/web-js/modules.js) calls it outside the agent;
// under the agent the declared substitute answers names, and this module
// draws only the button.
//
// - `showPaymentRequest` is a `gesture` send: it runs inside the button's
//   press, so `show()` is called before anything is awaited, while the
//   press's activation holds.
// - The sheet's events call the app's handlers through `invoke`, by the
//   call's `request:<n>` handle; each has the declared budget, past which an
//   update leaves the sheet as it was and an authorization fails.
// - A `retry` answer is `PaymentResponse.retry()` with its field errors, and
//   the final handler runs again on the next authorization.
// - Google Pay's `environment` is `TEST` until the app has a production
//   Google Pay merchant, which the bake will say (§13 question 7).
// - `cancel` aborts the request unless its final handler is running (D16 item 2).

const APPLE = 'https://apple.com/apple-pay';
const GOOGLE = 'https://google.com/pay';
const BUDGET = 25000;

/** A method's `data`: JSON text from a Contract call, an object from data code. */
const dataOf = m => {
  if (typeof m?.data !== 'string') return m?.data ?? undefined;
  try { return JSON.parse(m.data); } catch { return undefined; }
};
const methods = list => (Array.isArray(list) ? list : []).map(m => {
  const data = dataOf(m);
  return m.supportedMethods === GOOGLE ? { supportedMethods: GOOGLE, data: { ...data, environment: 'TEST' } } : { supportedMethods: m.supportedMethods, ...(data ? { data } : {}) };
});
const without = o => Object.fromEntries(Object.entries(o ?? {}).filter(([, v]) => v != null));
const details = d => without({ id: d?.id, total: d?.total, displayItems: d?.displayItems, shippingOptions: d?.shippingOptions,
  modifiers: d?.modifiers?.map(m => without({ ...m, data: dataOf(m) })) });
const update = u => without({ error: u?.error, total: u?.total, displayItems: u?.displayItems, shippingOptions: u?.shippingOptions,
  modifiers: u?.modifiers?.map(m => without({ ...m, data: dataOf(m) })), shippingAddressErrors: u?.shippingAddressErrors,
  payerErrors: u?.payerErrors, paymentMethodErrors: u?.paymentMethodErrors });
const address = a => a ? (typeof a.toJSON === 'function' ? a.toJSON() : { ...a }) : null;
const failure = e => ({ error: typeof e?.name === 'string' && /Error$/.test(e.name) ? e.name : 'OperationError', ...(e?.message ? { message: e.message } : {}) });
const LABELS = { plain: 'Pay', buy: 'Buy', checkout: 'Check out', order: 'Order', book: 'Book', donate: 'Donate', subscribe: 'Subscribe', pay: 'Pay' };

class PaymentButton {
  constructor({ element, props, events }) {
    const doc = element.ownerDocument;
    this.button = doc.createElement('button');
    this.button.type = 'button';
    this.button.addEventListener('click', () => events.press());
    element.append(this.button);
    this.setProps(props);
  }
  setProps(props) {
    const type = props.buttonType || 'plain', b = this.button;
    b.disabled = props.disabled === true || props.disabled === 'true';
    // Safari draws Apple's own button (Apple Pay on the web, "Displaying Apple Pay buttons using CSS").
    if (typeof CSS === 'object' && CSS.supports?.('-webkit-appearance', '-apple-pay-button')) {
      b.textContent = '';
      b.setAttribute('aria-label', `${LABELS[type]} with Apple Pay`);
      b.style.cssText = `-webkit-appearance:-apple-pay-button;-apple-pay-button-type:${type === 'pay' ? 'plain' : type};width:100%;height:100%;min-height:30px`;
      return;
    }
    b.removeAttribute('aria-label');
    b.textContent = LABELS[type] ?? 'Pay';
    b.style.cssText = 'width:100%;height:100%;min-height:40px;border:0;border-radius:6px;background:#000;color:#fff;font:600 16px system-ui;cursor:pointer';
  }
  destroy() { this.button.remove(); }
}

export const roster = { 'exact-payment-button': { view: PaymentButton } };

export function implementation({ invoke = () => Promise.resolve({ error: 'NotSupportedError' }), log = () => {} }) {
  const present = typeof PaymentRequest === 'function';
  // Each shown request, by the call's number: its request, and whether its final handler runs.
  const shown = new Map();
  /** The app's handler for request `n`, its answer parsed, or null past the budget or on an error. */
  const call = (n, handler, arg) => new Promise(resolve => {
    const timer = setTimeout(() => { log(`payments: ${handler} for request ${n}: past its budget`); resolve(null); }, BUDGET);
    invoke(`request:${n}`, handler, arg).then(a => {
      clearTimeout(timer);
      try { resolve(typeof a?.text === 'string' ? JSON.parse(a.text) : null); } catch { resolve(null); }
    }, () => { clearTimeout(timer); resolve(null); });
  });
  return {
    async paymentRequestStatus({ methodData }) {
      if (!present) return { available: false, canMakePayment: false, hasEnrolledInstrument: false };
      const total = { label: 'Total', amount: { currency: 'USD', value: '0.00' } };
      for (const m of methods(methodData)) {
        try {
          const request = new PaymentRequest([m], { total });
          if (!(await request.canMakePayment())) continue;
          const enrolled = typeof request.hasEnrolledInstrument === 'function' ? await request.hasEnrolledInstrument().catch(() => null) : null;
          return { available: true, canMakePayment: true, hasEnrolledInstrument: enrolled, methodName: m.supportedMethods };
        } catch { /* a method this browser does not have */ }
      }
      return { available: true, canMakePayment: false, hasEnrolledInstrument: false };
    },
    showPaymentRequest(args, { request: n }) {
      if (!present) return { error: 'NotSupportedError' };
      let pr;
      try { pr = new PaymentRequest(methods(args.methodData), details(args.details), without(args.options)); }
      catch (e) { return failure(e); }
      const id = pr.id, entry = { pr, final: false };
      shown.set(n, entry);
      const arg = (e, extra) => ({ requestId: id, shippingAddress: address(pr.shippingAddress), shippingOption: pr.shippingOption ?? null, methodName: e?.methodName ?? null, ...extra });
      const updating = handler => e => e.updateWith(call(n, handler, arg(e)).then(u => u ? update(u) : { error: 'unavailable' }));
      if (args.onshippingaddresschange) pr.onshippingaddresschange = updating(args.onshippingaddresschange);
      if (args.onshippingoptionchange) pr.onshippingoptionchange = updating(args.onshippingoptionchange);
      if (args.onpaymentmethodchange) {
        pr.onpaymentmethodchange = e => e.updateWith(call(n, args.onpaymentmethodchange, arg(e, { methodDetails: e.methodDetails ?? null })).then(u => u ? update(u) : {}));
      }
      if (args.onmerchantvalidation) {
        pr.onmerchantvalidation = e => e.complete(call(n, args.onmerchantvalidation, { requestId: id, methodName: e.methodName, validationURL: e.validationURL })
          .then(v => v?.merchantSession ?? Promise.reject(new DOMException('no merchant session', 'OperationError'))));
      }
      // `show()` first, in the press; everything after it may wait.
      const showing = pr.show();
      return (async () => {
        let response;
        try { response = await showing; } catch (e) { shown.delete(n); return failure(e); }
        try {
          for (;;) {
            entry.final = true;
            const r = response;
            const answer = await call(n, args.onpaymentauthorized, { requestId: id, methodName: r.methodName, details: r.details,
              shippingAddress: address(r.shippingAddress), shippingOption: r.shippingOption ?? null, payerName: r.payerName ?? null,
              payerEmail: r.payerEmail ?? null, payerPhone: r.payerPhone ?? null });
            entry.final = false;
            if (answer?.retry && typeof answer.retry === 'object') {
              await r.retry(without({ error: answer.retry.error, shippingAddress: answer.retry.shippingAddress, payer: answer.retry.payer, paymentMethod: answer.retry.paymentMethod }));
              continue;
            }
            const result = ['success', 'fail', 'unknown'].includes(answer?.result) ? answer.result : 'fail';
            await r.complete(result);
            const apple = r.methodName === APPLE ? { paymentMethod: r.details?.token?.paymentMethod ?? null } : null;
            const info = r.details?.paymentMethodData;
            const android = r.methodName === GOOGLE ? { paymentMethodDescription: info?.description ?? null, cardNetwork: info?.info?.cardNetwork ?? null } : null;
            return { requestId: id, methodName: r.methodName, shippingAddress: address(r.shippingAddress), shippingOption: r.shippingOption ?? null,
              payerName: r.payerName ?? null, payerEmail: r.payerEmail ?? null, payerPhone: r.payerPhone ?? null, result,
              message: answer ? null : 'authorize-timeout', data: answer?.data == null ? null : JSON.stringify(answer.data), apple, android };
          }
        } catch (e) { return failure(e); }
        finally { shown.delete(n); }
      })();
    },
    /** A send let go: its sheet aborted, unless its final handler runs, whose answer then completes it. */
    cancel(n) {
      const entry = shown.get(n);
      if (!entry || entry.final) return;
      entry.pr.abort().catch(() => {});
    },
  };
}
