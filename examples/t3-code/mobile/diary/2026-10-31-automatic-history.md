surfaces: ios
agent: Codex gpt-6-astra
task: automatically fetch older conversation pages

### Rough

The list's start-edge event rearms after leaving its margin. Short or filtered
pages can leave the reader inside that margin. A keyed intersection sentinel
requests the next page after settlement, including on an empty feed. One ordinary
native paging path passes. Empty and filtered-page timing remain unqualified.

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

The reader now captures the first visible row and its offset at the Root-owned
scroll boundary. An accepted history page advances a client revision, and the
Root restores that keyed row with the captured offset only when the route and
selected environment/thread still match. A same-route thread replacement
therefore cannot inherit an older reader position.

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
- iOS build: smooth at reconciled `124af97418ad28de8e3a24415609bfcbbd4da375`;
  all 3,289 recorded inputs and 92 bundle files match. Strict signature
  verification passes. The executable SHA-256 is
  `16c551abbb02021b9a0c3b62388c3f562f356d622db91066a39adf97b0c73f3b`.
- driving the app: one ordinary paging path passes on actual Duo A44 outer panel.
  The coordinator completes 12 harmless turns. Normal relaunch and reopen show
  a real `hasMoreHistory=true` boundary containing markers 03 through 12.
  Upward drags automatically reveal 02 and 01 without a Load more action.
  The reader stays near the same completed 03 text through the observed prepend
  and reaches the first row. Gesture motion prevents an exact offset claim.
  The owned session closes without shutdown. Prior core drafts remain untouched.
  Evidence is under `target/t3-mini-coordinator/evidence/history-native-acceptance/`
  in the coordinator worktree. Loading, errors, Retry, filtered-page continuation,
  selection during paging and reflow, other Duo states and performance remain open.
- reconciling with PR216 `ef3a139f34`: smooth; the remaining 14-file delta is
  automatic history. Integrated tables, Markdown reuse, first-read preferences,
  Home and passive geometry are preserved. All 400 focused tests pass with
  4,541 assertions. Strict TypeScript initially rejects the stale generated
  geometry declaration; Contract compilation regenerates it and strict types
  then pass. The failed log is retained with the successful checks under
  `target/t3-conversation-evidence/reconciliation/`. Earlier native and root
  receipts retain their original source identities. The full mobile suite passes
  4,220 tests with 37,599 assertions, no skips and both actual native bridges.
  The coordinator squash-merges the exact reviewed history patch into PR216
  as `d09f377143`. PR216 remains draft with auto-merge disabled.
