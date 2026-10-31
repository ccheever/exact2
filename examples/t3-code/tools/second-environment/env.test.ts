// The environment fixture's own pieces, offline: its port rule, the lane server's environment,
// the pinned providers, the label shim, the offline proxy's refusal and the drive's ops. No T3
// server starts here.
import { afterAll, describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FINISH_WELCOME, driverFlags, pairOps, redact } from "./env.mjs";
import { forget as forgetStandin } from "../acp-standin/install.mjs";
import { makeHub, HUB_ACCOUNTS } from "./usage-hub.mjs";
import { answer, PULL_REQUEST } from "./gh-standin.mjs";
import { respond } from "./github-api-standin.mjs";
import { resetDetail } from "./github-api-detail.mjs";
import { githubCertificates, ENVIRONMENTS, HUB, LOCAL_PORT, SEEDS, STANDIN_INSTANCE, assertOfflinePortClosed, checkPort, envDir, fixturePaths, offlineEnv, run, seedRepository, seedSettings, serverEnv, setupEnvironment, standinInstance, turnCompleted, turnSeeded, welcomeWait, serverExecutable, example, withoutScratch, writeSamples } from "./lib.mjs";

const root = mkdtempSync(join(tmpdir(), "t3-fixture-env-test-"));
afterAll(() => rmSync(root, { recursive: true, force: true }));
const paths = fixturePaths({ T3_FIXTURE_ROOT: root });

describe("ports", () => {
  test("16500-16599 by default, another lane's range by T3_FIXTURE_PORTS, never 3773", () => {
    for (const port of [LOCAL_PORT, ...Object.values(ENVIRONMENTS).map((entry) => entry.port)]) expect(checkPort(port, {})).toBe(port);
    expect(() => checkPort(16600, {})).toThrow("16500-16599");
    expect(checkPort(16700, { T3_FIXTURE_PORTS: "16700-16710" })).toBe(16700);
    expect(() => checkPort(3773, { T3_FIXTURE_PORTS: "3000-4000" })).toThrow();
  });
});

