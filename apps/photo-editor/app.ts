// The photo editor's data (LLP 1133.019 §11). An edit is saved to Photos as
// a new photo: the photo transformed by the edit into app:/tmp/edits/, then
// added to the library. The recent photos are the library's newest, read
// only once the person asks to see them or the read is granted, since the
// first read while `prompt` asks (D7).
//
// A print of the photo is paid with the device's wallet (LLP 1133.034 §10).
// The print lab (`fixture/prints.ts`) makes each checkout with its price
// fixed; the wallet's sheet calls `tools` while it is up: Safari's merchant
// validation, a shipping quote for the sheet's redacted address, and the
// order, which carries the payment to the lab and answers how it went.
import type { Answer, Storage } from './app.contract.d.ts';
import { addToPhotoLibrary, photoAssets } from '@exact/photos';

export const appId = 'com.exact.photoeditor';
const EDITS = 'app:/tmp/edits';
const LAB = 'http://127.0.0.1:4334';
const MERCHANT = 'merchant.com.exact.photoeditor';
export const grants = [
  'device.camera cameraPurpose',
  'device.photos-add savePurpose',
  'device.photos recentsPurpose',
  'fs.read app:/tmp',
  'fs.write app:/tmp',
  `payment.apple-pay ${MERCHANT}`,
  'payment.google-pay',
  `net.fetch ${LAB}`,
].join('\n');

// The wallets' method data: Apple Pay for the merchant ID, Google Pay through
// Google's `example` gateway, which answers test tokens only.
const APPLE_PAY = {
  version: 3,
  merchantIdentifier: MERCHANT,
  merchantCapabilities: ['supports3DS'],
  supportedNetworks: ['visa', 'masterCard', 'amex'],
  countryCode: 'US',
};
const GOOGLE_PAY = {
  apiVersion: 2,
  apiVersionMinor: 0,
  allowedPaymentMethods: [{
    type: 'CARD',
    parameters: { allowedAuthMethods: ['PAN_ONLY', 'CRYPTOGRAM_3DS'], allowedCardNetworks: ['VISA', 'MASTERCARD', 'AMEX'] },
    tokenizationSpecification: { type: 'PAYMENT_GATEWAY', parameters: { gateway: 'example', gatewayMerchantId: 'exampleGatewayMerchantId' } },
  }],
  merchantInfo: { merchantName: 'Photo Editor Prints' },
};
const METHODS = [
  { supportedMethods: 'https://apple.com/apple-pay', data: APPLE_PAY },
  { supportedMethods: 'https://google.com/pay', data: GOOGLE_PAY },
];
// Every field of a shape is answered; an option the app leaves unsaid is null.
const OPTIONS = { requestPayerName: null, requestPayerEmail: true, requestPayerPhone: null, requestShipping: true, requestBillingAddress: null, shippingType: null };
const item = (label: string, value: string) => ({ label, amount: { currency: 'USD', value }, pending: null });

/** A checkout's details: its id the lab's, its total the lab's fixed price. */
const details = (id: string, size: string, price: string) => ({
  id,
  total: item('Photo Editor Prints', price),
  displayItems: [item(`${size} print`, price)],
  shippingOptions: null,
  modifiers: null,
});

const post = async (path: string, body: unknown) => {
  const res = await fetch(`${LAB}${path}`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) });
  return { status: res.status, ok: res.ok, json: await res.json().catch(() => ({})) };
};

// The sheet's handlers (LLP 1133.034 D5), each given what the sheet's event gives.
export const tools = {
  async validateMerchant(e: { validationURL: string }) {
    const { json } = await post('/apple-pay/session', { url: e.validationURL, domain: 'localhost' });
    return json;
  },
  async quoteShipping(e: { requestId: string; shippingAddress: unknown; shippingOption: string | null }) {
    const { json } = await post(`/checkouts/${e.requestId}/quote`, { address: e.shippingAddress, option: e.shippingOption });
    return json;
  },
  async placeOrder(r: { requestId: string; methodName: string; details: unknown; shippingAddress: unknown; shippingOption: string | null; payerEmail: string | null }) {
    let res;
    try {
      res = await post(`/checkouts/${r.requestId}/pay`, { method: r.methodName, payment: r.details, ship: r.shippingAddress, option: r.shippingOption, email: r.payerEmail });
    } catch {
      return { result: 'fail' };
    }
    if (res.status === 422) return { retry: res.json.errors };
    if (!res.ok) return { result: 'fail' };
    if (res.json.state === 'indeterminate') return { result: 'unknown' };
    if (res.json.state !== 'paid') return { result: 'fail' };
    return { result: 'success', data: { orderId: res.json.orderId } };
  },
};

