# Recipe: porting an app, a release build, splitting the work

**Name the port when you make it:** `exact new <path> --name "Euxy" --bundle-id com.yourcompany.euxy`,
with the original's name and bundle id if it replaces the original on a device. A bundle id
changed after the first build makes the next build cold (46 s in the euxy port) and leaves the
app installed under the old id, to delete by hand. Changing one later means app.json's `app.id`
and app.ts's `appId` together. A new app ships Linux as well; a port of an iPhone app runs
`bun exact.mjs update --without linux` next, or each `symbol:sf/…` warns that Linux draws it blank.
It ships the Mac too, so a `tamber-unreachable` warning names a Mac control that ignores the
design (NSSlider's accent); an app that ships no Mac deletes app.json's `host.macos` and
`deploy.store.macos`, which `update --without` does not take. The SF Symbols the web draws blank
are one warning line.

**Comparing a port with the original.** `agent ios --touch platform` makes each tap a real touch,
which opens UIKit's menus and reaches a native module's own controls: drive with it to compare a
port's screens with the original's ([ios-driving](ios-driving.md)). Porting a React Native app,
read [its pitfalls](../agent-pitfalls.md#porting-a-react-native-app) first.

**What ships.** `ios --release` builds what ships; it ignores `EXACT_AGENT`, so drive the
development build. Start its first build early with `--prebuild --release`. Time launches
on a simulator no `--touch platform` drive has used: it leaves accessibility on, about 1 s slower.
To turn it off: `xcrun simctl spawn <udid> defaults write com.apple.Accessibility
AutomationEnabled -bool false`, then `xcrun simctl shutdown <udid>` and `boot <udid>`.
A launch times the installed build, never a plan an `agent ios` drive swapped in: `bun exact.mjs
ios` each variant first. `SIMCTL_CHILD_EXACT_SMOKE=1 xcrun simctl launch --console-pty <udid>
<bundle id>` prints Exact's phases to first draw (exec→main, scene, runner and layout, apply,
painted) and exits; a data module's first answer is a hook's to stamp.

**Sound, MIDI and a transport are Exact's** (`sequence`, `play(tracks…)`, `exactMIDI()`,
`sendMIDI`, `midimessage`: [the guide's "Sound, MIDI and media"](../contract-for-agents.md#sound-midi-and-media),
`apps/metronome`). A [native module](native-module.md) is for what Exact lacks; never port an
audio or MIDI engine into one.

**Splitting a port among helpers:** commit the seam between the halves first (the shared types,
the native module's interface), then start each helper in the background from that commit,
owning its half and stubbing none of another's. Keep test screens off branches you merge, and
keep working while they run.