describe("a lane server's environment", () => {
  test("is built from nothing: no token, home or config of this shell; its homes under the fixture root; offline", () => {
    const built = serverEnv(paths, "second", { USER: "u", GH_TOKEN: "leak", OPENAI_API_KEY: "leak", HOME: "/Users/u", CODEX_HOME: "/Users/u/.codex" });
    expect(Object.values(built)).not.toContain("leak");
    for (const key of ["HOME", "CODEX_HOME", "CLAUDE_CONFIG_DIR", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "T3CODE_HOME", "TMPDIR", "GH_CONFIG_DIR"]) expect(built[key]!.startsWith(envDir(paths, "second"))).toBe(true);
    expect(built).toMatchObject({ T3CODE_TELEMETRY_ENABLED: "false", NODE_USE_ENV_PROXY: "1", HTTPS_PROXY: "http://127.0.0.1:16599", NO_PROXY: "127.0.0.1,localhost" });
    expect(built.PATH.split(":")[0]).toBe(join(envDir(paths, "second"), "bin"));
  });

  test("the offline proxy port must refuse: a listener there stops a start", async () => {
    const server = createServer().listen(0, "127.0.0.1");
    await new Promise((done) => server.once("listening", done));
    const port = (server.address() as { port: number }).port;
    const env = { T3_FIXTURE_PORTS: `${port}-${port}`, T3_FIXTURE_OFFLINE_PORT: String(port) };
    expect(offlineEnv(env).HTTPS_PROXY).toBe(`http://127.0.0.1:${port}`);
    await expect(assertOfflinePortClosed(env)).rejects.toThrow("offline proxy port");
    server.close();
    await new Promise((done) => server.once("close", done));
    await expect(assertOfflinePortClosed(env)).resolves.toBeUndefined();
  });

  test("--scratch stops Git's repository search at the environment, so the server offers a folder for threads without a project", () => {
    expect(serverEnv(paths, "second", {}, { scratch: true }).GIT_CEILING_DIRECTORIES).toBe(envDir(paths, "second"));
    expect(serverEnv(paths, "second", {}).GIT_CEILING_DIRECTORIES).toBeUndefined();
  });

  test("no-scratch makes This machine's home a Git repository, so its server offers no folder for threads without a project", () => {
    const home = withoutScratch(join(root, "local-no-scratch"));
    expect(run("/usr/bin/git", ["-C", home, "rev-parse", "--is-inside-work-tree"]).stdout.trim()).toBe("true");
    expect(withoutScratch(home)).toBe(home);
  });

  test("samples writes a Python file, a one-page PDF and a video", () => {
    const [py, pdf, mp4] = writeSamples(join(root, "samples"));
    expect(readFileSync(py, "utf8")).toStartWith("def greet");
    expect(run("/usr/bin/file", ["-b", pdf]).stdout).toStartWith("PDF document, version 1.4, 1 page");
    expect(run("/usr/bin/file", ["-b", mp4]).stdout).toStartWith("ISO Media");
  });

  test("--runtime takes another release's archive from .runtime-cache", () => {
    expect(() => serverExecutable(paths, "0.0.1-missing")).toThrow(join(example, ".runtime-cache", "t3-0.0.1-missing-darwin-arm64.tar.gz"));
  });

  test("settings pin every built-in provider to a missing binary and keep what a drive added", () => {
    const home = join(root, "settings-home");
    seedSettings(home);
    const file = join(home, "userdata/settings.json");
    const first = JSON.parse(readFileSync(file, "utf8"));
    for (const id of ["codex", "claudeAgent", "cursor", "grok", "pi", "opencode"]) expect(first.providers[id].binaryPath).toStartWith("/nonexistent/");
    writeFileSync(file, JSON.stringify({ ...first, providerInstances: { kept: { driver: "acpRegistry" } }, providers: { ...first.providers, codex: { enabled: false } } }));
    const second = JSON.parse(readFileSync(seedSettings(home), "utf8"));
    expect(second.providerInstances).toEqual({ kept: { driver: "acpRegistry" } });
    expect(second.providers.codex).toEqual({ enabled: false, binaryPath: "/nonexistent/t3-fixture/codex" });
  });

  test("the login shell finds the label shim first, and it answers ComputerName with the lane's label", () => {
    setupEnvironment(paths, "third");
    const env = serverEnv(paths, "third", { USER: "u" });
    const out = run("/bin/zsh", ["-ilc", "command -v scutil; scutil --get ComputerName"], { env }).stdout.trim().split("\n");
    expect(out.slice(-2)).toEqual([join(envDir(paths, "third"), "bin/scutil"), "Lane Third"]);
  });

  test("a seeded repository has one commit and its Git origin, which the clone groups projects by", () => {
    setupEnvironment(paths, "second");
    const entry = SEEDS.second.find((seed) => seed.repo === "shared-notes")!;
    expect(SEEDS.third.find((seed) => seed.repo === "shared-notes")!.origin).toBe(entry.origin);
    const path = seedRepository(paths, "second", entry);
    const env = serverEnv(paths, "second", { USER: "u" });
    expect(run("/usr/bin/git", ["remote", "get-url", "origin"], { env, cwd: path }).stdout.trim()).toBe(entry.origin);
    expect(run("/usr/bin/git", ["rev-list", "--count", "HEAD"], { env, cwd: path }).stdout.trim()).toBe("1");
    expect(seedRepository(paths, "second", entry)).toBe(path);
    expect(existsSync(join(path, "README.md"))).toBe(true);
  });
});