/** The edit in the photo's terms, as `<photo-editor>` reports it. */
type Edit = { rotate?: number; crop?: { x: number; y: number; width: number; height: number } };

const failed = (code: string, message: string) => ({ saved: false, asset: '', error: code, message, metadata: '' });

const sources: Record<string, (args: any, store: unknown, storage: Storage) => Promise<unknown>> = {
  savePhoto: async ([photo, edit]: [string, string], _store, storage) => {
    let edits: string[] = [];
    try {
      edits = await storage.fs.readdir(EDITS);
    } catch {
      // No edit saved yet: the first makes the directory.
    }
    await storage.fs.mkdir(EDITS);
    let e: Edit = {};
    try {
      e = JSON.parse(edit || '{}') as Edit;
    } catch {
      // An edit the editor has not reported yet is the photo as it is.
    }
    let made;
    try {
      made = await storage.fs.transformImage(photo, `${EDITS}/${edits.length + 1}`, { rotate: e.rotate ?? 0, crop: e.crop });
    } catch (err: any) {
      return failed(err?.code ?? 'UnknownError', `Couldn’t make the edited photo (${err?.code ?? err?.message ?? 'error'}).`);
    }
    try {
      const added = await addToPhotoLibrary({ path: made.path });
      return { saved: true, asset: added.asset ?? '', error: '', message: 'Saved to Photos', metadata: made.metadata };
    } catch (err: any) {
      const code = err?.code ?? err?.name ?? 'UnknownError';
      return failed(code, code === 'NotAllowedError' ? 'Photo Editor can’t add to your photos. Turn it on in Settings.' : `Couldn’t save to Photos (${code}).`);
    }
  },
  // The method data whether or not the sheet is open, so the editor can ask
  // the wallet first; a checkout at the lab once it is, for its size.
  printCheckout: async ([open, size]: [boolean, string]) => {
    const methodData = METHODS.map((m) => ({ supportedMethods: m.supportedMethods, data: JSON.stringify(m.data) }));
    const none = { id: '', size, price: '', methodData, details: details('', size, '0.00'), options: OPTIONS, button: '', error: '' };
    if (!open) return none;
    let made;
    try {
      made = await post('/checkouts', { size });
    } catch {
      return { ...none, error: 'The print lab isn’t answering.' };
    }
    if (!made.ok) return { ...none, error: `The print lab can’t print ${size}.` };
    const d = details(made.json.id, size, made.json.price);
    // The payment button's checkout (the web's sheet opens in its press).
    const button = JSON.stringify({ methodData: METHODS, details: d, options: OPTIONS });
    return { ...none, id: made.json.id, price: made.json.price, details: d, button };
  },
  // What the lab did, which the confirmation shows: a payment that finished
  // after its screen went away is here too.
  printOrders: async () => {
    try {
      const res = await fetch(`${LAB}/checkouts`);
      const tried = (await res.json()).orders.filter((o: { attempts: number }) => o.attempts > 0);
      return { orders: tried.map(({ id, size, price, shipping, state, orderId }: Record<string, string>) => ({ id, size, price, shipping, state, orderId })), error: '' };
    } catch {
      return { orders: [], error: 'The print lab isn’t answering.' };
    }
  },
  // A read refused (denied, or a selection that left nothing) shows no strip, not the last one.
  recents: async ([show]: [boolean]) => {
    if (!show) return { assets: [] };
    try {
      return await photoAssets({ limit: 24 });
    } catch {
      return { assets: [] };
    }
  },
};

export const answer = ((source: string, args: unknown, store: unknown, storage: Storage) => sources[source](args, store, storage)) as unknown as Answer;
