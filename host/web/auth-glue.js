// `openAuthSession` on the web (LLP 1069.006 D4): a popup to the
// authorization URL, opened inside the press's own call stack, and the
// app's static callback page (`/.exact/auth/callback`, `auth-callback.js`)
// returning on two channels — `opener.postMessage` and a same-origin
// `BroadcastChannel`, the one that survives a provider's
// Cross-Origin-Opener-Policy. The runner rules (grants, one session,
// supersession) and checks the callback (`exact_runner::auth::accept`); this
// only opens, listens and reports. A closed popup alone is not a
// cancellation: 499 only after `closed` held for a grace period with no
// message, or at the bound. No redirect fallback: a blocked popup is 428.
// Loaded after first paint when the app grants `auth.session`.
const GRACE_MS = 3000;           // `closed` true this long, and no message: cancelled
const BOUND_MS = 10 * 60 * 1000; // no callback within this: cancelled (D4 item 4)
const POLL_MS = 250;
let live = null; // { ticket, popup, done, timers }

function finish(env, report) {
  if (!live) return;
  const { ticket, popup, channel, onMessage, poll, bound } = live;
  live = null;
  clearInterval(poll); clearTimeout(bound);
  removeEventListener('message', onMessage);
  channel?.close();
  if (report) { env.call({ op: 'done', ticket, ...report }); env.deliver(ticket); }
  else { try { popup?.close(); } catch {} } // superseded: the runner let go
}

/** Arm an `{"op":"auth","ticket"}` batch op, synchronously. `env`: `call`
 * (the runner's word, `exact_auth`), `deliver(ticket)` (fulfil what the
 * runner settled), `active(ticket)`, `agent`, `log`. */
export function arm(op, env) {
  const activated = navigator.userActivation ? navigator.userActivation.isActive : true;
  const reply = env.call({ op: 'arm', ticket: op.ticket, agent: env.agent, origin: location.origin, popup: activated });
  if (reply.error) { env.log(`auth ${op.ticket}: ${reply.error}`); return; }
  if (reply.settled) { env.deliver(op.ticket); return; }
  if (!reply.present) return; // held for the agent (D7)
  // A new session supersedes this window's live one (the runner already let
  // its ticket go, or refused this one with 409).
  if (live) finish(env, null);
  const name = 'exact-auth-' + Array.from(crypto.getRandomValues(new Uint8Array(12)), b => b.toString(16).padStart(2, '0')).join('');
  const popup = window.open(reply.present.url, name, 'popup,width=520,height=680');
  if (!popup) {
    env.call({ op: 'done', ticket: op.ticket, status: 428, message: 'popup blocked' });
    env.deliver(op.ticket);
    return;
  }
  const want = new URL(reply.present.callback);
  const matches = url => {
    try { const u = new URL(url); return u.origin === want.origin && u.pathname === want.pathname; } catch { return false; }
  };
  const accept = (url, via) => {
    if (!live || !matches(url)) return;
    env.log(`auth ${op.ticket}: a callback arrived (${via})`);
    finish(env, { url });
  };
  const onMessage = event => {
    // The opener path: the app's own origin, from the popup it owns.
    if (event.origin !== location.origin || event.data?.type !== 'exact-auth') return;
    if (event.source && event.source !== popup) return;
    accept(String(event.data.url), 'opener');
  };
  let channel = null;
  try {
    channel = new BroadcastChannel('exact-auth');
    channel.onmessage = event => { if (event.data?.type === 'exact-auth') accept(String(event.data.url), 'channel'); };
  } catch {}
  addEventListener('message', onMessage);
  let closedSince = null;
  const poll = setInterval(() => {
    if (!live) return;
    if (!env.active(op.ticket)) { finish(env, null); return; } // superseded or forgotten (D3)
    let closed = false;
    try { closed = popup.closed; } catch {}
    if (!closed) { closedSince = null; return; }
    closedSince ??= Date.now();
    if (Date.now() - closedSince >= GRACE_MS) finish(env, { status: 499, message: 'cancelled' });
  }, POLL_MS);
  const bound = setTimeout(() => finish(env, { status: 499, message: 'cancelled' }), BOUND_MS);
  live = { ticket: op.ticket, popup, channel, onMessage, poll, bound };
}

globalThis.exact ??= {};
globalThis.exact.authHost = { arm };
