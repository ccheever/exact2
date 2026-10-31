surfaces: ios
agent: Codex gpt-6-astra
task: automatically fetch older conversation pages

### Rough

The list's start-edge event rearms after leaving its margin. Short or filtered
pages can leave the reader inside that margin. A keyed intersection sentinel
requests the next page after settlement, including on an empty feed. Native
timing and prepend anchors still need driven verification.

Independent review found that Stop retired the client's history request while
the presentation helper still reported it pending. The integrated regression
holds the old response across Stop and a replacement request. Both owners must
observe the same retirement before the old response settles. A history epoch
now retires both owners. The success and failure regressions failed before
that change and pass after it.

### Lean in

Contract member extraction preserved the compiled plan byte for byte before
adding the history mutation. Existing action fixtures read the extracted
member without a harness change. A separate queued mutation keeps history
delivery independent of Send and Stop.

### Needed

- pagination: by hand; full request identity and settled-page observation keys
  reject stale and duplicate work. Retry appears only after a failure.
- scrolling: provided by the virtualized keyed list; changes to the first row's
  status height and absolute-top behavior remain native acceptance cases.
- bounded history storage: missing from this unit; the reducer still retains
  every fetched page. Eight pages without displayed progress and a 64-cursor
  recent window bound continuation guards, not transcript residency.

### Checkpoints

- editing Contract and TypeScript: smooth; 283 focused tests and strict
  TypeScript pass, including the Stop retirement regressions. The full Contract
  compiles; its member extraction produced identical compiler output before
  the new history mutation was added.
- iOS build: smooth; the normal development bundle built and signed before that
  refinement. All 3,289 recorded source inputs matched their hashes and strict
  code-signature verification passed. The builder's simulator roster probe
  passed; no app installation or drive was performed.
- driving the app: pending. No visual, anchor, selection, streaming, Stop, Duo
  or release-performance acceptance is claimed by these source checks.
