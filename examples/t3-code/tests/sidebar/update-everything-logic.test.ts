// Ported from T3 Code 9c161462c9 updateEverything.logic.test.ts (MIT; LICENSE-T3).
import { obj } from '../../src/client/domain';
type ServerSelfUpdateCapability = "boot-service" | "respawn" | "desktop-managed";
const EnvironmentId = { make: (id: string) => id };
import { describe, expect, it } from "bun:test";

import {
  collectRestartingMachines,
  collectServerUpdateTargets,
  closeFinishedUpdateRun,
  collectUpdateEverythingResult,
  describeUpdateCapsule,
  describeUpdateEverything,
  getUpdateEverythingToastView,
  resolveUpdateRunMachineStatus,
  runUpdateEverything,
  type UpdateEverythingMachine,
  type UpdateEverythingRun,
  type UpdateRunMachine,
  type UpdateRunMachineStatus,
  UPDATE_SLOW_AFTER_MS,
} from "../../src/sidebar/update-everything-logic";

const runMachine = (overrides: Partial<UpdateRunMachine> = {}): UpdateRunMachine => ({
  environmentId: EnvironmentId.make("alvin"),
  label: "alvin",
  updatesProviders: true,
  server: {
    environmentId: EnvironmentId.make("alvin"),
    serverLabel: "alvin",
    environmentKey: "https://alvin\nalvin", fromVersion: "0.0.40",
    selfUpdate: "boot-service",
    targetVersion: "0.0.50",
  },
  handedOffVersion: null,
  startedAt: 0,
  result: null,
  failure: null,
  providerFailure: null,
  ...overrides,
});

function machine(
  id: string,
  serverVersion: string,
  selfUpdate: ServerSelfUpdateCapability | undefined,
  overrides: Partial<UpdateEverythingMachine> = {},
): UpdateEverythingMachine {
  return {
    environmentId: EnvironmentId.make(id),
    key: `https://${id}\n${id}`,
    label: id,
    serverConfig: {
      environment: {
        environmentId: EnvironmentId.make(id),
        label: id,
        platform: { os: "linux", arch: "x64" },
        serverVersion,
        capabilities: {
          repositoryIdentity: true,
          ...(selfUpdate ? { serverSelfUpdate: selfUpdate } : {}),
          ...(selfUpdate === "desktop-managed" ? { desktopAppUpdate: true } : {}),
        },
      },
      settings: { continueThreadsAfterServerUpdate: false },
    },
    connected: true,
    hostedByThisApp: false,
    canMaintain: true,
    updateState: { status: "idle" },
    ...overrides,
  };
}

describe("collectServerUpdateTargets", () => {
  it("updates remote servers that can move to the target and names every skipped one", () => {
    const { targets, skipped } = collectServerUpdateTargets(
      [
        machine("this-app", "0.0.40", "desktop-managed", { hostedByThisApp: true }),
        machine("service", "0.0.40", "boot-service"),
        machine("nightly-service", "0.0.49-nightly.20261001.3", "boot-service"),
        machine("other-desktop", "0.0.49-nightly.20261001.3", "desktop-managed"),
        machine("offline", "0.0.40", "boot-service", { connected: false }),
        machine("never-connected", "0.0.40", "boot-service", { serverConfig: null }),
        machine("no-scope", "0.0.40", "boot-service", { canMaintain: false }),
        machine("busy", "0.0.40", "boot-service", {
          updateState: {
            status: "running",
            stage: "downloading",
            fromVersion: "0.0.40",
            targetVersion: "0.0.50",
          },
        }),
        machine("current", "0.0.50", "boot-service"),
        machine("current-offline", "0.0.50", "boot-service", { connected: false }),
        machine("manual", "0.0.40", undefined),
      ],
      "0.0.50",
    );

    expect(targets.map((target) => [target.serverLabel, target.targetVersion])).toEqual([
      ["service", "0.0.50"],
      ["other-desktop", "0.0.50"],
    ]);
    expect(skipped.map((entry) => [entry.label, entry.reason])).toEqual([
      ["nightly-service", "on the nightly channel"],
      ["offline", "not connected"],
      ["never-connected", "not connected"],
      ["no-scope", "no permission"],
      ["busy", "already updating"],
      ["manual", "needs a manual update"],
    ]);
  });
});

