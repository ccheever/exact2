# Recipe: porting an app, a release build, splitting the work

**Name the port when you make it:** `exact new <path> --name "Euxy" --bundle-id com.yourcompany.euxy`,
with the original's name and bundle id if it replaces the original on a device. A bundle id
changed after the first build makes the next build cold (46 s in the euxy port) and leaves the
app installed under the old id, to delete by hand. Changing one later means app.json's `app.id`
and app.ts's `appId` together.

**Comparing a port with the original.** `agent ios --touch platform` makes each tap a real touch,
which opens UIKit's menus and reaches a native module's own controls: drive with it to compare a
port's screens with the original's ([ios-driving](ios-driving.md)). Porting a React Native app,
read [its pitfalls](../agent-pitfalls.md#porting-a-react-native-app) first.

**What ships.** `ios --release` builds what ships (`--prebuild --release` early: its first build
is minutes; `EXACT_UPDATE_TRUST=production bun exact.mjs agent ios …` drives it).

**Splitting a port among helpers:** commit the seam between the halves first (the shared types,
the native module's interface), then start each helper in the background from that commit,
owning its half and stubbing none of another's. Keep test screens off branches you merge, and
keep working while they run.
