#!/usr/bin/env bun
// Sends each pull request RPC of the fake-github-fixture verb table to lane T3 servers working
// on the sandbox, and records what came back, which `gh` calls the server made for it, and, for
// every write, what GitHub says afterwards (read back with the lane gh, never with the server).
//
//   bun probe.mjs [--keep-servers]
//
// Writes go to the probe's own pull requests (`chore/check-<run>-<i>`, label `chore`), so the
// seeded ones keep their states. Output: <lane>/logs/probe-<run>.json and a Markdown table on stdout.
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { addProject, connectTo, start, stop, which } from "./lane.mjs";
import { api, ghCallsSince, ghLogSize, lanePaths, readSandbox, signedIn } from "./lib.mjs";

const paths = lanePaths();
const sandbox = readSandbox(paths) ?? { repo: "owner/name" };
const repo = sandbox.repo, [owner, name] = repo.split("/"), run = new Date().toISOString().replace(/\D/g, "").slice(0, 14);
const hasSecond = !!sandbox.second && signedIn(paths, "second");
const results = [];
const sleep = (ms) => Bun.sleep(ms);
const gh = (account, args, opts) => api(paths, account, args, opts);
const graphql = (query, vars = {}) => gh("primary", ["graphql", "-f", `query=${query}`, ...Object.entries(vars).flatMap(([k, v]) => [typeof v === "number" ? "-F" : "-f", `${k}=${v}`])]).body?.data;

/** A short name for a `gh` call: the subcommand, the REST path, or the GraphQL field it is about. */
const KEYWORDS = ["unmarkFileAsViewed", "markFileAsViewed", "addPullRequestReviewThreadReply", "unresolveReviewThread", "resolveReviewThread", "updatePullRequestReviewComment", "updateIssueComment",
  "updatePullRequestBranch", "updatePullRequest", "removeReaction", "addReaction", "revertPullRequest", "enablePullRequestAutoMerge", "disablePullRequestAutoMerge", "convertPullRequestToDraft", "markPullRequestReadyForReview",
  "REVIEW_DISMISSED_EVENT", "reviewThreads", "viewerCanUpdateBranch", "viewerDidAuthor", "assignableUsers", "suggestedReviewers", "labels(first", "files(first", "search(", "stackEntry", "s0: repository", "nodes(ids", "node(id", "viewerPermission", "statusCheckRollup", "pullRequest(number"];
export function verb(argv, repository = repo) {
  const text = argv.replaceAll(repository, "O/R").replace(/owner=\S+ name=\S+/, "");
  if (text.startsWith("api graphql")) {
    const found = KEYWORDS.find((word) => text.includes(word));
    return `graphql ${found ?? (text.includes("--input") || !text.includes("query=") ? "(stdin)" : "query")}`;
  }
  if (text.startsWith("api")) return text.replace(/--hostname \S+ /, "").replace(/-H \S+ /g, "").split(" ").filter((part) => !part.startsWith("-") || ["-X"].includes(part)).slice(0, 4).join(" ").slice(0, 90);
  return text.replace(/--repo \S+ /, "").replace(/--hostname \S+ /, "").slice(0, 70).trim();
}

/** One probe row: the call, its decoded summary or failure, its gh calls, and its read-back. */
async function probe(id, rpc, call, { summary = () => "", readback } = {}) {
  const offset = ghLogSize(paths), started = Date.now();
  const row = { id, rpc, ok: false, ms: 0, summary: "", error: "", gh: [], readback: null };
  try {
    const value = await call();
    row.ok = true; row.summary = String(summary(value) ?? "");
    row.value = value;
  } catch (error) { row.error = error.message; }
  row.ms = Date.now() - started;
  row.gh = [...new Set(ghCallsSince(paths, offset).filter((c) => c.config.endsWith("/gh") || c.config.endsWith("/gh-second")).map((c) => verb(c.argv)))];
  if (row.ok && readback) {
    await sleep(1500);
    try { const answer = await readback(row.value); row.readback = { ok: !!answer?.ok, detail: answer?.detail ?? "" }; }
    catch (error) { row.readback = { ok: false, detail: error.message }; }
  }
  delete row.value;
  results.push(row);
  console.error(`${row.ok ? (row.readback && !row.readback.ok ? "WARN" : "ok  ") : "FAIL"} ${id} ${rpc} ${row.ok ? row.summary : row.error}${row.readback ? ` | ${row.readback.detail}` : ""}`);
  return row;
}
const unavailable = (id, rpc, reason) => { results.push({ id, rpc, ok: null, ms: 0, summary: "", error: "", gh: [], readback: null, unavailable: reason }); console.error(`n/a  ${id} ${rpc} ${reason}`); };

/**
 * One of the probe's own pull requests (`chore/check-<run>-<i>`, label `chore`, neutral words only:
 * the repository is a plain playground), with a 12-line note, made through the REST API.
 */