describe("collectRestartingMachines", () => {
  it("lists updated servers, and this app's servers only when it installs", () => {
    const withContinuation = (
      id: string,
      overrides: Partial<UpdateEverythingMachine> = {},
      enabled = false,
    ) => {
      const base = machine(id, "0.0.40", "boot-service", overrides);
      return {
        ...base,
        serverConfig: {
          environment: {
            ...obj(base.serverConfig!.environment),
            capabilities: {
              ...obj(obj(base.serverConfig!.environment).capabilities),
              threadRestartContinuation: true,
            },
          },
          settings: { continueThreadsAfterServerUpdate: enabled },
        },
      };
    };
    const machines = [
      withContinuation("this-app", { hostedByThisApp: true }, true),
      withContinuation("alvin", {}, true),
      withContinuation("opted-out", {}, false),
      machine("old", "0.0.40", "boot-service"),
      withContinuation("providers-only"),
    ];
    const servers = ["alvin", "opted-out", "old"].map((id) => ({
      environmentId: EnvironmentId.make(id),
    }));

    expect(collectRestartingMachines(machines, servers, true)).toEqual([
      { environmentId: "this-app", continuesThreads: true },
      { environmentId: "alvin", continuesThreads: true },
      { environmentId: "opted-out", continuesThreads: false },
      { environmentId: "old", continuesThreads: false },
    ]);
    expect(
      collectRestartingMachines(machines, servers, false).map((entry) => entry.environmentId),
    ).toEqual(["alvin", "opted-out", "old"]);
  });
});

describe("runUpdateEverything", () => {
  it("restarts servers after providers finish and returns every result", async () => {
    const calls: string[] = [];
    let finishProviders = (_value: string) => {};
    const run = runUpdateEverything({
      downloadLocal: async () => {
        calls.push("download");
        return true;
      },
      updateProviders: () =>
        new Promise<string>((resolve) => {
          calls.push("providers");
          finishProviders = resolve;
        }),
      updateServers: async () => {
        calls.push("servers");
        return 2;
      },
    });

    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(calls).toEqual(["download", "providers"]);
    finishProviders("provider results");
    expect(await run).toEqual({ localReady: true, providers: "provider results", servers: 2 });
    expect(calls).toEqual(["download", "providers", "servers"]);
  });

  it("still updates remote machines when the local download fails", async () => {
    const calls: string[] = [];
    const result = await runUpdateEverything({
      downloadLocal: async () => false,
      updateProviders: async () => {
        calls.push("providers");
      },
      updateServers: async () => {
        calls.push("servers");
      },
    });

    expect(calls).toEqual(["providers", "servers"]);
    expect(result.localReady).toBe(false);
  });
});

describe("resolveUpdateRunMachineStatus", () => {
  const running = (stage: "downloading" | "installing" | "resuming") =>
    ({ status: "running", stage, fromVersion: "0.0.40", targetVersion: "0.0.50" }) as const;

  it("follows the run phase, then the server's own update stage", () => {
    const idle = { status: "idle" } as const;
    expect(resolveUpdateRunMachineStatus(runMachine(), "providers", idle)).toBe("providers");
    expect(resolveUpdateRunMachineStatus(runMachine(), "servers", idle)).toBe("waiting");
    expect(resolveUpdateRunMachineStatus(runMachine(), "servers", running("installing"))).toBe(
      "downloading",
    );
    expect(resolveUpdateRunMachineStatus(runMachine(), "servers", running("resuming"))).toBe(
      "restarting",
    );
    expect(
      resolveUpdateRunMachineStatus(
        runMachine({ updatesProviders: false }),
        "providers",
        running("downloading"),
      ),
    ).toBe("downloading");
    expect(resolveUpdateRunMachineStatus(runMachine({ result: "failed" }), "servers", idle)).toBe(
      "failed",
    );
    expect(
      resolveUpdateRunMachineStatus(
        runMachine({ server: null, handedOffVersion: "0.0.50" }),
        "servers",
        idle,
      ),
    ).toBe("restarting");
  });
});

