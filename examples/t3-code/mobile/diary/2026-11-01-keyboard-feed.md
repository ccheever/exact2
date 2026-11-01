surfaces: ios
agent: Codex gpt-6.1-sol
task: keep the thread feed's keyboard visible while the composer is active

### Rough

The pinned Expo ThreadFeed keeps its keyboard while a person drags the
transcript. Exact's ThreadFeed declared its own vertical pan and edge
containment but left keyboard dismissal to the platform default, so an ordinary
transcript drag could dismiss the keyboard while the composer was active.

### Lean in

The Exact thread feed now declares `keyboardDismissMode="none"`, matching the
upstream ThreadFeed source. Home keeps its separate `on-drag` policy, so Home
search and list drags continue to dismiss the keyboard as before. The existing
scroll and Home-swipe regressions now guard the thread policy alongside its
vertical axis and containment checks.

### Needed

The latest handoff still reports Send overlap with the outer Duo rail while the
keyboard is visible. A clean runtime drive is required to distinguish that
remaining route-reservation issue from keyboard dismissal; this change makes no
frame or reachability claim.

### Checkpoints

The focused scroll test passes after the Contract change. The local Duo could
not be driven because another Device panel session owns it; no simulator
acceptance or framework defect is claimed.
