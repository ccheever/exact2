# Source capture fails when a staged file is missing from the working tree, stopping metrics, smoke and deploy

**Status:** Open
**Systems:** scripts/deploy.mjs capture, metrics, smoke, deploy
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-02
**Related:** scripts/deploy.mjs addCapturedPaths, captureRepository

`captureRepository` (`scripts/deploy.mjs`) starts a scratch index from `HEAD`, lists
`HEAD`'s files plus everything the real index has (`ls-files --cached --others`), and
passes the whole list to `git add --all --force --pathspec-from-file`. A path that is in
the real index but is neither in `HEAD` nor on disk (status `AD`: staged as added, then
deleted) matches nothing. `git add` stops with `fatal: pathspec '<path>' did not match
any files`, and the capture refuses the whole run.

Every command that captures the source stops on it: `metrics.mjs` and `smoke.mjs`
through `withAppFixture`, and `deploy.mjs` through `snapshotOf`. This happens in the
shared main checkout whenever another session leaves an `AD` path, and it isn't about
anything the person running metrics did. Seen 2026-10-02: `bun scripts/metrics.mjs`
refused with `could not capture tracked and untracked source: fatal: pathspec
'llp/current/1075.001-native-platform-control-recommendation.rfc.md' did not match any
files` while another session had that link staged and removed.

Reproduction in a scratch repository:

```sh
git init -q r && cd r && echo a > a.txt && git add a.txt && git commit -qm init
echo b > b.txt && git add b.txt && rm b.txt          # status: AD b.txt
printf 'a.txt\0b.txt\0' > ../paths
GIT_INDEX_FILE=../tmp-index git read-tree HEAD
GIT_INDEX_FILE=../tmp-index git add --force --all --pathspec-from-file=../paths --pathspec-file-nul
# fatal: pathspec 'b.txt' did not match any files
```

Fix: leave out the inventoried paths that are neither in the scratch index nor on disk.
A staged addition the working tree no longer has is absent from the source, which is
what capturing the working tree should record.
