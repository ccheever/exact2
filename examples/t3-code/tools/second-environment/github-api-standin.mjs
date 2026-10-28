#!/usr/bin/env bun
// GitHub's API for a lane server, answering for the `gh` stand-in's pull request (gh-standin.mjs). The
// server reads pull requests over HTTPS with a token (packages/source-control-github, gitHubApiUrls:
// `https://api.github.com` and `/graphql` for github.com), through the proxy its environment names.
// This process is that proxy: it tunnels CONNECT to api.github.com:443 into its own HTTPS server, whose
// certificate the lane's CA signs (the server trusts it by NODE_EXTRA_CA_CERTS), and refuses every
// other host, as the closed offline port does. Each request is logged with its answer's status.
//
//   bun github-api-standin.mjs <proxy port> <api port> <cert dir> <log file>
import { appendFileSync, readFileSync } from "node:fs";
import { createServer as createHttpServer } from "node:http";
import { createServer as createHttpsServer } from "node:https";
import { connect } from "node:net";
import { join } from "node:path";
import { PULL_REQUEST } from "./gh-standin.mjs";
import { DETAIL_OPERATIONS, detailRest, detailWrite } from "./github-api-detail.mjs";

export const API_HOST = "api.github.com";

const pr = PULL_REQUEST;
const url = `https://github.com/${pr.owner}/${pr.name}/pull/${pr.number}`;
const ours = (v) => v.owner === pr.owner && v.name === pr.name;
/** packages/source-control-github's PullRequestNode fragment. */
const node = () => ({ number: pr.number, title: pr.title, url, baseRefName: pr.baseRefName, headRefName: pr.headRefName, headRefOid: "0".repeat(40),
  state: "OPEN", isDraft: false, mergedAt: null, closedAt: null, updatedAt: pr.updatedAt, isCrossRepository: false,
  headRepository: { name: pr.name, nameWithOwner: `${pr.owner}/${pr.name}` }, headRepositoryOwner: { __typename: "User", login: pr.owner } });

/** The counts of the detail's patch and file stats (github-api-detail.mjs). */
const COUNTS = { additions: 203, deletions: 1, changedFiles: 3 };
/** The PullRequestSummary query's pull request: open, mergeable, its checks passing. */
const summary = () => ({ number: pr.number, title: pr.title, url, state: "OPEN", isDraft: false, mergeable: "MERGEABLE", reviewDecision: null,
  ...COUNTS, updatedAt: pr.updatedAt, mergedAt: null, closedAt: null, headRefName: pr.headRefName, baseRefName: pr.baseRefName,
  author: { __typename: "User", login: pr.author, avatarUrl: "https://avatars.githubusercontent.com/u/1", name: "Lane Fixture" },
  latestReviews: { nodes: [] }, commits: { nodes: [{ commit: { statusCheckRollup: { state: "SUCCESS" } } }] }, stack: null, stackEntry: null });

/**
 * The Pull Requests page's listing row (pullRequestListings.graphql PullRequestRow): the counts only with `$stats`,
 * as the cross-repository search reads them afterwards (PullRequestStats).
 */
const listRow = (v) => ({ number: pr.number, title: pr.title, url, author: { __typename: "User", login: pr.author, avatarUrl: "https://avatars.githubusercontent.com/u/1", name: "Lane Fixture" },
  headRefName: pr.headRefName, baseRefName: pr.baseRefName, state: "OPEN", isDraft: false, mergeable: "MERGEABLE", reviewDecision: null, latestReviews: { nodes: [] },
  ...(v.stats ? { additions: COUNTS.additions, deletions: COUNTS.deletions } : {}), createdAt: pr.createdAt, updatedAt: pr.updatedAt, mergedAt: null,
  repository: { nameWithOwner: `${pr.owner}/${pr.name}` }, reviewRequests: { nodes: [] }, labels: { nodes: [] },
  commits: { nodes: [{ commit: { statusCheckRollup: { state: "SUCCESS" } } }] }, ...(v.stacks ? { stack: null, stackEntry: null } : {}) });
const noMore = { hasNextPage: false, endCursor: null };
/** A search names its repositories (`repo:owner/name`) and its states; the one open pull request answers an open or any-state one. */
const searched = (q) => String(q).toLowerCase().includes(`repo:${pr.owner}/${pr.name}`) && !/\bis:(closed|merged)\b/i.test(String(q));

/** The repository's labels and the people one can name in it (RepositoryLabels, RepositoryAuthors: Filters › Labels and
 *  Author search the host), each narrowed by `$query` on its name or login, as GitHub's own search narrows them. */
const LABELS = [{ name: "bug", color: "d73a4a" }, { name: "documentation", color: "0075ca" }, { name: "enhancement", color: "a2eeef" }, { name: "good first issue", color: "7057ff" }];
const PEOPLE = [{ login: pr.author, name: "Lane Fixture" }, { login: "lane-reviewer", name: "Lane Reviewer" }, { login: "octo-hostonly", name: "Octo Host" }]
  .map((person) => ({ ...person, avatarUrl: "https://avatars.githubusercontent.com/u/1" }));
const narrowed = (query, texts) => !query || texts.some((text) => String(text ?? "").toLowerCase().includes(String(query).toLowerCase()));

