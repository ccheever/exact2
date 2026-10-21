// The detail panel's reads and writes for the GitHub API stand-in (github-api-standin.mjs): the pull request's
// core detail with its checks, its activity and review threads, the files viewed, the node ids a write first
// resolves, Comment, Close and Reopen, and the patch (GET pulls/<n> as application/vnd.github.diff). The shapes are
// the selections of the pinned server's queries (reference packages/source-control-github/src/server/
// pullRequestReads.graphql, pullRequestReviews.graphql, pullRequestMutations.graphql; extra fields an older
// runtime does not ask for are harmless). A comment and a close change
// what the next read answers, for this process's life; nothing reaches GitHub.
import { PULL_REQUEST } from "./gh-standin.mjs";

const pr = PULL_REQUEST;
const url = `https://github.com/${pr.owner}/${pr.name}/pull/${pr.number}`;
const ours = (v) => v.owner === pr.owner && v.name === pr.name && Number(v.number) === pr.number;
const actor = (login, name = null) => ({ __typename: "User", login, avatarUrl: "https://avatars.githubusercontent.com/u/1", name, id: `U_${login}` });

/** What the session's writes changed: the comments posted, and whether the pull request is closed. */
const live = { comments: [], closedAt: null };
export const resetDetail = () => { live.comments = []; live.closedAt = null; };

/**
 * Checks of every kind the checks popover draws: passed and skipped (each with its run), running and failed, and a
 * status context without a link, so the label and Details columns line up across rows that differ.
 */
const contexts = () => [
  { __typename: "CheckRun", name: "build", status: "COMPLETED", conclusion: "SUCCESS", startedAt: "2026-10-02T09:01:00Z", completedAt: "2026-10-02T09:05:00Z", detailsUrl: `${url}/checks?run=1`, isRequired: false, checkSuite: { workflowRun: { workflow: { name: "CI" } } } },
  { __typename: "CheckRun", name: "lint", status: "COMPLETED", conclusion: "SKIPPED", startedAt: "2026-10-02T09:01:00Z", completedAt: "2026-10-02T09:01:00Z", detailsUrl: `${url}/checks?run=2`, isRequired: false, checkSuite: { workflowRun: { workflow: { name: "CI" } } } },
  { __typename: "CheckRun", name: "test (macos)", status: "IN_PROGRESS", conclusion: null, startedAt: "2026-10-02T09:02:00Z", completedAt: null, detailsUrl: `${url}/checks?run=3`, isRequired: false, checkSuite: { workflowRun: { workflow: { name: "CI" } } } },
  { __typename: "StatusContext", context: "docs/preview", state: "SUCCESS", targetUrl: null, createdAt: "2026-10-02T09:03:00Z", description: "Preview built", isRequired: false },
];
const lines = (count, word) => Array.from({ length: count }, (_, index) => `${word} ${index + 1}`);
/** The patch: the README edit the review thread sits on, and two long new files, so the Code tab scrolls. */
export const DETAIL_PATCH = [
  "diff --git a/README.md b/README.md", "index 1111111..2222222 100644", "--- a/README.md", "+++ b/README.md", "@@ -1,3 +1,5 @@",
  " # shared-notes", " ", "-A fixture repository.", "+A fixture repository. The notes live in `notes/`.", "+", "+See notes/plan.md for the plan.",
  "diff --git a/notes/plan.md b/notes/plan.md", "new file mode 100644", "index 0000000..3333333", "--- /dev/null", "+++ b/notes/plan.md", "@@ -0,0 +1,120 @@",
  ...lines(120, "+- Plan step"),
  "diff --git a/notes/outline.ts b/notes/outline.ts", "new file mode 100644", "index 0000000..4444444", "--- /dev/null", "+++ b/notes/outline.ts", "@@ -0,0 +1,80 @@",
  ...lines(80, "+export const section").map((line, index) => `${line.replace(/ (\d+)$/, `${index + 1}`)} = ${index + 1};`),
  "",
].join("\n");
const state = () => (live.closedAt ? "CLOSED" : "OPEN");
const commit = { oid: "a".repeat(40), messageHeadline: "Move the notes into notes/", committedDate: "2026-10-02T08:30:00Z" };

