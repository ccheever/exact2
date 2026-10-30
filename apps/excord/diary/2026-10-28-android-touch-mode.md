surfaces: android
agent: Claude Code (Claude Opus 5.5)
task: Excord on an Android emulator against the fixture, and the focus ring a Back left

### Rough

- After typing in #general and going Back, the #general row in the channel list wore a
  focus ring though nothing but touches had been used. Popping the route restores the
  row's focus, and the Linux presenter, which draws Android, followed Chrome's
  `:focus-visible` rule: focus moved from a text field, which shows a ring, so the row
  showed one too. Android has touch mode instead (`View.isInTouchMode`): after a touch
  no focus is highlighted until a key is pressed. Fixed in the presenter: on Android a
  finger or pen contact, on the page or on the composer the reader hosts, puts the focus
  back in its keyless state, as Windows starts, and a route's arrival follows it. A
  hardware Enter in a hosted field does not end touch mode: the presenter cannot tell it
  from the soft keyboard's action key.
- Channel and DM rows ended in `symbol:forward-chevron`, which Android drew as the iOS
  disclosure chevron. Discord's iOS and Android channel lists have none, nor do Messages'
  conversations, and the split beside the chat already hid them: the rows now say nothing
  there.
- The newest message's two avatars were empty grey circles once the older page had
  loaded, and after scrolling far up and back other rows' were too. The Android reader
  draws each picture in a slot node that rows draw by reference, and exact2 sends a
  slot's drawing only when it changes. HWUI discards the drawing of a node that leaves
  its tree, so a slot whose rows were all freed before the row taking it over was drawn
  kept nothing; and freeing a picture dropped every slot that had ever drawn it,
  including slots that now drew another. The reader now keeps each slot's drawing and
  records it again when a row draws it empty, and a slot leaves a picture's list when
  its drawing changes (`host/android/canvas/.../Slots.kt`).
- Gradle on the build Mac kept reporting `compileReleaseKotlin` up to date after
  `git apply` changed the reader's Kotlin over ssh, so the APK carried the old code and
  two rounds of this hunt tested nothing new (QUEUE.md).

### Lean in

- `agent.mjs android --reader --app excord --storage <name>` installs the release APK and
  drives it against the fixture once `adb reverse tcp:4829 tcp:4829` is set; the
  screenshots are the device's own.

### Needed

- dialogs: Android still presents no dialog or popover its author lays out (QUEUE.md).

### Checkpoints

- android: channel list, transcript with pictures, reactions, replies and code blocks,
  and the composer with the keyboard; Back from a channel leaves no ring on its row.