/** The GraphQL operations the server sends, by name: each answers `data` from the variables, or null for none. */
const OPERATIONS = {
  RepositoryLabels: (v) => ({ repository: ours(v) ? { labels: { pageInfo: { hasNextPage: false }, nodes: LABELS.filter((label) => narrowed(v.query, [label.name])) } } : null }),
  RepositoryAuthors: (v) => ({ repository: ours(v) ? { mentionableUsers: { pageInfo: { hasNextPage: false }, nodes: PEOPLE.filter((person) => narrowed(v.query, [person.login, person.name])) } } : null }),
  PullRequestsByHead: (v) => ({ repository: ours(v) ? { pullRequests: { nodes: v.head === pr.headRefName ? [node()] : [] } } : null }),
  PullRequestByNumber: (v) => ({ repository: ours(v) ? { pullRequest: Number(v.number) === pr.number ? node() : null } : null }),
  PullRequestSummary: (v) => ({ repository: ours(v) ? { pullRequest: Number(v.number) === pr.number ? summary() : null } : null }),
  PullRequestSearch: (v) => ({ search: { pageInfo: noMore, nodes: searched(v.q) ? [listRow(v)] : [] } }),
  PullRequestList: (v) => ({ repository: ours(v) ? { pullRequests: { pageInfo: noMore, nodes: (v.states ?? ["OPEN"]).includes("OPEN") ? [listRow(v)] : [] } } : null }),
  PullRequestStats: (v) => ({ repository: ours(v) ? { pullRequest: Number(v.number) === pr.number ? { additions: COUNTS.additions, deletions: COUNTS.deletions } : null } : null }),
  ...DETAIL_OPERATIONS,
};
const REST = { "/user": () => ({ id: 1, login: pr.author }) };

/**
 * The server batches lookups into one document (HEAD_LOOKUPS_PER_DOCUMENT): each is aliased `_N_repository`
 * with its variables prefixed `_N_`. Answers each alias from its own variables, or the plain field.
 */
function batched(query, variables, operation) {
  const prefixes = [...query.matchAll(/(_\d+_)repository:repository\b/g)].map((match) => match[1]);
  if (!prefixes.length) return operation(variables);
  return Object.fromEntries(prefixes.map((prefix) => [`${prefix}repository`, operation(Object.fromEntries(Object.entries(variables)
    .filter(([key]) => key.startsWith(prefix)).map(([key, value]) => [key.slice(prefix.length), value]))).repository]));
}

/** An operation's name: `query Name(…)` or `mutation Name(…)`. */
export const operationOf = (query) => /^\s*(?:query|mutation)\s+(\w+)/.exec(query)?.[1] ?? "";

/** The HTTP answer to one API request: { status, type, body, name }. `accept` is the request's Accept header. */
export function respond(method, path, body, accept = "") {
  const json = (status, value, name = "") => ({ status, type: "application/json", body: JSON.stringify(value), name });
  const { pathname } = new URL(path, `https://${API_HOST}`);
  if (method === "POST" && pathname === "/graphql") {
    const { query = "", variables = {} } = JSON.parse(body || "{}"), name = operationOf(query);
    const data = OPERATIONS[name] && batched(query, variables, OPERATIONS[name]);
    // GraphQL reports a failure in a 200 answer's `errors`.
    return data ? json(200, { data }, name) : json(200, { errors: [{ type: "NOT_SUPPORTED", message: `the lane's GitHub stand-in does not answer ${name || "this query"}` }] }, name || query.slice(0, 160));
  }
  const write = detailWrite(method, pathname, body);
  if (write !== null) return json(200, write.value, write.name);
  const patch = detailRest(method, pathname, accept);
  if (patch !== null) return { status: 200, type: "text/plain; charset=utf-8", body: patch, name: "diff" };
  const rest = method === "GET" ? REST[pathname] : undefined;
  return rest ? json(200, rest()) : json(404, { message: "Not Found" });
}

if (import.meta.main) {
  const [proxyPort, apiPort, certDir, log] = process.argv.slice(2);
  const note = (line) => appendFileSync(log, `${new Date().toISOString()}\t${line}\n`);
  const api = createHttpsServer({ key: readFileSync(join(certDir, "api.key")), cert: readFileSync(join(certDir, "api.crt")) }, (request, response) => {
    let body = "";
    request.on("data", (chunk) => { body += chunk; });
    request.on("end", () => {
      const answered = respond(request.method ?? "GET", request.url ?? "/", body, String(request.headers.accept ?? ""));
      note(`${answered.status}\t${request.method} ${request.url}${answered.name ? ` ${answered.name}` : ""}`);
      response.writeHead(answered.status, { "content-type": answered.type });
      response.end(answered.body);
    });
  });
  api.listen(Number(apiPort), "127.0.0.1");
  const proxy = createHttpServer((request, response) => { note(`403\tproxy ${request.method} ${request.url}`); response.writeHead(403).end(); });
  proxy.on("connect", (request, socket, head) => {
    if (request.url !== `${API_HOST}:443`) { note(`403\tCONNECT ${request.url}`); socket.end("HTTP/1.1 403 Forbidden\r\n\r\n"); return; }
    const upstream = connect(Number(apiPort), "127.0.0.1", () => {
      socket.write("HTTP/1.1 200 Connection Established\r\n\r\n");
      if (head?.length) upstream.write(head);
      upstream.pipe(socket); socket.pipe(upstream);
    });
    upstream.on("error", () => socket.destroy());
    socket.on("error", () => upstream.destroy());
  });
  proxy.listen(Number(proxyPort), "127.0.0.1");
}