describe("update capsule", () => {
  const run = (overrides: Partial<UpdateEverythingRun> = {}): UpdateEverythingRun => ({
    localVersion: null,
    phase: "servers",
    restartAt: null,
    machines: [],
    ...overrides,
  });
  const view = (
    label: string,
    status: UpdateRunMachineStatus,
    overrides: Partial<UpdateRunMachine> = {},
  ) => ({
    machine: runMachine({ environmentId: EnvironmentId.make(label), label, ...overrides }),
    status,
  });

  it("counts down this app's restart before anything else", () => {
    expect(
      describeUpdateCapsule(
        run({ phase: "countdown", restartAt: 5_000, localVersion: "0.0.50" }),
        [view("alvin", "restarting")],
        { status: "waiting" },
        1_200,
      ),
    ).toEqual({ tone: "default", label: "Restart in 4s", action: "later" });
  });

  it("names the slowest machine once it passes the slow threshold", () => {
    const machines = [
      view("bb-1", "done"),
      view("zbook", "downloading", { startedAt: 60_000 }),
      view("alvin", "restarting", { startedAt: 0 }),
    ];
    expect(describeUpdateCapsule(run(), machines, null, 60_000)).toEqual({
      tone: "default",
      label: "1 of 3 updated",
      action: null,
    });
    expect(describeUpdateCapsule(run(), machines, null, UPDATE_SLOW_AFTER_MS + 6 * 60_000)).toEqual(
      {
        tone: "slow",
        label: "alvin",
        slow: { status: "restarting", elapsed: "9m" },
        action: null,
      },
    );
  });

  it("offers a retry only for failed servers it can update again", () => {
    expect(describeUpdateCapsule(run(), [view("alvin", "failed")], null, 0)).toEqual({
      tone: "failed",
      label: "alvin failed",
      action: "retry",
    });
    expect(
      describeUpdateCapsule(
        run(),
        [view("alvin", "failed", { server: null }), view("box", "failed", { server: null })],
        null,
        0,
      ),
    ).toEqual({ tone: "failed", label: "2 failed", action: "dismiss" });
  });

  it("ends the run only when every machine is done and this app is not installing", () => {
    const done = runMachine({ result: "done" });
    expect(closeFinishedUpdateRun(run({ machines: [done] }))).toBeNull();
    expect(
      closeFinishedUpdateRun(run({ machines: [done], localVersion: "0.0.50" })),
    ).not.toBeNull();
    expect(
      closeFinishedUpdateRun(run({ machines: [runMachine({ result: "failed" })] })),
    ).not.toBeNull();
    expect(closeFinishedUpdateRun(run({ machines: [done], phase: "providers" }))).not.toBeNull();
  });
});

describe("update everything summary", () => {
  const result = collectUpdateEverythingResult({
    localVersion: "0.0.50",
    localFailures: ["Could not download update: Network error."],
    servers: [
      { label: "alvin", outcome: { status: "failed", message: "Download failed." } },
      { label: "box", outcome: { status: "updated" } },
      { label: "studio", outcome: { status: "updated" } },
      { label: "busy", outcome: { status: "skipped" } },
    ],
    providers: [
      { status: "updated" },
      { status: "interrupted" },
      { status: "failed", line: "box · Codex: npm exited with code 1." },
    ],
  });

  it("counts updates and names the machine of each failure", () => {
    expect(result).toEqual({
      localVersion: "0.0.50",
      updatedServers: 2,
      updatedProviders: 1,
      restarting: [],
      failures: [
        "This app: Could not download update: Network error.",
        "alvin: Download failed.",
        "box · Codex: npm exited with code 1.",
      ],
    });
  });

  it("includes this app only when the relaunch landed on its version", () => {
    expect(getUpdateEverythingToastView(result, "0.0.50")).toEqual({
      type: "error",
      title: "Updated this app, 2 servers, and 1 provider",
      description: [
        "This app: Could not download update: Network error.",
        "alvin: Download failed.",
        "box · Codex: npm exited with code 1.",
      ].join("\n"),
    });
    expect(getUpdateEverythingToastView({ ...result, failures: [] }, "0.0.40")).toEqual({
      type: "success",
      title: "Updated 2 servers and 1 provider",
    });
  });

  it("reports a failure when nothing updated, and nothing when nothing finished", () => {
    const empty = { localVersion: null, updatedServers: 0, updatedProviders: 0, restarting: [] };
    expect(
      getUpdateEverythingToastView({ ...empty, failures: ["alvin: offline"] }, "0.0.50"),
    ).toEqual({ type: "error", title: "Update failed", description: "alvin: offline" });
    expect(getUpdateEverythingToastView({ ...empty, failures: [] }, "0.0.50")).toBeNull();
  });

  it("names servers still restarting when this app relaunched", () => {
    expect(
      getUpdateEverythingToastView(
        { ...result, failures: [], restarting: ["alvin", "zbook"] },
        "0.0.50",
      ),
    ).toEqual({
      type: "success",
      title: "Updated this app, 2 servers, and 1 provider",
      description: "alvin and zbook are still restarting.",
    });
  });
});