describe("threads with a turn (seed --turns)", () => {
  test("only `second` has them: a completed turn, one with file edits, one waiting on an approval; and an empty project", () => {
    expect(SEEDS.second.flatMap((entry) => (entry.turns ?? []).map((turn) => [entry.repo, turn.title, turn.prompt, !!turn.approval]))).toEqual([
      ["background-only", "Background thread (completed turn)", "Say hello from the stand-in", false],
      ["agent-edits", "Turn with file edits", "Edit the notes", false],
      ["agent-edits", "Edit awaiting approval", "Ask before editing the notes", true]]);
    expect(SEEDS.second.find((entry) => entry.repo === "empty-project")!.threads).toEqual([]);
    expect(SEEDS.third.some((entry) => entry.turns)).toBe(false);
    expect(STANDIN_INSTANCE).toMatch(/^[a-zA-Z][a-zA-Z0-9_-]*$/); // ProviderInstanceId
  });

  test("a turn is done once the latest run has completed and none is active; an approval turn once its request is pending", () => {
    const completed = { latestRunId: "r1", latestRunCompletedAt: "2026-10-16T00:00:00Z", activeRunId: null };
    expect(turnCompleted(completed)).toBe(true);
    expect(turnCompleted({ latestRunId: null, latestRunCompletedAt: null })).toBe(false);
    expect(turnCompleted({ latestRunId: "r1", latestRunCompletedAt: null, activeRunId: "r1" })).toBe(false);
    expect(turnCompleted(undefined)).toBe(false);
    const waiting = { latestRunId: "r1", latestRunCompletedAt: null, activeRunId: "r1", pendingRuntimeRequest: { id: "q1", kind: "file-change" } };
    const after = (status: string) => turnSeeded({ approval: true }, { ...completed, status });
    expect([turnSeeded({ approval: true }, waiting), after("completed"), after("cancelled"), turnSeeded({ approval: true }, { ...waiting, pendingRuntimeRequest: null })]).toEqual([true, true, false, false]);
    expect(turnSeeded({}, waiting)).toBe(false);
    // A held turn is seeded while it runs.
    expect([turnSeeded({ hold: true }, { latestRunId: "r1", latestRunCompletedAt: null, activeRunId: "r1" }), turnSeeded({ hold: true }, { latestRunId: null, activeRunId: null })]).toEqual([true, false]);
  });

  test("seed --more-turns adds a second approval, an italic answer and a held turn to agent-edits", () => {
    expect(SEEDS.second.flatMap((entry) => (entry.moreTurns ?? []).map((turn) => [entry.repo, turn.title, turn.prompt, !!turn.approval, !!turn.hold]))).toEqual([
      ["agent-edits", "Second edit awaiting approval", "Ask before editing the notes", true, false],
      ["agent-edits", "Italic answer", "Show the italic sample", false, false],
      ["agent-edits", "Turn held open", "Hold the turn", false, true]]);
  });

  test("the stand-in's instance is one install.mjs forget removes", () => {
    const home = join(root, "forget-home");
    seedSettings(home);
    const file = join(home, "userdata/settings.json");
    writeFileSync(file, JSON.stringify({ ...JSON.parse(readFileSync(file, "utf8")), providerInstances: { [STANDIN_INSTANCE]: standinInstance() } }));
    expect(forgetStandin(home)).toEqual([STANDIN_INSTANCE]);
  });
});

describe("a drive", () => {
  test("pairs the first environment in the open welcome field and each later one through Add a computer", () => {
    expect(pairOps("http://127.0.0.1:16510/pair#token=A", 0)[0]).toBe("type welcome-pairing-link http://127.0.0.1:16510/pair#token=A");
    expect(pairOps("http://127.0.0.1:16511/pair#token=B", 1).slice(0, 2)).toEqual(["tap welcome-add-computer", "type welcome-pairing-link http://127.0.0.1:16511/pair#token=B"]);
  });
  test("with This machine running, opens Add a computer before the first pairing too (the fold starts closed)", () => {
    expect(pairOps("http://127.0.0.1:16510/pair#token=A", 0, true).slice(0, 2)).toEqual(["tap welcome-add-computer", "type welcome-pairing-link http://127.0.0.1:16510/pair#token=A"]);
    expect(pairOps("http://127.0.0.1:16511/pair#token=B", 1, true).filter(op => op === "tap welcome-add-computer")).toHaveLength(1);
    expect(FINISH_WELCOME).toContain("tap welcome-skip-import");
  });
  test("passes --epoch to the driver with the size and the store", () => {
    expect(driverFlags(["--local", "--epoch", "now", "--storage", "s1"])).toEqual(["--size", "1280x840", "--storage", "s1", "--epoch", "now"]);
    expect(driverFlags(["--size", "900x700", "--json"])).toEqual(["--size", "900x700", "--storage", "t3-fixtures", "--json"]);
    // A drive against a server an earlier drive used takes a new seed (clone/issues/command-receipt-relaunch.md).
    expect(driverFlags(["--pair", "second", "--seed", "48211", "--storage", "s2"])).toEqual(["--size", "1280x840", "--storage", "s2", "--seed", "48211"]);
  });
  test("prints no pairing token", () => {
    expect(redact('typed #448 "welcome-pairing-link" · value="http://127.0.0.1:16510/pair#token=LEY3T64Y3TVC"')).toBe('typed #448 "welcome-pairing-link" · value="http://127.0.0.1:16510/pair#token=<redacted>"');
  });
});

