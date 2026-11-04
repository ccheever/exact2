surfaces: web, ios
agent: Codex gpt-6-astra
task: expose transcript Markdown table coordinates

### Rough

The transcript table already had table, column-header and cell roles, but its
virtualized source coordinates were not exposed. Contract also refused the
ARIA row and column count/index attributes, so the mobile parity source could
not describe a wide table's position to assistive technology.

### Lean in

The kernel schema and Contract lowerer now carry `aria-rowcount`,
`aria-colcount`, `aria-rowindex` and `aria-colindex` as integer properties.
The web host writes them to the DOM and the web agent includes them in tree
snapshots. Thread Markdown supplies the table dimensions and one-based cell
coordinates while retaining the fixed 160-point column geometry.

### Needed

The iOS host still follows UIKit's table accessibility presentation, which
does not expose these web ARIA coordinates as separate native properties. A
paired native table drive remains needed for VoiceOver and cell-selection
acceptance; this source change does not claim that native result.

### Checkpoints

- Mobile Contract build passes with `--errors-only`.
- Focused thread tests pass: 87 tests, 493 assertions.
- Contract ARIA integration tests pass: 23 tests.
- Web host coordinate projection test passes.
