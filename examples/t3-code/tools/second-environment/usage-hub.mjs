#!/usr/bin/env bun
// A CLIProxyAPI hub stand-in, so the Usage page has pooled limits to draw. It answers the part of
// the hub's management API that a T3 server's usage limit source reads (reference
// apps/server/src/usage/cliproxyApi.ts): `auth-files`, `api-call` for the Codex usage, reset-credit
// and consume URLs and Claude's OAuth usage URL, and `reset-quota`. Accounts and windows are fixed
// below; a consumed credit empties that account's session window. It contacts nothing.
//
//   bun usage-hub.mjs <port> <management key>

const HOUR = 3600, DAY = 24 * HOUR;

/** Three Codex accounts and two Claude ones: each pool's windows draw one segment per account. */
export const HUB_ACCOUNTS = [
  { id: "alex.json", provider: "codex", email: "alex@t3-fixture.invalid", plan: "plus", session: 35, weekly: 62, sessionResetIn: 2 * HOUR, weeklyResetIn: 3 * DAY, credits: 2 },
  { id: "sam.json", provider: "codex", email: "sam@t3-fixture.invalid", plan: "pro", session: 80, weekly: 41, sessionResetIn: 40 * 60, weeklyResetIn: 5 * DAY, credits: 0 },
  { id: "jo.json", provider: "codex", email: "jo@t3-fixture.invalid", plan: "team", session: 12, weekly: 90, sessionResetIn: 4 * HOUR, weeklyResetIn: DAY, credits: 1 },
  { id: "kim.json", provider: "claude", email: "kim@t3-fixture.invalid", session: 55, weekly: 30, sessionResetIn: 3 * HOUR, weeklyResetIn: 4 * DAY },
  { id: "lee.json", provider: "claude", email: "lee@t3-fixture.invalid", session: 5, weekly: 77, sessionResetIn: HOUR, weeklyResetIn: 2 * DAY },
].map((account, index) => ({ ...account, authIndex: `lane-${index + 1}` }));

/** The hub's answers, as a function of the request: (method, path, body) → [status, json]. */
export function makeHub(accounts = HUB_ACCOUNTS, now = () => Date.now()) {
  const state = new Map(accounts.map((account) => [account.authIndex, {
    ...account,
    creditIds: Array.from({ length: account.credits ?? 0 }, (_, n) => `${account.authIndex}-credit-${n + 1}`),
  }]));
  const at = (offsetSeconds) => Math.floor(now() / 1000) + offsetSeconds;
  const iso = (offsetSeconds) => new Date(at(offsetSeconds) * 1000).toISOString();
  const providerAnswer = (account, url) => {
    if (account.provider === "claude" && url === "https://api.anthropic.com/api/oauth/usage")
      return { five_hour: { utilization: account.session, resets_at: iso(account.sessionResetIn) }, seven_day: { utilization: account.weekly, resets_at: iso(account.weeklyResetIn) } };
    if (account.provider !== "codex") return null;
    if (url.endsWith("/wham/usage")) return { plan_type: account.plan, rate_limit: {
      primary_window: { used_percent: account.session, reset_at: at(account.sessionResetIn), limit_window_seconds: 5 * HOUR },
      secondary_window: { used_percent: account.weekly, reset_at: at(account.weeklyResetIn), limit_window_seconds: 7 * DAY },
    } };
    if (url.endsWith("/wham/rate-limit-reset-credits"))
      return { credits: account.creditIds.map((id, n) => ({ id, status: "available", reset_type: "codex_rate_limits", expires_at: iso((n + 10) * DAY) })) };
    return null;
  };
  return (method, path, body) => {
    if (method === "GET" && path === "/v0/management/auth-files") return [200, { files: [...state.values()].map((account) => ({
      id: account.id, auth_index: account.authIndex, provider: account.provider, email: account.email,
      ...(account.provider === "codex" ? { id_token: { chatgpt_account_id: `acct-${account.authIndex}`, chatgpt_plan_type: account.plan } } : {}),
    })) }];
    if (method === "POST" && path === "/v0/management/reset-quota") return state.has(body?.auth_index) ? [200, { status: "ok" }] : [404, { error: "unknown auth_index" }];
    if (method !== "POST" || path !== "/v0/management/api-call") return [404, { error: "not found" }];
    const account = state.get(body?.auth_index);
    if (!account) return [200, { status_code: 404, body: "{}" }];
    const url = String(body.url ?? "");
    if (url.endsWith("/wham/rate-limit-reset-credits/consume") && account.provider === "codex") {
      const { credit_id: creditId } = JSON.parse(body.data ?? "{}");
      if (!account.creditIds.includes(creditId)) return [200, { status_code: 200, body: JSON.stringify({ code: "no_credit" }) }];
      account.creditIds = account.creditIds.filter((id) => id !== creditId);
      account.session = 0;
      return [200, { status_code: 200, body: JSON.stringify({ code: "reset" }) }];
    }
    const answer = providerAnswer(account, url);
    return [200, answer ? { status_code: 200, body: JSON.stringify(answer) } : { status_code: 404, body: "{}" }];
  };
}

if (import.meta.main) {
  const port = Number(process.argv[2]), key = process.argv[3];
  if (!Number.isInteger(port) || !key) throw new Error("usage: bun usage-hub.mjs <port> <management key>");
  const hub = makeHub();
  Bun.serve({ hostname: "127.0.0.1", port, async fetch(request) {
    if (request.headers.get("authorization") !== `Bearer ${key}`) return Response.json({ error: "unauthorized" }, { status: 401 });
    const body = request.method === "POST" ? await request.json().catch(() => ({})) : undefined;
    const [status, json] = hub(request.method, new URL(request.url).pathname, body);
    return Response.json(json, { status });
  } });
  console.log(`usage hub stand-in on 127.0.0.1:${port}`);
}