describe("the usage hub stand-in (seed --limits)", () => {
  test("lists its accounts, answers each one's usage the way cliproxyApi.ts reads it, and a consumed credit empties the session window", () => {
    const now = Date.parse("2026-10-17T00:00:00Z"), seconds = now / 1000;
    const hub = makeHub(HUB_ACCOUNTS, () => now);
    const [, list] = hub("GET", "/v0/management/auth-files", undefined) as [number, { files: { provider: string; auth_index: string }[] }];
    expect(list.files.map((file) => file.provider)).toEqual(["codex", "codex", "codex", "claude", "claude"]);
    const call = (index: string, url: string, data?: object) => {
      const [status, reply] = hub("POST", "/v0/management/api-call", { auth_index: index, url, ...(data ? { data: JSON.stringify(data) } : {}) }) as [number, { status_code: number; body: string }];
      expect([status, reply.status_code]).toEqual([200, 200]);
      return JSON.parse(reply.body);
    };
    const codex = "https://chatgpt.com/backend-api/wham";
    expect(call("lane-1", `${codex}/usage`).rate_limit.primary_window).toEqual({ used_percent: 35, reset_at: seconds + 7200, limit_window_seconds: 18000 });
    expect(call("lane-1", `${codex}/rate-limit-reset-credits`).credits.map((credit: { id: string }) => credit.id)).toEqual(["lane-1-credit-1", "lane-1-credit-2"]);
    expect(call("lane-1", `${codex}/rate-limit-reset-credits/consume`, { credit_id: "lane-1-credit-1" })).toEqual({ code: "reset" });
    expect(call("lane-1", `${codex}/usage`).rate_limit.primary_window.used_percent).toBe(0);
    expect(call("lane-1", `${codex}/rate-limit-reset-credits/consume`, { credit_id: "lane-1-credit-1" })).toEqual({ code: "no_credit" });
    expect(call("lane-4", "https://api.anthropic.com/api/oauth/usage").five_hour).toEqual({ utilization: 55, resets_at: new Date(now + 3 * 3_600_000).toISOString() });
    expect(hub("POST", "/v0/management/reset-quota", { auth_index: "lane-1" })[0]).toBe(200);
    expect(checkPort(HUB.port, {})).toBe(HUB.port);
  });
});

describe("the gh stand-in (seed --pull-request)", () => {
  test("links its pull request to the thread's branch and answers the panel's reads; refuses the rest, as a gh off GitHub would", () => {
    const json = (argv: string[], stdin = "") => { const result = answer(argv, () => stdin); expect(result.code).toBe(0); return JSON.parse(result.out); };
    const byHead = (head: string) => json(["api", "graphql", "--hostname", "github.com", "--input", "-"],
      JSON.stringify({ query: "query PullRequestsByHead($owner: String!) {}", variables: { owner: "t3-fixture", name: "shared-notes", h0: head, s0: ["OPEN"] } })).data.repository.h0.nodes;
    expect(byHead("feature/notes")).toEqual([expect.objectContaining({ number: PULL_REQUEST.number, headRefName: "feature/notes", isCrossRepository: false, url: "https://github.com/t3-fixture/shared-notes/pull/7" })]);
    expect(byHead("main")).toEqual([]);
    const summaries = json(["api", "graphql", "-f", 'query=query PullRequestSummaries {\n  s0: repository(owner: "t3-fixture", name: "shared-notes") { pullRequest(number: 7) { number } }\n  s1: repository(owner: "other", name: "repo") { pullRequest(number: 7) { number } }\n}']).data;
    expect([summaries.s0.pullRequest.state, summaries.s1.pullRequest]).toEqual(["OPEN", null]);
    const asked = ["api", "graphql", "-f", "owner=t3-fixture", "-f", "name=shared-notes", "-F", "number=7"];
    expect(json([...asked, "-f", "query=query { repository { mergeCommitAllowed } }"]).data.repository).toMatchObject({ viewerPermission: "WRITE", pullRequest: { state: "OPEN", mergeable: "MERGEABLE" } });
    expect(json([...asked, "-F", "cursor=null", "-f", "query=query { reviewThreads(first: 50) }"]).data.repository.pullRequest.reviewThreads.nodes[0]).toMatchObject({ path: "README.md", isResolved: false });
    expect(answer(["api", "--hostname", "github.com", "repos/t3-fixture/shared-notes/stacks?pull_request=7"]).out).toBe("[]");
    expect(answer(["pr", "diff", "7", "--repo", "github.com/t3-fixture/shared-notes", "--color", "never"]).out).toContain("+++ b/README.md\n@@ -1,3 +1,5 @@");
    expect(json(["pr", "view", "7", "--repo", "github.com/t3-fixture/shared-notes", "--json", "author,comments,reviews,commits"]).comments).toEqual([]);
    const list = (...args: string[]) => json(["pr", "list", ...args, "--json", "number"]).map((row: { number: number }) => row.number);
    expect([list("--repo", "github.com/t3-fixture/shared-notes", "--state", "open"), list("--repo", "github.com/t3-fixture/agent-edits", "--state", "open"), list("--repo", "github.com/t3-fixture/shared-notes", "--state", "merged")]).toEqual([[7], [], []]);
    for (const refused of [["pr", "merge", "7"], ["api", "graphql", "-f", "query=query { search }"]]) expect(answer(refused).code).toBe(1);
  });
});

