#!/usr/bin/env bun
// A `gh` stand-in that knows one open pull request, so a lane thread on its branch has a linked
// pull request (the sidebar's glyph, the chat header's link, the detail panel with its actions
// menu, Merge and one review thread to reply to) without GitHub. It answers the calls a T3 server makes for those (reference
// apps/server/src/sourceControl/GitHubCli.ts and GitHubPullRequestCli.ts, the shapes
// pullRequest/gitHubPullRequestJson.ts decodes) and fails every other one, as a gh that cannot
// reach GitHub would. It merges nothing and contacts nothing.
//
//   bin/gh (lib.mjs `installGhStandin`) runs: bun gh-standin.mjs <gh arguments>
import { appendFileSync, readFileSync } from "node:fs";

export const PULL_REQUEST = {
  owner: "t3-fixture", name: "shared-notes", number: 7, title: "Keep the notes in one place",
  headRefName: "feature/notes", baseRefName: "main", author: "lane-fixture",
  createdAt: "2026-10-01T09:00:00Z", updatedAt: "2026-10-02T09:00:00Z",
  body: "Moves the scattered notes into `notes/` and links them from the README.",
};
const pr = PULL_REQUEST;
const url = `https://github.com/${pr.owner}/${pr.name}/pull/${pr.number}`;
const author = { __typename: "User", login: pr.author, name: "Lane Fixture" };
const listItem = () => ({
  number: pr.number, title: pr.title, url, baseRefName: pr.baseRefName, headRefName: pr.headRefName, state: "OPEN", isDraft: false,
  mergedAt: null, closedAt: null, createdAt: pr.createdAt, updatedAt: pr.updatedAt, isCrossRepository: false,
  headRepository: { name: pr.name, nameWithOwner: `${pr.owner}/${pr.name}` }, headRepositoryOwner: { login: pr.owner },
});
const counts = { additions: 12, deletions: 3, changedFiles: 2 };
const summary = () => ({ ...listItem(), ...counts, mergeable: "MERGEABLE", reviewDecision: null, author,
  latestReviews: { nodes: [] }, commits: { nodes: [{ commit: { statusCheckRollup: { state: "SUCCESS" } } }] }, stack: null, stackEntry: null });
const detail = () => ({ data: { repository: { mergeCommitAllowed: true, squashMergeAllowed: true, rebaseMergeAllowed: true, viewerPermission: "WRITE",
  pullRequest: { ...listItem(), ...counts, author, body: pr.body, mergeable: "MERGEABLE", reviewDecision: null, headRefOid: "0".repeat(40), autoMergeRequest: null,
    viewerCanUpdate: true, viewerDidAuthor: false, viewerCanUpdateBranch: false, baseRef: null,
    reviewRequests: { nodes: [] }, labels: { nodes: [] }, commits: { nodes: [{ commit: { statusCheckRollup: null } }] } } } } });

/** One unresolved review thread on README.md, so the detail panel's conversation has a Reply. */
const reviewThreads = () => ({ data: { viewer: { login: pr.author }, repository: { pullRequest: {
  author, viewerCanUpdate: true, viewerDidAuthor: false, reactionGroups: [], comments: { nodes: [] }, reviews: { nodes: [] },
  reviewThreads: { totalCount: 1, pageInfo: { hasNextPage: false, endCursor: null }, nodes: [{
    id: "lane-thread-1", isResolved: false, isOutdated: false, path: "README.md", line: 3, diffSide: "RIGHT",
    comments: { totalCount: 1, pageInfo: { hasNextPage: false, endCursor: null }, nodes: [{
      id: "lane-comment-1", author: { __typename: "User", login: "lane-reviewer", name: "Lane Reviewer" }, body: "Should this line link to notes/plan.md?",
      createdAt: "2026-10-02T10:00:00Z", url: `${url}#discussion_r1`, reactionGroups: [] }] } }] } } } } });

