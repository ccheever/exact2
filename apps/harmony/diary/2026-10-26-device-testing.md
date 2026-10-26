surfaces: ios, web
agent: Codex, GPT-6
task: verify background synchronization

## Rough

A queued message reached the server while the app was hidden, but its saved job remained. Overlapping platform tasks now wait for the same invocation to finish. Relaunch also shifted periodic deadlines; storing epoch timestamps fixes that. These checks required several scheduled device runs; elapsed debugging time was not measured. A deterministic task-lifetime regression and a relaunch check now cover both failures.

## Lean in

The saved trace and persisted wake requests distinguish delivery from cleanup without reopening the app.

## Needed

- background work: provided, rough (see Rough)
- notifications: missing foreground permission-request API; appearance and tap remain unverified

## Checkpoints

- iOS build: smooth
- editing app.contract: smooth
- driving the app: rough; background scheduling requires native device evidence