describe("the GitHub API stand-in (seed --pull-request)", () => {
  const graphql = (query: string, variables: Record<string, unknown>) => {
    const answered = respond("POST", "/graphql", JSON.stringify({ query, variables }));
    expect(answered.status).toBe(200);
    return JSON.parse(answered.body);
  };
  test("answers the server's batched head lookup, alias by alias, from each alias's variables", () => {
    const data = graphql("query PullRequestsByHead($_0_owner:String!){_0_repository:repository(owner:$_0_owner){pullRequests{nodes{number}}}_1_repository:repository(owner:$_1_owner){pullRequests{nodes{number}}}}",
      { _0_owner: "t3-fixture", _0_name: "shared-notes", _0_head: "feature/notes", _1_owner: "t3-fixture", _1_name: "shared-notes", _1_head: "main" }).data;
    expect(data._0_repository.pullRequests.nodes).toEqual([expect.objectContaining({ number: 7, state: "OPEN", headRefOid: "0".repeat(40) })]);
    expect(data._1_repository.pullRequests.nodes).toEqual([]);
  });
  test("answers the summary and the lookup by number; anything else is an error, a REST path 404", () => {
    expect(graphql("query PullRequestSummary($owner:String!){}", { owner: "t3-fixture", name: "shared-notes", number: 7, stacks: false }).data.repository.pullRequest)
      .toMatchObject({ state: "OPEN", isDraft: false, mergeable: "MERGEABLE", additions: 203, deletions: 1, changedFiles: 3 });
    expect(graphql("query PullRequestByNumber($owner:String!){}", { owner: "other", name: "repo", number: 7 }).data.repository).toBeNull();
    expect(graphql("query ViewerLabels($owner:String!){}", {}).errors[0].message).toContain("ViewerLabels");
    expect(JSON.parse(respond("GET", "/user", "").body)).toMatchObject({ login: PULL_REQUEST.author });
    expect(respond("GET", "/repos/t3-fixture/shared-notes/pulls/7/files", "").status).toBe(404);
  });
  test("answers the Pull Requests page: the search and a repository's list name #7 while open, its counts with $stats or after", () => {
    const search = (q: string, stats = false) => graphql("query PullRequestSearch($q:String!){}", { q, first: 100, stacks: false, stats }).data.search.nodes;
    expect(search("is:pr is:open archived:false repo:t3-fixture/shared-notes")).toEqual([expect.objectContaining({ number: 7, state: "OPEN", repository: { nameWithOwner: "t3-fixture/shared-notes" } })]);
    expect(search("is:pr is:open repo:t3-fixture/shared-notes")[0].additions).toBeUndefined();
    expect(search("is:pr is:open repo:t3-fixture/shared-notes", true)[0]).toMatchObject({ additions: 203, deletions: 1 });
    expect([search("is:pr is:merged repo:t3-fixture/shared-notes"), search("is:pr is:open repo:t3-fixture/agent-edits")]).toEqual([[], []]);
    const list = (states: string[]) => graphql("query PullRequestList($owner:String!){}", { owner: "t3-fixture", name: "shared-notes", states, first: 100, stacks: false, stats: false }).data.repository.pullRequests.nodes;
    expect([list(["OPEN"]).length, list(["MERGED"]).length]).toEqual([1, 0]);
    expect(graphql("query PullRequestStats($owner:String!){}", { owner: "t3-fixture", name: "shared-notes", number: 7 }).data.repository.pullRequest).toEqual({ additions: 203, deletions: 1 });
  });
  test("answers the detail panel: core with its checks, activity, review threads, the patch, then a comment and a close", () => {
    resetDetail();
    const v = { owner: "t3-fixture", name: "shared-notes", number: 7 };
    const core = graphql("query PullRequestCore($owner:String!){}", { ...v, headRef: "refs/pull/7/head", required: true }).data.repository.pullRequest;
    expect([core.state, core.commits.nodes[0].commit.statusCheckRollup.contexts.nodes.map((node: { name?: string; context?: string }) => node.name ?? node.context)]).toEqual(["OPEN", ["build", "lint", "test (macos)", "docs/preview"]]);
    const threads = graphql("query ReviewThreads($owner:String!){}", v).data.repository.pullRequest.reviewThreads.nodes;
    expect(threads[0].comments.nodes.map((comment: { id: string }) => comment.id)).toEqual(["PRRC_1", "PRRC_2"]);
    const patch = respond("GET", "/repos/t3-fixture/shared-notes/pulls/7", "", "application/vnd.github.diff");
    expect([patch.status, patch.body.includes("+++ b/notes/outline.ts")]).toEqual([200, true]);
    graphql("mutation AddComment($subjectId:ID!){}", { subjectId: "PR_7", body: "Superseded by #8" });
    graphql("mutation ClosePullRequest($pullRequestId:ID!){}", { pullRequestId: "PR_7" });
    const comments = graphql("query PullRequestActivity($owner:String!){}", { ...v, head: true, withComments: true, withReviews: true }).data.repository.pullRequest.comments.nodes;
    expect([comments.at(-1).body, graphql("query PullRequestCore($owner:String!){}", { ...v, headRef: "refs/pull/7/head", required: true }).data.repository.pullRequest.state]).toEqual(["Superseded by #8", "CLOSED"]);
    resetDetail();
  });
  test("answers the Filters pickers' host search, narrowed by the query, and takes a review whose comment covers a range", () => {
    resetDetail();
    const v = { owner: "t3-fixture", name: "shared-notes" };
    const labels = (query: string | null) => graphql("query RepositoryLabels($owner:String!){}", { ...v, query }).data.repository.labels.nodes.map((label: { name: string }) => label.name);
    const authors = (query: string | null) => graphql("query RepositoryAuthors($owner:String!){}", { ...v, query }).data.repository.mentionableUsers.nodes.map((person: { login: string }) => person.login);
    expect([labels(null), labels("doc"), authors(null).length, authors("octo"), authors("reviewer")]).toEqual([["bug", "documentation", "enhancement", "good first issue"], ["documentation"], 3, ["octo-hostonly"], ["lane-reviewer"]]);
    const review = { event: "COMMENT", body: "", comments: [{ path: "notes/plan.md", start_line: 3, start_side: "RIGHT", line: 5, side: "RIGHT", body: "These three steps." }] };
    const answered = respond("POST", "/repos/t3-fixture/shared-notes/pulls/7/reviews", JSON.stringify(review));
    expect([answered.status, answered.name]).toEqual([200, "submitReview COMMENT [notes/plan.md:3-5]"]);
    const threads = graphql("query ReviewThreads($owner:String!){}", { ...v, number: 7 }).data.repository.pullRequest.reviewThreads.nodes;
    expect(threads.at(-1)).toMatchObject({ path: "notes/plan.md", startLine: 3, line: 5, comments: { nodes: [{ body: "These three steps." }] } });
    resetDetail();
  });
  test("its certificate is made once, and again when it would expire within a day", () => {
    const dir = join(root, "github-certs");
    expect(githubCertificates(dir)).toBe(true);
    expect(run("/usr/bin/openssl", ["verify", "-CAfile", join(dir, "ca.crt"), join(dir, "api.crt")]).stdout).toContain("OK");
    expect(run("/usr/bin/openssl", ["x509", "-noout", "-ext", "subjectAltName", "-in", join(dir, "api.crt")], { allowFail: true }).stdout + run("/usr/bin/openssl", ["x509", "-noout", "-text", "-in", join(dir, "api.crt")]).stdout).toContain("DNS:api.github.com");
    expect(githubCertificates(dir)).toBe(false);
    run("/usr/bin/openssl", ["x509", "-req", "-in", "api.csr", "-CA", "ca.crt", "-CAkey", "ca.key", "-out", "api.crt", "-days", "1", "-extfile", "api.cnf", "-extensions", "ext"], { cwd: dir });
    expect(githubCertificates(dir)).toBe(true);
  });
  test("lets the viewer allow edits by maintainers on its fork's pull request, and keeps the choice and the files viewed", () => {
    resetDetail();
    const v = { owner: "t3-fixture", name: "shared-notes", number: 7 };
    const core = () => graphql("query PullRequestCore($owner:String!){}", { ...v, headRef: "refs/pull/7/head", required: true }).data.repository.pullRequest;
    // v0.1.5 gitHubPullRequestJson.ts toPullRequestCore: open, cross-repository, authored by the viewer, a User's fork, a head ref.
    expect(core()).toMatchObject({ maintainerCanModify: true, isCrossRepository: true, viewerDidAuthor: true, headRepositoryOwner: { __typename: "User", login: PULL_REQUEST.author }, headRef: { id: "REF_1" } });
    graphql("mutation UpdatePullRequest($pullRequestId:ID!){}", { pullRequestId: "PR_7", maintainerCanModify: false });
    expect(core().maintainerCanModify).toBe(false);
    // SetFileViewed comes aliased per file, its variables prefixed alike.
    const marked = graphql("mutation SetFileViewed($_0_pullRequestId:ID!,$_0_path:String!,$_0_viewed:Boolean!,$_1_pullRequestId:ID!,$_1_path:String!,$_1_viewed:Boolean!){}",
      { _0_pullRequestId: "PR_7", _0_path: "README.md", _0_viewed: true, _1_pullRequestId: "PR_7", _1_path: "notes/plan.md", _1_viewed: true }).data;
    expect(marked).toEqual({ _0_markFileAsViewed: { clientMutationId: null }, _1_markFileAsViewed: { clientMutationId: null } });
    graphql("mutation SetFileViewed($pullRequestId:ID!,$path:String!,$viewed:Boolean!){}", { pullRequestId: "PR_7", path: "notes/plan.md", viewed: false });
    const viewed = graphql("query PullRequestFilesViewed($owner:String!){}", v).data.repository.pullRequest.files.nodes;
    expect(viewed.map((file: { path: string; viewerViewedState: string }) => [file.path, file.viewerViewedState])).toEqual([["README.md", "VIEWED"], ["notes/plan.md", "UNVIEWED"], ["notes/outline.ts", "UNVIEWED"]]);
    expect(graphql("mutation SetFileViewed($pullRequestId:ID!,$path:String!,$viewed:Boolean!){}", { pullRequestId: "PR_7", path: "elsewhere.md", viewed: true }).errors).toBeArray();
    resetDetail();
  });
  test("names the activity's commit by node id and answers its stats, as the 0.1.5 server reads them", () => {
    const v = { owner: "t3-fixture", name: "shared-notes", number: 7, head: true, withComments: false, withReviews: false };
    const commit = graphql("query PullRequestActivity($owner:String!){}", v).data.repository.pullRequest.commits.nodes[0].commit;
    expect(commit).toMatchObject({ id: "C_1", parents: { totalCount: 1 } });
    expect(graphql("query PullRequestCommitStats($ids:[ID!]!){}", { ids: [commit.id, "C_other"] }).data.nodes).toEqual([{ oid: commit.oid, additions: 203, deletions: 1 }, null]);
  });
  test("a --pull-request server trusts the stand-in's CA and sends its HTTPS to the stand-in's proxy", () => {
    const built = serverEnv(paths, "second", { T3_FIXTURE_GITHUB_PORT: "16592" }, { githubApi: true });
    expect(built).toMatchObject({ HTTPS_PROXY: "http://127.0.0.1:16592", NO_PROXY: "127.0.0.1,localhost", NODE_EXTRA_CA_CERTS: join(root, "github-api", "ca.crt") });
  });
});

describe("the wait for the welcome wizard", () => {
  test("20 s until the local home holds the pinned runtime, 5 s after", () => {
    const home = join(root, "wait-home");
    expect(welcomeWait(home)).toBe(20000);
    const { version } = JSON.parse(readFileSync(join(import.meta.dir, "../../server-runtime/runtime-pin.json"), "utf8"));
    mkdirSync(join(home, "runtime/versions", version), { recursive: true });
    expect(welcomeWait(home)).toBe(5000);
  });
});
