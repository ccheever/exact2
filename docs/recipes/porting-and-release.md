# Recipe: porting an app, a release build, splitting the work

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
