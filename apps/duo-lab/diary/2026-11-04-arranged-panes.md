surfaces: ios, macos, web
agent: Codex
task: present retained authored panes in native containers

### Rough

- Element hatches lacked native containment for authored children. The Apple host now accepts UIKit arrangements and reports pane allocations through containing blocks.
- Separate layout branches rebuilt pane state. The Panes screen keeps both instances mounted while switching presentation.
- Segment lengths used as flex bases shrank across the hinge when chrome reduced the container. Segmented panes now start at the viewport origin, put chrome inside Workspace and follow reading direction. Book panes use the smaller segment width to clear unequal hinges.
- AppKit adoption reset a scroller from 120 points to zero. Mac keeps CSS layout until native retention is qualified.
- Readable-region insets could leave no text gutter. Shared reading padding keeps content inside safe edges; border-box sizing includes the editor's padding and border. Recycled image rows use the same gutter.
- A focused Keyboard editor returned to the Keyboard root when switching from the folded inner display to the closed panel. This remains queued.

### Lean in

- Authored controls and bindings work in UIKit containers and the CSS fallback. Native mode controls appear only where the native module reports arrangement support.
- An icon token uses the keyboard SF Symbol on Apple and an SVG on other hosts.

### Needed

- Native pane containment: by hand, added to the UIKit element hatch
- Responsive layout: provided
- Platform navigation: provided

### Checkpoints

- Contract editing: smooth. Shared Workspace and Activity components retain drafts and counters across presentation changes and Back.
- Apple builds: earlier iOS 27.1 and macOS builds pass. An iOS 27.0 drive retains drafts using CSS layout.
- Native drive: earlier open, folded and closed drives retain pane state, independent scrolling and a confirmation across a fold. The final real-touch fold smoke remains incomplete because the simulator stopped answering process launches.
- UIKit tests: earlier focused arrangement runs pass open and closed, with a visibility skip when both peers cannot appear. The full suite remains incomplete after iframe automation failures.
- Browser drive: regression coverage checks retained panes, drafts and counters in book and laptop layouts, RTL, unequal segments and nonzero safe areas. Physical Safari support remains a separate limit in the web fold signals note.
