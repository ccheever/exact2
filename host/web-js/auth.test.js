import { test, expect, mock } from "bun:test";

const clock = { now: 0, agent: true };
const journal = [];
let ticket = 0;
mock.module("./rt.js", () => ({ clock, journal, nextTicket: () => ++ticket }));
const { install, holds, answer } = await import("./auth.js");

test("authorization URL userinfo is refused without disclosure", async () => {
  globalThis.location = { origin: "https://app.example" };
  clock.agent = true;
  const grants = "auth.session https://*.example.com\nauth.callback https://app.example/.exact/auth/callback";
  install(grants, () => "login");
  const before = journal.length;
  const url = "https://user:review-secret@login.example.com/authorize?code=hidden";
  const pending = openAuthSession(url, { callback: authCallback(), state: "s" });
  for (const hold of holds()) answer({ ticket: hold.ticket, op: "tap", choice: "cancel" });
  const error = await pending.catch(e => e);
  expect(error).toBeInstanceOf(Error);
  expect(error.message).toContain("userinfo");
  expect(error.message).not.toContain("review-secret");
  expect(JSON.stringify(holds())).not.toContain("review-secret");
  expect(journal.slice(before).join("\n")).not.toContain("review-secret");
});

test("callback userinfo is refused without disclosure", async () => {
  globalThis.location = { origin: "https://app.example" };
  clock.agent = true;
  install("auth.session https://login.example.com", () => "login");
  const pending = openAuthSession("https://login.example.com/authorize", { callback: "https://user:review-secret@app.example/cb", state: "s" });
  for (const hold of holds()) answer({ ticket: hold.ticket, op: "tap", choice: "cancel" });
  const error = await pending.catch(e => e);
  expect(error.message).toContain("userinfo");
  expect(error.message).not.toContain("review-secret");
});

test("single-slash https callback cannot disclose userinfo", async () => {
  globalThis.location = { origin: "https://app.example" };
  clock.agent = true;
  install("auth.session https://login.example.com", () => "login");
  const pending = openAuthSession("https://login.example.com/authorize", { callback: "https:/user:review-secret@app.example/cb", state: "s" });
  for (const hold of holds()) answer({ ticket: hold.ticket, op: "tap", choice: "cancel" });
  const error = await pending.catch(e => e);
  expect(error.status).toBe(502);
  expect(error.message).not.toContain("review-secret");
});

test("backslashed or tabbed https callbacks cannot disclose userinfo", async () => {
  for (const callback of ["https:\\/user:review-secret@app.example/cb", "https://\\/user:review-secret@app.example/cb", "https://app.example/c\tb@user:review-secret"]) {
    globalThis.location = { origin: "https://app.example" };
    clock.agent = true;
    install("auth.session https://login.example.com", () => "login");
    const pending = openAuthSession("https://login.example.com/authorize", { callback, state: "s" });
    for (const hold of holds()) answer({ ticket: hold.ticket, op: "tap", choice: "cancel" });
    const error = await pending.catch(e => e);
    expect(error.status).toBe(502);
    expect(error.message).not.toContain("review-secret");
  }
});

test("ordinary exact-origin sessions still reach a held callback", async () => {
  globalThis.location = { origin: "https://app.example" };
  clock.agent = true;
  install("auth.session https://login.example.com\nauth.callback https://app.example/.exact/auth/callback", () => "login");
  const pending = openAuthSession("https://login.example.com/authorize?code=hidden", { callback: authCallback(), state: "s" });
  const [hold] = holds();
  expect(hold.device.args.origin).toBe("https://login.example.com");
  const callback = "https://app.example/.exact/auth/callback?state=s&code=returned";
  answer({ ticket: hold.ticket, op: "type", text: callback });
  expect(await pending).toBe(callback);
});

test("a loopback callback answers as the runner's arm does on the web (LLP 1133.005 D9)", async () => {
  globalThis.location = { origin: "http://localhost:8080" };
  clock.agent = true;
  install("auth.session https://login.example.com\nauth.callback http://127.0.0.1/oauth/loopback", () => "login");
  const failed = (callback) => openAuthSession("https://login.example.com/authorize", { callback, state: "s" }).catch(e => e);
  // Granted at any port, then 501: a page cannot listen; the scheme in any case, as the runner reads it.
  for (const callback of ["http://127.0.0.1:4332/oauth/loopback", "HTTP://127.0.0.1:4332/oauth/loopback"]) {
    const loopback = await failed(callback);
    expect([loopback.status, loopback.message]).toEqual([501, "a page cannot listen"]);
  }
  // Not granted, or not D9's form (a port `Host` would not carry as written): the grants refuse it first.
  for (const callback of ["http://127.0.0.1:4332/other", "http://127.0.0.1:80/oauth/loopback", "http://127.0.0.1:04332/oauth/loopback", "http://localhost:4332/oauth/loopback"])
    expect((await failed(callback)).status).toBe(403);
  // The page's own callback on a loopback development origin is held as ever.
  const page = openAuthSession("https://login.example.com/authorize", { callback: authCallback(), state: "s" });
  expect(holds().length).toBe(1);
  for (const hold of holds()) answer({ ticket: hold.ticket, op: "tap", choice: "cancel" });
  expect((await page.catch(e => e)).status).toBe(499);
});

test("a state outside the BMP is matched whole, as auth.rs decodes it", async () => {
  globalThis.location = { origin: "https://app.example" };
  clock.agent = true;
  install("auth.session https://login.example.com\nauth.callback https://app.example/cb", () => "login");
  const pending = openAuthSession("https://login.example.com/authorize", { callback: "https://app.example/cb", state: "s😀" });
  const [hold] = holds();
  answer({ ticket: hold.ticket, op: "type", text: "https://app.example/cb?code=c&state=s😀" });
  expect(await pending).toBe("https://app.example/cb?code=c&state=s😀");
});
