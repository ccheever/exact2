surfaces: web, macos, ios, linux, android
agent: Claude Code, Claude Opus 5.5
task: a fixture app that watches one place and says when you arrive

## Rough

- `button press=(on ? arriveOff : arriveOn)` was refused: "`press=` needs an
  action name or `action(args)`". One `toggle` action with an `if` inside does
  it. About 5 minutes.
- `aria-checked=(on ? "true" : "false")` was refused: "`aria-checked` takes a
  bool". `aria-checked=on` is right, and shorter.
- A grant line names a string key (`device.geolocation locationPurpose`) from
  `strings/en.json`. The first version used dotted keys
  (`purpose.location`); other fixtures use camelCase keys, so this one does too.
- An arrival's `showNotification` was refused with "the grants name no
  device.notifications": the built-in command needs its own grant line.
  `device.notifications noticePurpose` fixed it.

## Lean in

- Under the agent, `tap @<ref>` on a listed fence enters it and the next tap
  leaves it, so a drive tests an arrival without moving the machine.

## Needed

- location: provided
- geofencing: provided (`@exact/geofencing`)
- notifications: provided (the built-in `showNotification`)

## Checkpoints

- editing app.contract: rough (see Rough)
