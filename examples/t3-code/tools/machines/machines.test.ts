// The machines fixture's shape: the environments it pairs exist, sit in the fixture's port range
// and share one logical project, so the reference's machines list (the primary, then every
// switched-on saved environment; two or more) has three rows and the Run on menu two machines.
import { expect, test } from "bun:test";
import { ENVIRONMENTS, SEEDS, checkPort } from "../second-environment/lib.mjs";
import { MANY_MACHINES, MACHINES, OLDER, OPEN_CONNECTIONS } from "./machines.mjs";
import { SELF_UPDATE, rewrite, updateScript } from "./older-server.mjs";
import { CLIENT_VERSION } from "../../src/connections/version-skew";

test("two distinct environments in the fixture's ports, both holding shared-notes under one origin", () => {
  expect(new Set(MACHINES).size).toBe(2);
  for (const name of MACHINES) expect(checkPort(ENVIRONMENTS[name as keyof typeof ENVIRONMENTS].port, {})).toBeNumber();
  const origins = MACHINES.map((name) => SEEDS[name as keyof typeof SEEDS].find((seed) => seed.repo === "shared-notes")?.origin);
  expect(origins[0]).toBeString();
  expect(origins[1]).toBe(origins[0]);
});

test("the drive opens Settings › Connections once data has landed", () => {
  expect(OPEN_CONNECTIONS).toEqual(["tap connection-settings", "clock settle", "tap settings-connections", "clock data", "clock settle"]);
});

test("the older-server stand-in: every environment descriptor reads older with self-update; the update streams as the reference's", () => {
  expect(OLDER.version < CLIENT_VERSION).toBe(true);
  // A stable version, the client's channel, so the update dialog offers it rather than skipping it.
  expect([OLDER.version, CLIENT_VERSION].map((version) => version.includes("-"))).toEqual([false, false]);
  const descriptor = { environmentId: "e1", serverVersion: "0.0.46-nightly.20261005.2667", capabilities: { pullRequests: true } };
  expect(rewrite({ _tag: "Exit", exit: { value: { environment: descriptor, providers: [{ serverVersion: "kept" }] } } }, OLDER.version)).toEqual(
    { _tag: "Exit", exit: { value: { environment: { ...descriptor, serverVersion: OLDER.version, capabilities: { pullRequests: true, ...SELF_UPDATE } }, providers: [{ serverVersion: "kept" }] } } });
  const timing = { target: CLIENT_VERSION, download: 6000, install: 3000 };
  const frames = updateScript({ _tag: "Request", id: "1", tag: "server.updateServerWithProgress" }, timing);
  expect(frames.map(([delay, frame]) => [delay, frame._tag === "Chunk" ? (frame.values[0] as { type: string; stage?: string }).stage ?? frame.values[0].type : frame._tag])).toEqual(
    [[0, "downloading"], [6000, "installing"], [3000, "complete"], [0, "Exit"]]);
  expect(updateScript({ _tag: "Request", id: "2", tag: "server.updateServer" }, timing)).toEqual(
    [[9000, { _tag: "Exit", requestId: "2", exit: { _tag: "Success", value: { targetVersion: CLIENT_VERSION, method: "boot-service" } } }]]);
});

// A duplicate friendly name is deliberate: addresses must distinguish both Studio entries.
test("many-machine fixture has eight distinct machine ports and duplicate-name coverage", () => {
  expect(MANY_MACHINES).toHaveLength(7);
  expect(new Set(MANY_MACHINES.map(name => ENVIRONMENTS[name as keyof typeof ENVIRONMENTS].port)).size).toBe(7);
  expect(MANY_MACHINES.map(name => ENVIRONMENTS[name as keyof typeof ENVIRONMENTS].label).filter(label => label === 'Lane Studio')).toHaveLength(2);
});
