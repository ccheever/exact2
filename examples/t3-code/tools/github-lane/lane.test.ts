// The GitHub lane's own pieces, offline: the gh wrapper's refusal and pass-through, the lane
// server's environment, the login shell's PATH, and the seed's deterministic history and state
// rules. Nothing here reaches GitHub: the "real gh" is a stub that echoes what it was given.
import { afterAll, describe, expect, test } from "bun:test";
import { chmodSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { ghCallsSince, ghWrapper, lanePaths, run, serverEnv, setup, signedIn, toolEnv } from "./lib.mjs";
import { BULK, DESCRIPTION, HISTORY, LABELS, Seed, bulkBranch, branchOf, generationOf, inState, scenarios } from "./seed.mjs";
import { table, verb } from "./probe.mjs";

const root = mkdtempSync(join(tmpdir(), "github-lane-test-"));
afterAll(() => rmSync(root, { recursive: true, force: true }));
const stub = join(root, "real-gh");
writeFileSync(stub, '#!/bin/sh\nprintf "config=%s args=%s stdin=%s\\n" "$GH_CONFIG_DIR" "$*" "$(cat)"\n');
chmodSync(stub, 0o755);
const env = { T3_GITHUB_LANE: join(root, "lane"), T3_GITHUB_LANE_SHARED: join(root, "shared"), T3_GITHUB_LANE_GH: stub, USER: "lane" };
const paths = setup(lanePaths(env), env);

describe("the lane gh", () => {
  test("refuses without the account's own token, so nothing falls through to the keyring, and logs the attempt", () => {
    const refused = run(join(paths.bins.primary, "gh"), ["api", "user"], { env: toolEnv(paths, "primary"), allowFail: true, input: "" });
    expect(refused.status).toBe(4);
    expect(refused.stderr).toContain("has no token of its own");
    expect(signedIn(paths, "primary")).toBe(false);
    expect(ghCallsSince(paths, 0).at(-1)).toMatchObject({ config: paths.configs.primary, tokenFromEnv: false, argv: "api user" });
  });
  test("with a token in its config dir it runs the real CLI there, arguments and stdin unchanged", () => {
    writeFileSync(join(paths.configs.second, "hosts.yml"), "github.com:\n    users:\n        lane-second:\n            oauth_token: not-a-real-token\n    oauth_token: not-a-real-token\n    user: lane-second\n");
    expect(signedIn(paths, "second")).toBe(true);
    const ran = run(join(paths.bins.second, "gh"), ["api", "graphql", "-f", "query=query{viewer{login}}"], { env: toolEnv(paths, "second"), input: "{\"x\":1}" });
    expect(ran.stdout.trim()).toBe(`config=${paths.configs.second} args=api graphql -f query=query{viewer{login}} stdin={"x":1}`);
    const logged = readFileSync(paths.ghLog, "utf8");
    expect(logged).not.toContain("not-a-real-token");
  });
  test("the wrapper names its config dir and real CLI with shell quoting", () => {
    const text = ghWrapper({ real: "/opt/it's/gh", config: "/c d/gh", log: "/l/log" });
    expect(text).toContain("export GH_CONFIG_DIR='/c d/gh'");
    expect(text).toContain("exec '/opt/it'\\''s/gh' \"$@\"");
  });
});

describe("the lane server's environment", () => {
  test("is built from nothing: no token or config from this shell, its own homes, telemetry off", () => {
    const built = serverEnv(paths, "primary", { USER: "u", GH_TOKEN: "leak", GITHUB_TOKEN: "leak", HOME: "/Users/u", GH_CONFIG_DIR: "/Users/u/.config/gh" });
    expect(Object.values(built)).not.toContain("leak");
    expect(built).toMatchObject({ HOME: paths.homes.primary, GH_CONFIG_DIR: paths.configs.primary, T3CODE_TELEMETRY_ENABLED: "false", GIT_CONFIG_NOSYSTEM: "1" });
    expect(built.PATH.split(":")[0]).toBe(paths.bins.primary);
    for (const key of ["CODEX_HOME", "CLAUDE_CONFIG_DIR", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME", "XDG_STATE_HOME", "T3CODE_HOME", "TMPDIR"]) expect(built[key]!.startsWith(paths.lane)).toBe(true);
  });
  test("the login shell the server rebuilds PATH from finds the lane gh and its config dir first", () => {
    const shell = run("/bin/zsh", ["-ilc", 'command -v gh; printf "%s\\n" "$GH_CONFIG_DIR"'], { env: serverEnv(paths, "second", { USER: "u" }) }).stdout.trim().split("\n");
    expect(shell.slice(-2)).toEqual([join(paths.bins.second, "gh"), paths.configs.second]);
    expect(readFileSync(join(paths.homes.second, ".gitconfig"), "utf8")).toContain(`helper = !'${join(paths.bins.second, "gh")}' auth git-credential`);
  });
});

describe("the seed", () => {
  test("builds the same base history in any checkout (fixed contents, author and dates)", () => {
    const shas = [1, 2].map((n) => {
      const lane = { ...env, T3_GITHUB_LANE: join(root, `seed-${n}`) };
      const seed = new Seed({ paths: setup(lanePaths(lane), lane), log: () => {} });
      Object.assign(seed, { owner: "lane-owner", ownerId: 42, repo: "lane-owner/sandbox", name: "sandbox" });
      seed.ensureWork({ fetch: false });
      let parent: string | null = null;
      HISTORY.forEach(([, message, files], i) => { parent = seed.commit({ parent, files, message, at: Date.UTC(2026, 9, 1, 9 + i) }); });
      return parent;
    });
    expect(shas[0]).toMatch(/^[0-9a-f]{40}$/);
    expect(shas[0]).toBe(shas[1]);
  });
  test("finds a scenario's generations by branch name and opens the next one beside a used one", () => {
    expect(branchOf("feature/changelog", 1)).toBe("feature/changelog");
    expect(branchOf("feature/changelog", 3)).toBe("feature/changelog-3");
    expect(generationOf("feature/changelog", "feature/changelog")).toBe(1);
    expect(generationOf("feature/changelog", "feature/changelog-3")).toBe(3);
    expect(generationOf("feature/changelog", "feature/changelog-notes")).toBe(0);
    expect(generationOf("docs/usage-headings", "docs/usage-headings-2")).toBe(2);
  });
  test("knows when a pull request is where its scenario wants it", () => {
    const open = { state: "open", draft: false, merged_at: null, mergeable_state: "clean" };
    expect(inState("open", open)).toBe(true);
    expect(inState("draft", open)).toBe(false);
    expect(inState("draft", { ...open, draft: true })).toBe(true);
    expect(inState("closed", { ...open, state: "closed" })).toBe(true);
    expect(inState("closed", { ...open, state: "closed", merged_at: "2026-10-07T00:00:00Z" })).toBe(false);
    expect(inState("merged", { ...open, state: "closed", merged_at: "2026-10-07T00:00:00Z" })).toBe(true);
    expect(inState("conflicting", { ...open, mergeable_state: "dirty" })).toBe(true);
    expect(inState("behind", open, 0)).toBe(false);
    expect(inState("behind", open, 3)).toBe(true);
  });
  test("the second account adds its own, the reviewed and the cross-repository pull requests", () => {
    const one = scenarios({ second: null }).map((s) => s.key);
    const two = scenarios({ second: "lane-second" });
    expect(one).toEqual(["open-clean", "draft", "closed", "merged", "conflict", "failing", "running", "behind", "many-files", "stack-bottom", "stack-top"]);
    expect(two.find((s) => s.key === "stack-top")?.after).toBe("stack-bottom");
    expect(two.filter((s) => s.author === "second").map((s) => s.key)).toEqual(["second-review", "cross-repo"]);
    expect(two.find((s) => s.key === "cross-repo")?.fork).toBe(true);
    expect(Object.keys(two.find((s) => s.key === "many-files")!.files).length).toBe(310);
  });
});

describe("the playground stays neutral (user decision 2026-10-07)", () => {
  test("no name, title, body, label, message or file the seed writes mentions the tooling", () => {
    const words = /t3|sandbox|exact|clone|lane|probe|fixture/i;
    const texts: string[] = [DESCRIPTION, ...LABELS.flat(), bulkBranch(BULK)];
    for (const [, message, files] of HISTORY) texts.push(message as string, ...Object.entries(files as Record<string, string>).flat());
    for (const spec of scenarios({ second: "someone" })) texts.push(spec.branch, spec.title, spec.body, ...Object.entries(spec.files).flat(), ...(spec.labels ?? []), ...(spec.statuses ?? []).flat());
    expect(texts.filter((text) => words.test(text))).toEqual([]);
    // Bodies carry only neutral `<!-- ref:… -->` markers, from the seed and the probe alike.
    for (const file of ["seed.mjs", "probe.mjs"]) expect(readFileSync(join(import.meta.dir, file), "utf8").match(/<!-- (?!ref:)\S+/g)).toBe(null);
  });
});

describe("the probe's record", () => {
  test("names each gh call by subcommand, REST path or GraphQL field, with the repository abbreviated", () => {
    expect(verb("pr merge 12 --repo lane/sb --squash", "lane/sb")).toBe("pr merge 12 --squash");
    expect(verb("api --hostname github.com repos/lane/sb/pulls/12/files?per_page=100&page=2", "lane/sb")).toBe("api repos/O/R/pulls/12/files?per_page=100&page=2");
    expect(verb("api graphql --hostname github.com -f query=mutation($id:ID!){markFileAsViewed(input:{pullRequestId:$id})}", "lane/sb")).toBe("graphql markFileAsViewed");
    expect(verb("api graphql --hostname github.com --input -", "lane/sb")).toBe("graphql (stdin)");
  });
  test("prints one row per RPC with its result and read-back", () => {
    const text = table([{ id: "W1", rpc: "pullRequests.comment", ok: true, ms: 812, summary: "", error: "", gh: ["pr comment 5 --body-file -"], readback: { ok: true, detail: "1 comments on #5" } },
      { id: "W31", rpc: "stack actions", ok: null, ms: 0, summary: "", error: "", gh: [], readback: null, unavailable: "GitHub stacks: unavailable (HTTP 404)" }]);
    expect(text.split("\n").slice(2)).toEqual([
      "| W1 | pullRequests.comment | decoded (812 ms) | pr comment 5 --body-file - | confirmed: 1 comments on #5 |",
      "| W31 | stack actions | not producible: GitHub stacks: unavailable (HTTP 404) |  |  |",
    ]);
  });
});
