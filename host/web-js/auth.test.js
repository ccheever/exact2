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