const core = (v) => ({ repository: ours(v) ? { mergeCommitAllowed: true, squashMergeAllowed: true, rebaseMergeAllowed: true, autoMergeAllowed: true, viewerPermission: "WRITE", pullRequest: {
  number: pr.number, title: pr.title, url, body: pr.body, state: state(), isDraft: false, mergeable: "MERGEABLE", mergeStateStatus: "CLEAN", reviewDecision: null, additions: 203, deletions: 1, changedFiles: 3,
  createdAt: pr.createdAt, updatedAt: live.closedAt ?? pr.updatedAt, mergedAt: null, closedAt: live.closedAt, headRefName: pr.headRefName, baseRefName: pr.baseRefName, headRefOid: commit.oid,
  isCrossRepository: false, headRepositoryOwner: { __typename: "User", login: pr.owner }, headRepository: { nameWithOwner: `${pr.owner}/${pr.name}`, viewerPermission: "WRITE" }, headRef: { id: "REF_1" },
  author: actor(pr.author, "Lane Fixture"), autoMergeRequest: null, viewerCanUpdate: true, viewerDidAuthor: false, viewerCanUpdateBranch: false, baseRef: { compare: { behindBy: 0 } },
  reviewRequests: { nodes: [] }, labels: { nodes: [] },
  commits: { nodes: [{ commit: { statusCheckRollup: { contexts: { nodes: contexts(), pageInfo: { hasNextPage: false, endCursor: null } } } } }] } } } : null });

const page = (nodes) => ({ pageInfo: { hasNextPage: false, endCursor: null }, nodes });
const activity = (v) => {
  if (!ours(v)) return { repository: null };
  const comments = [
    { id: "IC_1", body: "Thanks, this reads much better.", createdAt: "2026-10-02T09:30:00Z", lastEditedAt: null, url: `${url}#issuecomment-1`, author: actor("lane-reviewer", "Lane Reviewer") },
    ...live.comments,
  ];
  return { repository: { pullRequest: {
    ...(v.head ? { author: actor(pr.author, "Lane Fixture"), commits: { nodes: [{ commit: { ...commit, authors: { nodes: [{ name: "Lane Fixture", email: "lane@example.test", user: { login: pr.author } }] } } }] } } : {}),
    ...(v.withComments ? { comments: page(comments) } : {}),
    ...(v.withReviews ? { reviews: page([{ id: "PRR_1", body: "One question on the README.", state: "COMMENTED", submittedAt: "2026-10-02T10:00:00Z", lastEditedAt: null, url: `${url}#pullrequestreview-1`, author: actor("lane-reviewer", "Lane Reviewer") }]) } : {}),
  } } };
};
/** One unresolved thread on README.md line 3: the reviewer's finding and the author's reply under it. */
const threadComment = (id, login, body, createdAt) => ({ id, author: { __typename: "User", login, avatarUrl: "https://avatars.githubusercontent.com/u/1" }, body, createdAt, lastEditedAt: null, url: `${url}#discussion_${id}`, reactionGroups: [] });
const reviewThreads = (v) => ({ viewer: { login: pr.author }, repository: ours(v) ? { pullRequest: {
  reviewThreads: { totalCount: 1, ...page([{ id: "PRRT_1", isResolved: false, isOutdated: false, path: "README.md", line: 3, diffSide: "RIGHT", comments: { totalCount: 2, ...page([
    threadComment("PRRC_1", "lane-reviewer", "Should this line link to notes/plan.md?", "2026-10-02T10:00:00Z"),
    threadComment("PRRC_2", pr.author, "Yes, it links to it now.", "2026-10-02T10:20:00Z")]) } }]) },
  viewerCanUpdate: true, viewerDidAuthor: false, author: { __typename: "User", login: pr.author, avatarUrl: "https://avatars.githubusercontent.com/u/1" }, reactionGroups: [],
  comments: { nodes: [{ id: "IC_1", lastEditedAt: null, author: { __typename: "User", login: "lane-reviewer", avatarUrl: "https://avatars.githubusercontent.com/u/1" }, reactionGroups: [] },
    ...live.comments.map((comment) => ({ id: comment.id, lastEditedAt: null, author: { __typename: "User", login: comment.author.login, avatarUrl: comment.author.avatarUrl }, reactionGroups: [] }))] },
  reviews: { nodes: [{ id: "PRR_1", lastEditedAt: null, author: { __typename: "User", login: "lane-reviewer", avatarUrl: "https://avatars.githubusercontent.com/u/1" }, reactionGroups: [] }] },
  reviewRequests: { nodes: [] }, latestReviews: { nodes: [{ state: "COMMENTED", author: { __typename: "User", login: "lane-reviewer", avatarUrl: "https://avatars.githubusercontent.com/u/1" } }] },
  reviewDismissals: page([]),
  commits: { nodes: [{ commit: { ...commit, additions: 203, deletions: 1, parents: { totalCount: 1 }, authors: { nodes: [{ name: "Lane Fixture", avatarUrl: "https://avatars.githubusercontent.com/u/1", user: { login: pr.author } }] } } }] },
} } : null });

