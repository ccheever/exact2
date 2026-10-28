import type { Answer } from './app.contract.d.ts';

// The drive writes the id `exact new` gave the app. Google Pay only, in its
// `TEST` environment, through Google's `example` gateway: test tokens only.
export const appId = '__APP_ID__';
const LAB = 'http://127.0.0.1:4334';
export const grants = ['payment.google-pay', `net.fetch ${LAB}`].join('\n');

const GOOGLE_PAY = {
  apiVersion: 2,
  apiVersionMinor: 0,
  allowedPaymentMethods: [{
    type: 'CARD',
    parameters: { allowedAuthMethods: ['PAN_ONLY', 'CRYPTOGRAM_3DS'], allowedCardNetworks: ['VISA', 'MASTERCARD', 'AMEX'] },
    tokenizationSpecification: { type: 'PAYMENT_GATEWAY', parameters: { gateway: 'example', gatewayMerchantId: 'exampleGatewayMerchantId' } },
  }],
  merchantInfo: { merchantName: 'Payments Fixture' },
};
const METHODS = [{ supportedMethods: 'https://google.com/pay', data: GOOGLE_PAY }];
// Every field of a shape is answered; an option the app leaves unsaid is null.
const OPTIONS = { requestPayerName: null, requestPayerEmail: true, requestPayerPhone: null, requestShipping: true, requestBillingAddress: null, shippingType: null };
const item = (label: string, value: string) => ({ label, amount: { currency: 'USD', value }, pending: null });
const details = (id: string, price: string) => ({ id, total: item('Payments Fixture', price), displayItems: null, shippingOptions: null, modifiers: null });

const post = async (path: string, body: unknown) => {
  const res = await fetch(`${LAB}${path}`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) });
  return { status: res.status, ok: res.ok, json: await res.json().catch(() => ({})) };
};

// The sheet's handlers (LLP 1133.034 D5), as the photo editor's.
export const tools = {
  async quoteShipping(e: { requestId: string; shippingAddress: unknown; shippingOption: string | null }) {
    return (await post(`/checkouts/${e.requestId}/quote`, { address: e.shippingAddress, option: e.shippingOption })).json;
  },
  async placeOrder(r: { requestId: string; methodName: string; details: unknown; shippingAddress: unknown; shippingOption: string | null; payerEmail: string | null }) {
    const res = await post(`/checkouts/${r.requestId}/pay`, { method: r.methodName, payment: r.details, ship: r.shippingAddress, option: r.shippingOption, email: r.payerEmail }).catch(() => null);
    if (!res) return { result: 'fail' };
    if (res.status === 422) return { retry: res.json.errors };
    if (res.ok && res.json.state === 'paid') return { result: 'success', data: { orderId: res.json.orderId } };
    return { result: res.ok && res.json.state === 'indeterminate' ? 'unknown' : 'fail' };
  },
};

const sources: Record<string, (args: any) => Promise<unknown>> = {
  checkout: async ([open]: [boolean]) => {
    const methodData = METHODS.map((m) => ({ supportedMethods: m.supportedMethods, data: JSON.stringify(m.data) }));
    const none = { id: '', methodData, details: details('', '0.00'), options: OPTIONS, button: '', error: '' };
    if (!open) return none;
    const made = await post('/checkouts', { size: '4x6' }).catch(() => null);
    if (!made?.ok) return { ...none, error: 'The print lab isn’t answering.' };
    const d = details(made.json.id, made.json.price);
    return { ...none, id: made.json.id, details: d, button: JSON.stringify({ methodData: METHODS, details: d, options: OPTIONS }) };
  },
};
export const answer = ((source: string, args: unknown) => sources[source](args)) as unknown as Answer;
