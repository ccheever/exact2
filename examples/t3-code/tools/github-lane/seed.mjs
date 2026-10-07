#!/usr/bin/env bun
// Seeds the GitHub lane's sandbox repository with what the clone's pull request features need,
// through the lane `gh` of the two signed-in accounts only, touching nothing but the sandbox and
// the second account's fork of it. Idempotent: a re-run finds what exists by branch name and by
// an invisible `<!-- lane:… -->` marker in every body it writes, puts a drifted pull request back
// where that is one call (draft again, closed again, reopened), and otherwise opens the next
// generation (`seed/<key>-g2`) beside the used one.
//
//   bun seed.mjs [--repo owner/name] [--create --visibility private|public] [--skip-bulk]
//
// `--create` is the only way the repository gets created (the user approves the sandbox first).
// Writes `sandbox.json` in the shared lane dir: the repository, the logins (never tokens), what
// GitHub let this account do, the base commits and every seeded pull request's number.
import { existsSync, mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { join } from "node:path";
import { api, gh, git, lanePaths, readSandbox, setup, signedIn, writeSandbox } from "./lib.mjs";

export const MARKER = "T3 Code clone GitHub lane sandbox";
const flag = (name) => process.argv.includes(name);
const option = (name) => { const at = process.argv.indexOf(name); return at > 0 ? process.argv[at + 1] : undefined; };
const sleep = (ms) => Bun.sleep(ms);
const mark = (step) => `\n\n<!-- lane:${step} -->`;
const hasMark = (body, step) => typeof body === "string" && body.includes(`<!-- lane:${step} -->`);

/** The sandbox's base history: fixed contents and dates, so its commits are the same on every run. */
export const VALIDATE_V1 = [
  "// Input validation for the sandbox app.",
  "export function validate(input: string): string[] {",
  "  const problems: string[] = [];",
  '  if (input.length === 0) problems.push("empty");',
  '  if (input.length > 80) problems.push("too long");',
  '  if (/\\s{2,}/.test(input)) problems.push("double spaces");',
  '  if (input !== input.trim()) problems.push("untrimmed");',
  "  return problems; // v1",
  "}",
  "",
  "export const isValid = (input: string) => validate(input).length === 0;",
  "",
].join("\n");
export const HISTORY = [
  ["c1", "Initial commit", { "README.md": "# T3 Code clone sandbox\n\nSynthetic data for the T3 Code clone's pull request checks.\n" }],
  ["c2", "Add the app entry", { "src/app.ts": 'import { greet } from "./util";\n\nexport function main(name: string): string {\n  return greet(name);\n}\n' }],
  ["c3", "Add helpers", { "src/util.ts": "export const greet = (name: string) => `Hello, ${name}!`;\nexport const shout = (text: string) => text.toUpperCase();\n" }],
  ["c4", "Write the guide", { "docs/guide.md": "# Guide\n\n## Install\n\nRun the app.\n\n## Use\n\nCall `main`.\n" }],
  ["c5", "Add the conflict fixture", { "CONFLICT.md": "status: base\n" }],
  ["c6", "Describe the sandbox", { "README.md": "# T3 Code clone sandbox\n\nSynthetic data for the T3 Code clone's pull request checks (exact2 `examples/t3-code`).\nNothing here is real code; every pull request is a fixture.\n" }],
  ["c7", "Change the conflict fixture on main", { "CONFLICT.md": "status: from main\n" }],
];

/** Every seeded pull request. `from` is a base commit key; `files` its change; `want` its state. */
export function scenarios({ second }) {
  const file = (key, text) => ({ [`seed/${key}.md`]: `${text}\n` });
  const list = [
    { key: "open-clean", title: "Add a changelog entry", from: "c7", files: { "CHANGELOG.md": "# Changelog\n\n- The sandbox exists.\n" }, want: "open",
      statuses: [["lane/ci", "success", "Build passed"]], labels: ["enhancement", "area:ui"], reviewers: second ? ["second"] : [] },
    { key: "draft", title: "Sketch the settings page", from: "c7", files: file("draft", "A draft that is not ready."), want: "draft", statuses: [["lane/ci", "success", "Build passed"]] },
    { key: "closed", title: "Try a different README tone", from: "c7", files: file("closed", "Closed without merging."), want: "closed" },
    { key: "merged", title: "Fix a typo in the guide", from: "c7", files: file("merged", "Merged by the seed."), want: "merged" },
    { key: "conflict", title: "Rewrite the conflict fixture", from: "c5", files: { "CONFLICT.md": "status: from the branch\n" }, want: "conflicting", statuses: [["lane/ci", "success", "Build passed"]] },
    { key: "failing", title: "Speed up the helpers", from: "c7", files: file("failing", "A change whose checks fail."), want: "open",
      statuses: [["lane/ci", "success", "Build passed"], ["lane/test", "failure", "2 tests failed"]], labels: ["bug"] },
    { key: "running", title: "Add an app banner", from: "c7", files: file("running", "A change whose checks are still running."), want: "open", statuses: [["lane/ci", "pending", "Build running"]] },
    { key: "behind", title: "Document the release steps", from: "c4", files: file("behind", "Branched before the last three commits on main."), want: "behind", statuses: [["lane/ci", "success", "Build passed"]] },
    { key: "many-files", title: "Generate 310 fixture files", from: "c7", files: Object.fromEntries(Array.from({ length: 310 }, (_, i) => [`fixtures/f${String(i + 1).padStart(3, "0")}.txt`, `fixture ${i + 1}\n`])), want: "open" },
  ];
  if (second) list.push(
    { key: "second-review", author: "second", title: "Add input validation", from: "c7", files: { "src/validate.ts": VALIDATE_V1 }, want: "open", conversation: "review", labels: ["needs-review"] },
    { key: "primary-review", title: "Tidy the guide headings", from: "c7", files: { "docs/guide.md": "# Guide\n\n## Installing\n\nRun the app.\n\n## Using\n\nCall `main`.\n" }, want: "open", conversation: "approved" },
    { key: "cross-repo", author: "second", fork: true, title: "Add a contributing guide", from: "c7", files: { "CONTRIBUTING.md": "# Contributing\n\nOpen a pull request from a fork.\n" }, want: "open" },
  );
  return list;
}
export const LABELS = [["area:ui", "1d76db", "Interface"], ["needs-review", "fbca04", "Waiting for a reviewer"], ["priority:high", "b60205", "Do this first"], ["probe", "c5def5", "Opened by the lane probe"]];
export const BULK = 105;
export const branchOf = (key, generation) => (generation > 1 ? `seed/${key}-g${generation}` : `seed/${key}`);
export const generationOf = (key, ref) => { const m = new RegExp(`^seed/${key.replace(/[^a-z0-9-]/g, "")}(?:-g(\\d+))?$`).exec(ref); return m ? Number(m[1] ?? 1) : 0; };

/** Is this pull request where its scenario wants it? (`pr` is a REST pull with `merged_at`, `draft`, `mergeable_state`.) */
export function inState(want, pr, behindBy = 0) {
  const open = pr.state === "open";
  if (want === "open") return open && !pr.draft;
  if (want === "draft") return open && pr.draft === true;
  if (want === "closed") return !open && !pr.merged_at;
  if (want === "merged") return !!pr.merged_at;
  if (want === "conflicting") return open && pr.mergeable_state === "dirty";
  if (want === "behind") return open && behindBy > 0;
  return false;
}

export class Seed {
  constructor({ paths = lanePaths(), repo, create = false, visibility = "private", log = console.log } = {}) {
    this.paths = paths; this.create = create; this.visibility = visibility; this.log = log; this.requestedRepo = repo;
    this.report = { created: [], reused: [], restored: [], unavailable: {} };
  }
  /** `gh api` as an account; a secondary rate limit waits a minute (then two, three) and asks again. */
  call(account, args, opts = {}) {
    for (let attempt = 1; ; attempt++) {
      const result = api(this.paths, account, args, { ...opts, allowFail: true });
      const limited = (result.status === 403 || result.status === 429) && /rate limit/i.test(JSON.stringify(result.body ?? ""));
      if (limited && attempt <= 3) { this.log(`rate limited; waiting ${attempt} min`); Bun.sleepSync(60_000 * attempt); continue; }
      if (opts.allowFail === false && (result.status < 200 || result.status >= 300)) throw new Error(`gh api ${args.join(" ").slice(0, 160)}: HTTP ${result.status} ${JSON.stringify(result.body)?.slice(0, 400)}`);
      return result;
    }
  }
  primary(args, opts) { return this.call("primary", args, opts); }
  second(args, opts) { return this.call("second", args, opts); }
  as(account) { return account === "second" ? this.second.bind(this) : this.primary.bind(this); }
  login(account) { return account === "second" ? this.secondLogin : this.owner; }

  identities() {
    const me = this.primary(["user"], { allowFail: false }).body;
    this.owner = me.login; this.ownerId = me.id; this.plan = me.plan?.name ?? "unknown";
    if (signedIn(this.paths, "second")) {
      const other = this.second(["user"], { allowFail: false }).body;
      if (other.login.toLowerCase() === me.login.toLowerCase()) throw new Error("both lane config dirs are signed in to the same account");
      this.secondLogin = other.login; this.secondId = other.id;
    }
    this.repo = this.requestedRepo ?? `${this.owner}/t3-code-clone-sandbox`;
    [this.repoOwner, this.name] = this.repo.split("/");
    if (this.repoOwner.toLowerCase() !== this.owner.toLowerCase()) throw new Error(`the sandbox must belong to the primary account (${this.owner}), not ${this.repoOwner}`);
    this.log(`primary ${this.owner} (plan ${this.plan}); second ${this.secondLogin ?? "not signed in"}; sandbox ${this.repo}`);
  }

  ensureRepo() {
    const found = this.primary([`repos/${this.repo}`]);
    if (found.status === 200) {
      if (!String(found.body.description ?? "").includes(MARKER)) throw new Error(`${this.repo} exists but is not the lane sandbox (its description lacks "${MARKER}"); refusing to touch it`);
      this.visibility = found.body.private ? "private" : "public";
      this.report.reused.push("repository");
      return;
    }
    if (found.status !== 404) throw new Error(`repos/${this.repo}: HTTP ${found.status}`);
    if (!this.create) throw new Error(`${this.repo} does not exist; pass --create once the user has approved it`);
    this.primary(["-X", "POST", "user/repos", "-f", `name=${this.name}`, "-F", `private=${this.visibility !== "public"}`,
      "-f", `description=${MARKER}: synthetic pull requests for exact2 examples/t3-code. Disposable.`,
      "-F", "has_wiki=false", "-F", "has_projects=false", "-F", "auto_init=false", "-F", "allow_auto_merge=true", "-F", "allow_update_branch=true", "-F", "delete_branch_on_merge=false"], { allowFail: false });
    this.report.created.push(`repository (${this.visibility})`);
  }

  /** The local object store the seed builds commits in (never pushed except named refs). */
  ensureWork({ fetch = true } = {}) {
    this.work = join(this.paths.work, this.name);
    if (!existsSync(join(this.work, ".git"))) { mkdirSync(this.work, { recursive: true }); git(this.paths, "primary", ["init", "--quiet", "-b", "main"], { cwd: this.work }); }
    this.g = (account, args, opts = {}) => git(this.paths, account, args, { cwd: this.work, ...opts });
    this.url = `https://github.com/${this.repo}.git`;
    if (fetch) this.g("primary", ["fetch", "--quiet", "--no-tags", this.url, "+refs/heads/*:refs/remotes/sandbox/*"], { allowFail: true });
  }

  identity(account) {
    const login = this.login(account), id = account === "second" ? this.secondId : this.ownerId;
    return { name: login, email: `${id}+${login}@users.noreply.github.com` };
  }
  /** One commit on `parent` with `files` (path → text, or null to delete), fixed author and date. */
  commit({ parent, files, message, account = "primary", at }) {
    const index = join(mkdtempSync(join(this.paths.lane, "tmp-index-")), "index");
    try {
      const env = { GIT_INDEX_FILE: index };
      if (parent) this.g(account, ["read-tree", parent], { env });
      const lines = [];
      for (const [path, text] of Object.entries(files)) {
        if (text === null) { this.g(account, ["update-index", "--force-remove", path], { env }); continue; }
        const blob = this.g(account, ["hash-object", "-w", "--stdin"], { input: text }).stdout.trim();
        lines.push(`100644 blob ${blob}\t${path}`);
      }
      if (lines.length) this.g(account, ["update-index", "--index-info"], { env, input: `${lines.join("\n")}\n` });
      const tree = this.g(account, ["write-tree"], { env }).stdout.trim();
      const who = this.identity(account), date = new Date(at).toISOString();
      return this.g(account, ["commit-tree", tree, ...(parent ? ["-p", parent] : []), "-m", message], {
        env: { GIT_AUTHOR_NAME: who.name, GIT_AUTHOR_EMAIL: who.email, GIT_AUTHOR_DATE: date, GIT_COMMITTER_NAME: who.name, GIT_COMMITTER_EMAIL: who.email, GIT_COMMITTER_DATE: date },
      }).stdout.trim();
    } finally { rmSync(join(index, ".."), { recursive: true, force: true }); }
  }

  ensureHistory() {
    this.base = {};
    let parent = null;
    HISTORY.forEach(([key, message, files], i) => { parent = this.commit({ parent, files, message, at: Date.UTC(2026, 9, 1, 9 + i) }); this.base[key] = parent; });
    const remoteMain = this.g("primary", ["rev-parse", "--verify", "--quiet", "refs/remotes/sandbox/main"], { allowFail: true }).stdout.trim();
    if (!remoteMain) {
      this.g("primary", ["push", "--quiet", this.url, `${this.base.c7}:refs/heads/main`]);
      this.report.created.push("main (7 commits)");
    } else if (this.g("primary", ["merge-base", "--is-ancestor", this.base.c7, remoteMain], { allowFail: true }).status !== 0) {
      throw new Error("the sandbox's main does not contain the seed history (another account's commits?); refusing to rewrite it");
    } else this.report.reused.push("main history");
  }

  ensureLabels() {
    for (const [name, color, description] of LABELS) {
      if (this.primary([`repos/${this.repo}/labels/${encodeURIComponent(name)}`]).status === 200) continue;
      this.primary(["-X", "POST", `repos/${this.repo}/labels`, "-f", `name=${name}`, "-f", `color=${color}`, "-f", `description=${description}`], { allowFail: false });
      this.report.created.push(`label ${name}`);
    }
  }

  async ensureCollaborator() {
    if (!this.secondLogin) return;
    if (this.primary([`repos/${this.repo}/collaborators/${this.secondLogin}`]).status === 204) { this.report.reused.push("collaborator"); return; }
    const invitations = this.primary([`repos/${this.repo}/invitations`]).body ?? [];
    if (!invitations.some((invite) => invite.invitee?.login?.toLowerCase() === this.secondLogin.toLowerCase())) {
      this.primary(["-X", "PUT", `repos/${this.repo}/collaborators/${this.secondLogin}`], { allowFail: false });
      this.report.created.push(`invitation to ${this.secondLogin}`);
    }
    const mine = (this.second(["user/repository_invitations"]).body ?? []).find((invite) => invite.repository?.full_name?.toLowerCase() === this.repo.toLowerCase());
    if (mine) { this.second(["-X", "PATCH", `user/repository_invitations/${mine.id}`], { allowFail: false }); this.report.created.push(`${this.secondLogin} accepted the invitation`); }
    for (let i = 0; i < 15 && this.primary([`repos/${this.repo}/collaborators/${this.secondLogin}`]).status !== 204; i++) await sleep(2000);
  }

  async ensureFork() {
    if (!this.secondLogin) return;
    this.fork = `${this.secondLogin}/${this.name}`;
    const found = this.second([`repos/${this.fork}`]);
    if (found.status === 200) {
      if (found.body.parent?.full_name?.toLowerCase() !== this.repo.toLowerCase()) throw new Error(`${this.fork} exists but is not a fork of ${this.repo}; refusing to touch it`);
      this.report.reused.push("fork"); return;
    }
    const made = this.second(["-X", "POST", `repos/${this.repo}/forks`, "-F", "default_branch_only=true"]);
    if (made.status !== 202 && made.status !== 200) { this.report.unavailable["cross-repo"] = `fork refused: HTTP ${made.status} ${JSON.stringify(made.body)?.slice(0, 200)}`; this.fork = null; return; }
    for (let i = 0; i < 30 && this.second([`repos/${this.fork}`]).status !== 200; i++) await sleep(2000);
    this.report.created.push(`fork ${this.fork}`);
  }

  /** Every pull request in the sandbox, by head (`owner:ref`). */
  listPulls() {
    this.pulls = (gh(this.paths, "primary", ["api", "--paginate", "--slurp", `repos/${this.repo}/pulls?state=all&per_page=100`], { json: true }) ?? []).flat();
  }
  pullsFor(key, account, fork) {
    const owner = fork ? this.secondLogin : this.repoOwner;
    return this.pulls.filter((pr) => pr.head?.repo?.owner?.login?.toLowerCase() === owner.toLowerCase() && generationOf(key, pr.head.ref) > 0)
      .sort((a, b) => generationOf(key, b.head.ref) - generationOf(key, a.head.ref));
  }
  detail(number) { return this.primary([`repos/${this.repo}/pulls/${number}`], { allowFail: false }).body; }
  behindBy(pr) { return this.primary([`repos/${this.repo}/compare/${pr.base.ref}...${pr.head.repo.owner.login}:${pr.head.ref}`]).body?.behind_by ?? 0; }

  /** Puts a used pull request back in one call where GitHub has one; false when it needs a new generation. */
  async restore(spec, pr) {
    const n = pr.number;
    if (spec.want === "draft" && pr.state === "open" && !pr.draft) {
      const r = this.primary(["graphql", "-f", "query=mutation($id:ID!){convertPullRequestToDraft(input:{pullRequestId:$id}){pullRequest{isDraft}}}", "-f", `id=${pr.node_id}`]);
      return r.status === 200 && !r.body?.errors;
    }
    if (spec.want === "closed" && pr.state === "open") return this.primary(["-X", "PATCH", `repos/${this.repo}/pulls/${n}`, "-f", "state=closed"]).status === 200;
    if (spec.want === "open" && pr.state === "closed" && !pr.merged_at) return this.as(spec.author)(["-X", "PATCH", `repos/${this.repo}/pulls/${n}`, "-f", "state=open"]).status === 200;
    if (spec.want === "merged" && pr.state === "open") return this.primary(["-X", "PUT", `repos/${this.repo}/pulls/${n}/merge`, "-f", "merge_method=squash"]).status === 200;
    return false;
  }

  async ensureScenario(spec) {
    const account = spec.author ?? "primary";
    const existing = this.pullsFor(spec.key, account, spec.fork);
    let generation = existing.length ? generationOf(spec.key, existing[0].head.ref) : 1;
    if (existing.length) {
      let pr = this.detail(existing[0].number);
      for (let i = 0; i < 5 && pr.state === "open" && pr.mergeable_state === "unknown"; i++) { await sleep(1500); pr = this.detail(pr.number); }
      if (inState(spec.want, pr, spec.want === "behind" ? this.behindBy(pr) : 0)) { this.report.reused.push(`#${pr.number} ${spec.key}`); return this.finish(spec, pr); }
      if (await this.restore(spec, pr)) { this.report.restored.push(`#${pr.number} ${spec.key}`); return this.finish(spec, this.detail(pr.number)); }
      generation += 1;
    }
    const branch = branchOf(spec.key, generation);
    const files = generation > 1 ? { ...spec.files, [`seed/${spec.key}-g${generation}.md`]: `generation ${generation}\n` } : spec.files;
    const head = this.commit({ parent: this.base[spec.from], files, account, message: `${spec.title}${generation > 1 ? ` (generation ${generation})` : ""}`, at: Date.UTC(2026, 9, 2, 9) + generation * 60_000 });
    const target = spec.fork ? `https://github.com/${this.fork}.git` : this.url;
    if (spec.fork && !this.fork) { this.report.unavailable[spec.key] ??= "no fork"; return null; }
    this.g(account, ["push", "--quiet", target, `${head}:refs/heads/${branch}`]);
    const body = `Seeded fixture: ${spec.key}.${mark(`pr-${spec.key}`)}`;
    const args = ["-X", "POST", `repos/${this.repo}/pulls`, "-f", `title=${spec.title}`, "-f", `head=${spec.fork ? `${this.secondLogin}:${branch}` : branch}`, "-f", "base=main", "-f", `body=${body}`];
    if (spec.want === "draft") args.push("-F", "draft=true");
    const made = this.as(account)(args);
    await sleep(1200);
    if (made.status !== 201) {
      const reason = `${made.status}: ${made.body?.message ?? ""} ${(made.body?.errors ?? []).map((e) => e.message ?? e.code).join("; ")}`.trim();
      this.report.unavailable[spec.key] = reason;
      if (spec.want === "draft" && /draft/i.test(reason)) this.capabilities.drafts = false;
      return null;
    }
    this.report.created.push(`#${made.body.number} ${spec.key}`);
    let pr = made.body;
    if (spec.want === "closed") { this.primary(["-X", "PATCH", `repos/${this.repo}/pulls/${pr.number}`, "-f", "state=closed"], { allowFail: false }); }
    if (spec.want === "merged") { await sleep(2000); this.primary(["-X", "PUT", `repos/${this.repo}/pulls/${pr.number}/merge`, "-f", "merge_method=squash"], { allowFail: false }); }
    pr = this.detail(pr.number);
    return this.finish(spec, pr);
  }

  /** Statuses, labels, review requests and conversations, each only where it is missing. */
  async finish(spec, pr) {
    if (pr.state === "open" && spec.statuses) {
      const present = new Set((this.primary([`repos/${this.repo}/commits/${pr.head.sha}/status`]).body?.statuses ?? []).map((s) => s.context));
      for (const [context, state, description] of spec.statuses) if (!present.has(context)) {
        this.primary(["-X", "POST", `repos/${this.repo}/statuses/${pr.head.sha}`, "-f", `state=${state}`, "-f", `context=${context}`, "-f", `description=${description}`, "-f", `target_url=https://github.com/${this.repo}/pull/${pr.number}`], { allowFail: false });
      }
    }
    if (pr.state === "open") for (const label of spec.labels ?? []) if (!pr.labels?.some((l) => l.name === label)) this.primary(["-X", "POST", `repos/${this.repo}/issues/${pr.number}/labels`, "-f", `labels[]=${label}`]);
    if (pr.state === "open") for (const who of spec.reviewers ?? []) {
      const login = this.login(who);
      if (login && !pr.requested_reviewers?.some((r) => r.login === login)) this.primary(["-X", "POST", `repos/${this.repo}/pulls/${pr.number}/requested_reviewers`, "-f", `reviewers[]=${login}`]);
    }
    if (spec.conversation === "review") await this.reviewConversation(pr);
    if (spec.conversation === "approved") await this.approvedConversation(pr);
    return pr.number;
  }

  /** A second-account pull request reviewed by the primary: line threads (unresolved, resolved, outdated, a 12-comment one), a dismissed review, comments and reactions. */
  async reviewConversation(pr) {
    const n = pr.number, repo = this.repo;
    const reviews = this.primary([`repos/${repo}/pulls/${n}/reviews?per_page=100`]).body ?? [];
    let first = reviews.find((r) => hasMark(r.body, "review-1"));
    if (!first) {
      const comments = [[4, "Should an empty input be an error or a warning?", "thread-open"], [6, "This regex also matches tabs; intended?", "thread-resolved"],
        [8, "Return a sorted list so the output is stable.", "thread-outdated"], [11, "Let us discuss the name here.", "thread-long"]]
        .map(([line, text, step]) => ({ path: "src/validate.ts", line, side: "RIGHT", body: `${text}${mark(step)}` }));
      const made = this.primary(["-X", "POST", `repos/${repo}/pulls/${n}/reviews`, "--input", "-"], { input: JSON.stringify({ commit_id: pr.head.sha, event: "REQUEST_CHANGES", body: `A few things before this lands.${mark("review-1")}`, comments }) });
      if (made.status !== 200) { this.report.unavailable["review-1"] = `HTTP ${made.status}`; return; }
      first = made.body; this.report.created.push(`#${n} review 1 with 4 line comments`); await sleep(1500);
    }
    const lineComments = gh(this.paths, "primary", ["api", `repos/${repo}/pulls/${n}/comments?per_page=100`], { json: true }) ?? [];
    const root = (step) => lineComments.find((c) => hasMark(c.body, step) && !c.in_reply_to_id);
    const long = root("thread-long");
    if (long) for (let i = 1; i <= 11; i++) {
      const step = `thread-long-reply-${i}`;
      if (lineComments.some((c) => hasMark(c.body, step))) continue;
      const replier = i % 2 ? "second" : "primary";
      this.as(replier)(["-X", "POST", `repos/${repo}/pulls/${n}/comments/${long.id}/replies`, "-f", `body=Reply ${i} in the long thread.${mark(step)}`], { allowFail: false });
      await sleep(800);
    }
    const threads = this.primary(["graphql", "-f", "query=query($o:String!,$r:String!,$n:Int!){repository(owner:$o,name:$r){pullRequest(number:$n){reviewThreads(first:50){nodes{id isResolved comments(first:1){nodes{body}}}}}}}", "-f", `o=${this.repoOwner}`, "-f", `r=${this.name}`, "-F", `n=${n}`]).body;
    const resolvedThread = threads?.data?.repository?.pullRequest?.reviewThreads?.nodes?.find((t) => hasMark(t.comments.nodes[0]?.body, "thread-resolved"));
    if (resolvedThread && !resolvedThread.isResolved) this.primary(["graphql", "-f", "query=mutation($id:ID!){resolveReviewThread(input:{threadId:$id}){thread{isResolved}}}", "-f", `id=${resolvedThread.id}`], { allowFail: false });
    // The follow-up commit moves line 8, which makes its thread outdated.
    const head = this.detail(n);
    const commits = this.primary([`repos/${repo}/pulls/${n}/commits`]).body ?? [];
    if (!commits.some((c) => c.commit.message.includes("lane: follow-up"))) {
      this.g("second", ["fetch", "--quiet", this.url, head.head.sha], { allowFail: true });
      const next = this.commit({ parent: head.head.sha, account: "second", message: "Sort the problems (lane: follow-up)", at: Date.UTC(2026, 9, 3, 9),
        files: { "src/validate.ts": VALIDATE_V1.replace("  return problems; // v1", "  return problems.sort(); // v2") } });
      this.g("second", ["push", "--quiet", this.url, `${next}:refs/heads/${head.head.ref}`]);
      this.report.created.push(`#${n} follow-up commit`); await sleep(1500);
    }
    if (first.state !== "DISMISSED") {
      const dismissed = this.primary(["-X", "PUT", `repos/${repo}/pulls/${n}/reviews/${first.id}/dismissals`, "-f", "message=Superseded after the follow-up commit.", "-f", "event=DISMISS"]);
      if (dismissed.status !== 200) this.report.unavailable["dismissal"] = `HTTP ${dismissed.status} ${dismissed.body?.message ?? ""}`;
    }
    if (!reviews.some((r) => hasMark(r.body, "review-2"))) this.primary(["-X", "POST", `repos/${repo}/pulls/${n}/reviews`, "-f", "event=REQUEST_CHANGES", "-f", `body=Still needs the empty-input case.${mark("review-2")}`], { allowFail: false });
    const issueComments = this.primary([`repos/${repo}/issues/${n}/comments?per_page=100`]).body ?? [];
    const say = (account, step, text) => issueComments.find((c) => hasMark(c.body, step)) ?? this.as(account)(["-X", "POST", `repos/${repo}/issues/${n}/comments`, "-f", `body=${text}${mark(step)}`], { allowFail: false }).body;
    const thanks = say("primary", "comment-1", "Thanks for this. I left notes inline.");
    say("second", "comment-2", "Updated: the problems are sorted now. Please take another look.");
    const reactions = this.primary([`repos/${repo}/issues/comments/${thanks.id}/reactions`]).body ?? [];
    for (const content of ["+1", "heart"]) if (!reactions.some((r) => r.content === content && r.user?.login === this.secondLogin)) this.second(["-X", "POST", `repos/${repo}/issues/comments/${thanks.id}/reactions`, "-f", `content=${content}`]);
    const prReactions = this.primary([`repos/${repo}/issues/${n}/reactions`]).body ?? [];
    if (!prReactions.some((r) => r.content === "rocket")) this.primary(["-X", "POST", `repos/${repo}/issues/${n}/reactions`, "-f", "content=rocket"]);
  }

  /** A primary pull request the second account approved and commented on. */
  async approvedConversation(pr) {
    const n = pr.number, repo = this.repo;
    const reviews = this.primary([`repos/${repo}/pulls/${n}/reviews?per_page=100`]).body ?? [];
    if (!reviews.some((r) => hasMark(r.body, "approve"))) this.second(["-X", "POST", `repos/${repo}/pulls/${n}/reviews`, "-f", "event=APPROVE", "-f", `body=Approved: the headings read better.${mark("approve")}`], { allowFail: false });
    const comments = this.primary([`repos/${repo}/issues/${n}/comments?per_page=100`]).body ?? [];
    if (!comments.some((c) => hasMark(c.body, "approved-comment"))) this.second(["-X", "POST", `repos/${repo}/issues/${n}/comments`, "-f", `body=One small thing for later: the Use section could link the API.${mark("approved-comment")}`], { allowFail: false });
  }

  /** Enough open pull requests to page the Pull Requests list past its 99-row slice. */
  async ensureBulk() {
    const have = new Set(this.pulls.filter((pr) => /^seed\/bulk-\d{3}$/.test(pr.head.ref) && pr.head.repo?.owner?.login?.toLowerCase() === this.repoOwner.toLowerCase()).map((pr) => pr.head.ref));
    const missing = Array.from({ length: BULK }, (_, i) => `seed/bulk-${String(i + 1).padStart(3, "0")}`).filter((ref) => !have.has(ref));
    if (!missing.length) { this.report.reused.push(`${BULK} bulk pull requests`); return; }
    const remote = new Set(this.g("primary", ["for-each-ref", "--format=%(refname:strip=3)", "refs/remotes/sandbox/seed/"]).stdout.split("\n").filter(Boolean));
    const refspecs = [];
    for (const ref of missing) {
      if (remote.has(ref)) continue;
      const n = ref.slice(-3);
      refspecs.push(`${this.commit({ parent: this.base.c7, files: { [`bulk/${n}.md`]: `Bulk fixture ${n}\n` }, message: `Bulk fixture ${n}`, at: Date.UTC(2026, 9, 1, 18) + Number(n) * 1000 })}:refs/heads/${ref}`);
    }
    for (let i = 0; i < refspecs.length; i += 40) this.g("primary", ["push", "--quiet", this.url, ...refspecs.slice(i, i + 40)]);
    for (const ref of missing) {
      const made = this.primary(["-X", "POST", `repos/${this.repo}/pulls`, "-f", `title=Bulk fixture ${ref.slice(-3)}`, "-f", `head=${ref}`, "-f", "base=main", "-f", `body=Seeded for list paging.${mark(`bulk-${ref.slice(-3)}`)}`]);
      if (made.status !== 201) throw new Error(`bulk ${ref}: HTTP ${made.status} ${JSON.stringify(made.body)?.slice(0, 300)}`);
      await sleep(1500);
    }
    this.report.created.push(`${missing.length} bulk pull requests`);
  }

  /** What GitHub lets this account do in this sandbox, asked rather than assumed. */
  probeCapabilities(numbers) {
    const any = numbers["open-clean"];
    const stacks = any ? this.primary([`repos/${this.repo}/stacks?pull_request=${any}`]) : { status: 0 };
    this.capabilities.stacks = stacks.status === 200 ? "available" : `unavailable (HTTP ${stacks.status} on repos/${this.repo}/stacks)`;
    const protection = this.primary([`repos/${this.repo}/branches/main/protection`]);
    this.capabilities.branchProtection = protection.status === 200 ? "configured" : protection.status === 404 ? "available, none set" : `unavailable (HTTP ${protection.status}: ${protection.body?.message ?? ""})`;
  }

  async run({ bulk = true } = {}) {
    setup(this.paths);
    for (const account of ["primary"]) if (!signedIn(this.paths, account)) throw new Error(`${account} is not signed in (README)`);
    this.identities();
    this.capabilities = { drafts: true };
    this.ensureRepo();
    this.ensureWork();
    this.ensureHistory();
    this.ensureLabels();
    await this.ensureCollaborator();
    await this.ensureFork();
    this.listPulls();
    if (bulk) await this.ensureBulk();
    const numbers = {};
    for (const spec of scenarios({ second: this.secondLogin })) numbers[spec.key] = await this.ensureScenario(spec);
    this.probeCapabilities(numbers);
    const previous = readSandbox(this.paths) ?? {};
    const sandbox = { ...previous, repo: this.repo, visibility: this.visibility, primary: this.owner, second: this.secondLogin ?? null, fork: this.fork ?? null, plan: this.plan,
      base: this.base, prs: numbers, capabilities: this.capabilities, unavailable: this.report.unavailable, seededAt: new Date().toISOString() };
    writeSandbox(this.paths, sandbox);
    return { sandbox, report: this.report };
  }
}

if (import.meta.main) {
  try {
    const visibility = option("--visibility") ?? "private";
    if (!["private", "public"].includes(visibility)) throw new Error("--visibility private|public");
    const seed = new Seed({ repo: option("--repo"), create: flag("--create"), visibility });
    const { sandbox, report } = await seed.run({ bulk: !flag("--skip-bulk") });
    console.log(JSON.stringify({ repo: sandbox.repo, prs: sandbox.prs, capabilities: sandbox.capabilities, report }, null, 2));
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
