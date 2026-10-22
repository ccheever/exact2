# Issue trackers

## T3 Code clone

Track clone specifications, tasks, dependencies and acceptance evidence in
[Exact T3Code Clone](https://linear.app/expo/project/exact-t3code-clone-df21cf45e26e).

- Workspace: Expo. Team: Engineering (`ENG`).
- Project: `P-ENG-624`. UUID: `e67d55c2-333a-4af2-b668-ed855a4d9efa`.
- Use the connected Linear tools. Read the project and existing issues before creating work.
- Create issues with `save_issue`, specifying the team and project. Use `parentId`
  for subtasks and `blockedBy` for dependencies.
- Read an issue and its comments before editing. Preserve labels and relations
  outside the change. Use additive label changes when available.
- Use existing statuses: Triage, Backlog, Todo, In Progress, In Review, Blocked,
  Done, Won't fix and Duplicate. Triage labels describe readiness separately.
- Claim through the coordinator before starting. Record the driving thread,
  worktree branch, owned files and acceptance criteria on the issue.
- A pushed commit enters In Review. Mark Done after the shared PR head passes
  the issue's acceptance checks. Include commit IDs and evidence links.
- When a skill says publish or fetch a ticket for clone work, use this project.
- Keep live task state in Linear. Existing app docs describe decisions and evidence;
  do not maintain a second task board in Markdown.

## Exact framework escalations

Use [ccheever/exact2 GitHub Issues](https://github.com/ccheever/exact2/issues)
for confirmed framework defects and decisions discovered during clone work.
Search existing GitHub issues and the local `issues/` records first. Link the
canonical issue from the blocked Linear task.

Use `gh` with `--repo ccheever/exact2`. For multiline issue or PR bodies, write
a temporary file and use `--body-file`. Routine bounded fixes may become
independent draft PRs after reproduction and verification. Decisions requiring
scope or authority go to an issue for the appropriate maintainer.

PRs as a request surface: no.

## Existing repository work

Other work keeps the filesystem convention in `docs/issues.md`. This setup
does not migrate existing issues. Each item has one canonical tracker; use links
for cross-tracker dependencies.

When running the triage skill, begin published triage notes with its required
AI-generated disclaimer. Read `docs/agents/triage-labels.md` for role names.