/** What the stand-in answers for one invocation: { out, code, err }. `stdin` is read only for `--input -`. */
export function answer(argv, stdin = () => "") {
  const ok = (value) => ({ out: typeof value === "string" ? value : JSON.stringify(value), code: 0 });
  const refuse = (err = "the lane's gh stand-in does not answer this call") => ({ out: "", code: 1, err });
  const fields = new Map();
  for (let i = 0; i < argv.length - 1; i++) if (argv[i] === "-f" || argv[i] === "-F") { const [key, ...rest] = argv[i + 1].split("="); fields.set(key, rest.join("=")); }
  const ours = (owner, name) => owner === pr.owner && name === pr.name;
  const [first, second] = argv;
  if (first === "--version") return ok("gh version 2.81.0 (lane fixture stand-in)\n");
  if (first === "auth" && second === "status") return ok({ hosts: { "github.com": [{ state: "success", active: true, host: "github.com", login: pr.author }] } });
  // The server passes this to its later gh calls as GH_TOKEN; it only ever reaches this stand-in.
  if (first === "auth" && second === "token") return ok("lane-fixture-placeholder-token\n");
  if (first === "api" && argv.includes("user")) return ok({ id: 1, login: pr.author });
  if (first === "api" && argv.some((arg) => arg.startsWith(`repos/${pr.owner}/${pr.name}/stacks`))) return ok("[]");
  if (first === "api" && second === "graphql") {
    if (argv.includes("--input")) {
      const body = JSON.parse(stdin() || "{}"), variables = body.variables ?? {};
      if (!String(body.query).includes("PullRequestsByHead")) return refuse();
      const aliases = Object.keys(variables).filter((key) => /^h\d+$/.test(key));
      return ok({ data: { repository: Object.fromEntries(aliases.map((alias) => [alias, { nodes: ours(variables.owner, variables.name) && variables[alias] === pr.headRefName ? [listItem()] : [] }])) } });
    }
    const query = fields.get("query") ?? "";
    if (/^query\s*\{\s*rateLimit/.test(query)) return ok({ data: { rateLimit: { cost: 1, limit: 5000, remaining: 4999, resetAt: new Date(Date.now() + 3_600_000).toISOString() } } });
    if (query.includes("PullRequestSummaries")) {
      const data = {};
      for (const match of query.matchAll(/(s\d+): repository\(owner: "([^"]+)", name: "([^"]+)"\) \{ pullRequest\(number: (\d+)\)/g))
        data[match[1]] = { pullRequest: ours(match[2], match[3]) && Number(match[4]) === pr.number ? summary() : null };
      return ok({ data });
    }
    const asked = ours(fields.get("owner"), fields.get("name")) && Number(fields.get("number")) === pr.number;
    if (asked && query.includes("mergeCommitAllowed")) return ok(detail());
    if (asked && query.includes("reviewThreads(first")) return ok(reviewThreads());
    return refuse();
  }
  if (first === "pr" && second === "list") {
    // By head (a thread's branch), or a repository's listing (the Pull Requests page) in a state that holds an open one.
    const value = (name) => (argv.includes(name) ? argv[argv.indexOf(name) + 1] : undefined);
    const repo = value("--repo"), head = value("--head"), state = value("--state") ?? "open";
    const listed = head !== undefined ? head === pr.headRefName : (repo === undefined || repo.replace(/^github\.com\//, "") === `${pr.owner}/${pr.name}`) && ["open", "all"].includes(state);
    return ok(listed ? [{ ...listItem(), ...counts, author, mergeable: "MERGEABLE", reviewDecision: null, reviewRequests: [], latestReviews: [], labels: [], statusCheckRollup: [] }] : []);
  }
  if (first === "pr" && second === "view" && Number(argv[2]) === pr.number) {
    const json = argv[argv.indexOf("--json") + 1] ?? "";
    if (json.split(",").includes("comments")) return ok({ author, comments: [], reviews: [], commits: [] });
    return ok({ ...listItem(), ...counts, author, body: pr.body, mergeable: "MERGEABLE", reviewDecision: null, reviewRequests: [], latestReviews: [], labels: [], statusCheckRollup: [], headRefOid: "0".repeat(40), autoMergeRequest: null });
  }
  return refuse();
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const result = answer(argv, () => readFileSync(0, "utf8"));
  if (process.env.T3_FIXTURE_GH_LOG) appendFileSync(process.env.T3_FIXTURE_GH_LOG, `${result.code}\t${argv.map((arg) => arg.slice(0, 120).replace(/\s+/g, " ")).join(" ")}\n`);
  if (result.out) process.stdout.write(result.out);
  if (result.err) process.stderr.write(`${result.err}\n`);
  process.exit(result.code);
}
