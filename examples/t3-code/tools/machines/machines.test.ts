// The machines fixture's shape: the environments it pairs exist, sit in the fixture's port range
// and share one logical project, so the reference's machines list (the primary, then every
// switched-on saved environment; two or more) has three rows and the Run on menu two machines.
import { expect, test } from "bun:test";
import { ENVIRONMENTS, SEEDS, checkPort } from "../second-environment/lib.mjs";
import { MACHINES, OPEN_CONNECTIONS } from "./machines.mjs";

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