let checks = 0;
function scratch(account, { from, draft = false, base = "main" } = {}) {
  const i = ++checks, id = `${run.slice(4, 12)}-${i}`, branch = `chore/check-${id}`, path = `checks/check-${id}.md`;
  const body = Array.from({ length: 12 }, (_, line) => `Line ${line + 1} of check note ${i}.`).join("\n") + "\n";
  gh(account, ["-X", "POST", `repos/${repo}/git/refs`, "-f", `ref=refs/heads/${branch}`, "-f", `sha=${from}`], { allowFail: false });
  gh(account, ["-X", "PUT", `repos/${repo}/contents/${path}`, "-f", `message=Add check note ${i}`, "-f", `content=${Buffer.from(body).toString("base64")}`, "-f", `branch=${branch}`], { allowFail: false });
  const made = gh(account, ["-X", "POST", `repos/${repo}/pulls`, "-f", `title=Add check note ${i}`, "-f", `head=${branch}`, "-f", `base=${base}`,
    "-f", `body=Adds check note ${i}.\n\n<!-- ref:check-${id} -->`, ...(draft ? ["-F", "draft=true"] : [])], { allowFail: false }).body;
  gh("primary", ["-X", "POST", `repos/${repo}/issues/${made.number}/labels`, "-f", "labels[]=chore"]);
  return { number: made.number, path, branch, i, nodeId: made.node_id };
}
/** main takes a merge only after `ci/build` passed (the seed's branch protection). */
const pass = (n) => gh("primary", ["-X", "POST", `repos/${repo}/statuses/${pull(n).head.sha}`, "-f", "state=success", "-f", "context=ci/build", "-f", "description=Build passed"], { allowFail: false });
const pull = (n) => gh("primary", [`repos/${repo}/pulls/${n}`], { allowFail: false }).body;
const mainSha = () => gh("primary", [`repos/${repo}/git/ref/heads/main`], { allowFail: false }).body.object.sha;

