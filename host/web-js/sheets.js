// A sheet that asks the app while it is up (LLP 1133.034 §9), on the JS
// target, as the runner's `modules/sheet.rs`: a ceremony whose substitute
// declares `asks` holds one `payment` ticket; `type @t {"shippingAddress":
// {…}}` (redacted by the W3C's rule), `{"shippingOption": "<id>"}` and
// `{"paymentMethod": {…}}` run the handler the call names from the data
// module's `tools`, as `tool`, and lay its `PaymentDetailsUpdate` over the
// ticket; `tap @t authorize` runs the final handler with a synthetic payment
// in the method's own shape. Apple Pay validates the merchant first. One
// handler at a time; each has the budget on the session's clock. A send let
// go closes the sheet, unless it is authorizing (the handler finishes, and
// its answer is dropped) or retrying (abandoned until `cancel`). The target
// is the web, whose sheet closes on every result. No token, address or payer
// reaches the ticket, the journal or `state.extensions`.
import { data, Store, say, clock } from "./rt.js";

const APPLE = "https://apple.com/apple-pay";
/** Apple's sandbox gateway, the host of the validation URL Safari would give. */
const VALIDATION_URL = "https://apple-pay-gateway-cert.apple.com/paymentservices/startSession";
const PAYER = { payerName: "Fixture Payer", payerEmail: "payer@fixture.test", payerPhone: "+15555550100" };
const REQUESTED = { payerName: "requestPayerName", payerEmail: "requestPayerEmail", payerPhone: "requestPayerPhone" };

/** A method's `data`: JSON text from a Contract call, an object from data code. */
const dataOf = m => {
  if (typeof m?.data !== "string") return m?.data ?? null;
  try { return JSON.parse(m.data); } catch { return null; }
};
const amountText = a => `${a?.currency ?? ""} ${a?.value ?? ""}`;
const redacted = address => {
  if (!address || typeof address !== "object") return address ?? null;
  const out = { ...address };
  for (const field of ["organization", "phone", "recipient", "addressLine"]) delete out[field];
  return out;
};
/** A `PaymentCurrencyAmount`'s refusal (W3C: an ISO 4217 code; a valid decimal monetary value). */
const amountRefused = (amount, what) => {
  const currency = amount?.currency ?? "", value = amount?.value ?? "";
  if (!/^[A-Za-z]{3}$/.test(currency)) return ["RangeError", `${what}'s currency \`${currency}\` is no ISO 4217 code`];
  if (!/^-?\d+(\.\d+)?$/.test(value)) return ["TypeError", `${what}'s value \`${value}\` is no decimal string`];
  return null;
};

