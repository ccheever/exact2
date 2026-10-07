#!/usr/bin/env bun
// Seeds the GitHub lane's repository with what the clone's pull request features need, through
// the lane `gh` of the two signed-in accounts only, touching nothing but that repository and the
// second account's fork of it. The repository is a plain playground (user decision 2026-10-07):
// neutral names and tiny files, no code from this repository or from T3 Code, nothing that
// names the tooling. Idempotent: a re-run finds the repository by the id it recorded at creation,
// the pull requests by branch name and every body it wrote by an invisible `<!-- ref:… -->` marker;
// it puts a drifted pull request back where that is one call (draft again, closed again,
// reopened), and otherwise opens the next generation (`feature/changelog-2`) beside the used one.
//
//   bun seed.mjs --repo owner/name [--create --visibility private|public] [--skip-bulk]
//
// `--create` is the only way the repository gets created (the user approves it first). An
// existing repository the lane did not create is never touched. Writes `sandbox.json` in the
// shared lane dir: the repository and its id, the logins (never tokens), what GitHub let these
// accounts do, the base commits and every seeded pull request's number.
import { existsSync, mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { join } from "node:path";
import { api, gh, git, lanePaths, readSandbox, setup, signedIn, writeSandbox } from "./lib.mjs";

const flag = (name) => process.argv.includes(name);
const option = (name) => { const at = process.argv.indexOf(name); return at > 0 ? process.argv[at + 1] : undefined; };
const sleep = (ms) => Bun.sleep(ms);
export const mark = (step) => `\n\n<!-- ref:${step} -->`;
export const hasMark = (body, step) => typeof body === "string" && body.includes(`<!-- ref:${step} -->`);
export const DESCRIPTION = "A small playground repository.";
/** The status every merge waits for (branch protection on main). */
export const REQUIRED = "ci/build";

export const VALIDATE_V1 = [
  "// Checks a line of input and lists what is wrong with it.",
  "export function validate(input) {",
  "  const problems = [];",
  '  if (input.length === 0) problems.push("empty");',
  '  if (input.length > 80) problems.push("too long");',
  '  if (/\\s{2,}/.test(input)) problems.push("double spaces");',
  '  if (input !== input.trim()) problems.push("untrimmed");',
  "  return problems;",
  "}",
  "",
  "export const isValid = (input) => validate(input).length === 0;",
  "",
].join("\n");
/** The base history: fixed contents and dates, so its commits are the same on every run. */
export const HISTORY = [
  ["c1", "Initial commit", { "README.md": "# playground\n\nA small repository for trying things out.\n" }],
  ["c2", "Add greeting helper", { "src/greet.js": "export function greet(name) {\n  return `Hello, ${name}!`;\n}\n" }],
  ["c3", "Add text helpers", { "src/text.js": "export const shout = (text) => text.toUpperCase();\nexport const whisper = (text) => text.toLowerCase();\n" }],
  ["c4", "Add usage notes", { "docs/usage.md": "# Usage\n\n## Install\n\nCopy the files.\n\n## Use\n\nCall `greet`.\n" }],
  ["c5", "Add status file", { "STATUS.md": "status: draft\n" }],
  ["c6", "Expand the README", { "README.md": "# playground\n\nA small repository for trying things out.\n\nSee docs/usage.md for how to use it.\n" }],
  ["c7", "Update status", { "STATUS.md": "status: published\n" }],
];

/** Every seeded pull request. `from` is a base commit key; `files` its change; `want` its state. */
export function scenarios({ second }) {
  const ok = [[REQUIRED, "success", "Build passed"]];
  const list = [
    { key: "open-clean", branch: "feature/changelog", title: "Add a changelog", body: "Adds a changelog file.", from: "c7", files: { "CHANGELOG.md": "# Changelog\n\n- First release.\n" },
      want: "open", statuses: ok, labels: ["enhancement", "area:ui"], reviewers: second ? ["second"] : [] },
    { key: "draft", branch: "feature/settings-page", title: "Sketch the settings page", body: "Not ready yet.", from: "c7", files: { "docs/settings.md": "# Settings\n\nTo be written.\n" }, want: "draft", statuses: ok },
    { key: "closed", branch: "feature/friendly-tone", title: "Try a friendlier tone", body: "An idea that did not work out.", from: "c7", files: { "docs/tone.md": "Hi there!\n" }, want: "closed" },
    { key: "merged", branch: "feature/faq", title: "Add a FAQ page", body: "Answers the common questions.", from: "c7", files: { "docs/faq.md": "# FAQ\n\nNothing asked yet.\n" }, want: "merged", statuses: ok },
    { key: "conflict", branch: "fix/status-wording", title: "Mark the status as in review", body: "Changes the status line.", from: "c5", files: { "STATUS.md": "status: in review\n" }, want: "conflicting", statuses: ok },
    { key: "failing", branch: "fix/text-helpers", title: "Speed up the text helpers", body: "Caches the case changes.", from: "c7", files: { "src/text.js": "const cache = new Map();\nexport const shout = (text) => text.toUpperCase();\nexport const whisper = (text) => text.toLowerCase();\n" },
      want: "open", statuses: [...ok, ["ci/test", "failure", "2 tests failed"]], labels: ["bug"] },
    { key: "running", branch: "feature/banner", title: "Add a banner helper", body: "Prints a banner.", from: "c7", files: { "src/banner.js": "export const banner = (text) => `== ${text} ==`;\n" }, want: "open", statuses: [[REQUIRED, "pending", "Build running"]] },
    { key: "behind", branch: "feature/release-notes", title: "Document the release steps", body: "Writes down how a release works.", from: "c4", files: { "docs/release.md": "# Release\n\n1. Tag.\n2. Publish.\n" }, want: "behind", statuses: ok },
    { key: "many-files", branch: "feature/sample-data", title: "Add sample data files", body: "310 small data files.", from: "c7",
      files: Object.fromEntries(Array.from({ length: 310 }, (_, i) => [`data/item-${String(i + 1).padStart(3, "0")}.txt`, `item ${i + 1}\n`])), want: "open", statuses: ok },
  ];
  if (second) list.push(
    { key: "second-review", author: "second", branch: "feature/input-validation", title: "Add input validation", body: "Adds a small validator.", from: "c7", files: { "src/validate.js": VALIDATE_V1 }, want: "open", conversation: "review", labels: ["needs-review"] },
    { key: "primary-review", branch: "docs/usage-headings", title: "Tidy the usage headings", body: "Renames two headings.", from: "c7", files: { "docs/usage.md": "# Usage\n\n## Installing\n\nCopy the files.\n\n## Using\n\nCall `greet`.\n" }, want: "open", conversation: "approved", statuses: ok },
    { key: "cross-repo", author: "second", fork: true, branch: "docs/contributing", title: "Add contributing notes", body: "How to send a change.", from: "c7", files: { "CONTRIBUTING.md": "# Contributing\n\nOpen a pull request from a fork.\n" }, want: "open" },
  );
  // Two pull requests in one GitHub stack (the second's base is the first's branch).
  list.push(
    { key: "stack-bottom", branch: "feature/greeting-options", title: "Let greet take options", body: "Adds an options argument.", from: "c7", files: { "src/greet.js": "export function greet(name, { loud = false } = {}) {\n  const text = `Hello, ${name}!`;\n  return loud ? text.toUpperCase() : text;\n}\n" }, want: "open", statuses: ok },
    { key: "stack-top", branch: "docs/greeting-options", title: "Document the greeting options", body: "Explains the options argument.", after: "stack-bottom", files: { "docs/greeting.md": "# Greeting\n\nPass `{ loud: true }` to shout.\n" }, want: "open", statuses: ok },
  );
  return list;
}
export const LABELS = [["area:ui", "1d76db", "Interface"], ["needs-review", "fbca04", "Waiting for a reviewer"], ["priority:high", "b60205", "Do this first"], ["chore", "c5def5", "Housekeeping"]];
export const BULK = 105;
export const bulkBranch = (n) => `chore/note-${String(n).padStart(3, "0")}`;
export const branchOf = (base, generation) => (generation > 1 ? `${base}-${generation}` : base);
export const generationOf = (base, ref) => {
  const m = new RegExp(`^${base.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}(?:-(\\d+))?$`).exec(ref);
  return m ? Number(m[1] ?? 1) : 0;
};

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
      const limited = (result.status === 403 || result.status === 429) && /rate limit/i.test(JSON.stringify(result.body ?? "") + result.stderr);
      if (limited && attempt <= 3) { this.log(`rate limited; waiting ${attempt} min`); Bun.sleepSync(60_000 * attempt); continue; }
      // No HTTP answer at all (a dropped connection, a gateway error before headers): ask again shortly.
      if (result.status === 0 && attempt <= 3) { this.log(`no answer (${result.stderr.trim().slice(0, 160)}); retrying`); Bun.sleepSync(10_000 * attempt); continue; }
      if (opts.allowFail === false && (result.status < 200 || result.status >= 300)) throw new Error(`gh api ${args.join(" ").slice(0, 160)}: HTTP ${result.status} ${JSON.stringify(result.body)?.slice(0, 400)} ${result.stderr.trim().slice(0, 300)}`);
      return result;
    }
  }
  primary(args, opts) { return this.call("primary", args, opts); }
  second(args, opts) { return this.call("second", args, opts); }
  as(account) { return account === "second" ? this.second.bind(this) : this.primary.bind(this); }
  login(account) { return account === "second" ? this.secondLogin : this.owner; }

  identities() {
    const me = this.primary(["user"], { allowFail: false }).body;
    this.owner = me.login; this.ownerId = me.id;
    if (signedIn(this.paths, "second")) {
      const other = this.second(["user"], { allowFail: false }).body;
      if (other.login.toLowerCase() === me.login.toLowerCase()) throw new Error("both lane config dirs are signed in to the same account");
      this.secondLogin = other.login; this.secondId = other.id;
    }
    if (!this.requestedRepo) throw new Error("name the repository: --repo owner/name");
    this.repo = this.requestedRepo;
    [this.repoOwner, this.name] = this.repo.split("/");
    if (this.repoOwner.toLowerCase() !== this.owner.toLowerCase()) throw new Error(`the repository must belong to the primary account (${this.owner}), not ${this.repoOwner}`);
    this.log(`primary ${this.owner}; second ${this.secondLogin ?? "not signed in"}; repository ${this.repo}`);
  }

  /** Reuses only the repository this lane created (its id is in sandbox.json); never one that was there before. */
  ensureRepo() {
    const recorded = readSandbox(this.paths);
    const found = this.primary([`repos/${this.repo}`]);
    if (found.status === 200) {
      if (recorded?.repo !== this.repo || recorded?.repoId !== found.body.id) throw new Error(`${this.repo} already exists and this lane did not create it (no matching id in sandbox.json); stopping without touching it`);
      this.repoId = found.body.id; this.visibility = found.body.private ? "private" : "public";
      this.report.reused.push("repository");
      return;
    }
    if (found.status !== 404) throw new Error(`repos/${this.repo}: HTTP ${found.status}`);
    if (!this.create) throw new Error(`${this.repo} does not exist; pass --create once the user has approved it`);
    const made = this.primary(["-X", "POST", "user/repos", "-f", `name=${this.name}`, "-F", `private=${this.visibility !== "public"}`, "-f", `description=${DESCRIPTION}`,
      "-F", "has_wiki=false", "-F", "has_projects=false", "-F", "auto_init=false", "-F", "allow_auto_merge=true", "-F", "allow_update_branch=true", "-F", "delete_branch_on_merge=false"], { allowFail: false }).body;
    this.repoId = made.id;
    writeSandbox(this.paths, { repo: this.repo, repoId: made.id, visibility: this.visibility, primary: this.owner, second: this.secondLogin ?? null, createdAt: new Date().toISOString() });
    this.report.created.push(`repository (${this.visibility})`);
  }

  /** The local object store the seed builds commits in (never pushed except named refs). */
  ensureWork({ fetch = true } = {}) {
    this.work = join(this.paths.work, this.name);
    if (!existsSync(join(this.work, ".git"))) { mkdirSync(this.work, { recursive: true }); git(this.paths, "primary", ["init", "--quiet", "-b", "main"], { cwd: this.work }); }
    this.g = (account, args, opts = {}) => git(this.paths, account, args, { cwd: this.work, ...opts });
    this.url = `https://github.com/${this.repo}.git`;
    if (fetch) this.g("primary", ["fetch", "--quiet", "--no-tags", "--prune", this.url, "+refs/heads/*:refs/remotes/origin/*"], { allowFail: true });
  }

  identity(account) {
    const login = this.login(account), id = account === "second" ? this.secondId : this.ownerId;
    return { name: login, email: `${id}+${login}@users.noreply.github.com` };
  }
  /** One commit on `parent` with `files` (path → text, or null to delete), fixed author and date. */
  commit({ parent, files, message, account = "primary", at }) {
    const dir = mkdtempSync(join(this.paths.lane, "tmp-index-")), index = join(dir, "index");
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
    } finally { rmSync(dir, { recursive: true, force: true }); }
  }

  ensureHistory() {
    this.base = {};
    let parent = null;
    HISTORY.forEach(([key, message, files], i) => { parent = this.commit({ parent, files, message, at: Date.UTC(2026, 9, 1, 9 + i) }); this.base[key] = parent; });
    const remoteMain = this.g("primary", ["rev-parse", "--verify", "--quiet", "refs/remotes/origin/main"], { allowFail: true }).stdout.trim();
    if (!remoteMain) {
      this.g("primary", ["push", "--quiet", this.url, `${this.base.c7}:refs/heads/main`]);
      this.report.created.push("main (7 commits)");
    } else if (this.g("primary", ["merge-base", "--is-ancestor", this.base.c7, remoteMain], { allowFail: true }).status !== 0) {
      throw new Error("main does not contain the seeded history; stopping without rewriting it");
    } else this.report.reused.push("main history");
  }

  /** main takes merges only once `ci/build` passed, admins included: the state auto-merge waits on. */
  ensureProtection() {
    const current = this.primary([`repos/${this.repo}/branches/main/protection`]);
    if (current.status === 200 && current.body?.required_status_checks?.contexts?.includes(REQUIRED) && current.body?.enforce_admins?.enabled) { this.capabilities.branchProtection = `main requires ${REQUIRED}`; return; }
    const set = this.primary(["-X", "PUT", `repos/${this.repo}/branches/main/protection`, "--input", "-"], {
      input: JSON.stringify({ required_status_checks: { strict: false, contexts: [REQUIRED] }, enforce_admins: true, required_pull_request_reviews: null, restrictions: null }) });
    this.capabilities.branchProtection = set.status === 200 ? `main requires ${REQUIRED}` : `unavailable (HTTP ${set.status}: ${set.body?.message ?? ""})`;
    if (set.status === 200) this.report.created.push(`branch protection (${REQUIRED})`);
  }

  ensureLabels() {
    for (const [name, color, description] of LABELS) {
      if (this.primary([`repos/${this.repo}/labels/${encodeURIComponent(name)}`]).status === 200) continue;
      this.primary(["-X", "POST", `repos/${this.repo}/labels`, "-f", `name=${name}`, "-f", `color=${color}`, "-f", `description=${description}`], { allowFail: false });
      this.report.created.push(`label ${name}`);
    }
  }

  /** The primary invites; the second accepts the invitation by its id (no listing of the second account's invitations). */
  async ensureCollaborator() {
    if (!this.secondLogin) return;
    const member = () => this.primary([`repos/${this.repo}/collaborators/${this.secondLogin}`]).status === 204;
    if (member()) { this.report.reused.push("collaborator"); return; }
    let invitation = (this.primary([`repos/${this.repo}/invitations`]).body ?? []).find((invite) => invite.invitee?.login?.toLowerCase() === this.secondLogin.toLowerCase());
    if (!invitation) {
      const sent = this.primary(["-X", "PUT", `repos/${this.repo}/collaborators/${this.secondLogin}`], { allowFail: false });
      invitation = sent.body;
      this.report.created.push(`invitation to ${this.secondLogin}`);
    }
    if (invitation?.id) { this.second(["-X", "PATCH", `user/repository_invitations/${invitation.id}`], { allowFail: false }); this.report.created.push(`${this.secondLogin} accepted the invitation`); }
    for (let i = 0; i < 15 && !member(); i++) await sleep(2000);
    if (!member()) throw new Error(`${this.secondLogin} is not a collaborator after accepting`);
  }

  async ensureFork() {
    if (!this.secondLogin) return;
    this.fork = `${this.secondLogin}/${this.name}`;
    const found = this.second([`repos/${this.fork}`]);
    if (found.status === 200) {
      if (found.body.parent?.full_name?.toLowerCase() !== this.repo.toLowerCase()) throw new Error(`${this.fork} exists but is not a fork of ${this.repo}; stopping without touching it`);
      this.report.reused.push("fork"); return;
    }
    const made = this.second(["-X", "POST", `repos/${this.repo}/forks`, "-F", "default_branch_only=true"]);
    if (made.status !== 202 && made.status !== 200) { this.report.unavailable["cross-repo"] = `fork refused: HTTP ${made.status} ${JSON.stringify(made.body)?.slice(0, 200)}`; this.fork = null; return; }
    if (made.body?.full_name?.toLowerCase() !== this.fork.toLowerCase()) throw new Error(`GitHub named the fork ${made.body?.full_name}, not ${this.fork}`);
    for (let i = 0; i < 30 && this.second([`repos/${this.fork}`]).status !== 200; i++) await sleep(2000);
    this.report.created.push(`fork ${this.fork}`);
  }

  /** Every pull request in the repository. */
  listPulls() {
    this.pulls = (gh(this.paths, "primary", ["api", "--paginate", "--slurp", `repos/${this.repo}/pulls?state=all&per_page=100`], { json: true }) ?? []).flat();
  }
  pullsFor(spec) {
    const owner = spec.fork ? this.secondLogin : this.repoOwner;
    return this.pulls.filter((pr) => pr.head?.repo?.owner?.login?.toLowerCase() === owner.toLowerCase() && generationOf(spec.branch, pr.head.ref) > 0)
      .sort((a, b) => generationOf(spec.branch, b.head.ref) - generationOf(spec.branch, a.head.ref));
  }
  detail(number) { return this.primary([`repos/${this.repo}/pulls/${number}`], { allowFail: false }).body; }
  behindBy(pr) { return this.primary([`repos/${this.repo}/compare/${pr.base.ref}...${pr.head.repo.owner.login}:${pr.head.ref}`]).body?.behind_by ?? 0; }
  /** Posts the scenario's statuses on the head commit, each only when missing. */
  statuses(spec, sha, number) {
    const present = new Set((this.primary([`repos/${this.repo}/commits/${sha}/status`]).body?.statuses ?? []).map((s) => s.context));
    for (const [context, state, description] of spec.statuses ?? []) if (!present.has(context)) {
      this.primary(["-X", "POST", `repos/${this.repo}/statuses/${sha}`, "-f", `state=${state}`, "-f", `context=${context}`, "-f", `description=${description}`, "-f", `target_url=https://github.com/${this.repo}/pull/${number}`], { allowFail: false });
    }
  }

  /** Puts a used pull request back in one call where GitHub has one; false when it needs a new generation. */
  async restore(spec, pr) {
    const n = pr.number;
    if (spec.want === "draft" && pr.state === "open" && !pr.draft) {
      const r = this.primary(["graphql", "-f", "query=mutation($id:ID!){convertPullRequestToDraft(input:{pullRequestId:$id}){pullRequest{isDraft}}}", "-f", `id=${pr.node_id}`]);
      return r.status === 200 && !r.body?.errors;
    }
    if (spec.want === "closed" && pr.state === "open") return this.primary(["-X", "PATCH", `repos/${this.repo}/pulls/${n}`, "-f", "state=closed"]).status === 200;
    if (spec.want === "open" && pr.state === "closed" && !pr.merged_at) return this.as(spec.author)(["-X", "PATCH", `repos/${this.repo}/pulls/${n}`, "-f", "state=open"]).status === 200;
    if (spec.want === "merged" && pr.state === "open") { this.statuses(spec, pr.head.sha, n); await sleep(2000); return this.primary(["-X", "PUT", `repos/${this.repo}/pulls/${n}/merge`, "-f", "merge_method=squash"]).status === 200; }
    return false;
  }

  async ensureScenario(spec) {
    const account = spec.author ?? "primary";
    if (spec.fork && !this.fork) { this.report.unavailable[spec.key] ??= "no fork"; return null; }
    const existing = this.pullsFor(spec);
    let generation = existing.length ? generationOf(spec.branch, existing[0].head.ref) : 1;
    if (existing.length) {
      let pr = this.detail(existing[0].number);
      for (let i = 0; i < 5 && pr.state === "open" && pr.mergeable_state === "unknown"; i++) { await sleep(1500); pr = this.detail(pr.number); }
      if (inState(spec.want, pr, spec.want === "behind" ? this.behindBy(pr) : 0)) { this.report.reused.push(`#${pr.number} ${spec.key}`); return this.finish(spec, pr); }
      if (await this.restore(spec, pr)) { this.report.restored.push(`#${pr.number} ${spec.key}`); return this.finish(spec, this.detail(pr.number)); }
      generation += 1;
    }
    const branch = branchOf(spec.branch, generation);
    const files = generation > 1 ? { ...spec.files, [`notes/${spec.key}-${generation}.txt`]: `round ${generation}\n` } : spec.files;
    const parent = spec.after ? this.heads[spec.after]?.sha : this.base[spec.from];
    if (!parent) { this.report.unavailable[spec.key] = `no ${spec.after} to build on`; return null; }
    const head = this.commit({ parent, files, account, message: spec.title, at: Date.UTC(2026, 9, 2, 9) + generation * 60_000 });
    this.g(account, ["push", "--quiet", spec.fork ? `https://github.com/${this.fork}.git` : this.url, `${head}:refs/heads/${branch}`]);
    const args = ["-X", "POST", `repos/${this.repo}/pulls`, "-f", `title=${spec.title}`, "-f", `head=${spec.fork ? `${this.secondLogin}:${branch}` : branch}`, "-f", `base=${spec.after ? this.heads[spec.after].ref : "main"}`, "-f", `body=${spec.body}${mark(`pr-${spec.key}`)}`];
    if (spec.want === "draft") args.push("-F", "draft=true");
    const made = this.as(account)(args);
    await sleep(1200);
    if (made.status !== 201) {
      this.report.unavailable[spec.key] = `${made.status}: ${made.body?.message ?? ""} ${(made.body?.errors ?? []).map((e) => e.message ?? e.code).join("; ")}`.trim();
      return null;
    }
    this.report.created.push(`#${made.body.number} ${spec.key}`);
    this.statuses(spec, head, made.body.number);
    if (spec.want === "closed") this.primary(["-X", "PATCH", `repos/${this.repo}/pulls/${made.body.number}`, "-f", "state=closed"], { allowFail: false });
    if (spec.want === "merged") { await sleep(3000); this.primary(["-X", "PUT", `repos/${this.repo}/pulls/${made.body.number}/merge`, "-f", "merge_method=squash"], { allowFail: false }); }
    return this.finish(spec, this.detail(made.body.number));
  }

  /** Statuses, labels, review requests and conversations, each only where it is missing. */
  async finish(spec, pr) {
    this.heads[spec.key] = { sha: pr.head.sha, ref: pr.head.ref, number: pr.number };
    if (pr.state === "open") this.statuses(spec, pr.head.sha, pr.number);
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
    const n = pr.number, repo = this.repo, path = "src/validate.js";
    const reviews = this.primary([`repos/${repo}/pulls/${n}/reviews?per_page=100`]).body ?? [];
    let first = reviews.find((r) => hasMark(r.body, "review-1"));
    if (!first) {
      const comments = [[4, "Should an empty input be an error or a warning?", "thread-open"], [6, "This pattern also matches tabs. Is that intended?", "thread-resolved"],
        [8, "Return a sorted list so the output is stable.", "thread-outdated"], [11, "Let us talk about the name here.", "thread-long"]]
        .map(([line, text, step]) => ({ path, line, side: "RIGHT", body: `${text}${mark(step)}` }));
      const made = this.primary(["-X", "POST", `repos/${repo}/pulls/${n}/reviews`, "--input", "-"], { input: JSON.stringify({ commit_id: pr.head.sha, event: "REQUEST_CHANGES", body: `A few things before this lands.${mark("review-1")}`, comments }) });
      if (made.status !== 200) { this.report.unavailable["review-1"] = `HTTP ${made.status} ${made.body?.message ?? ""}`; return; }
      first = made.body; this.report.created.push(`#${n} review with 4 line comments`); await sleep(1500);
    }
    const lineComments = (gh(this.paths, "primary", ["api", "--paginate", "--slurp", `repos/${repo}/pulls/${n}/comments?per_page=100`], { json: true }) ?? []).flat();
    const long = lineComments.find((c) => hasMark(c.body, "thread-long") && !c.in_reply_to_id);
    if (long) for (let i = 1; i <= 11; i++) {
      const step = `thread-long-reply-${i}`;
      if (lineComments.some((c) => hasMark(c.body, step))) continue;
      this.as(i % 2 ? "second" : "primary")(["-X", "POST", `repos/${repo}/pulls/${n}/comments/${long.id}/replies`, "-f", `body=Reply ${i} on the naming question.${mark(step)}`], { allowFail: false });
      await sleep(800);
    }
    const threads = this.primary(["graphql", "-f", "query=query($o:String!,$r:String!,$n:Int!){repository(owner:$o,name:$r){pullRequest(number:$n){reviewThreads(first:50){nodes{id isResolved comments(first:1){nodes{body}}}}}}}", "-f", `o=${this.repoOwner}`, "-f", `r=${this.name}`, "-F", `n=${n}`]).body;
    const resolvedThread = threads?.data?.repository?.pullRequest?.reviewThreads?.nodes?.find((t) => hasMark(t.comments.nodes[0]?.body, "thread-resolved"));
    if (resolvedThread && !resolvedThread.isResolved) this.primary(["graphql", "-f", "query=mutation($id:ID!){resolveReviewThread(input:{threadId:$id}){thread{isResolved}}}", "-f", `id=${resolvedThread.id}`], { allowFail: false });
    // The follow-up commit changes line 8, which makes its thread outdated.
    const head = this.detail(n);
    const commits = this.primary([`repos/${repo}/pulls/${n}/commits`]).body ?? [];
    if (!commits.some((c) => c.commit.message.startsWith("Sort the reported problems"))) {
      this.g("second", ["fetch", "--quiet", this.url, head.head.sha], { allowFail: true });
      const next = this.commit({ parent: head.head.sha, account: "second", message: "Sort the reported problems", at: Date.UTC(2026, 9, 3, 9),
        files: { [path]: VALIDATE_V1.replace("  return problems;", "  return problems.sort();") } });
      this.g("second", ["push", "--quiet", this.url, `${next}:refs/heads/${head.head.ref}`]);
      this.report.created.push(`#${n} follow-up commit`); await sleep(1500);
    }
    if (first.state !== "DISMISSED") {
      const dismissed = this.primary(["-X", "PUT", `repos/${repo}/pulls/${n}/reviews/${first.id}/dismissals`, "-f", "message=Superseded by the follow-up commit.", "-f", "event=DISMISS"]);
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
    if (!comments.some((c) => hasMark(c.body, "approved-comment"))) this.second(["-X", "POST", `repos/${repo}/issues/${n}/comments`, "-f", `body=One small thing for later: the Using section could link the helpers.${mark("approved-comment")}`], { allowFail: false });
  }

  /** Enough open pull requests to page the Pull Requests list past its 99-row slice. */
  async ensureBulk() {
    const have = new Set(this.pulls.filter((pr) => /^chore\/note-\d{3}$/.test(pr.head.ref) && pr.head.repo?.owner?.login?.toLowerCase() === this.repoOwner.toLowerCase()).map((pr) => pr.head.ref));
    const missing = Array.from({ length: BULK }, (_, i) => bulkBranch(i + 1)).filter((ref) => !have.has(ref));
    if (!missing.length) { this.report.reused.push(`${BULK} note pull requests`); return; }
    const remote = new Set(this.g("primary", ["for-each-ref", "--format=%(refname:strip=3)", "refs/remotes/origin/chore/"]).stdout.split("\n").filter(Boolean));
    const refspecs = [];
    for (const ref of missing) {
      if (remote.has(ref)) continue;
      const n = ref.slice(-3);
      refspecs.push(`${this.commit({ parent: this.base.c7, files: { [`notes/note-${n}.txt`]: `Note ${n}\n` }, message: `Add note ${n}`, at: Date.UTC(2026, 9, 1, 18) + Number(n) * 1000 })}:refs/heads/${ref}`);
    }
    for (let i = 0; i < refspecs.length; i += 40) this.g("primary", ["push", "--quiet", this.url, ...refspecs.slice(i, i + 40)]);
    for (const ref of missing) {
      const n = ref.slice(-3);
      const made = this.primary(["-X", "POST", `repos/${this.repo}/pulls`, "-f", `title=Add note ${n}`, "-f", `head=${ref}`, "-f", "base=main", "-f", `body=Adds note ${n}.${mark(`note-${n}`)}`]);
      // A retried request whose first try landed: GitHub says the pull request already exists.
      const landed = made.status === 422 && /already exists/i.test(JSON.stringify(made.body));
      if (made.status !== 201 && !landed) throw new Error(`note ${ref}: HTTP ${made.status} ${JSON.stringify(made.body)?.slice(0, 300)} ${made.stderr?.trim().slice(0, 300) ?? ""}`);
      await sleep(1500);
    }
    this.report.created.push(`${missing.length} note pull requests`);
  }

  /** The two stacked pull requests as one GitHub stack (REST `POST /repos/{o}/{r}/stacks`), once. */
  ensureStack(numbers) {
    if (numbers.some((n) => !n)) return null;
    const found = this.primary([`repos/${this.repo}/stacks?pull_request=${numbers[0]}`]).body;
    if (Array.isArray(found) && found[0]?.number) { this.report.reused.push(`stack ${found[0].number}`); return found[0].number; }
    const made = this.primary(["-X", "POST", `repos/${this.repo}/stacks`, "--input", "-"], { input: JSON.stringify({ pull_requests: numbers }) });
    if (made.status !== 201) { this.report.unavailable.stack = `HTTP ${made.status} ${made.body?.message ?? ""}`; return null; }
    this.report.created.push(`stack ${made.body.number} (#${numbers.join(", #")})`);
    return made.body.number;
  }

  /** What GitHub lets these accounts do here, asked rather than assumed. */
  probeCapabilities(numbers) {
    const any = numbers["open-clean"];
    const stacks = any ? this.primary([`repos/${this.repo}/stacks?pull_request=${any}`]) : { status: 0 };
    this.capabilities.stacks = stacks.status === 200 ? "available" : `unavailable (HTTP ${stacks.status} on repos/${this.repo}/stacks)`;
    this.capabilities.drafts = !this.report.unavailable.draft;
  }

  async run({ bulk = true } = {}) {
    setup(this.paths);
    if (!signedIn(this.paths, "primary")) throw new Error("primary is not signed in (README)");
    this.identities();
    this.capabilities = {};
    this.ensureRepo();
    this.ensureWork();
    this.ensureHistory();
    this.ensureProtection();
    this.ensureLabels();
    await this.ensureCollaborator();
    await this.ensureFork();
    this.listPulls();
    if (bulk) await this.ensureBulk();
    const numbers = {};
    this.heads = {};
    for (const spec of scenarios({ second: this.secondLogin })) numbers[spec.key] = await this.ensureScenario(spec);
    this.probeCapabilities(numbers);
    if (this.capabilities.stacks === "available") this.stacks = { seed: this.ensureStack([numbers["stack-bottom"], numbers["stack-top"]]) };
    const sandbox = { ...(readSandbox(this.paths) ?? {}), repo: this.repo, repoId: this.repoId, visibility: this.visibility, primary: this.owner, second: this.secondLogin ?? null, fork: this.fork ?? null,
      base: this.base, prs: numbers, stacks: this.stacks ?? null, capabilities: this.capabilities, unavailable: this.report.unavailable, seededAt: new Date().toISOString() };
    writeSandbox(this.paths, sandbox);
    return { sandbox, report: this.report };
  }
}

if (import.meta.main) {
  try {
    const visibility = option("--visibility") ?? "private";
    if (!["private", "public"].includes(visibility)) throw new Error("--visibility private|public");
    const seed = new Seed({ repo: option("--repo") ?? readSandbox(lanePaths())?.repo, create: flag("--create"), visibility });
    const { sandbox, report } = await seed.run({ bulk: !flag("--skip-bulk") });
    console.log(JSON.stringify({ repo: sandbox.repo, prs: sandbox.prs, capabilities: sandbox.capabilities, report }, null, 2));
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