const logStart = ghLogSize(paths);
const conns = {}, started = [];
/** Closes the sockets and stops the servers this run started, whatever happened in between. */
function cleanup() {
  for (const conn of Object.values(conns)) conn.close();
  if (!process.argv.includes("--keep-servers")) for (const account of started) stop(account);
}
async function main() {
  if (!sandbox.prs) throw new Error("no seeded sandbox: run seed.mjs first");
  const projects = {};
  for (const account of hasSecond ? ["primary", "second"] : ["primary"]) {
    const server = await start(account);
    started.push(account);
    const project = await addProject(account);
    projects[account] = project.projectId;
    conns[account] = await connectTo(account);
    console.error(`${account}: server pid ${server.pid} port ${server.port}, project ${project.projectId}`);
    results.push({ id: `S-${account}`, rpc: "lane server", ok: true, ms: 0, summary: `${server.serverVersion ?? "running"} on ${server.port}; login shell resolves gh to the lane wrapper: ${which(account).gh === which(account).expectedGh}`, error: "", gh: [], readback: null });
  }
  const P = conns.primary, S = conns.second;
  const ref = (number, account = "primary") => ({ projectId: projects[account], repository: repo, number });
  const prs = sandbox.prs;

  // Reads -------------------------------------------------------------------------------------
  await probe("R1", "pullRequests.routingIdentity", () => P.request("pullRequests.routingIdentity", { host: "github.com" }), { summary: (v) => `viewer ${v.viewer}`, readback: async (v) => ({ ok: v.viewer === sandbox.primary, detail: `lane gh user = ${sandbox.primary}` }) });
  await probe("R2", "pullRequests.routing", () => P.request("pullRequests.routing", ref(prs["open-clean"])), { summary: (v) => `${v.provider} ${v.viewer} ${v.projectTitle}` });
  let page1 = null;
  await probe("R3", "pullRequests.list (open, 10 a slice)", async () => (page1 = await P.request("pullRequests.list", { state: "open", projectId: projects.primary, limit: 10 })),
    { summary: (v) => `${v.entries.length} rows, truncated ${v.truncated}, cursors ${Object.keys(v.nextCursors).length}, viewer ${Object.values(v.viewers)[0]}` });
  if (page1 && Object.keys(page1.nextCursors).length) {
    await probe("R4", "pullRequests.list (cursors: next slice)", () => P.request("pullRequests.list", { state: "open", projectId: projects.primary, limit: 10, cursors: page1.nextCursors }),
      { summary: (v) => `${v.entries.length} rows, none repeated: ${!v.entries.some((e) => page1.entries.some((f) => f.number === e.number))}` });
  }
  await probe("R5", "pullRequests.list (open, the page's 99)", () => P.request("pullRequests.list", { state: "open", projectId: projects.primary }),
    { summary: (v) => `${v.entries.length} rows, truncated ${v.truncated}` });
  await probe("R6", "pullRequests.list (search)", () => P.request("pullRequests.list", { state: "all", projectId: projects.primary, query: "changelog" }),
    { summary: (v) => `${v.entries.length} rows: ${v.entries.map((e) => `#${e.number}`).join(" ")}`, readback: async (v) => ({ ok: v.entries.some((e) => e.number === prs["open-clean"]), detail: `#${prs["open-clean"]} found` }) });
  await probe("R7", "pullRequests.listStats", () => P.request("pullRequests.listStats", { refs: (page1?.entries ?? []).slice(0, 10).map((e) => ({ projectId: e.projectId, host: e.host, repository: e.repository, number: e.number })) }),
    { summary: (v) => `${v.stats.length} stats` });
  for (const key of Object.keys(prs).filter((k) => prs[k])) {
    await probe(`D-${key}`, `pullRequests.detail (${key} #${prs[key]})`, () => P.request("pullRequests.detail", ref(prs[key])),
      { summary: (v) => `${v.isDraft ? "draft" : v.state}, ${v.mergeability}, base ${v.baseComparison}${v.behindBy ? ` by ${v.behindBy}` : ""}, ${v.checks?.length ?? 0} checks, head repo ${v.headRepositoryNameWithOwner ?? "-"}, actions [${v.viewerPermissions?.actions?.join(" ")}] verdicts [${v.viewerPermissions?.verdicts?.join(" ")}]` });
  }
  await probe("R8", "pullRequests.summary", () => P.request("pullRequests.summary", ref(prs["open-clean"])), { summary: (v) => `${v.state} ${v.title}` });
  await probe("R9", "pullRequests.preview", () => P.request("pullRequests.preview", ref(prs["failing"])), { summary: (v) => `${v.title ?? ""} ${v.state ?? ""}` });
  await probe("R10", "pullRequests.checks (failing)", () => P.request("pullRequests.checks", ref(prs["failing"])), { summary: (v) => (v?.checks ?? []).map((c) => `${c.name}:${c.status}`).join(" "),
    readback: async (v) => { const s = gh("primary", [`repos/${repo}/commits/${pull(prs.failing).head.sha}/status`]).body; return { ok: (v?.checks ?? []).length === s.statuses.length, detail: `GitHub: ${s.statuses.map((x) => `${x.context}:${x.state}`).join(" ")}` }; } });
  await probe("R11", "pullRequests.checks (running)", () => P.request("pullRequests.checks", ref(prs["running"])), { summary: (v) => (v?.checks ?? []).map((c) => `${c.name}:${c.status}`).join(" ") });
  let activity = null;
  if (prs["second-review"]) {
    await probe("R12", "pullRequests.activity (reviewed)", async () => (activity = await P.request("pullRequests.activity", ref(prs["second-review"]))),
      { summary: (v) => `${v.comments.length} remarks (${[...new Set(v.comments.map((c) => c.kind))].join(",")}), ${v.reviewThreads.length} threads (resolved ${v.reviewThreads.filter((t) => t.isResolved).length}, outdated ${v.reviewThreads.filter((t) => t.isOutdated).length}, paged ${v.reviewThreads.filter((t) => t.nextCommentsCursor).length}), ${v.commits.length} commits, reviewers ${v.reviewers?.map((r) => r.login).join(",")}, PR reactions ${v.reactions?.length ?? 0}` });
    const long = activity?.reviewThreads.find((t) => t.nextCommentsCursor);
    if (long) await probe("R13", "pullRequests.threadComments", () => P.request("pullRequests.threadComments", { ...ref(prs["second-review"]), threadId: long.id, cursor: long.nextCommentsCursor }),
      { summary: (v) => `${v.comments.length} more comments, next ${v.nextCursor ? "yes" : "none"}`, readback: async (v) => { const total = graphql("query($o:String!,$r:String!,$n:Int!){repository(owner:$o,name:$r){pullRequest(number:$n){reviewThreads(first:20){nodes{id comments{totalCount}}}}}}", { o: owner, r: name, n: prs["second-review"] })
        ?.repository.pullRequest.reviewThreads.nodes.find((t) => t.id === long.id)?.comments.totalCount; return { ok: long.comments.length + v.comments.length === total, detail: `${long.comments.length} + ${v.comments.length} of ${total} comments on GitHub` }; } });
    else unavailable("R13", "pullRequests.threadComments", "no thread with a next page in the activity");
  }
  await probe("R14", "pullRequests.stack (not stacked)", () => P.request("pullRequests.stack", ref(prs["open-clean"])), { summary: (v) => (v === null ? "null (no stack)" : `stack #${v.number}`) });
  if (prs["stack-bottom"]) await probe("R14b", "pullRequests.stack (seeded stack)", () => P.request("pullRequests.stack", ref(prs["stack-bottom"])), {
    summary: (v) => (v === null ? "null" : `stack ${v.number} on ${v.base}: ${v.layers.map((l) => `#${l.number} ${l.headBranch} ${l.state}`).join(", ")}`),
    readback: async (v) => { const s = gh("primary", [`repos/${repo}/stacks?pull_request=${prs["stack-bottom"]}`]).body?.[0]; return { ok: s?.number === v?.number, detail: `GitHub stack ${s?.number}` }; } });
  await probe("R15", "pullRequests.linkedThreads", () => P.request("pullRequests.linkedThreads", ref(prs["open-clean"])), { summary: (v) => `${v.threads.length} threads` });
  await probe("R16", "pullRequests.reviewerCandidates", () => P.request("pullRequests.reviewerCandidates", ref(prs["open-clean"])), { summary: (v) => (v.candidates ?? []).map((c) => `${c.login}${c.isRequested ? "*" : ""}`).join(" ") });
  await probe("R17", "pullRequests.labelCandidates", () => P.request("pullRequests.labelCandidates", ref(prs["open-clean"])), { summary: (v) => `${(v.candidates ?? []).length} labels, applied ${(v.candidates ?? []).filter((c) => c.isApplied).map((c) => c.name).join(",")}` });
  // The diff travels over HTTP; 310 files makes GitHub refuse `pr diff` (406), so the server pages the files API.
  await probe("R18", "POST /api/pull-requests/diff (310 files, every slice)", async () => {
    let cursor, slices = 0, files = 0;
    do { const slice = await P.http("/api/pull-requests/diff", { ...ref(prs["many-files"]), ...(cursor ? { cursor } : {}) }); slices++; files += (slice.patch.match(/^diff --git /gm) ?? []).length; cursor = slice.nextCursor ?? undefined; } while (cursor && slices < 10);
    return { slices, files };
  }, { summary: (v) => `${v.slices} slices, ${v.files} files`, readback: async (v) => { const n = pull(prs["many-files"]).changed_files; return { ok: v.files === n, detail: `GitHub changed_files ${n}` }; } });
  await probe("R19", "POST /api/pull-requests/diff (open-clean, one read)", () => P.http("/api/pull-requests/diff", ref(prs["open-clean"])), { summary: (v) => `${(v.patch.match(/^diff --git /gm) ?? []).length} files, next ${v.nextCursor ? "yes" : "none"}` });
  await probe("R20", "pullRequests.diffFileContents", () => P.request("pullRequests.diffFileContents", { ...ref(prs["primary-review"] ?? prs["open-clean"]), changeType: "change", oldPath: "docs/usage.md", newPath: "docs/usage.md" }),
    { summary: (v) => `old ${v.oldContents.length} chars, new ${v.newContents.length} chars` });
  await probe("R21", "pullRequests.filesViewed", () => P.request("pullRequests.filesViewed", ref(prs["open-clean"])), { summary: (v) => `${v.files.length} files with a state, truncated ${v.truncated}` });
  await probe("R22", "pullRequests.invalidate (one, then all)", async () => { await P.request("pullRequests.invalidate", { reference: ref(prs["open-clean"]) }); await P.request("pullRequests.invalidate", {}); return true; }, { summary: () => "void" });
  await probe("R23", "pullRequests.subscribeRefreshes", () => P.stream("pullRequests.subscribeRefreshes", {}, (values) => values.length >= 1, 15_000), { summary: (v) => `first value ${v[0]}, interrupted` });

  // Profiles: which actions each viewer gets, from GitHub's own answer for that account.
  if (S) {
    for (const [id, account, key, profile] of [["P1", "primary", "open-clean", "admin, author"], ["P2", "primary", "second-review", "admin, reviewer"], ["P3", "second", "open-clean", "write, reviewer"], ["P4", "second", "second-review", "write, author"], ["P5", "second", "cross-repo", "write, cross-repository author"]]) {
      if (!prs[key]) continue;
      await probe(id, `pullRequests.detail as ${profile}`, () => conns[account].request("pullRequests.detail", ref(prs[key], account)),
        { summary: (v) => `actions [${v.viewerPermissions.actions.join(" ")}] verdicts [${v.viewerPermissions.verdicts.join(" ")}] resolve ${v.viewerPermissions.resolve} labels ${v.viewerPermissions.labels} reviewers ${v.viewerPermissions.requestReviewers}` });
    }
  }

  // Writes on the probe's own pull requests ---------------------------------------------------
  const base = mainSha();
  // The probe's own stack, built now so the merges below leave it behind main for the stack rebase.
  let sb = null, st = null, stackNumber = null;
  if (sandbox.capabilities?.stacks === "available") {
    sb = scratch("primary", { from: base });
    st = scratch("primary", { from: pull(sb.number).head.sha, base: sb.branch });
    const made = gh("primary", ["-X", "POST", `repos/${repo}/stacks`, "--input", "-"], { input: JSON.stringify({ pull_requests: [sb.number, st.number] }) });
    stackNumber = made.status === 201 ? made.body.number : null;
    if (!stackNumber) console.error(`stack create: HTTP ${made.status} ${JSON.stringify(made.body)}`);
  }
  const w = scratch("primary", { from: base });
  const marker = `check-${run}`;
  await probe("W1", "pullRequests.comment", () => P.request("pullRequests.comment", { ...ref(w.number), body: `Thanks, this reads well.\n\n<!-- ref:${marker}-comment -->` }), {
    readback: async () => { const c = gh("primary", [`repos/${repo}/issues/${w.number}/comments`]).body; return { ok: c.some((x) => x.body.includes(`${marker}-comment`)), detail: `${c.length} comments on #${w.number}` }; } });
  const conv = await P.request("pullRequests.activity", ref(w.number)).catch(() => null);
  const mine = conv?.comments.find((c) => c.kind === "issue-comment" && c.body.includes(`${marker}-comment`));
  if (mine) {
    await probe("W2", "pullRequests.updateComment (issue comment)", () => P.request("pullRequests.updateComment", { ...ref(w.number), commentId: mine.id, kind: "issue-comment", body: `Thanks, this reads well (edited).\n\n<!-- ref:${marker}-comment -->` }), {
      readback: async () => { const c = gh("primary", [`repos/${repo}/issues/${w.number}/comments`]).body.find((x) => x.body.includes(`${marker}-comment`)); return { ok: c?.body.startsWith("Thanks, this reads well (edited)."), detail: `body now "${c?.body.split("\n")[0]}"` }; } });
    await probe("W3", "pullRequests.setReaction (comment, on)", () => P.request("pullRequests.setReaction", { ...ref(w.number), subjectId: mine.id, content: "thumbs-up", reacted: true }), {
      readback: async () => { const c = gh("primary", [`repos/${repo}/issues/${w.number}/comments`]).body.find((x) => x.body.includes(`${marker}-comment`)); return { ok: c?.reactions?.["+1"] === 1, detail: `+1 count ${c?.reactions?.["+1"]}` }; } });
  } else unavailable("W2", "pullRequests.updateComment", "the posted comment did not come back in the activity");
  await probe("W4", "pullRequests.setReaction (pull request, on then off)", async () => {
    await P.request("pullRequests.setReaction", { ...ref(w.number), content: "heart", reacted: true });
    await sleep(1500);
    const on = (gh("primary", [`repos/${repo}/issues/${w.number}/reactions`]).body ?? []).some((r) => r.content === "heart");
    await P.request("pullRequests.setReaction", { ...ref(w.number), content: "heart", reacted: false });
    return { on };
  }, { readback: async (v) => { const off = !(gh("primary", [`repos/${repo}/issues/${w.number}/reactions`]).body ?? []).some((r) => r.content === "heart"); return { ok: v.on && off, detail: `heart after on: ${v.on}, after off: ${!off}` }; } });
  await probe("W5", "pullRequests.update (title and body)", () => P.request("pullRequests.update", { ...ref(w.number), title: `Add check note ${w.i} (renamed)`, body: `Updated description.\n\n<!-- ref:${marker}-body -->` }), {
    readback: async () => { const p = pull(w.number); return { ok: p.title === `Add check note ${w.i} (renamed)` && p.body.includes(`${marker}-body`), detail: `title "${p.title}"` }; } });
  await probe("W6", "pullRequests.setLabels (apply, remove)", async () => {
    await P.request("pullRequests.setLabels", { ...ref(w.number), labels: ["priority:high"], applied: true }); await sleep(1500);
    const on = pull(w.number).labels.some((l) => l.name === "priority:high");
    await P.request("pullRequests.setLabels", { ...ref(w.number), labels: ["priority:high"], applied: false });
    return { on };
  }, { readback: async (v) => { const off = !pull(w.number).labels.some((l) => l.name === "priority:high"); return { ok: v.on && off, detail: `label on: ${v.on}, removed: ${off}` }; } });
  if (hasSecond) {
    const candidates = await P.request("pullRequests.reviewerCandidates", ref(w.number)).catch(() => ({ candidates: [] }));
    const who = candidates.candidates.find((c) => c.login?.toLowerCase() === sandbox.second.toLowerCase());
    if (who) await probe("W7", "pullRequests.requestReviewers (ask, take back)", async () => {
      await P.request("pullRequests.requestReviewers", { ...ref(w.number), reviewers: [{ id: who.id, kind: who.kind ?? "user" }], requested: true }); await sleep(1500);
      const asked = pull(w.number).requested_reviewers.some((r) => r.login === sandbox.second);
      await P.request("pullRequests.requestReviewers", { ...ref(w.number), reviewers: [{ id: who.id, kind: who.kind ?? "user" }], requested: false });
      return { asked };
    }, { readback: async (v) => { const gone = !pull(w.number).requested_reviewers.some((r) => r.login === sandbox.second); return { ok: v.asked && gone, detail: `${sandbox.second} requested: ${v.asked}, then removed: ${gone}` }; } });
    else unavailable("W7", "pullRequests.requestReviewers", `${sandbox.second} is not among the reviewer candidates`);
  } else unavailable("W7", "pullRequests.requestReviewers", "no second account");
  await probe("W8", "pullRequests.submitReview (own: comment, 2 line comments)", () => P.request("pullRequests.submitReview", { ...ref(w.number), verdict: "comment", body: `Two small notes.\n\n<!-- ref:${marker}-review -->`,
    comments: [{ path: w.path, position: { kind: "added", newLine: 2 }, body: "Small note on line 2." }, { path: w.path, position: { kind: "added", newLine: 5 }, body: "Small note on line 5." }] }), {
    readback: async () => { const r = gh("primary", [`repos/${repo}/pulls/${w.number}/reviews`]).body, c = gh("primary", [`repos/${repo}/pulls/${w.number}/comments`]).body;
      return { ok: r.some((x) => x.state === "COMMENTED" && x.body.includes(`${marker}-review`)) && c.filter((x) => x.path === w.path).map((x) => x.line).sort().join() === "2,5", detail: `review ${r.at(-1)?.state}, line comments at ${c.map((x) => x.line).join(",")}` }; } });
  const afterReview = await P.request("pullRequests.activity", ref(w.number)).catch(() => null);
  const thread = afterReview?.reviewThreads.find((t) => t.path === w.path && t.line === 2);
  if (thread) {
    await probe("W9", "pullRequests.replyToThread", () => P.request("pullRequests.replyToThread", { ...ref(w.number), threadId: thread.id, body: `Agreed.\n\n<!-- ref:${marker}-reply -->` }), {
      readback: async () => { const c = gh("primary", [`repos/${repo}/pulls/${w.number}/comments`]).body.find((x) => x.body.includes(`${marker}-reply`)); return { ok: !!c?.in_reply_to_id, detail: `reply in_reply_to ${c?.in_reply_to_id}` }; } });
    const threadState = () => graphql("query($o:String!,$r:String!,$n:Int!){repository(owner:$o,name:$r){pullRequest(number:$n){reviewThreads(first:20){nodes{id isResolved}}}}}", { o: owner, r: name, n: w.number })
      ?.repository.pullRequest.reviewThreads.nodes.find((t) => t.id === thread.id)?.isResolved;
    await probe("W10", "pullRequests.setThreadResolution (resolve)", () => P.request("pullRequests.setThreadResolution", { ...ref(w.number), threadId: thread.id, resolved: true }), { readback: async () => ({ ok: threadState() === true, detail: `isResolved ${threadState()}` }) });
    await probe("W11", "pullRequests.setThreadResolution (unresolve)", () => P.request("pullRequests.setThreadResolution", { ...ref(w.number), threadId: thread.id, resolved: false }), { readback: async () => ({ ok: threadState() === false, detail: `isResolved ${threadState()}` }) });
    const lineComment = thread.comments[0];
    await probe("W12", "pullRequests.updateComment (line comment)", () => P.request("pullRequests.updateComment", { ...ref(w.number), commentId: lineComment.id, kind: "review-comment", body: "Small note on line 2 (edited)." }), {
      readback: async () => { const c = gh("primary", [`repos/${repo}/pulls/${w.number}/comments`]).body.find((x) => x.line === 2 && !x.in_reply_to_id); return { ok: c?.body === "Small note on line 2 (edited).", detail: `body "${c?.body}"` }; } });
  } else unavailable("W9-W12", "replyToThread, setThreadResolution, updateComment (line)", "the review's thread did not come back in the activity");
  const viewed = () => graphql("query($o:String!,$r:String!,$n:Int!){repository(owner:$o,name:$r){pullRequest(number:$n){files(first:10){nodes{path viewerViewedState}}}}}", { o: owner, r: name, n: w.number })?.repository.pullRequest.files.nodes.find((f) => f.path === w.path)?.viewerViewedState;
  await probe("W13", "pullRequests.setFilesViewed (viewed)", () => P.request("pullRequests.setFilesViewed", { ...ref(w.number), files: [{ path: w.path, viewed: true }] }), { readback: async () => ({ ok: viewed() === "VIEWED", detail: `viewerViewedState ${viewed()}` }) });
  await probe("W14", "pullRequests.filesViewed (after)", () => P.request("pullRequests.filesViewed", ref(w.number)), { summary: (v) => v.files.map((f) => `${f.path.split("/").at(-1)}:${f.viewed ?? f.state ?? JSON.stringify(f)}`).join(" ") });
  await probe("W15", "pullRequests.setFilesViewed (unviewed)", () => P.request("pullRequests.setFilesViewed", { ...ref(w.number), files: [{ path: w.path, viewed: false }] }), { readback: async () => ({ ok: viewed() === "UNVIEWED", detail: `viewerViewedState ${viewed()}` }) });
  const state = (n) => { const p = pull(n); return p.merged_at ? "merged" : p.draft ? "draft" : p.state; };
  await probe("W16", "pullRequests.runAction close", () => P.request("pullRequests.runAction", { ...ref(w.number), action: "close" }), { readback: async () => ({ ok: state(w.number) === "closed", detail: `state ${state(w.number)}` }) });
  await probe("W17", "pullRequests.runAction reopen", () => P.request("pullRequests.runAction", { ...ref(w.number), action: "reopen" }), { readback: async () => ({ ok: state(w.number) === "open", detail: `state ${state(w.number)}` }) });
  if (sandbox.capabilities?.drafts === false) unavailable("W18-W19", "runAction draft / ready", `GitHub refused draft pull requests in this repository: ${sandbox.unavailable?.draft ?? ""}`);
  else {
    await probe("W18", "pullRequests.runAction draft", () => P.request("pullRequests.runAction", { ...ref(w.number), action: "draft" }), { readback: async () => ({ ok: state(w.number) === "draft", detail: `state ${state(w.number)}` }) });
    await probe("W19", "pullRequests.runAction ready", () => P.request("pullRequests.runAction", { ...ref(w.number), action: "ready" }), { readback: async () => ({ ok: state(w.number) === "open", detail: `state ${state(w.number)}` }) });
  }
  const detailOf = (n) => P.request("pullRequests.detail", { ...ref(n), allowStale: false });
  await probe("W20", "pullRequests.runAction merge (merge)", async () => { pass(w.number); await sleep(2000); await detailOf(w.number); return P.request("pullRequests.runAction", { ...ref(w.number), action: "merge", mergeMethod: "merge" }); }, {
    readback: async () => { const p = pull(w.number), c = p.merge_commit_sha ? gh("primary", [`repos/${repo}/commits/${p.merge_commit_sha}`]).body : null; return { ok: !!p.merged_at && c?.parents?.length === 2, detail: `merged ${!!p.merged_at}, merge commit parents ${c?.parents?.length}` }; } });
  const sq = scratch("primary", { from: mainSha() });
  pass(sq.number);
  await probe("W21", "pullRequests.runAction merge (squash)", async () => { await sleep(3000); return P.request("pullRequests.runAction", { ...ref(sq.number), action: "merge", mergeMethod: "squash" }); }, {
    readback: async () => { const p = pull(sq.number), c = p.merge_commit_sha ? gh("primary", [`repos/${repo}/commits/${p.merge_commit_sha}`]).body : null; return { ok: !!p.merged_at && c?.parents?.length === 1, detail: `merged ${!!p.merged_at}, parents ${c?.parents?.length}` }; } });
  const rb = scratch("primary", { from: mainSha() });
  pass(rb.number);
  await probe("W22", "pullRequests.runAction merge (rebase)", async () => { await sleep(3000); return P.request("pullRequests.runAction", { ...ref(rb.number), action: "merge", mergeMethod: "rebase" }); }, {
    readback: async () => { const p = pull(rb.number); return { ok: !!p.merged_at, detail: `merged ${!!p.merged_at}` }; } });
  for (const [id, method] of [["W23", "merge"], ["W24", "rebase"]]) {
    const behind = scratch("primary", { from: sandbox.base.c4 });
    const compare = () => gh("primary", [`repos/${repo}/compare/main...${behind.branch}`]).body?.behind_by;
    await probe(id, `pullRequests.runAction update-branch (${method})`, async () => { await sleep(2000); const d = await detailOf(behind.number); if (!d.viewerPermissions.actions.includes("update-branch")) throw new Error(`not offered: base ${d.baseComparison}`); return P.request("pullRequests.runAction", { ...ref(behind.number), action: "update-branch", updateMethod: method }); }, {
      readback: async () => { await sleep(3000); return { ok: compare() === 0, detail: `behind_by ${compare()}` }; } });
    gh("primary", ["-X", "PATCH", `repos/${repo}/pulls/${behind.number}`, "-f", "state=closed"]);
  }
  const au = scratch("primary", { from: mainSha() });
  const autoState = () => graphql("query($o:String!,$r:String!,$n:Int!){repository(owner:$o,name:$r){pullRequest(number:$n){autoMergeRequest{mergeMethod}}}}", { o: owner, r: name, n: au.number })?.repository.pullRequest.autoMergeRequest;
  const auto = await probe("W25", "pullRequests.runAction enable-auto-merge", () => P.request("pullRequests.runAction", { ...ref(au.number), action: "enable-auto-merge", mergeMethod: "squash" }), { readback: async () => ({ ok: !!autoState(), detail: `autoMergeRequest ${JSON.stringify(autoState())}` }) });
  if (auto.ok) await probe("W26", "pullRequests.runAction disable-auto-merge", () => P.request("pullRequests.runAction", { ...ref(au.number), action: "disable-auto-merge" }), { readback: async () => ({ ok: autoState() === null, detail: `autoMergeRequest ${JSON.stringify(autoState())}` }) });
  else unavailable("W26", "runAction disable-auto-merge", "nothing armed: enable-auto-merge was refused (W25)");
  if (state(au.number) === "open") gh("primary", ["-X", "PATCH", `repos/${repo}/pulls/${au.number}`, "-f", "state=closed"]);
  const before = new Set(gh("primary", [`repos/${repo}/pulls?state=open&per_page=100&sort=created&direction=desc`]).body.map((p) => p.number));
  await probe("W27", "pullRequests.runAction revert", () => P.request("pullRequests.runAction", { ...ref(sq.number), action: "revert" }), {
    readback: async () => { const opened = gh("primary", [`repos/${repo}/pulls?state=open&per_page=20&sort=created&direction=desc`]).body.find((p) => !before.has(p.number) && /^Revert/.test(p.title));
      if (opened) gh("primary", ["-X", "PATCH", `repos/${repo}/pulls/${opened.number}`, "-f", "state=closed"]);
      return { ok: !!opened, detail: opened ? `#${opened.number} "${opened.title}" opened (closed again by the probe)` : "no revert pull request" }; } });
  if (prs["cross-repo"]) await probe("W28", "pullRequests.runAction approve-workflows (cross-repository)", () => P.request("pullRequests.runAction", { ...ref(prs["cross-repo"]), action: "approve-workflows" }), {
    readback: async () => { const runs = gh("primary", [`repos/${repo}/actions/runs?status=action_required`]).body; return { ok: true, detail: `nothing to approve: ${runs?.total_count ?? 0} runs await approval (the playground has no workflow, and GitHub asks approval only for outside or first-time contributors; the second account is a collaborator)` }; } });
  if (hasSecond) {
    const theirs = scratch("second", { from: mainSha() });
    await probe("W29", "pullRequests.submitReview (request-changes, 2 line comments)", () => P.request("pullRequests.submitReview", { ...ref(theirs.number), verdict: "request-changes", body: `Please change two lines.\n\n<!-- ref:${marker}-rc -->`,
      comments: [{ path: theirs.path, position: { kind: "added", newLine: 3 }, body: "Line 3 needs a change." }, { path: theirs.path, position: { kind: "added", newLine: 7 }, body: "Line 7 needs a change." }] }), {
      readback: async () => { const r = gh("primary", [`repos/${repo}/pulls/${theirs.number}/reviews`]).body, c = gh("primary", [`repos/${repo}/pulls/${theirs.number}/comments`]).body;
        return { ok: r.some((x) => x.state === "CHANGES_REQUESTED") && c.length === 2, detail: `review ${r.at(-1)?.state}, ${c.length} line comments at ${c.map((x) => x.line).join(",")}` }; } });
    await probe("W30", "pullRequests.submitReview (approve)", () => P.request("pullRequests.submitReview", { ...ref(theirs.number), verdict: "approve", body: "", comments: [] }), {
      readback: async () => { const r = gh("primary", [`repos/${repo}/pulls/${theirs.number}/reviews`]).body; return { ok: r.at(-1)?.state === "APPROVED", detail: `latest review ${r.at(-1)?.state}` }; } });
    gh("second", ["-X", "PATCH", `repos/${repo}/pulls/${theirs.number}`, "-f", "state=closed"]);
  }
  if (stackNumber) {
    const heads = async () => (await P.request("pullRequests.stack", ref(st.number))).layers.filter((l) => l.state !== "merged").map((l) => ({ number: l.number, headSha: l.headSha }));
    const behindBy = () => gh("primary", [`repos/${repo}/compare/main...${sb.branch}`]).body?.behind_by;
    const before = behindBy();
    await probe("W31", "pullRequests.runAction update-branch (stack rebase)", async () => P.request("pullRequests.runAction", { ...ref(st.number), action: "update-branch", updateMethod: "rebase", stackNumber, expectedStackHeads: await heads() }), {
      readback: async () => { await sleep(4000); const b = behindBy(), top = pull(st.number), bottom = pull(sb.number); return { ok: before > 0 && b === 0 && top.base.ref === sb.branch, detail: `bottom behind main ${before} → ${b}; top still on ${top.base.ref}; heads ${bottom.head.sha.slice(0, 7)}, ${top.head.sha.slice(0, 7)}` }; } });
    pass(sb.number); pass(st.number); await sleep(2000);
    await probe("W32", "pullRequests.runAction merge (whole stack, merge-async)", async () => P.request("pullRequests.runAction", { ...ref(st.number), action: "merge", mergeMethod: "squash", stackNumber, expectedStackHeads: await heads() }), {
      readback: async () => { for (let i = 0; i < 30 && !(pull(sb.number).merged_at && pull(st.number).merged_at); i++) await sleep(2000); return { ok: !!pull(sb.number).merged_at && !!pull(st.number).merged_at, detail: `#${sb.number} merged ${!!pull(sb.number).merged_at}, #${st.number} merged ${!!pull(st.number).merged_at}` }; } });
  } else unavailable("W31-W32", "runAction with stackNumber (stack rebase, stack merge)", `GitHub stacks: ${sandbox.capabilities?.stacks ?? "unknown"}`);

  return results;
}

export function table(rows) {
  const cell = (text) => String(text ?? "").replaceAll("|", "\\|").replaceAll("\n", " ");
  const lines = ["| # | RPC | Result | `gh` calls the server made | GitHub read back |", "| --- | --- | --- | --- | --- |"];
  for (const r of rows) {
    const result = r.unavailable ? `not producible: ${r.unavailable}` : r.ok ? `decoded${r.summary ? `: ${r.summary}` : ""} (${r.ms} ms)` : `error: ${r.error}`;
    const back = r.readback ? `${r.readback.ok ? "confirmed" : "MISMATCH"}: ${r.readback.detail}` : "";
    lines.push(`| ${r.id} | ${cell(r.rpc)} | ${cell(result)} | ${cell(r.gh.join("; "))} | ${cell(back)} |`);
  }
  return lines.join("\n");
}

if (import.meta.main) {
  const rows = await main().catch((error) => { console.error(error.stack ?? error.message); process.exitCode = 1; return results; }).finally(cleanup);
  const file = join(paths.logs, `probe-${run}.json`);
  const calls = ghCallsSince(paths, logStart);
  const configs = [...new Set(calls.map((c) => c.config))];
  writeFileSync(file, `${JSON.stringify({ run, repo, primary: sandbox.primary, second: sandbox.second, ghCalls: calls.length, ghConfigs: configs, rows }, null, 2)}\n`);
  console.log(`gh calls during the run: ${calls.length}, config dirs: ${configs.join(", ")}\n`);
  console.log(table(rows));
  console.log(`\n${rows.filter((r) => r.ok).length} decoded, ${rows.filter((r) => r.ok === false).length} errors, ${rows.filter((r) => r.readback && !r.readback.ok).length} read-back mismatches, ${rows.filter((r) => r.unavailable).length} not producible; ${file}`);
}