// `release(h, json)` answers the call and ends its ticket; `retire(h)` ends its ticket alone, recorded.
export function sheets({ release, retire }) {
  const open = new Map(); // held call → its sheet
  let payments = 0, calls = 0;
  const methods = row => row.sub.methods ?? {};
  const methodHere = (row, args) => (args?.methodData ?? []).map(m => m?.supportedMethods).find(m => methods(row)[m]?.targets?.includes("web")) ?? null;
  const grants = () => (data.grants ?? "").split("\n").map(l => l.trim().split(/\s+/));

  /** The call's refusal before any sheet (D10, D13, D16 item 4), as the runner's `methods_admitted` and value grants. */
  const admit = (row, args) => {
    const declared = methods(row), refuse = (error, message) => ({ error, message });
    const list = Array.isArray(args?.methodData) ? args.methodData : [];
    if (!list.length) return refuse("TypeError", "methodData is empty");
    const keys = (value, allowed, method, place) => {
      if (value == null) return null;
      if (typeof value !== "object") return refuse("TypeError", `${method}'s ${place} is no object`);
      for (const key of Object.keys(value)) {
        const code = declared[method]?.refuse?.[key];
        if (code) return refuse("TypeError", `${code}: \`${key}\` is the bake's, never the app's`);
        if (!(allowed ?? []).includes(key)) return refuse("TypeError", `\`${key}\` is no key of ${method}'s ${place}`);
      }
      return null;
    };
    const lines = grants();
    for (const m of list) {
      const d = declared[m?.supportedMethods];
      if (!d) continue;
      const bad = keys(dataOf(m), d.data, m.supportedMethods, "data");
      if (bad) return bad;
      if (!lines.some(([l]) => l === d.grant)) return refuse("NotSupportedError", `grant-missing (${m.supportedMethods} needs \`${d.grant}\`)`);
    }
    // A grant matched by a method's value (`methodData[].data.merchantIdentifier`, `when` its method).
    for (const [path, spec] of Object.entries(row.sub.nameGrants ?? {})) {
      if (!spec?.line || !path.startsWith("methodData[].")) continue;
      const rest = path.slice("methodData[].".length).split(".");
      for (const m of list) {
        if (spec.when && Object.entries(spec.when).some(([k, v]) => m?.[k] !== v)) continue;
        const value = rest[0] === "data" ? rest.slice(1).reduce((x, k) => x?.[k], dataOf(m)) : rest.reduce((x, k) => x?.[k], m);
        if (typeof value !== "string" || !lines.some(([l, t]) => l === spec.line && t === value)) {
          return refuse(spec.refuse ?? "NotAllowedError", `grant-missing (${path}=${value} needs \`${spec.line} ${value}\`)`);
        }
      }
    }
    const details = args?.details ?? {};
    const amounts = [["total", details.total?.amount]];
    for (const item of details.displayItems ?? []) amounts.push(["a display item", item?.amount]);
    for (const item of details.shippingOptions ?? []) amounts.push(["a shipping option", item?.amount]);
    const exclusive = new Set();
    for (const modifier of details.modifiers ?? []) {
      if (modifier?.total) amounts.push(["a modifier's total", modifier.total.amount]);
      const d = declared[modifier?.supportedMethods];
      if (!d) continue;
      const md = dataOf(modifier), bad = keys(md, d.modifier, modifier.supportedMethods, "modifier");
      if (bad) return bad;
      for (const key of d.exclusive ?? []) if (md?.[key] != null) exclusive.add(key);
    }
    if (exclusive.size > 1) return refuse("TypeError", `${[...exclusive].join(" and ")} exclude one another (PassKit)`);
    for (const [what, amount] of amounts) { const r = amountRefused(amount, what); if (r) return refuse(...r); }
    if (String(details.total?.amount?.value ?? "").startsWith("-")) return refuse("TypeError", "the total is negative");
    if (!methodHere(row, args)) return refuse("NotSupportedError", "no method in methodData that web has");
    return null;
  };

  const sheetOf = (h, id) => {
    const sub = h.row.sub, args = h.args ?? {}, details = args.details ?? {};
    const handlers = Object.fromEntries(Object.keys(sub.callbacks ?? {}).filter(k => typeof args[k] === "string").map(k => [k, args[k]]));
    const option = (details.shippingOptions ?? []).filter(o => o?.selected === true).at(-1)?.id ?? null;
    return { h, id: details.id ?? id, method: methodHere(h.row, args), handlers, origin: h.origin ?? "app",
      details: { total: details.total, displayItems: details.displayItems ?? null, shippingOptions: details.shippingOptions ?? null, modifiers: details.modifiers ?? null },
      options: args.options ?? {}, errors: {}, address: null, option, phase: "editing", retrying: false, last: null, letGo: false,
      asking: null, handled: [], budget: sub.ceremony?.budget ?? 25000, card: null };
  };

  /** The ticket's `args`: never an address, a payer or a token. */
  const args = h => {
    const s = open.get(h) ?? sheetOf(h, ""), o = s.options ?? {};
    return { request: "showPaymentRequest", ceremony: "showPaymentRequest", requestId: s.id, method: s.method,
      total: s.details.total ?? null, displayItems: s.details.displayItems, shippingOptions: s.details.shippingOptions, shippingOption: s.option,
      requested: { payerName: o.requestPayerName === true, payerEmail: o.requestPayerEmail === true, payerPhone: o.requestPayerPhone === true, shipping: o.requestShipping === true, billingAddress: o.requestBillingAddress === true },
      error: s.errors.error ?? null, shippingAddressErrors: s.errors.shippingAddressErrors ?? null, payerErrors: s.errors.payerErrors ?? null, paymentMethodErrors: s.errors.paymentMethodErrors ?? null,
      phase: s.phase, waiting: s.asking?.ask ?? null, result: s.last?.result ?? null, origin: s.origin, target: "web" };
  };

  /** The sheet is held: on the web with Apple Pay, merchant validation is asked first. */
  const opened = h => {
    const s = sheetOf(h, `payment-${++calls}`);
    open.set(h, s);
    say(`payment ${s.id}: held, ${s.method} ${amountText(s.details.total?.amount)}`);
    if (s.method !== APPLE) return;
    if (!s.handlers.onmerchantvalidation) return end(s, { error: "OperationError", message: "payment-merchant-validation: Apple Pay on the web needs onmerchantvalidation" });
    s.phase = "validating";
    run(s, "merchantValidation", "onmerchantvalidation", { requestId: s.id, methodName: APPLE, validationURL: VALIDATION_URL });
  };

  /** Run the handler `keyword` names for `ask`; `false` when the call names none. */
  const run = (s, ask, keyword, arg) => {
    const handler = s.handlers[keyword];
    if (!handler) return false;
    const call = ++calls, started = clock.now;
    s.asking = { ask, handler, call, started, ends: started + s.budget };
    const at = `payments: ${handler} for ${s.id}`;
    say(`${at}: run, tool`);
    let p;
    try { p = data.tool?.(handler, arg, Store, "tool"); } catch (e) { p = Promise.reject(e); }
    if (!p) { say(`${at}: refused, the data module's \`tools\` has no \`${handler}\``); p = Promise.reject(Object.assign(new Error("no handler"), { code: "NotFoundError" })); }
    Promise.resolve(p).then(v => handled(s, call, { text: JSON.stringify(v ?? null) }), e => { say(`${at}: failed: ${e?.message ?? e}`); handled(s, call, { error: e?.code ?? "OperationError" }); });
    return true;
  };

  const handled = (s, call, answer) => {
    if (s.asking?.call !== call || !open.has(s.h)) return say(`payments: handler call ${call} answered after its sheet moved on; dropped`);
    const asking = s.asking;
    s.asking = null;
    let value = null;
    if (typeof answer.text === "string") { try { value = JSON.parse(answer.text); } catch {} }
    const kind = value == null ? "error" : asking.ask !== "authorize" ? "update" : value.retry != null ? "retry" : value.result ?? "malformed";
    s.handled.push({ name: asking.handler, ms: clock.now - asking.started, answer: kind });
    if (asking.ask === "merchantValidation") {
      const session = value?.merchantSession;
      if (session && typeof session === "object" && Object.keys(session).length) s.phase = "editing";
      else end(s, { error: "OperationError", message: "merchant validation answered no merchant session" });
      return;
    }
    if (asking.ask === "authorize") {
      if (value?.retry && typeof value.retry === "object") {
        const r = value.retry;
        s.errors = { error: r.error ?? null, shippingAddressErrors: r.shippingAddress ?? null, payerErrors: r.payer ?? null, paymentMethodErrors: r.paymentMethod ?? null };
        s.retrying = true;
        s.phase = s.letGo ? "abandoned" : "editing";
        return;
      }
      if (["success", "fail", "unknown"].includes(value?.result)) return result(s, value.result, null, value.data ?? null);
      return result(s, "fail", "authorize-timeout", null);
    }
    if (value && typeof value === "object") updated(s, value);
    else s.errors = { error: "unavailable" };
  };

  const updated = (s, u) => {
    for (const f of ["total", "displayItems", "shippingOptions", "modifiers"]) if (u[f] != null) s.details[f] = u[f];
    if (Array.isArray(u.shippingOptions)) s.option = u.shippingOptions.filter(o => o?.selected === true).at(-1)?.id ?? null;
    s.errors = { error: u.error ?? null, shippingAddressErrors: u.shippingAddressErrors ?? null, payerErrors: u.payerErrors ?? null, paymentMethodErrors: u.paymentMethodErrors ?? null };
  };

  /** The authorization's result: the web's sheet closes on each (`complete()`). */
  const result = (s, value, message, completeData) => {
    s.last = { result: value, message, data: completeData };
    say(`payment ${s.id}: ${value}${message ? `, ${message}` : ""}`);
    if (s.letGo) return dropped(s, `closed after its call ended, ${value}`);
    end(s, paymentResult(s));
  };

  /** The send's `PaymentResult`: the response without its payment (D7). */
  const paymentResult = s => {
    const o = s.options ?? {}, last = s.last ?? {}, shipping = o.requestShipping === true;
    const out = { requestId: s.id, methodName: s.method, shippingAddress: shipping ? s.address : null, shippingOption: shipping ? s.option : null,
      result: last.result ?? null, message: last.message ?? null, data: last.data == null ? null : JSON.stringify(last.data) };
    for (const [field, option] of Object.entries(REQUESTED)) out[field] = o[option] === true ? PAYER[field] : null;
    if (s.card) out[s.method === APPLE ? "apple" : "android"] = s.card;
    return out;
  };

  const end = (s, json) => { release(s.h, json); open.delete(s.h); };

  /** A sheet whose call was let go closes; the send answers nothing. */
  const dropped = (s, why) => {
    say(`payment: ${why} (${s.id}, last result ${JSON.stringify(s.last?.result ?? null)})`);
    retire(s.h);
    open.delete(s.h);
  };

  /** A synthetic payment in the method's own shape, never a card's (§9). */
  const response = s => {
    const n = ++payments, marker = JSON.stringify({ exactSubstitute: true, requestId: s.id, payment: n }), o = s.options ?? {};
    let details;
    if (s.method === APPLE) {
      const method = { displayName: "Visa 0000", network: "Visa", type: "credit" };
      details = { token: { paymentData: { exactSubstitute: true, version: "EC_v1", data: marker }, paymentMethod: method, transactionIdentifier: n.toString(16).toUpperCase().padStart(64, "0") } };
      s.card = { paymentMethod: method };
    } else {
      const google = dataOf((s.h.args?.methodData ?? []).find(m => m?.supportedMethods === s.method)) ?? {};
      const tokenizing = google.allowedPaymentMethods?.[0]?.tokenizationSpecification?.type ?? "PAYMENT_GATEWAY";
      const token = tokenizing === "DIRECT"
        ? JSON.stringify({ protocolVersion: "ECv2", signature: "exact-substitute", intermediateSigningKey: { signedKey: "{}", signatures: ["exact-substitute"] }, signedMessage: marker })
        : marker;
      details = { apiVersion: 2, apiVersionMinor: 0, paymentMethodData: { type: "CARD", description: "Visa •••• 1111", info: { cardNetwork: "VISA", cardDetails: "1111" }, tokenizationData: { type: tokenizing, token } } };
      s.card = { paymentMethodDescription: "Visa •••• 1111", cardNetwork: "VISA" };
    }
    const shipping = o.requestShipping === true;
    const out = { requestId: s.id, methodName: s.method, details, shippingAddress: shipping ? s.address : null, shippingOption: shipping ? s.option : null };
    for (const [field, option] of Object.entries(REQUESTED)) out[field] = o[option] === true ? PAYER[field] : null;
    return out;
  };

  /** `tap @t authorize|cancel`, `type @t {…}`: the agent's answer, as the person's. */
  const answer = (h, req) => {
    const s = open.get(h), cap = "payment", done = answered => ({ ticket: req.ticket, capability: cap, answered, delivery: "substituted" });
    if (!s) return { error: `@${req.ticket}: payment: no sheet is up` };
    if (req.op === "tap") {
      if (!["authorize", "cancel"].includes(req.choice)) return { error: `@${req.ticket} (${cap}): tap takes cancel | authorize, not ${req.choice}` };
      say(`device ${cap} ${req.ticket} ${req.choice === "cancel" ? "cancelled" : "answered: authorize"}`);
      if (req.choice === "cancel") {
        if (s.phase === "authorizing") return done("cancel");
        if (s.phase === "abandoned") { dropped(s, "closed after its call ended"); return done("cancel"); }
        if (s.last) end(s, paymentResult(s));
        else { say(`payment ${s.id}: cancelled`); end(s, { error: "AbortError" }); }
        return done("cancel");
      }
      if (s.asking || s.phase === "authorizing" || s.phase === "validating") return { error: `@${req.ticket}: payment: the sheet is ${s.asking ? `waiting for ${s.asking.handler}` : s.phase}` };
      if (s.phase === "abandoned") {
        say(`payment ${s.id}: authorized after its call ended; answered as failed, no handler run`);
        dropped(s, "closed after its call ended, fail");
        return done("authorize");
      }
      const r = response(s);
      s.phase = "authorizing";
      s.errors = {};
      if (!run(s, "authorize", "onpaymentauthorized", r)) result(s, "fail", "authorize-timeout", null);
      return done("authorize");
    }
    const form = `@${req.ticket}: type one of {"shippingAddress": {…}}, {"shippingOption": "<id>"} or {"paymentMethod": {…}}`;
    let v;
    try { v = JSON.parse(req.text); } catch {}
    const keys = v && typeof v === "object" && !Array.isArray(v) ? Object.keys(v) : [];
    if (keys.length !== 1) return { error: form };
    if (s.asking) return { error: `@${req.ticket}: payment: the sheet waits for ${s.asking.handler}` };
    if (s.phase === "authorizing" || s.phase === "validating") return { error: `@${req.ticket}: payment: the sheet is ${s.phase}` };
    const [key] = keys;
    let ask, keyword, arg;
    if (key === "shippingAddress" || key === "paymentMethod") {
      if (!v[key] || typeof v[key] !== "object") return { error: `@${req.ticket}: payment: \`${key}\` is an object` };
    } else if (key === "shippingOption") {
      if (!(s.details.shippingOptions ?? []).some(o => o?.id === v.shippingOption)) return { error: `@${req.ticket}: payment: no shipping option ${JSON.stringify(v.shippingOption)} is shown` };
    } else return { error: form };
    say(`device ${cap} ${req.ticket} answered: a value`);
    if (key === "shippingAddress") {
      s.address = v.shippingAddress;
      [ask, keyword, arg] = ["shippingAddress", "onshippingaddresschange", { requestId: s.id, shippingAddress: redacted(s.address), shippingOption: s.option, methodName: s.method }];
    } else if (key === "shippingOption") {
      for (const o of s.details.shippingOptions ?? []) o.selected = o.id === v.shippingOption;
      s.option = v.shippingOption;
      [ask, keyword, arg] = ["shippingOption", "onshippingoptionchange", { requestId: s.id, shippingAddress: redacted(s.address), shippingOption: s.option, methodName: s.method }];
    } else {
      [ask, keyword, arg] = ["paymentMethod", "onpaymentmethodchange", { requestId: s.id, methodName: s.method, methodDetails: v.paymentMethod }];
    }
    // A request whose call ended calls the app no more (D6).
    if (s.phase !== "abandoned") run(s, ask, keyword, arg);
    return done("value");
  };

  /** Contract sends let go (§9): held while authorizing, abandoned while retrying, else closed. */
  const letGo = () => {
    for (const s of [...open.values()]) {
      if (s.letGo || s.h.kept()) continue;
      s.letGo = true;
      if (s.phase === "authorizing") say(`payment ${s.id}: its call was let go while authorizing; held until the handler answers`);
      else if (s.phase !== "abandoned" && s.retrying) { s.phase = "abandoned"; s.asking = null; say(`payment ${s.id}: abandoned, its call was let go; the sheet stays until it is closed`); }
      else if (s.phase !== "abandoned") dropped(s, "closed, its call was let go");
    }
  };

  /** Handlers past their budget on the session's clock (D5). */
  const deadlines = () => {
    for (const s of [...open.values()]) {
      if (!s.asking || s.asking.ends > clock.now) continue;
      const asking = s.asking;
      s.asking = null;
      say(`payments: ${asking.handler} for ${s.id}: past its budget`);
      s.handled.push({ name: asking.handler, ms: clock.now - asking.started, answer: "timeout" });
      if (asking.ask === "authorize") result(s, "fail", "authorize-timeout", null);
      else if (asking.ask === "merchantValidation") end(s, { error: "OperationError", message: "merchant validation is past its budget" });
      else s.errors = { error: "unavailable" };
    }
  };

  /** The sheet's entry in `state.extensions` (§9's disclosure). */
  const entry = (h, outcome) => {
    const s = open.get(h);
    return s && { name: "showPaymentRequest", requestId: s.id, method: s.method, total: amountText(s.details.total?.amount), origin: s.origin, outcome, handlers: s.handled };
  };

  return { admit, args, opened, answer, letGo, deadlines, entry, has: h => open.has(h) };
}