describe("collectServerUpdateTargets npm wait", () => {
  it("waits for npm only for hosts that install from it or when the release may lack archives", () => {
    const hosts = [
      machine("archive-service", "0.1.4", "boot-service"),
      machine("npm-service", "0.0.40", "boot-service"),
      machine("legacy-respawn", "0.1.4", "respawn"),
      machine("desktop", "0.1.4", "desktop-managed"),
    ];
    const waits = (releaseArchivesPublished: boolean) =>
      collectServerUpdateTargets(hosts, "0.1.5", releaseArchivesPublished).waitForNpm.map(
        (target) => target.serverLabel,
      );

    expect(waits(true)).toEqual(["npm-service", "legacy-respawn"]);
    // This app has no desktop update feed, so its archives never count as published.
    expect(waits(false)).toEqual(["archive-service", "npm-service", "legacy-respawn"]);
  });
});

describe("describeUpdateEverything", () => {
  it("lists one row per machine: this app's machines first, then servers, provider-only, and skipped", () => {
    const alvin = EnvironmentId.make("alvin");
    expect(
      describeUpdateEverything({
        localVersion: "0.0.50",
        localMachines: [{ environmentId: EnvironmentId.make("local"), label: "This Mac" }],
        servers: [
          { environmentId: alvin, serverLabel: "alvin", selfUpdate: "boot-service", targetVersion: "0.0.50" },
          { environmentId: EnvironmentId.make("studio"), serverLabel: "studio", selfUpdate: "desktop-managed", targetVersion: "0.0.50" },
        ],
        providerMachines: [
          { environmentId: alvin, label: "alvin", providers: ["Codex", "Claude"] },
          { environmentId: EnvironmentId.make("box"), label: "box", providers: ["Cursor"] },
        ],
        skipped: [{ environmentId: EnvironmentId.make("old"), label: "old", reason: "needs a manual update" }],
      }),
    ).toEqual({
      summary: "Update this app, 2 servers, and 3 providers",
      version: "0.0.50",
      providerCount: 3,
      rows: [
        { environmentId: "local", label: "This Mac", app: "this-app", skipped: null, providers: [] },
        { environmentId: "alvin", label: "alvin", app: "server", skipped: null, providers: ["Codex", "Claude"] },
        { environmentId: "studio", label: "studio", app: "desktop-app", skipped: null, providers: [] },
        { environmentId: "box", label: "box", app: null, skipped: null, providers: ["Cursor"] },
        { environmentId: "old", label: "old", app: null, skipped: "needs a manual update", providers: [] },
      ],
    });
  });

  it("reads naturally with one kind of update; the version is the first npm server's without this app", () => {
    const view = describeUpdateEverything({
      localVersion: null,
      localMachines: [],
      servers: [{ environmentId: EnvironmentId.make("alvin"), serverLabel: "alvin", selfUpdate: "boot-service", targetVersion: "0.1.5" }],
      providerMachines: [],
      skipped: [],
    });
    expect([view.summary, view.version, view.rows.map((row) => row.app)]).toEqual(["Update 1 server", "0.1.5", ["server"]]);
    expect(
      describeUpdateEverything({
        localVersion: null,
        localMachines: [],
        servers: [],
        providerMachines: [
          { environmentId: EnvironmentId.make("alvin"), label: "alvin", providers: ["Codex"] },
        ],
        skipped: [],
      }).summary,
    ).toBe("Update 1 provider");
  });
});
