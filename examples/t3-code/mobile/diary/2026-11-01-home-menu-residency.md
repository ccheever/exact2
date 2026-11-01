surfaces: ios, web
agent: Codex gpt-6-astra
task: bound Home menu residency while preserving virtualized feeds

### Rough

The clone's `mobile/home.contract` used a keyed virtualized list, but then walked
`items` again outside that list and mounted one `HomeMenu` and one
`HomeSwipeMenu` for every row that had actions. Those popovers are outside the
list's recycling boundary, so a large Home projection retained menu subtrees for
rows that were not visible. The thread feed already stays a single keyed
`virtualized=true` list with an estimated row height of 180.

The reference source at `/Users/nate/Projects/t3-code` keeps Home and the thread
feed on LegendList surfaces. Home uses `AnimatedLegendList` with `recycleItems`,
`itemsAreEqual`, `getItemType`, `estimatedItemSize=72` and `drawDistance=1000`,
while the thread feed uses `KeyboardAwareLegendList`, fixed item types/sizes and
`drawDistance=500`. Its swipe work is admitted for visible rows rather than
creating one menu subtree per item.

### Lean in

Compact Home now has one resident context menu and one resident snooze menu per
Home surface. A row's `contextmenu` or swipe action selects its key and scoped
identity before the shared popover opens; the menu still invokes the same
`menuAction`/`chooseSwipe` path. The context handler is a compact-only wrapper;
sidebar rows retain their per-row
`data-mobile-home-menu` and native `home-menu-sidebar-list-*` identifiers, so
`T3MobileHomeMenus` keeps ownership of that path.

The source-level residency metric is bounded from up to `2N` menu component
instances per Home list before the change to exactly two shared menu hosts after
it (`N` is the projected row count). The virtualized Home and thread list
attributes remain unchanged. Home's repeated source/project resolution also uses
maps so a projection refresh does not scan every project for every row.

### Needed

- Runtime frame pacing: unavailable from this source-only pass; no simulator FPS
  or buttery-scroll claim is made.
- Native visual/menu acceptance: still needs an exclusive populated iOS drive.

### Checkpoints

- focused regression: 75 Home/swipe/scroll tests pass, including the structural
  assertion that each Home surface has one `HomeMenu` and one `HomeSwipeMenu`,
  retains sidebar native data, and keeps both Home and thread feeds virtualized.
- Contract build: `cargo run -p contract -- build examples/t3-code/app.contract`
  completes; existing unrelated compiler warnings remain.
- source profiling evidence: reference LegendList settings above; clone menu
  residency is measured directly from the authored Contract structure.

The final iOS bake also keeps each virtualized Home item under one flow root while
retaining the compact/sidebar branch, adapts the swipe-menu action to its child
arity, and uses an explicit empty environment key for the map lookup. The signed
iOS simulator build and the focused Home/thread/scroll suite pass after these
compiler-boundary fixes.

Large project filters now keep selected project references and pending replacement
membership in sets, so a refresh does not scan a project's environment keys or all
of its threads for every row.