/** The GraphQL operations the detail panel adds, by name: each answers `data` from its variables. */
export const DETAIL_OPERATIONS = {
  PullRequestCore: core,
  PullRequestCheckContexts: (v) => ({ repository: ours(v) ? { pullRequest: { headRefOid: commit.oid, commits: { nodes: [{ commit: { statusCheckRollup: { contexts: { nodes: contexts(), pageInfo: { hasNextPage: false, endCursor: null } } } } }] } } } : null }),
  PullRequestActivity: activity,
  ReviewThreads: reviewThreads,
  ReviewDismissals: (v) => ({ repository: ours(v) ? { pullRequest: { timelineItems: page([]) } } : null }),
  ViewerPermissions: (v) => ({ repository: ours(v) ? { mergeCommitAllowed: true, squashMergeAllowed: true, rebaseMergeAllowed: true, autoMergeAllowed: true, viewerPermission: "WRITE", pullRequest: { viewerCanUpdate: true, viewerDidAuthor: false, headRepository: { viewerPermission: "WRITE" } } } : null }),
  PullRequestFilesViewed: (v) => ({ repository: ours(v) ? { pullRequest: { files: page(["README.md", "notes/plan.md", "notes/outline.ts"].map((path) => ({ path, viewerViewedState: "UNVIEWED" }))) } } : null }),
  PullRequestNodeId: (v) => ({ repository: ours(v) ? { pullRequest: { id: "PR_7" } } : null }),
  PullRequestSubject: (v) => ({ repository: ours(v) ? { pullRequest: { id: "PR_7" } } : null, node: v.subjectId === "PR_7" ? { __typename: "PullRequest", id: "PR_7" } : null }),
  ActorAvatars: () => ({ nodes: [] }),
  RoutingIdentityViewer: () => ({ viewer: { databaseId: 1, login: pr.author } }),
  PullRequestFileStats: (v) => ({ repository: ours(v) ? { pullRequest: { files: page([["README.md", 3, 1], ["notes/plan.md", 120, 0], ["notes/outline.ts", 80, 0]].map(([path, additions, deletions]) => ({ path, additions, deletions }))) } } : null }),
  AddComment: (v) => {
    if (v.subjectId !== "PR_7") return null;
    const n = live.comments.length + 2;
    live.comments.push({ id: `IC_${n}`, body: String(v.body ?? ""), createdAt: new Date().toISOString(), lastEditedAt: null, url: `${url}#issuecomment-${n}`, author: actor(pr.author, "Lane Fixture") });
    return { addComment: { clientMutationId: null } };
  },
  ClosePullRequest: (v) => { if (v.pullRequestId !== "PR_7") return null; live.closedAt = new Date().toISOString(); return { closePullRequest: { clientMutationId: null } }; },
  ReopenPullRequest: (v) => { if (v.pullRequestId !== "PR_7") return null; live.closedAt = null; return { reopenPullRequest: { clientMutationId: null } }; },
};

/** GET repos/<owner>/<name>/pulls/<n> asked as a diff: the patch; anything else there is not answered. */
export function detailRest(method, pathname, accept) {
  if (method === "GET" && pathname === `/repos/${pr.owner}/${pr.name}/pulls/${pr.number}` && /application\/vnd\.github\.(v3\.)?diff/.test(accept)) return DETAIL_PATCH;
  return null;
}
